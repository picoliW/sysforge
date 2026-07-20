//! UI-only state: current view, focus, in-panel selection and the
//! modal overlay.
//!
//! A [`ViewId`] is a full screen of the application; a [`PanelId`] is a
//! reusable component that may appear in several views. Owned
//! exclusively by the event loop — unlike [`crate::state::AppState`]
//! it is written by a single thread and needs no lock.

use sysforge_docker::collector::{ContainerInfo, DockerStatus};

use crate::input::{self, Action};
use crate::state::{AppState, DockerUiState};

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
}

impl ViewId {
    /// Every view, in switch-key order.
    pub const ALL: [Self; 6] = [
        Self::Overview,
        Self::Docker,
        Self::Processes,
        Self::Git,
        Self::Network,
        Self::Disk,
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
}

impl Overlay {
    /// An overlay with ready content.
    pub fn text(title: impl Into<String>, lines: Vec<String>) -> Self {
        Self {
            title: title.into(),
            lines,
            scroll: 0,
        }
    }

    fn loading(title: String) -> Self {
        Self {
            title,
            lines: vec![String::from("loading...")],
            scroll: 0,
        }
    }

    fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        let last = self.lines.len().saturating_sub(1);
        let last = u16::try_from(last).unwrap_or(u16::MAX);
        self.scroll = self.scroll.saturating_add(1).min(last);
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
}

/// An asynchronous result delivered back to the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    /// Content for the currently open overlay.
    OverlayContent {
        /// Replacement content lines.
        lines: Vec<String>,
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
    /// Modal overlay, if one is open.
    pub overlay: Option<Overlay>,
}

impl UiState {
    /// Applies an action given the latest observed state, possibly
    /// producing a [`Command`]. [`Action::Quit`] is handled by the
    /// caller, not here.
    pub fn handle(&mut self, action: Action, state: &AppState) -> Option<Command> {
        // Modal: an open overlay captures the interaction.
        if let Some(overlay) = &mut self.overlay {
            match action {
                Action::Close => self.overlay = None,
                Action::SelectionUp => overlay.scroll_up(),
                Action::SelectionDown => overlay.scroll_down(),
                _ => {}
            }
            return None;
        }

        let docker_enabled = state.docker != DockerUiState::Disabled;
        match action {
            Action::Quit => {}
            Action::Close => {
                // Esc means "back": from a dedicated view, to Overview.
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
                _ => {}
            },
            Action::SelectionDown => match self.focus {
                PanelId::Docker => {
                    self.docker_selected = self.docker_selected.saturating_add(1);
                }
                PanelId::Processes => {
                    self.processes_selected = self.processes_selected.saturating_add(1);
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
        }
        self.docker_selected = self
            .docker_selected
            .min(docker_rows(state).saturating_sub(1));
        self.processes_selected = self
            .processes_selected
            .min(process_rows(state).saturating_sub(1));
        None
    }

    /// Applies an asynchronous result. A late result for a closed
    /// overlay is dropped: the user's intent wins.
    pub fn apply_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::OverlayContent { lines } => {
                if let Some(overlay) = &mut self.overlay {
                    overlay.lines = lines;
                    overlay.scroll = 0;
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
    match &state.docker {
        DockerUiState::Observed(DockerStatus::Available(snap)) => snap.containers.len(),
        _ => 0,
    }
}

/// How many rows the processes table currently has.
fn process_rows(state: &AppState) -> usize {
    state.processes.as_ref().map_or(0, |s| s.processes.len())
}

/// The container behind a table row, if any.
fn selected_container(state: &AppState, index: usize) -> Option<&ContainerInfo> {
    match &state.docker {
        DockerUiState::Observed(DockerStatus::Available(snap)) => snap.containers.get(index),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_cycles_overview_without_docker() {
        let mut ui = UiState::default();
        let mut state = AppState::new(10, false, false);
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
        let mut state = AppState::new(10, false, false);
        state.docker = DockerUiState::Disabled;
        ui.handle(Action::SwitchView(ViewId::Docker), &state);
        assert_eq!(ui.view, ViewId::Overview);
    }

    #[test]
    fn dedicated_view_focuses_its_panel_and_esc_goes_back() {
        let mut ui = UiState::default();
        let state = AppState::new(10, true, true);
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
        let state = AppState::new(10, true, true);
        ui.handle(Action::SelectionUp, &state);
        assert_eq!(ui.docker_selected, 0);
    }

    #[test]
    fn open_overlay_captures_navigation_and_close() {
        let mut ui = UiState::default();
        let state = AppState::new(10, true, true);
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
}
