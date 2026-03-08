# Testing Strategy for claude-seer

Last Updated: 2026-03-08 | Version: 0.1.0

## Overview

claude-seer follows strict TDD (Red-Green-Refactor). Every feature begins
with a failing test. The architecture is designed so that all business logic
is testable without a terminal.

## Architecture for Testability

The key principle: **separate logic from presentation**. The codebase is
organized into layers that can be tested independently.

```
src/
  main.rs                  -- entry point: CLI args, terminal setup, run loop
  lib.rs                   -- re-exports public API for integration tests
  app.rs                   -- application state machine (no TUI deps)
  config.rs                -- CLI args, config file, paths

  data/
    mod.rs                 -- re-exports
    model.rs               -- core domain types (Session, Turn, Message, etc.)
    parser.rs              -- JSONL line parsing into raw records
    session_loader.rs      -- loads/indexes session files from disk
    token_attribution.rs   -- token categorization (7 categories)
    compaction.rs          -- compaction event detection
    search.rs              -- full-text and regex search across sessions
    error.rs               -- DataError (thiserror)

  source/
    mod.rs                 -- re-exports, DataSource trait
    filesystem.rs          -- local filesystem impl of DataSource
    error.rs               -- SourceError (thiserror)

  tui/
    mod.rs                 -- re-exports, terminal setup/teardown
    event.rs               -- event loop: crossterm events + app channels
    ui.rs                  -- root layout dispatcher
    widgets/
      mod.rs
      session_list.rs      -- session browser pane
      conversation.rs      -- message/turn viewer pane
      token_chart.rs       -- token usage visualization
      tool_detail.rs       -- tool call inspector
      status_bar.rs        -- status bar / command palette
      help.rs              -- help overlay
    error.rs               -- TuiError (thiserror)
```

`app.rs` owns all state transitions and is fully testable with unit
tests. The `tui/` layer is a thin mapping from AppState to ratatui
widgets, testable via `TestBackend` and snapshot tests. `data/` and
`source/` have zero TUI dependencies and are pure library code.

---

## 1. Unit Testing Approach

### Where tests live

Every module has a `#[cfg(test)] mod tests` block at the bottom.

### Pattern: rstest fixtures for common setup

```rust
use rstest::*;

/// A minimal valid JSONL line representing a user message.
#[fixture]
fn user_message_line() -> &'static str {
    r#"{"type":"user","message":{"role":"user","content":"hello"},"uuid":"abc-123","parentUuid":null,"sessionId":"sess-1","timestamp":"2026-03-08T10:00:00.000Z","cwd":"/tmp","version":"2.1.71","isSidechain":false,"userType":"external"}"#
}

/// A minimal valid JSONL line representing an assistant message.
#[fixture]
fn assistant_message_line() -> &'static str {
    r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hi"}],"model":"claude-opus-4-6","id":"msg_01","type":"message","usage":{"input_tokens":100,"output_tokens":50}},"uuid":"def-456","parentUuid":"abc-123","sessionId":"sess-1","timestamp":"2026-03-08T10:00:01.000Z","cwd":"/tmp","version":"2.1.71","isSidechain":false,"userType":"external"}"#
}

/// A temporary directory populated with sample JSONL session files.
#[fixture]
fn sample_sessions_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("projects").join("my-project");
    std::fs::create_dir_all(&project_dir).unwrap();
    std::fs::write(
        project_dir.join("session-1.jsonl"),
        include_str!("../tests/fixtures/session_basic.jsonl"),
    ).unwrap();
    dir
}
```

### Pattern: rstest `#[case]` for parameterized tests

```rust
#[rstest]
#[case::user_msg(r#"{"type":"user",...}"#, MessageType::User)]
#[case::assistant_msg(r#"{"type":"assistant",...}"#, MessageType::Assistant)]
#[case::progress_msg(r#"{"type":"progress",...}"#, MessageType::Progress)]
#[case::file_history(r#"{"type":"file-history-snapshot",...}"#, MessageType::FileHistory)]
fn parse_message_type(#[case] input: &str, #[case] expected: MessageType) {
    let entry = parse_jsonl_line(input).unwrap();
    assert_eq!(entry.message_type(), expected);
}
```

### What gets unit tested

| Module                     | What to test                                    | Edge cases                                          |
|----------------------------|-------------------------------------------------|-----------------------------------------------------|
| `data/parser`              | Deserialize each JSONL `type` variant            | Malformed JSON, unknown type, missing fields,        |
|                            |                                                 | truncated lines, empty lines, BOM markers            |
| `data/model`               | Data model accessors, derived traits             | Empty content, very large token counts, null uuids   |
| `data/session_loader`      | Turn assembly, two-pass parsing                 | Incomplete turns, orphaned messages, sidechains      |
| `data/search`              | Keyword matching, filtering by date/model/branch | Empty query, regex special chars, unicode,           |
|                            |                                                 | case insensitivity, no matches                       |
| `data/token_attribution`   | Token aggregation, category breakdown            | Zero tokens, overflow-large values, missing usage    |
| `data/compaction`          | Compaction event detection                       | No compaction, gradual growth, exact threshold       |
| `source/filesystem`        | Find .jsonl files under ~/.claude/projects/      | Empty dirs, nested dirs, symlinks, permission errors |
| `app` (state machine)      | State machine transitions                        | Invalid transitions, repeated key presses            |
| `app` (action handlers)    | Action handlers mutate state correctly           | Actions in wrong state, empty session list            |

### Testing error paths

Every `Result` return must have at least one test for the `Err` variant:

```rust
#[test]
fn parse_rejects_invalid_json() {
    let result = parse_jsonl_line("not json at all");
    assert!(result.is_err());
}

#[test]
fn parse_rejects_unknown_type() {
    let result = parse_jsonl_line(r#"{"type":"alien"}"#);
    assert!(matches!(result, Err(ParseError::UnknownType(_))));
}

#[test]
fn scanner_handles_missing_directory() {
    let result = scan_sessions(Path::new("/nonexistent/path"));
    assert!(result.is_err());
}

#[test]
fn scanner_handles_empty_directory() {
    let dir = tempfile::tempdir().unwrap();
    let sessions = scan_sessions(dir.path()).unwrap();
    assert!(sessions.is_empty());
}
```

---

## 2. Integration Testing (Minimal)

This app has no database, no network calls, and no shared mutable state.
The data pipeline is pure functions composed in sequence:
`read file -> parse lines -> build model -> render`. As a result, most
"integration" tests are redundant with unit tests that compose the same
function calls. We keep the `tests/` directory lean.

### What does NOT go in `tests/`

Cross-module workflows like "parse a file then aggregate tokens" are
tested as unit tests inside the relevant module. They are just function
calls -- putting them in `tests/` adds a separate compilation unit
(slower builds) for no additional confidence.

### What DOES go in `tests/`

**1. TUI E2E loop test** -- the one place where multiple components
(event handling, state mutation, rendering) interact in a loop:

```rust
// tests/tui_e2e.rs
fn run_app_with_events(events: Vec<Event>) -> String {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new(mock_sessions());

    for event in events {
        app.handle_event(event);
        if app.should_quit() { break; }
        terminal.draw(|f| ui::render(&app, f)).unwrap();
    }

    terminal.backend().to_string()
}
```

**2. Real-data validation** -- an `#[ignore]` test that runs against
the actual `~/.claude/` directory on a developer machine, never in CI.
This catches format drift when Claude Code ships new JSONL fields or
entry types.

```rust
// tests/validate_real_data.rs
#[test]
#[ignore] // Run manually: cargo test -- --ignored
fn validate_against_real_sessions() {
    let claude_home = dirs::home_dir().unwrap().join(".claude");
    if !claude_home.exists() {
        return; // Skip on machines without Claude Code
    }
    let sessions = scan_sessions(&claude_home).unwrap();
    for session_path in sessions.iter().take(5) {
        let result = parse_session_file(session_path);
        assert!(
            result.is_ok(),
            "Failed to parse {}: {:?}",
            session_path.display(),
            result.err()
        );
    }
}
```

### Why not commit real Claude Code output as fixtures?

Real sessions contain sensitive data: full file contents in
`tool_result` entries, absolute filesystem paths, prompt content that
may include secrets. They are also large (10MB+ for long sessions) and
represent a single format snapshot that goes stale. Hand-crafted
fixtures are controlled, small, safe, and stable.

---

## 3. TUI Testing

### Level 1: State machine tests (no terminal, high priority)

The app state machine is pure logic. Test every transition:

```rust
#[test]
fn pressing_enter_on_session_list_opens_detail_view() {
    let mut app = App::new(mock_sessions());
    app.select_session(0);
    app.handle_action(Action::Confirm);
    assert!(matches!(app.state(), AppState::SessionDetail { .. }));
}

#[test]
fn pressing_escape_in_detail_returns_to_list() {
    let mut app = App::new(mock_sessions());
    app.set_state(AppState::SessionDetail { index: 0 });
    app.handle_action(Action::Back);
    assert!(matches!(app.state(), AppState::SessionList));
}
```

### Level 2: Render snapshot tests with TestBackend + insta

Add `insta` to dev-dependencies for snapshot testing rendered frames.
This catches unintended rendering regressions.

```rust
use ratatui::prelude::*;
use ratatui::backend::TestBackend;
use insta::assert_snapshot;

#[test]
fn session_list_renders_correctly() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let app = App::new(mock_sessions());
    terminal.draw(|f| ui::render_session_list(f, &app)).unwrap();

    let view = terminal.backend().to_string();
    assert_snapshot!("session_list_default", view);
}
```

Snapshots are stored in `src/snapshots/` and reviewed with `cargo insta
review`. Any rendering change produces a diff for human review.

### Level 3: Full E2E terminal testing (lower priority)

Options researched:

1. **ratatui TestBackend + simulated input loop** (recommended first step)
   - Drive the full app loop with a sequence of injected `Event` values
   - Assert on the final `TestBackend` buffer contents
   - No real terminal needed; runs in CI

   ```rust
   fn run_app_with_events(events: Vec<Event>) -> String {
       let backend = TestBackend::new(80, 24);
       let mut terminal = Terminal::new(backend).unwrap();
       let mut app = App::new(mock_sessions());

       for event in events {
           app.handle_event(event);
           if app.should_quit() { break; }
           terminal.draw(|f| ui::render(&app, f)).unwrap();
       }

       terminal.backend().to_string()
   }

   #[test]
   fn e2e_navigate_and_quit() {
       let output = run_app_with_events(vec![
           key_event(KeyCode::Down),
           key_event(KeyCode::Enter),
           key_event(KeyCode::Char('q')),
       ]);
       // Assert the final frame contained expected content
       assert!(output.contains("Session Detail"));
   }
   ```

2. **tmux send-keys + capture-pane** (true E2E, CI-unfriendly)
   - Launch the real binary in a tmux session
   - Send keystrokes via `tmux send-keys`
   - Capture output via `tmux capture-pane -p`
   - Fragile, timing-dependent, but tests the real terminal path

3. **expect / pty-based testing**
   - Crates like `expectrl` or `rexpect` can spawn a PTY
   - Send input, wait for expected output patterns
   - More portable than tmux but still timing-sensitive

**Recommendation**: Start with approach 1 (TestBackend + injected events).
It covers 95% of what you need, runs fast, and works in CI. Defer
approaches 2 and 3 until there are real terminal-specific bugs to chase.

---

## 4. Test Fixture Strategy

### Fixture files

```
tests/
  fixtures/
    session_basic.jsonl                -- minimal valid session (user + assistant)
    session_multi_turn.jsonl           -- multi-turn conversation with tool use
    session_linear.jsonl               -- simple 3-turn, no tool use
    session_sidechain.jsonl            -- records with isSidechain: true
    session_resumed.jsonl              -- resumed session (timestamp gap, cwd change)
    session_orphaned_progress.jsonl    -- progress records without active turn
    session_mid_toolcall.jsonl         -- session ends with pending tool_use
    session_consecutive_users.jsonl    -- two user messages without assistant between
    session_mismatched_toolresult.jsonl -- tool_result with non-existent tool_use_id
    session_with_errors.jsonl          -- includes malformed lines mixed in
    session_empty.jsonl                -- empty file
    claude_home/                       -- mock ~/.claude/ directory structure
      projects/
        my-project/
          session-1.jsonl
          session-2.jsonl
        other-project/
          session-3.jsonl
```

### Fixture philosophy

**Static files over builders.** A `SessionBuilder` adds indirection
that makes test failures harder to debug. With four to six small JSONL
files, you can open the fixture and see exactly what the test is
parsing. If you later need to generate hundreds of parameterized
sessions (e.g., for benchmarks), add a builder then -- not before.

### Using real data (carefully)

For local development, use the `#[ignore]` validation test in
`tests/validate_real_data.rs` to run the parser against your actual
`~/.claude/projects/` directory. Never commit real session data.
Use `CLAUDE_HOME` env var to override the session directory path.

---

## 5. Coverage Targets

| Module                   | Target  | Rationale                                       |
|--------------------------|---------|-------------------------------------------------|
| `data/parser`            | >= 95%  | Core correctness, many edge cases               |
| `data/model`             | >= 90%  | Data structures, mostly derived                 |
| `data/session_loader`    | >= 95%  | Turn assembly is critical logic                 |
| `data/search`            | >= 90%  | User-facing feature, must handle bad input      |
| `data/token_attribution` | >= 95%  | Financial calculation, must be exact             |
| `data/compaction`        | >= 95%  | Heuristic detection, edge cases matter          |
| `source/filesystem`      | >= 85%  | Filesystem interactions, harder to cover all OS  |
| `app`                    | >= 95%  | State machine must be exhaustively tested        |
| `tui/`                   | >= 60%  | Thin rendering layer, covered by snapshots       |
| **Overall**              | >= 85%  |                                                  |

Run coverage: `cargo tarpaulin --out html --output-dir target/tarpaulin`

---

## 6. Benchmarking and Profiling Workflow

### Criterion benchmarks (regression tracking)

Located in `benches/benchmarks.rs`. Run with `cargo bench`.

```rust
use criterion::{Criterion, criterion_group, criterion_main, BenchmarkId};
use claude_seer::data::parser::parse_session_file;
use std::path::PathBuf;

fn bench_parse_session(c: &mut Criterion) {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/session_large.jsonl");

    c.bench_function("parse_1000_line_session", |b| {
        b.iter(|| {
            parse_session_file(&fixture).unwrap()
        });
    });
}

fn bench_parse_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_scaling");
    for size in [10, 100, 500, 1000] {
        let data = generate_jsonl_lines(size);
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &data,
            |b, data| b.iter(|| parse_jsonl_string(data)),
        );
    }
    group.finish();
}

criterion_group!(benches, bench_parse_session, bench_parse_scaling);
criterion_main!(benches);
```

### Divan benchmarks (quick iteration)

Located in `benches/divan_benchmarks.rs`. Run with
`cargo bench --bench divan_benchmarks`.

```rust
use claude_seer::data::parser::parse_jsonl_line;

#[divan::bench]
fn parse_user_message(bencher: divan::Bencher) {
    let line = include_str!("../tests/fixtures/user_message.jsonl");
    bencher.bench(|| parse_jsonl_line(divan::black_box(line)));
}

#[divan::bench(args = [10, 100, 1000])]
fn parse_n_lines(bencher: divan::Bencher, n: usize) {
    let lines: Vec<String> = generate_lines(n);
    bencher.bench(|| {
        for line in &lines {
            let _ = parse_jsonl_line(divan::black_box(line));
        }
    });
}
```

### dhat heap profiling

Build with `--profile dhat`, run the binary, analyze the output:

```rust
// In main.rs, behind a feature flag or cfg:
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();
    // ... rest of main
}
```

Workflow:
```bash
cargo build --profile dhat --features dhat-heap
./target/dhat/claude-seer
# Opens dhat-heap.json -- view at https://nnethercote.github.io/dh_view/dh_view.html
```

### Flamegraph profiling

```bash
cargo flamegraph --profile flamegraph -- --session /path/to/large.jsonl
# Produces flamegraph.svg
```

On macOS with `samply` (already in devshell):
```bash
cargo build --profile flamegraph
samply record ./target/flamegraph/claude-seer
```

---

## 7. CI Pipeline

Every commit should run these checks. Use GitHub Actions or similar.

### Fast checks (< 2 min, run on every push)

```yaml
- cargo fmt --check           # formatting
- cargo clippy -- -D warnings # lints, zero warnings policy
- cargo nextest run           # all unit + integration tests
```

### Extended checks (run on PR, nightly)

```yaml
- cargo tarpaulin --out xml   # coverage report
- cargo bench --no-run        # benchmarks compile
- cargo audit                 # security vulnerabilities
- cargo outdated              # dependency staleness
```

### Benchmark regression (run on main merges)

```yaml
- cargo bench -- --save-baseline main
# On PR branch:
- cargo bench -- --baseline main
# Criterion produces comparison reports
```

### Suggested just tasks

```just
# Justfile
test:
    cargo nextest run

test-verbose:
    cargo nextest run --status-level all

coverage:
    cargo tarpaulin --out html --output-dir target/tarpaulin
    open target/tarpaulin/tarpaulin-report.html

bench:
    cargo bench

bench-quick:
    cargo bench --bench divan_benchmarks

clippy:
    cargo clippy -- -D warnings

check: clippy test
    @echo "All checks passed"

profile-heap:
    cargo build --profile dhat --features dhat-heap
    ./target/dhat/claude-seer

profile-flamegraph:
    cargo flamegraph --profile flamegraph
```

---

## Summary of Test Patterns

| Pattern                | Tool          | Use case                              |
|------------------------|---------------|---------------------------------------|
| `#[fixture]`           | rstest        | Shared test setup (temp dirs, data)   |
| `#[case]`              | rstest        | Parameterized tests across variants   |
| `#[test]` in module    | stdlib        | Unit tests for each module            |
| `tests/tui_e2e.rs`     | cargo test    | Full app loop with TestBackend        |
| `tests/validate_*.rs`  | cargo test    | `#[ignore]` real-data format checks   |
| `TestBackend`          | ratatui       | Rendering assertions without terminal |
| `assert_snapshot!`     | insta         | Regression detection for UI output    |
| `tempfile::tempdir()`  | tempfile      | Isolated filesystem tests             |
| `include_str!`         | stdlib        | Embed fixture files at compile time   |
| `SessionBuilder`       | custom        | Programmatic test data generation     |
| `criterion_group!`     | criterion     | Tracked performance benchmarks        |
| `#[divan::bench]`      | divan         | Quick microbenchmarks                 |
| `dhat::Profiler`       | dhat          | Heap allocation profiling             |
