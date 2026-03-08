# Product Owner Memory

## Project Overview
- claude-seer: TUI app for visualizing Claude Code session data
- Stack: Rust + ratatui + crossterm
- Reference: claude-devtools (desktop/web app)
- TDD is mandatory; cargo-nextest + rstest for testing

## JSONL Format (verified 2026-03-08)
- Location: ~/.claude/projects/{encoded-project-path}/{session-uuid}.jsonl
- Record types: user, assistant, progress, system,
  file-history-snapshot, last-prompt, queue-operation
- Token usage in assistant messages at .message.usage
- Tool calls in assistant .message.content[] where type=tool_use
- Progress subtypes: bash_progress, hook_progress, agent_progress,
  search_results_received, query_update
- System subtypes: stop_hook_summary, turn_duration
- No dedicated "compaction" record type found yet; may need to
  infer from token count drops or context window resets
- See [jsonl-format.md](jsonl-format.md) for details

## MVP Definition (v0.1)
- 5 milestones: parser, session list, conversation view,
  token usage, app shell
- See docs/ROADMAP.md for full plan
- Priorities documented in [mvp-stories.md](mvp-stories.md)

## Release Plan (agreed 2026-03-08, team review)
- **v0.1**: Browse — session list + conversation viewer + token totals
- **v0.2**: Inspect — tool inspector (1-line summary + pane detail) + within-session search
- **v0.3**: Analyze — token attribution + compaction detection + syntax highlighting
- **v0.4**: Cross-session — search across sessions, side-by-side compare, aggregates
- **v0.5**: Advanced — subagent trees, SSH, notifications, export
- See [roadmap-decisions.md](roadmap-decisions.md) for full rationale

## Key Decisions
- Separation of data layer (parser) from TUI layer is critical
  for testability
- Parser must be fully unit-testable without any TUI dependency
- serde + serde_json for JSONL parsing
- All non-TUI code must have automated tests
- No separate integration test suite for v0.1-v0.3. Unit tests
  with real (anonymized) JSONL fixtures provide equivalent
  coverage. Revisit if network/DB layers are added (v0.4+).
- Test fixtures: synthetic in tests/fixtures/ for TDD cycles,
  real anonymized data in tests/fixtures/real/ for coverage
- Error enums defined in M1 (parser foundation), display in M5
- Empty state handling implemented in M2 (4 states: no dir,
  no sessions, loading, empty session)
- Turn assembly state machine diagram required before M1 coding
- Tool detail: 1-line summary in conversation, Enter opens
  detail in pane replacement, Esc returns (team-lead decision)
- Story 3.2 (turn navigation n/N) elevated to MUST-HAVE
