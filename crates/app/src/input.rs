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
}

#[must_use]
pub fn action_for(key: KeyEvent) -> Option<Action> {
    if key.kind != KeyEventKind::Press {
        return None; 
    }
    let ctrl_c = key.code == KeyCode::Char('c')
        && key.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl_c {
        return Some(Action::Quit);
    }
    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Esc => Some(Action::Close),
        KeyCode::Tab => Some(Action::FocusNext),
        KeyCode::BackTab => Some(Action::FocusPrev),
        KeyCode::Up => Some(Action::SelectionUp),
        KeyCode::Down => Some(Action::SelectionDown),
        KeyCode::Char('l') => Some(Action::OpenLogs),
        KeyCode::Char('1') => Some(Action::FocusPanel(PanelId::Cpu)),
        KeyCode::Char('2') => Some(Action::FocusPanel(PanelId::Memory)),
        KeyCode::Char('3') => Some(Action::FocusPanel(PanelId::Docker)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn esc_closes_instead_of_quitting() {
        assert_eq!(action_for(press(KeyCode::Esc)), Some(Action::Close));
        assert_eq!(action_for(press(KeyCode::Char('q'))), Some(Action::Quit));
    }

    #[test]
    fn l_opens_logs() {
        assert_eq!(action_for(press(KeyCode::Char('l'))), Some(Action::OpenLogs));
    }

    #[test]
    fn tab_cycles_and_backtab_reverses() {
        assert_eq!(action_for(press(KeyCode::Tab)), Some(Action::FocusNext));
        assert_eq!(action_for(press(KeyCode::BackTab)), Some(Action::FocusPrev));
    }

    #[test]
    fn unbound_keys_do_nothing() {
        assert_eq!(action_for(press(KeyCode::Char('x'))), None);
    }
}