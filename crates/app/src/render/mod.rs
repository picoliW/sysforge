//! Panel composition.
//!
//! This module is the only place that knows the whole [`AppState`]: it
//! splits the frame and hands each panel exactly the data it needs,
//! plus a [`RenderCtx`] carrying shared visual context. Panels are
//! independent by convention — each submodule exposes a `render`
//! function with its own tailored signature.

mod components;
mod cpu;
mod docker;
mod memory;
mod overlay;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::state::{AppState, DockerUiState};
use crate::theme::Theme;
use crate::ui::{PanelId, UiState};

/// Shared visual context handed to every panel.
pub(super) struct RenderCtx<'a> {
    /// Active theme.
    pub theme: &'a Theme,
    /// Whether the receiving panel currently has focus.
    pub focused: bool,
}

/// Draws a single frame from the observed state, the UI state and the
/// active theme.
pub fn render(frame: &mut Frame, state: &AppState, ui: &UiState, theme: &Theme) {
    let ctx = |panel: PanelId| RenderCtx {
        theme,
        focused: ui.focus == panel,
    };

    if state.docker == DockerUiState::Disabled {
        let [cpu_area, mem_area] =
            Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)])
                .areas(frame.area());
        cpu::render(
            frame,
            cpu_area,
            state.cpu.as_ref(),
            &state.cpu_history,
            &ctx(PanelId::Cpu),
        );
        memory::render(
            frame,
            mem_area,
            state.memory,
            &state.memory_history,
            &ctx(PanelId::Memory),
        );
    } else {
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
            &ctx(PanelId::Cpu),
        );
        memory::render(
            frame,
            mem_area,
            state.memory,
            &state.memory_history,
            &ctx(PanelId::Memory),
        );
        docker::render(
            frame,
            docker_area,
            &state.docker,
            ui.docker_selected,
            &ctx(PanelId::Docker),
        );
    }

    if let Some(overlay) = &ui.overlay {
        overlay::render(frame, overlay, theme);
    }
}
