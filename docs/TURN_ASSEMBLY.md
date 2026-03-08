# Turn Assembly State Machine

Last Updated: 2026-03-08 | Version: 0.1.0-planning

## Overview

The Turn Assembler transforms a flat stream of JSONL records into
structured `Turn` objects. Each turn represents one user message paired
with one assistant response. This document specifies the state machine
that drives that assembly.

## States

| State | Description |
|-------|-------------|
| `Idle` | No turn in progress. Waiting for a user message. |
| `AwaitingAssistant` | User message received, waiting for assistant response. |
| `AwaitingToolResult` | Assistant responded with `tool_use`, waiting for `tool_result`. |
| `TurnComplete` | A full turn has been assembled. Emit it and transition to `Idle`. |

## State Diagram

```
                     ┌──────────────────────────────────┐
                     │                                  │
                     ▼                                  │
               ┌──────────┐                             │
               │          │                             │
     ─────────►│   Idle   │◄────────────────────┐       │
   (start/     │          │                     │       │
    emit turn) └────┬─────┘                     │       │
                    │                           │       │
                    │ [user message]             │       │
                    │                           │       │
                    ▼                           │       │
          ┌──────────────────┐                  │       │
          │                  │                  │       │
          │ AwaitingAssistant│                  │       │
          │                  │                  │       │
          └────┬───────┬─────┘                  │       │
               │       │                        │       │
               │       │ [user message]*        │       │
               │       └────────────────────────┘       │
               │                                        │
               │ [assistant message]                    │
               │                                        │
               ▼                                        │
     ┌─────────────────────┐                            │
     │  Check stop_reason  │                            │
     └────┬──────────┬─────┘                            │
          │          │                                  │
          │          │ stop_reason == "tool_use"         │
          │          │                                  │
          │          ▼                                  │
          │  ┌───────────────────┐                      │
          │  │                   │──[tool_result]──┐    │
          │  │AwaitingToolResult │                  │    │
          │  │                   │◄─[assistant      │    │
          │  └───────────────────┘   tool_use]──────┘    │
          │          │                                  │
          │          │ [assistant end_turn]              │
          │          │                                  │
          │ stop_reason == "end_turn"                   │
          │ or stop_reason == null                      │
          │          │                                  │
          ▼          ▼                                  │
     ┌──────────────────┐                               │
     │                  │                               │
     │  TurnComplete    │───────────────────────────────┘
     │  (emit turn)     │
     │                  │
     └──────────────────┘
```

*When a second user message arrives while in `AwaitingAssistant`,
the first user message produced an incomplete turn. See "Incomplete
Turns" below.

## Transition Table

| Current State | Input Record | Guard / Condition | Action | Next State |
|---|---|---|---|---|
| `Idle` | `user` | `isSidechain == false` | Store as current user msg | `AwaitingAssistant` |
| `Idle` | `user` | `isSidechain == true` | Skip (sidechain) | `Idle` |
| `Idle` | `assistant` | — | Warn: orphaned assistant | `Idle` |
| `Idle` | `tool_result` | — | Warn: orphaned tool_result | `Idle` |
| `Idle` | `progress` | — | Warn: orphaned progress | `Idle` |
| `Idle` | other type | — | Ignore (summary, system, etc.) | `Idle` |
| `AwaitingAssistant` | `assistant` | `isSidechain == false` | Store assistant response | Check stop_reason |
| `AwaitingAssistant` | `assistant` | `isSidechain == true` | Skip (sidechain) | `AwaitingAssistant` |
| `AwaitingAssistant` | `user` | — | Warn: consecutive user. Emit current user as incomplete turn (`assistant_response: None`, `is_complete: false`). Replace stored user with the new user message and remain in `AwaitingAssistant`. | `AwaitingAssistant` |
| `AwaitingAssistant` | `progress` | — | Attach to current turn | `AwaitingAssistant` |
| `AwaitingAssistant` | other type | — | Ignore | `AwaitingAssistant` |
| Check stop_reason | — | `stop_reason == "tool_use"` | — | `AwaitingToolResult` |
| Check stop_reason | — | `stop_reason != "tool_use"` | — | `TurnComplete` |
| `AwaitingToolResult` | `tool_result` | `tool_use_id` matches | Attach result to tool call | See below |
| `AwaitingToolResult` | `tool_result` | `tool_use_id` no match | Warn: mismatched tool_result, attach anyway | See below |
| `AwaitingToolResult` | `assistant` | `isSidechain == false` | Assistant continues (new content blocks) | Check stop_reason |
| `AwaitingToolResult` | `assistant` | `isSidechain == true` | Skip (sidechain) | `AwaitingToolResult` |
| `AwaitingToolResult` | `user` (not tool_result) | — | Turn complete (tool result never arrived) | `TurnComplete` + new `AwaitingAssistant` |
| `AwaitingToolResult` | `progress` | — | Attach to current turn | `AwaitingToolResult` |
| `AwaitingToolResult` | other type | — | Ignore | `AwaitingToolResult` |
| `TurnComplete` | — | — | Emit turn, reset | `Idle` |

Note: After receiving a `tool_result` in `AwaitingToolResult`, the
assembler expects the next assistant message to continue the same turn.
It stays in `AwaitingToolResult` until an assistant message arrives.
When the assistant arrives, it checks `stop_reason` again to decide
between another tool round or turn completion.

## Tool Result Interleaving

A single turn can involve multiple tool calls. The pattern is:

```
user message
  assistant (tool_use, stop_reason="tool_use")  ─┐
  tool_result                                     │ repeated
  assistant (tool_use, stop_reason="tool_use")  ─┘ N times
  tool_result
  assistant (text, stop_reason="end_turn")      ← turn ends
```

All assistant messages and tool results within this chain belong to the
same turn. The assembler accumulates content blocks from each assistant
message and correlates each `tool_result` with its `tool_use` block via
`tool_use_id`.

## Progress Record Attachment

Progress records (`type: "progress"`) are not part of the conversation
but carry metadata (hook execution, timing). They attach to the nearest
turn currently being assembled:

- In `AwaitingAssistant`: attach to the pending turn (user message
  received, waiting for response)
- In `AwaitingToolResult`: attach to the pending turn
- In `Idle`: no turn to attach to — warn and discard

## Sidechain Conversations

Records with `isSidechain: true` represent subagent conversations that
run in parallel. In v0.1, these are **skipped entirely**. The assembler
checks `isSidechain` on every incoming record and discards sidechain
records without changing state.

Future versions (v0.5) will parse sidechains into `SubagentSession`
objects and link them to the parent turn via `parentToolUseId`.

## Orphaned Messages

An orphaned message is any record that appears in an unexpected state:

| Scenario | Handling |
|----------|----------|
| `assistant` in `Idle` (no preceding user) | Log warning, skip |
| `tool_result` in `Idle` (no preceding tool_use) | Log warning, skip |
| `progress` in `Idle` (no active turn) | Log warning, skip |
| `tool_result` with unknown `tool_use_id` | Log warning, attach to turn anyway |

Orphaned records are counted in `Session.parse_warnings` for display
in the TUI status bar.

## Incomplete Turns

A turn is incomplete when the session ends (EOF) before the assistant
responds, or before a tool result arrives. Handling:

| Scenario | Result |
|----------|--------|
| EOF in `AwaitingAssistant` | Emit turn with `assistant_response: None` (partial turn) |
| EOF in `AwaitingToolResult` | Emit turn with incomplete tool results |
| Consecutive user messages | Emit previous turn as incomplete, start new turn |

The `Turn` struct has an `is_complete: bool` field to flag these cases.
The TUI renders incomplete turns with a visual indicator.

## Record Types and Relevance

| Record Type | Relevant to Turn Assembly? | Notes |
|---|---|---|
| `user` | Yes | Starts a new turn |
| `assistant` | Yes | Completes or continues a turn |
| `tool_result` | Yes | Correlates with `tool_use` block |
| `progress` | Yes | Attaches as metadata |
| `system` | No | Contains turn_duration, hook summaries. Extracted in pass 1 for session metadata, not used in turn assembly. Useful for M4 aggregate stats. |
| `last-prompt` | No | Extracted in pass 1 (summary scan) to populate `SessionSummary.last_prompt`. Not processed by the turn assembler — this is a session-level metadata record, not a conversation message. |
| `summary` | No | Session-level metadata, skip |
| `file-history-snapshot` | No | Skip |
| `queue-operation` | No | Skip (captured as SessionEvent) |
| `lock` | No | Skip |
| Unknown types | No | Log debug, skip |

## Example: Normal 2-Turn Conversation

```
Record: user "hello"          → Idle → AwaitingAssistant
Record: assistant "hi there"  → AwaitingAssistant → TurnComplete
  (emit Turn 0)              → TurnComplete → Idle
Record: user "help me"        → Idle → AwaitingAssistant
Record: assistant "sure!"     → AwaitingAssistant → TurnComplete
  (emit Turn 1)              → TurnComplete → Idle
EOF                           → Idle (clean exit)
```

## Example: Turn with Tool Use

```
Record: user "read my file"        → Idle → AwaitingAssistant
Record: assistant [tool_use:Read]  → AwaitingAssistant → AwaitingToolResult
Record: tool_result "file content" → AwaitingToolResult (attach result)
Record: assistant "here it is"     → AwaitingToolResult → TurnComplete
  (emit Turn 0 with tool call)    → TurnComplete → Idle
```

## Example: Orphaned Progress

```
Record: progress (hook started)    → Idle: warn, skip
Record: user "hello"               → Idle → AwaitingAssistant
Record: progress (hook finished)   → AwaitingAssistant: attach to turn
Record: assistant "hi"             → AwaitingAssistant → TurnComplete
```

## Example: Consecutive Users

```
Record: user "first question"      → Idle → AwaitingAssistant
Record: user "actually, this"      → AwaitingAssistant: warn, emit
                                     incomplete Turn 0, store new user
                                     → AwaitingAssistant
Record: assistant "here you go"    → AwaitingAssistant → TurnComplete
  (emit Turn 1)
```

## Implementation Notes

- The assembler is a `struct TurnAssembler` with a `fn feed(&mut self,
  record: RawRecord) -> Option<Turn>` method. It returns `Some(Turn)`
  whenever a turn is complete.
- Call `fn finish(&mut self) -> Option<Turn>` at EOF to flush any
  incomplete turn.
- Warnings are collected in a `Vec<ParseWarning>` on the assembler
  and transferred to the `Session` after assembly.
- The assembler is stateful but has no I/O — it is purely a stream
  transformer and fully unit-testable.
