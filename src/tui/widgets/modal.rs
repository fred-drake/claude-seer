// Modal overlay widget for viewing full turn content.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::{AppState, ModalContent};
use crate::data::model::{ContentBlock, Turn, UserContent};

use super::conversation::{THINKING_ICON, tool_icon};
use super::layout::centered_rect;
use super::md_wrap::markdown_wrap;
use crate::data::classify::extract_tag_content;

/// Extract the full text content for a modal based on modal type and current turn.
fn extract_modal_text(turn: &Turn, modal: &ModalContent) -> String {
    match modal {
        ModalContent::User => match &turn.user_message.content {
            UserContent::Text(text) if text.contains("<task-notification>") => {
                let task_id = extract_tag_content(text, "task-id").unwrap_or("");
                let tool_use_id = extract_tag_content(text, "tool-use-id").unwrap_or("");
                let output_file = extract_tag_content(text, "output-file").unwrap_or("");
                let status = extract_tag_content(text, "status").unwrap_or("");
                let summary = extract_tag_content(text, "summary").unwrap_or("");
                let result = extract_tag_content(text, "result").unwrap_or("").trim();

                let mut parts = Vec::new();
                if !task_id.is_empty() {
                    parts.push(format!("Task ID: {task_id}"));
                }
                if !tool_use_id.is_empty() {
                    parts.push(format!("Tool Use ID: {tool_use_id}"));
                }
                if !output_file.is_empty() {
                    parts.push(format!("Output File: {output_file}"));
                }
                if !status.is_empty() {
                    parts.push(format!("Status: {status}"));
                }
                if !summary.is_empty() {
                    parts.push(format!("Summary: {summary}"));
                }
                parts.push(String::new()); // blank line separator
                parts.push(result.to_string());
                parts.join("\n")
            }
            UserContent::Text(text) => text.trim().to_string(),
            UserContent::ToolResults(results) => results
                .iter()
                .map(|r| {
                    if r.is_error {
                        format!("[Error] {}", r.content)
                    } else {
                        r.content.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
            UserContent::Mixed { text, tool_results } => {
                let mut parts = vec![text.clone()];
                for r in tool_results {
                    if r.is_error {
                        parts.push(format!("[Error] {}", r.content));
                    } else {
                        parts.push(r.content.clone());
                    }
                }
                parts.join("\n")
            }
        },
        ModalContent::Claude => {
            let Some(ref response) = turn.assistant_response else {
                return String::new();
            };
            let mut parts: Vec<String> = Vec::new();
            for block in &response.content_blocks {
                match block {
                    ContentBlock::Text(text) => {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            parts.push(trimmed.to_string());
                        }
                    }
                    ContentBlock::Thinking { text } => {
                        if !text.trim().is_empty() {
                            parts.push(format!("{THINKING_ICON} {text}"));
                        }
                    }
                    ContentBlock::ToolUse(tc) => {
                        let icon = tool_icon(&tc.result);
                        let input_str = serde_json::to_string_pretty(&tc.input)
                            .unwrap_or_else(|_| tc.input.to_string());
                        let mut tool_text = format!("{icon} {}:\n{input_str}", tc.name);
                        if let Some(ref result) = tc.result
                            && !result.content.is_empty()
                        {
                            tool_text.push_str(&format!("\n\n{}", result.content));
                        }
                        parts.push(tool_text);
                    }
                }
            }
            parts.join("\n\n")
        }
    }
}

/// Word-wrap text to fit within `max_cols` columns.
fn word_wrap_simple(text: &str, max_cols: usize) -> Vec<String> {
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
        let mut current_len: usize = 0;
        for word in words {
            let word_len = word.len();
            if current.is_empty() {
                current = word.to_string();
                current_len = word_len;
            } else if current_len + 1 + word_len <= max_cols {
                current.push(' ');
                current.push_str(word);
                current_len += 1 + word_len;
            } else {
                lines.push(current);
                current = word.to_string();
                current_len = word_len;
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

/// Build modal lines for a Claude response, rendering text blocks as markdown
/// and tool blocks as plain formatted text (no truncation, no code-block styling).
fn build_claude_modal_lines(turn: &Turn, content_width: usize) -> Vec<Line<'static>> {
    let Some(ref response) = turn.assistant_response else {
        return vec![Line::from("")];
    };

    let base_style = Style::default().fg(Color::White);
    let mut result: Vec<Line<'static>> = Vec::new();
    let mut first_block = true;

    for block in &response.content_blocks {
        // Blank line separator between blocks.
        if !first_block {
            result.push(Line::from(""));
        }
        first_block = false;

        match block {
            ContentBlock::Text(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    let md_lines = markdown_wrap(trimmed, content_width, base_style, true);
                    for bl in md_lines {
                        result.push(Line::from(bl.spans));
                    }
                }
            }
            ContentBlock::Thinking { text } => {
                if !text.trim().is_empty() {
                    let header = format!("{THINKING_ICON} Thinking:");
                    result.push(Line::from(Span::styled(
                        header,
                        Style::default().fg(Color::DarkGray),
                    )));
                    for line in word_wrap_simple(text, content_width) {
                        result.push(Line::from(Span::styled(
                            line,
                            Style::default().fg(Color::DarkGray),
                        )));
                    }
                }
            }
            ContentBlock::ToolUse(tc) => {
                let icon = tool_icon(&tc.result);
                result.push(Line::from(Span::styled(
                    format!("{icon} {}:", tc.name),
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )));

                // Pretty-print JSON input with syntax highlighting.
                let input_str = serde_json::to_string_pretty(&tc.input)
                    .unwrap_or_else(|_| tc.input.to_string());
                for line in input_str.lines() {
                    // Preserve leading indent, wrap the rest.
                    let indent_len = line.len() - line.trim_start().len();
                    let indent = &line[..indent_len];
                    let body = &line[indent_len..];
                    let wrap_width = content_width.saturating_sub(indent_len);
                    if wrap_width == 0 || body.len() <= wrap_width {
                        result.push(Line::from(json_highlight_line(line)));
                    } else {
                        // Detect the value color from the first line so
                        // continuation lines keep the same highlight.
                        let value_color = json_value_color(body);
                        let wrapped = word_wrap_simple(body, wrap_width);
                        for (i, segment) in wrapped.iter().enumerate() {
                            if i == 0 {
                                // First segment: full highlight with key.
                                let indented = format!("{indent}{segment}");
                                result.push(Line::from(json_highlight_line(&indented)));
                            } else {
                                // Continuation: indent + value color.
                                result.push(Line::from(Span::styled(
                                    format!("{indent}{segment}"),
                                    Style::default().fg(value_color),
                                )));
                            }
                        }
                    }
                }

                // Tool result content.
                if let Some(ref tool_result) = tc.result
                    && !tool_result.content.is_empty()
                {
                    result.push(Line::from(""));
                    let result_color = if tool_result.is_error {
                        Color::Red
                    } else {
                        Color::Green
                    };
                    for line in word_wrap_simple(&tool_result.content, content_width) {
                        result.push(Line::from(Span::styled(
                            line,
                            Style::default().fg(result_color),
                        )));
                    }
                }
            }
        }
    }

    if result.is_empty() {
        result.push(Line::from(""));
    }
    result
}

/// Build modal lines for a task notification, showing properties then markdown result.
fn build_task_modal_lines(text: &str, content_width: usize) -> Vec<Line<'static>> {
    let mut result: Vec<Line<'static>> = Vec::new();

    // Split into header properties and markdown body.
    let (header, md_body) = if let Some(idx) = text.find("\n\n") {
        (&text[..idx], text[idx + 2..].trim())
    } else {
        (text, "")
    };

    // Render header properties as styled lines.
    for line in header.lines() {
        result.push(Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Color::Yellow),
        )));
    }
    result.push(Line::from(""));

    // Render body as markdown.
    if !md_body.is_empty() {
        let base_style = Style::default().fg(Color::White);
        let md_lines = markdown_wrap(md_body, content_width, base_style, true);
        for bl in md_lines {
            result.push(Line::from(bl.spans));
        }
    }

    result
}

/// Syntax-highlight a single line of pretty-printed JSON into styled spans.
///
/// Colors: keys=Cyan, strings=Green, numbers/bools/null=Yellow, punctuation=DarkGray.
fn json_highlight_line(line: &str) -> Vec<Span<'static>> {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let punct_style = Style::default().fg(Color::DarkGray);

    let mut spans: Vec<Span<'static>> = Vec::new();
    if !indent.is_empty() {
        spans.push(Span::raw(indent.to_string()));
    }

    if trimmed.is_empty() {
        return spans;
    }

    // Pure structural lines: { } [ ] or with trailing comma
    let stripped = trimmed.trim_end_matches(',');
    if matches!(stripped, "{" | "}" | "[" | "]") {
        spans.push(Span::styled(trimmed.to_string(), punct_style));
        return spans;
    }

    // Key-value line: "key": value  or  "key": "value"
    if trimmed.starts_with('"') {
        if let Some(colon_pos) = find_key_end(trimmed) {
            let key_part = &trimmed[..colon_pos]; // includes quotes
            let separator = ": ";
            let value_start = colon_pos + separator.len();

            spans.push(Span::styled(
                key_part.to_string(),
                Style::default().fg(Color::Cyan),
            ));
            spans.push(Span::styled(": ".to_string(), punct_style));

            if value_start <= trimmed.len() {
                let value_part = &trimmed[value_start..];
                spans.extend(json_value_spans(value_part));
            }
            return spans;
        }
        // Standalone string value (e.g., in an array)
        spans.extend(json_value_spans(trimmed));
        return spans;
    }

    // Non-string value line (number, bool, null in array context)
    spans.extend(json_value_spans(trimmed));
    spans
}

/// Find the end of a JSON key (closing quote position + 1) in a `"key": value` line.
fn find_key_end(s: &str) -> Option<usize> {
    // Skip opening quote, find closing quote.
    let after_open = &s[1..];
    let close_quote = after_open.find('"')?;
    let key_end = 1 + close_quote + 1; // position after closing quote
    // Check that `: ` follows.
    if s[key_end..].starts_with(": ") {
        Some(key_end)
    } else {
        None
    }
}

/// Color a JSON value portion (after the `": "`).
fn json_value_spans(value: &str) -> Vec<Span<'static>> {
    let punct_style = Style::default().fg(Color::DarkGray);
    let trimmed = value.trim_end_matches(',');
    let trailing_comma = value.len() > trimmed.len();

    let mut spans = Vec::new();

    if trimmed.starts_with('"') {
        // String value
        spans.push(Span::styled(
            trimmed.to_string(),
            Style::default().fg(Color::Green),
        ));
    } else if matches!(trimmed, "true" | "false" | "null") || trimmed.parse::<f64>().is_ok() {
        spans.push(Span::styled(
            trimmed.to_string(),
            Style::default().fg(Color::Yellow),
        ));
    } else {
        // Structural or complex (e.g., `{`, `[`)
        spans.push(Span::styled(trimmed.to_string(), punct_style));
    }

    if trailing_comma {
        spans.push(Span::styled(",".to_string(), punct_style));
    }

    spans
}

/// Determine the highlight color for a JSON line's value portion.
///
/// Used to carry the value color forward onto wrapped continuation lines.
fn json_value_color(line: &str) -> Color {
    let trimmed = line.trim_start();
    // If it's a key-value line, look at the value after ": ".
    if trimmed.starts_with('"') {
        if let Some(colon_pos) = find_key_end(trimmed) {
            let value_start = colon_pos + 2; // skip ": "
            if value_start < trimmed.len() {
                let value = trimmed[value_start..].trim_end_matches(',');
                if value.starts_with('"') {
                    return Color::Green;
                } else if matches!(value, "true" | "false" | "null") || value.parse::<f64>().is_ok()
                {
                    return Color::Yellow;
                }
            }
        }
        // Standalone string (array element)
        return Color::Green;
    }
    Color::DarkGray
}

/// Render a modal overlay centered on the screen.
pub fn render_modal(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(ref modal) = state.modal else {
        return;
    };

    let Some(ref session) = state.current_session else {
        return;
    };

    if session.turns.is_empty() || state.current_turn_index >= session.turns.len() {
        return;
    }

    let turn = &session.turns[state.current_turn_index];
    let text = extract_modal_text(turn, modal);

    let is_tool_result = matches!(&turn.user_message.content, UserContent::ToolResults(_));
    let is_task_notification = matches!(
        &turn.user_message.content,
        UserContent::Text(t) if t.contains("<task-notification>")
    );
    let title = match modal {
        ModalContent::User if is_tool_result => " Tool Result ",
        ModalContent::User if is_task_notification => " Task Response ",
        ModalContent::User => " User Message ",
        ModalContent::Claude => " Claude Response ",
    };

    let border_color = match modal {
        ModalContent::User => Color::Cyan,
        ModalContent::Claude => Color::Magenta,
    };

    // Modal size: ~80% of screen
    let modal_width = (area.width as f32 * 0.8) as u16;
    let modal_height = (area.height as f32 * 0.8) as u16;
    let modal_width = modal_width.max(20).min(area.width.saturating_sub(2));
    let modal_height = modal_height.max(5).min(area.height.saturating_sub(2));

    let popup_area = centered_rect(modal_width, modal_height, area);

    // Content width inside the border (border takes 2 chars each side)
    let content_width = modal_width.saturating_sub(2) as usize;

    let lines: Vec<Line<'static>> = if matches!(modal, ModalContent::Claude) {
        build_claude_modal_lines(turn, content_width)
    } else if is_task_notification {
        build_task_modal_lines(&text, content_width)
    } else {
        let wrapped = word_wrap_simple(&text, content_width);
        wrapped
            .into_iter()
            .map(|s| Line::from(Span::raw(s)))
            .collect()
    };

    // Clear behind the popup
    frame.render_widget(Clear, popup_area);

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .scroll((state.modal_scroll as u16, 0));

    frame.render_widget(paragraph, popup_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::model::{
        AssistantResponse, ContentBlock, MessageId, TokenUsage, ToolCall, ToolName, ToolResult,
        Turn, UserContent, UserMessage,
    };

    fn make_user_turn(text: &str) -> Turn {
        Turn {
            index: 0,
            user_message: UserMessage {
                id: MessageId("msg-1".to_string()),
                timestamp: chrono::Utc::now(),
                content: UserContent::Text(text.to_string()),
            },
            assistant_response: None,
            duration: None,
            is_complete: true,
            events: Vec::new(),
        }
    }

    fn make_turn_with_response(user_text: &str, asst_text: &str) -> Turn {
        Turn {
            index: 0,
            user_message: UserMessage {
                id: MessageId("msg-1".to_string()),
                timestamp: chrono::Utc::now(),
                content: UserContent::Text(user_text.to_string()),
            },
            assistant_response: Some(AssistantResponse {
                id: MessageId("msg-2".to_string()),
                model: "claude-sonnet-4-20250514".to_string(),
                timestamp: chrono::Utc::now(),
                content_blocks: vec![ContentBlock::Text(asst_text.to_string())],
                usage: TokenUsage::default(),
                stop_reason: None,
            }),
            duration: None,
            is_complete: true,
            events: Vec::new(),
        }
    }

    #[test]
    fn extract_user_text() {
        let turn = make_user_turn("Hello world");
        let text = extract_modal_text(&turn, &ModalContent::User);
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn extract_claude_text() {
        let turn = make_turn_with_response("hi", "This is the response");
        let text = extract_modal_text(&turn, &ModalContent::Claude);
        assert_eq!(text, "This is the response");
    }

    #[test]
    fn extract_claude_no_response() {
        let turn = make_user_turn("hi");
        let text = extract_modal_text(&turn, &ModalContent::Claude);
        assert_eq!(text, "");
    }

    #[test]
    fn extract_claude_with_thinking() {
        let turn = Turn {
            index: 0,
            user_message: UserMessage {
                id: MessageId("msg-1".to_string()),
                timestamp: chrono::Utc::now(),
                content: UserContent::Text("hi".to_string()),
            },
            assistant_response: Some(AssistantResponse {
                id: MessageId("msg-2".to_string()),
                model: "claude-sonnet-4-20250514".to_string(),
                timestamp: chrono::Utc::now(),
                content_blocks: vec![
                    ContentBlock::Thinking {
                        text: "Let me think".to_string(),
                    },
                    ContentBlock::Text("Here is my answer".to_string()),
                ],
                usage: TokenUsage::default(),
                stop_reason: None,
            }),
            duration: None,
            is_complete: true,
            events: Vec::new(),
        };
        let text = extract_modal_text(&turn, &ModalContent::Claude);
        assert!(
            text.contains("○ Let me think"),
            "Expected thinking icon, got: {text}"
        );
        assert!(text.contains("Here is my answer"));
    }

    #[test]
    fn extract_claude_with_tool_use() {
        let turn = Turn {
            index: 0,
            user_message: UserMessage {
                id: MessageId("msg-1".to_string()),
                timestamp: chrono::Utc::now(),
                content: UserContent::Text("hi".to_string()),
            },
            assistant_response: Some(AssistantResponse {
                id: MessageId("msg-2".to_string()),
                model: "claude-sonnet-4-20250514".to_string(),
                timestamp: chrono::Utc::now(),
                content_blocks: vec![ContentBlock::ToolUse(ToolCall {
                    id: "tc-1".to_string(),
                    name: ToolName::Read,
                    input: serde_json::json!({"file_path": "/tmp/test.rs"}),
                    result: Some(ToolResult {
                        tool_use_id: "tc-1".to_string(),
                        content: "file contents".to_string(),
                        is_error: false,
                    }),
                })],
                usage: TokenUsage::default(),
                stop_reason: None,
            }),
            duration: None,
            is_complete: true,
            events: Vec::new(),
        };
        let text = extract_modal_text(&turn, &ModalContent::Claude);
        assert!(
            text.contains("◆ Read"),
            "Expected tool icon and name, got: {text}"
        );
        assert!(
            text.contains("/tmp/test.rs"),
            "Expected full file path in input, got: {text}"
        );
        assert!(
            text.contains("file contents"),
            "Expected tool result content, got: {text}"
        );
    }

    #[test]
    fn extract_claude_with_error_tool() {
        let turn = Turn {
            index: 0,
            user_message: UserMessage {
                id: MessageId("msg-1".to_string()),
                timestamp: chrono::Utc::now(),
                content: UserContent::Text("hi".to_string()),
            },
            assistant_response: Some(AssistantResponse {
                id: MessageId("msg-2".to_string()),
                model: "claude-sonnet-4-20250514".to_string(),
                timestamp: chrono::Utc::now(),
                content_blocks: vec![ContentBlock::ToolUse(ToolCall {
                    id: "tc-1".to_string(),
                    name: ToolName::Bash,
                    input: serde_json::json!({"command": "ls /nonexistent"}),
                    result: Some(ToolResult {
                        tool_use_id: "tc-1".to_string(),
                        content: "No such file or directory".to_string(),
                        is_error: true,
                    }),
                })],
                usage: TokenUsage::default(),
                stop_reason: None,
            }),
            duration: None,
            is_complete: true,
            events: Vec::new(),
        };
        let text = extract_modal_text(&turn, &ModalContent::Claude);
        assert!(
            text.contains("✗ Bash"),
            "Expected error tool icon and name, got: {text}"
        );
        assert!(
            text.contains("ls /nonexistent"),
            "Expected full command in input, got: {text}"
        );
        assert!(
            text.contains("No such file or directory"),
            "Expected error content, got: {text}"
        );
    }

    #[test]
    fn extract_claude_with_pending_tool() {
        let turn = Turn {
            index: 0,
            user_message: UserMessage {
                id: MessageId("msg-1".to_string()),
                timestamp: chrono::Utc::now(),
                content: UserContent::Text("hi".to_string()),
            },
            assistant_response: Some(AssistantResponse {
                id: MessageId("msg-2".to_string()),
                model: "claude-sonnet-4-20250514".to_string(),
                timestamp: chrono::Utc::now(),
                content_blocks: vec![ContentBlock::ToolUse(ToolCall {
                    id: "tc-1".to_string(),
                    name: ToolName::Read,
                    input: serde_json::json!({"file_path": "/tmp/file.txt"}),
                    result: None,
                })],
                usage: TokenUsage::default(),
                stop_reason: None,
            }),
            duration: None,
            is_complete: true,
            events: Vec::new(),
        };
        let text = extract_modal_text(&turn, &ModalContent::Claude);
        assert!(
            text.contains("◇ Read"),
            "Expected pending tool icon and name, got: {text}"
        );
        assert!(
            text.contains("/tmp/file.txt"),
            "Expected full file path in input, got: {text}"
        );
    }

    #[test]
    fn extract_task_notification_modal_shows_properties() {
        let raw = "<task-notification>\n<task-id>abc123</task-id>\n<tool-use-id>toolu_xyz</tool-use-id>\n<output-file>/tmp/output</output-file>\n<status>completed</status>\n<summary>Agent completed task</summary>\n<result>The detailed result</result>\n</task-notification>";
        let turn = make_user_turn(raw);
        let text = extract_modal_text(&turn, &ModalContent::User);
        assert!(
            text.contains("Task ID: abc123"),
            "Should show task ID: {text}"
        );
        assert!(
            text.contains("Tool Use ID: toolu_xyz"),
            "Should show tool use ID: {text}"
        );
        assert!(
            text.contains("Output File: /tmp/output"),
            "Should show output file: {text}"
        );
        assert!(
            text.contains("Status: completed"),
            "Should show status: {text}"
        );
        assert!(
            text.contains("Summary: Agent completed task"),
            "Should show summary: {text}"
        );
        assert!(
            text.contains("The detailed result"),
            "Should show result: {text}"
        );
    }

    // --- JSON syntax highlighting tests ---

    fn span_texts<'a>(spans: &'a [Span<'a>]) -> Vec<(&'a str, Option<Color>)> {
        spans
            .iter()
            .map(|s| (s.content.as_ref(), s.style.fg))
            .collect()
    }

    #[test]
    fn json_highlight_key_value_string() {
        let spans = json_highlight_line(r#"  "name": "hello","#);
        let texts = span_texts(&spans);
        // indent, key, colon, value, comma
        assert!(
            texts
                .iter()
                .any(|(t, c)| *t == "\"name\"" && *c == Some(Color::Cyan))
        );
        assert!(
            texts
                .iter()
                .any(|(t, c)| *t == "\"hello\"" && *c == Some(Color::Green))
        );
    }

    #[test]
    fn json_highlight_key_value_number() {
        let spans = json_highlight_line(r#"  "count": 42"#);
        let texts = span_texts(&spans);
        assert!(
            texts
                .iter()
                .any(|(t, c)| *t == "\"count\"" && *c == Some(Color::Cyan))
        );
        assert!(
            texts
                .iter()
                .any(|(t, c)| *t == "42" && *c == Some(Color::Yellow))
        );
    }

    #[test]
    fn json_highlight_key_value_bool() {
        let spans = json_highlight_line(r#"  "flag": true,"#);
        let texts = span_texts(&spans);
        assert!(
            texts
                .iter()
                .any(|(t, c)| *t == "\"flag\"" && *c == Some(Color::Cyan))
        );
        assert!(
            texts
                .iter()
                .any(|(t, c)| *t == "true" && *c == Some(Color::Yellow))
        );
        assert!(
            texts
                .iter()
                .any(|(t, c)| *t == "," && *c == Some(Color::DarkGray))
        );
    }

    #[test]
    fn json_highlight_structural_brace() {
        let spans = json_highlight_line("  {");
        let texts = span_texts(&spans);
        assert!(
            texts
                .iter()
                .any(|(t, c)| *t == "{" && *c == Some(Color::DarkGray))
        );
    }

    #[test]
    fn json_highlight_preserves_indent() {
        let spans = json_highlight_line(r#"    "key": "val""#);
        assert_eq!(spans[0].content.as_ref(), "    ");
    }

    #[test]
    fn word_wrap_simple_basic() {
        let lines = word_wrap_simple("hello world foo bar", 10);
        assert_eq!(lines, vec!["hello", "world foo", "bar"]);
    }

    #[test]
    fn word_wrap_simple_preserves_newlines() {
        let lines = word_wrap_simple("line one\nline two", 80);
        assert_eq!(lines, vec!["line one", "line two"]);
    }

    #[test]
    fn word_wrap_simple_empty() {
        let lines = word_wrap_simple("", 80);
        assert_eq!(lines, vec![""]);
    }
}
