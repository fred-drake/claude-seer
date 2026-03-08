// Status bar widget.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::AppState;

/// Render the status bar into the given area.
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let session_count = state.sessions.len();
    let status_text = format!(" {} session(s) | Press ? for help", session_count);

    let line = Line::from(vec![Span::styled(
        status_text,
        Style::default().fg(Color::White).bg(Color::DarkGray),
    )]);

    // Pad to fill width.
    let padding = " ".repeat(area.width.saturating_sub(line.width() as u16) as usize);
    let padded = Line::from(vec![
        line.spans.into_iter().next().unwrap_or_default(),
        Span::styled(padding, Style::default().bg(Color::DarkGray)),
    ]);

    let bar = Paragraph::new(padded);
    frame.render_widget(bar, area);
}
