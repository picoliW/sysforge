use std::io::{self, Stdout};

use anyhow::Result;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub struct TerminalGuard {
    terminal: Tui,
}

impl TerminalGuard {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok(Self { terminal })
    }

    pub fn terminal(&mut self) -> &mut Tui {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore();
    }
}

fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
}

pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        tracing::error!(panic = %info, "panic");
        default_hook(info);
    }));
}