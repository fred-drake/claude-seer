# Claude Seer - Version Log

Last Updated: 2026-03-08 | Version: 0.1.0-dev

## v0.1.0-dev (2026-03-08)

### Milestone 1: JSONL Parser Library & Data Layer

Implemented the complete data layer with 49 tests:

- **Error types**: `DataError` (3 variants) and `SourceError` (3 variants)
  with `thiserror` derives
- **Domain model**: Newtype IDs (`SessionId`, `MessageId`, `ProjectPath`),
  `Session`, `Turn`, `UserMessage`, `AssistantResponse`, `ContentBlock`,
  `ToolCall`, `ToolName` (11 known + `Other`), `TokenUsage`, `ParseWarning`,
  `SessionSummary`, and event types
- **JSONL parser**: `RawRecord` deserialization with `serde_json::Value`
  for the message body; domain mapping functions for user messages,
  assistant responses, tool results, and token usage
- **Turn assembly state machine**: Three-state assembler
  (`AwaitingUser` / `HaveUser` / `HavePair`) that handles normal pairs,
  tool result interleaving, consecutive users, orphaned records,
  sidechain filtering, incomplete turns, and malformed lines
- **Two-pass parsing**: `summary_scan()` for fast session list metadata,
  `extract_last_prompt()` for session titles, `load_session_from_str()`
  for full parse
- **DataSource trait**: `list_sessions()` + `load_session()` abstraction
- **FilesystemSource**: Production implementation scanning
  `~/.claude/projects/` directory structure
