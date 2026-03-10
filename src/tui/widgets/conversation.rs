// Conversation viewer widget -- displays turns from a session.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use unicode_width::UnicodeWidthStr;

use crate::app::{AppState, DisplayOptions};
use crate::data::model::{
    ContentBlock, Session, TokenUsage, ToolName, Turn, UserContent, format_tokens,
};

use super::text_utils::truncate_end;

const TOOL_ICON_SUCCESS: &str = "◆";
const TOOL_ICON_ERROR: &str = "✗";
const TOOL_ICON_PENDING: &str = "◇";

/// Context passed to `build_turn_lines` to avoid a long parameter list.
struct TurnRenderContext<'a> {
    total_turns: usize,
    is_current: bool,
    display: DisplayOptions,
    cumulative: &'a TokenUsage,
    width: u16,
}

/// Render the conversation view into the given area.
pub fn render_conversation(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(ref session) = state.current_session else {
        return;
    };

    let (lines, current_turn_start) = build_conversation_lines(
        session,
        state.current_turn_index,
        &state.display,
        area.width,
    );

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

/// Calculate the bubble width for chat messages.
fn bubble_width(area_width: u16) -> u16 {
    let raw = (area_width as f32 * 0.75) as u16;
    raw.clamp(40, 120).min(area_width)
}

/// Word-wrap text to fit within `max_cols` columns using unicode-width.
fn word_wrap(text: &str, max_cols: usize) -> Vec<String> {
    if max_cols == 0 {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    for raw_line in text.lines() {
        if raw_line.is_empty() {
            lines.push(String::new());
            continue;
        }
        let words: Vec<&str> = raw_line.split_whitespace().collect();
        if words.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        let mut current_width: usize = 0;
        for word in words {
            let word_width = UnicodeWidthStr::width(word);
            if current.is_empty() {
                // First word — if it's wider than max, char-split it.
                if word_width > max_cols {
                    char_split_push(word, max_cols, &mut current, &mut current_width, &mut lines);
                } else {
                    current = word.to_string();
                    current_width = word_width;
                }
            } else if current_width + 1 + word_width <= max_cols {
                current.push(' ');
                current.push_str(word);
                current_width += 1 + word_width;
            } else {
                lines.push(current);
                // Start new line with this word (char-split if needed).
                if word_width > max_cols {
                    current = String::new();
                    current_width = 0;
                    char_split_push(word, max_cols, &mut current, &mut current_width, &mut lines);
                } else {
                    current = word.to_string();
                    current_width = word_width;
                }
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Build all display lines for the conversation.
///
/// Returns `(lines, current_turn_start_line)` where `current_turn_start_line`
/// is the index into `lines` where the current turn's header begins.
fn build_conversation_lines(
    session: &Session,
    current_turn_index: usize,
    display: &DisplayOptions,
    area_width: u16,
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

        let ctx = TurnRenderContext {
            total_turns,
            is_current: i == current_turn_index,
            display: *display,
            cumulative: &cumulative,
            width: area_width,
        };
        let turn_lines = build_turn_lines(turn, &ctx);
        lines.extend(turn_lines);

        // Blank line between turns.
        if i + 1 < total_turns {
            lines.push(Line::from(""));
        }
    }

    (lines, current_turn_start_line)
}

/// Build display lines for a single turn using chat-style layout.
fn build_turn_lines(turn: &Turn, ctx: &TurnRenderContext) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let show_detail = ctx.display.any_detail_enabled();
    let bw = bubble_width(ctx.width);
    // Content width = bubble_width minus 2 for "▌ " prefix.
    let content_width = bw.saturating_sub(2) as usize;
    let use_alignment = ctx.width >= 50;
    let padding_cols = if use_alignment {
        ctx.width.saturating_sub(bw) as usize
    } else {
        0
    };

    let (user_border_color, user_text_color) = if ctx.is_current {
        (Color::Cyan, Color::White)
    } else {
        (Color::DarkGray, Color::Gray)
    };
    let (asst_border_color, asst_text_color) = if ctx.is_current {
        (Color::Green, Color::White)
    } else {
        (Color::DarkGray, Color::Gray)
    };

    // Turn header (only in detail mode).
    if show_detail {
        let header_style = if ctx.is_current {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let mut header_text = format!("--- Turn {}/{}", turn.index + 1, ctx.total_turns);
        if !turn.is_complete {
            header_text.push_str(" (incomplete)");
        }
        header_text.push_str(" ---");
        lines.push(Line::from(Span::styled(header_text, header_style)));
    }

    // User message.
    if show_detail {
        lines.push(Line::from(Span::styled(
            "User:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
    }

    let user_text = user_content_text(&turn.user_message.content, ctx.display.show_tools);
    let border_style = Style::default().fg(user_border_color);
    let text_style = Style::default().fg(user_text_color);

    if !user_text.is_empty() {
        let wrapped = word_wrap(&user_text, content_width);
        for wline in wrapped {
            let mut spans = Vec::new();
            if padding_cols > 0 {
                spans.push(Span::raw(" ".repeat(padding_cols)));
            }
            spans.push(Span::styled("▌ ", border_style));
            spans.push(Span::styled(wline, text_style));
            lines.push(Line::from(spans));
        }
    }

    // Assistant response.
    if let Some(ref response) = turn.assistant_response {
        // Blank line separator between user and assistant.
        lines.push(Line::from(""));

        if show_detail {
            lines.push(Line::from(Span::styled(
                "Assistant:",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )));
        }

        let asst_border = Style::default().fg(asst_border_color);
        let asst_text = Style::default().fg(asst_text_color);

        let mut has_text = false;
        let mut tool_count: usize = 0;

        for block in &response.content_blocks {
            match block {
                ContentBlock::Text(text) => {
                    has_text = true;
                    let wrapped = word_wrap(text, content_width);
                    for wline in wrapped {
                        lines.push(Line::from(vec![
                            Span::styled("▌ ", asst_border),
                            Span::styled(wline, asst_text),
                        ]));
                    }
                }
                ContentBlock::Thinking { text } => {
                    if ctx.display.show_thinking {
                        let wrapped = word_wrap(text, content_width.saturating_sub(2));
                        for wline in wrapped {
                            lines.push(Line::from(vec![
                                Span::styled("▌ ", asst_border),
                                Span::styled(
                                    format!("  ○ {wline}"),
                                    Style::default().fg(Color::DarkGray),
                                ),
                            ]));
                        }
                    }
                }
                ContentBlock::ToolUse(tc) => {
                    tool_count += 1;
                    if ctx.display.show_tools {
                        let icon = match &tc.result {
                            Some(result) => {
                                if result.is_error {
                                    TOOL_ICON_ERROR
                                } else {
                                    TOOL_ICON_SUCCESS
                                }
                            }
                            None => TOOL_ICON_PENDING,
                        };
                        let summary = tool_summary(&tc.name, &tc.input);
                        lines.push(Line::from(vec![
                            Span::styled("▌ ", asst_border),
                            Span::styled(
                                format!("  {icon} {}  {summary}", tc.name),
                                Style::default().fg(Color::Magenta),
                            ),
                        ]));
                    }
                }
            }
        }

        // Show collapsed summary for tool-only turns when tools are hidden.
        if !has_text && tool_count > 0 && !ctx.display.show_tools {
            let label = if tool_count == 1 {
                "tool call"
            } else {
                "tool calls"
            };
            lines.push(Line::from(vec![
                Span::styled("▌ ", asst_border),
                Span::styled(
                    format!("[{tool_count} {label} — press o to show]"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        // Token usage for this turn (when show_tokens is true).
        if ctx.display.show_tokens {
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
                lines.push(Line::from(vec![
                    Span::styled("▌ ", asst_border),
                    Span::styled(
                        format!("  tokens: {}", parts.join(" / ")),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));

                // Cumulative totals.
                if ctx.cumulative.total() > 0 {
                    let mut cum_parts = vec![
                        format!("{} in", format_tokens(ctx.cumulative.input_tokens)),
                        format!("{} out", format_tokens(ctx.cumulative.output_tokens)),
                    ];
                    if ctx.cumulative.cache_read_tokens > 0 {
                        cum_parts.push(format!(
                            "{} cache read",
                            format_tokens(ctx.cumulative.cache_read_tokens)
                        ));
                    }
                    if ctx.cumulative.cache_creation_tokens > 0 {
                        cum_parts.push(format!(
                            "{} cache write",
                            format_tokens(ctx.cumulative.cache_creation_tokens)
                        ));
                    }
                    lines.push(Line::from(vec![
                        Span::styled("▌ ", asst_border),
                        Span::styled(
                            format!(
                                "  \u{03A3} cumulative: {} ({} total)",
                                cum_parts.join(" / "),
                                format_tokens(ctx.cumulative.total())
                            ),
                            Style::default().fg(Color::Gray),
                        ),
                    ]));
                }
            }
        }
    }

    lines
}

/// Extract display text from user content.
fn user_content_text(content: &UserContent, show_tools: bool) -> String {
    match content {
        UserContent::Text(text) => text.clone(),
        UserContent::ToolResults(results) => {
            if !show_tools {
                return String::new();
            }
            let summaries: Vec<String> = results
                .iter()
                .map(|r| {
                    if r.is_error {
                        format!("[Tool Result Error] {}", truncate_end(&r.content, 80))
                    } else {
                        format!("[Tool Result] {}", truncate_end(&r.content, 80))
                    }
                })
                .collect();
            summaries.join("\n")
        }
        UserContent::Mixed { text, tool_results } => {
            let mut parts = vec![text.clone()];
            if show_tools {
                for r in tool_results {
                    if r.is_error {
                        parts.push(format!(
                            "[Tool Result Error] {}",
                            truncate_end(&r.content, 80)
                        ));
                    } else {
                        parts.push(format!("[Tool Result] {}", truncate_end(&r.content, 80)));
                    }
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
            truncate_end(cmd, 60)
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
            truncate_end(&s, 60)
        }
    }
}

/// Split a word character-by-character into lines when it exceeds `max_cols`.
fn char_split_push(
    word: &str,
    max_cols: usize,
    current: &mut String,
    current_width: &mut usize,
    lines: &mut Vec<String>,
) {
    for ch in word.chars() {
        let ch_w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if *current_width + ch_w > max_cols && !current.is_empty() {
            lines.push(std::mem::take(current));
            *current_width = 0;
        }
        current.push(ch);
        *current_width += ch_w;
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

    /// Helper: build a TurnRenderContext with sensible defaults.
    fn ctx(
        total_turns: usize,
        is_current: bool,
        display: DisplayOptions,
        cumulative: &TokenUsage,
    ) -> TurnRenderContext<'_> {
        TurnRenderContext {
            total_turns,
            is_current,
            display,
            cumulative,
            width: 80,
        }
    }

    /// DisplayOptions with show_tokens enabled (for detail-mode tests).
    fn display_tokens() -> DisplayOptions {
        DisplayOptions {
            show_tokens: true,
            show_tools: false,
            show_thinking: false,
        }
    }

    /// DisplayOptions with show_tools enabled.
    fn display_tools() -> DisplayOptions {
        DisplayOptions {
            show_tokens: false,
            show_tools: true,
            show_thinking: false,
        }
    }

    /// DisplayOptions with show_thinking enabled.
    fn display_thinking() -> DisplayOptions {
        DisplayOptions {
            show_tokens: false,
            show_tools: false,
            show_thinking: true,
        }
    }

    /// DisplayOptions with all flags enabled.
    fn display_all() -> DisplayOptions {
        DisplayOptions {
            show_tokens: true,
            show_tools: true,
            show_thinking: true,
        }
    }

    fn lines_text(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn make_session(turns: Vec<Turn>) -> crate::data::model::Session {
        crate::data::model::Session {
            id: crate::data::model::SessionId("test".to_string()),
            project: crate::data::model::ProjectPath(std::path::PathBuf::from("test")),
            file_path: std::path::PathBuf::from("/tmp/test.jsonl"),
            version: None,
            git_branch: None,
            started_at: None,
            last_activity: None,
            last_prompt: None,
            turns,
            token_totals: TokenUsage::default(),
            parse_warnings: Vec::new(),
        }
    }

    // --- user_content_text tests ---

    #[test]
    fn user_content_text_returns_text() {
        let content = UserContent::Text("hello world".to_string());
        assert_eq!(user_content_text(&content, true), "hello world");
    }

    #[test]
    fn user_content_text_tool_results() {
        let content = UserContent::ToolResults(vec![ToolResult {
            tool_use_id: "tool-1".to_string(),
            content: "result data".to_string(),
            is_error: false,
        }]);
        let text = user_content_text(&content, true);
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
        let text = user_content_text(&content, true);
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
        let text = user_content_text(&content, true);
        assert!(text.contains("some text"));
        assert!(text.contains("[Tool Result]"));
    }

    #[test]
    fn build_turn_lines_hides_user_tool_results_when_tools_hidden() {
        let content = UserContent::ToolResults(vec![ToolResult {
            tool_use_id: "tool-1".to_string(),
            content: "result data".to_string(),
            is_error: false,
        }]);
        let text = user_content_text(&content, false);
        assert!(text.is_empty());
    }

    #[test]
    fn build_turn_lines_mixed_content_respects_filters() {
        let content = UserContent::Mixed {
            text: "some text".to_string(),
            tool_results: vec![ToolResult {
                tool_use_id: "tool-1".to_string(),
                content: "result".to_string(),
                is_error: false,
            }],
        };
        let text = user_content_text(&content, false);
        assert!(text.contains("some text"));
        assert!(!text.contains("[Tool Result]"));
    }

    // --- tool_summary tests ---

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

    // --- truncate tests ---

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate_end("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_string_adds_ellipsis() {
        let long = "a".repeat(100);
        let result = truncate_end(&long, 20);
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), 20);
    }

    #[test]
    fn truncate_unicode_multibyte() {
        let result = truncate_end("héllo wörld café", 10);
        assert!(result.ends_with("..."));
        assert_eq!(result.chars().count(), 10);
    }

    // --- bubble_width tests ---

    #[test]
    fn bubble_width_80_cols_is_60() {
        assert_eq!(bubble_width(80), 60);
    }

    #[test]
    fn bubble_width_clamps_narrow() {
        // 30 * 0.75 = 22, clamp min 40, but min(40, 30) = 30
        assert_eq!(bubble_width(30), 30);
    }

    #[test]
    fn bubble_width_clamps_wide() {
        // 200 * 0.75 = 150, clamp max 120
        assert_eq!(bubble_width(200), 120);
    }

    #[test]
    fn bubble_width_30_cols() {
        // area_width=30 → raw=22, clamp(40,120)=40, min(40,30)=30
        assert_eq!(bubble_width(30), 30);
    }

    #[test]
    fn terminal_width_zero_no_panic() {
        assert_eq!(bubble_width(0), 0);
    }

    // --- word_wrap tests ---

    #[test]
    fn short_text_not_wrapped() {
        let result = word_wrap("hello world", 60);
        assert_eq!(result, vec!["hello world"]);
    }

    #[test]
    fn long_text_wraps_at_bubble_width() {
        let result = word_wrap("the quick brown fox jumps over the lazy dog", 20);
        assert!(result.len() > 1);
        for line in &result {
            assert!(UnicodeWidthStr::width(line.as_str()) <= 20);
        }
    }

    // --- build_turn_lines tests (detail mode — headers/labels visible) ---

    #[test]
    fn build_turn_lines_includes_header() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi there".to_string())]);
        let cumulative = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_tokens: 0,
            cache_read_tokens: 10,
        };
        let c = ctx(3, true, display_tokens(), &cumulative);
        let lines = build_turn_lines(&turn, &c);
        let header = lines[0].to_string();
        assert!(header.contains("Turn 1/3"));
    }

    #[test]
    fn build_turn_lines_includes_user_label() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        let default_usage = TokenUsage::default();
        let c = ctx(1, false, display_tokens(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        let has_user = lines.iter().any(|l| l.to_string().contains("User:"));
        assert!(has_user);
    }

    #[test]
    fn build_turn_lines_includes_assistant_label() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        let default_usage = TokenUsage::default();
        let c = ctx(1, false, display_tokens(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        let has_asst = lines.iter().any(|l| l.to_string().contains("Assistant:"));
        assert!(has_asst);
    }

    // --- clean view (default) omits headers/labels ---

    #[test]
    fn clean_view_omits_headers_and_labels() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        let default_usage = TokenUsage::default();
        let c = ctx(1, false, DisplayOptions::default(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        let text = lines_text(&lines);
        assert!(!text.contains("Turn 1/1"), "Should not have header: {text}");
        assert!(
            !text.contains("User:"),
            "Should not have User label: {text}"
        );
        assert!(
            !text.contains("Assistant:"),
            "Should not have Assistant label: {text}"
        );
    }

    #[test]
    fn detail_view_shows_headers_and_labels() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        let default_usage = TokenUsage::default();
        let c = ctx(1, false, display_all(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        let text = lines_text(&lines);
        assert!(text.contains("Turn 1/1"));
        assert!(text.contains("User:"));
        assert!(text.contains("Assistant:"));
    }

    #[test]
    fn each_flag_independently_triggers_headers() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        for opts in [display_tokens(), display_tools(), display_thinking()] {
            let default_usage = TokenUsage::default();
            let c = ctx(1, false, opts, &default_usage);
            let lines = build_turn_lines(&turn, &c);
            let text = lines_text(&lines);
            assert!(
                text.contains("Turn 1/1"),
                "Should have header with {opts:?}"
            );
        }
    }

    // --- tool/thinking filtering ---

    #[test]
    fn build_turn_lines_hides_tool_use_when_show_tools_false() {
        let turn = make_turn(
            0,
            "hello",
            vec![
                ContentBlock::Text("text".to_string()),
                ContentBlock::ToolUse(ToolCall {
                    id: "tc-1".to_string(),
                    name: ToolName::Read,
                    input: serde_json::json!({"file_path": "src/lib.rs"}),
                    result: None,
                }),
            ],
        );
        let default_usage = TokenUsage::default();
        let c = ctx(1, false, DisplayOptions::default(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        let text = lines_text(&lines);
        assert!(!text.contains("Read"), "Should hide tool: {text}");
    }

    #[test]
    fn build_turn_lines_shows_tool_use_when_show_tools_true() {
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
        let default_usage = TokenUsage::default();
        let c = ctx(1, false, display_tools(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        let text = lines_text(&lines);
        assert!(text.contains("Read"), "Should show tool: {text}");
        assert!(text.contains("src/lib.rs"), "Should show summary: {text}");
    }

    #[test]
    fn build_turn_lines_hides_thinking_when_show_thinking_false() {
        let turn = make_turn(
            0,
            "hello",
            vec![ContentBlock::Thinking {
                text: "deep thoughts".to_string(),
            }],
        );
        let default_usage = TokenUsage::default();
        let c = ctx(1, false, DisplayOptions::default(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        let text = lines_text(&lines);
        assert!(
            !text.contains("deep thoughts"),
            "Should hide thinking: {text}"
        );
    }

    #[test]
    fn build_turn_lines_shows_thinking_text_when_show_thinking_true() {
        let turn = make_turn(
            0,
            "hello",
            vec![ContentBlock::Thinking {
                text: "deep thoughts".to_string(),
            }],
        );
        let default_usage = TokenUsage::default();
        let c = ctx(1, false, display_thinking(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        let text = lines_text(&lines);
        assert!(
            text.contains("deep thoughts"),
            "Should show thinking text: {text}"
        );
        assert!(text.contains("○"), "Should show thinking icon: {text}");
    }

    #[test]
    fn build_turn_lines_tool_only_turn_shows_collapsed_summary() {
        let turn = make_turn(
            0,
            "hello",
            vec![
                ContentBlock::ToolUse(ToolCall {
                    id: "tc-1".to_string(),
                    name: ToolName::Read,
                    input: serde_json::json!({"file_path": "a.rs"}),
                    result: None,
                }),
                ContentBlock::ToolUse(ToolCall {
                    id: "tc-2".to_string(),
                    name: ToolName::Edit,
                    input: serde_json::json!({"file_path": "b.rs"}),
                    result: None,
                }),
            ],
        );
        let default_usage = TokenUsage::default();
        let c = ctx(1, false, DisplayOptions::default(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        let text = lines_text(&lines);
        assert!(
            text.contains("2 tool calls"),
            "Should show collapsed tool count: {text}"
        );
        assert!(
            text.contains("press o to show"),
            "Should hint at toggle: {text}"
        );
    }

    // --- chat alignment ---

    #[test]
    fn user_lines_have_leading_padding_at_width_80() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        let default_usage = TokenUsage::default();
        let c = ctx(1, true, DisplayOptions::default(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        // The user message line should have padding (right-aligned).
        let user_line = lines
            .iter()
            .find(|l| l.to_string().contains("hello"))
            .unwrap();
        let first_span = &user_line.spans[0];
        // At width=80, bubble=60, padding = 20 spaces.
        assert!(
            first_span.content.starts_with("                    "),
            "Expected 20-char padding, got: '{}'",
            first_span.content
        );
    }

    #[test]
    fn assistant_lines_have_no_leading_padding() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        let default_usage = TokenUsage::default();
        let c = ctx(1, true, DisplayOptions::default(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        // Find assistant line with "hi" (not the user "hello" line).
        let asst_line = lines
            .iter()
            .find(|l| {
                let s = l.to_string();
                s.contains("hi") && !s.contains("hello")
            })
            .unwrap();
        // First span should be the border "▌ ", not padding.
        assert_eq!(asst_line.spans[0].content.as_ref(), "▌ ");
    }

    #[test]
    fn narrow_terminal_disables_alignment() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        let cumulative = TokenUsage::default();
        let c = TurnRenderContext {
            total_turns: 1,
            is_current: true,
            display: DisplayOptions::default(),
            cumulative: &cumulative,
            width: 40, // < 50
        };
        let lines = build_turn_lines(&turn, &c);
        // User line should NOT have leading padding.
        let user_line = lines
            .iter()
            .find(|l| l.to_string().contains("hello"))
            .unwrap();
        assert_eq!(
            user_line.spans[0].content.as_ref(),
            "▌ ",
            "Narrow: user line should start with border, not padding"
        );
    }

    // --- token display ---

    #[test]
    fn build_turn_lines_shows_token_usage() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        let cumulative = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_tokens: 0,
            cache_read_tokens: 10,
        };
        let c = ctx(1, false, display_tokens(), &cumulative);
        let lines = build_turn_lines(&turn, &c);
        let text = lines_text(&lines);
        assert!(
            text.contains("tokens: 100 in / 50 out"),
            "Expected token display, got: {text}"
        );
    }

    #[test]
    fn build_turn_lines_zero_token_usage_emits_no_token_lines() {
        let mut turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        if let Some(ref mut resp) = turn.assistant_response {
            resp.usage = TokenUsage::default();
        }
        let default_usage = TokenUsage::default();
        let c = ctx(1, false, display_tokens(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        let text = lines_text(&lines);
        assert!(
            !text.contains("tokens:"),
            "Should not show token line: {text}"
        );
        assert!(
            !text.contains("cumulative"),
            "Should not show cumulative: {text}"
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
        let c = ctx(1, false, DisplayOptions::default(), &cumulative);
        let lines = build_turn_lines(&turn, &c);
        let text = lines_text(&lines);
        assert!(!text.contains("tokens:"), "Should hide tokens: {text}");
        assert!(
            !text.contains("cumulative"),
            "Should hide cumulative: {text}"
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
        let c = ctx(1, false, display_tokens(), &cumulative);
        let lines = build_turn_lines(&turn, &c);
        let text = lines_text(&lines);
        assert!(
            text.contains("\u{03A3} cumulative: 500 in / 200 out / 50 cache read (750 total)"),
            "Expected cumulative, got: {text}"
        );
    }

    #[test]
    fn build_turn_lines_shows_cache_creation_tokens() {
        let mut turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        if let Some(ref mut resp) = turn.assistant_response {
            resp.usage.cache_creation_tokens = 200;
        }
        let default_usage = TokenUsage::default();
        let c = ctx(1, false, display_tokens(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        let text = lines_text(&lines);
        assert!(text.contains("cache write"), "Expected cache write: {text}");
    }

    #[test]
    fn build_turn_lines_incomplete_turn_shows_indicator() {
        let mut turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        turn.is_complete = false;
        // Need detail mode to see header.
        let default_usage = TokenUsage::default();
        let c = ctx(1, false, display_tokens(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        let text = lines_text(&lines);
        assert!(text.contains("(incomplete)"), "Expected incomplete: {text}");
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
        let default_usage = TokenUsage::default();
        let c = ctx(1, false, display_tokens(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        let has_asst = lines.iter().any(|l| l.to_string().contains("Assistant:"));
        assert!(!has_asst);
    }

    // --- build_conversation_lines tests ---

    #[test]
    fn build_conversation_lines_cumulative_accumulation_correctness() {
        let session = make_session(vec![
            make_turn(0, "first", vec![ContentBlock::Text("r1".to_string())]),
            make_turn(1, "second", vec![ContentBlock::Text("r2".to_string())]),
        ]);

        let (lines, _) = build_conversation_lines(&session, 0, &display_tokens(), 80);
        let text = lines_text(&lines);

        // Each turn has 100 in / 50 out / 10 cache_read.
        // Turn 2 cumulative should be 200 in / 100 out / 20 cache_read = 320 total.
        assert!(
            text.contains("200 in"),
            "Expected cumulative 200 in: {text}"
        );
        assert!(
            text.contains("100 out"),
            "Expected cumulative 100 out: {text}"
        );
        assert!(
            text.contains("320 total"),
            "Expected cumulative 320 total: {text}"
        );
    }

    #[test]
    fn build_conversation_lines_returns_current_turn_start_line() {
        let session = make_session(vec![
            make_turn(0, "first", vec![ContentBlock::Text("reply 1".to_string())]),
            make_turn(1, "second", vec![ContentBlock::Text("reply 2".to_string())]),
            make_turn(2, "third", vec![ContentBlock::Text("reply 3".to_string())]),
        ]);

        let display = DisplayOptions::default();

        // Current turn = 0 should start at line 0.
        let (_, start_line_0) = build_conversation_lines(&session, 0, &display, 80);
        assert_eq!(start_line_0, 0);

        // Current turn = 1 should start after turn 0's lines + blank separator.
        let (_, start_line_1) = build_conversation_lines(&session, 1, &display, 80);
        assert!(start_line_1 > 0);

        // Current turn = 2 should start after turns 0 and 1 + separators.
        let (_, start_line_2) = build_conversation_lines(&session, 2, &display, 80);
        assert!(start_line_2 > start_line_1);
    }

    #[test]
    fn build_conversation_lines_single_turn_start_is_zero() {
        let session = make_session(vec![make_turn(
            0,
            "only",
            vec![ContentBlock::Text("reply".to_string())],
        )]);

        let (_, start_line) = build_conversation_lines(&session, 0, &DisplayOptions::default(), 80);
        assert_eq!(start_line, 0);
    }

    #[test]
    fn build_conversation_lines_multiple_turns() {
        let session = make_session(vec![
            make_turn(0, "first", vec![ContentBlock::Text("reply 1".to_string())]),
            make_turn(1, "second", vec![ContentBlock::Text("reply 2".to_string())]),
        ]);

        let (lines, _) = build_conversation_lines(&session, 0, &display_tokens(), 80);
        let text = lines_text(&lines);
        assert!(text.contains("Turn 1/2"));
        assert!(text.contains("Turn 2/2"));
        assert!(text.contains("first"));
        assert!(text.contains("second"));
    }

    // --- ▌ border tests ---

    #[test]
    fn lines_contain_left_border() {
        let turn = make_turn(0, "hello", vec![ContentBlock::Text("hi".to_string())]);
        let default_usage = TokenUsage::default();
        let c = ctx(1, true, DisplayOptions::default(), &default_usage);
        let lines = build_turn_lines(&turn, &c);
        let text = lines_text(&lines);
        assert!(text.contains('▌'), "Expected ▌ border: {text}");
    }
}
