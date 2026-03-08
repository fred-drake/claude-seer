// Status bar widget.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{AppState, View};

/// Render the status bar into the given area.
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let status_text = match &state.view {
        View::SessionList => {
            let session_count = state.sessions.len();
            if let Some(ref err) = state.last_error {
                format!(" {} session(s) | Error: {} | ? help", session_count, err)
            } else {
                format!(" {} session(s) | Press ? for help", session_count)
            }
        }
        View::Conversation(_) => {
            if let Some(ref session) = state.current_session {
                let total = session.turns.len();
                if total == 0 {
                    " Empty session | Esc: back | ? help".to_string()
                } else {
                    format!(
                        " Turn {}/{} | n/N: jump turns | j/k: scroll | Esc: back | ? help",
                        state.current_turn_index + 1,
                        total
                    )
                }
            } else {
                " Loading session... | Esc: back | ? help".to_string()
            }
        }
    };

    let line = Line::from(vec![Span::styled(
        status_text,
        Style::default().fg(Color::White).bg(Color::DarkGray),
    )]);

    // Pad to fill width.
    let line_width = u16::try_from(line.width()).unwrap_or(area.width);
    let padding = " ".repeat(area.width.saturating_sub(line_width) as usize);
    let padded = Line::from(vec![
        line.spans.into_iter().next().unwrap_or_default(),
        Span::styled(padding, Style::default().bg(Color::DarkGray)),
    ]);

    let bar = Paragraph::new(padded);
    frame.render_widget(bar, area);
}
