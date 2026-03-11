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
    // Check for task notifications first.
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
}
