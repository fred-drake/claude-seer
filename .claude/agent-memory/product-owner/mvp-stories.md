# MVP User Stories (v0.1)

## Implementation Order & Priority

The stories below are ordered by implementation sequence. Each
milestone builds on the previous one.

---

## Milestone 1: JSONL Parser (Foundation)

### Story 1.1: Parse Session Records
**Priority: MUST-HAVE**

As a developer using Claude Code,
I want the application to parse JSONL session log files,
So that my session data can be loaded and processed.

Acceptance criteria:
- [ ] Parses all record types: user, assistant, progress, system,
      file-history-snapshot, last-prompt, queue-operation
- [ ] Deserializes into strongly-typed Rust structs via serde
- [ ] Handles malformed lines gracefully (skip with warning, not crash)
- [ ] Streams records without loading entire file into memory
- [ ] Unit tests for each record type using fixture data
- [ ] Benchmark for parsing a ~1000 line JSONL file

Dependencies: None (pure data layer)
MVP scope: All record types parsed into structs
Full scope: Validated relationships between records (parentUuid chains)

### Story 1.2: Discover Sessions
**Priority: MUST-HAVE**

As a developer using Claude Code,
I want the application to discover all my session files,
So that I can see every project and session available.

Acceptance criteria:
- [ ] Scans ~/.claude/projects/ directory structure
- [ ] Decodes project path from directory name
- [ ] Extracts session metadata without parsing full file
      (first/last line for timestamps, file size)
- [ ] Groups sessions by project
- [ ] Configurable base path (not hardcoded to ~/.claude)
- [ ] Unit tests with mock directory structures

Dependencies: Story 1.1 (record parsing for metadata extraction)
MVP scope: List projects and sessions with basic metadata
Full scope: Watch for new sessions, real-time updates

### Story 1.3: Build Conversation Model
**Priority: MUST-HAVE**

As a developer using Claude Code,
I want session records assembled into a conversation model,
So that I can navigate the conversation as a sequence of turns.

Acceptance criteria:
- [ ] Groups records into turns (user message + assistant response)
- [ ] Preserves parent-child relationships (parentUuid threading)
- [ ] Extracts tool calls and their results per turn
- [ ] Calculates per-turn token usage from assistant message.usage
- [ ] Identifies sidechain conversations
- [ ] Unit tests for conversation assembly from fixture JSONL

Dependencies: Story 1.1
MVP scope: Linear conversation with turns and token counts
Full scope: Sidechain branching, subagent trees

---

## Milestone 2: Session List View

### Story 2.1: Project and Session List
**Priority: MUST-HAVE**

As a developer using Claude Code,
I want to see a list of my projects and their sessions,
So that I can find and select a session to examine.

Acceptance criteria:
- [ ] Two-level list: projects (top) -> sessions (nested)
- [ ] Each project shows: decoded path, session count
- [ ] Each session shows: date/time, message count, branch name
- [ ] Sorted by most recent session first
- [ ] Keyboard navigation: j/k or arrows to move, Enter to select
- [ ] Loading indicator while scanning directories

Dependencies: Story 1.2
MVP scope: Static list, select to open
Full scope: Filtering, search, favorites, sorting options

---

## Milestone 3: Conversation Viewer

### Story 3.1: Message Display
**Priority: MUST-HAVE**

As a developer using Claude Code,
I want to read the conversation messages in a selected session,
So that I can review what was discussed and what actions were taken.

Acceptance criteria:
- [ ] Displays user and assistant messages in chronological order
- [ ] Visual distinction between user and assistant messages
      (color, prefix, or layout)
- [ ] Shows tool call names inline (e.g. "[Read] /path/to/file")
- [ ] Scrollable with j/k, Page Up/Down, Home/End
- [ ] Wraps long lines to terminal width
- [ ] Press Escape or q to return to session list

Dependencies: Story 1.3, Story 2.1
MVP scope: Plain text display with basic formatting
Full scope: Syntax highlighting, expandable tool calls,
           markdown rendering

### Story 3.2: Turn Navigation
**Priority: SHOULD-HAVE**

As a developer using Claude Code,
I want to jump between turns in a conversation,
So that I can quickly find specific exchanges.

Acceptance criteria:
- [ ] n/N keys to jump to next/previous turn boundary
- [ ] Turn number indicator (e.g. "Turn 5 of 23")
- [ ] Turn boundaries visually marked (horizontal rule or separator)

Dependencies: Story 3.1
MVP scope: Jump between turns
Full scope: Turn summary sidebar, turn search

---

## Milestone 4: Token Usage Display

### Story 4.1: Per-Turn Token Summary
**Priority: MUST-HAVE**

As a developer using Claude Code,
I want to see token usage for each turn,
So that I can understand where my tokens are being spent.

Acceptance criteria:
- [ ] Shows input_tokens and output_tokens per assistant response
- [ ] Shows cache hit/miss ratio (cache_read vs cache_creation)
- [ ] Displays in a compact format alongside or below each turn
- [ ] Cumulative running total as conversation progresses

Dependencies: Story 1.3, Story 3.1
MVP scope: Numeric display per turn
Full scope: Bar charts, category breakdown, cost estimation

### Story 4.2: Session Token Summary
**Priority: SHOULD-HAVE**

As a developer using Claude Code,
I want to see aggregate token usage for a session,
So that I can understand the overall cost of a session.

Acceptance criteria:
- [ ] Total input and output tokens for the session
- [ ] Total cache creation and cache read tokens
- [ ] Displayed in a header/footer bar of the conversation view
- [ ] Optionally visible in session list as a column

Dependencies: Story 4.1
MVP scope: Totals in conversation view header
Full scope: Cost estimation in dollars, comparison across sessions

---

## Milestone 5: Application Shell

### Story 5.1: Navigation Framework
**Priority: MUST-HAVE**

As a developer using Claude Code,
I want consistent keyboard navigation throughout the app,
So that the interface feels natural and discoverable.

Acceptance criteria:
- [ ] Vim-style navigation (h/j/k/l, g/G for top/bottom)
- [ ] Tab or bracket keys to switch between panes/views
- [ ] ? key opens help overlay showing all keybindings
- [ ] q quits from any view (with confirmation if needed)
- [ ] Breadcrumb or title bar showing current location
- [ ] Status bar with context information

Dependencies: All previous milestones
MVP scope: Basic navigation between list and detail views
Full scope: Command palette, customizable keybindings

### Story 5.2: Error Handling and Logging
**Priority: MUST-HAVE**

As a developer using Claude Code,
I want the application to handle errors gracefully,
So that corrupt data or missing files do not crash the app.

Acceptance criteria:
- [ ] Malformed JSONL lines are skipped with a logged warning
- [ ] Missing directories show an informative message
- [ ] File permission errors are reported, not panicked
- [ ] Debug logging via RUST_LOG environment variable
- [ ] Log output to file (not terminal, since TUI owns stdout)

Dependencies: None (cross-cutting, built throughout)
MVP scope: No panics on bad data, log to file
Full scope: Error recovery, retry logic, error panel in TUI

---

## Deferred to v0.2+

These features are explicitly OUT of MVP scope:

- **Token attribution by category** (CLAUDE.md, skills, @-mentions,
  tool I/O, thinking, teams, user text) - requires deeper parsing
  of system prompts and content analysis
- **Compaction visualization** - no explicit compaction record found
  in JSONL format; needs heuristic detection
- **Tool call detail inspector** - syntax highlighting, inline diffs
- **Cross-session search** - requires indexing infrastructure
- **Subagent tree visualization** - complex rendering
- **Custom notification triggers** - regex engine, configuration UI
- **SSH remote access** - networking layer
- **Multi-pane comparison** - complex layout management
- **Cost estimation in dollars** - requires pricing data maintenance

---

## Testing Strategy

### Unit Tests (Mandatory, TDD)
All tests live in `#[cfg(test)]` modules within the source files
they test. No separate integration test binary.

**Synthetic fixtures** (`tests/fixtures/`):
- Parser: fixture JSONL -> parsed structs
- Session discovery: mock filesystem via DataSource trait
- Conversation model: fixture data -> turn assembly
- Token calculations: known inputs -> expected outputs
- Error handling: malformed lines, missing fields, empty files

**Real fixtures** (`tests/fixtures/real/`):
- Anonymized extracts from actual Claude Code sessions
- Full session parsing end-to-end (same unit test modules)
- Conversation model from multi-hundred-line sessions
- Parameterized via rstest to cover variety of session types
- Cover: short/long sessions, many tool calls, subagents,
  sidechains, malformed lines, version differences

**Rationale**: No database, network, or external service means
there is no meaningful integration boundary. Unit tests with
real data provide equivalent coverage without the overhead of
a separate test category. Revisit if v0.4 adds networking.

### TUI Tests (Deprioritized)
- Manual testing for v0.1
- Consider ratatui test backend for widget rendering tests
- E2E TUI testing frameworks evaluated for v0.2

### Benchmarks
- JSONL parsing throughput (criterion)
- Session discovery scan time (criterion)
- Memory usage for large sessions (dhat)
- Rendering frame time for conversation view
