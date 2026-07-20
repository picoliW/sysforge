//! The CPU panel.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::{Paragraph, Wrap};
use sysforge_system::cpu::CpuSnapshot;

use super::{RenderCtx, components};
use crate::history::History;

/// Renders the CPU panel: aggregate gauge, history sparkline and
/// per-core utilization.
pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    cpu: Option<&CpuSnapshot>,
    history: &History,
    ctx: &RenderCtx<'_>,
) {
    let block = components::panel_block(" CPU [1] ", ctx);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(cpu) = cpu else {
        frame.render_widget(Paragraph::new("sampling..."), inner);
        return;
    };

    let [gauge_area, spark_area, cores_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .margin(1)
    .areas(inner);

    frame.render_widget(components::percent_gauge(cpu.total, ctx.theme), gauge_area);
    components::sparkline(frame, spark_area, history, ctx.theme);

    let cores = cpu
        .per_core
        .iter()
        .enumerate()
        .map(|(i, pct)| format!("c{i:02} {pct:5.1}%"))
        .collect::<Vec<_>>()
        .join("   ");
    frame.render_widget(Paragraph::new(cores).wrap(Wrap { trim: true }), cores_area);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::theme::Theme;

    #[test]
    fn renders_placeholder_before_first_sample() {
        let theme = Theme::default();
        let ctx = RenderCtx {
            theme: &theme,
            focused: false,
        };
        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), None, &History::new(10), &ctx);
            })
            .expect("draw must succeed");

        let content: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(content.contains("sampling..."));
    }
}
