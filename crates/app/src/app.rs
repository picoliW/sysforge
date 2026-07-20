use std::sync::Arc;

use anyhow::Result;
use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use sysforge_common::collector::Collector;
use sysforge_disk::collector::DiskCollector;
use sysforge_docker::collector::DockerCollector;
use sysforge_git::collector::GitCollector;
use sysforge_network::collector::NetworkCollector;
use sysforge_system::cpu::CpuCollector;
use sysforge_system::memory::MemoryCollector;
use sysforge_system::process::ProcessCollector;

use tokio::sync::mpsc;

use crate::config::Config;
use crate::history::History;
use crate::input::{self, Action};
use crate::render;
use crate::state::{AppState, DockerUiState, GitUiState, SharedState};
use crate::terminal::Tui;
use crate::ui::{Command, UiEvent, UiState};

pub async fn run(terminal: &mut Tui, config: &Config) -> Result<()> {
    let state: SharedState = Arc::new(std::sync::RwLock::new(AppState::new(
        config.history.capacity,
        config.docker.enabled,
        config.git.enabled,
    )));

    spawn_collectors(&state, config);

    let (ui_events_tx, mut ui_events) = mpsc::unbounded_channel::<UiEvent>();
    let mut ui = UiState::default();
    let mut events = EventStream::new();
    let mut frame_timer = tokio::time::interval(config.ui.frame_interval());

    loop {
        tokio::select! {
                    _ = frame_timer.tick() => {
                        let snapshot = state.read().map(|s| s.clone()).unwrap_or_default();
        terminal.draw(|frame| render::render(frame, &snapshot, &ui, &config.theme))?;            }
                    Some(event) = ui_events.recv() => {
                        ui.apply_event(event);
                    }
                    Some(Ok(event)) = events.next() => {
                        if let Event::Key(key) = event {
                            if let Some(action) = input::action_for(key) {
                                if action == Action::Quit {
                                    return Ok(());
                                }
                                let snapshot =
                                    state.read().map(|s| s.clone()).unwrap_or_default();
                                if let Some(command) = ui.handle(action, &snapshot) {
                                    execute(command, config, ui_events_tx.clone());
                                }
                            }
                        }
                    }
                }
    }
}

fn execute(command: Command, config: &Config, events: mpsc::UnboundedSender<UiEvent>) {
    match command {
        Command::FetchDockerLogs { id } => {
            let socket = config.docker.socket.clone();
            tokio::spawn(async move {
                let lines = match sysforge_docker::logs::fetch_logs(&socket, &id).await {
                    Ok(lines) if lines.is_empty() => {
                        vec![String::from("(no log output)")]
                    }
                    Ok(lines) => lines,
                    Err(reason) => vec![format!("failed to fetch logs: {reason}")],
                };
                let _ = events.send(UiEvent::OverlayContent { lines });
            });
        }
    }
}

fn spawn_collector<C, F>(mut collector: C, state: SharedState, apply: F)
where
    C: Collector,
    F: Fn(&mut AppState, C::Output) + Send + 'static,
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

/// Starts every enabled collector on its own task.
fn spawn_collectors(state: &SharedState, config: &Config) {
    spawn_collector(
        MemoryCollector::new(config.collectors.memory.interval()),
        Arc::clone(state),
        |s, snap| {
            s.memory_history.push_percent(snap.used_percent());
            s.memory = Some(snap);
        },
    );

    spawn_collector(
        CpuCollector::new(config.collectors.cpu.interval()),
        Arc::clone(state),
        |s, snap| {
            if let Some(snap) = snap {
                s.cpu_history.push_percent(snap.total);
                s.cpu = Some(snap);
            }
        },
    );

    spawn_collector(
        ProcessCollector::new(config.collectors.processes.interval()),
        Arc::clone(state),
        |s, snap| {
            s.processes = Some(snap);
        },
    );

    if config.docker.enabled {
        spawn_collector(
            DockerCollector::new(config.docker.clone()),
            Arc::clone(state),
            |s, status| {
                s.docker = DockerUiState::Observed(status);
            },
        );
    }

    if config.git.enabled {
        spawn_collector(
            GitCollector::new(config.git.clone()),
            Arc::clone(state),
            |s, status| {
                s.git = GitUiState::Observed(status);
            },
        );
    }

    if config.network.enabled {
        let capacity = config.history.capacity;
        spawn_collector(
            NetworkCollector::new(config.network.interval()),
            Arc::clone(state),
            move |s, snap| {
                for iface in &snap.interfaces {
                    s.network_history
                        .entry(iface.name.clone())
                        .or_insert_with(|| History::new(capacity))
                        .push_rate(iface.total_rate());
                }
                s.network = Some(snap);
            },
        );
    }

    if config.disk.enabled {
        let capacity = config.history.capacity;
        spawn_collector(
            DiskCollector::new(config.disk.interval()),
            Arc::clone(state),
            move |s, snap| {
                for device in &snap.devices {
                    s.disk_history
                        .entry(device.name.clone())
                        .or_insert_with(|| History::new(capacity))
                        .push_rate(device.total_rate());
                }
                s.disk = Some(snap);
            },
        );
    }
}
