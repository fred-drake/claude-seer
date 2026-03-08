# QA Engineer Memory - claude-seer

## Project Structure
- Rust 2024 edition, ratatui TUI app
- Parses Claude Code JSONL session logs from ~/.claude/projects/
- Dev env via Nix flake with cargo-nextest, cargo-tarpaulin, cargo-flamegraph, samply

## JSONL Format (from real ~/.claude/ data)
- Each line is a JSON object with `type` field: "user", "assistant", "progress",
  "tool_result", "file-history-snapshot"
- Key fields: `uuid`, `parentUuid`, `sessionId`, `timestamp`, `cwd`, `version`,
  `gitBranch`, `isSidechain`, `message`
- Assistant messages have nested `usage` with: `input_tokens`, `output_tokens`,
  `cache_creation_input_tokens`, `cache_read_input_tokens`
- Assistant content is an array of blocks: `{"type":"text","text":"..."}` or
  `{"type":"tool_use","id":"...","name":"...","input":{...}}`
- Sessions are UUID-named .jsonl files under ~/.claude/projects/<encoded-path>/

## Testing Stack
- rstest 0.23 (fixtures, parameterized), criterion 0.5, divan 0.1, dhat 0.3
- insta 1 (snapshot testing for TUI renders), tempfile 3
- cargo-nextest as runner, cargo-tarpaulin for coverage
- `dhat-heap` feature flag for heap profiling

## Test Fixtures
- tests/fixtures/session_basic.jsonl - 2 messages (user + assistant)
- tests/fixtures/session_multi_turn.jsonl - 7 entries with tool use
- tests/fixtures/session_with_errors.jsonl - mixed valid/invalid lines
- tests/fixtures/session_empty.jsonl - empty file
- tests/fixtures/claude_home/ - mock ~/.claude/ directory structure

## Architecture Principle
- Separate logic (app/) from presentation (ui/)
- app/ layer is pure state machine, fully unit testable
- ui/ layer tested via ratatui TestBackend + insta snapshots
- TUI E2E: inject Event sequences into app loop with TestBackend

## Integration Test Philosophy (decided 2026-03-08)
- No DB/network/shared state = minimal integration tests needed
- Cross-module pipeline tests (parse+aggregate, scan+search) belong as unit tests
  in the relevant module, NOT in tests/ -- avoids extra compilation units
- tests/ directory reserved for: (1) TUI E2E loop test, (2) #[ignore] real-data validation
- Never commit real ~/.claude/ session data -- use #[ignore] test for local validation
- Static fixture files over SessionBuilder -- simpler, easier to debug failures
- Real JSONL risk: sensitive data, large files, format snapshots that go stale

## Coverage Targets
- parser/tokens/app-state: >= 95%
- models/search/actions: >= 90%
- scanner: >= 85%, ui: >= 60%, overall: >= 85%

## Key Files
- docs/TESTING_STRATEGY.md - full testing plan
- Cargo.toml has `[features] dhat-heap = []` for profiling
