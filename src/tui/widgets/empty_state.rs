// Empty state widgets -- guidance messages for various empty states.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::EmptyState;

/// Render the appropriate empty state message.
pub fn render_empty_state(frame: &mut Frame, area: Rect, empty_state: &EmptyState) {
    match empty_state {
        EmptyState::Loading => render_loading(frame, area),
        EmptyState::NoDirectory => render_no_directory(frame, area),
        EmptyState::NoSessions => render_no_sessions(frame, area),
        EmptyState::EmptySession => render_empty_session(frame, area),
    }
}

fn render_loading(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Loading sessions...",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Scanning ~/.claude/projects/ for session data.",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default().title(" Sessions ").borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn render_no_directory(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  No Claude Code data found",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  claude-seer reads session logs from ~/.claude/projects/"),
        Line::from(""),
        Line::from("  To get started:"),
        Line::from(Span::styled(
            "  1. Install Claude Code (https://docs.anthropic.com/en/docs/claude-code)",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            "  2. Run a few sessions with Claude Code",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            "  3. Re-launch claude-seer",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Or use --path to point to a custom data directory.",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default().title(" Sessions ").borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn render_no_sessions(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  No sessions found",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  The projects directory exists but contains no session files."),
        Line::from(""),
        Line::from("  Run a Claude Code session to generate session data,"),
        Line::from("  then re-launch claude-seer."),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default().title(" Sessions ").borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn render_empty_session(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Session contains no conversation turns",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Press Esc to go back.",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}
