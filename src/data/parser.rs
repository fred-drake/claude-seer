use crate::data::error::DataError;
use crate::data::model::{
    AssistantResponse, ContentBlock, MessageId, TokenUsage, ToolCall, ToolName, ToolResult,
    UserContent, UserMessage,
};

/// Raw deserialized JSONL record before domain mapping.
#[derive(Debug, serde::Deserialize)]
pub struct RawRecord {
    #[serde(rename = "type")]
    pub record_type: String,
    pub uuid: Option<String>,
    #[serde(rename = "parentUuid")]
    pub parent_uuid: Option<String>,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub timestamp: Option<String>,
    pub message: Option<serde_json::Value>,
    #[serde(rename = "isSidechain")]
    pub is_sidechain: Option<bool>,
    pub version: Option<String>,
    #[serde(rename = "gitBranch")]
    pub git_branch: Option<String>,
    pub cwd: Option<String>,
    /// Catch-all for fields we haven't typed yet.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Parse a single JSONL line into a RawRecord.
pub fn parse_raw_record(line: &str, line_number: usize) -> Result<RawRecord, DataError> {
    serde_json::from_str(line).map_err(|e| DataError::ParseError {
        line: line_number,
        reason: e.to_string(),
    })
}

/// Parse a timestamp string into a chrono DateTime.
pub fn parse_timestamp(ts: &str) -> Result<chrono::DateTime<chrono::Utc>, DataError> {
    ts.parse::<chrono::DateTime<chrono::Utc>>()
        .map_err(|e| DataError::ParseError {
            line: 0,
            reason: format!("invalid timestamp '{ts}': {e}"),
        })
}

/// Extract a UserMessage from a RawRecord of type "user".
pub fn extract_user_message(record: &RawRecord) -> Result<UserMessage, DataError> {
    let uuid = record
        .uuid
        .as_deref()
        .ok_or_else(|| DataError::MissingField {
            field: "uuid".to_string(),
            record_type: "user".to_string(),
        })?;

    let ts_str = record
        .timestamp
        .as_deref()
        .ok_or_else(|| DataError::MissingField {
            field: "timestamp".to_string(),
            record_type: "user".to_string(),
        })?;
    let timestamp = parse_timestamp(ts_str)?;

    let message = record
        .message
        .as_ref()
        .ok_or_else(|| DataError::MissingField {
            field: "message".to_string(),
            record_type: "user".to_string(),
        })?;

    let content = extract_user_content(message)?;

    Ok(UserMessage {
        id: MessageId(uuid.to_string()),
        timestamp,
        content,
    })
}

/// Extract user content from the message value.
fn extract_user_content(message: &serde_json::Value) -> Result<UserContent, DataError> {
    let content = message
        .get("content")
        .ok_or_else(|| DataError::MissingField {
            field: "content".to_string(),
            record_type: "user".to_string(),
        })?;

    match content {
        serde_json::Value::String(text) => Ok(UserContent::Text(text.clone())),
        serde_json::Value::Array(items) => {
            let mut tool_results = Vec::new();
            let mut text_parts = Vec::new();

            for item in items {
                let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match item_type {
                    "tool_result" => {
                        let tool_use_id = item
                            .get("tool_use_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let result_content = item
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let is_error = item
                            .get("is_error")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        tool_results.push(ToolResult {
                            tool_use_id,
                            content: result_content,
                            is_error,
                        });
                    }
                    "text" => {
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            text_parts.push(text.to_string());
                        }
                    }
                    _ => {}
                }
            }

            if !text_parts.is_empty() && !tool_results.is_empty() {
                Ok(UserContent::Mixed {
                    text: text_parts.join("\n"),
                    tool_results,
                })
            } else if !tool_results.is_empty() {
                Ok(UserContent::ToolResults(tool_results))
            } else {
                Ok(UserContent::Text(text_parts.join("\n")))
            }
        }
        _ => Ok(UserContent::Text(content.to_string())),
    }
}

/// Extract an AssistantResponse from a RawRecord of type "assistant".
pub fn extract_assistant_response(record: &RawRecord) -> Result<AssistantResponse, DataError> {
    let uuid = record
        .uuid
        .as_deref()
        .ok_or_else(|| DataError::MissingField {
            field: "uuid".to_string(),
            record_type: "assistant".to_string(),
        })?;

    let ts_str = record
        .timestamp
        .as_deref()
        .ok_or_else(|| DataError::MissingField {
            field: "timestamp".to_string(),
            record_type: "assistant".to_string(),
        })?;
    let timestamp = parse_timestamp(ts_str)?;

    let message = record
        .message
        .as_ref()
        .ok_or_else(|| DataError::MissingField {
            field: "message".to_string(),
            record_type: "assistant".to_string(),
        })?;

    let model = message
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let stop_reason = message
        .get("stop_reason")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let content_blocks = extract_content_blocks(message)?;
    let usage = extract_token_usage(message);

    Ok(AssistantResponse {
        id: MessageId(uuid.to_string()),
        model,
        timestamp,
        content_blocks,
        usage,
        stop_reason,
    })
}

/// Extract content blocks from an assistant message.
fn extract_content_blocks(message: &serde_json::Value) -> Result<Vec<ContentBlock>, DataError> {
    let content = match message.get("content") {
        Some(serde_json::Value::Array(arr)) => arr,
        _ => return Ok(Vec::new()),
    };

    let mut blocks = Vec::new();
    for block in content {
        let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match block_type {
            "text" => {
                let text = block
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                blocks.push(ContentBlock::Text(text));
            }
            "thinking" => {
                let text = block
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                blocks.push(ContentBlock::Thinking { text });
            }
            "tool_use" => {
                let id = block
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name_str = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let input = block
                    .get("input")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                blocks.push(ContentBlock::ToolUse(ToolCall {
                    id,
                    name: ToolName::parse(name_str),
                    input,
                    result: None,
                }));
            }
            _ => {}
        }
    }

    Ok(blocks)
}

/// Extract token usage from an assistant message.
pub fn extract_token_usage(message: &serde_json::Value) -> TokenUsage {
    let usage = match message.get("usage") {
        Some(u) => u,
        None => return TokenUsage::default(),
    };

    TokenUsage {
        input_tokens: usage
            .get("input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        output_tokens: usage
            .get("output_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        cache_creation_tokens: usage
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        cache_read_tokens: usage
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
    }
}

/// Extract tool results from a tool_result record's message.
pub fn extract_tool_results(record: &RawRecord) -> Result<Vec<ToolResult>, DataError> {
    let message = record
        .message
        .as_ref()
        .ok_or_else(|| DataError::MissingField {
            field: "message".to_string(),
            record_type: "tool_result".to_string(),
        })?;

    let content = match message.get("content") {
        Some(serde_json::Value::Array(items)) => items,
        _ => return Ok(Vec::new()),
    };

    let mut results = Vec::new();
    for item in content {
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if item_type == "tool_result" {
            let tool_use_id = item
                .get("tool_use_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let result_content = item
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let is_error = item
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            results.push(ToolResult {
                tool_use_id,
                content: result_content,
                is_error,
            });
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_raw_record_from_user_line() {
        let line = r#"{"parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/home/user/project","sessionId":"sess-basic-001","version":"2.1.71","gitBranch":"main","type":"user","message":{"role":"user","content":"What is Rust?"},"uuid":"msg-001","timestamp":"2026-03-08T10:00:00.000Z","permissionMode":"bypassPermissions"}"#;
        let record = parse_raw_record(line, 1).unwrap();
        assert_eq!(record.record_type, "user");
        assert_eq!(record.uuid.as_deref(), Some("msg-001"));
        assert_eq!(record.session_id.as_deref(), Some("sess-basic-001"));
        assert!(record.parent_uuid.is_none());
        assert_eq!(record.is_sidechain, Some(false));
    }

    #[test]
    fn parse_raw_record_from_assistant_line() {
        let line = r#"{"parentUuid":"msg-001","isSidechain":false,"userType":"external","cwd":"/home/user/project","sessionId":"sess-basic-001","version":"2.1.71","gitBranch":"main","message":{"model":"claude-opus-4-6","id":"msg_resp_001","type":"message","role":"assistant","content":[{"type":"text","text":"Rust is great."}],"stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":150,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":25,"service_tier":"standard"}},"type":"assistant","uuid":"msg-002","timestamp":"2026-03-08T10:00:05.000Z"}"#;
        let record = parse_raw_record(line, 2).unwrap();
        assert_eq!(record.record_type, "assistant");
        assert_eq!(record.parent_uuid.as_deref(), Some("msg-001"));
        assert!(record.message.is_some());
    }

    #[test]
    fn parse_raw_record_malformed_line_returns_error() {
        let line = "this is not valid json";
        let result = parse_raw_record(line, 5);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            DataError::ParseError { line, .. } => assert_eq!(line, 5),
            _ => panic!("expected ParseError"),
        }
    }

    #[test]
    fn parse_raw_record_empty_json_returns_error() {
        let result = parse_raw_record("{}", 1);
        assert!(result.is_err());
    }

    #[test]
    fn parse_raw_record_from_tool_result_line() {
        let line = r#"{"parentUuid":"mt-003","isSidechain":false,"userType":"external","cwd":"/home/user/project","sessionId":"sess-multi-001","version":"2.1.71","gitBranch":"feature/auth","type":"tool_result","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool_001","content":"fn main() {}"}]},"uuid":"mt-004","timestamp":"2026-03-08T10:00:05.000Z"}"#;
        let record = parse_raw_record(line, 4).unwrap();
        assert_eq!(record.record_type, "tool_result");
        assert_eq!(record.parent_uuid.as_deref(), Some("mt-003"));
    }

    #[test]
    fn parse_raw_record_from_progress_line() {
        let line = r#"{"parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/home/user/project","sessionId":"sess-orphan-001","version":"2.1.71","gitBranch":"main","type":"progress","uuid":"orp-001","timestamp":"2026-03-08T09:59:55.000Z","message":{"type":"hook_progress","hook_name":"pre-commit","command":"cargo fmt --check"}}"#;
        let record = parse_raw_record(line, 1).unwrap();
        assert_eq!(record.record_type, "progress");
    }

    #[test]
    fn extract_user_message_text() {
        let line = r#"{"parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/home/user/project","sessionId":"sess-basic-001","version":"2.1.71","gitBranch":"main","type":"user","message":{"role":"user","content":"What is Rust?"},"uuid":"msg-001","timestamp":"2026-03-08T10:00:00.000Z","permissionMode":"bypassPermissions"}"#;
        let record = parse_raw_record(line, 1).unwrap();
        let user_msg = extract_user_message(&record).unwrap();
        assert_eq!(user_msg.id, MessageId("msg-001".to_string()));
        match &user_msg.content {
            UserContent::Text(text) => assert_eq!(text, "What is Rust?"),
            _ => panic!("expected Text content"),
        }
    }

    #[test]
    fn extract_user_message_missing_uuid_returns_error() {
        let line = r#"{"type":"user","message":{"role":"user","content":"test"},"timestamp":"2026-03-08T10:00:00.000Z"}"#;
        let record = parse_raw_record(line, 1).unwrap();
        let result = extract_user_message(&record);
        assert!(result.is_err());
    }

    #[test]
    fn extract_assistant_response_with_text_and_usage() {
        let line = r#"{"parentUuid":"msg-001","isSidechain":false,"userType":"external","cwd":"/home/user/project","sessionId":"sess-basic-001","version":"2.1.71","gitBranch":"main","message":{"model":"claude-opus-4-6","id":"msg_resp_001","type":"message","role":"assistant","content":[{"type":"text","text":"Rust is great."}],"stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":150,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":25,"service_tier":"standard"}},"type":"assistant","uuid":"msg-002","timestamp":"2026-03-08T10:00:05.000Z"}"#;
        let record = parse_raw_record(line, 2).unwrap();
        let response = extract_assistant_response(&record).unwrap();
        assert_eq!(response.id, MessageId("msg-002".to_string()));
        assert_eq!(response.model, "claude-opus-4-6");
        assert_eq!(response.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(response.usage.input_tokens, 150);
        assert_eq!(response.usage.output_tokens, 25);
        assert_eq!(response.content_blocks.len(), 1);
        match &response.content_blocks[0] {
            ContentBlock::Text(text) => assert_eq!(text, "Rust is great."),
            _ => panic!("expected Text block"),
        }
    }

    #[test]
    fn extract_assistant_response_with_tool_use() {
        let line = r#"{"parentUuid":"mt-002","isSidechain":false,"userType":"external","cwd":"/home/user/project","sessionId":"sess-multi-001","version":"2.1.71","gitBranch":"feature/auth","message":{"model":"claude-opus-4-6","id":"msg_mt_resp_001","type":"message","role":"assistant","content":[{"type":"tool_use","id":"tool_001","name":"Read","input":{"file_path":"/home/user/project/src/main.rs"}}],"stop_reason":"tool_use","stop_sequence":null,"usage":{"input_tokens":250,"cache_creation_input_tokens":0,"cache_read_input_tokens":5000,"output_tokens":45,"service_tier":"standard"}},"type":"assistant","uuid":"mt-003","timestamp":"2026-03-08T10:00:04.000Z"}"#;
        let record = parse_raw_record(line, 3).unwrap();
        let response = extract_assistant_response(&record).unwrap();
        assert_eq!(response.content_blocks.len(), 1);
        match &response.content_blocks[0] {
            ContentBlock::ToolUse(tc) => {
                assert_eq!(tc.name, ToolName::Read);
                assert_eq!(tc.id, "tool_001");
            }
            _ => panic!("expected ToolUse block"),
        }
        assert_eq!(response.usage.cache_read_tokens, 5000);
    }

    #[test]
    fn extract_tool_results_from_tool_result_record() {
        let line = r#"{"parentUuid":"mt-003","isSidechain":false,"userType":"external","cwd":"/home/user/project","sessionId":"sess-multi-001","version":"2.1.71","gitBranch":"feature/auth","type":"tool_result","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool_001","content":"fn main() {}"}]},"uuid":"mt-004","timestamp":"2026-03-08T10:00:05.000Z"}"#;
        let record = parse_raw_record(line, 4).unwrap();
        let results = extract_tool_results(&record).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_use_id, "tool_001");
        assert_eq!(results[0].content, "fn main() {}");
        assert!(!results[0].is_error);
    }

    #[test]
    fn extract_user_message_with_tool_results() {
        let line = r#"{"parentUuid":"mt-003","isSidechain":false,"userType":"external","cwd":"/home/user/project","sessionId":"sess-multi-001","version":"2.1.71","gitBranch":"feature/auth","type":"tool_result","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool_001","content":"fn main() {}"}]},"uuid":"mt-004","timestamp":"2026-03-08T10:00:05.000Z"}"#;
        let record = parse_raw_record(line, 4).unwrap();
        let user_msg = extract_user_message(&record).unwrap();
        match &user_msg.content {
            UserContent::ToolResults(results) => {
                assert_eq!(results.len(), 1);
                assert_eq!(results[0].tool_use_id, "tool_001");
            }
            _ => panic!("expected ToolResults content"),
        }
    }

    #[test]
    fn parse_timestamp_valid() {
        use chrono::Datelike;
        let ts = parse_timestamp("2026-03-08T10:00:00.000Z").unwrap();
        assert_eq!(ts.year(), 2026);
    }

    #[test]
    fn parse_timestamp_invalid_returns_error() {
        let result = parse_timestamp("not-a-timestamp");
        assert!(result.is_err());
    }

    #[test]
    fn extract_user_message_missing_timestamp_returns_error() {
        let line = r#"{"type":"user","uuid":"msg-001","message":{"role":"user","content":"test"}}"#;
        let record = parse_raw_record(line, 1).unwrap();
        let result = extract_user_message(&record);
        assert!(result.is_err());
        match result.unwrap_err() {
            DataError::MissingField { field, record_type } => {
                assert_eq!(field, "timestamp");
                assert_eq!(record_type, "user");
            }
            other => panic!("expected MissingField for timestamp, got {:?}", other),
        }
    }

    #[test]
    fn extract_user_message_missing_message_returns_error() {
        let line = r#"{"type":"user","uuid":"msg-001","timestamp":"2026-03-08T10:00:00.000Z"}"#;
        let record = parse_raw_record(line, 1).unwrap();
        let result = extract_user_message(&record);
        assert!(result.is_err());
        match result.unwrap_err() {
            DataError::MissingField { field, record_type } => {
                assert_eq!(field, "message");
                assert_eq!(record_type, "user");
            }
            other => panic!("expected MissingField for message, got {:?}", other),
        }
    }

    #[test]
    fn extract_assistant_response_missing_uuid_returns_error() {
        let line = r#"{"type":"assistant","timestamp":"2026-03-08T10:00:00.000Z","message":{"model":"claude","content":[],"usage":{"input_tokens":0,"output_tokens":0}}}"#;
        let record = parse_raw_record(line, 1).unwrap();
        let result = extract_assistant_response(&record);
        assert!(result.is_err());
        match result.unwrap_err() {
            DataError::MissingField { field, record_type } => {
                assert_eq!(field, "uuid");
                assert_eq!(record_type, "assistant");
            }
            other => panic!("expected MissingField for uuid, got {:?}", other),
        }
    }

    #[test]
    fn extract_assistant_response_missing_timestamp_returns_error() {
        let line = r#"{"type":"assistant","uuid":"msg-002","message":{"model":"claude","content":[],"usage":{"input_tokens":0,"output_tokens":0}}}"#;
        let record = parse_raw_record(line, 1).unwrap();
        let result = extract_assistant_response(&record);
        assert!(result.is_err());
        match result.unwrap_err() {
            DataError::MissingField { field, record_type } => {
                assert_eq!(field, "timestamp");
                assert_eq!(record_type, "assistant");
            }
            other => panic!("expected MissingField for timestamp, got {:?}", other),
        }
    }

    #[test]
    fn extract_assistant_response_missing_message_returns_error() {
        let line =
            r#"{"type":"assistant","uuid":"msg-002","timestamp":"2026-03-08T10:00:00.000Z"}"#;
        let record = parse_raw_record(line, 1).unwrap();
        let result = extract_assistant_response(&record);
        assert!(result.is_err());
        match result.unwrap_err() {
            DataError::MissingField { field, record_type } => {
                assert_eq!(field, "message");
                assert_eq!(record_type, "assistant");
            }
            other => panic!("expected MissingField for message, got {:?}", other),
        }
    }

    #[test]
    fn extract_user_content_mixed_text_and_tool_results() {
        let line = r#"{"type":"user","uuid":"msg-mix","timestamp":"2026-03-08T10:00:00.000Z","message":{"role":"user","content":[{"type":"text","text":"Here is the file"},{"type":"tool_result","tool_use_id":"tool_x","content":"file contents","is_error":false}]}}"#;
        let record = parse_raw_record(line, 1).unwrap();
        let user_msg = extract_user_message(&record).unwrap();
        match &user_msg.content {
            UserContent::Mixed { text, tool_results } => {
                assert_eq!(text, "Here is the file");
                assert_eq!(tool_results.len(), 1);
                assert_eq!(tool_results[0].tool_use_id, "tool_x");
            }
            other => panic!("expected Mixed content, got {:?}", other),
        }
    }

    #[test]
    fn extract_content_blocks_thinking() {
        let line = r#"{"type":"assistant","uuid":"msg-think","timestamp":"2026-03-08T10:00:00.000Z","message":{"model":"claude","content":[{"type":"thinking","text":"Let me think about this..."}],"usage":{"input_tokens":10,"output_tokens":5}}}"#;
        let record = parse_raw_record(line, 1).unwrap();
        let response = extract_assistant_response(&record).unwrap();
        assert_eq!(response.content_blocks.len(), 1);
        match &response.content_blocks[0] {
            ContentBlock::Thinking { text } => {
                assert_eq!(text, "Let me think about this...");
            }
            other => panic!("expected Thinking block, got {:?}", other),
        }
    }

    #[test]
    fn extract_tool_results_missing_message_returns_error() {
        let line =
            r#"{"type":"tool_result","uuid":"tr-001","timestamp":"2026-03-08T10:00:00.000Z"}"#;
        let record = parse_raw_record(line, 1).unwrap();
        let result = extract_tool_results(&record);
        assert!(result.is_err());
        match result.unwrap_err() {
            DataError::MissingField { field, record_type } => {
                assert_eq!(field, "message");
                assert_eq!(record_type, "tool_result");
            }
            other => panic!("expected MissingField for message, got {:?}", other),
        }
    }
}
