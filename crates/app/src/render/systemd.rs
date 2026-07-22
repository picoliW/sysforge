//! The systemd panel: services with activation state.

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Paragraph, Row, Table, TableState};
use sysforge_common::availability::Availability;
use sysforge_common::domain_state::DomainState;
use sysforge_systemd::collector::ServiceState;

use super::{RenderCtx, components};
use crate::state::SystemdUiState;

/// Renders the systemd panel.
pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    systemd: &SystemdUiState,
    selected: usize,
    ctx: &RenderCtx<'_>,
) {
    match systemd {
        DomainState::Disabled | DomainState::Pending => {
            placeholder(
                frame,
                area,
                " Services [7] ",
                "sampling...",
                ctx,
                ctx.theme.muted,
            );
        }
        DomainState::Observed(Availability::Unavailable { reason }) => {
            placeholder(
                frame,
                area,
                " Services [7] ─ offline ",
                reason,
                ctx,
                ctx.theme.warning,
            );
        }
        DomainState::Observed(Availability::Available(snap)) => {
            let title = format!(
                " Services [7] ({} active · {} failed) ",
                snap.active, snap.failed
            );
            let block = components::panel_block(&title, ctx);
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let header = Row::new(["SERVICE", "STATE", "SUB", "DESCRIPTION"])
                .style(Style::default().add_modifier(Modifier::BOLD));
            let rows = snap.services.iter().map(|svc| {
                let color = match svc.state {
                    ServiceState::Failed => ctx.theme.error,
                    ServiceState::Active => ctx.theme.success,
                    ServiceState::Inactive | ServiceState::Other => ctx.theme.muted,
                };
                Row::new([
                    svc.name.clone(),
                    state_label(svc.state).to_owned(),
                    svc.sub.clone(),
                    svc.description.clone(),
                ])
                .style(Style::default().fg(color))
            });
            let table = Table::new(
                rows,
                [
                    Constraint::Percentage(28),
                    Constraint::Length(9),
                    Constraint::Length(10),
                    Constraint::Min(0),
                ],
            )
            .header(header)
            .column_spacing(2)
            .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

            let mut table_state = TableState::default();
            if ctx.focused && !snap.services.is_empty() {
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

fn state_label(state: ServiceState) -> &'static str {
    match state {
        ServiceState::Active => "active",
        ServiceState::Inactive => "inactive",
        ServiceState::Failed => "failed",
        ServiceState::Other => "other",
    }
}
