// Event loop: crossterm events + background I/O results.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use crate::app::Action;

/// Events that can arrive in the main loop.
pub enum AppEvent {
    /// A crossterm terminal event.
    Terminal(Event),
    /// Sessions finished loading from the background thread.
    SessionsLoaded(
        Result<Vec<crate::data::model::SessionSummary>, crate::source::error::SourceError>,
    ),
    /// A tick for periodic updates (resize detection, etc.).
    Tick,
}

/// Spawns a thread that reads crossterm events and sends them as AppEvent.
pub fn spawn_event_reader(tx: mpsc::Sender<AppEvent>) {
    thread::spawn(move || {
        loop {
            // Poll with a timeout so we can detect when the channel is dropped.
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
pub fn map_key_to_action(key: KeyEvent, show_help: bool) -> Option<Action> {
    // If help is showing, any key dismisses it.
    if show_help {
        return Some(Action::ToggleHelp);
    }

    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('j') | KeyCode::Down => Some(Action::NavigateDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::NavigateUp),
        KeyCode::Enter => Some(Action::SelectSession),
        KeyCode::Esc => Some(Action::BackToList),
        KeyCode::Char('?') => Some(Action::ToggleHelp),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn q_maps_to_quit() {
        let action = map_key_to_action(key(KeyCode::Char('q')), false);
        assert!(matches!(action, Some(Action::Quit)));
    }

    #[test]
    fn j_maps_to_navigate_down() {
        let action = map_key_to_action(key(KeyCode::Char('j')), false);
        assert!(matches!(action, Some(Action::NavigateDown)));
    }

    #[test]
    fn k_maps_to_navigate_up() {
        let action = map_key_to_action(key(KeyCode::Char('k')), false);
        assert!(matches!(action, Some(Action::NavigateUp)));
    }

    #[test]
    fn arrow_down_maps_to_navigate_down() {
        let action = map_key_to_action(key(KeyCode::Down), false);
        assert!(matches!(action, Some(Action::NavigateDown)));
    }

    #[test]
    fn arrow_up_maps_to_navigate_up() {
        let action = map_key_to_action(key(KeyCode::Up), false);
        assert!(matches!(action, Some(Action::NavigateUp)));
    }

    #[test]
    fn enter_maps_to_select_session() {
        let action = map_key_to_action(key(KeyCode::Enter), false);
        assert!(matches!(action, Some(Action::SelectSession)));
    }

    #[test]
    fn esc_maps_to_back_to_list() {
        let action = map_key_to_action(key(KeyCode::Esc), false);
        assert!(matches!(action, Some(Action::BackToList)));
    }

    #[test]
    fn question_mark_toggles_help() {
        let action = map_key_to_action(key(KeyCode::Char('?')), false);
        assert!(matches!(action, Some(Action::ToggleHelp)));
    }

    #[test]
    fn ctrl_c_maps_to_quit() {
        let action = map_key_to_action(
            key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL),
            false,
        );
        assert!(matches!(action, Some(Action::Quit)));
    }

    #[test]
    fn unknown_key_returns_none() {
        let action = map_key_to_action(key(KeyCode::Char('x')), false);
        assert!(action.is_none());
    }

    #[test]
    fn help_visible_any_key_dismisses() {
        let action = map_key_to_action(key(KeyCode::Char('x')), true);
        assert!(matches!(action, Some(Action::ToggleHelp)));
    }
}
