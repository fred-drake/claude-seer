// Shared text utilities for TUI widgets.

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
}
