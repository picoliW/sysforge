use std::sync::Arc;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Gauge, Paragraph, Row, Sparkline, Table, Wrap};
use sysforge_common::collector::Collector;
use sysforge_docker::collector::{DockerCollector, DockerStatus};
use sysforge_system::cpu::CpuCollector;
use sysforge_system::memory::MemoryCollector;

use crate::config::Config;
use crate::history::History;
use crate::state::{AppState, DockerUiState, SharedState};
use crate::terminal::Tui;

pub async fn run(terminal: &mut Tui, config: &Config) -> Result<()> {
    let state: SharedState = Arc::new(std::sync::RwLock::new(AppState::new(
        config.history.capacity,
        config.docker.enabled,
    )));

    if config.docker.enabled {
        spawn_collector(
            DockerCollector::new(config.docker.clone()),
            Arc::clone(&state),
            |s, status| {
                s.docker = DockerUiState::Observed(status);
            },
        );
    }

    spawn_collector(
        MemoryCollector::new(config.collectors.memory.interval()),
        Arc::clone(&state),
        |s, snap| {
            s.memory_history.push_percent(snap.used_percent());
            s.memory = Some(snap);
        },
    );

    spawn_collector(
        CpuCollector::new(config.collectors.cpu.interval()),
        Arc::clone(&state),
        |s, snap| {
            if let Some(snap) = snap {
                s.cpu_history.push_percent(snap.total);
                s.cpu = Some(snap);
            }
        },
    );

    let mut events = EventStream::new();
    let mut frame_timer = tokio::time::interval(config.ui.frame_interval());

    loop {
        tokio::select! {
            _ = frame_timer.tick() => {
                            let snapshot = state.read().map(|s| s.clone()).unwrap_or_default();
                            terminal.draw(|frame| render(frame, &snapshot))?;
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

fn spawn_collector<C>(mut collector: C, state: SharedState, apply: fn(&mut AppState, C::Output))
where
    C: Collector,
{
    tokio::spawn(async move {
        tracing::info!(collector = collector.name(), "collector started");
        let mut timer = tokio::time::interval(collector.interval());
        loop {
            timer.tick().await;
            match collector.collect().await {
                Ok(sample) => {
                    if let Ok(mut s) = state.write() {
                        apply(&mut s, sample);
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        collector = collector.name(),
                        error = %err,
                        "collector failed",
                    );
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

fn render(frame: &mut Frame, state: &AppState) {
    if state.docker == DockerUiState::Disabled {
        let [cpu_area, mem_area] =
            Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)])
                .areas(frame.area());
        render_cpu(frame, cpu_area, state);
        render_memory(frame, mem_area, state);
        return;
    }

    let [cpu_area, mem_area, docker_area] = Layout::vertical([
        Constraint::Percentage(30),
        Constraint::Percentage(25),
        Constraint::Min(0),
    ])
    .areas(frame.area());
    render_cpu(frame, cpu_area, state);
    render_memory(frame, mem_area, state);
    render_docker(frame, docker_area, state);
}

fn render_cpu(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = panel_block(" CPU ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(cpu) = &state.cpu else {
        frame.render_widget(Paragraph::new("sampling..."), inner);
        return;
    };

    let [gauge_area, spark_area, cores_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .margin(1)
    .areas(inner);

    frame.render_widget(percent_gauge(cpu.total), gauge_area);
    render_sparkline(frame, spark_area, &state.cpu_history);

    let cores = cpu
        .per_core
        .iter()
        .enumerate()
        .map(|(i, pct)| format!("c{i:02} {pct:5.1}%"))
        .collect::<Vec<_>>()
        .join("   ");
    frame.render_widget(Paragraph::new(cores).wrap(Wrap { trim: true }), cores_area);
}

fn render_memory(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = panel_block(" Memory ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(mem) = state.memory else {
        frame.render_widget(Paragraph::new("sampling..."), inner);
        return;
    };

    let [top_area, spark_area] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)])
        .margin(1)
        .areas(inner);

    let [gauge_area, details_area] =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Min(0)])
            .spacing(2)
            .areas(top_area);

    frame.render_widget(percent_gauge(mem.used_percent()), gauge_area);
    frame.render_widget(
        Paragraph::new(format!(
            "used {} / {}   swap {} / {}",
            format_bytes(mem.used()),
            format_bytes(mem.total),
            format_bytes(mem.swap_used()),
            format_bytes(mem.swap_total),
        )),
        details_area,
    );

    render_sparkline(frame, spark_area, &state.memory_history);
}

fn render_sparkline(frame: &mut Frame, area: Rect, history: &History) {
    let data = history.last(area.width as usize);
    let spark = Sparkline::default()
        .data(&data)
        .max(100)
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(spark, area);
}

fn render_docker(frame: &mut Frame, area: Rect, state: &AppState) {
    let (title, body) = match &state.docker {
        DockerUiState::Disabled => return,
        DockerUiState::Pending => (String::from(" Docker "), None),
        DockerUiState::Observed(DockerStatus::Unavailable { reason }) => {
            (String::from(" Docker ─ offline "), Some(reason.clone()))
        }
        DockerUiState::Observed(DockerStatus::Available(snap)) => {
            let title = format!(
                " Docker ({}/{} running) ",
                snap.running(),
                snap.containers.len()
            );
            let block = panel_block(&title);
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let header = Row::new(["NAME", "IMAGE", "STATE", "STATUS"])
                .style(Style::default().add_modifier(Modifier::BOLD));
            let rows = snap.containers.iter().map(|c| {
                let style = if c.is_running() {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                Row::new([
                    c.name.clone(),
                    c.image.clone(),
                    c.state.clone(),
                    c.status.clone(),
                ])
                .style(style)
            });
            let table = Table::new(
                rows,
                [
                    Constraint::Percentage(25),
                    Constraint::Percentage(30),
                    Constraint::Length(10),
                    Constraint::Min(0),
                ],
            )
            .header(header)
            .column_spacing(2);
            frame.render_widget(table, inner);
            return;
        }
    };

    let block = panel_block(&title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(body.unwrap_or_else(|| String::from("sampling...")))
            .style(Style::default().fg(Color::DarkGray)),
        inner,
    );
}

fn panel_block(title: &str) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
}

fn percent_gauge(percent: f64) -> Gauge<'static> {
    Gauge::default()
        .ratio((percent / 100.0).clamp(0.0, 1.0))
        .label(format!("{percent:.1}%"))
        .gauge_style(Style::default().fg(Color::Cyan))
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
