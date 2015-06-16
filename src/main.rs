extern crate rustc_serialize;
extern crate docopt;

use std::process;
use std::io;
use std::io::prelude::*;

use docopt::Docopt;

mod draw_box;
use self::draw_box::{ DrawBox, SimpleBox, TitleBox };
mod charset;
use self::charset::Charset;

// Write the Docopt usage string.
static USAGE: &'static str = "
Usage: little_boxes [options]

Options:
  -c, --charset <charset>    The charset to draw the box with [default: thick]
  -t, --title <title>        Add a title to the box
  --title-align <alignment>  The Alignment of the title [default: left]
  --command <command>    Run a command and put its output in the box
  -h, --help                 Shows this help
  --version                  Show version
";

// Get the version from cargo
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_charset: String,
    flag_title: String,
    flag_command: String,
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

    // Handle charset
    let charset = match args.flag_charset.as_ref() {
        "thick" => Charset {
            horizontal: '━',
            vertical: '┃',
            corner_up_left: '┏',
            corner_up_right: '┓',
            corner_down_left: '┗',
            corner_down_right: '┛',
            t_right: '┣',
            t_left: '┫',
        },
        "thin" => Charset {
            horizontal: '─',
            vertical: '│',
            corner_up_left: '┌',
            corner_up_right: '┐',
            corner_down_left: '└',
            corner_down_right: '┘',
            t_right: '├',
            t_left: '┤',
        },
        "double" => Charset {
            horizontal: '═',
            vertical: '║',
            corner_up_left: '╔',
            corner_up_right: '╗',
            corner_down_left: '╚',
            corner_down_right: '╝',
            t_right: '╠',
            t_left: '╣',
        },
        "box" => Charset {
            horizontal: '█',
            vertical: '█',
            corner_up_left: '█',
            corner_up_right: '█',
            corner_down_left: '█',
            corner_down_right: '█',
            t_right: '█',
            t_left: '█',
        },
        "rounded" => Charset {
            horizontal: '─',
            vertical: '│',
            corner_up_left: '╭',
            corner_up_right: '╮',
            corner_down_left: '╰',
            corner_down_right: '╯',
            t_right: '├',
            t_left: '┤',
        },
        "dot" => Charset {
            horizontal: '⠶',
            vertical: '⣿',
            corner_up_left: '⣶',
            corner_up_right: '⣶',
            corner_down_left: '⠿',
            corner_down_right: '⠿',
            t_right: '⡷',
            t_left: '⢾',
        },
        _ => {
            println!("Charset must be one of thick, thin, double, box, rounded, dot");
            process::exit(1);
        }
    };

    // let basic_box: SimpleBox = DrawBox::new(input, charset);
    // basic_box.print();

    let mut title_box: TitleBox = DrawBox::new(input, charset);
    title_box.set_title("Test Title");
    title_box.print();
}
