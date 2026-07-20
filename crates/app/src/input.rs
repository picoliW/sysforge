//! Keyboard input translation and its documentation.
//!
//! Bindings are *data*: one declarative table drives both the runtime
//! keymap ([`action_for`]) and the help overlay ([`help_lines`]). A new
//! key is one new table row — help can never drift out of date.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::ui::ViewId;

/// Everything the user can ask the application to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Exit the application.
    Quit,
    /// Close the current context: an overlay, or a dedicated view.
    Close,
    /// Switch to a view.
    SwitchView(ViewId),
    /// Focus the next panel within the current view.
    FocusNext,
    /// Focus the previous panel within the current view.
    FocusPrev,
    /// Move the in-panel selection up.
    SelectionUp,
    /// Move the in-panel selection down.
    SelectionDown,
    /// Open the logs of the selected item.
    OpenLogs,
    /// Open the help overlay.
    OpenHelp,
}

/// Help section a binding belongs to. Presentation only: what an
/// action *means* is still decided in [`crate::ui`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    /// Always applicable.
    Global,
    /// Switching between views.
    Views,
    /// Moving between and inside panels.
    Navigation,
    /// Docker actions.
    Docker,
}

impl Context {
    /// Section order and titles in the help overlay.
    const ORDER: [Self; 4] = [Self::Global, Self::Views, Self::Navigation, Self::Docker];

    fn title(self) -> &'static str {
        match self {
            Self::Global => "Global",
            Self::Views => "Views",
            Self::Navigation => "Navigation",
            Self::Docker => "Docker",
        }
    }
}

/// One key binding: the single source of truth for behavior and help.
#[derive(Debug, Clone, Copy)]
pub struct Binding {
    code: KeyCode,
    modifiers: KeyModifiers,
    action: Action,
    context: Context,
    description: &'static str,
}

/// The complete keymap.
const BINDINGS: &[Binding] = &[
    Binding {
        code: KeyCode::Char('q'),
        modifiers: KeyModifiers::NONE,
        action: Action::Quit,
        context: Context::Global,
        description: "quit",
    },
    Binding {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::CONTROL,
        action: Action::Quit,
        context: Context::Global,
        description: "quit",
    },
    Binding {
        code: KeyCode::Esc,
        modifiers: KeyModifiers::NONE,
        action: Action::Close,
        context: Context::Global,
        description: "close overlay / back to overview",
    },
    Binding {
        code: KeyCode::Char('?'),
        modifiers: KeyModifiers::NONE,
        action: Action::OpenHelp,
        context: Context::Global,
        description: "help",
    },
    Binding {
        code: KeyCode::Char('1'),
        modifiers: KeyModifiers::NONE,
        action: Action::SwitchView(ViewId::Overview),
        context: Context::Views,
        description: "overview",
    },
    Binding {
        code: KeyCode::Char('2'),
        modifiers: KeyModifiers::NONE,
        action: Action::SwitchView(ViewId::Docker),
        context: Context::Views,
        description: "docker view",
    },
    Binding {
        code: KeyCode::Char('3'),
        modifiers: KeyModifiers::NONE,
        action: Action::SwitchView(ViewId::Processes),
        context: Context::Views,
        description: "processes view",
    },
    Binding {
        code: KeyCode::Tab,
        modifiers: KeyModifiers::NONE,
        action: Action::FocusNext,
        context: Context::Navigation,
        description: "focus next panel",
    },
    Binding {
        code: KeyCode::BackTab,
        modifiers: KeyModifiers::NONE,
        action: Action::FocusPrev,
        context: Context::Navigation,
        description: "focus previous panel",
    },
    Binding {
        code: KeyCode::Up,
        modifiers: KeyModifiers::NONE,
        action: Action::SelectionUp,
        context: Context::Navigation,
        description: "selection up / scroll up",
    },
    Binding {
        code: KeyCode::Down,
        modifiers: KeyModifiers::NONE,
        action: Action::SelectionDown,
        context: Context::Navigation,
        description: "selection down / scroll down",
    },
    Binding {
        code: KeyCode::Char('l'),
        modifiers: KeyModifiers::NONE,
        action: Action::OpenLogs,
        context: Context::Docker,
        description: "logs of the selected container",
    },
];

/// Modifier bits that distinguish bindings. SHIFT is deliberately
/// ignored: it is already encoded in the `KeyCode` itself (`?` *is*
/// shifted `/`; `BackTab` *is* shifted Tab), and terminals disagree on
/// whether to also set the SHIFT bit for those keys.
const DISTINGUISHING: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::ALT);

/// Maps a key event to an action, if it is bound to one.
#[must_use]
pub fn action_for(key: KeyEvent) -> Option<Action> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    let pressed = key.modifiers.intersection(DISTINGUISHING);
    BINDINGS
        .iter()
        .find(|b| b.code == key.code && b.modifiers == pressed)
        .map(|b| b.action)
}

/// The help overlay content, generated from the same table that drives
/// the keymap.
#[must_use]
pub fn help_lines() -> Vec<String> {
    let mut lines = Vec::new();
    for context in Context::ORDER {
        lines.push(format!("{}:", context.title()));
        for binding in BINDINGS.iter().filter(|b| b.context == context) {
            lines.push(format!(
                "  {:<11} {}",
                key_label(binding),
                binding.description
            ));
        }
        lines.push(String::new());
    }
    lines.pop();
    lines
}

/// Human-readable label for a binding's key.
fn key_label(binding: &Binding) -> String {
    let base = match binding.code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Esc => String::from("Esc"),
        KeyCode::Tab => String::from("Tab"),
        KeyCode::BackTab => String::from("Shift+Tab"),
        KeyCode::Up => String::from("↑"),
        KeyCode::Down => String::from("↓"),
        other => format!("{other:?}"),
    };
    if binding.modifiers.contains(KeyModifiers::CONTROL) {
        format!("Ctrl+{base}")
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn esc_closes_and_q_quits() {
        assert_eq!(action_for(press(KeyCode::Esc)), Some(Action::Close));
        assert_eq!(action_for(press(KeyCode::Char('q'))), Some(Action::Quit));
    }

    #[test]
    fn digits_switch_views() {
        assert_eq!(
            action_for(press(KeyCode::Char('2'))),
            Some(Action::SwitchView(ViewId::Docker))
        );
    }

    #[test]
    fn shift_noise_does_not_break_matching() {
        let question = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT);
        assert_eq!(action_for(question), Some(Action::OpenHelp));
        let backtab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        assert_eq!(action_for(backtab), Some(Action::FocusPrev));
    }

    #[test]
    fn ctrl_is_distinguishing() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(action_for(ctrl_c), Some(Action::Quit));
        assert_eq!(action_for(press(KeyCode::Char('c'))), None);
    }

    #[test]
    fn no_two_bindings_collide() {
        for (i, a) in BINDINGS.iter().enumerate() {
            for b in &BINDINGS[i + 1..] {
                assert!(
                    !(a.code == b.code && a.modifiers == b.modifiers),
                    "duplicate binding for {:?}",
                    a.code
                );
            }
        }
    }

    #[test]
    fn help_mentions_every_binding() {
        let help = help_lines().join("\n");
        for binding in BINDINGS {
            assert!(
                help.contains(binding.description),
                "help is missing {:?}",
                binding.action
            );
        }
    }
}
