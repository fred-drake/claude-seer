// Conversation viewer widget -- displays turns from a session.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::AppState;
use crate::data::model::{
    ContentBlock, Session, TokenUsage, ToolName, Turn, UserContent, format_tokens,
};

/// Render the conversation view into the given area.
pub fn render_conversation(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(ref session) = state.current_session else {
        return;
    };

    let (lines, current_turn_start) =
        build_conversation_lines(session, state.current_turn_index, state.show_tokens);

    let total_lines = lines.len();
    let visible_height = area.height as usize;
    let max_scroll = total_lines.saturating_sub(visible_height);
    // Use current turn's start line as scroll base so n/N navigation
    // (which resets scroll_offset to 0) shows the selected turn at top.
    let effective_scroll = current_turn_start.saturating_add(state.scroll_offset);
    let clamped_scroll = effective_scroll.min(max_scroll);

    let paragraph = Paragraph::new(lines).scroll((clamped_scroll as u16, 0));

    frame.render_widget(paragraph, area);
}

/// Build all display lines for the conversation.
///
/// Returns `(lines, current_turn_start_line)` where `current_turn_start_line`
/// is the index into `lines` where the current turn's header begins.
fn build_conversation_lines(
    session: &Session,
    current_turn_index: usize,
    show_tokens: bool,
) -> (Vec<Line<'static>>, usize) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let total_turns = session.turns.len();
    let mut cumulative = TokenUsage::default();
    let mut current_turn_start_line: usize = 0;

    for (i, turn) in session.turns.iter().enumerate() {
        // Accumulate token usage from this turn's response.
        if let Some(ref response) = turn.assistant_response {
            cumulative.add(&response.usage);
        }

        if i == current_turn_index {
            current_turn_start_line = lines.len();
        }

        let is_current = i == current_turn_index;
        let turn_lines = build_turn_lines(turn, total_turns, is_current, show_tokens, &cumulative);
        lines.extend(turn_lines);

        // Blank line between turns.
        if i + 1 < total_turns {
            lines.push(Line::from(""));
        }
    }

    (lines, current_turn_start_line)
}

/// Build display lines for a single turn.
fn build_turn_lines(
    turn: &Turn,
    total_turns: usize,
    is_current: bool,
    show_tokens: bool,
    cumulative: &TokenUsage,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Turn header.
    let header_style = if is_current {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let mut header_text = format!("--- Turn {}/{}", turn.index + 1, total_turns);
    if !turn.is_complete {
        header_text.push_str(" (incomplete)");
    }
    header_text.push_str(" ---");

    lines.push(Line::from(Span::styled(header_text, header_style)));

    // User message.
    lines.push(Line::from(Span::styled(
        "User:",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));

    let user_text = user_content_text(&turn.user_message.content);
    for text_line in user_text.lines() {
        lines.push(Line::from(format!("  {text_line}")));
    }

    // Assistant response.
    if let Some(ref response) = turn.assistant_response {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Assistant:",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));

        for block in &response.content_blocks {
            match block {
                ContentBlock::Text(text) => {
                    for text_line in text.lines() {
                        lines.push(Line::from(format!("  {text_line}")));
                    }
                }
                ContentBlock::Thinking { .. } => {
                    lines.push(Line::from(Span::styled(
                        "  [Thinking]",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                ContentBlock::ToolUse(tc) => {
                    let summary = tool_summary(&tc.name, &tc.input);
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("  [{}]", tc.name),
                            Style::default().fg(Color::Magenta),
                        ),
                        Span::styled(format!(" {summary}"), Style::default().fg(Color::DarkGray)),
                    ]));
                }
            }
        }

        // Token usage for this turn (when show_tokens is true).
        if show_tokens {
            let usage = &response.usage;
            if usage.total() > 0 {
                let mut parts = vec![
                    format!("{} in", format_tokens(usage.input_tokens)),
                    format!("{} out", format_tokens(usage.output_tokens)),
                ];
                if usage.cache_read_tokens > 0 {
                    parts.push(format!(
                        "{} cache read",
                        format_tokens(usage.cache_read_tokens)
                    ));
                }
                if usage.cache_creation_tokens > 0 {
                    parts.push(format!(
                        "{} cache write",
                        format_tokens(usage.cache_creation_tokens)
                    ));
                }
                lines.push(Line::from(Span::styled(
                    format!("  tokens: {}", parts.join(" / ")),
                    Style::default().fg(Color::DarkGray),
                )));

                // Cumulative totals.
                if cumulative.total() > 0 {
                    let mut cum_parts = vec![
                        format!("{} in", format_tokens(cumulative.input_tokens)),
                        format!("{} out", format_tokens(cumulative.output_tokens)),
                    ];
                    if cumulative.cache_read_tokens > 0 {
                        cum_parts.push(format!(
                            "{} cache read",
                            format_tokens(cumulative.cache_read_tokens)
                        ));
                    }
                    if cumulative.cache_creation_tokens > 0 {
                        cum_parts.push(format!(
                            "{} cache write",
                            format_tokens(cumulative.cache_creation_tokens)
                        ));
                    }
                    lines.push(Line::from(Span::styled(
                        format!(
                            "  \u{03A3} cumulative: {} ({} total)",
                            cum_parts.join(" / "),
                            format_tokens(cumulative.total())
                        ),
                        Style::default().fg(Color::Gray),
                    )));
                }
            }
        }
    }

    lines
}

/// Extract display text from user content.
fn user_content_text(content: &UserContent) -> String {
    match content {
        UserContent::Text(text) => text.clone(),
        UserContent::ToolResults(results) => {
            let summaries: Vec<String> = results
                .iter()
                .map(|r| {
                    if r.is_error {
                        format!("[Tool Result Error] {}", truncate(&r.content, 80))
                    } else {
                        format!("[Tool Result] {}", truncate(&r.content, 80))
                    }
                })
                .collect();
            summaries.join("\n")
        }
        UserContent::Mixed { text, tool_results } => {
            let mut parts = vec![text.clone()];
            for r in tool_results {
                if r.is_error {
                    parts.push(format!("[Tool Result Error] {}", truncate(&r.content, 80)));
                } else {
                    parts.push(format!("[Tool Result] {}", truncate(&r.content, 80)));
                }
            }
            parts.join("\n")
        }
    }
}

/// Generate a one-line summary for a tool call.
fn tool_summary(name: &ToolName, input: &serde_json::Value) -> String {
    match name {
        ToolName::Read | ToolName::Edit | ToolName::Write => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        ToolName::Bash => {
            let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
            truncate(cmd, 60)
        }
        ToolName::Glob => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        ToolName::Grep => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => {
            let s = input.to_string();
            truncate(&s, 60)
        }
    }
}

/// Truncate a string to max_len characters, appending "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .nth(max_len.saturating_sub(3))
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}...", &s[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::model::{
        AssistantResponse, MessageId, TokenUsage, ToolCall, ToolResult, Turn, UserMessage,
    };

    fn make_user_message(text: &str) -> UserMessage {
        UserMessage {
            id: MessageId("user-1".to_string()),
            timestamp: chrono::Utc::now(),
            content: UserContent::Text(text.to_string()),
        }
    }

    fn make_assistant_response(blocks: Vec<ContentBlock>) -> AssistantResponse {
        AssistantResponse {
            id: MessageId("asst-1".to_string()),
            model: "claude-opus-4-6".to_string(),
            timestamp: chrono::Utc::now(),
            content_blocks: blocks,
            usage: TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_tokens: 0,
                cache_read_tokens: 10,
            },
            stop_reason: Some("end_turn".to_string()),
        }
    }

    fn make_turn(index: usize, user_text: &str, blocks: Vec<ContentBlock>) -> Turn {
        Turn {
            index,
            user_message: make_user_message(user_text),
            assistant_response: Some(make_assistant_response(blocks)),
            duration: None,
            is_complete: true,
            events: Vec::new(),
        }
    }

    #[test]
    fn user_content_text_returns_text() {
        let content = UserContent::Text("hello world".to_string());
        assert_eq!(user_content_text(&content), "hello world");
    }

    #[test]
    fn user_content_text_tool_results() {
        let content = UserContent::ToolResults(vec![ToolResult {
            tool_use_id: "tool-1".to_string(),
            content: "result data".to_string(),
            is_error: false,
        }]);
        let text = user_content_text(&content);
        assert!(text.contains("[Tool Result]"));
        assert!(text.contains("result data"));
    }

    #[test]
    fn user_content_text_tool_results_error() {
        let content = UserContent::ToolResults(vec![ToolResult {
            tool_use_id: "tool-1".to_string(),
            content: "error msg".to_string(),
            is_error: true,
        }]);
        let text = user_content_text(&content);
        assert!(text.contains("[Tool Result Error]"));
    }

    #[test]
    fn user_content_text_mixed() {
        let content = UserContent::Mixed {
            text: "some text".to_string(),
            tool_results: vec![ToolResult {
                tool_use_id: "tool-1".to_string(),
                content: "result".to_string(),
                is_error: false,
            }],
        };
        let text = user_content_text(&content);
        assert!(text.contains("some text"));
        assert!(text.contains("[Tool Result]"));
    }

    #[test]
    fn tool_summary_read() {
        let input = serde_json::json!({"file_path": "src/main.rs"});
        let summary = tool_summary(&ToolName::Read, &input);
        assert_eq!(summary, "src/main.rs");
    }

    #[test]
    fn tool_summary_bash() {
        let input = serde_json::json!({"command": "cargo test"});
        let summary = tool_summary(&ToolName::Bash, &input);
        assert_eq!(summary, "cargo test");
    }

    #[test]
    fn tool_summary_glob() {
        let input = serde_json::json!({"pattern": "**/*.rs"});
        let summary = tool_summary(&ToolName::Glob, &input);
        assert_eq!(summary, "**/*.rs");
    }

    #[test]
    fn tool_summary_grep() {
        let input = serde_json::json!({"pattern": "fn main"});
        let summary = tool_summary(&ToolName::Grep, &input);
        assert_eq!(summary, "fn main");
    }

    #[test]
    fn tool_summary_unknown_tool() {
        let input = serde_json::json!({"key": "value"});
        let summary = tool_summary(&ToolName::Other("Custom".to_string()), &input);
        assert!(summary.contains("key"));
    }

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_string_adds_ellipsis() {
        let long = "a".repeat(100);
        let result = truncate(&long, 20);
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), 20);
    }

    #[test]
    fn build_turn_lines_includes_header() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi there".to_string())]);
        let cumulative = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_tokens: 0,
            cache_read_tokens: 10,
        };
        let lines = build_turn_lines(&turn, 3, true, true, &cumulative);
        let header = lines[0].to_string();
        assert!(header.contains("Turn 1/3"));
    }

    #[test]
    fn build_turn_lines_includes_user_label() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        let lines = build_turn_lines(&turn, 1, false, true, &TokenUsage::default());
        let has_user = lines.iter().any(|l| l.to_string().contains("User:"));
        assert!(has_user);
    }

    #[test]
    fn build_turn_lines_includes_assistant_label() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        let lines = build_turn_lines(&turn, 1, false, true, &TokenUsage::default());
        let has_asst = lines.iter().any(|l| l.to_string().contains("Assistant:"));
        assert!(has_asst);
    }

    #[test]
    fn build_turn_lines_shows_thinking_collapsed() {
        let turn = make_turn(
            0,
            "hello",
            vec![ContentBlock::Thinking {
                text: "deep thoughts".to_string(),
            }],
        );
        let lines = build_turn_lines(&turn, 1, false, true, &TokenUsage::default());
        let has_thinking = lines.iter().any(|l| l.to_string().contains("[Thinking]"));
        assert!(has_thinking);
        // Should NOT show the actual thinking text.
        let has_content = lines
            .iter()
            .any(|l| l.to_string().contains("deep thoughts"));
        assert!(!has_content);
    }

    #[test]
    fn build_turn_lines_shows_tool_use_summary() {
        let turn = make_turn(
            0,
            "hello",
            vec![ContentBlock::ToolUse(ToolCall {
                id: "tc-1".to_string(),
                name: ToolName::Read,
                input: serde_json::json!({"file_path": "src/lib.rs"}),
                result: None,
            })],
        );
        let lines = build_turn_lines(&turn, 1, false, true, &TokenUsage::default());
        let has_tool = lines
            .iter()
            .any(|l| l.to_string().contains("[Read] src/lib.rs"));
        assert!(has_tool);
    }

    #[test]
    fn build_turn_lines_shows_token_usage() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        // Cumulative should include the turn's own usage in production.
        let cumulative = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_tokens: 0,
            cache_read_tokens: 10,
        };
        let lines = build_turn_lines(&turn, 1, false, true, &cumulative);
        let has_tokens = lines
            .iter()
            .any(|l| l.to_string().contains("tokens: 100 in / 50 out"));
        assert!(has_tokens);
    }

    #[test]
    fn build_turn_lines_zero_token_usage_emits_no_token_lines() {
        let mut turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        // Override with zero usage.
        if let Some(ref mut resp) = turn.assistant_response {
            resp.usage = TokenUsage::default();
        }
        let lines = build_turn_lines(&turn, 1, false, true, &TokenUsage::default());
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !text.contains("tokens:"),
            "Should not show token line for zero usage, got: {text}"
        );
        assert!(
            !text.contains("cumulative"),
            "Should not show cumulative for zero usage, got: {text}"
        );
    }

    #[test]
    fn build_conversation_lines_cumulative_accumulation_correctness() {
        let session = crate::data::model::Session {
            id: crate::data::model::SessionId("test".to_string()),
            project: crate::data::model::ProjectPath(std::path::PathBuf::from("test")),
            file_path: std::path::PathBuf::from("/tmp/test.jsonl"),
            version: None,
            git_branch: None,
            started_at: None,
            last_activity: None,
            last_prompt: None,
            turns: vec![
                make_turn(0, "first", vec![ContentBlock::Text("r1".to_string())]),
                make_turn(1, "second", vec![ContentBlock::Text("r2".to_string())]),
            ],
            token_totals: TokenUsage::default(),
            parse_warnings: Vec::new(),
        };

        let (lines, _) = build_conversation_lines(&session, 0, true);
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        // Each turn has 100 in / 50 out / 10 cache_read.
        // Turn 2 cumulative should be 200 in / 100 out / 20 cache_read = 320 total.
        assert!(
            text.contains("200 in"),
            "Expected cumulative 200 in, got: {text}"
        );
        assert!(
            text.contains("100 out"),
            "Expected cumulative 100 out, got: {text}"
        );
        assert!(
            text.contains("320 total"),
            "Expected cumulative 320 total, got: {text}"
        );
    }

    #[test]
    fn build_turn_lines_incomplete_turn_shows_indicator() {
        let mut turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        turn.is_complete = false;
        let lines = build_turn_lines(&turn, 1, false, true, &TokenUsage::default());
        let header = lines[0].to_string();
        assert!(header.contains("(incomplete)"));
    }

    #[test]
    fn build_turn_lines_no_assistant_response() {
        let turn = Turn {
            index: 0,
            user_message: make_user_message("hello"),
            assistant_response: None,
            duration: None,
            is_complete: false,
            events: Vec::new(),
        };
        let lines = build_turn_lines(&turn, 1, false, true, &TokenUsage::default());
        // Should have header + user label + user text, but no assistant section.
        let has_asst = lines.iter().any(|l| l.to_string().contains("Assistant:"));
        assert!(!has_asst);
    }

    #[test]
    fn truncate_unicode_multibyte() {
        // Multi-byte UTF-8 chars must not cause panic from byte-slicing.
        let result = truncate("héllo wörld café", 10);
        assert!(result.ends_with("..."));
        // Should have 10 chars total (7 chars + "...")
        assert_eq!(result.chars().count(), 10);
    }

    #[test]
    fn tool_summary_edit() {
        let input = serde_json::json!({"file_path": "src/app.rs"});
        let summary = tool_summary(&ToolName::Edit, &input);
        assert_eq!(summary, "src/app.rs");
    }

    #[test]
    fn tool_summary_write() {
        let input = serde_json::json!({"file_path": "src/main.rs"});
        let summary = tool_summary(&ToolName::Write, &input);
        assert_eq!(summary, "src/main.rs");
    }

    #[test]
    fn tool_summary_missing_file_path() {
        let input = serde_json::json!({"other_key": "value"});
        let summary = tool_summary(&ToolName::Read, &input);
        assert_eq!(summary, "");
    }

    #[test]
    fn build_turn_lines_shows_cache_creation_tokens() {
        let mut turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        if let Some(ref mut resp) = turn.assistant_response {
            resp.usage.cache_creation_tokens = 200;
        }
        let lines = build_turn_lines(&turn, 1, false, true, &TokenUsage::default());
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("cache write"),
            "Expected cache write display, got: {text}"
        );
    }

    #[test]
    fn build_turn_lines_hides_tokens_when_show_tokens_false() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        let cumulative = TokenUsage {
            input_tokens: 500,
            output_tokens: 200,
            cache_creation_tokens: 0,
            cache_read_tokens: 50,
        };
        let lines = build_turn_lines(&turn, 1, false, false, &cumulative);
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !text.contains("tokens:"),
            "Should hide per-turn tokens, got: {text}"
        );
        assert!(
            !text.contains("cumulative"),
            "Should hide cumulative, got: {text}"
        );
    }

    #[test]
    fn build_turn_lines_shows_cumulative_tokens() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        let cumulative = TokenUsage {
            input_tokens: 500,
            output_tokens: 200,
            cache_creation_tokens: 0,
            cache_read_tokens: 50,
        };
        let lines = build_turn_lines(&turn, 1, false, true, &cumulative);
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("\u{03A3} cumulative: 500 in / 200 out / 50 cache read (750 total)"),
            "Expected cumulative breakdown, got: {text}"
        );
    }

    #[test]
    fn build_conversation_lines_returns_current_turn_start_line() {
        let session = crate::data::model::Session {
            id: crate::data::model::SessionId("test".to_string()),
            project: crate::data::model::ProjectPath(std::path::PathBuf::from("test")),
            file_path: std::path::PathBuf::from("/tmp/test.jsonl"),
            version: None,
            git_branch: None,
            started_at: None,
            last_activity: None,
            last_prompt: None,
            turns: vec![
                make_turn(0, "first", vec![ContentBlock::Text("reply 1".to_string())]),
                make_turn(1, "second", vec![ContentBlock::Text("reply 2".to_string())]),
                make_turn(2, "third", vec![ContentBlock::Text("reply 3".to_string())]),
            ],
            token_totals: TokenUsage::default(),
            parse_warnings: Vec::new(),
        };

        // Current turn = 0 should start at line 0.
        let (_, start_line_0) = build_conversation_lines(&session, 0, false);
        assert_eq!(start_line_0, 0);

        // Current turn = 1 should start after turn 0's lines + blank separator.
        let (_lines_t0, _) = build_conversation_lines(&session, 0, false);
        let turn_0_lines =
            build_turn_lines(&session.turns[0], 3, true, false, &TokenUsage::default());
        // Turn 0 lines + 1 blank separator = start of turn 1.
        let expected_start_1 = turn_0_lines.len() + 1;
        let (_, start_line_1) = build_conversation_lines(&session, 1, false);
        assert_eq!(start_line_1, expected_start_1);

        // Current turn = 2 should start after turns 0 and 1 + separators.
        let (_, start_line_2) = build_conversation_lines(&session, 2, false);
        assert!(start_line_2 > start_line_1);
    }

    #[test]
    fn build_conversation_lines_single_turn_start_is_zero() {
        let session = crate::data::model::Session {
            id: crate::data::model::SessionId("test".to_string()),
            project: crate::data::model::ProjectPath(std::path::PathBuf::from("test")),
            file_path: std::path::PathBuf::from("/tmp/test.jsonl"),
            version: None,
            git_branch: None,
            started_at: None,
            last_activity: None,
            last_prompt: None,
            turns: vec![make_turn(
                0,
                "only",
                vec![ContentBlock::Text("reply".to_string())],
            )],
            token_totals: TokenUsage::default(),
            parse_warnings: Vec::new(),
        };

        let (_, start_line) = build_conversation_lines(&session, 0, false);
        assert_eq!(start_line, 0);
    }

    #[test]
    fn build_conversation_lines_multiple_turns() {
        let session = crate::data::model::Session {
            id: crate::data::model::SessionId("test".to_string()),
            project: crate::data::model::ProjectPath(std::path::PathBuf::from("test")),
            file_path: std::path::PathBuf::from("/tmp/test.jsonl"),
            version: None,
            git_branch: None,
            started_at: None,
            last_activity: None,
            last_prompt: None,
            turns: vec![
                make_turn(0, "first", vec![ContentBlock::Text("reply 1".to_string())]),
                make_turn(1, "second", vec![ContentBlock::Text("reply 2".to_string())]),
            ],
            token_totals: TokenUsage::default(),
            parse_warnings: Vec::new(),
        };

        let (lines, _) = build_conversation_lines(&session, 0, true);
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Turn 1/2"));
        assert!(text.contains("Turn 2/2"));
        assert!(text.contains("first"));
        assert!(text.contains("second"));
    }
}
