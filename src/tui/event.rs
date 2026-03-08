// Event loop: crossterm events + background I/O results.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use crate::app::{Action, View};

/// Events that can arrive in the main loop.
pub enum AppEvent {
    /// A crossterm terminal event.
    Terminal(Event),
    /// Sessions finished loading from the background thread.
    SessionsLoaded(
        Result<Vec<crate::data::model::SessionSummary>, crate::source::error::SourceError>,
    ),
    /// A single session finished loading from the background thread.
    SessionLoaded(Result<crate::data::model::Session, crate::source::error::SourceError>),
    /// A tick for periodic updates (resize detection, etc.).
    Tick,
}

/// Spawns a thread that reads crossterm events and sends them as AppEvent.
pub fn spawn_event_reader(tx: mpsc::Sender<AppEvent>) {
    thread::spawn(move || {
        loop {
            if event::poll(Duration::from_millis(100)).unwrap_or(false)
                && let Ok(evt) = event::read()
                && tx.send(AppEvent::Terminal(evt)).is_err()
            {
                break;
            }
        }
    });
}

/// Map a crossterm KeyEvent to an Action (if applicable).
pub fn map_key_to_action(key: KeyEvent, show_help: bool, view: &View) -> Option<Action> {
    // If help is showing, any key dismisses it.
    if show_help {
        return Some(Action::ToggleHelp);
    }

    // Global bindings (work in any view).
    match key.code {
        KeyCode::Char('q') => return Some(Action::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return Some(Action::Quit);
        }
        KeyCode::Char('?') => return Some(Action::ToggleHelp),
        KeyCode::Esc => return Some(Action::BackToList),
        _ => {}
    }

    // View-specific bindings.
    match view {
        View::SessionList => match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Action::NavigateDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::NavigateUp),
            KeyCode::Enter => Some(Action::SelectSession),
            _ => None,
        },
        View::Conversation(_) => match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Action::NavigateDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::NavigateUp),
            KeyCode::Char('n') => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    Some(Action::PrevTurn)
                } else {
                    Some(Action::NextTurn)
                }
            }
            KeyCode::Char('N') => Some(Action::PrevTurn),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::model::SessionId;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn key_with_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn session_list_view() -> View {
        View::SessionList
    }

    fn conversation_view() -> View {
        View::Conversation(SessionId("test".to_string()))
    }

    #[test]
    fn q_maps_to_quit() {
        let action = map_key_to_action(key(KeyCode::Char('q')), false, &session_list_view());
        assert!(matches!(action, Some(Action::Quit)));
    }

    #[test]
    fn j_maps_to_navigate_down_in_session_list() {
        let action = map_key_to_action(key(KeyCode::Char('j')), false, &session_list_view());
        assert!(matches!(action, Some(Action::NavigateDown)));
    }

    #[test]
    fn k_maps_to_navigate_up_in_session_list() {
        let action = map_key_to_action(key(KeyCode::Char('k')), false, &session_list_view());
        assert!(matches!(action, Some(Action::NavigateUp)));
    }

    #[test]
    fn arrow_down_maps_to_navigate_down() {
        let action = map_key_to_action(key(KeyCode::Down), false, &session_list_view());
        assert!(matches!(action, Some(Action::NavigateDown)));
    }

    #[test]
    fn arrow_up_maps_to_navigate_up() {
        let action = map_key_to_action(key(KeyCode::Up), false, &session_list_view());
        assert!(matches!(action, Some(Action::NavigateUp)));
    }

    #[test]
    fn enter_maps_to_select_session_in_session_list() {
        let action = map_key_to_action(key(KeyCode::Enter), false, &session_list_view());
        assert!(matches!(action, Some(Action::SelectSession)));
    }

    #[test]
    fn esc_maps_to_back_to_list() {
        let action = map_key_to_action(key(KeyCode::Esc), false, &session_list_view());
        assert!(matches!(action, Some(Action::BackToList)));
    }

    #[test]
    fn question_mark_toggles_help() {
        let action = map_key_to_action(key(KeyCode::Char('?')), false, &session_list_view());
        assert!(matches!(action, Some(Action::ToggleHelp)));
    }

    #[test]
    fn ctrl_c_maps_to_quit() {
        let action = map_key_to_action(
            key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL),
            false,
            &session_list_view(),
        );
        assert!(matches!(action, Some(Action::Quit)));
    }

    #[test]
    fn unknown_key_returns_none() {
        let action = map_key_to_action(key(KeyCode::Char('x')), false, &session_list_view());
        assert!(action.is_none());
    }

    #[test]
    fn help_visible_any_key_dismisses() {
        let action = map_key_to_action(key(KeyCode::Char('x')), true, &session_list_view());
        assert!(matches!(action, Some(Action::ToggleHelp)));
    }

    #[test]
    fn n_maps_to_next_turn_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Char('n')), false, &conversation_view());
        assert!(matches!(action, Some(Action::NextTurn)));
    }

    #[test]
    fn shift_n_maps_to_prev_turn_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Char('N')), false, &conversation_view());
        assert!(matches!(action, Some(Action::PrevTurn)));
    }

    #[test]
    fn j_scrolls_down_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Char('j')), false, &conversation_view());
        assert!(matches!(action, Some(Action::NavigateDown)));
    }

    #[test]
    fn k_scrolls_up_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Char('k')), false, &conversation_view());
        assert!(matches!(action, Some(Action::NavigateUp)));
    }

    #[test]
    fn enter_does_nothing_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Enter), false, &conversation_view());
        assert!(action.is_none());
    }

    #[test]
    fn esc_maps_to_back_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Esc), false, &conversation_view());
        assert!(matches!(action, Some(Action::BackToList)));
    }

    #[test]
    fn shift_n_with_modifier_maps_to_prev_turn() {
        let action = map_key_to_action(
            key_with_mod(KeyCode::Char('n'), KeyModifiers::SHIFT),
            false,
            &conversation_view(),
        );
        assert!(matches!(action, Some(Action::PrevTurn)));
    }

    #[test]
    fn q_maps_to_quit_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Char('q')), false, &conversation_view());
        assert!(matches!(action, Some(Action::Quit)));
    }
}
