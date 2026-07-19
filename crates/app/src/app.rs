use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, BorderType, Borders, Gauge, Paragraph};
use sysforge_common::collector::Collector;
use sysforge_system::memory::{MemoryCollector, MemorySnapshot};

use crate::state::{AppState, SharedState};
use crate::terminal::Tui;

const FRAME_INTERVAL: Duration = Duration::from_millis(100); // ~10 fps

pub async fn run(terminal: &mut Tui) -> Result<()> {
    let state: SharedState = Arc::default();

    spawn_collector(MemoryCollector, Arc::clone(&state), |s, snap| {
        s.memory = Some(snap);
    });

    let mut events = EventStream::new();
    let mut frame_timer = tokio::time::interval(FRAME_INTERVAL);

    loop {
        tokio::select! {
            _ = frame_timer.tick() => {
                let memory = state.read().map(|s| s.memory).unwrap_or_default();
                terminal.draw(|frame| render(frame, memory))?;
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


fn spawn_collector<C>(
    mut collector: C,
    state: SharedState,
    apply: fn(&mut AppState, C::Output),
) where
    C: Collector,
{
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(collector.interval());
        loop {
            timer.tick().await;
            match collector.collect().await {
                Ok(sample) => {
                    if let Ok(mut s) = state.write() {
                        apply(&mut s, sample);
                    }
                }
                Err(_err) => {

                }
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

fn render(frame: &mut Frame, memory: Option<MemorySnapshot>) {
    let outer = Block::default()
        .title(" SysForge ─ Memory ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = outer.inner(frame.area());
    frame.render_widget(outer, frame.area());

    let Some(mem) = memory else {
        frame.render_widget(Paragraph::new("sampling..."), inner);
        return;
    };

    let [gauge_area, details_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)])
            .margin(1)
            .areas(inner);

    let gauge = Gauge::default()
        .ratio((mem.used_percent() / 100.0).clamp(0.0, 1.0))
        .label(format!("{:.1}%", mem.used_percent()))
        .gauge_style(Style::default().fg(Color::Cyan));
    frame.render_widget(gauge, gauge_area);

    let details = Paragraph::new(format!(
        "used {} / {}   swap {} / {}",
        format_bytes(mem.used()),
        format_bytes(mem.total),
        format_bytes(mem.swap_used()),
        format_bytes(mem.swap_total),
    ));
    frame.render_widget(details, details_area);
}

#[allow(clippy::cast_precision_loss)]
fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}