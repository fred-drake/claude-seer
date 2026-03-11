// Classification of user messages based on raw text content.
//
// This module provides pure data classification logic for identifying
// message types (normal, system, command, task notification) from the
// raw text content found in JSONL session logs.

/// Classification of a user message based on its raw text content.
#[derive(Debug, PartialEq)]
pub enum UserMessageKind {
    /// Normal user-typed message with cleaned text.
    Normal(String),
    /// System message (from local-command-caveat) with XML stripped.
    System(String),
    /// User command (e.g. /clear) with command name and optional args.
    Command { command: String, args: String },
    /// Teammate message from an agent team member.
    TeammateMessage {
        teammate_id: String,
        color: String,
        summary: String,
        content: String,
    },
    /// Task notification from a subagent.
    TaskNotification {
        task_id: String,
        tool_use_id: String,
        output_file: String,
        status: String,
        summary: String,
        result: String,
    },
}

/// Classify a raw user message text into Normal, System, Command, or TaskNotification.
///
/// Commands are checked first because they also contain local-command-caveat tags.
pub fn classify_user_text(raw: &str) -> UserMessageKind {
    // Check for teammate messages first.
    if raw.contains("<teammate-message") {
        // Extract attributes from opening tag.
        let teammate_id = extract_attribute(raw, "teammate-message", "teammate_id")
            .unwrap_or_default()
            .to_string();
        let color = extract_attribute(raw, "teammate-message", "color")
            .unwrap_or_default()
            .to_string();
        let summary = extract_attribute(raw, "teammate-message", "summary")
            .unwrap_or_default()
            .to_string();
        // Content is the body between opening and closing tags.
        let content = extract_teammate_body(raw).unwrap_or_default().to_string();
        return UserMessageKind::TeammateMessage {
            teammate_id,
            color,
            summary,
            content,
        };
    }

    // Check for task notifications.
    if raw.contains("<task-notification>") {
        return UserMessageKind::TaskNotification {
            task_id: extract_tag_content(raw, "task-id")
                .unwrap_or("")
                .to_string(),
            tool_use_id: extract_tag_content(raw, "tool-use-id")
                .unwrap_or("")
                .to_string(),
            output_file: extract_tag_content(raw, "output-file")
                .unwrap_or("")
                .to_string(),
            status: extract_tag_content(raw, "status")
                .unwrap_or("unknown")
                .to_string(),
            summary: extract_tag_content(raw, "summary")
                .unwrap_or("")
                .to_string(),
            result: extract_tag_content(raw, "result")
                .unwrap_or("")
                .trim()
                .to_string(),
        };
    }

    // Check for commands first -- they also contain local-command-caveat.
    if let Some(cmd) = extract_tag_content(raw, "command-name") {
        let args = extract_tag_content(raw, "command-args")
            .unwrap_or("")
            .trim()
            .to_string();
        return UserMessageKind::Command {
            command: cmd.to_string(),
            args,
        };
    }

    // Check for system messages.
    if raw.contains("<local-command-caveat>") {
        return UserMessageKind::System(strip_xml_tags(raw));
    }

    UserMessageKind::Normal(raw.to_string())
}

/// Strip all XML tags from the given text, returning only the plain content.
pub fn strip_xml_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    // Collapse whitespace and trim.
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract an attribute value from an XML-like opening tag.
///
/// For example, given `<teammate-message teammate_id="architect" color="blue">`,
/// `extract_attribute(text, "teammate-message", "teammate_id")` returns `Some("architect")`.
fn extract_attribute<'a>(text: &'a str, tag: &str, attr: &str) -> Option<&'a str> {
    let open_start = text.find(&format!("<{tag}"))?;
    let tag_end = text[open_start..].find('>')? + open_start;
    let tag_str = &text[open_start..tag_end];
    let attr_pattern = format!("{attr}=\"");
    let attr_start = tag_str.find(&attr_pattern)? + attr_pattern.len();
    let attr_end = tag_str[attr_start..].find('"')? + attr_start;
    Some(&tag_str[attr_start..attr_end])
}

/// Extract the body content of a `<teammate-message ...>...</teammate-message>` tag.
fn extract_teammate_body(text: &str) -> Option<&str> {
    let open_start = text.find("<teammate-message")?;
    let body_start = text[open_start..].find('>')? + open_start + 1;
    let body_end = text.find("</teammate-message>")?;
    if body_start >= body_end {
        return None;
    }
    let body = text[body_start..body_end].trim();
    if body.is_empty() { None } else { Some(body) }
}

/// Extract the text content between the first occurrence of `<tag>` and `</tag>`.
///
/// Uses a forward search for the closing tag from the opening tag's end position.
/// This correctly handles content containing angle brackets (e.g., `Vec<String>`)
/// as long as the content does not contain the exact closing tag string `</tag>`.
/// This assumption holds for Claude Code's structured XML format.
pub fn extract_tag_content<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open).map(|i| i + open.len())?;
    let end = text[start..].find(&close).map(|i| start + i)?;
    Some(&text[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- classify_user_text tests ---

    #[test]
    fn classify_normal_plain_text() {
        let result = classify_user_text("hello world");
        assert_eq!(result, UserMessageKind::Normal("hello world".to_string()));
    }

    #[test]
    fn classify_system_message_from_local_command_caveat() {
        let raw = "<local-command-caveat>Some system info about <b>local</b> commands</local-command-caveat>";
        let result = classify_user_text(raw);
        match result {
            UserMessageKind::System(text) => {
                assert!(!text.contains('<'), "Should strip XML tags: {text}");
                assert!(text.contains("system info"), "Should keep content: {text}");
            }
            other => panic!("Expected System, got: {other:?}"),
        }
    }

    #[test]
    fn classify_command_message() {
        let raw = "<command-name>/clear</command-name><command-args></command-args><command-message>clear the conversation</command-message>";
        let result = classify_user_text(raw);
        match result {
            UserMessageKind::Command { command, args } => {
                assert_eq!(command, "/clear");
                assert!(args.is_empty());
            }
            other => panic!("Expected Command, got: {other:?}"),
        }
    }

    #[test]
    fn classify_command_with_args() {
        let raw = "<command-name>/model</command-name><command-args>opus</command-args><command-message>switch model</command-message>";
        let result = classify_user_text(raw);
        match result {
            UserMessageKind::Command { command, args } => {
                assert_eq!(command, "/model");
                assert_eq!(args, "opus");
            }
            other => panic!("Expected Command, got: {other:?}"),
        }
    }

    #[test]
    fn classify_task_notification() {
        let raw = "<task-notification>\n<task-id>abc123</task-id>\n<tool-use-id>toolu_xyz</tool-use-id>\n<output-file>/tmp/output</output-file>\n<status>completed</status>\n<summary>Agent completed task</summary>\n<result>The result content here</result>\n</task-notification>";
        let result = classify_user_text(raw);
        match result {
            UserMessageKind::TaskNotification {
                task_id,
                tool_use_id,
                output_file,
                status,
                summary,
                result,
            } => {
                assert_eq!(task_id, "abc123");
                assert_eq!(tool_use_id, "toolu_xyz");
                assert_eq!(output_file, "/tmp/output");
                assert_eq!(status, "completed");
                assert_eq!(summary, "Agent completed task");
                assert_eq!(result, "The result content here");
            }
            other => panic!("Expected TaskNotification, got: {other:?}"),
        }
    }

    // --- Issue #5: Edge case tests ---

    #[test]
    fn classify_task_notification_missing_sub_tags() {
        // Task notification with no sub-tags at all — should use defaults.
        let raw = "<task-notification></task-notification>";
        let result = classify_user_text(raw);
        match result {
            UserMessageKind::TaskNotification {
                task_id,
                tool_use_id,
                output_file,
                status,
                summary,
                result,
            } => {
                assert_eq!(task_id, "");
                assert_eq!(tool_use_id, "");
                assert_eq!(output_file, "");
                assert_eq!(status, "unknown");
                assert_eq!(summary, "");
                assert_eq!(result, "");
            }
            other => panic!("Expected TaskNotification, got: {other:?}"),
        }
    }

    #[test]
    fn classify_task_notification_partial_sub_tags() {
        // Task notification with only status and summary — rest should default.
        let raw = "<task-notification>\n<status>failed</status>\n<summary>It broke</summary>\n</task-notification>";
        let result = classify_user_text(raw);
        match result {
            UserMessageKind::TaskNotification {
                task_id,
                status,
                summary,
                ..
            } => {
                assert_eq!(task_id, "");
                assert_eq!(status, "failed");
                assert_eq!(summary, "It broke");
            }
            other => panic!("Expected TaskNotification, got: {other:?}"),
        }
    }

    // --- classify_user_text teammate message tests ---

    #[test]
    fn classify_teammate_message() {
        let raw = r#"<teammate-message teammate_id="architect" color="blue" summary="Architecture review findings for 4 commits">
## Code Architecture Review — 4 Unpushed Commits

### Finding 1: MAJOR — `classify_user_text` belongs in `data/`

This is domain logic.
</teammate-message>"#;
        let result = classify_user_text(raw);
        match result {
            UserMessageKind::TeammateMessage {
                teammate_id,
                color,
                summary,
                content,
            } => {
                assert_eq!(teammate_id, "architect");
                assert_eq!(color, "blue");
                assert_eq!(summary, "Architecture review findings for 4 commits");
                assert!(content.contains("## Code Architecture Review"));
                assert!(content.contains("This is domain logic."));
            }
            other => panic!("Expected TeammateMessage, got: {other:?}"),
        }
    }

    #[test]
    fn classify_teammate_message_missing_attributes() {
        let raw = r#"<teammate-message teammate_id="qa">Some content</teammate-message>"#;
        let result = classify_user_text(raw);
        match result {
            UserMessageKind::TeammateMessage {
                teammate_id,
                color,
                summary,
                content,
            } => {
                assert_eq!(teammate_id, "qa");
                assert!(color.is_empty());
                assert!(summary.is_empty());
                assert_eq!(content, "Some content");
            }
            other => panic!("Expected TeammateMessage, got: {other:?}"),
        }
    }

    #[test]
    fn classify_teammate_message_empty_body() {
        let raw = r#"<teammate-message teammate_id="test" color="red" summary="empty"></teammate-message>"#;
        let result = classify_user_text(raw);
        match result {
            UserMessageKind::TeammateMessage { content, .. } => {
                assert!(content.is_empty());
            }
            other => panic!("Expected TeammateMessage, got: {other:?}"),
        }
    }

    // --- extract_tag_content tests ---

    #[test]
    fn extract_tag_content_with_angle_brackets_in_content() {
        // Content contains angle brackets that are NOT the closing tag.
        let text = "<result>Vec<String> is a type</result>";
        let content = extract_tag_content(text, "result");
        assert_eq!(content, Some("Vec<String> is a type"));
    }

    #[test]
    fn extract_tag_content_with_generic_types() {
        let text = "<result>HashMap<String, Vec<u8>></result>";
        let content = extract_tag_content(text, "result");
        assert_eq!(content, Some("HashMap<String, Vec<u8>>"));
    }

    #[test]
    fn extract_tag_content_missing_tag() {
        let text = "no tags here";
        assert_eq!(extract_tag_content(text, "result"), None);
    }

    #[test]
    fn extract_tag_content_missing_close_tag() {
        let text = "<result>some content without close";
        assert_eq!(extract_tag_content(text, "result"), None);
    }

    // --- Issue #5: Teammate message edge case tests ---

    #[test]
    fn classify_teammate_message_attribute_reorder() {
        // Attributes in different order: color before teammate_id.
        let raw = r#"<teammate-message color="green" teammate_id="qa" summary="test">body</teammate-message>"#;
        let result = classify_user_text(raw);
        match result {
            UserMessageKind::TeammateMessage {
                teammate_id,
                color,
                summary,
                content,
            } => {
                assert_eq!(teammate_id, "qa");
                assert_eq!(color, "green");
                assert_eq!(summary, "test");
                assert_eq!(content, "body");
            }
            other => panic!("Expected TeammateMessage, got: {other:?}"),
        }
    }

    #[test]
    fn classify_teammate_message_escaped_quotes_truncates() {
        // Known limitation: extract_attribute stops at first `"` after attr="
        // so escaped quotes in values like summary="Bob said \"hello\"" will
        // truncate at the backslash-quote.
        let raw = r#"<teammate-message teammate_id="qa" summary="Bob said \"hello\"">body</teammate-message>"#;
        let result = classify_user_text(raw);
        match result {
            UserMessageKind::TeammateMessage { summary, .. } => {
                // The parser finds the first closing `"` after summary=", which is
                // the backslash-escaped quote. It truncates there.
                assert_eq!(summary, r"Bob said \");
            }
            other => panic!("Expected TeammateMessage, got: {other:?}"),
        }
    }

    #[test]
    fn classify_teammate_message_missing_closing_tag() {
        // Missing </teammate-message> — body extraction returns None → empty content.
        let raw =
            r#"<teammate-message teammate_id="qa" color="blue" summary="test">unclosed content"#;
        let result = classify_user_text(raw);
        match result {
            UserMessageKind::TeammateMessage {
                teammate_id,
                content,
                ..
            } => {
                assert_eq!(teammate_id, "qa");
                assert!(
                    content.is_empty(),
                    "Content should be empty without closing tag: {content}"
                );
            }
            other => panic!("Expected TeammateMessage, got: {other:?}"),
        }
    }

    #[test]
    fn classify_teammate_message_takes_priority_over_task_notification() {
        // A message containing both <teammate-message and <task-notification>
        // should be classified as TeammateMessage (checked first).
        let raw = r#"<teammate-message teammate_id="qa" color="blue" summary="test"><task-notification>inner</task-notification></teammate-message>"#;
        let result = classify_user_text(raw);
        assert!(
            matches!(result, UserMessageKind::TeammateMessage { .. }),
            "Should be TeammateMessage, not TaskNotification: {result:?}"
        );
    }
}
