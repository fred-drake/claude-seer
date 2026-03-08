// Root layout dispatcher -- decides what to render based on app state.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::app::{AppState, View};
use crate::tui::widgets::{empty_state, help, session_list, status_bar};

/// Render the entire UI based on current app state.
pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // Main layout: content area + status bar.
    let chunks = Layout::vertical([
        Constraint::Min(1),    // content
        Constraint::Length(1), // status bar
    ])
    .split(area);

    let content_area = chunks[0];
    let status_area = chunks[1];

    // Render content based on view + empty state.
    match &state.view {
        View::SessionList => {
            if let Some(ref empty) = state.empty_state {
                empty_state::render_empty_state(frame, content_area, empty);
            } else {
                session_list::render_session_list(frame, content_area, state);
            }
        }
        View::Conversation(_session_id) => {
            // Placeholder -- will be implemented in M3.
            if let Some(ref empty) = state.empty_state {
                empty_state::render_empty_state(frame, content_area, empty);
            }
        }
    }

    // Status bar always visible.
    status_bar::render_status_bar(frame, status_area, state);

    // Help overlay on top of everything.
    if state.show_help {
        help::render_help(frame, area);
    }
}
