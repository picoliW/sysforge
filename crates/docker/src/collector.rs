use std::path::Path;
use std::time::Duration;

use bollard::Docker;
use bollard::container::ListContainersOptions;
use bollard::models::ContainerSummary;
use sysforge_common::collector::{Collector, CollectorError};

use crate::config::DockerConfig;

const CONNECT_TIMEOUT_SECS: u64 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerInfo {
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
}

impl ContainerInfo {
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.state == "running"
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DockerSnapshot {
    pub containers: Vec<ContainerInfo>,
}

impl DockerSnapshot {
    #[must_use]
    pub fn running(&self) -> usize {
        self.containers.iter().filter(|c| c.is_running()).count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockerStatus {
    Available(DockerSnapshot),
    Unavailable { reason: String },
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
        Self {
            config,
            client: None,
            was_available: None,
        }
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

        Ok(snapshot_from(summaries))
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

fn snapshot_from(summaries: Vec<ContainerSummary>) -> DockerSnapshot {
    let mut containers: Vec<ContainerInfo> = summaries
        .into_iter()
        .map(|s| ContainerInfo {
            name: s
                .names
                .unwrap_or_default()
                .first()
                .map(|n| n.trim_start_matches('/').to_owned())
                .unwrap_or_else(|| String::from("<unnamed>")),
            image: s.image.unwrap_or_default(),
            state: s.state.unwrap_or_default(),
            status: s.status.unwrap_or_default(),
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
}
