# Claude Seer - Version Log

Last Updated: 2026-03-08 | Version: 0.1.0-dev (M2)

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

### Milestone 2: Session Discovery & Listing with Empty States

Implemented the TUI application shell with session browsing (29 new
tests, 117 total):

- **app.rs**: Pure state machine with `Action`/`SideEffect` enums.
  `AppState::handle_action()` handles Quit, NavigateUp/Down,
  SelectSession, BackToList, Resize, SessionsLoaded, LoadError,
  ToggleHelp. Sessions sorted by last_activity descending and
  grouped by ProjectPath.
- **tui/event.rs**: Event loop with crossterm + mpsc channels.
  `map_key_to_action()` maps j/k/Up/Down, Enter, Esc, q, ?, Ctrl-C.
- **tui/ui.rs**: Root layout dispatcher (content area + status bar).
- **tui/widgets/session_list.rs**: Session browser with project
  headers and nested session entries showing date, turn count,
  branch, and last prompt.
- **tui/widgets/status_bar.rs**: Bottom bar with session count and
  help hint.
- **tui/widgets/help.rs**: Centered help overlay toggled with ?.
- **tui/widgets/empty_state.rs**: Four empty states:
  Loading, NoDirectory (guidance), NoSessions (guidance),
  EmptySession.
- **tui/mod.rs**: Terminal setup/teardown wrapper with Drop impl.
- **main.rs**: CLI args (--path, --log-file) via clap, background
  session loading via std::thread + mpsc, full event loop.
