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

// A simple box around the content
pub struct SimpleBox {
    content: Vec<String>,
    charset: Charset,
    max_length: usize,
}

impl DrawBox for SimpleBox {
    fn new(content: Vec<String>, charset: Charset) -> SimpleBox {
        //  Get the longest line in the output
        // Cleaner approace, but max_by is still marked unstable
        // let longest_line = match input.iter().max_by(|x| x.len()) {
        //     Some(line) => line.len(),
        //     _          => 0,
        // };
        let mut sorted_input = content.clone();
        sorted_input.sort_by(|a, b| b.len().cmp(&a.len()));
        let max_length = sorted_input[0].len();

        SimpleBox {
            content: content,
            charset: charset,
            max_length: max_length,
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

            // Pad shorter lines with spaces
            for _ in 0..(self.max_length - line.len()) {
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
