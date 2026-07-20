use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, BorderType, Borders, Gauge, Sparkline};

use crate::history::History;

pub(super) fn panel_block(title: &str, focused: bool) -> Block<'_> {
    let border = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border)
}

pub(super) fn percent_gauge(percent: f64) -> Gauge<'static> {
    Gauge::default()
        .ratio((percent / 100.0).clamp(0.0, 1.0))
        .label(format!("{percent:.1}%"))
        .gauge_style(Style::default().fg(Color::Cyan))
}

pub(super) fn sparkline(frame: &mut Frame, area: Rect, history: &History) {
    let data = history.last(area.width as usize);
    let spark = Sparkline::default()
        .data(&data)
        .max(100)
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(spark, area);
}

#[allow(clippy::cast_precision_loss)]
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
