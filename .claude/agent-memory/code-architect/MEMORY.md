# Claude Seer - Code Architect Memory

## Project Overview
TUI app for visualizing Claude Code JSONL session logs. Rust + ratatui.

## Key Files
- `docs/DATA_FLOW.md` - Full system architecture & data flow design
- `docs/ROADMAP.md` - Feature roadmap with release plan
- `Cargo.toml` - Dependencies: ratatui, crossterm, thiserror, miette, tracing
- Edition 2024 Rust

## Architecture Decisions
1. **Three-layer separation**: `data/` (no TUI deps) -> `app.rs` (no TUI deps) -> `tui/`
2. **Turn-based model**: Flat JSONL assembled into user/assistant Turn pairs
3. **DataSource trait**: In `source/mod.rs`, abstracts FS for testing
4. **Explicit side effects**: `AppState::handle_action()` is pure, returns `SideEffect` enum
5. **Two-pass parsing**: Summary scan for list, full parse on open
6. **std::thread + mpsc**: No tokio until SSH in v0.4
7. **Newtype IDs**: `SessionId`, `MessageId`, `ProjectPath` prevent type confusion
8. **thiserror per module, miette at boundary only**

## JSONL Format (from ~/.claude/projects/)
- Record types: user, assistant, progress, system, file-history-snapshot, last-prompt, queue-operation
- Assistant content blocks: text, thinking, tool_use
- Tool names: Read, Edit, Write, Bash, Glob, Grep, WebSearch, WebFetch, Agent, etc.
- Token usage: input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens
- Subagents stored in `<session-uuid>/subagents/agent-<id>.jsonl`
- Tool results stored in `<session-uuid>/tool-results/<id>.txt`

## User Preferences
- TDD mandatory - all non-TUI code must be testable without terminal
- Modular design, small focused modules
- No fallbacks - they hide real failures
- Uses rstest for parameterized tests, criterion/divan for benchmarks
