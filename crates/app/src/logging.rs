use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

pub fn init() -> Result<WorkerGuard> {
    let dir = log_dir()?;
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("creating log directory {}", dir.display()))?;

    let appender = tracing_appender::rolling::daily(&dir, "sysforge.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "sysforge started");
    Ok(guard)
}

fn log_dir() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("", "", "sysforge")
        .context("could not determine a home directory for this user")?;
    Ok(dirs
        .state_dir()
        .unwrap_or_else(|| dirs.data_local_dir())
        .to_path_buf())
}
