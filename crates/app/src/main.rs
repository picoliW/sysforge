mod app;
mod logging;
mod state;
mod terminal;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = logging::init()?;
    terminal::install_panic_hook();

    let mut guard = terminal::TerminalGuard::new()?;
    let result = app::run(guard.terminal()).await;
    drop(guard); 

    if let Err(err) = &result {
        tracing::error!(error = %err, "sysforge exited with error");
    }
    result
}