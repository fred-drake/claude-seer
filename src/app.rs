// Application state machine -- pure logic, no TUI dependencies.

use crate::data::model::{ProjectPath, Session, SessionId, SessionSummary};
use crate::data::usage::{TitleBarInfo, UsageData};

/// The view the TUI should render.
#[derive(Debug, PartialEq, Eq)]
pub enum View {
    SessionList,
    Conversation(SessionId),
}

/// Empty state conditions for display.
#[derive(Debug, PartialEq, Eq)]
pub enum EmptyState {
    /// No ~/.claude/ directory found.
    NoDirectory,
    /// Directory exists but no sessions found.
    NoSessions,
    /// Session list is loading.
    Loading,
    /// Selected session has 0 turns.
    EmptySession,
}

/// All possible user/system actions.
#[derive(Debug)]
pub enum Action {
    Quit,
    NavigateUp,
    NavigateDown,
    SelectSession,
    BackToList,
    NextTurn,
    PrevTurn,
    Resize(u16, u16),
    SessionsLoaded(Vec<SessionSummary>),
    SessionLoaded(Box<Session>),
    SessionLoadError(String),
    LoadError(String),
    ToggleHelp,
    ToggleTokens,
    VersionLoaded(String),
    UsageLoaded(UsageData),
}

/// Side effects that the caller must execute.
#[derive(Debug, PartialEq, Eq)]
pub enum SideEffect {
    Exit,
    LoadSessionList,
    LoadSession(SessionId),
}

/// Application state.
pub struct AppState {
    pub view: View,
    pub empty_state: Option<EmptyState>,
    pub sessions: Vec<SessionSummary>,
    pub selected_index: usize,
    pub show_help: bool,
    pub show_tokens: bool,
    pub terminal_size: (u16, u16),
    pub current_session: Option<Session>,
    pub current_turn_index: usize,
    pub scroll_offset: usize,
    pub last_error: Option<String>,
    pub title_bar: TitleBarInfo,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            view: View::SessionList,
            empty_state: Some(EmptyState::Loading),
            sessions: Vec::new(),
            selected_index: 0,
            show_help: false,
            show_tokens: true,
            terminal_size: (80, 24),
            current_session: None,
            current_turn_index: 0,
            scroll_offset: 0,
            last_error: None,
            title_bar: TitleBarInfo::new(),
        }
    }

    /// Group sessions by project, preserving sort order within groups.
    pub fn grouped_sessions(&self) -> Vec<(&ProjectPath, Vec<&SessionSummary>)> {
        let mut groups: Vec<(&ProjectPath, Vec<&SessionSummary>)> = Vec::new();
        for session in &self.sessions {
            if let Some(group) = groups.iter_mut().find(|(p, _)| **p == session.project) {
                group.1.push(session);
            } else {
                groups.push((&session.project, vec![session]));
            }
        }
        groups
    }

    /// Pure state machine: process action, return optional side effect.
    pub fn handle_action(&mut self, action: Action) -> Option<SideEffect> {
        match action {
            Action::Quit => Some(SideEffect::Exit),

            Action::SessionsLoaded(mut summaries) => {
                // Sort by last_activity descending (most recent first).
                summaries.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
                if summaries.is_empty() {
                    self.empty_state = Some(EmptyState::NoSessions);
                } else {
                    self.empty_state = None;
                }
                self.sessions = summaries;
                self.selected_index = 0;
                None
            }

            Action::LoadError(_msg) => {
                // For M2, all load errors are treated as "no directory" since
                // that's the most common case. Future milestones may
                // differentiate error types (permission denied, corrupt data,
                // etc.) and show more specific guidance.
                self.empty_state = Some(EmptyState::NoDirectory);
                None
            }

            Action::NavigateDown => match &self.view {
                View::SessionList => {
                    if !self.sessions.is_empty() && self.selected_index < self.sessions.len() - 1 {
                        self.selected_index += 1;
                    }
                    None
                }
                View::Conversation(_) => {
                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                    None
                }
            },

            Action::NavigateUp => match &self.view {
                View::SessionList => {
                    if !self.sessions.is_empty() && self.selected_index > 0 {
                        self.selected_index -= 1;
                    }
                    None
                }
                View::Conversation(_) => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    None
                }
            },

            Action::SelectSession => {
                if self.sessions.is_empty() {
                    return None;
                }
                let id = self.sessions[self.selected_index].id.clone();
                self.view = View::Conversation(id.clone());
                self.empty_state = Some(EmptyState::Loading);
                self.current_session = None;
                self.current_turn_index = 0;
                self.scroll_offset = 0;
                Some(SideEffect::LoadSession(id))
            }

            Action::BackToList => match &self.view {
                View::SessionList => Some(SideEffect::Exit),
                View::Conversation(_) => {
                    self.view = View::SessionList;
                    self.current_session = None;
                    self.current_turn_index = 0;
                    self.scroll_offset = 0;
                    self.empty_state = if self.sessions.is_empty() {
                        Some(EmptyState::NoSessions)
                    } else {
                        None
                    };
                    None
                }
            },

            Action::ToggleHelp => {
                self.show_help = !self.show_help;
                None
            }

            Action::ToggleTokens => {
                self.show_tokens = !self.show_tokens;
                None
            }

            Action::SessionLoaded(session) => {
                self.view = View::Conversation(session.id.clone());
                self.empty_state = if session.turns.is_empty() {
                    Some(EmptyState::EmptySession)
                } else {
                    None
                };
                self.current_session = Some(*session);
                self.current_turn_index = 0;
                self.scroll_offset = 0;
                self.last_error = None;
                None
            }

            Action::SessionLoadError(msg) => {
                self.view = View::SessionList;
                self.last_error = Some(msg);
                None
            }

            Action::NextTurn => {
                if let Some(ref session) = self.current_session
                    && !session.turns.is_empty()
                    && self.current_turn_index < session.turns.len() - 1
                {
                    self.current_turn_index += 1;
                    self.scroll_offset = 0;
                }
                None
            }

            Action::PrevTurn => {
                if self.current_session.is_some() && self.current_turn_index > 0 {
                    self.current_turn_index -= 1;
                    self.scroll_offset = 0;
                }
                None
            }

            Action::Resize(w, h) => {
                self.terminal_size = (w, h);
                None
            }

            Action::VersionLoaded(version) => {
                self.title_bar.claude_version = Some(version);
                None
            }

            Action::UsageLoaded(usage) => {
                self.title_bar.usage = Some(usage);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_app_state_starts_in_session_list_view() {
        let state = AppState::new();
        assert!(matches!(state.view, View::SessionList));
    }

    #[test]
    fn new_app_state_empty_state_is_loading() {
        let state = AppState::new();
        assert_eq!(state.empty_state, Some(EmptyState::Loading));
    }

    #[test]
    fn handle_quit_returns_exit() {
        let mut state = AppState::new();
        let effect = state.handle_action(Action::Quit);
        assert_eq!(effect, Some(SideEffect::Exit));
    }

    fn make_session(id: &str, turn_count: usize) -> Session {
        use crate::data::model::{MessageId, TokenUsage, Turn, UserContent, UserMessage};
        use std::path::PathBuf;

        let turns: Vec<Turn> = (0..turn_count)
            .map(|i| Turn {
                index: i,
                user_message: UserMessage {
                    id: MessageId(format!("msg-user-{i}")),
                    timestamp: chrono::Utc::now(),
                    content: UserContent::Text(format!("User message {i}")),
                },
                assistant_response: None,
                duration: None,
                is_complete: true,
                events: Vec::new(),
            })
            .collect();

        Session {
            id: SessionId(id.to_string()),
            project: ProjectPath(PathBuf::from("test-project")),
            file_path: PathBuf::from(format!("/tmp/{id}.jsonl")),
            version: Some("2.1.71".to_string()),
            git_branch: Some("main".to_string()),
            started_at: None,
            last_activity: None,
            last_prompt: Some("test prompt".to_string()),
            turns,
            token_totals: TokenUsage::default(),
            parse_warnings: Vec::new(),
        }
    }

    fn make_summaries(count: usize) -> Vec<SessionSummary> {
        use crate::data::model::{ProjectPath, TokenUsage};
        use std::path::PathBuf;

        (0..count)
            .map(|i| SessionSummary {
                id: SessionId(format!("session-{i}")),
                project: ProjectPath(PathBuf::from("test-project")),
                file_path: PathBuf::from(format!("/tmp/session-{i}.jsonl")),
                file_size: 1000,
                last_prompt: Some(format!("prompt {i}")),
                started_at: None,
                last_activity: None,
                turn_count: 5,
                total_tokens: TokenUsage::default(),
                git_branch: Some("main".to_string()),
            })
            .collect()
    }

    #[test]
    fn sessions_loaded_clears_loading_state() {
        let mut state = AppState::new();
        let summaries = make_summaries(3);
        let effect = state.handle_action(Action::SessionsLoaded(summaries));
        assert!(effect.is_none());
        assert_eq!(state.sessions.len(), 3);
        assert_eq!(state.empty_state, None);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn sessions_loaded_empty_sets_no_sessions() {
        let mut state = AppState::new();
        let effect = state.handle_action(Action::SessionsLoaded(Vec::new()));
        assert!(effect.is_none());
        assert_eq!(state.empty_state, Some(EmptyState::NoSessions));
    }

    #[test]
    fn load_error_no_directory() {
        let mut state = AppState::new();
        let effect = state.handle_action(Action::LoadError("directory not found".to_string()));
        assert!(effect.is_none());
        assert_eq!(state.empty_state, Some(EmptyState::NoDirectory));
    }

    #[test]
    fn navigate_down_increments_index() {
        let mut state = AppState::new();
        state.handle_action(Action::SessionsLoaded(make_summaries(3)));
        assert_eq!(state.selected_index, 0);
        let effect = state.handle_action(Action::NavigateDown);
        assert_eq!(effect, None);
        assert_eq!(state.selected_index, 1);
        let effect = state.handle_action(Action::NavigateDown);
        assert_eq!(effect, None);
        assert_eq!(state.selected_index, 2);
    }

    #[test]
    fn navigate_down_stops_at_bottom() {
        let mut state = AppState::new();
        state.handle_action(Action::SessionsLoaded(make_summaries(2)));
        state.handle_action(Action::NavigateDown);
        let effect = state.handle_action(Action::NavigateDown);
        assert_eq!(effect, None);
        state.handle_action(Action::NavigateDown);
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn navigate_down_single_element_stays_at_zero() {
        let mut state = AppState::new();
        state.handle_action(Action::SessionsLoaded(make_summaries(1)));
        let effect = state.handle_action(Action::NavigateDown);
        assert_eq!(effect, None);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn navigate_up_decrements_index() {
        let mut state = AppState::new();
        state.handle_action(Action::SessionsLoaded(make_summaries(3)));
        state.handle_action(Action::NavigateDown);
        state.handle_action(Action::NavigateDown);
        let effect = state.handle_action(Action::NavigateUp);
        assert_eq!(effect, None);
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn navigate_up_stops_at_top() {
        let mut state = AppState::new();
        state.handle_action(Action::SessionsLoaded(make_summaries(3)));
        let effect = state.handle_action(Action::NavigateUp);
        assert_eq!(effect, None);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn navigate_on_empty_does_nothing() {
        let mut state = AppState::new();
        state.handle_action(Action::SessionsLoaded(Vec::new()));
        let effect = state.handle_action(Action::NavigateDown);
        assert_eq!(effect, None);
        assert_eq!(state.selected_index, 0);
        let effect = state.handle_action(Action::NavigateUp);
        assert_eq!(effect, None);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn select_session_returns_load_effect() {
        let mut state = AppState::new();
        state.handle_action(Action::SessionsLoaded(make_summaries(3)));
        state.handle_action(Action::NavigateDown); // index 1
        let effect = state.handle_action(Action::SelectSession);
        assert_eq!(
            effect,
            Some(SideEffect::LoadSession(SessionId("session-1".to_string())))
        );
    }

    #[test]
    fn select_session_on_empty_returns_none() {
        let mut state = AppState::new();
        state.handle_action(Action::SessionsLoaded(Vec::new()));
        let effect = state.handle_action(Action::SelectSession);
        assert!(effect.is_none());
    }

    #[test]
    fn toggle_help() {
        let mut state = AppState::new();
        assert!(!state.show_help);
        let effect = state.handle_action(Action::ToggleHelp);
        assert_eq!(effect, None);
        assert!(state.show_help);
        let effect = state.handle_action(Action::ToggleHelp);
        assert_eq!(effect, None);
        assert!(!state.show_help);
    }

    #[test]
    fn resize_updates_terminal_size() {
        let mut state = AppState::new();
        let effect = state.handle_action(Action::Resize(120, 40));
        assert_eq!(effect, None);
        assert_eq!(state.terminal_size, (120, 40));
    }

    #[test]
    fn back_to_list_from_session_list_quits() {
        let mut state = AppState::new();
        let effect = state.handle_action(Action::BackToList);
        assert_eq!(effect, Some(SideEffect::Exit));
    }

    #[test]
    fn back_to_list_from_conversation_returns_to_session_list() {
        let mut state = AppState::new();
        state.view = View::Conversation(SessionId("test-session".to_string()));
        let effect = state.handle_action(Action::BackToList);
        assert_eq!(effect, None);
        assert_eq!(state.view, View::SessionList);
    }

    #[test]
    fn sessions_loaded_resets_selected_index_after_navigation() {
        let mut state = AppState::new();
        state.handle_action(Action::SessionsLoaded(make_summaries(5)));
        state.handle_action(Action::NavigateDown);
        state.handle_action(Action::NavigateDown);
        assert_eq!(state.selected_index, 2);
        // Loading new sessions should reset index.
        state.handle_action(Action::SessionsLoaded(make_summaries(3)));
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn grouped_sessions_empty_returns_empty() {
        let state = AppState::new();
        let groups = state.grouped_sessions();
        assert!(groups.is_empty());
    }

    #[test]
    fn sessions_sorted_by_last_activity_descending() {
        use crate::data::model::{ProjectPath, TokenUsage};
        use chrono::TimeZone;
        use std::path::PathBuf;

        let mut state = AppState::new();
        let summaries = vec![
            SessionSummary {
                id: SessionId("old".to_string()),
                project: ProjectPath(PathBuf::from("proj-a")),
                file_path: PathBuf::from("/tmp/old.jsonl"),
                file_size: 100,
                last_prompt: None,
                started_at: None,
                last_activity: Some(chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
                turn_count: 1,
                total_tokens: TokenUsage::default(),
                git_branch: None,
            },
            SessionSummary {
                id: SessionId("new".to_string()),
                project: ProjectPath(PathBuf::from("proj-a")),
                file_path: PathBuf::from("/tmp/new.jsonl"),
                file_size: 100,
                last_prompt: None,
                started_at: None,
                last_activity: Some(chrono::Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap()),
                turn_count: 1,
                total_tokens: TokenUsage::default(),
                git_branch: None,
            },
        ];
        state.handle_action(Action::SessionsLoaded(summaries));
        // Most recent should be first.
        assert_eq!(state.sessions[0].id, SessionId("new".to_string()));
        assert_eq!(state.sessions[1].id, SessionId("old".to_string()));
    }

    #[test]
    fn session_loaded_stores_session_and_transitions_to_conversation() {
        let mut state = AppState::new();
        state.handle_action(Action::SessionsLoaded(make_summaries(3)));
        let session = make_session("sess-1", 3);
        let effect = state.handle_action(Action::SessionLoaded(Box::new(session)));
        assert!(effect.is_none());
        assert_eq!(
            state.view,
            View::Conversation(SessionId("sess-1".to_string()))
        );
        assert!(state.current_session.is_some());
        assert_eq!(state.current_turn_index, 0);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn session_loaded_empty_session_sets_empty_state() {
        let mut state = AppState::new();
        let session = make_session("sess-empty", 0);
        let effect = state.handle_action(Action::SessionLoaded(Box::new(session)));
        assert!(effect.is_none());
        assert_eq!(
            state.view,
            View::Conversation(SessionId("sess-empty".to_string()))
        );
        assert_eq!(state.empty_state, Some(EmptyState::EmptySession));
        assert!(state.current_session.is_some());
    }

    #[test]
    fn session_load_error_returns_to_session_list() {
        let mut state = AppState::new();
        state.view = View::Conversation(SessionId("sess-1".to_string()));
        let effect = state.handle_action(Action::SessionLoadError("not found".to_string()));
        assert!(effect.is_none());
        assert_eq!(state.view, View::SessionList);
    }

    #[test]
    fn next_turn_increments_index() {
        let mut state = AppState::new();
        let session = make_session("sess-1", 5);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        assert_eq!(state.current_turn_index, 0);
        state.handle_action(Action::NextTurn);
        assert_eq!(state.current_turn_index, 1);
        state.handle_action(Action::NextTurn);
        assert_eq!(state.current_turn_index, 2);
    }

    #[test]
    fn next_turn_stops_at_last() {
        let mut state = AppState::new();
        let session = make_session("sess-1", 2);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        state.handle_action(Action::NextTurn);
        assert_eq!(state.current_turn_index, 1);
        state.handle_action(Action::NextTurn);
        assert_eq!(state.current_turn_index, 1); // stays at last
    }

    #[test]
    fn prev_turn_decrements_index() {
        let mut state = AppState::new();
        let session = make_session("sess-1", 5);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        state.handle_action(Action::NextTurn);
        state.handle_action(Action::NextTurn);
        assert_eq!(state.current_turn_index, 2);
        state.handle_action(Action::PrevTurn);
        assert_eq!(state.current_turn_index, 1);
    }

    #[test]
    fn prev_turn_stops_at_first() {
        let mut state = AppState::new();
        let session = make_session("sess-1", 3);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        state.handle_action(Action::PrevTurn);
        assert_eq!(state.current_turn_index, 0);
    }

    #[test]
    fn next_turn_resets_scroll_offset() {
        let mut state = AppState::new();
        let session = make_session("sess-1", 3);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        state.scroll_offset = 5;
        state.handle_action(Action::NextTurn);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn navigate_down_in_conversation_scrolls() {
        let mut state = AppState::new();
        let session = make_session("sess-1", 3);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        assert_eq!(state.scroll_offset, 0);
        state.handle_action(Action::NavigateDown);
        assert_eq!(state.scroll_offset, 1);
        state.handle_action(Action::NavigateDown);
        assert_eq!(state.scroll_offset, 2);
    }

    #[test]
    fn navigate_up_in_conversation_scrolls() {
        let mut state = AppState::new();
        let session = make_session("sess-1", 3);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        state.scroll_offset = 5;
        state.handle_action(Action::NavigateUp);
        assert_eq!(state.scroll_offset, 4);
    }

    #[test]
    fn navigate_up_in_conversation_stops_at_zero() {
        let mut state = AppState::new();
        let session = make_session("sess-1", 3);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        state.handle_action(Action::NavigateUp);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn back_to_list_from_conversation_clears_session() {
        let mut state = AppState::new();
        let session = make_session("sess-1", 3);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        state.current_turn_index = 2;
        state.scroll_offset = 5;
        state.handle_action(Action::BackToList);
        assert_eq!(state.view, View::SessionList);
        assert!(state.current_session.is_none());
        assert_eq!(state.current_turn_index, 0);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn next_turn_without_session_does_nothing() {
        let mut state = AppState::new();
        state.handle_action(Action::NextTurn);
        assert_eq!(state.current_turn_index, 0);
    }

    #[test]
    fn prev_turn_without_session_does_nothing() {
        let mut state = AppState::new();
        state.handle_action(Action::PrevTurn);
        assert_eq!(state.current_turn_index, 0);
    }

    #[test]
    fn prev_turn_resets_scroll_offset() {
        let mut state = AppState::new();
        let session = make_session("sess-1", 3);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        state.handle_action(Action::NextTurn);
        state.scroll_offset = 5;
        state.handle_action(Action::PrevTurn);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn next_turn_on_empty_session_does_nothing() {
        let mut state = AppState::new();
        let session = make_session("sess-empty", 0);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        assert_eq!(state.current_turn_index, 0);
        state.handle_action(Action::NextTurn);
        assert_eq!(state.current_turn_index, 0);
    }

    #[test]
    fn select_session_clears_conversation_state() {
        let mut state = AppState::new();
        state.handle_action(Action::SessionsLoaded(make_summaries(3)));
        // Load a session first to set conversation state.
        let session = make_session("sess-1", 5);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        state.current_turn_index = 3;
        state.scroll_offset = 10;
        // Now select a different session.
        state.view = View::SessionList;
        state.handle_action(Action::SelectSession);
        assert!(state.current_session.is_none());
        assert_eq!(state.current_turn_index, 0);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn back_to_list_recomputes_empty_state() {
        let mut state = AppState::new();
        // Start with no sessions loaded.
        state.handle_action(Action::SessionsLoaded(Vec::new()));
        assert_eq!(state.empty_state, Some(EmptyState::NoSessions));
        // Simulate entering a conversation (even though sessions are empty,
        // test the logic path).
        state.view = View::Conversation(SessionId("test".to_string()));
        state.handle_action(Action::BackToList);
        // Should recompute: sessions is empty, so NoSessions.
        assert_eq!(state.empty_state, Some(EmptyState::NoSessions));
    }

    #[test]
    fn back_to_list_recomputes_empty_state_with_sessions() {
        let mut state = AppState::new();
        state.handle_action(Action::SessionsLoaded(make_summaries(3)));
        state.view = View::Conversation(SessionId("test".to_string()));
        state.handle_action(Action::BackToList);
        // Sessions exist, so empty_state should be None.
        assert_eq!(state.empty_state, None);
    }

    #[test]
    fn session_load_error_stores_last_error() {
        let mut state = AppState::new();
        state.view = View::Conversation(SessionId("sess-1".to_string()));
        state.handle_action(Action::SessionLoadError("file not found".to_string()));
        assert_eq!(state.last_error, Some("file not found".to_string()));
    }

    #[test]
    fn session_loaded_clears_last_error() {
        let mut state = AppState::new();
        state.last_error = Some("previous error".to_string());
        let session = make_session("sess-1", 3);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        assert!(state.last_error.is_none());
    }

    #[test]
    fn show_tokens_defaults_to_true() {
        let state = AppState::new();
        assert!(state.show_tokens);
    }

    #[test]
    fn toggle_tokens_flips_show_tokens() {
        let mut state = AppState::new();
        assert!(state.show_tokens);
        let effect = state.handle_action(Action::ToggleTokens);
        assert_eq!(effect, None);
        assert!(!state.show_tokens);
        let effect = state.handle_action(Action::ToggleTokens);
        assert_eq!(effect, None);
        assert!(state.show_tokens);
    }

    #[test]
    fn select_session_sets_loading_state() {
        let mut state = AppState::new();
        state.handle_action(Action::SessionsLoaded(make_summaries(3)));
        state.handle_action(Action::SelectSession);
        // After SelectSession, the view should transition to Conversation
        // with a Loading empty state while the session loads.
        assert_eq!(
            state.view,
            View::Conversation(SessionId("session-0".to_string()))
        );
        assert_eq!(state.empty_state, Some(EmptyState::Loading));
    }

    #[test]
    fn grouped_sessions_returns_project_groups() {
        use crate::data::model::{ProjectPath, TokenUsage};
        use std::path::PathBuf;

        let mut state = AppState::new();
        let summaries = vec![
            SessionSummary {
                id: SessionId("s1".to_string()),
                project: ProjectPath(PathBuf::from("proj-a")),
                file_path: PathBuf::from("/tmp/s1.jsonl"),
                file_size: 100,
                last_prompt: None,
                started_at: None,
                last_activity: None,
                turn_count: 1,
                total_tokens: TokenUsage::default(),
                git_branch: None,
            },
            SessionSummary {
                id: SessionId("s2".to_string()),
                project: ProjectPath(PathBuf::from("proj-b")),
                file_path: PathBuf::from("/tmp/s2.jsonl"),
                file_size: 100,
                last_prompt: None,
                started_at: None,
                last_activity: None,
                turn_count: 1,
                total_tokens: TokenUsage::default(),
                git_branch: None,
            },
            SessionSummary {
                id: SessionId("s3".to_string()),
                project: ProjectPath(PathBuf::from("proj-a")),
                file_path: PathBuf::from("/tmp/s3.jsonl"),
                file_size: 100,
                last_prompt: None,
                started_at: None,
                last_activity: None,
                turn_count: 1,
                total_tokens: TokenUsage::default(),
                git_branch: None,
            },
        ];
        state.handle_action(Action::SessionsLoaded(summaries));
        let groups = state.grouped_sessions();
        assert_eq!(groups.len(), 2);
        // proj-a has 2 sessions, proj-b has 1
        let proj_a = groups
            .iter()
            .find(|(p, _)| p.0.as_os_str() == "proj-a")
            .unwrap();
        assert_eq!(proj_a.1.len(), 2);
    }

    #[test]
    fn version_loaded_stores_version() {
        let mut state = AppState::new();
        assert!(state.title_bar.claude_version.is_none());
        let effect = state.handle_action(Action::VersionLoaded("2.1.71".to_string()));
        assert_eq!(effect, None);
        assert_eq!(state.title_bar.claude_version, Some("2.1.71".to_string()));
    }

    #[test]
    fn usage_loaded_stores_usage() {
        use crate::data::usage::{UsageData, UsageWindow};

        let mut state = AppState::new();
        assert!(state.title_bar.usage.is_none());
        let usage = UsageData {
            five_hour: Some(UsageWindow {
                utilization: 42.0,
                resets_at: None,
            }),
            seven_day: Some(UsageWindow {
                utilization: 65.0,
                resets_at: None,
            }),
            seven_day_opus: None,
        };
        let effect = state.handle_action(Action::UsageLoaded(usage));
        assert_eq!(effect, None);
        let stored = state.title_bar.usage.as_ref().unwrap();
        assert!((stored.five_hour.as_ref().unwrap().utilization - 42.0).abs() < f64::EPSILON);
    }
}
