//! The processes panel.

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Paragraph, Row, Table, TableState};
use sysforge_system::process::ProcessSnapshot;

use super::{RenderCtx, components};

/// Renders the top-processes table.
pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    processes: Option<&ProcessSnapshot>,
    selected: usize,
    ctx: &RenderCtx<'_>,
) {
    let Some(snap) = processes else {
        let block = components::panel_block(" Processes [4] ", ctx);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new("sampling...").style(Style::default().fg(ctx.theme.muted)),
            inner,
        );
        return;
    };

    let title = format!(
        " Processes [4] (top {} of {}) ",
        snap.processes.len(),
        snap.total
    );
    let block = components::panel_block(&title, ctx);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let header = Row::new(["PID", "NAME", "S", "CPU%", "MEM"])
        .style(Style::default().add_modifier(Modifier::BOLD));
    let rows = snap.processes.iter().map(|p| {
        let cpu = p
            .cpu_percent
            .map_or_else(|| String::from("-"), |c| format!("{c:5.1}"));
        Row::new([
            p.pid.to_string(),
            p.name.clone(),
            p.state.to_string(),
            cpu,
            components::format_bytes(p.memory),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(7),
            Constraint::Min(10),
            Constraint::Length(1),
            Constraint::Length(6),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .column_spacing(2)
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut table_state = TableState::default();
    if ctx.focused && !snap.processes.is_empty() {
        table_state.select(Some(selected));
    }
    frame.render_stateful_widget(table, inner, &mut table_state);
}
