use std::io::{self, Write};

use unicode_width::UnicodeWidthStr;

use super::charset::Charset;

// Find the display width of visible chars in a String (strips ANSI codes first)
fn count_visible_chars(input: &str) -> usize {
    strip_ansi_escapes::strip_str(input).width()
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
    title: Option<&'a str>,
    charset: Charset,
    // Visible width between the vertical borders, excluding the single space of margin on each side
    inner_width: usize,
}

impl<'a> DrawBox<'a> {
    pub fn new(content: Vec<String>, title: Option<&'a str>, charset: Charset) -> DrawBox<'a> {
        let content_width = calculate_max_length(&content);
        // A title needs four extra columns for the left corner, horizontal, its tees and its spacing
        let inner_width = match title {
            Some(title) => content_width.max(count_visible_chars(title) + 4),
            None => content_width,
        };

        DrawBox {
            content,
            title,
            charset,
            inner_width,
        }
    }

    pub fn render(&self, out: &mut impl Write) -> io::Result<()> {
        self.render_top(out)?;
        self.render_middle(out)?;
        self.render_bottom(out)
    }

    fn render_top(&self, out: &mut impl Write) -> io::Result<()> {
        write!(out, "{}", self.charset.corner_up_left)?;
        match self.title {
            Some(title) => {
                write!(
                    out,
                    "{}{} {} {}",
                    self.charset.horizontal_top, self.charset.t_left, title, self.charset.t_right
                )?;
                let title_width = count_visible_chars(title);
                repeat(
                    out,
                    self.charset.horizontal_top,
                    self.inner_width - title_width - 3,
                )?;
            }
            None => repeat(out, self.charset.horizontal_top, self.inner_width + 2)?,
        }
        writeln!(out, "{}", self.charset.corner_up_right)
    }

    fn render_middle(&self, out: &mut impl Write) -> io::Result<()> {
        for line in self.content.iter() {
            write!(out, "{} {}", self.charset.vertical, line)?;
            repeat(out, ' ', self.inner_width - count_visible_chars(line))?;
            writeln!(out, " {}", self.charset.vertical)?;
        }
        Ok(())
    }

    fn render_bottom(&self, out: &mut impl Write) -> io::Result<()> {
        write!(out, "{}", self.charset.corner_down_left)?;
        repeat(out, self.charset.horizontal_bottom, self.inner_width + 2)?;
        writeln!(out, "{}", self.charset.corner_down_right)
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
}
