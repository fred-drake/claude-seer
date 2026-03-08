// Status bar widget.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{AppState, View};
use crate::data::model::format_tokens;

/// Build the status bar text from application state.
fn build_status_text(state: &AppState) -> String {
    match &state.view {
        View::SessionList => {
            let session_count = state.sessions.len();
            if let Some(ref err) = state.last_error {
                format!(" {} session(s) | Error: {} | ? help", session_count, err)
            } else {
                format!(" {} session(s) | Press ? for help", session_count)
            }
        }
        View::Conversation(_) => {
            if let Some(ref session) = state.current_session {
                let total = session.turns.len();
                if total == 0 {
                    " Empty session | Esc: back | ? help".to_string()
                } else {
                    let token_total = session.token_totals.total();
                    let token_part = if state.show_tokens && token_total > 0 {
                        format!(" | {} tokens", format_tokens(token_total))
                    } else {
                        String::new()
                    };
                    let token_hint = if token_total > 0 { " | t: tokens" } else { "" };
                    format!(
                        " Turn {}/{}{} | n/N: jump | j/k: scroll{} | Esc: back | ? help",
                        state.current_turn_index + 1,
                        total,
                        token_part,
                        token_hint,
                    )
                }
            } else {
                " Loading session... | Esc: back | ? help".to_string()
            }
        }
    }
}

/// Render the status bar into the given area.
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let status_text = build_status_text(state);

    let line = Line::from(vec![Span::styled(
        status_text,
        Style::default().fg(Color::White).bg(Color::DarkGray),
    )]);

    // Pad to fill width.
    let line_width = u16::try_from(line.width()).unwrap_or(area.width);
    let padding = " ".repeat(area.width.saturating_sub(line_width) as usize);
    let padded = Line::from(vec![
        line.spans.into_iter().next().unwrap_or_default(),
        Span::styled(padding, Style::default().bg(Color::DarkGray)),
    ]);

    let bar = Paragraph::new(padded);
    frame.render_widget(bar, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::data::model::{
        MessageId, ProjectPath, Session, SessionId, TokenUsage, Turn, UserContent, UserMessage,
    };
    use std::path::PathBuf;

    fn make_session_with_tokens(turn_count: usize, token_totals: TokenUsage) -> Session {
        let turns: Vec<Turn> = (0..turn_count)
            .map(|i| Turn {
                index: i,
                user_message: UserMessage {
                    id: MessageId(format!("msg-{i}")),
                    timestamp: chrono::Utc::now(),
                    content: UserContent::Text(format!("msg {i}")),
                },
                assistant_response: None,
                duration: None,
                is_complete: true,
                events: Vec::new(),
            })
            .collect();

        Session {
            id: SessionId("test".to_string()),
            project: ProjectPath(PathBuf::from("test-project")),
            file_path: PathBuf::from("/tmp/test.jsonl"),
            version: None,
            git_branch: None,
            started_at: None,
            last_activity: None,
            last_prompt: None,
            turns,
            token_totals,
            parse_warnings: Vec::new(),
        }
    }

    #[test]
    fn status_bar_shows_token_total_in_conversation() {
        let mut state = AppState::new();
        let session = make_session_with_tokens(
            3,
            TokenUsage {
                input_tokens: 10_000,
                output_tokens: 5_000,
                cache_creation_tokens: 0,
                cache_read_tokens: 2_000,
            },
        );
        state.current_session = Some(session);
        state.view = View::Conversation(SessionId("test".to_string()));
        state.show_tokens = true;

        let text = build_status_text(&state);
        assert!(
            text.contains("17k tokens"),
            "Expected token total in status, got: {text}"
        );
    }

    #[test]
    fn status_bar_hides_tokens_when_show_tokens_false() {
        let mut state = AppState::new();
        let session = make_session_with_tokens(
            3,
            TokenUsage {
                input_tokens: 10_000,
                output_tokens: 5_000,
                cache_creation_tokens: 0,
                cache_read_tokens: 2_000,
            },
        );
        state.current_session = Some(session);
        state.view = View::Conversation(SessionId("test".to_string()));
        state.show_tokens = false;

        let text = build_status_text(&state);
        assert!(
            !text.contains("17k tokens"),
            "Should hide token count, got: {text}"
        );
    }

    #[test]
    fn status_bar_session_list_unchanged() {
        let state = AppState::new();
        let text = build_status_text(&state);
        assert!(text.contains("session(s)"));
    }

    #[test]
    fn status_bar_hides_tokens_for_zero_token_session() {
        let mut state = AppState::new();
        let session = make_session_with_tokens(2, TokenUsage::default());
        state.current_session = Some(session);
        state.view = View::Conversation(SessionId("test".to_string()));
        state.show_tokens = true;

        let text = build_status_text(&state);
        assert!(
            !text.contains("tokens"),
            "Should not show 'tokens' for zero-token session, got: {text}"
        );
    }

    #[test]
    fn status_bar_shows_t_keybinding_hint() {
        let mut state = AppState::new();
        let session = make_session_with_tokens(
            2,
            TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
            },
        );
        state.current_session = Some(session);
        state.view = View::Conversation(SessionId("test".to_string()));

        let text = build_status_text(&state);
        assert!(
            text.contains("t: tokens"),
            "Expected t keybinding hint, got: {text}"
        );
    }
}
