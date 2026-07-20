use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DockerConfig {
    pub enabled: bool,
    pub socket: String,
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
