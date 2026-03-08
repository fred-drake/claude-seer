use std::path::PathBuf;

use crate::data::error::DataError;
use crate::data::model::{
    AssistantResponse, ContentBlock, ParseWarning, ProjectPath, Session, SessionId, TokenUsage,
    ToolResult, Turn, UserMessage,
};
use crate::data::parser::{
    RawRecord, extract_assistant_response, extract_token_usage, extract_tool_results,
    extract_user_message, parse_raw_record, parse_timestamp,
};

/// Turn assembly state machine states.
#[derive(Debug)]
enum AssemblerState {
    /// Waiting for a user message to start a new turn.
    AwaitingUser,
    /// Have a user message, waiting for assistant response.
    HaveUser(UserMessage),
    /// Have user + assistant, may get more assistant blocks or tool_results.
    HavePair {
        user_message: UserMessage,
        assistant_responses: Vec<AssistantResponse>,
    },
}

/// Summary scan result to avoid complex tuple return.
pub struct SummaryScanResult {
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
    pub git_branch: Option<String>,
    pub version: Option<String>,
    pub turn_count: usize,
    pub total_tokens: TokenUsage,
}

/// Load and fully parse a session from a string of JSONL content.
///
/// TODO: This currently collects all parsed records into a Vec before
/// assembling turns. For large sessions, this could be optimized to
/// stream records through the assembler instead of collecting.
pub fn load_session_from_str(
    content: &str,
    id: SessionId,
    project: ProjectPath,
    file_path: PathBuf,
) -> Result<Session, DataError> {
    let mut warnings: Vec<ParseWarning> = Vec::new();
    let mut records: Vec<(usize, RawRecord)> = Vec::new();

    // Parse all lines, collecting warnings for bad ones.
    for (line_idx, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match parse_raw_record(line, line_idx + 1) {
            Ok(record) => records.push((line_idx + 1, record)),
            Err(_) => {
                warnings.push(ParseWarning::MalformedLine {
                    line: line_idx + 1,
                    reason: "failed to parse JSON".to_string(),
                });
            }
        }
    }

    // Extract metadata from first record.
    let version = records.first().and_then(|(_, r)| r.version.clone());
    let git_branch = records.first().and_then(|(_, r)| r.git_branch.clone());
    let started_at = records
        .first()
        .and_then(|(_, r)| r.timestamp.as_deref())
        .and_then(|ts| parse_timestamp(ts).ok());
    let last_activity = records
        .last()
        .and_then(|(_, r)| r.timestamp.as_deref())
        .and_then(|ts| parse_timestamp(ts).ok());

    // Filter out sidechains and non-conversation records.
    let conversation_records: Vec<&RawRecord> = records
        .iter()
        .filter(|(_, r)| {
            if r.is_sidechain == Some(true) {
                if let Some(uuid) = &r.uuid {
                    warnings.push(ParseWarning::SkippedSidechain { uuid: uuid.clone() });
                }
                return false;
            }
            true
        })
        .map(|(_, r)| r)
        .collect();

    // Assemble turns using state machine.
    let turns = assemble_turns(&conversation_records, &mut warnings);

    // Compute token totals.
    let mut token_totals = TokenUsage::default();
    for turn in &turns {
        if let Some(ref response) = turn.assistant_response {
            token_totals.add(&response.usage);
        }
    }

    Ok(Session {
        id,
        project,
        file_path,
        version,
        git_branch,
        started_at,
        last_activity,
        last_prompt: None, // Populated by caller from summary scan.
        turns,
        token_totals,
        parse_warnings: warnings,
    })
}

/// Assemble raw records into turns using a state machine.
fn assemble_turns(records: &[&RawRecord], warnings: &mut Vec<ParseWarning>) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    let mut state = AssemblerState::AwaitingUser;

    // Build a map of tool_use_id -> tool_result for correlation.
    let mut tool_result_map: std::collections::HashMap<String, ToolResult> =
        std::collections::HashMap::new();
    for record in records {
        if record.record_type == "tool_result"
            && let Ok(results) = extract_tool_results(record)
        {
            for result in results {
                tool_result_map.insert(result.tool_use_id.clone(), result);
            }
        }
    }

    for record in records {
        match record.record_type.as_str() {
            "user" => {
                // Finalize any in-progress turn.
                state = finalize_turn(state, &mut turns, &tool_result_map);

                match extract_user_message(record) {
                    Ok(user_msg) => {
                        state = AssemblerState::HaveUser(user_msg);
                    }
                    Err(_) => {
                        if let Some(uuid) = &record.uuid {
                            warnings.push(ParseWarning::OrphanedRecord {
                                uuid: uuid.clone(),
                                record_type: "user".to_string(),
                            });
                        }
                    }
                }
            }
            "assistant" => {
                if let Ok(response) = extract_assistant_response(record) {
                    state = match state {
                        AssemblerState::HaveUser(user_msg) => AssemblerState::HavePair {
                            user_message: user_msg,
                            assistant_responses: vec![response],
                        },
                        AssemblerState::HavePair {
                            user_message,
                            mut assistant_responses,
                        } => {
                            assistant_responses.push(response);
                            AssemblerState::HavePair {
                                user_message,
                                assistant_responses,
                            }
                        }
                        AssemblerState::AwaitingUser => {
                            // Orphaned assistant without user.
                            if let Some(uuid) = &record.uuid {
                                warnings.push(ParseWarning::OrphanedRecord {
                                    uuid: uuid.clone(),
                                    record_type: "assistant".to_string(),
                                });
                            }
                            AssemblerState::AwaitingUser
                        }
                    };
                } else if matches!(state, AssemblerState::AwaitingUser) {
                    // Failed to parse orphaned assistant.
                    if let Some(uuid) = &record.uuid {
                        warnings.push(ParseWarning::OrphanedRecord {
                            uuid: uuid.clone(),
                            record_type: "assistant".to_string(),
                        });
                    }
                }
            }
            "tool_result" => {
                // Tool results continue the current turn. The result
                // is already in tool_result_map for correlation.
                if !matches!(&state, AssemblerState::HavePair { .. }) {
                    // Orphaned tool result.
                    if let Some(uuid) = &record.uuid {
                        warnings.push(ParseWarning::OrphanedRecord {
                            uuid: uuid.clone(),
                            record_type: "tool_result".to_string(),
                        });
                    }
                }
            }
            "progress" | "system" | "file-history-snapshot" | "queue-operation" => {
                // Skip non-conversation records for now.
            }
            "last-prompt" => {
                // Filtered out -- used in summary scan only.
            }
            _ => {
                // Unknown record type -- skip.
            }
        }
    }

    // Finalize any remaining in-progress turn.
    finalize_turn(state, &mut turns, &tool_result_map);

    turns
}

/// Finalize the current state into a turn (if applicable) and return AwaitingUser.
fn finalize_turn(
    state: AssemblerState,
    turns: &mut Vec<Turn>,
    tool_result_map: &std::collections::HashMap<String, ToolResult>,
) -> AssemblerState {
    match state {
        AssemblerState::AwaitingUser => {}
        AssemblerState::HaveUser(user_msg) => {
            // Incomplete turn -- user message without assistant response.
            // Note: duration is always None for now. It will be computed from
            // system/progress records once system record parsing is implemented.
            turns.push(Turn {
                index: turns.len(),
                user_message: user_msg,
                assistant_response: None,
                duration: None,
                is_complete: false,
                events: Vec::new(),
            });
        }
        AssemblerState::HavePair {
            user_message,
            assistant_responses,
        } => {
            // Merge all assistant responses into one.
            let merged = merge_assistant_responses(assistant_responses, tool_result_map);
            let is_complete = merged
                .stop_reason
                .as_deref()
                .is_some_and(|r| r == "end_turn");

            turns.push(Turn {
                index: turns.len(),
                user_message,
                assistant_response: Some(merged),
                duration: None,
                is_complete,
                events: Vec::new(),
            });
        }
    }
    AssemblerState::AwaitingUser
}

/// Merge multiple assistant responses into one.
fn merge_assistant_responses(
    responses: Vec<AssistantResponse>,
    tool_result_map: &std::collections::HashMap<String, ToolResult>,
) -> AssistantResponse {
    debug_assert!(!responses.is_empty());

    let first = &responses[0];
    let last = responses.last().unwrap();

    let mut all_blocks: Vec<ContentBlock> = Vec::new();
    let mut total_usage = TokenUsage::default();

    for response in &responses {
        for block in &response.content_blocks {
            let mut block = block.clone();
            // Correlate tool use blocks with their results.
            if let ContentBlock::ToolUse(ref mut tc) = block
                && let Some(result) = tool_result_map.get(&tc.id)
            {
                tc.result = Some(result.clone());
            }
            all_blocks.push(block);
        }
        total_usage.add(&response.usage);
    }

    AssistantResponse {
        id: first.id.clone(),
        model: first.model.clone(),
        timestamp: first.timestamp,
        content_blocks: all_blocks,
        usage: total_usage,
        stop_reason: last.stop_reason.clone(),
    }
}

/// Extract the last-prompt value from a session's content (pass 1).
pub fn extract_last_prompt(content: &str) -> Option<String> {
    let mut last_prompt = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Quick check before full parse.
        if !line.contains("\"type\"") {
            continue;
        }

        if let Ok(record) = parse_raw_record(line, 0)
            && record.record_type == "last-prompt"
            && let Some(msg) = &record.message
            && let Some(text) = msg.get("content").and_then(|v| v.as_str())
        {
            last_prompt = Some(text.to_string());
        }
    }

    // If no explicit last-prompt, find the last user message.
    if last_prompt.is_none() {
        for line in content.lines().rev() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(record) = parse_raw_record(line, 0)
                && record.record_type == "user"
                && let Some(msg) = &record.message
                && let Some(text) = msg.get("content").and_then(|v| v.as_str())
            {
                last_prompt = Some(text.to_string());
                break;
            }
        }
    }

    last_prompt
}

/// Summary scan: extract timestamps and metadata without full parse.
pub fn summary_scan(content: &str) -> SummaryScanResult {
    let mut result = SummaryScanResult {
        started_at: None,
        last_activity: None,
        git_branch: None,
        version: None,
        turn_count: 0,
        total_tokens: TokenUsage::default(),
    };

    let mut lines = content.lines();

    // First line for start time and metadata.
    if let Some(first_line) = lines.next() {
        let first_line = first_line.trim();
        if !first_line.is_empty()
            && let Ok(record) = parse_raw_record(first_line, 1)
        {
            result.started_at = record
                .timestamp
                .as_deref()
                .and_then(|ts| parse_timestamp(ts).ok());
            result.git_branch = record.git_branch;
            result.version = record.version;

            // Also process first line for turn counting and token accumulation.
            if first_line.contains("\"type\":\"user\"")
                && !first_line.contains("\"isSidechain\":true")
            {
                result.turn_count += 1;
            }
            if first_line.contains("\"type\":\"assistant\"")
                && let Some(ref msg) = record.message
            {
                let usage = extract_token_usage(msg);
                result.total_tokens.add(&usage);
            }
        }
    }

    // Track last non-empty line for end time, count user turns, and accumulate tokens.
    let mut last_non_empty: Option<String> = None;
    let mut line_count = 1; // Already processed line 1 above.
    for line in lines {
        let line = line.trim();
        line_count += 1;
        if line.is_empty() {
            continue;
        }
        last_non_empty = Some(line.to_string());

        if line.contains("\"type\":\"user\"") && !line.contains("\"isSidechain\":true") {
            result.turn_count += 1;
        }

        // Accumulate tokens from assistant lines.
        if line.contains("\"type\":\"assistant\"")
            && let Ok(record) = parse_raw_record(line, line_count)
            && let Some(ref msg) = record.message
        {
            let usage = extract_token_usage(msg);
            result.total_tokens.add(&usage);
        }
    }

    // Last line for end time. If no subsequent lines, last_activity = started_at.
    if let Some(last_line) = last_non_empty
        && let Ok(record) = parse_raw_record(&last_line, line_count)
    {
        result.last_activity = record
            .timestamp
            .as_deref()
            .and_then(|ts| parse_timestamp(ts).ok());
    } else {
        result.last_activity = result.started_at;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn load_fixture(name: &str) -> Session {
        let content = std::fs::read_to_string(fixture_path(name)).unwrap();
        load_session_from_str(
            &content,
            SessionId("test-session".to_string()),
            ProjectPath(PathBuf::from("/test")),
            fixture_path(name),
        )
        .unwrap()
    }

    #[test]
    fn assemble_basic_session_one_turn() {
        let session = load_fixture("session_basic.jsonl");
        assert_eq!(session.turns.len(), 1);
        assert!(session.turns[0].is_complete);
        assert!(session.turns[0].assistant_response.is_some());
    }

    #[test]
    fn assemble_linear_session_three_turns() {
        let session = load_fixture("session_linear.jsonl");
        assert_eq!(session.turns.len(), 3);
        for turn in &session.turns {
            assert!(turn.is_complete);
            assert!(turn.assistant_response.is_some());
        }
        assert_eq!(session.turns[0].index, 0);
        assert_eq!(session.turns[1].index, 1);
        assert_eq!(session.turns[2].index, 2);
    }

    #[test]
    fn assemble_multi_turn_with_tool_use() {
        let session = load_fixture("session_multi_turn.jsonl");
        assert_eq!(session.turns.len(), 2);

        // First turn should have tool use blocks.
        let response = session.turns[0].assistant_response.as_ref().unwrap();
        let has_tool_use = response
            .content_blocks
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolUse(_)));
        assert!(has_tool_use);

        // Tool results should be correlated.
        let tool_block = response
            .content_blocks
            .iter()
            .find_map(|b| match b {
                ContentBlock::ToolUse(tc) if tc.id == "tool_001" => Some(tc),
                _ => None,
            })
            .unwrap();
        assert!(tool_block.result.is_some());
    }

    #[test]
    fn assemble_sidechain_records_skipped() {
        let session = load_fixture("session_sidechain.jsonl");
        assert_eq!(session.turns.len(), 1);
        assert!(session.turns[0].is_complete);

        let sidechain_warnings: Vec<_> = session
            .parse_warnings
            .iter()
            .filter(|w| matches!(w, ParseWarning::SkippedSidechain { .. }))
            .collect();
        assert!(!sidechain_warnings.is_empty());
    }

    #[test]
    fn assemble_mid_toolcall_incomplete_turn() {
        let session = load_fixture("session_mid_toolcall.jsonl");
        assert_eq!(session.turns.len(), 1);
        assert!(!session.turns[0].is_complete);
    }

    #[test]
    fn assemble_consecutive_users() {
        let session = load_fixture("session_consecutive_users.jsonl");
        assert_eq!(session.turns.len(), 2);
        assert!(!session.turns[0].is_complete);
        assert!(session.turns[0].assistant_response.is_none());
        assert!(session.turns[1].is_complete);
    }

    #[test]
    fn assemble_session_with_errors_skips_bad_lines() {
        let session = load_fixture("session_with_errors.jsonl");
        assert!(!session.turns.is_empty());
        let malformed_warnings: Vec<_> = session
            .parse_warnings
            .iter()
            .filter(|w| matches!(w, ParseWarning::MalformedLine { .. }))
            .collect();
        assert!(!malformed_warnings.is_empty());
    }

    #[test]
    fn assemble_empty_session() {
        let session = load_fixture("session_empty.jsonl");
        assert!(session.turns.is_empty());
    }

    #[test]
    fn assemble_resumed_session() {
        let session = load_fixture("session_resumed.jsonl");
        assert_eq!(session.turns.len(), 2);
        for turn in &session.turns {
            assert!(turn.is_complete);
        }
    }

    #[test]
    fn assemble_orphaned_progress_records() {
        let session = load_fixture("session_orphaned_progress.jsonl");
        assert_eq!(session.turns.len(), 1);
        assert!(session.turns[0].is_complete);
    }

    #[test]
    fn assemble_mismatched_tool_result() {
        let session = load_fixture("session_mismatched_toolresult.jsonl");
        assert!(!session.turns.is_empty());

        let response = session.turns[0].assistant_response.as_ref().unwrap();
        let tool_block = response
            .content_blocks
            .iter()
            .find_map(|b| match b {
                ContentBlock::ToolUse(tc) if tc.id == "tool_mm_001" => Some(tc),
                _ => None,
            })
            .unwrap();
        assert!(tool_block.result.is_none());
    }

    #[test]
    fn token_totals_accumulated() {
        let session = load_fixture("session_linear.jsonl");
        assert!(session.token_totals.input_tokens > 0);
        assert!(session.token_totals.output_tokens > 0);
    }

    #[test]
    fn session_metadata_extracted() {
        let session = load_fixture("session_basic.jsonl");
        assert_eq!(session.version.as_deref(), Some("2.1.71"));
        assert_eq!(session.git_branch.as_deref(), Some("main"));
        assert!(session.started_at.is_some());
        assert!(session.last_activity.is_some());
    }

    #[test]
    fn extract_last_prompt_from_basic_session() {
        let content = std::fs::read_to_string(fixture_path("session_basic.jsonl")).unwrap();
        let prompt = extract_last_prompt(&content);
        assert_eq!(prompt.as_deref(), Some("What is Rust?"));
    }

    #[test]
    fn extract_last_prompt_from_linear_session() {
        let content = std::fs::read_to_string(fixture_path("session_linear.jsonl")).unwrap();
        let prompt = extract_last_prompt(&content);
        assert_eq!(prompt.as_deref(), Some("What editor support is available?"));
    }

    #[test]
    fn summary_scan_extracts_timestamps() {
        let content = std::fs::read_to_string(fixture_path("session_basic.jsonl")).unwrap();
        let result = summary_scan(&content);
        assert!(result.started_at.is_some());
        assert!(result.last_activity.is_some());
        assert_eq!(result.git_branch.as_deref(), Some("main"));
        assert_eq!(result.version.as_deref(), Some("2.1.71"));
        assert_eq!(result.turn_count, 1);
    }

    #[test]
    fn summary_scan_counts_turns() {
        let content = std::fs::read_to_string(fixture_path("session_linear.jsonl")).unwrap();
        let result = summary_scan(&content);
        assert_eq!(result.turn_count, 3);
    }

    #[test]
    fn summary_scan_empty_input() {
        let result = summary_scan("");
        assert!(result.started_at.is_none());
        assert!(result.last_activity.is_none());
        assert!(result.git_branch.is_none());
        assert!(result.version.is_none());
        assert_eq!(result.turn_count, 0);
        assert_eq!(result.total_tokens.total(), 0);
    }

    #[test]
    fn extract_last_prompt_returns_none_for_empty() {
        let prompt = extract_last_prompt("");
        assert!(prompt.is_none());
    }

    #[test]
    fn extract_last_prompt_returns_none_for_no_user_messages() {
        // Only an assistant line, no user or last-prompt records.
        let content = r#"{"type":"assistant","uuid":"a1","timestamp":"2026-03-08T10:00:00.000Z","message":{"model":"claude","content":[{"type":"text","text":"hi"}],"usage":{"input_tokens":0,"output_tokens":0}}}"#;
        let prompt = extract_last_prompt(content);
        assert!(prompt.is_none());
    }

    #[test]
    fn summary_scan_does_not_double_count_first_line() {
        // When the first line is a user record, peek reads it for metadata
        // but doesn't advance the iterator. The for loop then processes it
        // again. The turn_count and token totals must not be inflated.
        let user_line = r#"{"parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/home/user/project","sessionId":"sess-001","version":"2.1.71","gitBranch":"main","type":"user","message":{"role":"user","content":"Hello"},"uuid":"msg-001","timestamp":"2026-03-08T10:00:00.000Z"}"#;
        let assistant_line = r#"{"parentUuid":"msg-001","isSidechain":false,"userType":"external","cwd":"/home/user/project","sessionId":"sess-001","version":"2.1.71","gitBranch":"main","message":{"model":"claude-opus-4-6","id":"resp_001","type":"message","role":"assistant","content":[{"type":"text","text":"Hi there"}],"stop_reason":"end_turn","usage":{"input_tokens":100,"output_tokens":20,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}},"type":"assistant","uuid":"msg-002","timestamp":"2026-03-08T10:00:05.000Z"}"#;
        let content = format!("{}\n{}", user_line, assistant_line);
        let result = summary_scan(&content);
        assert_eq!(
            result.turn_count, 1,
            "first line user record should be counted exactly once"
        );
        assert_eq!(
            result.total_tokens.input_tokens, 100,
            "assistant tokens should be counted exactly once"
        );
        assert_eq!(
            result.total_tokens.output_tokens, 20,
            "assistant output tokens should be counted exactly once"
        );
    }

    #[test]
    fn summary_scan_single_line_sets_last_activity() {
        // A session with only one line should still have last_activity set.
        let content = r#"{"parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/home/user/project","sessionId":"sess-001","version":"2.1.71","gitBranch":"main","type":"user","message":{"role":"user","content":"Hello"},"uuid":"msg-001","timestamp":"2026-03-08T10:00:00.000Z"}"#;
        let result = summary_scan(content);
        assert!(result.started_at.is_some());
        assert!(
            result.last_activity.is_some(),
            "single-line session should have last_activity set"
        );
        assert_eq!(result.started_at, result.last_activity);
    }

    #[test]
    fn summary_scan_accumulates_tokens() {
        let content = std::fs::read_to_string(fixture_path("session_linear.jsonl")).unwrap();
        let result = summary_scan(&content);
        assert!(result.total_tokens.input_tokens > 0);
        assert!(result.total_tokens.output_tokens > 0);
    }
}
