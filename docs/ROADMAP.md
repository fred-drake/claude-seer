# Claude Seer - Product Roadmap

Last Updated: 2026-03-09 | Version: 0.1.1 (chat-style conversation redesign)

## Vision

Claude Seer is a TUI application that lets developers visualize and
explore their Claude Code session data directly in the terminal. It reads
the JSONL session logs that Claude Code writes to `~/.claude/projects/`
and presents them through an interactive, keyboard-driven interface.

The reference product is [claude-devtools](https://github.com/matt1398/claude-devtools),
a desktop/web application. Claude Seer brings the same insights to the
terminal, where Claude Code developers already work.

## Architecture

- **Stack**: Rust + ratatui + crossterm + thiserror + miette + tracing
- **Data source**: JSONL files in `~/.claude/projects/`
- **Testing**: TDD-first, cargo-nextest, rstest, criterion/divan benchmarks
- **Profiling**: cargo-flamegraph, samply, dhat
- **Detailed design**: See [DATA_FLOW.md](./DATA_FLOW.md)

### Module Layout

| Module | Purpose | TUI dependency? |
|--------|---------|-----------------|
| `data/` | Domain types, JSONL parser, search, analysis | No |
| `source/` | DataSource trait + filesystem impl | No |
| `app.rs` | State machine, action handling | No |
| `tui/` | Terminal, event loop, widgets | Yes |
| `main.rs` | Entry point, CLI, wiring | Yes |

### Key Design Decisions

1. **Turn-based model**: Raw JSONL is flat, but we reconstruct
   user/assistant turn pairs during parsing. All navigation
   and analysis operates on turns.
2. **Explicit side effects**: `AppState::handle_action()` is pure.
   I/O requests are returned as `SideEffect` values for the caller.
3. **DataSource trait**: Abstracts filesystem access for testability.
4. **Two-pass parsing**: Summary scan for the session list (fast),
   full parse only when a session is opened.
5. **std::thread + mpsc**: No async runtime. Background I/O sends
   results through channels to keep the TUI responsive.

## JSONL Data Format

Each session is a `.jsonl` file named by session UUID. Records have these
`type` values:

| Type | Description |
|------|-------------|
| `user` | User messages (text, tool results) |
| `assistant` | Assistant messages (text, thinking, tool_use) |
| `progress` | Hook progress, bash progress, agent progress |
| `system` | Hook summaries, turn durations |
| `file-history-snapshot` | File state snapshots |
| `last-prompt` | Last user prompt in session |
| `queue-operation` | Queue operations |

Key fields per record: `sessionId`, `uuid`, `parentUuid`, `timestamp`,
`cwd`, `gitBranch`, `version`, `isSidechain`.

Assistant messages contain `message.usage` with token counts:
`input_tokens`, `output_tokens`, `cache_creation_input_tokens`,
`cache_read_input_tokens`.

Assistant content blocks have types: `text`, `thinking`, `tool_use`.
Tool use blocks contain `name` and `input` fields.

## Pre-Implementation Prerequisites

Before Milestone 1 begins, these items must be completed:

1. **Turn assembly state machine diagram** — Formal state diagram for
   `TurnAssemblerState` covering: normal user/assistant pairs, tool result
   interleaving, progress record attachment, orphaned messages, sidechain
   conversations, and incomplete turns. This is a spec, not post-hoc docs.
2. **Add `serde` and `serde_json` to Cargo.toml** — Required for all
   parsing work.
3. **Create missing test fixtures** — `session_linear.jsonl`,
   `session_sidechain.jsonl`, `session_resumed.jsonl`,
   `session_orphaned_progress.jsonl`, `session_mid_toolcall.jsonl`,
   `session_consecutive_users.jsonl`, `session_mismatched_toolresult.jsonl`.
4. **Reconcile module naming** — TESTING_STRATEGY.md must match the
   canonical module layout in DATA_FLOW.md.
5. **Define CLI args** — At minimum: `--path`, `--help`, `--version`.

## Release Plan

### v0.1 - MVP: Session Browser + Conversation Viewer

**What you can do:** Launch `claude-seer` in your terminal and browse all
your Claude Code projects and sessions. Select a session to read the full
conversation — user messages, assistant responses, and tool call names are
displayed in a scrollable, color-coded view. Each turn shows token usage
(input, output, cache read/write), and a running cumulative total tracks
spend across the conversation. A help overlay (`?`) lists all keybindings.
If no Claude Code data exists, you see a helpful guidance message.

**How it works in practice:** You open a terminal, run `claude-seer`, and
see a project list. Projects are decoded from `~/.claude/projects/` directory
names and sorted by most recent activity, with the CWD project highlighted
first. Each project shows session count and relative time. Press `Enter` to
open a project and see its sessions — each with date/time, message count,
branch name, and first prompt. Press `Enter` again to open a session, and
the view fills with the conversation. You scroll through messages, jump
between turns with `n`/`N`, and see token counts alongside each assistant
response. Press `Esc` to navigate back through the hierarchy. Press `q` to
out or quit. Use `--path /custom/path` for non-standard installs.

**Milestones:**
1. JSONL parser library — data layer with error type skeletons
   (`DataError`, `SourceError` via thiserror), no TUI. Turn assembly
   state machine handles edge cases (sidechains skipped in v0.1,
   progress records attached to nearest turn, orphaned messages warned
   and skipped).
2. Session discovery and listing — includes empty state handling:
   - No `~/.claude/` directory: full-screen guidance message
   - Directory exists, no sessions: "(empty)" with guidance
   - Session loading: "Loading session..." text
   - Empty session (0 turns): "Session contains no conversation turns"
3. Conversation viewer with message display and turn navigation
   (`n`/`N` to jump turns — MUST-HAVE, not optional)
4. Token usage display (per-turn and cumulative)
5. Application shell (navigation, help overlay, logging to file)

**What is NOT in v0.1:**
- No tool call detail views — you see tool names (e.g. `[Read]`, `[Bash]`)
  but cannot inspect inputs, outputs, or diffs
- No syntax highlighting of code within messages
- No search — you browse and scroll only
- No token attribution by category — you see raw totals, not a breakdown
  of what contributed to the context window
- No compaction detection — token drops between turns are not flagged
- No subagent/team visualization — subagent tool calls appear as regular
  tool calls without tree structure
- No SSH remote access — local `~/.claude/` only
- No rich error diagnostics (miette) — basic error/warning messages
  appear in the status bar, but miette-style rich context (source
  snippets, help text, related errors) is not exposed to the user

---

### v0.1.1 - Chat-Style Conversation Redesign (IMPLEMENTED)

**What changed:** The conversation view was redesigned from a flat,
label-heavy layout to a modern chat-style display. User messages are
right-aligned with Cyan `▌` borders, Claude messages are left-aligned
with Green `▌` borders. The clean default hides tool calls, thinking
blocks, and token counts — showing only final text output.

**New keybindings:**
- `o` — Toggle tool call visibility (icons: `◆` success, `✗` error,
  `◇` pending)
- `T` — Toggle thinking block visibility (icon: `○`)
- `t` — Toggle token display (unchanged, but default is now OFF)

**Architecture changes:**
- `DisplayOptions` struct in `app.rs` replaces standalone `show_tokens`
- `TurnRenderContext` struct in `conversation.rs` replaces parameter list
- `bubble_width()` + `word_wrap()` for responsive text layout
- Headers/labels hidden in clean mode, shown when any detail flag is on
- Chat alignment disabled at terminal width < 50 for graceful degradation
- `unicode-width` added as direct dependency for column-accurate wrapping

---

### v0.2 - Tool Inspector + Search

**What you can do:** Inspect any tool call in detail. In the conversation
view, tool calls show a one-line summary (e.g. `[Read] src/main.rs (42
lines)` or `[Bash] cargo test → exit 0`). Press `Enter` on a tool call
and the content pane is replaced with the tool's detail view — full input
and output with independent scrolling. Press `Esc` to return to the
conversation at your previous scroll position. This follows the same
Enter=drill-in / Esc=back-out pattern used throughout the app.
Within-session search (`/`) lets you find text in the current conversation.
Tool calls with no result (session ended mid-tool-use) show the tool input
with `(no output captured)`.

**How it works in practice:** While reading a conversation, you see
one-line tool call summaries inline. Press `Enter` on `[Read] src/main.rs`
and the content pane is replaced with the full file contents and line
numbers. For `Edit` calls, you see the old and new text. For `Bash` calls,
you see the command and its output. Press `Esc` to return to the
conversation exactly where you left off. Press `/` to search within the
session — matches are highlighted and you can jump between them with
`n`/`N`. Multi-line inline previews may be added in a future release if
users request them.

**What is NOT in v0.2:**
- No syntax highlighting in tool detail views — plain text only
- No token attribution by category
- No compaction detection
- No cross-session search
- No side-by-side session comparison
- No subagent tree views
- No custom notifications or alerts
- No SSH remote access

---

### v0.3 - Token Attribution + Compaction

**What you can do:** See where your tokens are being spent. Each turn's
context is broken down by category — system prompt, CLAUDE.md files, user
text, tool I/O, thinking blocks — with color-coded attribution. Compaction
events (where Claude Code silently compresses conversation history) are
detected and visualized with before/after token deltas, so you can see
when and how much context was lost. Tool detail views now include syntax
highlighting for code.

**How it works in practice:** Toggle the token view with `t` to see
attribution bars alongside each turn, color-coded by category. You can
immediately spot which turns consumed the most context on tool output vs
thinking vs user text. Compaction markers appear in the conversation where
context was compressed, showing the token delta. Tool detail views now
render code with syntax highlighting.

**What is NOT in v0.3:**
- No cross-session search
- No side-by-side session comparison
- No project-level aggregate statistics
- No subagent tree views
- No custom notifications or alerts
- No SSH remote access
- No data export

---

### v0.4 - Cross-Session Analysis

**What you can do:** Search across all sessions in a project (or globally)
using a command palette (`Ctrl-k`). Results show matching lines with context
snippets and you can jump directly to the matching message. Compare two
sessions side-by-side in a split view — useful for comparing different
approaches to the same problem or reviewing how a session evolved. See
project-level aggregate statistics: total tokens spent, session count,
average session length, most active time periods.

**How it works in practice:** Press `Ctrl-k` to open the command palette,
type a regex pattern, and results stream in (with debouncing) from all
sessions in the current project. Select a result to jump directly to that
message in context. To compare sessions, mark two sessions in the list
with `m` and press `c` to open them side-by-side (requires 120+ column
terminal; narrower terminals show a synced-scroll single-pane fallback).
The project view header shows aggregate stats.

**What is NOT in v0.4:**
- No subagent/team tree visualization
- No custom notification triggers
- No SSH remote access
- No data export or reporting

---

### v0.5 - Advanced Features

**What you can do:** Visualize subagent and team execution as expandable
trees — see which agents were spawned, what tasks they were given, and how
they relate to the parent conversation. Each subagent node shows its own
metrics (tokens, duration, tool calls). Define custom notification triggers
using regex patterns — flag sessions where `.env` files were accessed,
payment-related paths were touched, or token usage exceeded a threshold.
Connect to remote machines via SSH to browse session logs from other
development environments. Export session data or analysis results for
external use.

**How it works in practice:** When viewing a session that spawned subagents,
a tree view shows the full execution hierarchy. You can expand any node to
see its conversation, or collapse it to see just the summary metrics. A
notification panel (toggled with `n`) shows triggered alerts across sessions.
To monitor a remote machine, press `r` to open the SSH connection dialog,
select a configured host from `~/.ssh/config`, and browse remote sessions
as if they were local. Export a session summary to JSON or plain text with
`:export`.

**Note:** SSH support introduces the tokio async runtime. The channel-based
event architecture is forward-compatible — async results feed through the
same `mpsc::channel` as thread-based I/O. The `DataSource` trait may need
an async variant or wrapper.

## Feature Status

| Feature | Status | Version |
|---------|--------|---------|
| Project scaffold | Done | 0.1.0 |
| Turn assembly state machine | Done | 0.1.0 |
| JSONL parser + error types | Done | 0.1.0 |
| Session discovery + empty states | Done | 0.1.0 |
| Conversation viewer + turn nav | Done | 0.1.0 |
| Token usage display | Done | 0.1.0 |
| Application shell | Done | 0.1.0 |
| Tool detail view | Planned | 0.2.0 |
| Within-session search | Planned | 0.2.0 |
| Token attribution (7 categories) | Planned | 0.3.0 |
| Compaction detection | Planned | 0.3.0 |
| Syntax highlighting | Planned | 0.3.0 |
| Cross-session search | Planned | 0.4.0 |
| Session comparison | Planned | 0.4.0 |
| Project aggregate stats | Planned | 0.4.0 |
| Subagent tree visualization | Planned | 0.5.0 |
| Custom notifications | Planned | 0.5.0 |
| SSH remote access | Planned | 0.5.0 |
| Data export | Planned | 0.5.0 |

## Future Considerations

Non-blocking notes from the M1 prerequisite review. These are not
assigned to a specific milestone yet but should be addressed as the
relevant features are implemented.

- **Incomplete turn navigation**: `n`/`N` turn navigation should always
  land on incomplete turns (don't skip them). They represent real user
  interactions and hiding them would be confusing.
- **Sidechain visibility indicator**: When a tool call is a subagent
  spawn with `isSidechain: true`, display it as
  `[Agent] (sidechain hidden)` in the conversation view. Low-cost since
  `ToolName::Agent` already exists. Sets user expectations for v0.5.
- **`tool_summary()` helper**: ~~Each tool type needs a one-line summary
  for inline display.~~ **Done in M3** — implemented in
  `tui/widgets/conversation.rs` with per-tool formatting.
- ~~**`parse_warnings` on Session**~~: **Done in M5** -- displayed in
  conversation status bar with singular/plural grammar.
- ~~**Log file security**~~: **Done in M5** -- documented in `--help`
  via clap `long_help` on `--log-file`.
- ~~**CLI env var in help**~~: **Done in M5** -- `CLAUDE_SEER_PATH` and
  `CLAUDE_SEER_LOG_FILE` appear via clap `env` attribute.
- **Future CLI flags**: `--session <UUID>` (open directly),
  `--project <PATH>` (filter to project), `--no-color` (disable
  colors). Not needed for v0.1.
- **`extract_token_usage` direct tests**: Add dedicated unit tests
  for the happy path and missing-usage fallback of
  `extract_token_usage`.
- **`merge_assistant_responses` accumulation test**: Test that token
  usage is accumulated across multiple assistant responses within one
  turn and that `stop_reason` comes from the last response.
- **`ToolResult.is_error` test coverage**: Add test fixture and test
  for `is_error: true` tool results.
- **`extract_user_content` fallback test**: Test the fallback case
  where content is neither string nor array.
- **Smarter project path decoding**: The current `decoded_path()` is
  lossy -- dashes in real directory names (e.g., `fred-drake`,
  `github.com`) get converted to path separators. Consider
  filesystem-based path resolution or showing the raw encoded name
  with a visual indicator.
- **TUI rendering note**: Use `ProjectPath::decoded_path()` not the
  `Display` trait when rendering project paths in widgets. `Display`
  shows the raw encoded directory name.
- **Scroll offset clamping**: Render-time clamping prevents blank
  space past content, but `AppState.scroll_offset` can hold unclamped
  values. If any future code reads scroll_offset expecting valid
  bounds, it must re-clamp. Consider clamping in `handle_action` if
  content line count becomes available to the state machine.
- **Paragraph scroll u16 overflow**: `Paragraph::scroll()` takes
  `u16`. Conversations with >65535 rendered lines would silently
  wrap. Use `u16::try_from().unwrap_or(u16::MAX)` when this becomes
  a realistic scenario.
- ~~**View-aware help text**~~: **Done in M5** -- help overlay shows
  view-specific descriptions (select vs scroll, Enter vs Esc context).
- **Vim navigation keys**: `g`/`G` (first/last), `Ctrl-d`/`Ctrl-u`
  (half-page scroll), `Home`/`End` are not yet implemented. Track
  for a future keybinding enhancement pass.
- **Conversation render optimization**: `build_conversation_lines`
  renders ALL turns every frame. For large sessions (hundreds of
  turns), consider rendering only visible turns or caching line
  output with invalidation on turn change/scroll.
- ~~**Status bar parse_warnings**~~: **Done in M5** -- see above.
- **`build_turn_lines` parameter count**: Now has 5 parameters
  (`turn`, `total_turns`, `is_current`, `show_tokens`, `cumulative`).
  Consider an options struct if more display flags are added (e.g.,
  `show_thinking`, `show_tool_details` in v0.2+).
- ~~**Status bar truncation at narrow terminals**~~: **Done in M5** --
  progressive disclosure drops hints from right (? help, Esc, t,
  j/k, n/N) based on terminal width.
- **Cumulative token category breakdown**: The cumulative line shows
  in/out/cache totals. A per-category attribution breakdown
  (system prompt, tool I/O, thinking, etc.) fits v0.3 token
  attribution milestone.
- **Title bar usage refresh**: Usage data is fetched once at startup
  and goes stale during long sessions. Add manual refresh (`r`
  keybinding) and/or auto-refresh on a timer (every 5 minutes).
  The channel architecture already supports re-sending
  `UsageLoaded` events.
- **Show reset time when usage is high**: `resets_at` is stored in
  `UsageWindow` but never displayed. When usage is yellow/red,
  show the reset time in the title bar or a tooltip/detail view.
- **Opus-specific usage display**: `seven_day_opus` is fetched but
  not rendered. Consider displaying it when non-zero, either
  inline in the title bar or in a detailed usage view.
- **Title bar color assertion tests**: Tests verify text content
  but don't assert on span colors. Add per-tier tests confirming
  the style of usage value spans (green/yellow/red).
- **Title bar render integration test**: No test renders via
  `ratatui::backend::TestBackend` to verify the full render
  pipeline (padding, alignment). Add with snapshot testing in v0.2.
- **Lighter HTTP client**: `ureq` pulls ~11 transitive dependencies
  for a single GET call. If binary size becomes a concern, consider
  `minreq` or shelling out to `curl`.
- **Progressive disclosure hint order**: The hint drop order is
  hardcoded in both session list and conversation status bars.
  Consider making it configurable if more hints are added in future
  milestones.
- **Warning detail view**: Allow inspecting individual parse warnings
  (line number, reason, etc.) rather than just showing a count.
  Natural fit for v0.2 alongside tool detail views.
- **`list_sessions_for_project()` streaming optimization**:
  Currently reads entire JSONL files, same as `list_sessions()`.
  Consider streaming/partial read (first + last lines) for large
  sessions. Target v0.2.
- **Deprecate `list_sessions()` trait method**: No longer used in the
  production flow (replaced by `list_projects()` +
  `list_sessions_for_project()`). Consider removing from the
  `DataSource` trait in a future version.
- **`AppState` field grouping**: AppState has 15+ fields. Consider
  grouping into sub-structs (`ProjectListState`,
  `SessionListState`, `ConversationState`) when adding more views.
- **`g`/`G` keybindings**: Jump-to-top/bottom navigation not yet
  implemented. Defer to a follow-up keybinding enhancement pass
  alongside `Ctrl-d`/`Ctrl-u` (half-page scroll).
