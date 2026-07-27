//! The `[k8s]` section of the user configuration.

use std::time::Duration;

use serde::Deserialize;

/// Kubernetes collector options (`[k8s]`).
///
/// Unlike the local domains, Kubernetes is **disabled by default**:
/// most machines have no cluster configured, and enabling it would make
/// SysForge try to reach a cluster that isn't there on every start.
/// Users with a cluster opt in.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct K8sConfig {
    /// Whether the Kubernetes collector runs at all (default `false`).
    pub enabled: bool,
    /// Milliseconds between snapshots of the locally watched state
    /// (default 3000). No API call happens per tick: the network side
    /// is a continuous watch that only carries deltas.
    pub interval_ms: u64,
}

impl Default for K8sConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_ms: 3000,
        }
    }
}

impl K8sConfig {
    /// Sampling interval as a [`Duration`].
    #[must_use]
    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.interval_ms)
    }
}
