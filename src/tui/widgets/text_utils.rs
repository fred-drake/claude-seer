// Shared text utilities for TUI widgets.

use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

/// Truncate a string to `max_len` characters, appending "..." if truncated.
pub fn truncate_end(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .nth(max_len.saturating_sub(3))
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}...", &s[..end])
    }
}

/// Truncate a string to fit within `max_cols` display columns, appending
/// the Unicode ellipsis character `…` (1 column) if it exceeds the limit.
///
/// Returns the original string unchanged if it fits. When `max_cols` is 0,
/// returns an empty string.
pub fn truncate_to_width(text: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    let width = UnicodeWidthStr::width(text);
    if width <= max_cols {
        return text.to_string();
    }
    // Reserve 1 column for the ellipsis.
    let target = max_cols.saturating_sub(1);
    let mut result = String::new();
    let mut current_width: usize = 0;
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + ch_width > target {
            break;
        }
        result.push(ch);
        current_width += ch_width;
    }
    result.push('\u{2026}'); // …
    result
}

/// Return the correct singular or plural label for a count.
pub fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_end_short_string_unchanged() {
        assert_eq!(truncate_end("hello", 10), "hello");
    }

    #[test]
    fn truncate_end_exact_length_unchanged() {
        assert_eq!(truncate_end("hello", 5), "hello");
    }

    #[test]
    fn truncate_end_long_string_truncated() {
        let result = truncate_end("hello world!", 8);
        assert!(result.ends_with("..."));
        assert!(result.chars().count() <= 8);
    }

    #[test]
    fn truncate_end_handles_multibyte() {
        let s = "aaaa\u{1F600}bbbb"; // emoji is multi-byte
        let result = truncate_end(s, 6);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn plural_singular() {
        assert_eq!(plural(1, "session", "sessions"), "session");
    }

    #[test]
    fn plural_zero_is_plural() {
        assert_eq!(plural(0, "session", "sessions"), "sessions");
    }

    #[test]
    fn plural_many() {
        assert_eq!(plural(5, "warning", "warnings"), "warnings");
    }

    // --- truncate_to_width tests ---

    #[test]
    fn truncate_to_width_short_text_unchanged() {
        assert_eq!(truncate_to_width("hello", 10), "hello");
    }

    #[test]
    fn truncate_to_width_long_text_gets_ellipsis() {
        let result = truncate_to_width("a very long tool path here", 15);
        assert!(
            result.ends_with('\u{2026}'),
            "Should end with ellipsis: {result}"
        );
        assert!(
            UnicodeWidthStr::width(result.as_str()) <= 15,
            "Should fit in 15 cols: {result}"
        );
    }

    #[test]
    fn truncate_to_width_exact_fit_unchanged() {
        assert_eq!(truncate_to_width("12345", 5), "12345");
    }

    #[test]
    fn truncate_to_width_empty_string_returns_empty() {
        assert_eq!(truncate_to_width("", 10), "");
    }

    #[test]
    fn truncate_to_width_max_cols_zero_returns_empty() {
        assert_eq!(truncate_to_width("hello", 0), "");
    }

    #[test]
    fn truncate_to_width_max_cols_one_returns_ellipsis_for_long_text() {
        let result = truncate_to_width("hello", 1);
        assert_eq!(result, "\u{2026}");
        assert_eq!(UnicodeWidthStr::width(result.as_str()), 1);
    }

    #[test]
    fn truncate_to_width_max_cols_one_returns_char_for_single_char() {
        assert_eq!(truncate_to_width("h", 1), "h");
    }
}
