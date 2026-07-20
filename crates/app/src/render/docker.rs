use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Paragraph, Row, Table, TableState};
use sysforge_docker::collector::DockerStatus;

use super::components;
use crate::state::DockerUiState;

pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    docker: &DockerUiState,
    selected: usize,
    focused: bool,
) {
    match docker {
        DockerUiState::Disabled => {}
        DockerUiState::Pending => {
            placeholder(frame, area, " Docker [3] ", "sampling...", focused);
        }
        DockerUiState::Observed(DockerStatus::Unavailable { reason }) => {
            placeholder(frame, area, " Docker [3] ─ offline ", reason, focused);
        }
        DockerUiState::Observed(DockerStatus::Available(snap)) => {
            let title = format!(
                " Docker [3] ({}/{} running) ",
                snap.running(),
                snap.containers.len()
            );
            let block = components::panel_block(&title, focused);
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let header = Row::new(["NAME", "CPU%", "MEM", "IMAGE", "STATE", "STATUS"])
                .style(Style::default().add_modifier(Modifier::BOLD));
            let rows = snap.containers.iter().map(|c| {
                let style = if c.is_running() {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
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
                .style(style)
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
            if focused && !snap.containers.is_empty() {
                table_state.select(Some(selected));
            }
            frame.render_stateful_widget(table, inner, &mut table_state);
        }
    }
}

fn placeholder(frame: &mut Frame, area: Rect, title: &str, body: &str, focused: bool) {
    let block = components::panel_block(title, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(body.to_owned()).style(Style::default().fg(Color::DarkGray)),
        inner,
    );
}
