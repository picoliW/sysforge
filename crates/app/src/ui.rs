use sysforge_docker::collector::{ContainerInfo, DockerStatus};

use crate::input::{self, Action};
use crate::state::{AppState, DockerUiState};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PanelId {
    #[default]
    Cpu,
    Memory,
    Docker,
}

impl PanelId {
    const ORDER: [Self; 3] = [Self::Cpu, Self::Memory, Self::Docker];

    fn step(self, forward: bool, docker_enabled: bool) -> Self {
        let order = Self::ORDER;
        let len = order.len();
        let position = order.iter().position(|p| *p == self).unwrap_or(0);
        let mut index = position;
        loop {
            index = if forward {
                (index + 1) % len
            } else {
                (index + len - 1) % len
            };
            let candidate = order[index];
            if candidate != Self::Docker || docker_enabled {
                return candidate;
            }
            if index == position {
                return self;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overlay {
    pub title: String,
    pub lines: Vec<String>,
    pub scroll: u16,
}

impl Overlay {
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
    pub fn text(title: impl Into<String>, lines: Vec<String>) -> Self {
        Self {
            title: title.into(),
            lines,
            scroll: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    FetchDockerLogs { id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    OverlayContent { lines: Vec<String> },
}

#[derive(Debug, Default)]
pub struct UiState {
    pub focus: PanelId,
    pub docker_selected: usize,
    pub overlay: Option<Overlay>,
}

impl UiState {
    pub fn handle(&mut self, action: Action, state: &AppState) -> Option<Command> {
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
            Action::Quit | Action::Close => {}
            Action::FocusNext => self.focus = self.focus.step(true, docker_enabled),
            Action::FocusPrev => self.focus = self.focus.step(false, docker_enabled),
            Action::FocusPanel(panel) => {
                if panel != PanelId::Docker || docker_enabled {
                    self.focus = panel;
                }
            }
            Action::SelectionUp => {
                if self.focus == PanelId::Docker {
                    self.docker_selected = self.docker_selected.saturating_sub(1);
                }
            }
            Action::SelectionDown => {
                if self.focus == PanelId::Docker {
                    let last = docker_rows(state).saturating_sub(1);
                    self.docker_selected = (self.docker_selected + 1).min(last);
                }
            }
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
        None
    }

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
}

fn docker_rows(state: &AppState) -> usize {
    match &state.docker {
        DockerUiState::Observed(DockerStatus::Available(snap)) => snap.containers.len(),
        _ => 0,
    }
}

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
    fn tab_skips_docker_when_disabled() {
        let mut ui = UiState::default();
        let mut state = AppState::new(10, false);
        state.docker = DockerUiState::Disabled;
        ui.handle(Action::FocusNext, &state);
        ui.handle(Action::FocusNext, &state);
        assert_eq!(ui.focus, PanelId::Cpu);
    }

    #[test]
    fn direct_focus_on_disabled_docker_is_ignored() {
        let mut ui = UiState::default();
        let state = AppState::new(10, false);
        ui.handle(Action::FocusPanel(PanelId::Docker), &state);
        assert_eq!(ui.focus, PanelId::Cpu);
    }

    #[test]
    fn selection_clamps_at_zero() {
        let mut ui = UiState::default();
        let state = AppState::new(10, true);
        ui.handle(Action::SelectionUp, &state);
        assert_eq!(ui.docker_selected, 0);
    }

    #[test]
    fn open_overlay_captures_navigation_and_close() {
        let mut ui = UiState::default();
        let state = AppState::new(10, true);
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
