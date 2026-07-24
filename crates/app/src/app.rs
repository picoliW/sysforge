use std::sync::Arc;

use anyhow::Result;
use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use sysforge_common::collector::Collector;
use sysforge_common::domain_state::DomainState;
use sysforge_disk::collector::DiskCollector;
use sysforge_docker::collector::DockerCollector;
use sysforge_git::collector::GitCollector;
use sysforge_network::collector::NetworkCollector;
use sysforge_system::cpu::CpuCollector;
use sysforge_system::memory::MemoryCollector;
use sysforge_system::process::ProcessCollector;
use sysforge_systemd::collector::SystemdCollector;

use tokio::sync::mpsc;

use crate::config::Config;
use crate::history::History;
use crate::input::{self, Action};
use crate::render;
use crate::state::{AppState, SharedState};
use crate::terminal::Tui;
use crate::ui::{ActionCommand, ActionOutcome, Command, UiEvent, UiState};

pub async fn run(terminal: &mut Tui, config: &Config) -> Result<()> {
    let state: SharedState = Arc::new(std::sync::RwLock::new(AppState::new(
        config.history.capacity,
        config.docker.enabled,
        config.git.enabled,
        config.systemd.enabled,
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

        Command::RunAction(action) => {
            let events = events.clone();
            tokio::spawn(async move {
                let outcome = run_action(action).await;
                let _ = events.send(UiEvent::ActionFinished { outcome });
            });
        }
    }
}

/// Executes a domain action. Phase 1 only knows the no-op used to
/// validate the pipeline; phase 2 adds real container and service
/// actions.
#[expect(clippy::unused_async, reason = "real actions in phase 2 await I/O")]
async fn run_action(action: ActionCommand) -> ActionOutcome {
    match action {
        ActionCommand::Noop => {
            tracing::info!("test action executed");
            ActionOutcome::Success("test action completed".to_owned())
        }
    }
}

fn spawn_collector<C, F>(mut collector: C, state: SharedState, apply: F) -> &'static str
where
    C: Collector,
    F: Fn(&mut AppState, C::Output) + Send + 'static,
{
    let name = collector.name();
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
    name
}

/// Starts every enabled collector on its own task, returning the names
/// of the collectors it started. The bootstrap and the guard test both
/// rely on this being the single place domains are wired in.
fn spawn_collectors(state: &SharedState, config: &Config) -> Vec<&'static str> {
    let mut started = Vec::new();

    started.push(spawn_collector(
        MemoryCollector::new(config.collectors.memory.interval()),
        Arc::clone(state),
        |s, snap| {
            s.memory_history.push_percent(snap.used_percent());
            s.memory = Some(snap);
        },
    ));

    started.push(spawn_collector(
        CpuCollector::new(config.collectors.cpu.interval()),
        Arc::clone(state),
        |s, snap| {
            if let Some(snap) = snap {
                s.cpu_history.push_percent(snap.total);
                s.cpu = Some(snap);
            }
        },
    ));

    started.push(spawn_collector(
        ProcessCollector::new(config.collectors.processes.interval()),
        Arc::clone(state),
        |s, snap| {
            s.processes = Some(snap);
        },
    ));

    if config.docker.enabled {
        started.push(spawn_collector(
            DockerCollector::new(config.docker.clone()),
            Arc::clone(state),
            |s, status| {
                s.docker = DomainState::Observed(status);
            },
        ));
    }

    if config.git.enabled {
        started.push(spawn_collector(
            GitCollector::new(config.git.clone()),
            Arc::clone(state),
            |s, status| {
                s.git = DomainState::Observed(status);
            },
        ));
    }

    if config.network.enabled {
        let capacity = config.history.capacity;
        started.push(spawn_collector(
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
        ));
    }

    if config.disk.enabled {
        let capacity = config.history.capacity;
        started.push(spawn_collector(
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
        ));
    }

    if config.systemd.enabled {
        started.push(spawn_collector(
            SystemdCollector::new(config.systemd.interval()),
            Arc::clone(state),
            |s, status| {
                s.systemd = DomainState::Observed(status);
            },
        ));
    }

    tracing::info!(count = started.len(), collectors = ?started, "collectors started");
    started
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every domain that must be wired into the bootstrap. Adding a
    /// domain means adding its collector name here — and the test below
    /// fails until `spawn_collectors` actually starts it.
    const EXPECTED_DOMAINS: &[&str] = &[
        "memory", "cpu", "process", "docker", "git", "network", "disk", "systemd",
    ];

    /// A config with every optional domain enabled, so the bootstrap
    /// attempts to start all of them.
    fn all_enabled_config() -> Config {
        let mut config = Config::default();
        config.docker.enabled = true;
        config.git.enabled = true;
        config.network.enabled = true;
        config.disk.enabled = true;
        config.systemd.enabled = true;
        config
    }

    /// The architectural guarantee: every domain present in the codebase
    /// is actually started by the bootstrap. A domain implemented but
    /// never spawned — the bug that silently showed "sampling..." for
    /// memory and network in the past — fails here instead of at runtime.
    #[tokio::test]
    async fn every_domain_is_wired_into_the_bootstrap() {
        let config = all_enabled_config();
        let state: SharedState = Arc::new(std::sync::RwLock::new(AppState::new(
            config.history.capacity,
            config.docker.enabled,
            config.git.enabled,
            config.systemd.enabled,
        )));

        let mut started = spawn_collectors(&state, &config);
        started.sort_unstable();

        let mut expected: Vec<&str> = EXPECTED_DOMAINS.to_vec();
        expected.sort_unstable();

        assert_eq!(
            started, expected,
            "collectors started by the bootstrap do not match the expected \
             domains — a domain was likely added to AppState/config but not \
             spawned in spawn_collectors (or vice versa)"
        );
    }
}
