use std::fs::File;
use std::io::prelude::*;
use std::io::{self, BufReader};
use std::path::PathBuf;
use std::process;

use clap::{arg, command, value_parser, ArgMatches};

mod draw_box;
use self::draw_box::{DrawBox, SimpleBox, TitleBox};
mod charset;
use self::charset::{get_charset, Charset};

fn get_input(matches: &ArgMatches) -> io::Result<Vec<String>> {
    // Read from a file if the flag is passed
    if let Some(file_path) = matches.get_one::<PathBuf>("file") {
        dbg!(file_path);

        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        return reader.lines().collect();
    }

    // Read stdin and convert to vector of Strings
    let stdin = io::stdin();
    stdin.lock().lines().collect()
}

fn run() -> io::Result<()> {
    let matches = command!()
        .arg(arg!(-t --title <TITLE> "Add a title to the box").required(false))
        .arg(
            arg!(-c --charset <CHARSET> "The charset to draw the box with")
                .value_parser(["thick", "thin", "double", "box", "rounded", "dot"])
                .default_value("thick"),
        )
        .arg(
            arg!(-f --file <FILE> "Read input from a file instead of stdin")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(arg!(--all "Compare all charsets"))
        .get_matches();

    let input = get_input(&matches)?;

    // Compare all the charsets
    if let Some(true) = matches.get_one::<bool>("all") {
        let charsets = vec!["thick", "thin", "double", "box", "rounded", "dot"];

        for charset_name in charsets {
            let charset = get_charset(charset_name);
            println!("{}:", charset_name);
            print_box(input.clone(), matches.get_one::<String>("title"), charset);
        }

        process::exit(0);
    }

    // It is safe to .unwrap here because Clap's default will ensure that there is always a value
    print_box(
        input,
        matches.get_one::<String>("title"),
        get_charset(
            matches
                .get_one::<String>("charset")
                .expect("Invalid charset recieved from Clap"),
        ),
    );

    Ok(())
}

fn print_box(content: Vec<String>, title: Option<&String>, charset: Charset) {
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

fn main() {
    match run() {
        Ok(_) => (),
        Err(err) => {
            println!("Error: {}", err);
            std::process::exit(1);
        }
    };
}
