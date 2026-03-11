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
    /// Projects finished loading from the background thread.
    ProjectsLoaded(
        Result<Vec<crate::data::model::ProjectSummary>, crate::source::error::SourceError>,
    ),
    /// Sessions finished loading from the background thread.
    SessionsLoaded(
        Result<Vec<crate::data::model::SessionSummary>, crate::source::error::SourceError>,
    ),
    /// A single session finished loading from the background thread.
    SessionLoaded(Result<crate::data::model::Session, crate::source::error::SourceError>),
    /// Claude Code version was detected.
    VersionLoaded(String),
    /// Usage data was fetched.
    UsageLoaded(crate::data::usage::UsageData),
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
pub fn map_key_to_action(
    key: KeyEvent,
    show_help: bool,
    view: &View,
    modal_active: bool,
) -> Option<Action> {
    // If help is showing, any key dismisses it.
    if show_help {
        return Some(Action::ToggleHelp);
    }

    // Modal bindings take priority when a modal is open.
    if modal_active {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Some(Action::DismissModal),
            KeyCode::Char('j') | KeyCode::Down => Some(Action::ModalScrollDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::ModalScrollUp),
            _ => None,
        };
    }

    // Global bindings (work in any view).
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => return Some(Action::BackToList),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return Some(Action::Quit);
        }
        KeyCode::Char('?') => return Some(Action::ToggleHelp),
        _ => {}
    }

    // View-specific bindings.
    match view {
        View::ProjectList => match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Action::NavigateDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::NavigateUp),
            KeyCode::Enter => Some(Action::SelectProject),
            _ => None,
        },
        View::SessionList => match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Action::NavigateDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::NavigateUp),
            KeyCode::Enter => Some(Action::SelectSession),
            _ => None,
        },
        View::Conversation(_) => match key.code {
            KeyCode::Char('j') => Some(Action::NextTurn),
            KeyCode::Char('k') => Some(Action::PrevTurn),
            KeyCode::Down => Some(Action::NavigateDown),
            KeyCode::Up => Some(Action::NavigateUp),
            KeyCode::Char('u') => Some(Action::ShowUserModal),
            KeyCode::Char('c') => Some(Action::ShowClaudeModal),
            KeyCode::Char('t') => Some(Action::ToggleTokens),
            KeyCode::Char('o') => Some(Action::ToggleTools),
            KeyCode::Char('T') => Some(Action::ToggleThinking),
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

    fn project_list_view() -> View {
        View::ProjectList
    }

    fn session_list_view() -> View {
        View::SessionList
    }

    fn conversation_view() -> View {
        View::Conversation(SessionId("test".to_string()))
    }

    #[test]
    fn enter_maps_to_select_project_in_project_list() {
        let action = map_key_to_action(key(KeyCode::Enter), false, &project_list_view(), false);
        assert!(matches!(action, Some(Action::SelectProject)));
    }

    #[test]
    fn j_maps_to_navigate_down_in_project_list() {
        let action = map_key_to_action(key(KeyCode::Char('j')), false, &project_list_view(), false);
        assert!(matches!(action, Some(Action::NavigateDown)));
    }

    #[test]
    fn k_maps_to_navigate_up_in_project_list() {
        let action = map_key_to_action(key(KeyCode::Char('k')), false, &project_list_view(), false);
        assert!(matches!(action, Some(Action::NavigateUp)));
    }

    #[test]
    fn esc_maps_to_back_in_project_list() {
        let action = map_key_to_action(key(KeyCode::Esc), false, &project_list_view(), false);
        assert!(matches!(action, Some(Action::BackToList)));
    }

    #[test]
    fn q_maps_to_back_to_list() {
        let action = map_key_to_action(key(KeyCode::Char('q')), false, &session_list_view(), false);
        assert!(matches!(action, Some(Action::BackToList)));
    }

    #[test]
    fn j_maps_to_navigate_down_in_session_list() {
        let action = map_key_to_action(key(KeyCode::Char('j')), false, &session_list_view(), false);
        assert!(matches!(action, Some(Action::NavigateDown)));
    }

    #[test]
    fn k_maps_to_navigate_up_in_session_list() {
        let action = map_key_to_action(key(KeyCode::Char('k')), false, &session_list_view(), false);
        assert!(matches!(action, Some(Action::NavigateUp)));
    }

    #[test]
    fn arrow_down_maps_to_navigate_down() {
        let action = map_key_to_action(key(KeyCode::Down), false, &session_list_view(), false);
        assert!(matches!(action, Some(Action::NavigateDown)));
    }

    #[test]
    fn arrow_up_maps_to_navigate_up() {
        let action = map_key_to_action(key(KeyCode::Up), false, &session_list_view(), false);
        assert!(matches!(action, Some(Action::NavigateUp)));
    }

    #[test]
    fn enter_maps_to_select_session_in_session_list() {
        let action = map_key_to_action(key(KeyCode::Enter), false, &session_list_view(), false);
        assert!(matches!(action, Some(Action::SelectSession)));
    }

    #[test]
    fn esc_maps_to_back_to_list() {
        let action = map_key_to_action(key(KeyCode::Esc), false, &session_list_view(), false);
        assert!(matches!(action, Some(Action::BackToList)));
    }

    #[test]
    fn question_mark_toggles_help() {
        let action = map_key_to_action(key(KeyCode::Char('?')), false, &session_list_view(), false);
        assert!(matches!(action, Some(Action::ToggleHelp)));
    }

    #[test]
    fn ctrl_c_maps_to_quit() {
        let action = map_key_to_action(
            key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL),
            false,
            &session_list_view(),
            false,
        );
        assert!(matches!(action, Some(Action::Quit)));
    }

    #[test]
    fn unknown_key_returns_none() {
        let action = map_key_to_action(key(KeyCode::Char('x')), false, &session_list_view(), false);
        assert!(action.is_none());
    }

    #[test]
    fn help_visible_any_key_dismisses() {
        let action = map_key_to_action(key(KeyCode::Char('x')), true, &session_list_view(), false);
        assert!(matches!(action, Some(Action::ToggleHelp)));
    }

    #[test]
    fn j_maps_to_next_turn_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Char('j')), false, &conversation_view(), false);
        assert!(matches!(action, Some(Action::NextTurn)));
    }

    #[test]
    fn k_maps_to_prev_turn_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Char('k')), false, &conversation_view(), false);
        assert!(matches!(action, Some(Action::PrevTurn)));
    }

    #[test]
    fn arrow_down_scrolls_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Down), false, &conversation_view(), false);
        assert!(matches!(action, Some(Action::NavigateDown)));
    }

    #[test]
    fn arrow_up_scrolls_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Up), false, &conversation_view(), false);
        assert!(matches!(action, Some(Action::NavigateUp)));
    }

    #[test]
    fn u_maps_to_show_user_modal_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Char('u')), false, &conversation_view(), false);
        assert!(matches!(action, Some(Action::ShowUserModal)));
    }

    #[test]
    fn c_maps_to_show_claude_modal_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Char('c')), false, &conversation_view(), false);
        assert!(matches!(action, Some(Action::ShowClaudeModal)));
    }

    #[test]
    fn enter_does_nothing_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Enter), false, &conversation_view(), false);
        assert!(action.is_none());
    }

    #[test]
    fn esc_maps_to_back_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Esc), false, &conversation_view(), false);
        assert!(matches!(action, Some(Action::BackToList)));
    }

    #[test]
    fn t_maps_to_toggle_tokens_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Char('t')), false, &conversation_view(), false);
        assert!(matches!(action, Some(Action::ToggleTokens)));
    }

    #[test]
    fn t_does_nothing_in_session_list() {
        let action = map_key_to_action(key(KeyCode::Char('t')), false, &session_list_view(), false);
        assert!(action.is_none());
    }

    #[test]
    fn q_maps_to_back_to_list_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Char('q')), false, &conversation_view(), false);
        assert!(matches!(action, Some(Action::BackToList)));
    }

    #[test]
    fn o_maps_to_toggle_tools_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Char('o')), false, &conversation_view(), false);
        assert!(matches!(action, Some(Action::ToggleTools)));
    }

    #[test]
    fn o_does_nothing_in_session_list() {
        let action = map_key_to_action(key(KeyCode::Char('o')), false, &session_list_view(), false);
        assert!(action.is_none());
    }

    #[test]
    fn shift_t_maps_to_toggle_thinking_in_conversation() {
        let action = map_key_to_action(key(KeyCode::Char('T')), false, &conversation_view(), false);
        assert!(matches!(action, Some(Action::ToggleThinking)));
    }

    #[test]
    fn shift_t_does_nothing_in_session_list() {
        let action = map_key_to_action(key(KeyCode::Char('T')), false, &session_list_view(), false);
        assert!(action.is_none());
    }

    // --- Modal key handling tests ---

    #[test]
    fn modal_esc_dismisses_modal() {
        let action = map_key_to_action(key(KeyCode::Esc), false, &conversation_view(), true);
        assert!(matches!(action, Some(Action::DismissModal)));
    }

    #[test]
    fn modal_q_dismisses_modal() {
        let action = map_key_to_action(key(KeyCode::Char('q')), false, &conversation_view(), true);
        assert!(matches!(action, Some(Action::DismissModal)));
    }

    #[test]
    fn modal_j_scrolls_down() {
        let action = map_key_to_action(key(KeyCode::Char('j')), false, &conversation_view(), true);
        assert!(matches!(action, Some(Action::ModalScrollDown)));
    }

    #[test]
    fn modal_k_scrolls_up() {
        let action = map_key_to_action(key(KeyCode::Char('k')), false, &conversation_view(), true);
        assert!(matches!(action, Some(Action::ModalScrollUp)));
    }

    #[test]
    fn modal_arrow_down_scrolls_down() {
        let action = map_key_to_action(key(KeyCode::Down), false, &conversation_view(), true);
        assert!(matches!(action, Some(Action::ModalScrollDown)));
    }

    #[test]
    fn modal_arrow_up_scrolls_up() {
        let action = map_key_to_action(key(KeyCode::Up), false, &conversation_view(), true);
        assert!(matches!(action, Some(Action::ModalScrollUp)));
    }

    #[test]
    fn modal_other_keys_return_none() {
        let action = map_key_to_action(key(KeyCode::Char('x')), false, &conversation_view(), true);
        assert!(action.is_none());
    }
}
