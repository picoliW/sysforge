//! The `[git]` section of the user configuration.

use serde::Deserialize;

/// Git collector options (`[git]`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GitConfig {
    /// Whether the Git collector runs at all (default `true`).
    pub enabled: bool,
    /// Repository path. Empty means the process's working directory.
    pub repo_path: String,
    /// Milliseconds between samples (default 3000).
    pub interval_ms: u64,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            repo_path: String::new(),
            interval_ms: 3000,
        }
    }
}
