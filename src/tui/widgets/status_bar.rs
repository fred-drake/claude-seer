// Status bar widget.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{AppState, View};
use crate::data::model::format_tokens;

use super::text_utils::plural;

/// Build the status bar text from application state (unlimited width, for tests).
#[cfg(test)]
fn build_status_text(state: &AppState) -> String {
    build_status_text_for_width(state, usize::MAX)
}

/// Build the status bar text, progressively dropping hints if it exceeds max_width.
#[cfg(test)]
fn build_status_text_for_width(state: &AppState, max_width: usize) -> String {
    build_status_spans_for_width(state, max_width)
        .iter()
        .map(|s| s.content.as_ref())
        .collect()
}

/// Build status bar as styled spans, with progressive hint dropping.
fn build_status_spans_for_width<'a>(state: &AppState, max_width: usize) -> Vec<Span<'a>> {
    let default_style = Style::default().fg(Color::White).bg(Color::DarkGray);
    let warning_style = Style::default().fg(Color::Yellow).bg(Color::DarkGray);

    match &state.view {
        View::ProjectList => {
            let project_count = state.projects.len();
            let project_label = plural(project_count, "project", "projects");
            let core = format!(" {} {project_label}", project_count);
            let hints: &[&str] = &[" | Esc: quit", " | ? help"];

            for drop_count in 0..=hints.len() {
                let keep = hints.len() - drop_count;
                let suffix: String = hints[..keep].iter().copied().collect();
                let candidate = format!("{core}{suffix}");
                if candidate.len() <= max_width {
                    return vec![Span::styled(candidate, default_style)];
                }
            }

            vec![Span::styled(core, default_style)]
        }
        View::SessionList => {
            let session_count = state.sessions.len();
            let session_label = plural(session_count, "session", "sessions");
            let core = if let Some(ref err) = state.last_error {
                format!(" {} {session_label} | Error: {}", session_count, err)
            } else {
                format!(" {} {session_label}", session_count)
            };

            let hints: &[&str] = &[" | Esc: back", " | ? help"];

            for drop_count in 0..=hints.len() {
                let keep = hints.len() - drop_count;
                let suffix: String = hints[..keep].iter().copied().collect();
                let candidate = format!("{core}{suffix}");
                if candidate.len() <= max_width {
                    return vec![Span::styled(candidate, default_style)];
                }
            }

            vec![Span::styled(core, default_style)]
        }
        View::Conversation(_) => {
            if let Some(ref session) = state.current_session {
                let total = session.turns.len();
                if total == 0 {
                    return vec![Span::styled(
                        " Empty session | Esc: back | ? help",
                        default_style,
                    )];
                }

                let token_total = session.token_totals.total();
                let token_part = if state.display.show_tokens && token_total > 0 {
                    format!(" | {} tokens", format_tokens(token_total))
                } else {
                    String::new()
                };
                let warning_count = session.parse_warnings.len();
                let warning_text = if warning_count > 0 {
                    let label = plural(warning_count, "warning", "warnings");
                    Some(format!(" | {warning_count} {label}"))
                } else {
                    None
                };

                let core_prefix = format!(
                    " Turn {}/{}{}",
                    state.current_turn_index + 1,
                    total,
                    token_part,
                );

                // Total width of core (prefix + optional warning).
                let core_len = core_prefix.len() + warning_text.as_ref().map_or(0, |w| w.len());

                // Keybinding hints in priority order (last dropped first).
                let hints: &[&str] = if token_total > 0 {
                    &[
                        " | n/N: jump",
                        " | j/k: scroll",
                        " | o: tools",
                        " | T: thinking",
                        " | t: tokens",
                        " | Esc: back",
                        " | ? help",
                    ]
                } else {
                    &[
                        " | n/N: jump",
                        " | j/k: scroll",
                        " | o: tools",
                        " | T: thinking",
                        " | Esc: back",
                        " | ? help",
                    ]
                };

                // Try including all hints, then progressively drop from the end.
                for drop_count in 0..=hints.len() {
                    let keep = hints.len() - drop_count;
                    let suffix: String = hints[..keep].iter().copied().collect();
                    if core_len + suffix.len() <= max_width {
                        let mut spans = vec![Span::styled(core_prefix.clone(), default_style)];
                        if let Some(ref wt) = warning_text {
                            spans.push(Span::styled(wt.clone(), warning_style));
                        }
                        if !suffix.is_empty() {
                            spans.push(Span::styled(suffix, default_style));
                        }
                        return spans;
                    }
                }

                // Even core alone is too long; return it anyway.
                let mut spans = vec![Span::styled(core_prefix, default_style)];
                if let Some(wt) = warning_text {
                    spans.push(Span::styled(wt, warning_style));
                }
                spans
            } else {
                vec![Span::styled(
                    " Loading session... | Esc: back | ? help",
                    default_style,
                )]
            }
        }
    }
}

/// Render the status bar into the given area.
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let spans = build_status_spans_for_width(state, area.width as usize);

    let line = Line::from(spans);

    // Pad to fill width.
    let line_width = u16::try_from(line.width()).unwrap_or(area.width);
    let padding = " ".repeat(area.width.saturating_sub(line_width) as usize);
    let mut padded_spans = line.spans;
    padded_spans.push(Span::styled(padding, Style::default().bg(Color::DarkGray)));
    let padded = Line::from(padded_spans);

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
        state.display.show_tokens = true;

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
        state.display.show_tokens = false;

        let text = build_status_text(&state);
        assert!(
            !text.contains("17k tokens"),
            "Should hide token count, got: {text}"
        );
    }

    #[test]
    fn status_bar_project_list_shows_project_count() {
        let state = AppState::new();
        let text = build_status_text(&state);
        assert!(text.contains("projects"));
    }

    #[test]
    fn status_bar_project_list_shows_esc_quit() {
        let state = AppState::new();
        let text = build_status_text(&state);
        assert!(
            text.contains("Esc: quit"),
            "Project list status bar should show Esc hint, got: {text}"
        );
    }

    #[test]
    fn status_bar_session_list_shows_session_count() {
        let mut state = AppState::new();
        state.view = View::SessionList;
        let text = build_status_text(&state);
        assert!(text.contains("sessions"));
    }

    #[test]
    fn status_bar_session_list_narrow_drops_help_first() {
        let mut state = AppState::new();
        state.view = View::SessionList;
        let full = build_status_text_for_width(&state, 200);
        assert!(full.contains("? help"), "Full should have ? help: {full}");

        // Width that drops ? help but keeps Esc
        let narrow = build_status_text_for_width(&state, full.len() - 1);
        assert!(
            !narrow.contains("? help"),
            "Narrow should drop ? help: {narrow}"
        );
        assert!(
            narrow.contains("Esc: back"),
            "Narrow should still have Esc: {narrow}"
        );
    }

    #[test]
    fn status_bar_session_list_very_narrow_drops_all_hints() {
        let mut state = AppState::new();
        state.view = View::SessionList;
        let text = build_status_text_for_width(&state, 15);
        assert!(
            text.contains("sessions"),
            "Very narrow should keep core: {text}"
        );
        assert!(
            !text.contains("Esc"),
            "Very narrow should drop all hints: {text}"
        );
    }

    #[test]
    fn status_bar_hides_tokens_for_zero_token_session() {
        let mut state = AppState::new();
        let session = make_session_with_tokens(2, TokenUsage::default());
        state.current_session = Some(session);
        state.view = View::Conversation(SessionId("test".to_string()));
        state.display.show_tokens = true;

        let text = build_status_text(&state);
        assert!(
            !text.contains("tokens"),
            "Should not show 'tokens' for zero-token session, got: {text}"
        );
    }

    #[test]
    fn status_bar_shows_parse_warning_count() {
        use crate::data::model::ParseWarning;

        let mut state = AppState::new();
        let mut session = make_session_with_tokens(3, TokenUsage::default());
        session.parse_warnings = vec![
            ParseWarning::MalformedLine {
                line: 1,
                reason: "bad json".to_string(),
            },
            ParseWarning::OrphanedRecord {
                uuid: "abc".to_string(),
                record_type: "assistant".to_string(),
            },
            ParseWarning::MismatchedToolResult {
                tool_use_id: "xyz".to_string(),
            },
        ];
        state.current_session = Some(session);
        state.view = View::Conversation(SessionId("test".to_string()));

        let text = build_status_text(&state);
        assert!(
            text.contains("3 warnings"),
            "Expected '3 warnings' in status, got: {text}"
        );
    }

    #[test]
    fn status_bar_hides_warnings_when_zero() {
        let mut state = AppState::new();
        let session = make_session_with_tokens(3, TokenUsage::default());
        state.current_session = Some(session);
        state.view = View::Conversation(SessionId("test".to_string()));

        let text = build_status_text(&state);
        assert!(
            !text.contains("warning"),
            "Should not show warnings when count is 0, got: {text}"
        );
    }

    #[test]
    fn status_bar_shows_singular_warning() {
        use crate::data::model::ParseWarning;

        let mut state = AppState::new();
        let mut session = make_session_with_tokens(3, TokenUsage::default());
        session.parse_warnings = vec![ParseWarning::MalformedLine {
            line: 1,
            reason: "bad".to_string(),
        }];
        state.current_session = Some(session);
        state.view = View::Conversation(SessionId("test".to_string()));

        let text = build_status_text(&state);
        assert!(
            text.contains("1 warning") && !text.contains("1 warnings"),
            "Expected singular '1 warning' in status, got: {text}"
        );
    }

    #[test]
    fn status_bar_narrow_drops_help_hint_first() {
        let mut state = AppState::new();
        let session = make_session_with_tokens(
            3,
            TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
            },
        );
        state.current_session = Some(session);
        state.view = View::Conversation(SessionId("test".to_string()));

        // Full text should contain "? help"
        let full = build_status_text_for_width(&state, 200);
        assert!(full.contains("? help"), "Full should have ? help: {full}");

        // At a width that's too narrow for the full text, "? help" should drop
        let narrow = build_status_text_for_width(&state, full.len() - 1);
        assert!(
            !narrow.contains("? help"),
            "Narrow should drop ? help: {narrow}"
        );
        // But should still have the core info
        assert!(
            narrow.contains("Turn 1/3"),
            "Should keep Turn info: {narrow}"
        );
    }

    #[test]
    fn status_bar_very_narrow_keeps_turn_info() {
        let mut state = AppState::new();
        let session = make_session_with_tokens(
            3,
            TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
            },
        );
        state.current_session = Some(session);
        state.view = View::Conversation(SessionId("test".to_string()));

        let text = build_status_text_for_width(&state, 15);
        assert!(
            text.contains("Turn 1/3"),
            "Very narrow should keep Turn info: {text}"
        );
        assert!(
            !text.contains("n/N"),
            "Very narrow should drop keybinding hints: {text}"
        );
    }

    #[test]
    fn status_bar_warning_span_is_yellow() {
        use crate::data::model::ParseWarning;

        let mut state = AppState::new();
        let mut session = make_session_with_tokens(3, TokenUsage::default());
        session.parse_warnings = vec![ParseWarning::MalformedLine {
            line: 1,
            reason: "bad".to_string(),
        }];
        state.current_session = Some(session);
        state.view = View::Conversation(SessionId("test".to_string()));

        let spans = build_status_spans_for_width(&state, usize::MAX);
        let yellow_span = spans.iter().find(|s| s.content.contains("warning"));
        assert!(
            yellow_span.is_some(),
            "Expected a span containing 'warning'"
        );
        assert_eq!(
            yellow_span.unwrap().style.fg,
            Some(Color::Yellow),
            "Warning span should be yellow"
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
