//! Service listing via `systemctl --output=json`.

use std::time::Duration;

use serde::Deserialize;
use sysforge_common::availability::{Availability, AvailabilityTracker};
use sysforge_common::collector::{Collector, CollectorError};
use tokio::process::Command;

/// Activation state of a service, grouped for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    /// Running (`active`).
    Active,
    /// Stopped without error (`inactive`).
    Inactive,
    /// Failed (`failed`).
    Failed,
    /// Any other state (`activating`, `deactivating`, ...).
    Other,
}

impl ServiceState {
    fn from_active(active: &str) -> Self {
        match active {
            "active" => Self::Active,
            "inactive" => Self::Inactive,
            "failed" => Self::Failed,
            _ => Self::Other,
        }
    }

    /// Sort rank: failed first, then active, then the rest.
    fn rank(self) -> u8 {
        match self {
            Self::Failed => 0,
            Self::Active => 1,
            Self::Other => 2,
            Self::Inactive => 3,
        }
    }
}

/// One service unit as shown in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceInfo {
    /// Unit name (`ssh.service`).
    pub name: String,
    /// Grouped activation state.
    pub state: ServiceState,
    /// Low-level state (`running`, `dead`, `exited`, ...).
    pub sub: String,
    /// Human description.
    pub description: String,
}

/// One reading of the service table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemdSnapshot {
    /// Services: failed first, then active, then the rest; by name within.
    pub services: Vec<ServiceInfo>,
    /// How many are active.
    pub active: usize,
    /// How many have failed.
    pub failed: usize,
}

/// Shape of one entry in `systemctl --output=json`.
#[derive(Debug, Deserialize)]
struct RawUnit {
    unit: String,
    active: String,
    sub: String,
    description: String,
}

/// Samples systemd services at a configurable interval.
#[derive(Debug)]
pub struct SystemdCollector {
    interval: Duration,
    availability: AvailabilityTracker,
}

impl SystemdCollector {
    /// Creates a collector sampling at the given interval.
    #[must_use]
    pub fn new(interval: Duration) -> Self {
        Self { interval, availability: AvailabilityTracker::new("systemd") }
    }

    async fn try_collect(&self) -> Result<SystemdSnapshot, String> {
        let output = Command::new("systemctl")
            .args(["list-units", "--type=service", "--all", "--output=json"])
            .output()
            .await
            .map_err(|e| format!("running systemctl: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "systemctl failed: {}",
                stderr.lines().next().unwrap_or("unknown error").trim()
            ));
        }

        parse_units(&output.stdout).map_err(|e| format!("parsing systemctl output: {e}"))
    }
}

impl Collector for SystemdCollector {
    type Output = Availability<SystemdSnapshot>;

    fn name(&self) -> &'static str {
        "systemd"
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    async fn collect(&mut self) -> Result<Availability<SystemdSnapshot>, CollectorError> {
        let result = self.try_collect().await;
        Ok(self.availability.wrap(result))
    }
}

/// Pure parser over `systemctl --output=json` bytes, unit-testable
/// without systemd.
fn parse_units(json: &[u8]) -> Result<SystemdSnapshot, serde_json::Error> {
    let raw: Vec<RawUnit> = serde_json::from_slice(json)?;

    let mut services: Vec<ServiceInfo> = raw
        .into_iter()
        .map(|u| ServiceInfo {
            name: u.unit,
            state: ServiceState::from_active(&u.active),
            sub: u.sub,
            description: u.description,
        })
        .collect();

    let active = services.iter().filter(|s| s.state == ServiceState::Active).count();
    let failed = services.iter().filter(|s| s.state == ServiceState::Failed).count();

    services.sort_by(|a, b| {
        a.state.rank().cmp(&b.state.rank()).then_with(|| a.name.cmp(&b.name))
    });

    Ok(SystemdSnapshot { services, active, failed })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = br#"[
        {"unit":"ssh.service","load":"loaded","active":"active","sub":"running","description":"OpenSSH server"},
        {"unit":"nginx.service","load":"loaded","active":"failed","sub":"failed","description":"nginx"},
        {"unit":"cron.service","load":"loaded","active":"inactive","sub":"dead","description":"cron"}
    ]"#;

    #[test]
    fn parses_and_counts() {
        let snap = parse_units(SAMPLE).expect("sample must parse");
        assert_eq!(snap.services.len(), 3);
        assert_eq!(snap.active, 1);
        assert_eq!(snap.failed, 1);
    }

    #[test]
    fn failed_services_sort_first() {
        let snap = parse_units(SAMPLE).expect("sample must parse");
        assert_eq!(snap.services[0].name, "nginx.service");
        assert_eq!(snap.services[0].state, ServiceState::Failed);
    }

    #[test]
    fn malformed_json_is_an_error() {
        assert!(parse_units(b"not json").is_err());
    }
}
