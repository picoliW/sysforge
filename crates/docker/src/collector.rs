use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use bollard::Docker;
use bollard::container::{ListContainersOptions, Stats, StatsOptions};
use bollard::models::ContainerSummary;
use futures::StreamExt;
use futures::future::join_all;
use sysforge_common::collector::{Collector, CollectorError};

use crate::config::DockerConfig;

const CONNECT_TIMEOUT_SECS: u64 = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub cpu_percent: Option<f64>,
    pub memory_usage: Option<u64>,
}

impl ContainerInfo {
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.state == "running"
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DockerSnapshot {
    pub containers: Vec<ContainerInfo>,
}

impl DockerSnapshot {
    #[must_use]
    pub fn running(&self) -> usize {
        self.containers.iter().filter(|c| c.is_running()).count()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DockerStatus {
    Available(DockerSnapshot),
    Unavailable {
        reason: String,
    },
}

#[derive(Debug)]
pub struct DockerCollector {
    config: DockerConfig,
    client: Option<Docker>,
    was_available: Option<bool>,
}

impl DockerCollector {
    #[must_use]
    pub fn new(config: DockerConfig) -> Self {
        Self { config, client: None, was_available: None }
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

        let options = ListContainersOptions::<String> { all: true, ..Default::default() };
        let summaries = client
            .list_containers(Some(options))
            .await
            .map_err(|e| format!("list containers: {e}"))?;

        let mut snapshot = snapshot_from(summaries);
        enrich_with_stats(client, &mut snapshot).await;
        Ok(snapshot)
    }

    fn note_availability(&mut self, up: bool, detail: &str) {
        if self.was_available == Some(up) {
            return;
        }
        if up {
            tracing::info!("docker available");
        } else {
            tracing::warn!(reason = detail, "docker unavailable");
        }
        self.was_available = Some(up);
    }
}

impl Collector for DockerCollector {
    type Output = DockerStatus;

    fn name(&self) -> &'static str {
        "docker"
    }

    fn interval(&self) -> Duration {
        Duration::from_millis(self.config.interval_ms)
    }

    async fn collect(&mut self) -> Result<DockerStatus, CollectorError> {
        match self.try_collect().await {
            Ok(snapshot) => {
                self.note_availability(true, "");
                Ok(DockerStatus::Available(snapshot))
            }
            Err(reason) => {
                self.note_availability(false, &reason);
                Ok(DockerStatus::Unavailable { reason })
            }
        }
    }
}

async fn enrich_with_stats(client: &Docker, snapshot: &mut DockerSnapshot) {
    let lookups = snapshot
        .containers
        .iter()
        .filter(|c| c.is_running())
        .map(|c| {
            let id = c.id.clone();
            async move { (id.clone(), one_shot_stats(client, &id).await) }
        });
    let results: HashMap<String, Option<(Option<f64>, Option<u64>)>> =
        join_all(lookups).await.into_iter().collect();

    for container in &mut snapshot.containers {
        if let Some(Some((cpu, memory))) = results.get(&container.id) {
            container.cpu_percent = *cpu;
            container.memory_usage = *memory;
        }
    }
}

async fn one_shot_stats(
    client: &Docker,
    id: &str,
) -> Option<(Option<f64>, Option<u64>)> {
    let options = StatsOptions { stream: false, one_shot: false };
    let stats = client.stats(id, Some(options)).next().await?.ok()?;
    Some(measure(&stats))
}

fn measure(stats: &Stats) -> (Option<f64>, Option<u64>) {
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

#[allow(clippy::cast_precision_loss)] 
fn cpu_percent(cpu_delta: u64, system_delta: u64, online_cpus: u64) -> f64 {
    if system_delta == 0 {
        return 0.0;
    }
    cpu_delta as f64 / system_delta as f64 * online_cpus as f64 * 100.0
}

fn snapshot_from(summaries: Vec<ContainerSummary>) -> DockerSnapshot {
    let mut containers: Vec<ContainerInfo> = summaries
        .into_iter()
        .map(|s| ContainerInfo {
            id: s.id.unwrap_or_default(),
            name: s
                .names
                .unwrap_or_default()
                .first()
                .map(|n| n.trim_start_matches('/').to_owned())
                .unwrap_or_else(|| String::from("<unnamed>")),
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
        let pct = cpu_percent(100, 1_000, 4);
        assert!((pct - 40.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zero_system_delta_is_zero_percent() {
        assert!(cpu_percent(500, 0, 8).abs() < f64::EPSILON);
    }
}