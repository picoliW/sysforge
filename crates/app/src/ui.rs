use crate::input::Action;
use crate::state::{AppState, DockerUiState};
use sysforge_docker::collector::DockerStatus;

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
            index = if forward { (index + 1) % len } else { (index + len - 1) % len };
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

#[derive(Debug, Default)]
pub struct UiState {
    pub focus: PanelId,
    pub docker_selected: usize,
}

impl UiState {
    pub fn handle(&mut self, action: Action, state: &AppState) {
        let docker_enabled = state.docker != DockerUiState::Disabled;
        match action {
            Action::Quit => {}
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
        }
        self.docker_selected = self
            .docker_selected
            .min(docker_rows(state).saturating_sub(1));
    }
}

fn docker_rows(state: &AppState) -> usize {
    match &state.docker {
        DockerUiState::Observed(DockerStatus::Available(snap)) => snap.containers.len(),
        _ => 0,
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
}