extern crate rustc_serialize;
extern crate docopt;

use std::process;
use std::io;
use std::io::prelude::*;

use docopt::Docopt;

mod draw_box;
use self::draw_box::{DrawBox, SimpleBox, TitleBox};
mod charset;
use self::charset::{Charset, charsets};

// Write the Docopt usage string.
static USAGE: &'static str = "
Usage: little_boxes [options]

Options:
  -c, --charset <charset>    The charset to draw the box with [default: thick]
                             Available charsets: thick, thin, double, box, rounded and dot
  -t, --title <title>        Add a title to the box
  -h, --help                 Shows this help
  -v, --version              Show version
";

// Get the version from cargo
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_charset: String,
    flag_title: String,
    flag_version: bool,
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

    // Read stdin and convert to vector of Strings
    let stdin = io::stdin();
    let input = stdin.lock()
        .lines()
        .map(|line| line.ok())
        .map(|line| {
            match line {
                Some(a) => a,
                None => String::new(),
            }
        })
        .collect();

    // Handle charset
    let charset: Charset = match charsets(args.flag_charset.as_ref()) {
        Some(charset) => charset,
        None => {
            println!("Charset must be one of thick, thin, double, box, rounded, dot");
            process::exit(1);
        }
    };

    if args.flag_title != "" {
        let mut title_box: TitleBox = DrawBox::new(input, charset);
        title_box.set_title(args.flag_title.as_ref());
        title_box.print();
    } else {
        let basic_box: SimpleBox = DrawBox::new(input, charset);
        basic_box.print();
    }
}
