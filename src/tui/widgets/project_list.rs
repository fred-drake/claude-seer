// Project browser widget -- renders project list.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use crate::app::AppState;
use crate::data::model::{ProjectSummary, format_relative_time};

use super::text_utils::plural;

/// Render the project list into the given area.
pub fn render_project_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let items: Vec<ListItem> = state
        .projects
        .iter()
        .map(|project| {
            let is_cwd = state
                .cwd
                .as_ref()
                .is_some_and(|cwd| project.path.matches_cwd(cwd));

            let lines = format_project_lines(project, is_cwd);
            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title(" Projects ").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut list_state = ListState::default();
    if !state.projects.is_empty() {
        list_state.select(Some(state.project_selected_index));
    }

    frame.render_stateful_widget(list, area, &mut list_state);
}

/// Format a project as multi-line content for the list.
fn format_project_lines(project: &ProjectSummary, is_cwd: bool) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Line 1: Project name with optional CWD indicator.
    let name_style = if is_cwd {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };

    let name_text = if is_cwd {
        format!("  {} {}", '\u{25CF}', project.display_name)
    } else {
        format!("  {}", project.display_name)
    };
    lines.push(Line::from(Span::styled(name_text, name_style)));

    // Line 2: Decoded path (shortened with ~).
    let decoded = project.path.decoded_path();
    let shortened = shorten_path(&decoded);
    lines.push(Line::from(Span::styled(
        format!("  {shortened}"),
        Style::default().fg(Color::DarkGray),
    )));

    // Line 3: Session count + relative time.
    let session_label = plural(project.session_count, "session", "sessions");
    let time_part = project
        .last_activity
        .map(|dt| format!(" \u{00B7} {}", format_relative_time(dt)))
        .unwrap_or_default();
    lines.push(Line::from(Span::styled(
        format!("  {} {session_label}{time_part}", project.session_count),
        Style::default().fg(Color::Gray),
    )));

    // Line 4: Blank separator.
    lines.push(Line::from(""));

    lines
}

/// Shorten a decoded path for display — replace home dir with ~, left-truncate if >60 chars.
fn shorten_path(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    let shortened = if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        if let Some(rest) = s.strip_prefix(home_str.as_ref()) {
            format!("~{rest}")
        } else {
            s.to_string()
        }
    } else {
        s.to_string()
    };

    if shortened.len() > 60 {
        // Find a safe char boundary for truncation (avoid panic on multi-byte UTF-8).
        let target = shortened.len() - 57;
        let safe_start = shortened
            .char_indices()
            .map(|(i, _)| i)
            .find(|&i| i >= target)
            .unwrap_or(target);
        format!("...{}", &shortened[safe_start..])
    } else {
        shortened
    }
}
