use std::borrow::Cow;
use std::io::{self, Write};

use unicode_width::UnicodeWidthStr;
use vte::{Params, Parser, Perform};

use super::charset::Charset;

const TAB_WIDTH: usize = 8;

// Find the display width of visible chars in a String (strips ANSI codes first)
fn count_visible_chars(input: &str) -> usize {
    strip_ansi_escapes::strip_str(input).width()
}

// Uses the same vte parser as strip_ansi_escapes so the columns agree with count_visible_chars
#[derive(Default)]
struct TabStopTracker {
    column_at_last_tab: usize,
    // Measured as a whole string rather than per char so grapheme-aware widths match count_visible_chars
    printed_since_last_tab: String,
    // vte executes C0 bytes even mid-sequence; expanding a tab there would inject bytes into the escape
    in_sequence: bool,
    tab_pending: bool,
}

impl TabStopTracker {
    fn column(&self) -> usize {
        self.column_at_last_tab + self.printed_since_last_tab.width()
    }
}

impl Perform for TabStopTracker {
    fn print(&mut self, c: char) {
        self.in_sequence = false;
        self.printed_since_last_tab.push(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\t' if !self.in_sequence => self.tab_pending = true,
            // CAN and SUB abort any sequence in progress
            0x18 | 0x1A => self.in_sequence = false,
            _ => (),
        }
    }

    fn unhook(&mut self) {
        self.in_sequence = false;
    }

    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {
        self.in_sequence = false;
    }

    fn csi_dispatch(
        &mut self,
        _params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: char,
    ) {
        self.in_sequence = false;
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {
        self.in_sequence = false;
    }
}

// Terminal tab stops depend on the screen column, which differs once the line sits inside a box,
// so tabs become spaces counted from the start of the line, like expand(1)
fn expand_tabs(input: &str) -> Cow<'_, str> {
    if !input.contains('\t') {
        return Cow::Borrowed(input);
    }

    let mut parser = Parser::new();
    let mut tracker = TabStopTracker::default();
    let mut expanded = String::with_capacity(input.len());
    let mut copied_up_to = 0;

    // One byte at a time so each tab vte reports can be tied back to its position in the input
    for (index, byte) in input.bytes().enumerate() {
        parser.advance(&mut tracker, &[byte]);
        if byte == 0x1B {
            tracker.in_sequence = true;
        }

        if tracker.tab_pending {
            tracker.tab_pending = false;
            expanded.push_str(&input[copied_up_to..index]);
            let column = tracker.column();
            let spaces = TAB_WIDTH - column % TAB_WIDTH;
            expanded.extend(std::iter::repeat_n(' ', spaces));
            tracker.column_at_last_tab = column + spaces;
            tracker.printed_since_last_tab.clear();
            copied_up_to = index + 1;
        }
    }
    expanded.push_str(&input[copied_up_to..]);

    Cow::Owned(expanded)
}

fn calculate_max_length(content: &[String]) -> usize {
    content
        .iter()
        .map(|line| count_visible_chars(line))
        .max()
        .unwrap_or(0)
}

fn repeat(out: &mut impl Write, c: char, count: usize) -> io::Result<()> {
    for _ in 0..count {
        write!(out, "{}", c)?;
    }
    Ok(())
}

pub struct DrawBox<'a> {
    content: Vec<String>,
    title: Option<Cow<'a, str>>,
    charset: Charset,
    // Visible width between the vertical borders, excluding the single space of margin on each side
    inner_width: usize,
    border_sgr: Option<&'a str>,
}

impl<'a> DrawBox<'a> {
    pub fn new(content: Vec<String>, title: Option<&'a str>, charset: Charset) -> DrawBox<'a> {
        let content: Vec<String> = content
            .into_iter()
            .map(|line| match expand_tabs(&line) {
                Cow::Owned(expanded) => expanded,
                Cow::Borrowed(_) => line,
            })
            .collect();
        let title = title.map(expand_tabs);

        let content_width = calculate_max_length(&content);
        // A title needs four extra columns for the left corner, horizontal, its tees and its spacing
        let inner_width = match &title {
            Some(title) => content_width.max(count_visible_chars(title) + 4),
            None => content_width,
        };

        DrawBox {
            content,
            title,
            charset,
            inner_width,
            border_sgr: None,
        }
    }

    pub fn border_sgr(mut self, sgr: Option<&'a str>) -> DrawBox<'a> {
        self.border_sgr = sgr;
        self
    }

    pub fn render(&self, out: &mut impl Write) -> io::Result<()> {
        self.render_top(out)?;
        self.render_middle(out)?;
        self.render_bottom(out)
    }

    fn begin_border(&self, out: &mut impl Write) -> io::Result<()> {
        match self.border_sgr {
            Some(sgr) => write!(out, "{}", sgr),
            None => Ok(()),
        }
    }

    fn end_border(&self, out: &mut impl Write) -> io::Result<()> {
        match self.border_sgr {
            Some(_) => write!(out, "\u{1b}[0m"),
            None => Ok(()),
        }
    }

    fn render_top(&self, out: &mut impl Write) -> io::Result<()> {
        self.begin_border(out)?;
        write!(out, "{}", self.charset.corner_up_left)?;
        match &self.title {
            Some(title) => {
                write!(
                    out,
                    "{}{} ",
                    self.charset.horizontal_top, self.charset.t_left
                )?;
                self.end_border(out)?;
                write!(out, "{}", title)?;
                self.begin_border(out)?;
                write!(out, " {}", self.charset.t_right)?;
                let title_width = count_visible_chars(title);
                repeat(
                    out,
                    self.charset.horizontal_top,
                    self.inner_width - title_width - 3,
                )?;
            }
            None => repeat(out, self.charset.horizontal_top, self.inner_width + 2)?,
        }
        write!(out, "{}", self.charset.corner_up_right)?;
        self.end_border(out)?;
        writeln!(out)
    }

    fn render_middle(&self, out: &mut impl Write) -> io::Result<()> {
        for line in self.content.iter() {
            self.begin_border(out)?;
            write!(out, "{} ", self.charset.vertical)?;
            self.end_border(out)?;
            write!(out, "{}", line)?;
            repeat(out, ' ', self.inner_width - count_visible_chars(line))?;
            self.begin_border(out)?;
            write!(out, " {}", self.charset.vertical)?;
            self.end_border(out)?;
            writeln!(out)?;
        }
        Ok(())
    }

    fn render_bottom(&self, out: &mut impl Write) -> io::Result<()> {
        self.begin_border(out)?;
        write!(out, "{}", self.charset.corner_down_left)?;
        repeat(out, self.charset.horizontal_bottom, self.inner_width + 2)?;
        write!(out, "{}", self.charset.corner_down_right)?;
        self.end_border(out)?;
        writeln!(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charset::get_charset;

    fn render(content: &[&str], title: Option<&str>) -> String {
        let content = content.iter().map(|line| line.to_string()).collect();
        let mut out = Vec::new();
        DrawBox::new(content, title, get_charset("ascii"))
            .render(&mut out)
            .unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn test_count_visible_chars() {
        assert_eq!(3, count_visible_chars("abc"), "Three normal ASCII chars");
        assert_eq!(
            3,
            count_visible_chars("\u{001b}[31mabc\u{001b}[0m"),
            "Three ASCII chars made red"
        );
    }

    #[test]
    fn test_expand_tabs_without_tabs_is_unchanged() {
        let input = "plain \u{001b}[31mred\u{001b}[0m 🦀";
        assert!(matches!(expand_tabs(input), Cow::Borrowed(s) if s == input));
    }

    #[test]
    fn test_expand_tabs_at_column_zero() {
        assert_eq!(expand_tabs("\tab"), "        ab");
    }

    #[test]
    fn test_expand_tabs_mid_line() {
        assert_eq!(expand_tabs("a\tb"), "a       b");
    }

    #[test]
    fn test_expand_tabs_one_before_tab_stop() {
        assert_eq!(expand_tabs("abcdefg\tx"), "abcdefg x");
    }

    #[test]
    fn test_expand_tabs_exactly_at_tab_stop() {
        assert_eq!(expand_tabs("abcdefgh\tx"), "abcdefgh        x");
    }

    #[test]
    fn test_expand_tabs_multiple() {
        assert_eq!(expand_tabs("a\tbc\t\td"), "a       bc              d");
        assert_eq!(expand_tabs("x\t"), "x       ");
    }

    #[test]
    fn test_expand_tabs_after_ansi_escape() {
        assert_eq!(
            expand_tabs("\u{001b}[31mab\u{001b}[0m\tc"),
            "\u{001b}[31mab\u{001b}[0m      c"
        );
    }

    #[test]
    fn test_expand_tabs_after_osc_escape() {
        assert_eq!(
            expand_tabs("\u{001b}]8;;https://example.com\u{0007}ab\u{001b}]8;;\u{001b}\\\tc"),
            "\u{001b}]8;;https://example.com\u{0007}ab\u{001b}]8;;\u{001b}\\      c"
        );
    }

    #[test]
    fn test_expand_tabs_after_wide_char() {
        assert_eq!(expand_tabs("日本\tx"), "日本    x");
        assert_eq!(expand_tabs("🦀\tx"), "🦀      x");
    }

    #[test]
    fn test_expand_tabs_after_coloured_segment_and_wide_char() {
        // Colour codes add 0 columns and 日 adds 2, so the tab sits at column 5
        assert_eq!(
            expand_tabs("\u{001b}[1;31mab\u{001b}[0m日c\td"),
            "\u{001b}[1;31mab\u{001b}[0m日c   d"
        );
    }

    #[test]
    fn test_expand_tabs_inside_escape_sequence_is_left_alone() {
        let input = "\u{001b}[\t31mab";
        assert_eq!(expand_tabs(input), input);
    }

    #[test]
    fn test_render_with_tabs() {
        assert_eq!(
            render(&["a\tb", "abcdefgh"], Some("x\ty")),
            "+-- x       y --+\n| a       b     |\n| abcdefgh      |\n+---------------+\n"
        );
    }

    #[test]
    fn test_calculate_max_length() {
        let lines: Vec<String> = vec![
            String::from("Hello"),
            String::from("Hola"),
            String::from("Caio"),
        ];
        assert_eq!(calculate_max_length(&lines), 5);

        let empty: Vec<String> = vec![];
        assert_eq!(calculate_max_length(&empty), 0);
    }

    #[test]
    fn test_calculate_max_length_ignores_ansi() {
        // The coloured line has more bytes but fewer visible columns, so a raw measure picks the wrong line
        let lines: Vec<String> = vec![
            String::from("\u{001b}[1;36mabc\u{001b}[0m"),
            String::from("abcdef"),
        ];
        assert_eq!(calculate_max_length(&lines), 6);
    }

    #[test]
    fn test_render_without_title() {
        assert_eq!(
            render(&["ab", "abcd"], None),
            "+------+\n| ab   |\n| abcd |\n+------+\n"
        );
        assert_eq!(render(&[], None), "+--+\n+--+\n");
    }

    #[test]
    fn test_render_title_narrower_than_content() {
        assert_eq!(
            render(&["abcdefgh"], Some("ab")),
            "+-- ab ----+\n| abcdefgh |\n+----------+\n"
        );
    }

    #[test]
    fn test_render_title_exactly_fits_content() {
        assert_eq!(
            render(&["abcdef"], Some("ab")),
            "+-- ab --+\n| abcdef |\n+--------+\n"
        );
    }

    #[test]
    fn test_render_title_wider_than_content() {
        assert_eq!(
            render(&["a"], Some("abcd")),
            "+-- abcd --+\n| a        |\n+----------+\n"
        );
    }

    #[test]
    fn test_render_empty_title() {
        assert_eq!(render(&["a"], Some("")), "+--  --+\n| a    |\n+------+\n");
    }

    fn render_red(content: &[&str], title: Option<&str>) -> String {
        let content = content.iter().map(|line| line.to_string()).collect();
        let mut out = Vec::new();
        DrawBox::new(content, title, get_charset("ascii"))
            .border_sgr(Some("\u{1b}[31m"))
            .render(&mut out)
            .unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn test_render_coloured_without_title() {
        assert_eq!(
            render_red(&["ab", "abcd"], None),
            "\u{1b}[31m+------+\u{1b}[0m\n\
             \u{1b}[31m| \u{1b}[0mab  \u{1b}[31m |\u{1b}[0m\n\
             \u{1b}[31m| \u{1b}[0mabcd\u{1b}[31m |\u{1b}[0m\n\
             \u{1b}[31m+------+\u{1b}[0m\n"
        );
    }

    #[test]
    fn test_render_coloured_with_title() {
        assert_eq!(
            render_red(&["abcdefgh"], Some("ab")),
            "\u{1b}[31m+-- \u{1b}[0mab\u{1b}[31m ----+\u{1b}[0m\n\
             \u{1b}[31m| \u{1b}[0mabcdefgh\u{1b}[31m |\u{1b}[0m\n\
             \u{1b}[31m+----------+\u{1b}[0m\n"
        );
    }

    #[test]
    fn test_render_coloured_passes_content_and_title_escapes_through() {
        assert_eq!(
            render_red(
                &["\u{1b}[1;36mabc\u{1b}[0m", "abcdef"],
                Some("\u{1b}[32mt\u{1b}[0m")
            ),
            "\u{1b}[31m+-- \u{1b}[0m\u{1b}[32mt\u{1b}[0m\u{1b}[31m ---+\u{1b}[0m\n\
             \u{1b}[31m| \u{1b}[0m\u{1b}[1;36mabc\u{1b}[0m   \u{1b}[31m |\u{1b}[0m\n\
             \u{1b}[31m| \u{1b}[0mabcdef\u{1b}[31m |\u{1b}[0m\n\
             \u{1b}[31m+--------+\u{1b}[0m\n"
        );
    }
}
