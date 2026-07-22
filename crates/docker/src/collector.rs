//! Container listing and per-container stats via the Docker Engine API.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use bollard::Docker;
use bollard::container::{ListContainersOptions, Stats, StatsOptions};
use bollard::models::ContainerSummary;
use futures::StreamExt;
use futures::future::join_all;
use sysforge_common::collector::{Collector, CollectorError};
use sysforge_common::availability::{Availability, AvailabilityTracker};

use crate::config::DockerConfig;

/// Seconds bollard waits for the daemon before giving up.
const CONNECT_TIMEOUT_SECS: u64 = 5;

/// (cpu %, memory bytes) as measured for one running container.
type Measured = (Option<f64>, Option<u64>);

/// One container as shown in the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerInfo {
    /// Engine identifier, used for stats and log lookups.
    pub id: String,
    /// Container name without the API's leading slash.
    pub name: String,
    /// Image the container was created from.
    pub image: String,
    /// Machine state: `running`, `exited`, `paused`, ...
    pub state: String,
    /// Human status: "Up 3 hours", "Exited (0) 2 days ago", ...
    pub status: String,
    /// CPU utilization; may exceed 100% (one full core = 100%).
    /// `None` for containers that are not running.
    pub cpu_percent: Option<f64>,
    /// Memory in use, in bytes. `None` when not running.
    pub memory_usage: Option<u64>,
}

impl ContainerInfo {
    /// Whether the container is currently running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.state == "running"
    }
}

/// One reading of the Docker Engine.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DockerSnapshot {
    /// All containers (running or not), running first, then by name.
    pub containers: Vec<ContainerInfo>,
}

impl DockerSnapshot {
    /// How many containers are running.
    #[must_use]
    pub fn running(&self) -> usize {
        self.containers.iter().filter(|c| c.is_running()).count()
    }
}

/// What the collector observed about the Docker domain.
///
/// `Unavailable` is a *valid observation*, not an error: a stopped
/// daemon is a true fact about the system, and the UI renders it.
/// Samples the Docker Engine at a configurable interval.
#[derive(Debug)]
pub struct DockerCollector {
    config: DockerConfig,
    client: Option<Docker>,
    availability: AvailabilityTracker,
}

impl DockerCollector {
    /// Creates a collector from its configuration. No connection is
    /// attempted here: the socket may legitimately not exist yet.
    #[must_use]
    pub fn new(config: DockerConfig) -> Self {
        Self { config, client: None, availability: AvailabilityTracker::new("docker") }
    }

    async fn try_collect(&mut self) -> Result<DockerSnapshot, String> {
        if !Path::new(&self.config.socket).exists() {
            self.client = None;
            return Err(format!("socket not found: {}", self.config.socket));
        }

        if self.client.is_none() {
            let client = Docker::connect_with_unix(
                &self.config.socket,
                CONNECT_TIMEOUT_SECS,
                bollard::API_DEFAULT_VERSION,
            )
            .map_err(|e| format!("connect: {e}"))?;
            self.client = Some(client);
        }
        let client = self.client.as_ref().expect("client set above");

        let options = ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        };
        let summaries = client
            .list_containers(Some(options))
            .await
            .map_err(|e| format!("list containers: {e}"))?;

        let mut snapshot = snapshot_from(summaries);
        enrich_with_stats(client, &mut snapshot).await;
        Ok(snapshot)
    }
}

impl Collector for DockerCollector {
    type Output = Availability<DockerSnapshot>;

    fn name(&self) -> &'static str {
        "docker"
    }

    fn interval(&self) -> Duration {
        Duration::from_millis(self.config.interval_ms)
    }

    async fn collect(&mut self) -> Result<Availability<DockerSnapshot>, CollectorError> {
        let result = self.try_collect().await;
        Ok(self.availability.wrap(result))
    }
}

/// Fetches stats for every running container concurrently and fills
/// the snapshot in place. Failures leave the fields `None`: a container
/// that died mid-tick must not take the whole sample down.
async fn enrich_with_stats(client: &Docker, snapshot: &mut DockerSnapshot) {
    let lookups = snapshot
        .containers
        .iter()
        .filter(|c| c.is_running())
        .map(|c| {
            let id = c.id.clone();
            async move { (id.clone(), one_shot_stats(client, &id).await) }
        });
    let results: HashMap<String, Option<Measured>> = join_all(lookups).await.into_iter().collect();
    for container in &mut snapshot.containers {
        if let Some(Some((cpu, memory))) = results.get(&container.id) {
            container.cpu_percent = *cpu;
            container.memory_usage = *memory;
        }
    }
}

/// One stats reading. With `stream: false` the daemon collects two
/// samples internally and returns a single object carrying both
/// `cpu_stats` and `precpu_stats` — the delta arrives ready-made, and
/// the `Stream` never leaks past this function.
async fn one_shot_stats(client: &Docker, id: &str) -> Option<Measured> {
    let options = StatsOptions {
        stream: false,
        one_shot: false,
    };
    let stats = client.stats(id, Some(options)).next().await?.ok()?;
    Some(measure(&stats))
}

/// Extracts (cpu%, memory bytes) from a raw stats object.
fn measure(stats: &Stats) -> Measured {
    let cpu = match (
        stats.cpu_stats.system_cpu_usage,
        stats.precpu_stats.system_cpu_usage,
    ) {
        (Some(system), Some(pre_system)) => {
            let delta = stats
                .cpu_stats
                .cpu_usage
                .total_usage
                .saturating_sub(stats.precpu_stats.cpu_usage.total_usage);
            let system_delta = system.saturating_sub(pre_system);
            let online = stats.cpu_stats.online_cpus.unwrap_or(1).max(1);
            Some(cpu_percent(delta, system_delta, online))
        }
        _ => None,
    };
    (cpu, stats.memory_stats.usage)
}

/// The formula `docker stats` itself uses. May exceed 100%: a container
/// saturating two cores reads 200%.
#[allow(clippy::cast_precision_loss)] // nanosecond deltas are far below f64 precision loss
fn cpu_percent(cpu_delta: u64, system_delta: u64, online_cpus: u64) -> f64 {
    if system_delta == 0 {
        return 0.0;
    }
    cpu_delta as f64 / system_delta as f64 * online_cpus as f64 * 100.0
}

/// Pure mapping from API models to UI-ready data, unit-testable
/// without a daemon.
fn snapshot_from(summaries: Vec<ContainerSummary>) -> DockerSnapshot {
    let mut containers: Vec<ContainerInfo> = summaries
        .into_iter()
        .map(|s| ContainerInfo {
            id: s.id.unwrap_or_default(),
            name: s.names.unwrap_or_default().first().map_or_else(
                || String::from("<unnamed>"),
                |n| n.trim_start_matches('/').to_owned(),
            ),
            image: s.image.unwrap_or_default(),
            state: s.state.unwrap_or_default(),
            status: s.status.unwrap_or_default(),
            cpu_percent: None,
            memory_usage: None,
        })
        .collect();
    containers.sort_by(|a, b| {
        b.is_running()
            .cmp(&a.is_running())
            .then_with(|| a.name.cmp(&b.name))
    });
    DockerSnapshot { containers }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(name: &str, state: &str) -> ContainerSummary {
        ContainerSummary {
            id: Some(format!("id-{name}")),
            names: Some(vec![format!("/{name}")]),
            image: Some(String::from("img")),
            state: Some(state.to_owned()),
            status: Some(String::from("status")),
            ..Default::default()
        }
    }

    #[test]
    fn strips_leading_slash_from_names() {
        let snap = snapshot_from(vec![summary("web", "running")]);
        assert_eq!(snap.containers[0].name, "web");
    }

    #[test]
    fn sorts_running_first_then_by_name() {
        let snap = snapshot_from(vec![
            summary("b-stopped", "exited"),
            summary("z-up", "running"),
            summary("a-up", "running"),
        ]);
        let names: Vec<_> = snap.containers.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["a-up", "z-up", "b-stopped"]);
        assert_eq!(snap.running(), 2);
    }

    #[test]
    fn cpu_percent_follows_docker_cli_formula() {
        // container consumed 10% of the system delta on a 4-cpu host
        let pct = cpu_percent(100, 1_000, 4);
        assert!((pct - 40.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zero_system_delta_is_zero_percent() {
        assert!(cpu_percent(500, 0, 8).abs() < f64::EPSILON);
    }
}
