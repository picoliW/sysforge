use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::Frame;
use ratatui::layout::Alignment;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::state::{AppState, SharedState};
use crate::terminal::Tui;

const FRAME_INTERVAL: Duration = Duration::from_millis(100);
const DEMO_TICK: Duration = Duration::from_millis(500);

pub async fn run(terminal: &mut Tui) -> Result<()> {
    let state: SharedState = Arc::default();

    spawn_demo_collector(Arc::clone(&state));

    let mut events = EventStream::new();
    let mut frame_timer = tokio::time::interval(FRAME_INTERVAL);

    loop {
        tokio::select! {
            _ = frame_timer.tick() => {
                let ticks = state.read().map(|s| s.ticks).unwrap_or_default();
                terminal.draw(|frame| render(frame, ticks))?;
            }
            Some(Ok(event)) = events.next() => {
                if let Event::Key(key) = event {
                    if should_quit(key) {
                        return Ok(());
                    }
                }
            }
        }
    }
}

fn spawn_demo_collector(state: SharedState) {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(DEMO_TICK);
        loop {
            timer.tick().await;
            if let Ok(mut s) = state.write() {
                s.ticks += 1;
            }
        }
    });
}

fn should_quit(key: KeyEvent) -> bool {
    if key.kind != KeyEventKind::Press {
        return false;
    }
    matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

fn render(frame: &mut Frame, ticks: u64) {
    let block = Block::default()
        .title(" SysForge ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let content = Paragraph::new(format!(
        "collector heartbeat: {ticks}\n\npress q to quit"
    ))
    .alignment(Alignment::Center)
    .block(block);

    frame.render_widget(content, frame.area());
}

#[allow(unused)]
fn _doc_anchor(_: &AppState) {}