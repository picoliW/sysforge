//! The `[disk]` section of the user configuration.

use std::time::Duration;

use serde::Deserialize;

/// Disk collector options (`[disk]`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DiskConfig {
    /// Whether the disk collector runs at all (default `true`).
    pub enabled: bool,
    /// Milliseconds between samples (default 1000).
    pub interval_ms: u64,
}

impl Default for DiskConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_ms: 1000,
        }
    }
}

impl DiskConfig {
    /// Sampling interval as a [`Duration`].
    #[must_use]
    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.interval_ms)
    }
}
