//! The `[network]` section of the user configuration.

use serde::Deserialize;
use std::time::Duration;

/// Network collector options (`[network]`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkConfig {
    /// Whether the network collector runs at all (default `true`).
    pub enabled: bool,
    /// Milliseconds between samples (default 1000).
    pub interval_ms: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_ms: 1000,
        }
    }
}

impl NetworkConfig {
    /// Sampling interval as a [`Duration`].
    #[must_use]
    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.interval_ms)
    }
}
