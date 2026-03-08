# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

claude-seer is a Rust TUI application for visualizing Claude Code session data. It reads JSONL session logs from `~/.claude/projects/` and presents them through an interactive terminal interface built with ratatui + crossterm. Currently in early development (v0.1.0-planning).

## Commands

```bash
# Build
cargo build

# Run
cargo run

# Tests (use nextest, not cargo test)
cargo nextest run                          # all tests
cargo nextest run <test_name>              # single test by name
cargo nextest run -p claude-seer --lib     # lib tests only

# Lint & format
cargo clippy -- -D warnings               # zero warnings policy
cargo fmt --check                          # check formatting
cargo fmt                                  # fix formatting

# Coverage
cargo tarpaulin --out html --output-dir target/tarpaulin

# Benchmarks
cargo bench                               # criterion (regression tracking)
cargo bench --bench divan_benchmarks       # divan (quick iteration)

# Profiling
cargo flamegraph --profile flamegraph      # CPU flamegraph
cargo build --profile dhat --features dhat-heap  # heap profiling

# Security
cargo audit
```

## TDD Workflow (mandatory)

All implementation follows strict red-green-refactor:

1. **Red**: Write ONE failing test. Run it. Confirm it fails for the right reason.
2. **Green**: Write MINIMAL code to pass. No anticipatory coding.
3. **Refactor**: Improve structure only when tests are green.

Never write implementation without a failing test first. Never add more than one failing test at a time. See `.claude/tdd-guard/data/instructions.md` for detailed rules.

## Architecture

Three layers with strict separation — `data/`, `source/`, and `app.rs` must have **zero** dependencies on ratatui or crossterm:

| Module | Purpose | TUI dependency? |
|--------|---------|-----------------|
| `data/` | Domain types, JSONL parser, analysis | No |
| `source/` | DataSource trait + filesystem impl | No |
| `app.rs` | Pure state machine, action handling | No |
| `tui/` | Terminal, event loop, widgets | Yes |
| `main.rs` | Entry point, CLI, wiring | Yes |

Key patterns:
- **Pure state machine**: `AppState::handle_action(Action) -> Option<SideEffect>` — no I/O in app logic
- **JSONL parsing**: Typed envelope (`RawRecord`) + `serde_json::Value` for message body (handles format evolution)
- **Two-pass parsing**: Summary scan for session list (fast), full parse only when session is opened
- **Background I/O**: `std::thread` + `mpsc::channel` (no async runtime)
- **Error handling**: `thiserror` enums per module (`DataError`, `SourceError`), `miette` only in `main.rs`
- **Newtype IDs**: `SessionId`, `MessageId`, `ProjectPath` prevent compile-time mixups

See `docs/DATA_FLOW.md` for full architecture and `docs/ROADMAP.md` for release plan.

## Testing

- Unit tests in `#[cfg(test)]` modules within each source file
- Test fixtures: static JSONL files in `tests/fixtures/`
- `rstest` for fixtures (`#[fixture]`) and parameterized tests (`#[case]`)
- `insta` for snapshot testing of rendered TUI frames (v0.2+)
- Coverage targets: 95% parser/state machine, 85% overall
- Every `Result` return must have an `Err` path test
- `.unwrap()`/`.expect()` allowed in tests only — production code uses `Result`

## Code Style

- `cargo clippy -- -D warnings` and `cargo fmt` must pass before work is complete
- Prefer iterators over manual loops
- Keep functions small and focused
- No `.unwrap()` in production code

## Agent Team

Five agents in `.claude/agents/` for collaborative development:
- **engineer**: Writes code via TDD (has write access)
- **code-architect**: Designs modules, traits, data flow (read-only)
- **qa-engineer**: Reviews tests, coverage, benchmarks (has write access)
- **product-owner**: Defines requirements, prioritizes features (read-only)
- **tui-designer**: Designs layouts, widgets, UX (read-only)

### Using Agent Teams

For collaborative tasks (planning, reviews, multi-perspective analysis), use agent teams rather than subagents. Teams spawn each agent in its own tmux pane where they can message each other directly.

1. **Create team**: Use `TeamCreate` with a descriptive team name
2. **Create tasks**: Use `TaskCreate` for each agent's work
3. **Spawn teammates**: Use `Agent` tool with `team_name` and `name` parameters, setting `subagent_type` to the agent name (e.g., `product-owner`, `code-architect`)
4. **Coordinate**: Agents message each other via `SendMessage`. Use `broadcast` sparingly — prefer direct messages.
5. **Collect consensus**: After agents discuss, they report findings back to the team lead
6. **Shutdown**: Send `shutdown_request` to each teammate when done

Example: To have the team review a document, create one task per agent, spawn all four reviewers with their tasks, let them review and cross-discuss, then collect and synthesize their findings.

## Hooks

A PostToolUse hook automatically runs `cargo check` whenever a `.rs` file is written or edited.
