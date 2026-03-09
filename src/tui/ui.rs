// Root layout dispatcher -- decides what to render based on app state.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::app::{AppState, View};
use crate::tui::widgets::{conversation, empty_state, help, session_list, status_bar, title_bar};

/// Render the entire UI based on current app state.
pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // Main layout: title bar + content area + status bar.
    let chunks = Layout::vertical([
        Constraint::Length(1), // title bar
        Constraint::Min(1),    // content
        Constraint::Length(1), // status bar
    ])
    .split(area);

    let title_area = chunks[0];
    let content_area = chunks[1];
    let status_area = chunks[2];

    // Title bar always visible.
    title_bar::render_title_bar(frame, title_area, state);

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
            if let Some(ref empty) = state.empty_state {
                empty_state::render_empty_state(frame, content_area, empty);
            } else {
                conversation::render_conversation(frame, content_area, state);
            }
        }
    }

    // Status bar always visible.
    status_bar::render_status_bar(frame, status_area, state);

    // Help overlay on top of everything.
    if state.show_help {
        help::render_help(frame, area, &state.view);
    }
}
