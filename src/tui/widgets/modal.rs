// Modal overlay widget for viewing full turn content.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::{AppState, ModalContent};
use crate::data::model::{ContentBlock, Turn, UserContent};

use super::conversation::{THINKING_ICON, tool_icon, tool_summary};
use super::layout::centered_rect;
use super::text_utils::truncate_end;

/// Extract the full text content for a modal based on modal type and current turn.
fn extract_modal_text(turn: &Turn, modal: &ModalContent) -> String {
    match modal {
        ModalContent::User => match &turn.user_message.content {
            UserContent::Text(text) => text.clone(),
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
                    ContentBlock::Text(text) => parts.push(text.clone()),
                    ContentBlock::Thinking { text } => {
                        parts.push(format!("{THINKING_ICON} {text}"));
                    }
                    ContentBlock::ToolUse(tc) => {
                        let icon = tool_icon(&tc.result);
                        let summary = tool_summary(&tc.name, &tc.input);
                        let mut tool_text = format!("{icon} {}  {summary}", tc.name);
                        if let Some(ref result) = tc.result
                            && !result.content.is_empty()
                        {
                            tool_text
                                .push_str(&format!("\n{}", truncate_end(&result.content, 500)));
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
    let title = match modal {
        ModalContent::User if is_tool_result => " Tool Result ",
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
    let wrapped = word_wrap_simple(&text, content_width);

    let lines: Vec<Line<'static>> = wrapped
        .into_iter()
        .map(|s| Line::from(Span::raw(s)))
        .collect();

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
            text.contains("◆ Read  /tmp/test.rs"),
            "Expected rich tool summary, got: {text}"
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
            text.contains("✗ Bash  ls /nonexistent"),
            "Expected error tool icon, got: {text}"
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
            text.contains("◇ Read  /tmp/file.txt"),
            "Expected pending tool icon, got: {text}"
        );
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
