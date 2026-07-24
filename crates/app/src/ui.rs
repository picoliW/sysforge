//! UI-only state: current view, focus, in-panel selection and the
//! modal overlay.
//!
//! A [`ViewId`] is a full screen of the application; a [`PanelId`] is a
//! reusable component that may appear in several views. Owned
//! exclusively by the event loop — unlike [`crate::state::AppState`]
//! it is written by a single thread and needs no lock.

use sysforge_common::availability::Availability;
use sysforge_common::domain_state::DomainState;
use sysforge_docker::collector::ContainerInfo;

use crate::input::{self, Action};
use crate::state::AppState;

/// The reusable panels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PanelId {
    #[default]
    Cpu,
    Memory,
    Docker,
    Processes,
    Git,
    Network,
    Disk,
    /// systemd panel.
    Systemd,
}

/// The full screens of the application.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ViewId {
    #[default]
    Overview,
    Docker,
    Processes,
    Git,
    Network,
    Disk,
    /// systemd, full screen (`7`).
    Systemd,
}

impl ViewId {
    /// Every view, in switch-key order.
    pub const ALL: [Self; 7] = [
        Self::Overview,
        Self::Docker,
        Self::Processes,
        Self::Git,
        Self::Network,
        Self::Disk,
        Self::Systemd,
    ];

    /// Title shown in the view bar.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Docker => "docker",
            Self::Processes => "processes",
            Self::Git => "git",
            Self::Network => "network",
            Self::Disk => "disk",
            Self::Systemd => "systemd",
        }
    }

    /// The panels composing this view, in Tab order.
    fn panels(self, docker_enabled: bool) -> &'static [PanelId] {
        match (self, docker_enabled) {
            (Self::Overview, true) => &[
                PanelId::Cpu,
                PanelId::Memory,
                PanelId::Docker,
                PanelId::Processes,
            ],
            (Self::Overview, false) => &[PanelId::Cpu, PanelId::Memory, PanelId::Processes],
            (Self::Docker, _) => &[PanelId::Docker],
            (Self::Processes, _) => &[PanelId::Processes],
            (Self::Git, _) => &[PanelId::Git],
            (Self::Network, _) => &[PanelId::Network],
            (Self::Disk, _) => &[PanelId::Disk],
            (Self::Systemd, _) => &[PanelId::Systemd],
        }
    }
}

/// A modal, scrollable text view. Generic on purpose: today it shows
/// container logs and help; tomorrow resource details or errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overlay {
    /// Title shown on the overlay border.
    pub title: String,
    /// Content lines.
    pub lines: Vec<String>,
    /// First visible line.
    pub scroll: u16,
    /// What kind of overlay this is.
    pub kind: OverlayKind,
}

impl Overlay {
    /// A text overlay with content already available.
    fn text(title: &str, lines: Vec<String>) -> Self {
        Self {
            title: title.to_owned(),
            lines,
            scroll: 0,
            kind: OverlayKind::Text,
        }
    }

    /// An overlay whose content is still being fetched asynchronously.
    fn loading(title: String) -> Self {
        Self {
            title,
            lines: vec![String::from("loading...")],
            scroll: 0,
            kind: OverlayKind::Text,
        }
    }

    /// Scrolls one line up.
    fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    /// Scrolls one line down, stopping at the last line.
    fn scroll_down(&mut self) {
        let last = u16::try_from(self.lines.len().saturating_sub(1)).unwrap_or(u16::MAX);
        self.scroll = self.scroll.saturating_add(1).min(last);
    }

    /// A yes/no confirmation for a pending action.
    fn confirm(request: ActionRequest) -> Self {
        Self {
            title: String::from(" confirm "),
            lines: vec![
                request.prompt.clone(),
                String::new(),
                "[y] confirm    [n / Esc] cancel".to_owned(),
            ],
            scroll: 0,
            kind: OverlayKind::Confirm(request),
        }
    }

    /// Feedback after an action finishes.
    fn outcome(outcome: &ActionOutcome) -> Self {
        let (title, body) = match outcome {
            ActionOutcome::Success(msg) => (" done ", msg.clone()),
            ActionOutcome::Failure(msg) => (" failed ", msg.clone()),
        };
        Self {
            title: title.to_owned(),
            lines: vec![body, String::new(), "[Esc] close".to_owned()],
            scroll: 0,
            kind: OverlayKind::Text,
        }
    }
}
/// An intention produced by the UI for the runtime to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Fetch the logs of a container and deliver them as a [`UiEvent`].
    FetchDockerLogs {
        /// Engine identifier of the container.
        id: String,
    },
    /// Execute a confirmed domain action.
    RunAction(ActionCommand),
}

/// An asynchronous result delivered back to the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    OverlayContent {
        lines: Vec<String>,
    },
    /// An action finished; show its outcome.
    ActionFinished {
        outcome: ActionOutcome,
    },
}

/// View, focus, selections and overlay, mutated only by
/// [`UiState::handle`] and [`UiState::apply_event`].
#[derive(Debug, Default)]
pub struct UiState {
    /// The screen currently shown.
    pub view: ViewId,
    /// Which panel currently receives in-panel actions.
    pub focus: PanelId,
    /// Selected row in the Docker container table.
    pub docker_selected: usize,
    /// Selected row in the processes table.
    pub processes_selected: usize,
    /// Selected row in the systemd services table.
    pub systemd_selected: usize,
    /// Modal overlay, if one is open.
    pub overlay: Option<Overlay>,
}

/// A domain action awaiting or undergoing execution.
///
/// Actions change the state of the world (restarting a container,
/// stopping a service), so every one passes through explicit
/// confirmation before running and always reports its outcome. The
/// target is captured at proposal time, not execution time: what you
/// confirm is what you saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRequest {
    /// One-line description shown in the confirmation prompt.
    pub prompt: String,
    /// The work to perform once confirmed.
    pub command: ActionCommand,
}

/// The concrete work an action performs. Like [`Command`], this names
/// its domain: an intention has a destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionCommand {
    /// A no-op used to validate the action pipeline end to end without
    /// touching the system. Removed once real actions exist.
    Noop,
    // Phase 2 adds: RestartContainer { id }, StartService { name }, ...
}

/// How a finished action turned out, shown as feedback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionOutcome {
    /// The action succeeded.
    Success(String),
    /// The action failed, with a reason.
    #[expect(dead_code, reason = "constructed by real actions in phase 2")]
    Failure(String),
}
/// What an overlay is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayKind {
    /// Scrollable text (logs, help).
    Text,
    /// A yes/no confirmation for a pending action.
    Confirm(ActionRequest),
    /// An action is running; no input accepted.
    Running,
}

impl UiState {
    /// Applies an action given the latest observed state, possibly
    /// producing a [`Command`]. [`Action::Quit`] is handled by the
    /// caller, not here.
    pub fn handle(&mut self, action: Action, state: &AppState) -> Option<Command> {
        // Modal: an open overlay captures the interaction.
        if let Some(overlay) = &mut self.overlay {
            match (&overlay.kind, action) {
                (OverlayKind::Confirm(request), Action::Confirm) => {
                    let command = request.command.clone();
                    self.overlay = Some(Overlay {
                        title: " running ".to_owned(),
                        lines: vec!["running...".to_owned()],
                        scroll: 0,
                        kind: OverlayKind::Running,
                    });
                    return Some(Command::RunAction(command));
                }
                (OverlayKind::Confirm(_), Action::Close) => {
                    self.overlay = None; // cancelled — nothing ran
                }
                (OverlayKind::Running, _) => {} // input ignored while running
                (_, Action::Close) => self.overlay = None,
                (_, Action::SelectionUp) => overlay.scroll_up(),
                (_, Action::SelectionDown) => overlay.scroll_down(),
                _ => {}
            }
            return None;
        }

        let docker_enabled = !state.docker.is_disabled();
        match action {
            // Quit is handled in the run loop; Confirm only matters
            // while a confirmation overlay is open (handled above).
            Action::Quit | Action::Confirm => {}
            Action::Close => {
                if self.view != ViewId::Overview {
                    self.switch_to(ViewId::Overview, docker_enabled);
                }
            }
            Action::SwitchView(view) => {
                if view != ViewId::Docker || docker_enabled {
                    self.switch_to(view, docker_enabled);
                }
            }
            Action::FocusNext => {
                self.focus = cycle(self.view.panels(docker_enabled), self.focus, true);
            }
            Action::FocusPrev => {
                self.focus = cycle(self.view.panels(docker_enabled), self.focus, false);
            }
            Action::SelectionUp => match self.focus {
                PanelId::Docker => {
                    self.docker_selected = self.docker_selected.saturating_sub(1);
                }
                PanelId::Processes => {
                    self.processes_selected = self.processes_selected.saturating_sub(1);
                }
                PanelId::Systemd => {
                    self.systemd_selected = self.systemd_selected.saturating_sub(1);
                }
                _ => {}
            },
            Action::SelectionDown => match self.focus {
                PanelId::Docker => {
                    self.docker_selected = self.docker_selected.saturating_add(1);
                }
                PanelId::Processes => {
                    self.processes_selected = self.processes_selected.saturating_add(1);
                }
                PanelId::Systemd => {
                    self.systemd_selected = self.systemd_selected.saturating_add(1);
                }
                _ => {}
            },
            Action::OpenLogs => {
                if self.focus == PanelId::Docker {
                    if let Some(container) = selected_container(state, self.docker_selected) {
                        self.overlay =
                            Some(Overlay::loading(format!(" logs: {} ", container.name)));
                        return Some(Command::FetchDockerLogs {
                            id: container.id.clone(),
                        });
                    }
                }
            }
            Action::OpenHelp => {
                self.overlay = Some(Overlay::text(" help ", input::help_lines()));
            }
            Action::Propose => {
                self.overlay = Some(Overlay::confirm(ActionRequest {
                    prompt: String::from("Run the test action?"),
                    command: ActionCommand::Noop,
                }));
            } // Confirming only means something while a confirmation
              // overlay is open, which the modal branch above handles.
        }
        self.docker_selected = self
            .docker_selected
            .min(docker_rows(state).saturating_sub(1));
        self.processes_selected = self
            .processes_selected
            .min(process_rows(state).saturating_sub(1));
        self.systemd_selected = self
            .systemd_selected
            .min(systemd_rows(state).saturating_sub(1));
        None
    }

    /// Applies an asynchronous result. A late result for a closed
    pub fn apply_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::OverlayContent { lines } => {
                if let Some(overlay) = &mut self.overlay {
                    overlay.lines = lines;
                    overlay.scroll = 0;
                }
            }
            UiEvent::ActionFinished { outcome } => {
                // Only replace a Running overlay — if the user closed it,
                // the result is dropped (their intent wins).
                if matches!(
                    self.overlay.as_ref().map(|o| &o.kind),
                    Some(OverlayKind::Running)
                ) {
                    self.overlay = Some(Overlay::outcome(&outcome));
                }
            }
        }
    }

    fn switch_to(&mut self, view: ViewId, docker_enabled: bool) {
        self.view = view;
        self.focus = view.panels(docker_enabled)[0];
    }
}

/// The panel after (or before) `current` in `panels`, wrapping around.
fn cycle(panels: &[PanelId], current: PanelId, forward: bool) -> PanelId {
    let len = panels.len();
    let position = panels.iter().position(|p| *p == current).unwrap_or(0);
    let next = if forward {
        (position + 1) % len
    } else {
        (position + len - 1) % len
    };
    panels[next]
}

/// How many rows the Docker table currently has.
fn docker_rows(state: &AppState) -> usize {
    match state.docker.observed() {
        Some(Availability::Available(snap)) => snap.containers.len(),
        _ => 0,
    }
}

/// How many rows the processes table currently has.
fn process_rows(state: &AppState) -> usize {
    state.processes.as_ref().map_or(0, |s| s.processes.len())
}

/// How many rows the systemd services table currently has.
fn systemd_rows(state: &AppState) -> usize {
    match state.systemd.observed() {
        Some(Availability::Available(snap)) => snap.services.len(),
        _ => 0,
    }
}

/// The container behind a table row, if any.
fn selected_container(state: &AppState, index: usize) -> Option<&ContainerInfo> {
    match &state.docker {
        DomainState::Observed(Availability::Available(snap)) => snap.containers.get(index),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DockerUiState;

    #[test]
    fn tab_cycles_overview_without_docker() {
        let mut ui = UiState::default();
        let mut state = AppState::new(10, false, false, false);
        state.docker = DockerUiState::Disabled;
        ui.handle(Action::FocusNext, &state);
        ui.handle(Action::FocusNext, &state);
        assert_eq!(ui.focus, PanelId::Processes);
        ui.handle(Action::FocusNext, &state);
        assert_eq!(ui.focus, PanelId::Cpu);
    }

    #[test]
    fn switching_to_disabled_docker_view_is_ignored() {
        let mut ui = UiState::default();
        let mut state = AppState::new(10, false, false, false);
        state.docker = DockerUiState::Disabled;
        ui.handle(Action::SwitchView(ViewId::Docker), &state);
        assert_eq!(ui.view, ViewId::Overview);
    }

    #[test]
    fn dedicated_view_focuses_its_panel_and_esc_goes_back() {
        let mut ui = UiState::default();
        let state = AppState::new(10, true, true, true);
        ui.handle(Action::SwitchView(ViewId::Processes), &state);
        assert_eq!(ui.view, ViewId::Processes);
        assert_eq!(ui.focus, PanelId::Processes);
        ui.handle(Action::Close, &state);
        assert_eq!(ui.view, ViewId::Overview);
        assert_eq!(ui.focus, PanelId::Cpu);
    }

    #[test]
    fn selection_clamps_at_zero() {
        let mut ui = UiState::default();
        let state = AppState::new(10, true, true, true);
        ui.handle(Action::SelectionUp, &state);
        assert_eq!(ui.docker_selected, 0);
    }

    #[test]
    fn open_overlay_captures_navigation_and_close() {
        let mut ui = UiState::default();
        let state = AppState::new(10, true, true, true);
        ui.overlay = Some(Overlay::loading(String::from(" test ")));

        let command = ui.handle(Action::FocusNext, &state);
        assert_eq!(command, None);
        assert_eq!(ui.focus, PanelId::Cpu);
        ui.handle(Action::Close, &state);
        assert!(ui.overlay.is_none());
    }

    #[test]
    fn late_content_for_closed_overlay_is_dropped() {
        let mut ui = UiState::default();
        ui.apply_event(UiEvent::OverlayContent {
            lines: vec![String::from("x")],
        });
        assert!(ui.overlay.is_none());
    }

    #[test]
    fn action_requires_confirmation() {
        let mut ui = UiState::default();
        let state = AppState::new(10, true, true, true);
        // Proposing opens a confirmation and runs nothing.
        let cmd = ui.handle(Action::Propose, &state);
        assert_eq!(cmd, None);
        assert!(matches!(
            ui.overlay.as_ref().map(|o| &o.kind),
            Some(OverlayKind::Confirm(_))
        ));
        // Confirming is what actually dispatches the work.
        let cmd = ui.handle(Action::Confirm, &state);
        assert_eq!(cmd, Some(Command::RunAction(ActionCommand::Noop)));
    }

    #[test]
    fn cancelling_runs_nothing() {
        let mut ui = UiState::default();
        let state = AppState::new(10, true, true, true);
        ui.handle(Action::Propose, &state);
        let cmd = ui.handle(Action::Close, &state);
        assert_eq!(cmd, None);
        assert!(ui.overlay.is_none());
    }

    #[test]
    fn running_overlay_ignores_input() {
        let mut ui = UiState::default();
        let state = AppState::new(10, true, true, true);
        ui.handle(Action::Propose, &state);
        ui.handle(Action::Confirm, &state);
        // A second confirmation while running dispatches nothing.
        let cmd = ui.handle(Action::Confirm, &state);
        assert_eq!(cmd, None);
    }
}
