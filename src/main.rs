use std::io;
use std::io::prelude::*;
use std::process;

use clap::{arg, command};

mod draw_box;
use self::draw_box::{DrawBox, SimpleBox, TitleBox};
mod charset;
use self::charset::{get_charset, Charset};

fn main() {
    let matches = command!()
        .arg(arg!(-t --title <TITLE> "Add a title to the box").required(false))
        .arg(
            arg!(-c --charset <CHARSET> "The charset to draw the box with")
                .value_parser(["thick", "thin", "double", "box", "rounded", "dot"])
                .default_value("thick"),
        )
        .arg(arg!(--all "Compare all charsets"))
        .get_matches();

    // Read stdin and convert to vector of Strings
    let stdin = io::stdin();
    let input: Vec<String> = stdin
        .lock()
        .lines()
        .map(|line| line.ok())
        .map(|line| match line {
            Some(a) => a,
            None => String::new(),
        })
        .collect();

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
