use regex::Regex;

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
    let ansi_regex = Regex::new(r"(\x9B|\x1B\[)[0-?]*[ -/]*[@-~]").unwrap();
    ansi_regex.replace_all(input, "").chars().count()
}

// A simple box around the content
pub struct SimpleBox {
    content: Vec<String>,
    charset: Charset,
    max_length: usize,
}

impl DrawBox for SimpleBox {
    fn new(content: Vec<String>, charset: Charset) -> SimpleBox {
        //  Get the longest line in the output
        let mut sorted_input = content.clone();
        sorted_input.sort_by(|a, b| b.len().cmp(&a.len()));
        let max_length = sorted_input[0].len();

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
        let mut sorted_input = content.clone();
        sorted_input.sort_by(|a, b| b.len().cmp(&a.len()));
        let max_length = sorted_input[0].len();

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
        let num_pad: usize = if title_length < self.max_length {
            self.max_length + 2 - title_length
        } else if title_length == self.max_length {
            2
        } else {
            1
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
            let num_pad: usize = if title_length < self.max_length {
                self.max_length - length
            } else if title_length == self.max_length {
                title_length - length
            } else {
                title_length - length - 1
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
        let num_pad: usize = if title_length < self.max_length {
            self.max_length + 2
        } else if title_length == self.max_length {
            title_length + 2
        } else {
            title_length + 1
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
