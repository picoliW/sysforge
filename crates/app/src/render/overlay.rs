//! The generic modal overlay: a titled, scrollable text view drawn on
//! top of whatever is behind it.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::{Clear, Paragraph};

use super::{RenderCtx, components};
use crate::theme::Theme;
use crate::ui::Overlay;

/// Renders the overlay centered over the frame. An open overlay always
/// has focus.
pub(super) fn render(frame: &mut Frame, overlay: &Overlay, theme: &Theme) {
    let ctx = RenderCtx {
        theme,
        focused: true,
    };
    let area = centered(frame.area(), 84, 80);
    frame.render_widget(Clear, area);

    let block =
        components::panel_block(&overlay.title, &ctx).title_bottom(" Esc close · ↑/↓ scroll ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    frame.render_widget(
        Paragraph::new(overlay.lines.join("\n")).scroll((overlay.scroll, 0)),
        inner,
    );
}

/// A rect covering the given percentages of `area`, centered.
fn centered(area: Rect, width_pct: u16, height_pct: u16) -> Rect {
    let [_, mid, _] = Layout::vertical([
        Constraint::Percentage((100 - height_pct) / 2),
        Constraint::Percentage(height_pct),
        Constraint::Percentage((100 - height_pct) / 2),
    ])
    .areas(area);
    let [_, rect, _] = Layout::horizontal([
        Constraint::Percentage((100 - width_pct) / 2),
        Constraint::Percentage(width_pct),
        Constraint::Percentage((100 - width_pct) / 2),
    ])
    .areas(mid);
    rect
}
