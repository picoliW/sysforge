//! The Kubernetes panel: pods with readiness and status.

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Paragraph, Row, Table, TableState};
use sysforge_common::availability::Availability;
use sysforge_common::domain_state::DomainState;

use super::{RenderCtx, components};
use crate::state::K8sUiState;

/// Renders the Kubernetes panel.
pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    k8s: &K8sUiState,
    selected: usize,
    ctx: &RenderCtx<'_>,
) {
    match k8s {
        DomainState::Disabled | DomainState::Pending => {
            placeholder(
                frame,
                area,
                " Pods [8] ",
                "sampling...",
                ctx,
                ctx.theme.muted,
            );
        }
        DomainState::Observed(Availability::Unavailable { reason }) => {
            placeholder(
                frame,
                area,
                " Pods [8] ─ offline ",
                reason,
                ctx,
                ctx.theme.warning,
            );
        }
        DomainState::Observed(Availability::Available(snap)) => {
            let title = format!(
                " Pods [8] ({} / {} ready) ",
                snap.ready_pods, snap.total_pods
            );
            let block = components::panel_block(&title, ctx);
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let header = Row::new(["NAMESPACE", "POD", "READY", "STATUS", "RESTARTS"])
                .style(Style::default().add_modifier(Modifier::BOLD));
            let rows = snap.pods.iter().map(|pod| {
                Row::new([
                    pod.namespace.clone(),
                    pod.name.clone(),
                    format!("{}/{}", pod.ready, pod.total),
                    pod.status.clone(),
                    pod.restarts.to_string(),
                ])
                .style(Style::default().fg(status_color(&pod.status, ctx)))
            });
            let table = Table::new(
                rows,
                [
                    Constraint::Percentage(20),
                    Constraint::Min(0),
                    Constraint::Length(7),
                    Constraint::Length(20),
                    Constraint::Length(8),
                ],
            )
            .header(header)
            .column_spacing(2)
            .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

            let mut table_state = TableState::default();
            if ctx.focused && !snap.pods.is_empty() {
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
    color: Color,
) {
    let block = components::panel_block(title, ctx);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(body.to_owned()).style(Style::default().fg(color)),
        inner,
    );
}

/// Maps a pod status to a severity colour, reproducing the visual
/// intuition of `kubectl`: green for healthy, red for failure states,
/// yellow for transient ones.
fn status_color(status: &str, ctx: &RenderCtx<'_>) -> Color {
    match status {
        "Running" | "Completed" | "Succeeded" => ctx.theme.success,
        "CrashLoopBackOff"
        | "ImagePullBackOff"
        | "ErrImagePull"
        | "Error"
        | "OOMKilled"
        | "Failed"
        | "CreateContainerError" => ctx.theme.error,
        "Pending" | "ContainerCreating" | "PodInitializing" | "Terminating" | "Init" => {
            ctx.theme.warning
        }
        _ => ctx.theme.text,
    }
}
