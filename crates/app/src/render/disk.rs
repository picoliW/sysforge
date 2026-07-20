//! The Disk panel: filesystem usage gauges and per-device I/O.

use std::collections::HashMap;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use sysforge_disk::collector::{DeviceIo, DiskSnapshot, Filesystem};

use super::{RenderCtx, components};
use crate::history::History;

/// Renders the disk panel: filesystems on top, device I/O below.
pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    disk: Option<&DiskSnapshot>,
    history: &HashMap<String, History>,
    ctx: &RenderCtx<'_>,
) {
    let block = components::panel_block(" Disk [6] ", ctx);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(snap) = disk else {
        frame.render_widget(
            Paragraph::new("sampling...").style(Style::default().fg(ctx.theme.muted)),
            inner,
        );
        return;
    };

    let fs_height = u16::try_from(snap.filesystems.len()).unwrap_or(u16::MAX) + 1;
    let [fs_area, io_area] =
        Layout::vertical([Constraint::Length(fs_height), Constraint::Min(0)]).areas(inner);

    render_filesystems(frame, fs_area, &snap.filesystems, ctx);
    render_devices(frame, io_area, &snap.devices, history, ctx);
}

fn render_filesystems(
    frame: &mut Frame,
    area: Rect,
    filesystems: &[Filesystem],
    ctx: &RenderCtx<'_>,
) {
    let lines: Vec<Line> = filesystems
        .iter()
        .map(|fs| {
            let pct = fs.used_percent();
            let color = if pct >= 90.0 {
                ctx.theme.error
            } else if pct >= 75.0 {
                ctx.theme.warning
            } else {
                ctx.theme.success
            };
            Line::from(vec![
                Span::styled(
                    format!("{:<20}", fs.mount_point),
                    Style::default()
                        .fg(ctx.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{pct:5.1}%  "), Style::default().fg(color)),
                Span::styled(
                    format!(
                        "{} / {}",
                        components::format_bytes(fs.used),
                        components::format_bytes(fs.total)
                    ),
                    Style::default().fg(ctx.theme.muted),
                ),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_devices(
    frame: &mut Frame,
    area: Rect,
    devices: &[DeviceIo],
    history: &HashMap<String, History>,
    ctx: &RenderCtx<'_>,
) {
    let rows = devices.len().min((area.height / 2) as usize);
    let constraints: Vec<Constraint> = (0..rows).map(|_| Constraint::Length(2)).collect();
    let areas = Layout::vertical(constraints).split(area);

    for (device, row) in devices.iter().zip(areas.iter()) {
        let [label_area, spark_area] =
            Layout::horizontal([Constraint::Percentage(45), Constraint::Min(0)])
                .spacing(1)
                .areas(*row);

        let line = Line::from(vec![
            Span::styled(
                format!("{:<12}", device.name),
                Style::default()
                    .fg(ctx.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("r {} ", rate_label(device.read_rate)),
                Style::default().fg(ctx.theme.success),
            ),
            Span::styled(
                format!("w {}", rate_label(device.write_rate)),
                Style::default().fg(ctx.theme.warning),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), label_area);

        if let Some(history) = history.get(&device.name) {
            components::sparkline(frame, spark_area, history, ctx.theme);
        }
    }
}

/// Formats a byte/second rate, or "—" before the first delta.
fn rate_label(rate: Option<f64>) -> String {
    match rate {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // clamped
        Some(bps) => format!("{}/s", components::format_bytes(bps.max(0.0) as u64)),
        None => String::from("—"),
    }
}
