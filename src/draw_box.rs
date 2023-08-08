use super::charset::Charset;

pub trait DrawBox {
    fn new(content: Vec<String>, charset: Charset) -> Self;

    // Print the box
    fn print(&self);

    // Individual box parts
    fn print_top(&self);
    fn print_middle(&self);
    fn print_bottom(&self);
}

// Find the count of visible chars in a String
fn count_visible_chars(input: &str) -> usize {
    strip_ansi_escapes::strip(input).unwrap().len()
}

// A simple box around the content
pub struct SimpleBox {
    content: Vec<String>,
    charset: Charset,
    max_length: usize,
}

fn calculate_max_length(mut content: Vec<String>) -> usize {
    content.sort_unstable_by_key(|b| b.len());
    match content.last() {
        Some(line) => line.len(),
        None => 0,
    }
}

impl DrawBox for SimpleBox {
    fn new(content: Vec<String>, charset: Charset) -> SimpleBox {
        //  Get the longest line in the output
        let max_length = calculate_max_length(content.clone());

        SimpleBox {
            content,
            charset,
            max_length,
        }
    }

    fn print(&self) {
        self.print_top();
        self.print_middle();
        self.print_bottom();
    }

    fn print_top(&self) {
        print!("{}", self.charset.corner_up_left);
        for _ in 0..(self.max_length + 2) {
            print!("{}", self.charset.horizontal)
        }
        println!("{}", self.charset.corner_up_right);
    }

    fn print_middle(&self) {
        for line in self.content.iter() {
            print!("{} {}", self.charset.vertical, line);
            let length: usize = count_visible_chars(line);

            // Pad shorter lines with spaces
            for _ in 0..(self.max_length - length) {
                print!(" ");
            }

            println!(" {}", self.charset.vertical);
        }
    }

    fn print_bottom(&self) {
        print!("{}", self.charset.corner_down_left);
        for _ in 0..(self.max_length + 2) {
            print!("{}", self.charset.horizontal)
        }
        println!("{}", self.charset.corner_down_right);
    }
}

// A simple box around the content
pub struct TitleBox<'a> {
    title: &'a str,
    content: Vec<String>,
    charset: Charset,
    max_length: usize,
}

impl<'a> DrawBox for TitleBox<'a> {
    fn new(content: Vec<String>, charset: Charset) -> TitleBox<'a> {
        //  Get the longest line in the output
        let max_length = calculate_max_length(content.clone());

        TitleBox {
            title: "",
            content,
            charset,
            max_length,
        }
    }

    fn print(&self) {
        self.print_top();
        self.print_middle();
        self.print_bottom();
    }

    fn print_top(&self) {
        print!(
            "{}{}{} {} {}",
            self.charset.corner_up_left,
            self.charset.horizontal,
            self.charset.t_left,
            self.title,
            self.charset.t_right
        );

        let title_length = self.title.len() + 5;
        let num_pad: usize = match title_length.cmp(&self.max_length) {
            std::cmp::Ordering::Less => self.max_length + 2 - title_length,
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Equal => 2,
        };
        for _ in 0..num_pad {
            print!("{}", self.charset.horizontal)
        }
        println!("{}", self.charset.corner_up_right);
    }

    fn print_middle(&self) {
        for line in self.content.iter() {
            print!("{} {}", self.charset.vertical, line);

            // Pad shorter lines with spaces
            let length: usize = count_visible_chars(line);
            let title_length = self.title.len() + 5;
            let num_pad: usize = match title_length.cmp(&self.max_length) {
                std::cmp::Ordering::Less => self.max_length - length,
                std::cmp::Ordering::Greater => title_length - length - 1,
                std::cmp::Ordering::Equal => title_length - length,
            };
            for _ in 0..num_pad {
                print!(" ");
            }

            println!(" {}", self.charset.vertical);
        }
    }

    fn print_bottom(&self) {
        print!("{}", self.charset.corner_down_left);
        let title_length = self.title.len() + 5;
        let num_pad: usize = match title_length.cmp(&self.max_length) {
            std::cmp::Ordering::Less => self.max_length + 2,
            std::cmp::Ordering::Greater => title_length + 1,
            std::cmp::Ordering::Equal => title_length + 2,
        };
        for _ in 0..num_pad {
            print!("{}", self.charset.horizontal)
        }
        println!("{}", self.charset.corner_down_right);
    }
}

impl<'a> TitleBox<'a> {
    pub fn set_title(&mut self, title: &'a str) {
        self.title = title;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(calculate_max_length(lines), 5);

        let empty: Vec<String> = vec![];
        assert_eq!(calculate_max_length(empty), 0);
    }
}
