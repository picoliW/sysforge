//! SysForge entry point: installs logging and the panic hook, loads
//! configuration, builds the terminal guard and hands control to the
//! application loop.

mod app;
mod config;
mod history;
mod input;
mod logging;
mod render;
mod state;
mod terminal;
mod theme;
mod ui;

use anyhow::Result;

use crate::config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = logging::init()?;
    let config = Config::load()?;
    terminal::install_panic_hook();

    let mut guard = terminal::TerminalGuard::new()?;
    let result = app::run(guard.terminal(), &config).await;
    drop(guard);

    if let Err(err) = &result {
        tracing::error!(error = %err, "sysforge exited with error");
    }
    result
}
