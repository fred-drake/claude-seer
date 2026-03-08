# Claude Seer - System Architecture & Data Flow

Last Updated: 2026-03-08 | Version: 0.1.0-planning

## Module Structure

```
claude-seer/
  src/
    main.rs                  -- Entry point: CLI args, terminal setup, run loop
    lib.rs                   -- Re-exports public API for integration tests
    app.rs                   -- Application state machine (no TUI deps)

    data/
      mod.rs                 -- Re-exports
      model.rs               -- Core domain types (Session, Turn, Message, etc.)
      parser.rs              -- JSONL line parsing into raw records
      session_loader.rs      -- Loads/indexes session files from disk
      token_attribution.rs   -- Token categorization (7 categories)
      compaction.rs          -- Compaction event detection
      search.rs              -- Full-text and regex search across sessions
      error.rs               -- DataError (thiserror)

    source/
      mod.rs                 -- Re-exports, DataSource trait
      filesystem.rs          -- Local filesystem impl of DataSource
      error.rs               -- SourceError (thiserror)

    tui/
      mod.rs                 -- Re-exports, terminal setup/teardown
      event.rs               -- Event loop: crossterm events + app channels
      ui.rs                  -- Root layout dispatcher
      widgets/
        mod.rs
        session_list.rs      -- Session browser pane
        conversation.rs      -- Message/turn viewer pane
        token_chart.rs       -- Token usage visualization
        tool_detail.rs       -- Tool call inspector
        status_bar.rs        -- Status bar / command palette
        help.rs              -- Help overlay
      error.rs               -- TuiError (thiserror)
```

## Layer Separation

```
+-----------------------------------------------------+
|                     main.rs                          |
|  CLI parse -> config -> init tracing -> run          |
+-----------------------------------------------------+
         |                          |
         v                          v
+------------------+    +----------------------+
|    app.rs        |    |    tui/               |
|  AppState        |    |  Terminal, EventLoop  |
|  handle_action() |<---|  render()             |
|  (pure logic)    |--->|  (display only)       |
+------------------+    +----------------------+
         |
         v
+------------------+    +----------------------+
|    data/          |    |    source/            |
|  Session, Turn   |<---|  DataSource trait     |
|  Parser, Search  |    |  FilesystemSource     |
|  TokenAttribution|    +----------------------+
+------------------+
```

### The Three Laws of Layer Separation

1. **data/** has ZERO dependencies on `ratatui`, `crossterm`, or `tui/`.
   It is a pure library of domain types, parsing, and analysis.

2. **app.rs** has ZERO dependencies on `ratatui` or `crossterm`.
   It owns `AppState` and processes `Action` enums. It returns
   `ViewState` structs that the TUI reads. All business logic
   (navigation, filtering, selection) lives here.

3. **tui/** depends on `app` and `data` for reading state, but never
   mutates domain data directly. It translates crossterm `Event` into
   `Action`, calls `app.handle_action()`, then renders from the
   resulting state.

This makes `data/` and `app.rs` fully testable without a terminal.

## Core Data Model

### Domain Types (data/model.rs)

```rust
use std::path::PathBuf;

/// Newtype wrappers to prevent mixing IDs at compile time.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectPath(pub PathBuf);

/// A parsed session with all its turns.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub project: ProjectPath,
    pub file_path: PathBuf,
    pub version: Option<String>,
    pub git_branch: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
    pub last_prompt: Option<String>,
    pub turns: Vec<Turn>,
    pub token_totals: TokenUsage,
    pub parse_warnings: Vec<ParseWarning>,
}

/// A turn is one user message + one assistant response.
/// This is the primary unit of conversation navigation.
#[derive(Debug, Clone)]
pub struct Turn {
    pub index: usize,
    pub user_message: UserMessage,
    pub assistant_response: Option<AssistantResponse>,
    /// Populated by correlating `system` records containing
    /// `turn_duration` with their parent turn during pass 2.
    /// The turn assembler skips `system` records, but the
    /// session loader post-processes them to fill this field.
    pub duration: Option<std::time::Duration>,
    pub is_complete: bool,
    pub events: Vec<SessionEvent>,
}

#[derive(Debug, Clone)]
pub struct UserMessage {
    pub id: MessageId,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub content: UserContent,
}

#[derive(Debug, Clone)]
pub enum UserContent {
    Text(String),
    ToolResults(Vec<ToolResult>),
    Mixed { text: String, tool_results: Vec<ToolResult> },
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone)]
pub struct AssistantResponse {
    pub id: MessageId,
    pub model: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub content_blocks: Vec<ContentBlock>,
    pub usage: TokenUsage,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text(String),
    Thinking { text: String },
    ToolUse(ToolCall),
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: ToolName,
    pub input: serde_json::Value,
    /// Populated later by correlating with tool_result
    pub result: Option<ToolResult>,
}

/// Strongly typed tool names for pattern matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolName {
    Read,
    Edit,
    Write,
    Bash,
    Glob,
    Grep,
    WebSearch,
    WebFetch,
    Agent,
    TodoRead,
    TodoWrite,
    Other(String),
}

/// Token usage with cache breakdown.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
}

/// The 7 attribution categories for context window analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenCategory {
    SystemPrompt,
    UserMessage,
    AssistantText,
    Thinking,
    ToolInput,
    ToolOutput,
    CacheRead,
}

#[derive(Debug, Clone)]
pub struct TokenAttribution {
    pub by_category: std::collections::HashMap<TokenCategory, u64>,
    pub total: u64,
}

/// Progress and system events that are not part of the
/// conversation but are useful for visualization.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    HookProgress {
        hook_name: String,
        command: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    TurnDuration {
        duration_ms: u64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    AgentSpawn {
        agent_id: String,
        agent_type: String,
        prompt: String,
        parent_tool_use_id: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    CompactionDetected {
        turn_index: usize,
        tokens_before: u64,
        tokens_after: u64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    QueueOperation {
        operation: String,
        content: Option<String>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// Summary for session list display (avoids loading full session).
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: SessionId,
    pub project: ProjectPath,
    pub file_path: PathBuf,
    pub file_size: u64,
    pub last_prompt: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
    pub turn_count: usize,
    pub total_tokens: TokenUsage,
    pub git_branch: Option<String>,
}
```

### Why Turns Instead of Flat Messages

The JSONL is a flat stream, but users think in turns (I asked, Claude
answered). Reconstructing turns during parsing means:
- Navigation is `Turn`-based (up/down moves between turns)
- Token attribution is per-turn (each turn has a clear context window)
- Compaction is detected between turns (token count drops)
- The TUI never needs to reconstruct turn boundaries

### Why Newtype IDs

`SessionId`, `MessageId`, and `ProjectPath` are newtypes. Without them,
it is trivially easy to pass a session UUID where a message UUID is
expected. The compiler catches this.

## DataSource Trait (source/mod.rs)

```rust
/// Abstraction over where session data comes from.
/// This is the key trait for testability -- tests provide
/// an in-memory implementation, production uses filesystem.
pub trait DataSource: Send + Sync {
    /// List all available session summaries.
    /// Returns lightweight metadata without full parsing.
    fn list_sessions(&self) -> Result<Vec<SessionSummary>, SourceError>;

    /// Load and fully parse a single session.
    fn load_session(&self, id: &SessionId) -> Result<Session, SourceError>;

    /// Load subagent data for a session.
    fn load_subagents(
        &self,
        id: &SessionId,
    ) -> Result<Vec<SubagentSession>, SourceError>;

    /// Stream raw lines for search (avoids full parse).
    fn search_raw(
        &self,
        query: &SearchQuery,
    ) -> Result<Vec<SearchHit>, SourceError>;
}
```

### FilesystemSource

The production implementation discovers `~/.claude/projects/` structure:
```
~/.claude/projects/
  <project-path-slug>/
    <session-uuid>.jsonl
    <session-uuid>/
      subagents/
        agent-<id>.jsonl
        agent-<id>.meta.json
      tool-results/
        <id>.txt
```

It scans directories on startup, builds a `SessionSummary` index from
file metadata + a quick first/last-line parse (for timestamps and
last-prompt), and loads full sessions on demand.

### TestSource

For unit tests, a `TestSource` holds `HashMap<SessionId, Vec<String>>`
of raw JSONL lines. This lets tests construct specific scenarios
(compaction events, subagent trees, error cases) without touching
the filesystem.

## Application State Machine (app.rs)

As implemented through M3, the state machine handles session discovery,
navigation, empty state management, and conversation viewing with turn
navigation. Future milestones will add token display (M4) and search.

```rust
/// All possible user/system actions.
pub enum Action {
    Quit,
    NavigateUp,
    NavigateDown,
    SelectSession,                         // Uses selected_index
    BackToList,
    NextTurn,                              // Jump to next turn (M3)
    PrevTurn,                              // Jump to previous turn (M3)
    Resize(u16, u16),
    SessionsLoaded(Vec<SessionSummary>),   // Background result
    SessionLoaded(Box<Session>),           // Session loaded (M3)
    SessionLoadError(String),              // Session load failed (M3)
    LoadError(String),                     // Background error
    ToggleHelp,
    // Future: StartSearch, etc.
}

/// The view the TUI should render.
pub enum View {
    SessionList,
    Conversation(SessionId),
    // Future: ToolDetail, Search
}

/// Empty state conditions for display.
pub enum EmptyState {
    NoDirectory,    // No ~/.claude/ directory
    NoSessions,     // Directory exists, no sessions
    Loading,        // Session list or session loading
    EmptySession,   // Selected session has 0 turns
}

/// Side effects that the caller must execute.
pub enum SideEffect {
    Exit,
    LoadSessionList,
    LoadSession(SessionId),
    // Future: PerformSearch(SearchQuery)
}

pub struct AppState {
    pub view: View,
    pub empty_state: Option<EmptyState>,
    pub sessions: Vec<SessionSummary>,
    pub selected_index: usize,
    pub show_help: bool,
    pub terminal_size: (u16, u16),
    pub current_session: Option<Session>,
    pub current_turn_index: usize,
    pub scroll_offset: usize,
}
```

Key behaviors:
- Sessions are sorted by `last_activity` descending (most recent first)
- `grouped_sessions()` groups by `ProjectPath` for display
- `BackToList` from `SessionList` view triggers `Exit`
- `SessionsLoaded` with empty vec sets `NoSessions` empty state
- `LoadError` sets `NoDirectory` empty state
- `SelectSession` transitions to `Conversation` view with `Loading`
  empty state and returns `LoadSession` side effect
- `SessionLoaded` stores the session and clears loading state
- `SessionLoaded` with 0 turns sets `EmptySession` empty state
- `BackToList` from `Conversation` clears session state
- `NextTurn`/`PrevTurn` navigate between turns, resetting scroll
- `NavigateDown`/`NavigateUp` in `Conversation` view scroll content

### Why Side Effects Are Explicit

`AppState::handle_action()` is a pure function -- given state + action,
it produces new state + optional side effect. This means:
- Every state transition is testable with `assert_eq!`
- No mocking needed for navigation, selection, scrolling
- I/O (file loading, search) is triggered by the caller, not by app.rs
- The TUI event loop is the only place that calls DataSource methods

## Event Architecture (tui/event.rs)

```rust
pub enum AppEvent {
    /// Crossterm terminal event (key press, resize, etc.)
    Terminal(crossterm::event::Event),
    /// Session list loaded in background.
    SessionsLoaded(Result<Vec<SessionSummary>, SourceError>),
    /// Single session loaded in background (M3).
    SessionLoaded(Result<Session, SourceError>),
    /// Tick for any periodic updates (optional)
    Tick,
    // Future: SearchComplete(Vec<SearchHit>),
}
```

### Event Loop

```
                    +-------------------+
                    | crossterm::event  |
                    | ::read()          |
                    +--------+----------+
                             |
                             v
+-------------+     +--------+---------+     +----------------+
| Background  |---->| mpsc::Receiver   |---->| Main Loop      |
| I/O threads |     | (AppEvent)       |     | 1. recv event  |
+-------------+     +------------------+     | 2. map to Action
                                             | 3. app.handle  |
                                             | 4. exec side fx|
                                             | 5. terminal    |
                                             |    .draw(|f| { |
                                             |      render()  |
                                             |    })          |
                                             +----------------+
```

The main loop is single-threaded for rendering. Background I/O
(session loading, search) runs on `std::thread` and sends results
back through an `mpsc::channel`. This keeps the TUI responsive
during large file parsing.

We use `std::thread` + `mpsc` rather than `tokio` because:
- We have no network I/O yet (SSH is v0.4)
- File I/O is the only async need, and `spawn_blocking` adds
  unnecessary complexity
- crossterm's event polling is synchronous
- When SSH arrives in v0.4, we can add tokio then without
  restructuring -- the channel-based event architecture stays the same

## Error Handling Strategy

### Per-Module thiserror

Each module defines its own error type:

```rust
// data/error.rs
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("failed to parse JSONL at line {line}: {reason}")]
    ParseError { line: usize, reason: String },

    #[error("missing required field '{field}' in record type '{record_type}'")]
    MissingField { field: String, record_type: String },

    #[error("unknown record type: {0}")]
    UnknownRecordType(String),
}

// source/error.rs
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("session not found: {0}")]
    SessionNotFound(SessionId),

    #[error("failed to read session file")]
    IoError(#[from] std::io::Error),

    #[error("data parsing failed")]
    DataError(#[from] DataError),
}

// tui/error.rs
#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("terminal I/O error")]
    IoError(#[from] std::io::Error),

    #[error("data source error")]
    SourceError(#[from] SourceError),
}
```

### miette at the Boundary

`main.rs` wraps the top-level error in miette for user-facing display:

```rust
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("claude-seer encountered an error")]
pub enum AppError {
    #[error("terminal error")]
    Tui(#[from] TuiError),

    #[error("data source error")]
    #[diagnostic(help("Check that ~/.claude/projects/ exists and contains session files"))]
    Source(#[from] SourceError),
}
```

This keeps miette out of library code. Only the binary crate
imports miette.

## JSONL Parsing Strategy (data/parser.rs)

### Two-Pass Design

**Pass 1 - Summary scan** (`session_loader.rs`):
Read only the first line (for session start time), the last line
(for last activity), and any `last-prompt` line. This gives enough
metadata for the session list without parsing every record.

Note: `last_prompt` extraction happens entirely in pass 1. The turn
assembler does NOT process `last-prompt` records — they are filtered
out before reaching the assembler. The `SessionSummary.last_prompt`
field is populated during the summary scan. When a full session is
loaded in pass 2, the caller copies `SessionSummary.last_prompt` into
`Session.last_prompt` — pass 2 does not re-extract it.

**Pass 2 - Full parse** (`parser.rs`):
When a user opens a session, parse every line into `RawRecord`,
then assemble into `Turn` objects. This is where tool results
get correlated with tool calls (via `tool_use_id`).

```rust
/// Raw deserialized JSONL record before domain mapping.
/// Uses serde_json::Value for flexibility -- the JSONL
/// format evolves with Claude Code versions.
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
    // Catch-all for fields we haven't typed yet
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
```

### Why `serde_json::Value` for message content

The Claude Code JSONL format is not stable -- it adds fields across
versions and content block schemas vary. Strongly typing every field
would create a maintenance burden. Instead:

- `RawRecord` captures the envelope (type, uuid, timestamp) with
  concrete types
- `message` stays as `Value` and gets destructured in the domain
  mapping layer
- Unknown fields are captured in `extra` rather than rejected

This means we never fail on a newer JSONL format -- we just might
not extract every field from it.

## Compaction Detection (data/compaction.rs)

Compaction is detected heuristically by watching total context
window size across turns. When `input_tokens + cache_read_tokens +
cache_creation_tokens` drops significantly between consecutive
assistant responses, a compaction occurred.

```rust
pub fn detect_compactions(turns: &[Turn]) -> Vec<CompactionEvent> {
    let mut events = vec![];
    let mut prev_total = 0u64;

    for (i, turn) in turns.iter().enumerate() {
        let Some(ref response) = turn.assistant_response else {
            continue;
        };
        let usage = &response.usage;
        let total = usage.input_tokens
            + usage.cache_read_tokens
            + usage.cache_creation_tokens;

        if i > 0 && total < prev_total.saturating_mul(70) / 100 {
            // >30% drop signals compaction
            events.push(CompactionEvent {
                turn_index: i,
                tokens_before: prev_total,
                tokens_after: total,
                timestamp: response.timestamp,
            });
        }
        prev_total = total;
    }
    events
}
```

## Cross-Cutting Concerns

### Tracing

`tracing` spans wrap key operations:
- `parse_session` span with session_id field
- `load_sessions` span with project path
- `search` span with query

Tracing output goes to a file (not stdout, since that is the TUI).
Configure via `RUST_LOG` and `CLAUDE_SEER_LOG_FILE`.

### Testing Strategy

| Layer | Test type | How |
|-------|-----------|-----|
| `data/parser.rs` | Unit | Feed known JSONL strings, assert `RawRecord` |
| `data/model.rs` | Unit | Builder helpers, assert field access |
| `data/token_attribution.rs` | Unit | Given turns, assert category breakdown |
| `data/compaction.rs` | Unit | Given turns with token usage, assert events |
| `data/search.rs` | Unit | Given sessions, assert search hits |
| `source/filesystem.rs` | Integration | Tempdir with fixture JSONL files |
| `app.rs` | Unit | Construct AppState, fire actions, assert state |
| `tui/` | Manual | Cannot unit test ratatui rendering easily |

Test fixtures live in `tests/fixtures/` as `.jsonl` files containing
hand-crafted minimal scenarios.

### Performance Considerations

- Session list uses summary-only scan (no full parse)
- Full session parse is lazy (only when opened)
- Large sessions stream-parse line by line (no full file in memory)
- Token attribution computed once on load, cached in `Session`
- Search uses rayon for parallel file scanning (future optimization)
- Widget rendering avoids allocation: use `Line::from()` with
  borrowed `&str` where possible

## Data Flow: User Opens a Session

```
1. User presses Enter on session list
2. tui/event.rs maps KeyCode::Enter -> Action::SelectSession
3. app.handle_action() transitions to View::Conversation with Loading
   state and returns SideEffect::LoadSession(id)
4. Main loop spawns std::thread to call data_source.load_session(id)
5. TUI renders Loading empty state while thread works
6. Thread parses JSONL -> Vec<RawRecord>
7. Thread assembles RawRecord stream into Session with Turns
8. Thread sends AppEvent::SessionLoaded(Ok(session)) via channel
9. Main loop receives event, maps to Action::SessionLoaded(Box<Session>)
10. app.handle_action() stores session, clears loading state
11. Next render cycle: tui draws conversation view from
    app.current_session with turn navigation (n/N) and scrolling (j/k)
```

## Data Flow: Cross-Session Search

```
1. User types /search <query>
2. tui/event.rs -> Action::StartSearch(query)
3. app.handle_action() returns SideEffect::PerformSearch(query)
4. Main loop spawns thread: data_source.search_raw(query)
5. search_raw scans all JSONL files, returns matching lines with context
6. Thread sends AppEvent::SearchComplete(hits)
7. app.set_search_results(hits)
8. TUI renders search results with highlighted matches
```

## CLI Arguments (main.rs)

Argument parsing uses `clap` with the derive API. The CLI struct lives
in `src/main.rs` and is parsed before any other setup.

```rust
use clap::Parser;
use std::path::PathBuf;

/// claude-seer: TUI for visualizing Claude Code session data
#[derive(Parser, Debug)]
#[command(name = "claude-seer", version, about)]
pub struct Cli {
    /// Path to the Claude projects directory
    /// [default: ~/.claude/projects/]
    #[arg(long, short = 'p')]
    pub path: Option<PathBuf>,

    /// Path to the log file for tracing output
    /// [default: /tmp/claude-seer.log]
    #[arg(long)]
    pub log_file: Option<PathBuf>,
}
```

### Arguments

| Argument | Short | Type | Default | Description |
|---|---|---|---|---|
| `--path <PATH>` | `-p` | `PathBuf` | `~/.claude/projects/` | Override the session data directory |
| `--log-file <PATH>` | — | `PathBuf` | `/tmp/claude-seer.log` | Override the tracing log file location |
| `--help` | `-h` | flag | — | Display help text (provided by clap) |
| `--version` | `-V` | flag | — | Display version (provided by clap) |

### Resolution Order

Configuration values are resolved in this order (later wins):

1. Compiled defaults (`~/.claude/projects/`, `/tmp/claude-seer.log`)
2. CLI arguments (`--path`, `--log-file`)

### Usage in main.rs

```rust
fn main() -> miette::Result<()> {
    let cli = Cli::parse();

    let projects_path = match cli.path {
        Some(path) => path,
        None => dirs::home_dir()
            .ok_or_else(|| miette::miette!("could not determine home directory"))?
            .join(".claude/projects"),
    };

    let log_file_path = cli.log_file
        .unwrap_or_else(|| PathBuf::from("/tmp/claude-seer.log"));

    // Initialize tracing — write to log file if possible,
    // fall back to stderr.
    match std::fs::File::create(&log_file_path) {
        Ok(file) => {
            tracing_subscriber::fmt()
                .with_writer(file)
                .with_ansi(false)
                .init();
        }
        Err(_) => {
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .init();
        }
    }

    // ... create FilesystemSource with projects_path, etc.
}
```
