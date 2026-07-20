//! The `[docker]` section of the user configuration.
//!
//! Lives in this crate — the domain owns its configuration contract;
//! the application merely composes it.

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
/// Docker collector options (`[docker]`).
pub struct DockerConfig {
    /// Whether the Docker collector runs at all (default `true`).
    pub enabled: bool,
    /// Path to the Docker Engine unix socket.
    pub socket: String,
    /// Milliseconds between samples (default 2000).
    pub interval_ms: u64,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            socket: String::from("/var/run/docker.sock"),
            interval_ms: 2000,
        }
    }
}
