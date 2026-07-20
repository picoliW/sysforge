//! The Docker panel.

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Paragraph, Row, Table, TableState};
use sysforge_docker::collector::DockerStatus;

use super::{RenderCtx, components};
use crate::state::DockerUiState;

/// Renders the Docker panel: container table, or the offline/pending
/// placeholder.
pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    docker: &DockerUiState,
    selected: usize,
    ctx: &RenderCtx<'_>,
) {
    match docker {
        DockerUiState::Disabled => {}
        DockerUiState::Pending => {
            placeholder(
                frame,
                area,
                " Docker [3] ",
                "sampling...",
                ctx,
                ctx.theme.muted,
            );
        }
        DockerUiState::Observed(DockerStatus::Unavailable { reason }) => {
            placeholder(
                frame,
                area,
                " Docker [3] ─ offline ",
                reason,
                ctx,
                ctx.theme.warning,
            );
        }
        DockerUiState::Observed(DockerStatus::Available(snap)) => {
            let title = format!(
                " Docker [3] ({}/{} running) ",
                snap.running(),
                snap.containers.len()
            );
            let block = components::panel_block(&title, ctx);
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let header = Row::new(["NAME", "CPU%", "MEM", "IMAGE", "STATE", "STATUS"])
                .style(Style::default().add_modifier(Modifier::BOLD));
            let rows = snap.containers.iter().map(|c| {
                let color = if c.is_running() {
                    ctx.theme.success
                } else {
                    ctx.theme.muted
                };
                let cpu = c
                    .cpu_percent
                    .map_or_else(|| String::from("-"), |p| format!("{p:5.1}"));
                let mem = c
                    .memory_usage
                    .map_or_else(|| String::from("-"), components::format_bytes);
                Row::new([
                    c.name.clone(),
                    cpu,
                    mem,
                    c.image.clone(),
                    c.state.clone(),
                    c.status.clone(),
                ])
                .style(Style::default().fg(color))
            });
            let table = Table::new(
                rows,
                [
                    Constraint::Percentage(22),
                    Constraint::Length(6),
                    Constraint::Length(10),
                    Constraint::Percentage(24),
                    Constraint::Length(8),
                    Constraint::Min(0),
                ],
            )
            .header(header)
            .column_spacing(2)
            .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

            let mut table_state = TableState::default();
            if ctx.focused && !snap.containers.is_empty() {
                table_state.select(Some(selected));
            }
            frame.render_stateful_widget(table, inner, &mut table_state);
        }
    }
}

fn placeholder(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    body: &str,
    ctx: &RenderCtx<'_>,
    color: ratatui::style::Color,
) {
    let block = components::panel_block(title, ctx);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(body.to_owned()).style(Style::default().fg(color)),
        inner,
    );
}
