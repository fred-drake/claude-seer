# Reusable TUI Patterns

## Responsive Breakpoints
```
Width < 60:   Single panel (no session sidebar), abbreviated token bar
Width 60-100: Session panel 20 chars, normal layout
Width > 100:  Session panel 25 chars, compare mode practical
Height < 20:  Hide token summary bar
Height >= 20: Full layout
```

## Overlay Pattern (Command Palette, Help)
1. Render normal background (dimmed)
2. Use Clear widget on centered rect
3. Render Block with border on that rect
4. Content inside: input field (Paragraph) + results (List)

## Panel Focus Pattern
- Only one panel receives input at a time
- Visual indicator: border color changes (Blue=focused, DarkGray=unfocused)
- Event handling: check global keys first, then delegate to focused panel

## Scrollable Content Pattern
- Use ScrollbarState alongside content state
- Track: selected_index, scroll_offset, visible_height
- Scrollbar widget rendered on right edge of content area

## Expandable Item Pattern (messages, tool calls)
- Track expanded state as HashSet<ItemId> in view state
- Collapsed: single line summary (dimmed/italic)
- Expanded: full content rendered inline
- Enter toggles, Space toggles tool calls specifically

## MVP Phase Order
1. Session list + message thread + basic navigation
2. Tool call expansion + token summary bar
3. Token attribution + tool detail view + search
4. Compaction + agent tree + compare + notifications
