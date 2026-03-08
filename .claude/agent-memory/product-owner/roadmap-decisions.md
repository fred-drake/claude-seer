# Roadmap Review Decisions (2026-03-08)

Team review with product-owner, code-architect, tui-designer, qa-engineer.

## 6 Cross-Cutting Issues Resolved

### 1. v0.2 Scope Split (unanimous)
Original v0.2 was too large (tool inspector + syntax highlighting +
inline diffs + token attribution + compaction + search). Split into:
- v0.2: tool inspector + within-session search
- v0.3: token attribution + compaction + syntax highlighting
- v0.4: cross-session analysis (was v0.3)
- v0.5: advanced features (was v0.4)
Rationale: each release = one theme. Compaction needs real JSONL
examples we haven't captured yet.

### 2. Tool Detail: Pane Replacement (team-lead decision)
Two proposals: (a) 5-line inline preview + overlay modal,
(b) 1-line summary + pane replacement on Enter.
Decision: start with (b) — simpler, consistent with Enter/Esc
drill-in/back pattern. Inline previews can be added later as polish.

### 3. Turn Assembly Diagram (unanimous)
Must be created BEFORE M1 coding starts. Covers 6 edge cases:
normal pairs, tool result interleaving, progress record attachment,
orphaned messages, sidechain conversations, incomplete turns.
Architect produces, QA validates test coverage against it.

### 4. Error Types in M1 (PO + QA aligned)
Define DataError and SourceError enums in M1. Two severities:
MalformedLine (skip + log) and CorruptFile (abort load).
User-facing error display defers to M5.

### 5. Module Naming (deferred to architect + QA)
TESTING_STRATEGY.md uses different names than ROADMAP/DATA_FLOW.
Must reconcile before M1.

### 6. Empty State in M2 (3-of-4, then unanimous)
4 states to handle:
1. No ~/.claude/ directory -> full-screen guidance
2. Directory exists, no sessions -> "(empty)" + guidance
3. Loading -> "Loading session..." text
4. Empty session file -> "No conversation turns"
TUI-designer provided wireframes.

## Product Gaps Identified (from initial review)
- Missing CLI args story (--path, --help, --version) for v0.1
- Session list should show last_prompt as preview/title
- Project path display needs decoding (dashes to slashes)
- Project sorting by most recent activity not specified
- Performance targets undefined (propose: list <1s, open <500ms, render <16ms)
- Story 3.2 (turn navigation) elevated to MUST-HAVE
