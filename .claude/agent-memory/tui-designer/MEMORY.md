# TUI Designer Memory

## Project Stack
- Rust 2024 edition, ratatui 0.29, crossterm 0.28
- Error handling: thiserror + miette
- Diagnostics: tracing + tracing-subscriber

## Established Layout Pattern
- Two-panel default: session list (left, fixed width) + content (right, flexible)
- Session panel hides at terminal width < 60
- Vertical: header(1) + body(min 10) + token bar(2) + keyhints(1)
- Detail view replaces message view contextually (Esc to go back)
- Modal overlays for command palette and help (centered, with Clear widget)

## Navigation Conventions
- Vim-style: j/k, g/G, Ctrl-d/Ctrl-u, arrow key aliases
- Tab/Shift-Tab cycles focus between panels
- Enter expands/selects, Esc goes back, q quits
- View modes: t=tokens, c=compaction, a=agents, s=compare, n=notifications
- / or Ctrl-k opens command palette (modal search)

## Color Conventions
- Focused border: Blue, unfocused: DarkGray
- Roles: Cyan=[H]uman, Green=[A]ssistant, Yellow=[S]ystem
- Tools: Magenta=name, Green=success, Red=error, Yellow=pending
- Tokens (7 categories): Blue/Green/Yellow/Cyan/Magenta/Red/White
- Diffs: Green=add, Red=remove, DarkGray=context
- Red is reserved for errors only

## Widget Choices
- Session list: ratatui List + ListState
- Message thread: custom StatefulWidget (Paragraph blocks + Scrollbar)
- Token bars: BarChart
- Compaction: Gauge + Paragraph
- Agent tree: custom indented Paragraph with box-drawing chars
- Command palette: overlay with Paragraph input + List results
- All borders: Block with title

## State Architecture
- AppMode enum: Normal, Search, Help, Compare
- FocusPanel enum: Sessions, Messages, Detail
- ViewMode enum: Messages, Tokens, Compaction, AgentTree, Notifications
- Global keys handled first, then delegated to focused panel handler

## Roadmap Review Decisions (2026-03-08)
- **Tool detail (v0.2)**: One-line summary in conversation + pane replacement on Enter. No inline expansion, no overlay, no 5-line preview. Start simple, add polish later.
- **Version split**: v0.2=tool inspector+search, v0.3=tokens+compaction, v0.4=cross-session, v0.5=advanced
- **Empty states**: Implement in M2 (not M5). 4 states: no ~/.claude/, no sessions, loading (static text), empty session
- **Error types**: Skeleton enums in M1 so TUI can render errors from day one
- **Loading indicator**: Static "Loading session..." text, not animated spinner (deterministic for snapshot tests)
- **Tool calls without results**: Show header + "(no output captured)"

## Design Files
- Full design doc: see conversation or docs/TUI_DESIGN.md when created
- Patterns file: patterns.md (widget reuse, responsive breakpoints)
