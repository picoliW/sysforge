mod components;
mod cpu;
mod docker;
mod memory;
mod overlay;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::state::{AppState, DockerUiState};
use crate::ui::{PanelId, UiState};

pub fn render(frame: &mut Frame, state: &AppState, ui: &UiState) {
    if state.docker == DockerUiState::Disabled {
        let [cpu_area, mem_area] =
            Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)])
                .areas(frame.area());
        cpu::render(
            frame,
            cpu_area,
            state.cpu.as_ref(),
            &state.cpu_history,
            ui.focus == PanelId::Cpu,
        );
        memory::render(
            frame,
            mem_area,
            state.memory,
            &state.memory_history,
            ui.focus == PanelId::Memory,
        );
        return;
    }

    let [cpu_area, mem_area, docker_area] = Layout::vertical([
        Constraint::Percentage(30),
        Constraint::Percentage(25),
        Constraint::Min(0),
    ])
    .areas(frame.area());
    cpu::render(
        frame,
        cpu_area,
        state.cpu.as_ref(),
        &state.cpu_history,
        ui.focus == PanelId::Cpu,
    );
    memory::render(
        frame,
        mem_area,
        state.memory,
        &state.memory_history,
        ui.focus == PanelId::Memory,
    );
    docker::render(
        frame,
        docker_area,
        &state.docker,
        ui.docker_selected,
        ui.focus == PanelId::Docker,
    );
    if let Some(overlay) = &ui.overlay {
        overlay::render(frame, overlay);
    }
}
