// Application state machine -- pure logic, no TUI dependencies.

use std::path::PathBuf;

use crate::data::model::{
    ContentBlock, ProjectPath, ProjectSummary, Session, SessionId, SessionSummary, Turn,
    UserContent,
};
use crate::data::usage::{TitleBarInfo, UsageData};

/// The view the TUI should render.
#[derive(Debug, PartialEq, Eq)]
pub enum View {
    ProjectList,
    SessionList,
    Conversation(SessionId),
}

/// Empty state conditions for display.
#[derive(Debug, PartialEq, Eq)]
pub enum EmptyState {
    /// No ~/.claude/ directory found.
    NoDirectory,
    /// No projects with sessions found.
    NoProjects,
    /// Directory exists but no sessions found.
    NoSessions,
    /// Loading projects at startup.
    LoadingProjects,
    /// Loading sessions for a selected project.
    LoadingSessions,
    /// Loading a single session.
    LoadingSession,
    /// Selected session has 0 turns.
    EmptySession,
}

/// Display options for the conversation view.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DisplayOptions {
    pub show_tokens: bool,
    pub show_tools: bool,
    pub show_thinking: bool,
}

impl DisplayOptions {
    /// Whether any detail flag is enabled (used to decide header/label visibility).
    pub fn any_detail_enabled(&self) -> bool {
        self.show_tokens || self.show_tools || self.show_thinking
    }

    /// Whether a turn produces visible output with current display settings.
    pub fn is_turn_visible(&self, turn: &Turn) -> bool {
        // User content visible?
        let user_visible = match &turn.user_message.content {
            UserContent::Text(t) => !t.is_empty(),
            UserContent::ToolResults(_) => self.show_tools,
            UserContent::Mixed { text, .. } => !text.is_empty() || self.show_tools,
        };
        if user_visible {
            return true;
        }

        // Assistant content visible?
        if let Some(ref response) = turn.assistant_response {
            for block in &response.content_blocks {
                match block {
                    ContentBlock::Text(t) if !t.is_empty() => return true,
                    ContentBlock::Thinking { .. } if self.show_thinking => return true,
                    ContentBlock::ToolUse(_) if self.show_tools => return true,
                    _ => {}
                }
            }
        }

        false
    }
}

/// Which modal overlay is showing.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ModalContent {
    User,
    Claude,
}

/// All possible user/system actions.
#[derive(Debug)]
pub enum Action {
    Quit,
    NavigateUp,
    NavigateDown,
    SelectProject,
    SelectSession,
    BackToList,
    NextTurn,
    PrevTurn,
    ShowUserModal,
    ShowClaudeModal,
    DismissModal,
    ModalScrollDown,
    ModalScrollUp,
    Resize(u16, u16),
    ProjectsLoaded(Vec<ProjectSummary>),
    SessionsLoaded(Vec<SessionSummary>),
    SessionLoaded(Box<Session>),
    SessionLoadError(String),
    LoadError(String),
    ToggleHelp,
    ToggleTokens,
    ToggleTools,
    ToggleThinking,
    VersionLoaded(String),
    UsageLoaded(UsageData),
}

/// Side effects that the caller must execute.
#[derive(Debug, PartialEq, Eq)]
pub enum SideEffect {
    Exit,
    LoadSessionList,
    LoadSession(SessionId),
    LoadProjectSessions(ProjectPath),
}

/// Application state.
pub struct AppState {
    pub view: View,
    pub empty_state: Option<EmptyState>,
    pub projects: Vec<ProjectSummary>,
    pub project_selected_index: usize,
    pub selected_project: Option<ProjectPath>,
    pub cwd: Option<PathBuf>,
    pub sessions: Vec<SessionSummary>,
    pub selected_index: usize,
    pub show_help: bool,
    pub display: DisplayOptions,
    pub terminal_size: (u16, u16),
    pub current_session: Option<Session>,
    pub current_turn_index: usize,
    pub scroll_offset: usize,
    pub last_error: Option<String>,
    pub title_bar: TitleBarInfo,
    pub modal: Option<ModalContent>,
    pub modal_scroll: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            view: View::ProjectList,
            empty_state: Some(EmptyState::LoadingProjects),
            projects: Vec::new(),
            project_selected_index: 0,
            selected_project: None,
            cwd: None,
            sessions: Vec::new(),
            selected_index: 0,
            show_help: false,
            display: DisplayOptions::default(),
            terminal_size: (80, 24),
            current_session: None,
            current_turn_index: 0,
            scroll_offset: 0,
            last_error: None,
            title_bar: TitleBarInfo::new(),
            modal: None,
            modal_scroll: 0,
        }
    }

    pub fn with_cwd(mut self, cwd: Option<PathBuf>) -> Self {
        self.cwd = cwd;
        self
    }

    /// Pure state machine: process action, return optional side effect.
    pub fn handle_action(&mut self, action: Action) -> Option<SideEffect> {
        match action {
            Action::Quit => Some(SideEffect::Exit),

            Action::ProjectsLoaded(mut projects) => {
                // Sort: CWD match first, then by last_activity descending.
                if let Some(ref cwd) = self.cwd {
                    projects.sort_by(|a, b| {
                        let a_match = a.path.matches_cwd(cwd);
                        let b_match = b.path.matches_cwd(cwd);
                        match (a_match, b_match) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => b.last_activity.cmp(&a.last_activity),
                        }
                    });
                } else {
                    projects.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
                }

                if projects.is_empty() {
                    self.empty_state = Some(EmptyState::NoProjects);
                } else {
                    self.empty_state = None;
                }
                self.projects = projects;
                self.project_selected_index = 0;
                None
            }

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

            Action::LoadError(msg) => {
                self.empty_state = Some(EmptyState::NoDirectory);
                self.last_error = Some(msg);
                None
            }

            Action::NavigateDown => match &self.view {
                View::ProjectList => {
                    if !self.projects.is_empty()
                        && self.project_selected_index < self.projects.len() - 1
                    {
                        self.project_selected_index += 1;
                    }
                    None
                }
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
                View::ProjectList => {
                    if !self.projects.is_empty() && self.project_selected_index > 0 {
                        self.project_selected_index -= 1;
                    }
                    None
                }
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

            Action::SelectProject => {
                if self.projects.is_empty() {
                    return None;
                }
                let project = &self.projects[self.project_selected_index];
                let path = project.path.clone();
                self.selected_project = Some(path.clone());
                self.view = View::SessionList;
                self.empty_state = Some(EmptyState::LoadingSessions);
                self.sessions.clear();
                self.selected_index = 0;
                Some(SideEffect::LoadProjectSessions(path))
            }

            Action::SelectSession => {
                if self.sessions.is_empty() {
                    return None;
                }
                let id = self.sessions[self.selected_index].id.clone();
                self.view = View::Conversation(id.clone());
                self.empty_state = Some(EmptyState::LoadingSession);
                self.current_session = None;
                self.current_turn_index = 0;
                self.scroll_offset = 0;
                Some(SideEffect::LoadSession(id))
            }

            Action::BackToList => match &self.view {
                View::ProjectList => Some(SideEffect::Exit),
                View::SessionList => {
                    self.view = View::ProjectList;
                    self.sessions.clear();
                    self.selected_index = 0;
                    self.selected_project = None;
                    self.empty_state = if self.projects.is_empty() {
                        Some(EmptyState::NoProjects)
                    } else {
                        None
                    };
                    None
                }
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
                self.display.show_tokens = !self.display.show_tokens;
                None
            }

            Action::ToggleTools => {
                self.display.show_tools = !self.display.show_tools;
                None
            }

            Action::ToggleThinking => {
                self.display.show_thinking = !self.display.show_thinking;
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
                if let Some(ref session) = self.current_session {
                    let len = session.turns.len();
                    let mut next = self.current_turn_index + 1;
                    while next < len && !self.display.is_turn_visible(&session.turns[next]) {
                        next += 1;
                    }
                    if next < len {
                        self.current_turn_index = next;
                        self.scroll_offset = 0;
                    }
                }
                None
            }

            Action::PrevTurn => {
                if let Some(ref session) = self.current_session {
                    let mut prev = self.current_turn_index.saturating_sub(1);
                    while prev > 0 && !self.display.is_turn_visible(&session.turns[prev]) {
                        prev -= 1;
                    }
                    if prev < self.current_turn_index
                        && self.display.is_turn_visible(&session.turns[prev])
                    {
                        self.current_turn_index = prev;
                        self.scroll_offset = 0;
                    }
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

            Action::ShowUserModal => {
                self.modal = Some(ModalContent::User);
                self.modal_scroll = 0;
                None
            }

            Action::ShowClaudeModal => {
                self.modal = Some(ModalContent::Claude);
                self.modal_scroll = 0;
                None
            }

            Action::DismissModal => {
                self.modal = None;
                self.modal_scroll = 0;
                None
            }

            Action::ModalScrollDown => {
                self.modal_scroll = self.modal_scroll.saturating_add(1);
                None
            }

            Action::ModalScrollUp => {
                self.modal_scroll = self.modal_scroll.saturating_sub(1);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_app_state_starts_in_project_list_view() {
        let state = AppState::new();
        assert!(matches!(state.view, View::ProjectList));
    }

    #[test]
    fn new_app_state_empty_state_is_loading() {
        let state = AppState::new();
        assert_eq!(state.empty_state, Some(EmptyState::LoadingProjects));
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

    fn make_project_summaries(count: usize) -> Vec<ProjectSummary> {
        use chrono::TimeZone;

        (0..count)
            .map(|i| ProjectSummary {
                path: ProjectPath(PathBuf::from(format!("-Users-test-project-{i}"))),
                display_name: format!("project-{i}"),
                session_count: i + 1,
                last_activity: Some(
                    chrono::Utc
                        .with_ymd_and_hms(2026, 1, 1 + i as u32, 0, 0, 0)
                        .unwrap(),
                ),
            })
            .collect()
    }

    #[test]
    fn sessions_loaded_clears_loading_state() {
        let mut state = AppState::new();
        state.view = View::SessionList;
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
        state.view = View::SessionList;
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
        state.view = View::SessionList;
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
        state.view = View::SessionList;
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
        state.view = View::SessionList;
        state.handle_action(Action::SessionsLoaded(make_summaries(1)));
        let effect = state.handle_action(Action::NavigateDown);
        assert_eq!(effect, None);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn navigate_up_decrements_index() {
        let mut state = AppState::new();
        state.view = View::SessionList;
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
        state.view = View::SessionList;
        state.handle_action(Action::SessionsLoaded(make_summaries(3)));
        let effect = state.handle_action(Action::NavigateUp);
        assert_eq!(effect, None);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn navigate_on_empty_session_list_does_nothing() {
        let mut state = AppState::new();
        state.view = View::SessionList;
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
        state.view = View::SessionList;
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
        state.view = View::SessionList;
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
    fn back_to_list_from_project_list_quits() {
        let mut state = AppState::new();
        let effect = state.handle_action(Action::BackToList);
        assert_eq!(effect, Some(SideEffect::Exit));
    }

    #[test]
    fn back_to_list_from_session_list_goes_to_project_list() {
        let mut state = AppState::new();
        state.view = View::SessionList;
        state.handle_action(Action::ProjectsLoaded(make_project_summaries(3)));
        state.view = View::SessionList;
        let effect = state.handle_action(Action::BackToList);
        assert_eq!(effect, None);
        assert_eq!(state.view, View::ProjectList);
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
        state.view = View::SessionList;
        state.handle_action(Action::SessionsLoaded(make_summaries(5)));
        state.handle_action(Action::NavigateDown);
        state.handle_action(Action::NavigateDown);
        assert_eq!(state.selected_index, 2);
        // Loading new sessions should reset index.
        state.handle_action(Action::SessionsLoaded(make_summaries(3)));
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn sessions_sorted_by_last_activity_descending() {
        use crate::data::model::{ProjectPath, TokenUsage};
        use chrono::TimeZone;
        use std::path::PathBuf;

        let mut state = AppState::new();
        state.view = View::SessionList;
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
        state.view = View::SessionList;
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
        state.view = View::SessionList;
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
        state.view = View::SessionList;
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
        state.view = View::SessionList;
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
    fn display_options_default_values() {
        let opts = DisplayOptions::default();
        assert!(!opts.show_tokens);
        assert!(!opts.show_tools);
        assert!(!opts.show_thinking);
    }

    #[test]
    fn show_tokens_defaults_to_false() {
        let state = AppState::new();
        assert!(!state.display.show_tokens);
    }

    #[test]
    fn toggle_tokens_flips_show_tokens() {
        let mut state = AppState::new();
        assert!(!state.display.show_tokens);
        let effect = state.handle_action(Action::ToggleTokens);
        assert_eq!(effect, None);
        assert!(state.display.show_tokens);
        let effect = state.handle_action(Action::ToggleTokens);
        assert_eq!(effect, None);
        assert!(!state.display.show_tokens);
    }

    #[test]
    fn toggle_tools_flips_display_show_tools() {
        let mut state = AppState::new();
        assert!(!state.display.show_tools);
        state.handle_action(Action::ToggleTools);
        assert!(state.display.show_tools);
        state.handle_action(Action::ToggleTools);
        assert!(!state.display.show_tools);
    }

    #[test]
    fn toggle_thinking_flips_display_show_thinking() {
        let mut state = AppState::new();
        assert!(!state.display.show_thinking);
        state.handle_action(Action::ToggleThinking);
        assert!(state.display.show_thinking);
        state.handle_action(Action::ToggleThinking);
        assert!(!state.display.show_thinking);
    }

    #[test]
    fn toggle_tools_is_independent_of_toggle_tokens() {
        let mut state = AppState::new();
        state.handle_action(Action::ToggleTools);
        assert!(state.display.show_tools);
        assert!(!state.display.show_tokens);
        state.handle_action(Action::ToggleTokens);
        assert!(state.display.show_tools);
        assert!(state.display.show_tokens);
    }

    // --- ProjectList tests ---

    #[test]
    fn projects_loaded_with_cwd_match_sorts_cwd_first() {
        let mut state = AppState::new();
        state.cwd = Some(PathBuf::from("/Users/test/project-0"));
        let projects = make_project_summaries(3);
        state.handle_action(Action::ProjectsLoaded(projects));
        // project-0 matches CWD, should be first.
        assert_eq!(state.projects[0].display_name, "project-0");
    }

    #[test]
    fn projects_loaded_no_cwd_sorts_by_last_activity_desc() {
        let mut state = AppState::new();
        let projects = make_project_summaries(3);
        state.handle_action(Action::ProjectsLoaded(projects));
        // Most recent (project-2, Jan 3) should be first.
        assert_eq!(state.projects[0].display_name, "project-2");
    }

    #[test]
    fn projects_loaded_empty_sets_no_projects() {
        let mut state = AppState::new();
        state.handle_action(Action::ProjectsLoaded(Vec::new()));
        assert_eq!(state.empty_state, Some(EmptyState::NoProjects));
    }

    #[test]
    fn select_project_returns_load_project_sessions() {
        let mut state = AppState::new();
        state.handle_action(Action::ProjectsLoaded(make_project_summaries(3)));
        let expected_path = state.projects[0].path.clone();
        let effect = state.handle_action(Action::SelectProject);
        assert_eq!(
            effect,
            Some(SideEffect::LoadProjectSessions(expected_path.clone()))
        );
        assert_eq!(state.view, View::SessionList);
        assert_eq!(state.empty_state, Some(EmptyState::LoadingSessions));
        assert_eq!(state.selected_project, Some(expected_path));
    }

    #[test]
    fn select_project_on_empty_returns_none() {
        let mut state = AppState::new();
        state.handle_action(Action::ProjectsLoaded(Vec::new()));
        let effect = state.handle_action(Action::SelectProject);
        assert!(effect.is_none());
    }

    #[test]
    fn back_from_session_list_preserves_project_selection_index() {
        let mut state = AppState::new();
        state.handle_action(Action::ProjectsLoaded(make_project_summaries(3)));
        state.project_selected_index = 1;
        state.handle_action(Action::SelectProject);
        assert!(state.selected_project.is_some());
        // Now back
        state.handle_action(Action::BackToList);
        assert_eq!(state.view, View::ProjectList);
        assert_eq!(state.project_selected_index, 1);
        assert!(!state.projects.is_empty());
        assert_eq!(state.selected_project, None);
    }

    #[test]
    fn navigate_down_in_project_list() {
        let mut state = AppState::new();
        state.handle_action(Action::ProjectsLoaded(make_project_summaries(3)));
        assert_eq!(state.project_selected_index, 0);
        state.handle_action(Action::NavigateDown);
        assert_eq!(state.project_selected_index, 1);
        state.handle_action(Action::NavigateDown);
        assert_eq!(state.project_selected_index, 2);
        // Stops at bottom.
        state.handle_action(Action::NavigateDown);
        assert_eq!(state.project_selected_index, 2);
    }

    #[test]
    fn navigate_up_in_project_list() {
        let mut state = AppState::new();
        state.handle_action(Action::ProjectsLoaded(make_project_summaries(3)));
        state.project_selected_index = 2;
        state.handle_action(Action::NavigateUp);
        assert_eq!(state.project_selected_index, 1);
        state.handle_action(Action::NavigateUp);
        assert_eq!(state.project_selected_index, 0);
        // Stops at top.
        state.handle_action(Action::NavigateUp);
        assert_eq!(state.project_selected_index, 0);
    }

    #[test]
    fn navigate_on_empty_project_list_does_nothing() {
        let mut state = AppState::new();
        state.handle_action(Action::ProjectsLoaded(Vec::new()));
        state.handle_action(Action::NavigateDown);
        assert_eq!(state.project_selected_index, 0);
        state.handle_action(Action::NavigateUp);
        assert_eq!(state.project_selected_index, 0);
    }

    #[test]
    fn round_trip_project_session_conversation_back() {
        let mut state = AppState::new();
        state.handle_action(Action::ProjectsLoaded(make_project_summaries(2)));
        assert_eq!(state.view, View::ProjectList);

        // Select project
        state.handle_action(Action::SelectProject);
        assert_eq!(state.view, View::SessionList);

        // Simulate sessions loaded
        state.handle_action(Action::SessionsLoaded(make_summaries(2)));

        // Select session
        state.handle_action(Action::SelectSession);
        assert!(matches!(state.view, View::Conversation(_)));

        // Back to session list
        state.handle_action(Action::BackToList);
        assert_eq!(state.view, View::SessionList);

        // Back to project list
        state.handle_action(Action::BackToList);
        assert_eq!(state.view, View::ProjectList);

        // Back from project list exits
        let effect = state.handle_action(Action::BackToList);
        assert_eq!(effect, Some(SideEffect::Exit));
    }

    #[test]
    fn select_session_sets_loading_state() {
        let mut state = AppState::new();
        state.view = View::SessionList;
        state.handle_action(Action::SessionsLoaded(make_summaries(3)));
        state.handle_action(Action::SelectSession);
        assert_eq!(
            state.view,
            View::Conversation(SessionId("session-0".to_string()))
        );
        assert_eq!(state.empty_state, Some(EmptyState::LoadingSession));
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
    fn new_app_state_modal_is_none() {
        let state = AppState::new();
        assert_eq!(state.modal, None);
        assert_eq!(state.modal_scroll, 0);
    }

    #[test]
    fn show_user_modal_sets_modal_state() {
        let mut state = AppState::new();
        let session = make_session("sess-1", 3);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        let effect = state.handle_action(Action::ShowUserModal);
        assert_eq!(effect, None);
        assert_eq!(state.modal, Some(ModalContent::User));
        assert_eq!(state.modal_scroll, 0);
    }

    #[test]
    fn show_claude_modal_sets_modal_state() {
        let mut state = AppState::new();
        let session = make_session("sess-1", 3);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        let effect = state.handle_action(Action::ShowClaudeModal);
        assert_eq!(effect, None);
        assert_eq!(state.modal, Some(ModalContent::Claude));
        assert_eq!(state.modal_scroll, 0);
    }

    #[test]
    fn dismiss_modal_clears_modal_state() {
        let mut state = AppState::new();
        state.modal = Some(ModalContent::User);
        state.modal_scroll = 5;
        let effect = state.handle_action(Action::DismissModal);
        assert_eq!(effect, None);
        assert_eq!(state.modal, None);
        assert_eq!(state.modal_scroll, 0);
    }

    #[test]
    fn modal_scroll_down_increments() {
        let mut state = AppState::new();
        state.modal = Some(ModalContent::User);
        let effect = state.handle_action(Action::ModalScrollDown);
        assert_eq!(effect, None);
        assert_eq!(state.modal_scroll, 1);
        state.handle_action(Action::ModalScrollDown);
        assert_eq!(state.modal_scroll, 2);
    }

    #[test]
    fn modal_scroll_up_decrements_saturating() {
        let mut state = AppState::new();
        state.modal = Some(ModalContent::User);
        state.modal_scroll = 3;
        let effect = state.handle_action(Action::ModalScrollUp);
        assert_eq!(effect, None);
        assert_eq!(state.modal_scroll, 2);
    }

    #[test]
    fn modal_scroll_up_stops_at_zero() {
        let mut state = AppState::new();
        state.modal = Some(ModalContent::User);
        state.modal_scroll = 0;
        state.handle_action(Action::ModalScrollUp);
        assert_eq!(state.modal_scroll, 0);
    }

    #[test]
    fn show_user_modal_resets_scroll() {
        let mut state = AppState::new();
        let session = make_session("sess-1", 3);
        state.handle_action(Action::SessionLoaded(Box::new(session)));
        state.modal_scroll = 10;
        state.handle_action(Action::ShowUserModal);
        assert_eq!(state.modal_scroll, 0);
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
