// Shared layout utilities for TUI widgets.

use ratatui::layout::{Constraint, Flex, Layout, Rect};

/// Create a centered Rect of given width and height within the outer Rect.
pub fn centered_rect(width: u16, height: u16, outer: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(outer);

    Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0])[0]
}
