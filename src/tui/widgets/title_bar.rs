// Title bar widget -- shows Claude Code version and usage stats.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::AppState;

/// Build the left-side text (app name + version).
fn build_left_spans(state: &AppState) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(
        " claude-seer",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];

    if let Some(ref version) = state.title_bar.claude_version {
        spans.push(Span::styled(
            format!(" (Claude Code v{version})"),
            Style::default().fg(Color::Gray),
        ));
    }

    spans
}

/// Pick a color based on utilization level.
fn usage_color(utilization: f64) -> Color {
    if utilization >= 80.0 {
        Color::Red
    } else if utilization >= 50.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

/// Build the right-side spans with color-coded usage values.
fn build_right_spans(state: &AppState) -> Vec<Span<'static>> {
    let Some(ref usage) = state.title_bar.usage else {
        return Vec::new();
    };

    let mut spans = Vec::new();

    if let Some(ref five_hour) = usage.five_hour {
        spans.push(Span::styled("5h: ", Style::default().fg(Color::Gray)));
        spans.push(Span::styled(
            format!("{:.0}%", five_hour.utilization),
            Style::default().fg(usage_color(five_hour.utilization)),
        ));
    }

    if let Some(ref seven_day) = usage.seven_day {
        if !spans.is_empty() {
            spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
        }
        spans.push(Span::styled("7d: ", Style::default().fg(Color::Gray)));
        spans.push(Span::styled(
            format!("{:.0}%", seven_day.utilization),
            Style::default().fg(usage_color(seven_day.utilization)),
        ));
    }

    if !spans.is_empty() {
        spans.insert(0, Span::styled("Usage: ", Style::default().fg(Color::Gray)));
        spans.push(Span::raw(" "));
    }

    spans
}

/// Build spans showing enabled display options (e.g. "+tool +think +token").
fn build_display_option_spans(state: &AppState) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let opts = &state.display;

    let flags: &[(&str, bool)] = &[
        ("+tool", opts.show_tools),
        ("+think", opts.show_thinking),
        ("+token", opts.show_tokens),
    ];

    for &(label, enabled) in flags {
        if enabled {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                label,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    spans
}

/// Render the title bar into the given area.
pub fn render_title_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let left_spans = build_left_spans(state);
    let display_spans = build_display_option_spans(state);
    let right_spans = build_right_spans(state);

    // Calculate widths using Unicode-aware width.
    let left_width: usize = left_spans.iter().map(|s| s.width()).sum();
    let display_width: usize = display_spans.iter().map(|s| s.width()).sum();
    let right_width: usize = right_spans.iter().map(|s| s.width()).sum();
    let total_width = area.width as usize;

    // Build the full line with padding between left and right.
    let mut spans = left_spans;
    spans.extend(display_spans);
    let padding_len = total_width.saturating_sub(left_width + display_width + right_width);
    spans.push(Span::styled(
        " ".repeat(padding_len),
        Style::default().bg(Color::DarkGray),
    ));
    spans.extend(right_spans);

    let line = Line::from(spans);
    let bar = Paragraph::new(line).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(bar, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::usage::{UsageData, UsageWindow};

    #[test]
    fn left_spans_show_app_name() {
        let state = AppState::new();
        let spans = build_left_spans(&state);
        let text: String = spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.contains("claude-seer"));
    }

    #[test]
    fn left_spans_include_version_when_set() {
        let mut state = AppState::new();
        state.title_bar.claude_version = Some("2.1.71".to_string());
        let spans = build_left_spans(&state);
        let text: String = spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.contains("v2.1.71"));
    }

    #[test]
    fn left_spans_no_version_when_unset() {
        let state = AppState::new();
        let spans = build_left_spans(&state);
        let text: String = spans.iter().map(|s| s.content.to_string()).collect();
        assert!(!text.contains("Claude Code"));
    }

    fn spans_to_text(spans: &[Span]) -> String {
        spans.iter().map(|s| s.content.to_string()).collect()
    }

    #[test]
    fn right_spans_empty_when_no_usage_data() {
        let state = AppState::new();
        let spans = build_right_spans(&state);
        assert!(spans.is_empty());
    }

    #[test]
    fn right_spans_shows_five_hour_usage() {
        let mut state = AppState::new();
        state.title_bar.usage = Some(UsageData {
            five_hour: Some(UsageWindow {
                utilization: 42.0,
                resets_at: None,
            }),
            seven_day: None,
            seven_day_opus: None,
        });
        let text = spans_to_text(&build_right_spans(&state));
        assert!(text.contains("5h: "), "got: {text}");
        assert!(text.contains("42%"), "got: {text}");
    }

    #[test]
    fn right_spans_shows_both_windows() {
        let mut state = AppState::new();
        state.title_bar.usage = Some(UsageData {
            five_hour: Some(UsageWindow {
                utilization: 10.0,
                resets_at: None,
            }),
            seven_day: Some(UsageWindow {
                utilization: 55.0,
                resets_at: None,
            }),
            seven_day_opus: None,
        });
        let text = spans_to_text(&build_right_spans(&state));
        assert!(text.contains("10%"), "got: {text}");
        assert!(text.contains("55%"), "got: {text}");
    }

    #[test]
    fn usage_color_green_for_low() {
        assert_eq!(usage_color(0.0), Color::Green);
        assert_eq!(usage_color(49.0), Color::Green);
    }

    #[test]
    fn usage_color_yellow_for_medium() {
        assert_eq!(usage_color(50.0), Color::Yellow);
        assert_eq!(usage_color(79.0), Color::Yellow);
    }

    #[test]
    fn usage_color_red_for_high() {
        assert_eq!(usage_color(80.0), Color::Red);
        assert_eq!(usage_color(100.0), Color::Red);
    }

    #[test]
    fn display_options_shows_tool_when_enabled() {
        let mut state = AppState::new();
        state.display.show_tools = true;
        let spans = build_display_option_spans(&state);
        let text = spans_to_text(&spans);
        assert!(text.contains("+tool"), "got: {text}");
    }

    #[test]
    fn display_options_shows_think_when_enabled() {
        let mut state = AppState::new();
        state.display.show_thinking = true;
        let spans = build_display_option_spans(&state);
        let text = spans_to_text(&spans);
        assert!(text.contains("+think"), "got: {text}");
    }

    #[test]
    fn display_options_shows_token_when_enabled() {
        let mut state = AppState::new();
        state.display.show_tokens = true;
        let spans = build_display_option_spans(&state);
        let text = spans_to_text(&spans);
        assert!(text.contains("+token"), "got: {text}");
    }

    #[test]
    fn display_options_empty_when_none_enabled() {
        let state = AppState::new();
        let spans = build_display_option_spans(&state);
        assert!(spans.is_empty());
    }

    #[test]
    fn display_options_shows_multiple() {
        let mut state = AppState::new();
        state.display.show_tools = true;
        state.display.show_tokens = true;
        let spans = build_display_option_spans(&state);
        let text = spans_to_text(&spans);
        assert!(text.contains("+tool"), "got: {text}");
        assert!(text.contains("+token"), "got: {text}");
    }

    #[test]
    fn right_spans_include_usage_label() {
        let mut state = AppState::new();
        state.title_bar.usage = Some(UsageData {
            five_hour: Some(UsageWindow {
                utilization: 25.0,
                resets_at: None,
            }),
            seven_day: None,
            seven_day_opus: None,
        });
        let spans = build_right_spans(&state);
        let text: String = spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.contains("Usage:"), "got: {text}");
    }
}
