//! The Git panel: branch, working-tree summary and recent commits.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use sysforge_git::collector::{GitSnapshot, GitStatus};

use super::{RenderCtx, components};
use crate::state::GitUiState;

/// Renders the Git panel from its UI state.
pub(super) fn render(frame: &mut Frame, area: Rect, git: &GitUiState, ctx: &RenderCtx<'_>) {
    let block = components::panel_block(" Git [4] ", ctx);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let placeholder = |frame: &mut Frame, text: &str| {
        frame.render_widget(
            Paragraph::new(text.to_owned()).style(Style::default().fg(ctx.theme.muted)),
            inner,
        );
    };

    match git {
        GitUiState::Disabled | GitUiState::Pending => placeholder(frame, "sampling..."),
        GitUiState::Observed(GitStatus::NotARepository) => {
            placeholder(frame, "not a git repository");
        }
        GitUiState::Observed(GitStatus::Unavailable { reason }) => {
            frame.render_widget(
                Paragraph::new(reason.clone()).style(Style::default().fg(ctx.theme.warning)),
                inner,
            );
        }
        GitUiState::Observed(GitStatus::Repository(snap)) => {
            render_repository(frame, inner, snap, ctx);
        }
    }
}

fn render_repository(frame: &mut Frame, area: Rect, snap: &GitSnapshot, ctx: &RenderCtx<'_>) {
    let [header_area, commits_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(area);

    let tree = snap.working_tree;
    let status_line = if tree.is_clean() {
        Span::styled("clean", Style::default().fg(ctx.theme.success))
    } else {
        Span::styled(
            format!(
                "{} staged · {} modified · {} untracked",
                tree.staged, tree.modified, tree.untracked
            ),
            Style::default().fg(ctx.theme.warning),
        )
    };
    let header = vec![
        Line::from(vec![
            Span::styled("branch ", Style::default().fg(ctx.theme.muted)),
            Span::styled(
                snap.branch.clone(),
                Style::default()
                    .fg(ctx.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(status_line),
    ];
    frame.render_widget(Paragraph::new(header), header_area);

    let commits: Vec<Line> = snap
        .commits
        .iter()
        .map(|c| {
            Line::from(vec![
                Span::styled(
                    format!("{} ", c.short_hash),
                    Style::default().fg(ctx.theme.accent),
                ),
                Span::raw(format!("{} ", c.summary)),
                Span::styled(
                    format!("· {} · {}", c.author, c.when),
                    Style::default().fg(ctx.theme.muted),
                ),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(commits), commits_area);
}
