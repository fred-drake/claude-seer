// Session browser widget -- renders grouped session list.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use crate::app::AppState;
use crate::data::model::SessionSummary;

/// Render the session list into the given area.
pub fn render_session_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let groups = state.grouped_sessions();

    let mut items: Vec<ListItem> = Vec::new();
    let mut list_index = 0;
    let mut selected_list_index = None;

    for (project, sessions) in &groups {
        // Project header.
        let decoded = project.decoded_path();
        let header = Line::from(vec![Span::styled(
            format!(" {} ", decoded.display()),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]);
        items.push(ListItem::new(header));
        list_index += 1;

        // Sessions under this project.
        for session in sessions {
            let is_selected = state
                .sessions
                .get(state.selected_index)
                .is_some_and(|s| s.id == session.id);
            if is_selected {
                selected_list_index = Some(list_index);
            }

            let line = format_session_line(session);
            items.push(ListItem::new(line));
            list_index += 1;
        }
    }

    let list = List::new(items)
        .block(Block::default().title(" Sessions ").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut list_state = ListState::default();
    list_state.select(selected_list_index);

    frame.render_stateful_widget(list, area, &mut list_state);
}

/// Format a single session line for the list.
fn format_session_line(session: &SessionSummary) -> Line<'static> {
    let mut spans = Vec::new();

    // Indent for nesting under project header.
    spans.push(Span::raw("   "));

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
