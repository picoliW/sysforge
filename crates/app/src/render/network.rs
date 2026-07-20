//! The Network panel: interfaces with throughput and traffic sparklines.

use std::collections::HashMap;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use sysforge_network::collector::{InterfaceInfo, NetworkSnapshot};

use super::{RenderCtx, components};
use crate::history::History;

/// Renders the network panel.
pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    network: Option<&NetworkSnapshot>,
    history: &HashMap<String, History>,
    ctx: &RenderCtx<'_>,
) {
    let block = components::panel_block(" Network [5] ", ctx);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(snap) = network else {
        frame.render_widget(
            Paragraph::new("sampling...").style(Style::default().fg(ctx.theme.muted)),
            inner,
        );
        return;
    };

    // One row of fixed height per interface.
    let rows = snap.interfaces.len().min((inner.height / 2) as usize);
    let constraints: Vec<Constraint> = (0..rows).map(|_| Constraint::Length(2)).collect();
    let areas = Layout::vertical(constraints).split(inner);

    for (iface, row) in snap.interfaces.iter().zip(areas.iter()) {
        render_interface(frame, *row, iface, history.get(&iface.name), ctx);
    }
}

fn render_interface(
    frame: &mut Frame,
    area: Rect,
    iface: &InterfaceInfo,
    history: Option<&History>,
    ctx: &RenderCtx<'_>,
) {
    let [label_area, spark_area] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Min(0)])
            .spacing(1)
            .areas(area);

    let line = Line::from(vec![
        Span::styled(
            format!("{:<10}", iface.name),
            Style::default()
                .fg(ctx.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("↓{} ", rate_label(iface.rx_rate)),
            Style::default().fg(ctx.theme.success),
        ),
        Span::styled(
            format!("↑{}", rate_label(iface.tx_rate)),
            Style::default().fg(ctx.theme.warning),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), label_area);

    if let Some(history) = history {
        components::sparkline(frame, spark_area, history, ctx.theme);
    }
}

/// Formats a byte/second rate, or "—" before the first delta.
fn rate_label(rate: Option<f64>) -> String {
    match rate {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // clamped
        Some(bytes_per_sec) => {
            format!(
                "{}/s",
                components::format_bytes(bytes_per_sec.max(0.0) as u64)
            )
        }
        None => String::from("—"),
    }
}
