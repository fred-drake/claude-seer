// Help overlay widget.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::View;

use super::layout::centered_rect;

/// Build the help text descriptions for j/k keys based on current view.
fn jk_descriptions(view: &View) -> (&'static str, &'static str) {
    match view {
        View::ProjectList => ("Select next project", "Select previous project"),
        View::SessionList => ("Select next session", "Select previous session"),
        View::Conversation(_) => ("Next turn", "Previous turn"),
    }
}

/// Build the help lines for the overlay based on the current view.
fn build_help_lines(view: &View) -> Vec<Line<'static>> {
    let (j_desc, k_desc) = jk_descriptions(view);

    let mut lines = vec![
        Line::from(Span::styled(
            "Keybindings",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Navigation",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  j / Down   ", Style::default().fg(Color::Yellow)),
            Span::raw(j_desc),
        ]),
        Line::from(vec![
            Span::styled("  k / Up     ", Style::default().fg(Color::Yellow)),
            Span::raw(k_desc),
        ]),
    ];

    match view {
        View::ProjectList => {
            lines.push(Line::from(vec![
                Span::styled("  Enter      ", Style::default().fg(Color::Yellow)),
                Span::raw("Open project"),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Esc        ", Style::default().fg(Color::Yellow)),
                Span::raw("Quit"),
            ]));
        }
        View::SessionList => {
            lines.push(Line::from(vec![
                Span::styled("  Enter      ", Style::default().fg(Color::Yellow)),
                Span::raw("Open session"),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Esc        ", Style::default().fg(Color::Yellow)),
                Span::raw("Back to projects"),
            ]));
        }
        View::Conversation(_) => {
            lines.push(Line::from(vec![
                Span::styled("  Down/Up    ", Style::default().fg(Color::Yellow)),
                Span::raw("Scroll down / Scroll up"),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Esc        ", Style::default().fg(Color::Yellow)),
                Span::raw("Back to sessions"),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Conversation",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::styled("  u          ", Style::default().fg(Color::Yellow)),
                Span::raw("View user message"),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  c          ", Style::default().fg(Color::Yellow)),
                Span::raw("View claude response"),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  t          ", Style::default().fg(Color::Yellow)),
                Span::raw("Toggle token display"),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  o          ", Style::default().fg(Color::Yellow)),
                Span::raw("Toggle tool calls"),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  T          ", Style::default().fg(Color::Yellow)),
                Span::raw("Toggle thinking"),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  General",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("  q          ", Style::default().fg(Color::Yellow)),
        Span::raw("Quit"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Ctrl-C     ", Style::default().fg(Color::Yellow)),
        Span::raw("Quit"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  ?          ", Style::default().fg(Color::Yellow)),
        Span::raw("Toggle help"),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Press any key to close",
        Style::default().fg(Color::DarkGray),
    )));

    lines
}

/// Calculate the popup height based on help content and available area.
fn help_popup_height(view: &View, area_height: u16) -> u16 {
    let lines = build_help_lines(view);
    let content_height = lines.len() as u16 + 2; // +2 for top/bottom border
    content_height.min(area_height.saturating_sub(4))
}

/// Render the help overlay centered on screen.
pub fn render_help(frame: &mut Frame, area: Rect, view: &View) {
    let help_width = 50u16.min(area.width.saturating_sub(4));
    let help_height = help_popup_height(view, area.height);

    let popup_area = centered_rect(help_width, help_height, area);

    // Clear the area behind the popup.
    frame.render_widget(Clear, popup_area);

    let lines = build_help_lines(view);

    let help = Paragraph::new(lines).block(
        Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(help, popup_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::model::SessionId;

    /// Extract all raw text from help lines for assertion.
    fn help_text(view: &View) -> String {
        build_help_lines(view)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn session_list_help_shows_select_next_session() {
        let text = help_text(&View::SessionList);
        assert!(
            text.contains("Select next session"),
            "Expected 'Select next session', got:\n{text}"
        );
    }

    #[test]
    fn session_list_help_shows_select_previous_session() {
        let text = help_text(&View::SessionList);
        assert!(
            text.contains("Select previous session"),
            "Expected 'Select previous session', got:\n{text}"
        );
    }

    #[test]
    fn session_list_help_shows_open_session() {
        let text = help_text(&View::SessionList);
        assert!(
            text.contains("Open session"),
            "Expected 'Open session', got:\n{text}"
        );
    }

    #[test]
    fn session_list_help_does_not_show_turn_navigation() {
        let text = help_text(&View::SessionList);
        assert!(
            !text.contains("Next turn"),
            "Session list should not show turn nav, got:\n{text}"
        );
    }

    #[test]
    fn conversation_help_shows_jk_turn_navigation() {
        let view = View::Conversation(SessionId("test".to_string()));
        let text = help_text(&view);
        assert!(
            text.contains("Next turn"),
            "Expected 'Next turn', got:\n{text}"
        );
        assert!(
            text.contains("Previous turn"),
            "Expected 'Previous turn', got:\n{text}"
        );
    }

    #[test]
    fn conversation_help_shows_arrow_scroll() {
        let view = View::Conversation(SessionId("test".to_string()));
        let text = help_text(&view);
        assert!(
            text.contains("Scroll down"),
            "Expected 'Scroll down', got:\n{text}"
        );
        assert!(
            text.contains("Scroll up"),
            "Expected 'Scroll up', got:\n{text}"
        );
    }

    #[test]
    fn conversation_help_shows_modal_keys() {
        let view = View::Conversation(SessionId("test".to_string()));
        let text = help_text(&view);
        assert!(
            text.contains("View user message"),
            "Expected 'View user message', got:\n{text}"
        );
        assert!(
            text.contains("View claude response"),
            "Expected 'View claude response', got:\n{text}"
        );
    }

    #[test]
    fn conversation_help_shows_back_to_sessions() {
        let view = View::Conversation(SessionId("test".to_string()));
        let text = help_text(&view);
        assert!(
            text.contains("Back to sessions"),
            "Expected 'Back to sessions', got:\n{text}"
        );
    }

    #[test]
    fn help_popup_height_fits_session_list_content() {
        let lines = build_help_lines(&View::SessionList);
        let content_height = lines.len() as u16 + 2; // +2 for border
        let area_height = 40u16;
        let expected = content_height.min(area_height.saturating_sub(4));
        let actual = help_popup_height(&View::SessionList, area_height);
        assert_eq!(
            actual, expected,
            "Session list help height should match content, not be hardcoded at 24"
        );
        // Session list has fewer lines than 24, so it should be smaller
        assert!(
            actual < 24,
            "Session list help should be shorter than 24 lines, got {actual}"
        );
    }

    #[test]
    fn help_popup_height_capped_by_area() {
        // Even if content is tall, height should not exceed area.height - 4
        let area_height = 10u16;
        let actual =
            help_popup_height(&View::Conversation(SessionId("t".to_string())), area_height);
        assert!(
            actual <= area_height.saturating_sub(4),
            "Help height {actual} should be capped at area_height - 4 = {}",
            area_height.saturating_sub(4)
        );
    }

    #[test]
    fn help_popup_height_conversation_taller_than_session_list() {
        let session_h = help_popup_height(&View::SessionList, 40);
        let conv_h = help_popup_height(&View::Conversation(SessionId("t".to_string())), 40);
        assert!(
            conv_h > session_h,
            "Conversation help ({conv_h}) should be taller than session list help ({session_h})"
        );
    }

    #[test]
    fn all_views_show_quit_and_help() {
        for view in [
            View::ProjectList,
            View::SessionList,
            View::Conversation(SessionId("t".to_string())),
        ] {
            let text = help_text(&view);
            assert!(text.contains("Quit"), "Expected 'Quit' in {text}");
            assert!(
                text.contains("Toggle help"),
                "Expected 'Toggle help' in {text}"
            );
        }
    }

    #[test]
    fn conversation_help_shows_toggle_tools() {
        let view = View::Conversation(SessionId("test".to_string()));
        let text = help_text(&view);
        assert!(
            text.contains("Toggle tool calls"),
            "Expected 'Toggle tool calls', got:\n{text}"
        );
    }

    #[test]
    fn conversation_help_shows_toggle_thinking() {
        let view = View::Conversation(SessionId("test".to_string()));
        let text = help_text(&view);
        assert!(
            text.contains("Toggle thinking"),
            "Expected 'Toggle thinking', got:\n{text}"
        );
    }
}
