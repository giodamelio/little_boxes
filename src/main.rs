#[macro_use] extern crate serde_derive;
extern crate docopt;
extern crate regex;

use std::process;
use std::io;
use std::io::prelude::*;

use docopt::Docopt;

mod draw_box;
use self::draw_box::{DrawBox, SimpleBox, TitleBox};
mod charset;
use self::charset::{Charset, get_charset};

// Write the Docopt usage string.
static USAGE: &'static str = "
Usage: little_boxes [options]

Options:
  -c, --charset <charset>    The charset to draw the box with [default: thick]
                             Available charsets: thick, thin, double, box, rounded and dot
  -t, --title <title>        Add a title to the box
  -a, --all                  Compare all the styles
  -h, --help                 Shows this help
  -v, --version              Show version
";

// Get the version from cargo
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

#[derive(Deserialize, Debug)]
struct Args {
    flag_charset: String,
    flag_title: Option<String>,
    flag_all: bool,
    flag_version: bool,
}

fn main() {
    // Parse args
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.parse())
        .unwrap_or_else(|e| e.exit())
        .deserialize().expect("DOCOPT FAILURE");

    // If we want the version
    if args.flag_version {
        println!("little_boxes v{}", VERSION.unwrap_or("unknown"));
        process::exit(0);
    }

    // Read stdin and convert to vector of Strings
    let stdin = io::stdin();
    let input: Vec<String> = stdin.lock()
        .lines()
        .map(|line| line.ok())
        .map(|line| {
            match line {
                Some(a) => a,
                None => String::new(),
            }
        })
        .collect();

    // Compare all the charsets
    if args.flag_all {
        let charsets = vec!["thick", "thin", "double", "box", "rounded", "dot"];

        for charset_name in charsets {
            if let Some(charset) = get_charset(charset_name) {
                println!("{}:", charset_name);
                print_box(input.clone(), args.flag_title.clone(), charset);
            }
        }

        process::exit(0);
    }

    // Handle charset
    let charset: Charset = match get_charset(args.flag_charset.as_ref()) {
        Some(charset) => charset,
        None => {
            println!("Charset must be one of thick, thin, double, box, rounded, dot");
            process::exit(1);
        }
    };

    print_box(input, args.flag_title, charset);
}

fn print_box(content: Vec<String>, title: Option<String>, charset: Charset) {
    match title {
        Some(title) => {
            let mut title_box: TitleBox = DrawBox::new(content, charset);
            title_box.set_title(title.as_ref());
            title_box.print();
        }
        None => {
            let basic_box: SimpleBox = DrawBox::new(content, charset);
            basic_box.print();
        }
    }
}
