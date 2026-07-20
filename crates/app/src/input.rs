use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::ui::PanelId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    Close,
    FocusNext,
    FocusPrev,
    FocusPanel(PanelId),
    SelectionUp,
    SelectionDown,
    OpenLogs,
    OpenHelp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Global,
    Navigation,
    Docker,
}

impl Context {
    const ORDER: [Self; 3] = [Self::Global, Self::Navigation, Self::Docker];

    fn title(self) -> &'static str {
        match self {
            Self::Global => "Global",
            Self::Navigation => "Navigation",
            Self::Docker => "Docker",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Binding {
    code: KeyCode,
    modifiers: KeyModifiers,
    action: Action,
    context: Context,
    description: &'static str,
}

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
        description: "close overlay / go back",
    },
    Binding {
        code: KeyCode::Char('?'),
        modifiers: KeyModifiers::NONE,
        action: Action::OpenHelp,
        context: Context::Global,
        description: "help",
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
        code: KeyCode::Char('1'),
        modifiers: KeyModifiers::NONE,
        action: Action::FocusPanel(PanelId::Cpu),
        context: Context::Navigation,
        description: "focus CPU panel",
    },
    Binding {
        code: KeyCode::Char('2'),
        modifiers: KeyModifiers::NONE,
        action: Action::FocusPanel(PanelId::Memory),
        context: Context::Navigation,
        description: "focus memory panel",
    },
    Binding {
        code: KeyCode::Char('3'),
        modifiers: KeyModifiers::NONE,
        action: Action::FocusPanel(PanelId::Docker),
        context: Context::Navigation,
        description: "focus Docker panel",
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

const DISTINGUISHING: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::ALT);

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

#[must_use]
pub fn help_lines() -> Vec<String> {
    let mut lines = Vec::new();
    for context in Context::ORDER {
        lines.push(format!("{}:", context.title()));
        for binding in BINDINGS.iter().filter(|b| b.context == context) {
            lines.push(format!("  {:<11} {}", key_label(binding), binding.description));
        }
        lines.push(String::new());
    }
    lines.pop();
    lines
}

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
    fn shift_noise_does_not_break_matching() {
        let question =
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT);
        assert_eq!(action_for(question), Some(Action::OpenHelp));
        let backtab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        assert_eq!(action_for(backtab), Some(Action::FocusPrev));
    }

    #[test]
    fn ctrl_is_distinguishing() {
        let ctrl_c =
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
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