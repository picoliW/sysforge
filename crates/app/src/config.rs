use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub ui: UiConfig,
    pub history: HistoryConfig,
    pub collectors: CollectorsConfig,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiConfig {
    pub frame_interval_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HistoryConfig {
    pub capacity: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CollectorsConfig {
    pub memory: CollectorConfig,
    pub cpu: CollectorConfig,
}

/// Options common to every collector.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CollectorConfig {
    pub interval_ms: u64,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            frame_interval_ms: 100,
        }
    }
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self { capacity: 600 }
    }
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self { interval_ms: 1000 }
    }
}

impl UiConfig {
    #[must_use]
    pub fn frame_interval(&self) -> Duration {
        Duration::from_millis(self.frame_interval_ms)
    }
}

impl CollectorConfig {
    #[must_use]
    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.interval_ms)
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        let config = match std::fs::read_to_string(&path) {
            Ok(raw) => {
                toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {
                tracing::info!(path = %path.display(), "no config file, using defaults");
                Self::default()
            }
            Err(e) => {
                return Err(e).with_context(|| format!("reading {}", path.display()));
            }
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.ui.frame_interval_ms < 10 {
            bail!("ui.frame_interval_ms must be at least 10 (100 fps)");
        }
        for (name, collector) in [
            ("memory", &self.collectors.memory),
            ("cpu", &self.collectors.cpu),
        ] {
            if collector.interval_ms < 100 {
                bail!("collectors.{name}.interval_ms must be at least 100");
            }
        }
        if self.history.capacity < 10 {
            bail!("history.capacity must be at least 10");
        }
        Ok(())
    }
}

fn config_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("", "", "sysforge")
        .context("could not determine a home directory for this user")?;
    Ok(dirs.config_dir().join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_all_defaults() {
        let config: Config = toml::from_str("").expect("empty toml must parse");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn partial_file_overrides_only_what_it_mentions() {
        let config: Config = toml::from_str("[collectors.cpu]\ninterval_ms = 2000\n")
            .expect("partial toml must parse");
        assert_eq!(config.collectors.cpu.interval_ms, 2000);
        assert_eq!(config.collectors.memory, CollectorConfig::default());
        assert_eq!(config.ui, UiConfig::default());
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(toml::from_str::<Config>("[ui]\nframe_intervall_ms = 50\n").is_err());
    }

    #[test]
    fn hostile_values_fail_validation() {
        let config: Config = toml::from_str("[ui]\nframe_interval_ms = 1\n").expect("parses");
        assert!(config.validate().is_err());
    }
}
