mod app;
mod state;
mod terminal;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    terminal::install_panic_hook();

    let mut guard = terminal::TerminalGuard::new()?;
    let result = app::run(guard.terminal()).await;
    drop(guard);

    result
}