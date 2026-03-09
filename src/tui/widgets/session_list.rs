// Session browser widget -- renders session list scoped to one project.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use crate::app::AppState;
use crate::data::model::SessionSummary;

/// Render the session list into the given area.
pub fn render_session_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let items: Vec<ListItem> = state
        .sessions
        .iter()
        .map(|session| ListItem::new(format_session_line(session)))
        .collect();

    let title = if let Some(ref project) = state.selected_project {
        let name = project.display_name();
        let count = state.sessions.len();
        format!(" {} \u{2014} {} sessions ", name, count)
    } else {
        " Sessions ".to_string()
    };

    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut list_state = ListState::default();
    if !state.sessions.is_empty() {
        list_state.select(Some(state.selected_index));
    }

    frame.render_stateful_widget(list, area, &mut list_state);
}

/// Format a single session line for the list.
fn format_session_line(session: &SessionSummary) -> Line<'static> {
    let mut spans = Vec::new();

    spans.push(Span::raw(" "));

    // Date/time.
    if let Some(ts) = session.last_activity {
        let formatted = ts.format("%Y-%m-%d %H:%M").to_string();
        spans.push(Span::styled(formatted, Style::default().fg(Color::Yellow)));
    } else {
        spans.push(Span::styled(
            "                ",
            Style::default().fg(Color::DarkGray),
        ));
    }

    spans.push(Span::raw("  "));

    // Turn count.
    spans.push(Span::styled(
        format!("{:>3} turns", session.turn_count),
        Style::default().fg(Color::Green),
    ));

    spans.push(Span::raw("  "));

    // Branch name.
    if let Some(ref branch) = session.git_branch {
        spans.push(Span::styled(
            format!("[{}]", branch),
            Style::default().fg(Color::Magenta),
        ));
        spans.push(Span::raw(" "));
    }

    // Last prompt (title).
    if let Some(ref prompt) = session.last_prompt {
        let truncated = if prompt.chars().count() > 60 {
            let s: String = prompt.chars().take(57).collect();
            format!("{s}...")
        } else {
            prompt.clone()
        };
        spans.push(Span::styled(truncated, Style::default().fg(Color::White)));
    }

    Line::from(spans)
}
