//! Small visual building blocks shared by the panels.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Borders, Gauge, Sparkline};

use super::RenderCtx;
use crate::history::History;
use crate::theme::Theme;

/// Standard SysForge panel frame; the focused panel gets the accent
/// border, unfocused panels recede.
pub(super) fn panel_block<'a>(title: &'a str, ctx: &RenderCtx<'_>) -> Block<'a> {
    let color = if ctx.focused {
        ctx.theme.accent
    } else {
        ctx.theme.border
    };
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
}

/// Standard percentage gauge.
pub(super) fn percent_gauge(percent: f64, theme: &Theme) -> Gauge<'static> {
    Gauge::default()
        .ratio((percent / 100.0).clamp(0.0, 1.0))
        .label(format!("{percent:.1}%"))
        .gauge_style(Style::default().fg(theme.accent))
}

/// Percentage sparkline with a fixed 0–100 scale, newest sample at the
/// right edge.
pub(super) fn sparkline(frame: &mut Frame, area: Rect, history: &History, theme: &Theme) {
    let data = history.last(area.width as usize);
    let spark = Sparkline::default()
        .data(&data)
        .max(100) // fixed scale: idle noise must not look like mountains
        .style(Style::default().fg(theme.accent));
    frame.render_widget(spark, area);
}

/// Human-readable binary units (KiB, MiB, GiB...).
#[allow(clippy::cast_precision_loss)] // byte counts are far below f64 precision loss
pub(super) fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_picks_sensible_units() {
        assert_eq!(format_bytes(512), "512.0 B");
        assert_eq!(format_bytes(2048), "2.0 KiB");
        assert_eq!(format_bytes(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }
}
