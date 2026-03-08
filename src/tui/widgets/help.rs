// Help overlay widget.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::layout::centered_rect;

/// Render the help overlay centered on screen.
pub fn render_help(frame: &mut Frame, area: Rect) {
    let help_width = 50u16.min(area.width.saturating_sub(4));
    let help_height = 24u16.min(area.height.saturating_sub(4));

    let popup_area = centered_rect(help_width, help_height, area);

    // Clear the area behind the popup.
    frame.render_widget(Clear, popup_area);

    let lines = vec![
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
            Span::raw("Move down / Scroll down"),
        ]),
        Line::from(vec![
            Span::styled("  k / Up     ", Style::default().fg(Color::Yellow)),
            Span::raw("Move up / Scroll up"),
        ]),
        Line::from(vec![
            Span::styled("  Enter      ", Style::default().fg(Color::Yellow)),
            Span::raw("Open session"),
        ]),
        Line::from(vec![
            Span::styled("  Esc        ", Style::default().fg(Color::Yellow)),
            Span::raw("Back / Close"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Conversation",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  n          ", Style::default().fg(Color::Yellow)),
            Span::raw("Next turn"),
        ]),
        Line::from(vec![
            Span::styled("  N          ", Style::default().fg(Color::Yellow)),
            Span::raw("Previous turn"),
        ]),
        Line::from(vec![
            Span::styled("  t          ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle token display (on by default)"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  General",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  q          ", Style::default().fg(Color::Yellow)),
            Span::raw("Quit"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl-C     ", Style::default().fg(Color::Yellow)),
            Span::raw("Quit"),
        ]),
        Line::from(vec![
            Span::styled("  ?          ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle help"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Press any key to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let help = Paragraph::new(lines).block(
        Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(help, popup_area);
}
