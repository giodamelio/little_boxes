extern crate rustc_serialize;
extern crate docopt;

use std::process;

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

    println!("{:?}", args);
}
