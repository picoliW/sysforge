//! View composition.
//!
//! A view is a full screen; panels are reusable components that views
//! arrange. This module is the only place that knows the whole
//! [`AppState`]: it dispatches on the current [`ViewId`], splits the
//! frame, and hands each panel exactly the data it needs plus a
//! [`RenderCtx`].

mod components;
mod cpu;
mod disk;
mod docker;
mod git;
mod memory;
mod network;
mod overlay;
mod processes;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};

use crate::state::{AppState, DockerUiState};
use crate::theme::Theme;
use crate::ui::{PanelId, UiState, ViewId};

/// Shared visual context handed to every panel.
pub(super) struct RenderCtx<'a> {
    /// Active theme.
    pub theme: &'a Theme,
    /// Whether the receiving panel currently has focus.
    pub focused: bool,
}

/// Draws a single frame: view bar, the current view, and any overlay.
pub fn render(frame: &mut Frame, state: &AppState, ui: &UiState, theme: &Theme) {
    let [bar_area, body] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(frame.area());
    components::view_bar(frame, bar_area, ui.view, theme);

    let ctx = |panel: PanelId| RenderCtx {
        theme,
        focused: ui.focus == panel,
    };
    match ui.view {
        ViewId::Overview => render_overview(frame, body, state, ui, &ctx),
        ViewId::Docker => docker::render(
            frame,
            body,
            &state.docker,
            ui.docker_selected,
            &ctx(PanelId::Docker),
        ),
        ViewId::Processes => processes::render(
            frame,
            body,
            state.processes.as_ref(),
            ui.processes_selected,
            &ctx(PanelId::Processes),
        ),
        ViewId::Git => git::render(frame, body, &state.git, &ctx(PanelId::Git)),
        ViewId::Network => network::render(
            frame,
            body,
            state.network.as_ref(),
            &state.network_history,
            &ctx(PanelId::Network),
        ),
        ViewId::Disk => disk::render(
            frame,
            body,
            state.disk.as_ref(),
            &state.disk_history,
            &ctx(PanelId::Disk),
        ),
    }

    if let Some(overlay) = &ui.overlay {
        overlay::render(frame, overlay, theme);
    }
}

/// The summary view: every domain at a glance.
fn render_overview<'a>(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    ui: &UiState,
    ctx: &dyn Fn(PanelId) -> RenderCtx<'a>,
) {
    if state.docker == DockerUiState::Disabled {
        let [cpu_area, mem_area, proc_area] = Layout::vertical([
            Constraint::Percentage(30),
            Constraint::Percentage(25),
            Constraint::Min(0),
        ])
        .areas(area);
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
        processes::render(
            frame,
            proc_area,
            state.processes.as_ref(),
            ui.processes_selected,
            &ctx(PanelId::Processes),
        );
    } else {
        let [cpu_area, mem_area, docker_area, proc_area] = Layout::vertical([
            Constraint::Percentage(20),
            Constraint::Percentage(18),
            Constraint::Percentage(30),
            Constraint::Min(0),
        ])
        .areas(area);
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
        processes::render(
            frame,
            proc_area,
            state.processes.as_ref(),
            ui.processes_selected,
            &ctx(PanelId::Processes),
        );
    }
}
