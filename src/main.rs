extern crate rustc_serialize;
extern crate docopt;

use std::process;
use std::io;
use std::io::prelude::*;

use docopt::Docopt;

// Write the Docopt usage string.
static USAGE: &'static str = "
Usage: little_boxes [options]
       little_boxes (--help | --version)

Options:
  -t, --title <title>      Add a title to the box
  -c, --command <command>  Run a command and put its output in the box
  --version                Show version
";

// Get the version from cargo
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_title: String,
    flag_command: String,
    flag_version: bool,
}

struct Charset {
    horizontal: char,
    vertical: char,
    corner_up_left: char,
    corner_up_right: char,
    corner_down_left: char,
    corner_down_right: char,
    t_right: char,
    t_left: char,
}

fn main() {
    // Parse args
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());

    // If we want the version
    if args.flag_version {
        println!("little_boxes v{}", VERSION.unwrap_or("unknown"));
        process::exit(0);
    }

    // Choose input
    let input: Vec<String>;
    if args.flag_command != "" {
        // Command output
        println!("Not Implemented");
        process::exit(1);
    } else {
        // Stdin
        let stdin = io::stdin();

        // Read stdin and convert to vector of Strings
        input = stdin
            .lock()
            .lines()
            .map(|line| line.ok())
            .map(|line| {
                match line {
                    Some(a) => a,
                    None => String::new(),
                }
            })
            .collect();
    }

    // Print it in a box
    box_it(input);
}

fn box_it(input: Vec<String>) {
    // Defalt charset
    let charset = Charset {
        horizontal: '━',
        vertical: '┃',
        corner_up_left: '┏',
        corner_up_right: '┓',
        corner_down_left: '┗',
        corner_down_right: '┛',
        t_right: '┣',
        t_left: '┫',
    };

    //  Get the longest line in the output
    // Cleaner approace, but max_by is still marked unstable
    // let longest_line = match input.iter().max_by(|x| x.len()) {
    //     Some(line) => line.len(),
    //     _          => 0,
    // };
    let mut sorted_input = input.clone();
    sorted_input.sort_by(|a, b| b.len().cmp(&a.len()));
    let max_length = sorted_input[0].len();

    // Print top of box
    print!("{}", charset.corner_up_left);
    for _ in 0..(max_length + 2) {
        print!("{}", charset.horizontal)
    }
    println!("{}", charset.corner_up_right);

    // Print the lines
    for line in input {
        print!("{} {}", charset.vertical, line);

        // Pad shorter lines with spaces
        for _ in 0..(max_length - line.len()) {
            print!(" ");
        }

        println!(" {}", charset.vertical);
    }

    // Print bottom of box
    print!("{}", charset.corner_down_left);
    for _ in 0..(max_length + 2) {
        print!("{}", charset.horizontal)
    }
    println!("{}", charset.corner_down_right);
}
