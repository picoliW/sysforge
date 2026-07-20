use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::Paragraph;
use sysforge_system::memory::MemorySnapshot;

use super::components;
use crate::history::History;

pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    memory: Option<MemorySnapshot>,
    history: &History,
    focused: bool,
) {
    let block = components::panel_block(" Memory [2] ", focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(mem) = memory else {
        frame.render_widget(Paragraph::new("sampling..."), inner);
        return;
    };

    let [top_area, spark_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)])
            .margin(1)
            .areas(inner);

    let [gauge_area, details_area] =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Min(0)])
            .spacing(2)
            .areas(top_area);

    frame.render_widget(components::percent_gauge(mem.used_percent()), gauge_area);
    frame.render_widget(
        Paragraph::new(format!(
            "used {} / {}   swap {} / {}",
            components::format_bytes(mem.used()),
            components::format_bytes(mem.total),
            components::format_bytes(mem.swap_used()),
            components::format_bytes(mem.swap_total),
        )),
        details_area,
    );

    components::sparkline(frame, spark_area, history);
}