//! Semantic color roles of the interface.
//!
//! Panels never name hues: they ask for roles ("accent", "muted") and
//! the theme decides the color. Users override any role in the
//! `[theme]` section of the configuration, using color names
//! (`"cyan"`, `"light blue"`) or hex values (`"#ff8800"`).

use ratatui::style::Color;
use serde::Deserialize;

/// The color assigned to each visual role (`[theme]`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    /// Highlights: focused borders, gauges, sparklines.
    pub accent: Color,
    /// Panel borders at rest.
    pub border: Color,
    /// Primary text where an explicit color is needed.
    pub text: Color,
    /// De-emphasized content: placeholders, stopped containers.
    pub muted: Color,
    /// Healthy / running.
    pub success: Color,
    /// Degraded but expected states, such as an offline domain.
    pub warning: Color,
    /// Failures.
    pub error: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent: Color::Cyan,
            border: Color::DarkGray,
            text: Color::Reset,
            muted: Color::DarkGray,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
        }
    }
}
