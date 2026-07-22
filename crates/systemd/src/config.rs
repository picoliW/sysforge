//! The `[systemd]` section of the user configuration.

use std::time::Duration;

use serde::Deserialize;

/// systemd collector options (`[systemd]`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SystemdConfig {
    /// Whether the systemd collector runs at all (default `true`).
    pub enabled: bool,
    /// Milliseconds between samples (default 3000).
    pub interval_ms: u64,
}

impl Default for SystemdConfig {
    fn default() -> Self {
        Self { enabled: true, interval_ms: 3000 }
    }
}

impl SystemdConfig {
    /// Sampling interval as a [`Duration`].
    #[must_use]
    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.interval_ms)
    }
}
