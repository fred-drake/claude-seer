// Project browser widget -- renders project list.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use unicode_width::UnicodeWidthStr;

use crate::app::AppState;
use crate::data::model::{ProjectSummary, format_relative_time};

use super::text_utils::plural;

/// Render the project list into the given area.
pub fn render_project_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let items: Vec<ListItem> = state
        .projects
        .iter()
        .enumerate()
        .map(|(i, project)| {
            let is_cwd = state
                .cwd
                .as_ref()
                .is_some_and(|cwd| project.path.matches_cwd(cwd));
            let is_selected = i == state.project_selected_index;

            let available_width = area.width.saturating_sub(5);
            let lines = format_project_lines(project, is_cwd, is_selected, available_width);
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
///
/// When `available_width` is wide enough to fit name + path on a single line,
/// they are combined. Otherwise, name and path are on separate lines.
fn format_project_lines(
    project: &ProjectSummary,
    is_cwd: bool,
    is_selected: bool,
    available_width: u16,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let name_style = if is_cwd {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };

    // Path color: Black when highlighted (DarkGray bg), DarkGray otherwise.
    let path_color = if is_selected {
        Color::Black
    } else {
        Color::DarkGray
    };

    let name_text = if is_cwd {
        format!("  {} {}", '\u{25CF}', project.display_name)
    } else {
        format!("  {}", project.display_name)
    };

    let decoded = project.path.decoded_path();
    let shortened = shorten_path(&decoded);
    let separator = "  ";
    let combined_width = name_text.width() + separator.width() + shortened.width();

    if combined_width <= available_width as usize {
        // Single-line layout: name + path on one line.
        lines.push(Line::from(vec![
            Span::styled(name_text, name_style),
            Span::styled(separator.to_string(), Style::default()),
            Span::styled(shortened, Style::default().fg(path_color)),
        ]));
    } else {
        // Two-line layout: name and path on separate lines.
        lines.push(Line::from(Span::styled(name_text, name_style)));
        lines.push(Line::from(Span::styled(
            format!("  {shortened}"),
            Style::default().fg(path_color),
        )));
    }

    // Session count + relative time.
    let session_label = plural(project.session_count, "session", "sessions");
    let time_part = project
        .last_activity
        .map(|dt| format!(" \u{00B7} {}", format_relative_time(dt)))
        .unwrap_or_default();
    lines.push(Line::from(Span::styled(
        format!("  {} {session_label}{time_part}", project.session_count),
        Style::default().fg(Color::Gray),
    )));

    // Blank separator.
    lines.push(Line::from(""));

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::model::{ProjectPath, ProjectSummary};
    use std::path::PathBuf;

    fn make_project(name: &str, path: &str) -> ProjectSummary {
        ProjectSummary {
            path: ProjectPath(PathBuf::from(path)),
            display_name: name.to_string(),
            session_count: 5,
            last_activity: None,
        }
    }

    #[test]
    fn narrow_terminal_keeps_name_and_path_on_separate_lines() {
        let project = make_project("seer", "-Users-fdrake-Source-claude-seer");
        let lines = format_project_lines(&project, false, false, 30);
        // Should be 4 lines: name, path, stats, blank
        assert_eq!(lines.len(), 4);
        let name_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        let path_text: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(name_text.contains("seer"));
        assert!(!name_text.contains("/Users"));
        assert!(path_text.contains("/Users"));
    }

    #[test]
    fn exact_boundary_width_uses_single_line_layout() {
        let project = make_project("seer", "-Users-fdrake-Source-claude-seer");
        // "  seer" (6) + "  " (2) + "/Users/fdrake/Source/claude/seer" (32) = 40
        let lines_at_boundary = format_project_lines(&project, false, false, 40);
        assert_eq!(
            lines_at_boundary.len(),
            3,
            "should use single-line at exact fit"
        );

        let lines_one_less = format_project_lines(&project, false, false, 39);
        assert_eq!(
            lines_one_less.len(),
            4,
            "should use two-line when one char short"
        );
    }

    #[test]
    fn cwd_indicator_affects_boundary_calculation() {
        let project = make_project("seer", "-Users-fdrake-Source-claude-seer");
        // "  ● seer" (8) + "  " (2) + "/Users/fdrake/Source/claude/seer" (32) = 42
        let lines_at_boundary = format_project_lines(&project, true, false, 42);
        assert_eq!(lines_at_boundary.len(), 3, "CWD single-line at exact fit");

        let lines_one_less = format_project_lines(&project, true, false, 41);
        assert_eq!(lines_one_less.len(), 4, "CWD two-line when one char short");
    }

    #[test]
    fn selected_item_uses_black_path_color() {
        let project = make_project("seer", "-Users-fdrake-Source-claude-seer");
        // Wide layout (single line): path span should be Black when selected
        let lines = format_project_lines(&project, false, true, 120);
        let path_span = &lines[0].spans[2];
        assert_eq!(path_span.style.fg, Some(Color::Black));

        // Unselected: path span should be DarkGray
        let lines = format_project_lines(&project, false, false, 120);
        let path_span = &lines[0].spans[2];
        assert_eq!(path_span.style.fg, Some(Color::DarkGray));

        // Narrow layout (two lines): path line should be Black when selected
        let lines = format_project_lines(&project, false, true, 30);
        let path_span = &lines[1].spans[0];
        assert_eq!(path_span.style.fg, Some(Color::Black));
    }

    #[test]
    fn wide_terminal_combines_name_and_path_on_one_line() {
        let project = make_project("seer", "-Users-fdrake-Source-claude-seer");
        let lines = format_project_lines(&project, false, false, 120);
        // Should be 3 lines: combined name+path, stats, blank
        assert_eq!(lines.len(), 3);
        // First line should contain both name and path
        let first_line_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(first_line_text.contains("seer"));
        assert!(first_line_text.contains("/Users/fdrake/Source/claude/seer"));
    }
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
