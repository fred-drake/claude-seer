// Application state machine -- pure logic, no TUI dependencies.

use crate::data::model::{ProjectPath, SessionId, SessionSummary};

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
    Resize(u16, u16),
    SessionsLoaded(Vec<SessionSummary>),
    LoadError(String),
    ToggleHelp,
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
    pub terminal_size: (u16, u16),
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
            terminal_size: (80, 24),
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

            Action::NavigateDown => {
                if !self.sessions.is_empty() && self.selected_index < self.sessions.len() - 1 {
                    self.selected_index += 1;
                }
                None
            }

            Action::NavigateUp => {
                if !self.sessions.is_empty() && self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                None
            }

            Action::SelectSession => {
                if self.sessions.is_empty() {
                    return None;
                }
                let id = self.sessions[self.selected_index].id.clone();
                // TODO(M3): Transition to View::Conversation(id) when session
                // data arrives via a SessionLoaded event. Don't transition here
                // because the conversation data hasn't been loaded yet — we'd
                // show an empty view. The LoadSession side effect is currently
                // a no-op stub in main.rs.
                Some(SideEffect::LoadSession(id))
            }

            Action::BackToList => match &self.view {
                View::SessionList => Some(SideEffect::Exit),
                View::Conversation(_) => {
                    self.view = View::SessionList;
                    None
                }
            },

            Action::ToggleHelp => {
                self.show_help = !self.show_help;
                None
            }

            Action::Resize(w, h) => {
                self.terminal_size = (w, h);
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
            .find(|(p, _)| p.0 == PathBuf::from("proj-a"))
            .unwrap();
        assert_eq!(proj_a.1.len(), 2);
    }
}
