// Markdown-to-styled-lines renderer for chat bubbles.
//
// Converts markdown text into word-wrapped, styled `BubbleLine`s using
// pulldown-cmark for parsing and ratatui `Span`s for styling.

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use super::text_utils::truncate_to_width;

/// A single line of content to be placed inside a chat bubble.
///
/// Each line is composed of styled spans and a pre-computed display width.
#[derive(Debug, Clone)]
pub struct BubbleLine {
    pub spans: Vec<Span<'static>>,
    pub display_width: usize,
}

impl BubbleLine {
    /// Create a `BubbleLine` from a plain string with a single style.
    pub fn plain(text: String, style: Style) -> Self {
        let display_width = unicode_width::UnicodeWidthStr::width(text.as_str());
        Self {
            spans: vec![Span::styled(text, style)],
            display_width,
        }
    }

    /// Create a `BubbleLine` from pre-built spans.
    pub fn rich(spans: Vec<Span<'static>>) -> Self {
        let display_width = spans
            .iter()
            .map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
            .sum();
        Self {
            spans,
            display_width,
        }
    }
}

/// Convert markdown text into wrapped, styled lines for the bubble renderer.
///
/// Parses `text` as markdown, applies style modifiers (bold, italic, code),
/// and word-wraps to fit within `max_cols` columns. The `base_style` is used
/// as the foundation; markdown modifiers are layered on top.
pub fn markdown_wrap(
    text: &str,
    max_cols: usize,
    base_style: Style,
    is_current_turn: bool,
) -> Vec<BubbleLine> {
    if max_cols == 0 {
        return vec![BubbleLine::plain(String::new(), base_style)];
    }

    let mut builder = MdWrapBuilder::new(max_cols, base_style, is_current_turn);
    builder.process(text);
    builder.finish()
}

/// Single-pass builder that walks pulldown-cmark events and produces
/// word-wrapped, styled `BubbleLine`s.
struct MdWrapBuilder {
    max_cols: usize,
    base_style: Style,
    is_current_turn: bool,
    /// Stack of style modifiers (pushed on Start, popped on End).
    style_stack: Vec<Style>,
    /// Completed lines.
    lines: Vec<BubbleLine>,
    /// Spans accumulated for the current line.
    current_spans: Vec<Span<'static>>,
    /// Display width of current line so far.
    current_width: usize,
    /// Whether we are inside a fenced/indented code block.
    in_code_block: bool,
    /// Stack of list contexts (supports nested lists).
    list_stack: Vec<ListContext>,
    /// Prefix to prepend to the next text content (for list items).
    pending_prefix: Option<String>,
    /// Continuation indent width for wrapped list items.
    continuation_indent: usize,
}

/// Context for tracking list numbering.
#[derive(Debug, Clone)]
enum ListContext {
    Unordered,
    Ordered { next_number: u64 },
}

impl MdWrapBuilder {
    fn new(max_cols: usize, base_style: Style, is_current_turn: bool) -> Self {
        Self {
            max_cols,
            base_style,
            is_current_turn,
            style_stack: Vec::new(),
            lines: Vec::new(),
            current_spans: Vec::new(),
            current_width: 0,
            in_code_block: false,
            list_stack: Vec::new(),
            pending_prefix: None,
            continuation_indent: 0,
        }
    }

    /// Get the current effective style (base + all stacked modifiers).
    fn current_style(&self) -> Style {
        let mut style = self.base_style;
        for s in &self.style_stack {
            style = style.patch(*s);
        }
        style
    }

    /// Style for inline code: White on DarkGray (current) or Gray with no bg (non-current).
    fn inline_code_style(&self) -> Style {
        if self.is_current_turn {
            self.current_style().fg(Color::White).bg(Color::DarkGray)
        } else {
            self.current_style().fg(Color::Gray)
        }
    }

    /// Style for code block text: White on DarkGray (current) or Gray with no bg (non-current).
    fn code_block_style(&self) -> Style {
        if self.is_current_turn {
            self.current_style().fg(Color::White).bg(Color::DarkGray)
        } else {
            self.current_style().fg(Color::Gray)
        }
    }

    /// Style for the code block left-border prefix.
    fn code_block_border_style(&self) -> Style {
        if self.is_current_turn {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Black)
        }
    }

    fn process(&mut self, text: &str) {
        use pulldown_cmark::{Event, Parser, Tag, TagEnd};
        use ratatui::style::Modifier;

        let parser = Parser::new(text);
        for event in parser {
            match event {
                Event::Start(Tag::Strong) => {
                    self.style_stack
                        .push(Style::default().add_modifier(Modifier::BOLD));
                }
                Event::End(TagEnd::Strong) => {
                    self.style_stack.pop();
                }
                Event::Start(Tag::Emphasis) => {
                    self.style_stack
                        .push(Style::default().add_modifier(Modifier::ITALIC));
                }
                Event::End(TagEnd::Emphasis) => {
                    self.style_stack.pop();
                }
                Event::Start(Tag::Heading { .. }) => {
                    self.finish_line();
                    self.style_stack
                        .push(Style::default().add_modifier(Modifier::BOLD));
                }
                Event::End(TagEnd::Heading(_)) => {
                    self.style_stack.pop();
                    self.finish_line();
                }
                Event::Start(Tag::CodeBlock(_)) => {
                    self.finish_line();
                    // Blank line before code block for visual separation.
                    self.lines
                        .push(BubbleLine::plain(String::new(), self.base_style));
                    self.in_code_block = true;
                }
                Event::End(TagEnd::CodeBlock) => {
                    self.finish_line();
                    self.in_code_block = false;
                    // Blank line after code block for visual separation.
                    self.lines
                        .push(BubbleLine::plain(String::new(), self.base_style));
                }
                Event::Code(code) => {
                    let code_style = self.inline_code_style();
                    self.push_text(&code, code_style);
                }
                Event::Text(cow_text) => {
                    if self.in_code_block {
                        let code_style = self.code_block_style();
                        self.push_code_block_text(&cow_text, code_style);
                    } else {
                        let style = self.current_style();
                        self.push_text(&cow_text, style);
                    }
                }
                Event::SoftBreak => {
                    let style = self.current_style();
                    self.push_text(" ", style);
                }
                Event::HardBreak => {
                    self.finish_line();
                }
                Event::End(TagEnd::Paragraph) => {
                    self.finish_line();
                    // Blank line between paragraphs (only outside lists).
                    if self.list_stack.is_empty() {
                        self.lines
                            .push(BubbleLine::plain(String::new(), self.base_style));
                    }
                }
                Event::Start(Tag::List(first_number)) => {
                    self.finish_line();
                    match first_number {
                        Some(start) => {
                            self.list_stack
                                .push(ListContext::Ordered { next_number: start });
                        }
                        None => {
                            self.list_stack.push(ListContext::Unordered);
                        }
                    }
                }
                Event::End(TagEnd::List(_)) => {
                    self.finish_line();
                    self.list_stack.pop();
                }
                Event::Start(Tag::Item) => {
                    self.finish_line();
                    let indent = "  ".repeat(self.list_stack.len().saturating_sub(1));
                    let prefix = match self.list_stack.last_mut() {
                        Some(ListContext::Unordered) => {
                            format!("{indent}\u{2022} ")
                        }
                        Some(ListContext::Ordered { next_number }) => {
                            let n = *next_number;
                            *next_number += 1;
                            format!("{indent}{n}. ")
                        }
                        None => String::new(),
                    };
                    self.pending_prefix = Some(prefix);
                }
                Event::End(TagEnd::Item) => {
                    self.finish_line();
                    self.continuation_indent = 0;
                }
                _ => {}
            }
        }
    }

    /// Push text with word-wrapping, splitting on whitespace.
    fn push_text(&mut self, text: &str, style: Style) {
        // If there's a pending prefix (from list items), prepend it.
        // Use `prefix_on_line` to avoid an extra space before the first word.
        let mut prefix_on_line = false;
        if let Some(prefix) = self.pending_prefix.take() {
            let prefix_width = unicode_width::UnicodeWidthStr::width(prefix.as_str());
            self.continuation_indent = prefix_width;
            self.current_spans
                .push(Span::styled(prefix, self.base_style));
            self.current_width += prefix_width;
            prefix_on_line = true;
        }

        for word in text.split_whitespace() {
            let word_width = unicode_width::UnicodeWidthStr::width(word);

            if self.current_width == 0 || prefix_on_line {
                prefix_on_line = false;
                // First word on line (or right after prefix).
                if self.current_width + word_width > self.max_cols {
                    if self.current_width > 0 {
                        self.finish_line();
                        self.push_continuation_indent();
                    }
                    self.char_split_push(word, style);
                } else {
                    self.current_spans
                        .push(Span::styled(word.to_string(), style));
                    self.current_width += word_width;
                }
            } else if self.current_width + 1 + word_width <= self.max_cols {
                // Fits on current line with a space.
                self.current_spans
                    .push(Span::styled(format!(" {word}"), style));
                self.current_width += 1 + word_width;
            } else {
                // Doesn't fit; start a new line.
                self.finish_line();
                self.push_continuation_indent();
                let effective_max = self.max_cols;
                if self.current_width + word_width > effective_max {
                    self.char_split_push(word, style);
                } else {
                    self.current_spans
                        .push(Span::styled(word.to_string(), style));
                    self.current_width += word_width;
                }
            }
        }
    }

    /// Push continuation indent spaces for wrapped list items.
    fn push_continuation_indent(&mut self) {
        if self.continuation_indent > 0 {
            let indent = " ".repeat(self.continuation_indent);
            self.current_spans
                .push(Span::styled(indent, self.base_style));
            self.current_width += self.continuation_indent;
        }
    }

    /// Push code block text: no word-wrapping, split on newlines, hard truncation.
    /// Each line gets a left-border prefix for visual distinction from inline code.
    fn push_code_block_text(&mut self, text: &str, style: Style) {
        let prefix = "\u{258E} "; // ▎ + space = left border prefix
        let prefix_width = 2;
        let border_style = self.code_block_border_style();
        // Reserve space for the prefix when truncating.
        let code_max = self.max_cols.saturating_sub(prefix_width);

        for raw_line in text.split('\n') {
            let truncated = truncate_to_width(raw_line, code_max);
            let code_width = unicode_width::UnicodeWidthStr::width(truncated.as_str());
            self.current_spans
                .push(Span::styled(prefix.to_string(), border_style));
            self.current_spans.push(Span::styled(truncated, style));
            self.current_width += prefix_width + code_width;
            self.finish_line();
        }
    }

    /// Split a word character-by-character when it exceeds max_cols.
    fn char_split_push(&mut self, word: &str, style: Style) {
        let mut buf = String::new();
        let mut buf_width: usize = 0;

        for ch in word.chars() {
            let ch_w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if self.current_width + buf_width + ch_w > self.max_cols
                && !(buf.is_empty() && self.current_spans.is_empty())
            {
                if !buf.is_empty() {
                    self.current_spans.push(Span::styled(buf.clone(), style));
                    self.current_width += buf_width;
                    buf.clear();
                    buf_width = 0;
                }
                self.finish_line();
            }
            buf.push(ch);
            buf_width += ch_w;
        }
        if !buf.is_empty() {
            self.current_spans.push(Span::styled(buf, style));
            self.current_width += buf_width;
        }
    }

    /// Finish the current line and push it to `self.lines`.
    fn finish_line(&mut self) {
        if self.current_spans.is_empty() {
            return;
        }
        let spans = std::mem::take(&mut self.current_spans);
        let width = self.current_width;
        self.current_width = 0;
        self.lines.push(BubbleLine {
            spans,
            display_width: width,
        });
    }

    /// Finalize and return all accumulated lines.
    fn finish(mut self) -> Vec<BubbleLine> {
        self.finish_line();
        // Remove trailing blank line if present (from final paragraph end).
        if let Some(last) = self.lines.last()
            && last.display_width == 0
            && last.spans.len() == 1
        {
            self.lines.pop();
        }
        if self.lines.is_empty() {
            self.lines
                .push(BubbleLine::plain(String::new(), self.base_style));
        }
        self.lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Style};

    /// Helper: collect all text from a BubbleLine into a single string.
    fn line_text(bl: &BubbleLine) -> String {
        bl.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    /// Helper: collect all lines' text joined by newline.
    fn all_text(lines: &[BubbleLine]) -> String {
        lines
            .iter()
            .map(|l| line_text(l))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn plain_text_short_fits_in_one_line() {
        let style = Style::default();
        let result = markdown_wrap("hello world", 60, style, true);
        assert_eq!(result.len(), 1);
        assert_eq!(all_text(&result), "hello world");
    }

    #[test]
    fn bold_text_has_bold_modifier() {
        use ratatui::style::Modifier;
        let style = Style::default();
        let result = markdown_wrap("hello **bold** world", 60, style, true);
        assert_eq!(result.len(), 1);
        let line = &result[0];
        // Should have at least 3 spans: "hello", " bold", " world"
        // The bold span should have BOLD modifier.
        let bold_span = line
            .spans
            .iter()
            .find(|s| s.content.contains("bold"))
            .expect("Should have a span containing 'bold'");
        assert!(
            bold_span.style.add_modifier.contains(Modifier::BOLD),
            "Bold span should have BOLD modifier: {:?}",
            bold_span.style
        );
    }

    #[test]
    fn italic_text_has_italic_modifier() {
        use ratatui::style::Modifier;
        let style = Style::default();
        let result = markdown_wrap("hello *italic* world", 60, style, true);
        assert_eq!(result.len(), 1);
        let italic_span = result[0]
            .spans
            .iter()
            .find(|s| s.content.contains("italic"))
            .expect("Should have a span containing 'italic'");
        assert!(
            italic_span.style.add_modifier.contains(Modifier::ITALIC),
            "Should have ITALIC modifier: {:?}",
            italic_span.style
        );
    }

    #[test]
    fn inline_code_has_distinct_style() {
        let style = Style::default();
        let result = markdown_wrap("use `foo()` here", 60, style, true);
        assert_eq!(result.len(), 1);
        let code_span = result[0]
            .spans
            .iter()
            .find(|s| s.content.contains("foo()"))
            .expect("Should have a span containing 'foo()'");
        assert_eq!(
            code_span.style.fg,
            Some(Color::White),
            "Inline code should be White: {:?}",
            code_span.style
        );
        assert_eq!(
            code_span.style.bg,
            Some(Color::DarkGray),
            "Inline code should have DarkGray bg: {:?}",
            code_span.style
        );
    }

    #[test]
    fn code_block_not_word_wrapped() {
        let style = Style::default();
        // A code block with a long line should NOT be word-wrapped.
        let md = "```\nfn very_long_function_name(with: many, parameters: here, that: exceed) -> Result<(), Error>\n```";
        let result = markdown_wrap(md, 30, style, true);
        // The code line should be truncated (not split into multiple lines).
        let code_line = result
            .iter()
            .find(|l| line_text(l).contains("very_long"))
            .expect("Should have a line with code content");
        assert!(
            code_line.display_width <= 30,
            "Code line should be truncated to max_cols, got width {}",
            code_line.display_width
        );
        // Code block text should have White fg on DarkGray bg (current turn).
        let code_span = code_line
            .spans
            .iter()
            .find(|s| s.content.contains("very_long"))
            .unwrap();
        assert_eq!(code_span.style.fg, Some(Color::White));
        assert_eq!(code_span.style.bg, Some(Color::DarkGray));
    }

    #[test]
    fn unordered_list_renders_bullets() {
        let style = Style::default();
        let result = markdown_wrap("- first item\n- second item", 60, style, true);
        let text = all_text(&result);
        assert!(
            text.contains("\u{2022} first item"),
            "Should have bullet for first item: {text}"
        );
        assert!(
            text.contains("\u{2022} second item"),
            "Should have bullet for second item: {text}"
        );
    }

    #[test]
    fn ordered_list_renders_numbers() {
        let style = Style::default();
        let result = markdown_wrap("1. alpha\n2. beta\n3. gamma", 60, style, true);
        let text = all_text(&result);
        assert!(
            text.contains("1. alpha"),
            "Should have numbered item: {text}"
        );
        assert!(
            text.contains("2. beta"),
            "Should have numbered item: {text}"
        );
        assert!(
            text.contains("3. gamma"),
            "Should have numbered item: {text}"
        );
    }

    #[test]
    fn heading_renders_bold() {
        use ratatui::style::Modifier;
        let style = Style::default();
        let result = markdown_wrap("# My Heading", 60, style, true);
        let heading_line = result
            .iter()
            .find(|l| line_text(l).contains("My Heading"))
            .expect("Should have heading line");
        // All spans in the heading line should have BOLD.
        for span in &heading_line.spans {
            assert!(
                span.style.add_modifier.contains(Modifier::BOLD),
                "Heading span '{}' should be bold: {:?}",
                span.content,
                span.style
            );
        }
    }

    #[test]
    fn bold_style_preserved_across_line_break() {
        use ratatui::style::Modifier;
        let style = Style::default();
        // Bold text that spans a line break.
        let result = markdown_wrap(
            "normal **bold word one bold word two** end",
            20,
            style,
            true,
        );
        // The bold words should wrap across lines, but each bold word
        // should still have BOLD modifier.
        let all_bold_spans: Vec<&Span> = result
            .iter()
            .flat_map(|l| &l.spans)
            .filter(|s| s.style.add_modifier.contains(Modifier::BOLD))
            .collect();
        assert!(
            !all_bold_spans.is_empty(),
            "Should have bold spans across wrapped lines"
        );
        // All bold words should be accounted for.
        let bold_text: String = all_bold_spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            bold_text.contains("bold") && bold_text.contains("word"),
            "Bold text should contain 'bold' and 'word': {bold_text}"
        );
    }

    #[test]
    fn paragraphs_separated_by_blank_line() {
        let style = Style::default();
        let result = markdown_wrap("First paragraph.\n\nSecond paragraph.", 60, style, true);
        // Should have: first line, blank line, second line.
        assert!(
            result.len() >= 3,
            "Should have at least 3 lines (2 paragraphs + separator): got {}",
            result.len()
        );
        let blank_line = &result[1];
        assert_eq!(
            blank_line.display_width, 0,
            "Separator should be blank line"
        );
    }

    #[test]
    fn max_cols_zero_returns_empty_line() {
        let result = markdown_wrap("anything", 0, Style::default(), true);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].display_width, 0);
    }

    #[test]
    fn empty_text_returns_one_empty_line() {
        let result = markdown_wrap("", 60, Style::default(), true);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].display_width, 0);
    }

    #[test]
    fn base_style_fg_is_used_for_plain_text() {
        let style = Style::default().fg(Color::White);
        let result = markdown_wrap("hello", 60, style, true);
        assert_eq!(result[0].spans[0].style.fg, Some(Color::White));
    }

    #[test]
    fn bold_inherits_base_style_color() {
        use ratatui::style::{Color, Modifier};
        let style = Style::default().fg(Color::Green);
        let result = markdown_wrap("**bold**", 60, style, true);
        let bold_span = &result[0].spans[0];
        assert_eq!(bold_span.style.fg, Some(Color::Green));
        assert!(bold_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn plain_text_wraps_at_max_cols() {
        let style = Style::default();
        let result = markdown_wrap(
            "the quick brown fox jumps over the lazy dog",
            20,
            style,
            true,
        );
        assert!(result.len() > 1, "Should wrap: got {} lines", result.len());
        for line in &result {
            assert!(
                line.display_width <= 20,
                "Line too wide: {} (max 20)",
                line.display_width
            );
        }
        // All words should be present.
        let combined = all_text(&result);
        assert!(combined.contains("quick"));
        assert!(combined.contains("lazy"));
    }

    #[test]
    fn inline_code_uses_background_style_current_turn() {
        let style = Style::default().fg(Color::White);
        let result = markdown_wrap("use `foo()` here", 60, style, true);
        let code_span = result[0]
            .spans
            .iter()
            .find(|s| s.content.contains("foo()"))
            .expect("Should have a span containing 'foo()'");
        assert_eq!(code_span.style.fg, Some(Color::White));
        assert_eq!(code_span.style.bg, Some(Color::DarkGray));
    }

    #[test]
    fn inline_code_dimmed_for_non_current_turn() {
        let style = Style::default().fg(Color::Gray);
        let result = markdown_wrap("use `foo()` here", 60, style, false);
        let code_span = result[0]
            .spans
            .iter()
            .find(|s| s.content.contains("foo()"))
            .expect("Should have a span containing 'foo()'");
        assert_eq!(code_span.style.fg, Some(Color::Gray));
        assert_eq!(
            code_span.style.bg, None,
            "Non-current code should have no bg"
        );
    }

    #[test]
    fn code_block_uses_prefix_style_current_turn() {
        let style = Style::default().fg(Color::White);
        let md = "```\nlet x = 1;\n```";
        let result = markdown_wrap(md, 60, style, true);
        let code_line = result
            .iter()
            .find(|l| line_text(l).contains("let x"))
            .expect("Should have code line");
        // Code block lines should start with a left-border prefix.
        let text = line_text(code_line);
        assert!(
            text.starts_with("\u{258E} "),
            "Code block should have left-border prefix: {text:?}"
        );
        // The code text span should have White fg on DarkGray bg.
        let code_span = code_line
            .spans
            .iter()
            .find(|s| s.content.contains("let x"))
            .unwrap();
        assert_eq!(code_span.style.fg, Some(Color::White));
        assert_eq!(code_span.style.bg, Some(Color::DarkGray));
    }

    #[test]
    fn code_block_dimmed_for_non_current_turn() {
        let style = Style::default().fg(Color::Gray);
        let md = "```\nlet x = 1;\n```";
        let result = markdown_wrap(md, 60, style, false);
        let code_line = result
            .iter()
            .find(|l| line_text(l).contains("let x"))
            .expect("Should have code line");
        let code_span = code_line
            .spans
            .iter()
            .find(|s| s.content.contains("let x"))
            .unwrap();
        assert_eq!(code_span.style.fg, Some(Color::Gray));
        assert_eq!(
            code_span.style.bg, None,
            "Non-current code block should have no bg"
        );
    }

    #[test]
    fn list_item_continuation_lines_are_indented() {
        let style = Style::default();
        // A list item whose text wraps. "• " is 2 chars, so continuation
        // lines should also indent by 2 chars.
        let result = markdown_wrap(
            "- this item is long enough that it should wrap to the next line",
            30,
            style,
            true,
        );
        let texts: Vec<String> = result.iter().map(|l| line_text(l)).collect();
        // First line starts with bullet.
        assert!(
            texts[0].starts_with("\u{2022} "),
            "First line should start with bullet: {:?}",
            texts[0]
        );
        // Continuation line(s) should be indented (start with spaces).
        assert!(result.len() > 1, "Should wrap to multiple lines: {texts:?}");
        assert!(
            texts[1].starts_with("  "),
            "Continuation line should be indented: {:?}",
            texts[1]
        );
    }

    #[test]
    fn nested_lists_two_levels() {
        let style = Style::default();
        let md = "- outer\n  - inner one\n  - inner two\n- outer again";
        let result = markdown_wrap(md, 60, style, true);
        let texts: Vec<String> = result.iter().map(|l| line_text(l)).collect();
        // Inner items should be indented.
        let has_inner = texts.iter().any(|t| t.contains("inner one"));
        assert!(has_inner, "Should have inner list items: {texts:?}");
        // Outer items should have bullet.
        assert!(
            texts.iter().any(|t| t.starts_with("\u{2022} outer")),
            "Should have outer bullet: {texts:?}"
        );
    }

    #[test]
    fn multi_line_code_block() {
        let style = Style::default();
        let md = "```\nline one\nline two\nline three\n```";
        let result = markdown_wrap(md, 60, style, true);
        let texts: Vec<String> = result.iter().map(|l| line_text(l)).collect();
        assert!(
            texts.iter().any(|t| t.contains("line one")),
            "Should have first code line: {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.contains("line two")),
            "Should have second code line: {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.contains("line three")),
            "Should have third code line: {texts:?}"
        );
    }

    #[test]
    fn nested_bold_italic() {
        use ratatui::style::Modifier;
        let style = Style::default();
        let result = markdown_wrap("***bold and italic***", 60, style, true);
        let span = result[0]
            .spans
            .iter()
            .find(|s| s.content.contains("bold"))
            .expect("Should have bold+italic span");
        assert!(
            span.style.add_modifier.contains(Modifier::BOLD),
            "Should have BOLD: {:?}",
            span.style
        );
        assert!(
            span.style.add_modifier.contains(Modifier::ITALIC),
            "Should have ITALIC: {:?}",
            span.style
        );
    }

    #[test]
    fn code_block_with_blank_lines_between_content() {
        let style = Style::default();
        let md = "```\nfirst\n\nlast\n```";
        let result = markdown_wrap(md, 60, style, true);
        let texts: Vec<String> = result.iter().map(|l| line_text(l)).collect();
        assert!(
            texts.iter().any(|t| t.contains("first")),
            "Should have first line: {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.contains("last")),
            "Should have last line: {texts:?}"
        );
    }

    #[test]
    fn word_exactly_equal_to_max_cols() {
        let style = Style::default();
        // "abcdefghij" is exactly 10 chars.
        let result = markdown_wrap("abcdefghij", 10, style, true);
        assert_eq!(result.len(), 1);
        assert_eq!(line_text(&result[0]), "abcdefghij");
        assert_eq!(result[0].display_width, 10);
    }

    #[test]
    fn code_block_surrounded_by_blank_lines() {
        let style = Style::default();
        let md = "before\n\n```\ncode\n```\n\nafter";
        let result = markdown_wrap(md, 60, style, true);
        let texts: Vec<String> = result.iter().map(|l| line_text(l)).collect();
        // Find the code line index.
        let code_idx = texts
            .iter()
            .position(|t| t.contains("code"))
            .expect("Should have code line");
        // There should be a blank line before the code line.
        assert!(code_idx > 0, "Code line shouldn't be first");
        assert_eq!(
            texts[code_idx - 1],
            "",
            "Should have blank line before code block"
        );
        // There should be a blank line after the code block's trailing empty line.
        // Code blocks produce an extra empty line from the trailing newline split.
        // Find the first blank line after the code line.
        let after_code = texts[code_idx + 1..].iter().position(|t| t.is_empty());
        assert!(
            after_code.is_some(),
            "Should have blank line after code block, got: {texts:?}"
        );
    }

    #[test]
    fn truncate_to_width_appends_ellipsis_when_clipped() {
        let truncated = truncate_to_width("abcdefghij", 7);
        assert_eq!(truncated, "abcdef\u{2026}");
        assert_eq!(unicode_width::UnicodeWidthStr::width(truncated.as_str()), 7);
    }

    #[test]
    fn truncate_to_width_no_ellipsis_when_fits() {
        let result = truncate_to_width("abcdef", 10);
        assert_eq!(result, "abcdef");
    }
}
