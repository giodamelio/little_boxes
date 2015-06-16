use super::charset::Charset;

pub struct DrawBox {
    content: Vec<String>,
    charset: Charset,
}

impl DrawBox {
    pub fn new(content: Vec<String>, charset: Charset) -> DrawBox {
        DrawBox {
            content: content,
            charset: charset,
        }
    }

    pub fn print(&self) {
        //  Get the longest line in the output
        // Cleaner approace, but max_by is still marked unstable
        // let longest_line = match input.iter().max_by(|x| x.len()) {
        //     Some(line) => line.len(),
        //     _          => 0,
        // };
        let mut sorted_input = self.content.clone();
        sorted_input.sort_by(|a, b| b.len().cmp(&a.len()));
        let max_length = sorted_input[0].len();

        // Print top of box
        print!("{}", self.charset.corner_up_left);
        for _ in 0..(max_length + 2) {
            print!("{}", self.charset.horizontal)
        }
        println!("{}", self.charset.corner_up_right);

        // Print the lines
        for line in self.content.iter() {
            print!("{} {}", self.charset.vertical, line);

            // Pad shorter lines with spaces
            for _ in 0..(max_length - line.len()) {
                print!(" ");
            }

            println!(" {}", self.charset.vertical);
        }

        // Print bottom of box
        print!("{}", self.charset.corner_down_left);
        for _ in 0..(max_length + 2) {
            print!("{}", self.charset.horizontal)
        }
        println!("{}", self.charset.corner_down_right);
    }
}
