use std::io;
use std::io::prelude::*;
use std::process;

use clap::{crate_authors, crate_description, crate_version, App, Arg};

mod draw_box;
use self::draw_box::{DrawBox, SimpleBox, TitleBox};
mod charset;
use self::charset::{get_charset, Charset};

fn main() {
    let matches = App::new("little_boxes")
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .max_term_width(80)
        .arg(
            Arg::with_name("title")
                .short("t")
                .long("title")
                .takes_value(true)
                .value_name("title")
                .help("Add a title to the box"),
        )
        .arg(
            Arg::with_name("charset")
                .short("c")
                .long("charset")
                .takes_value(true)
                .possible_values(&["thick", "thin", "double", "box", "rounded", "dot"])
                .default_value("thick")
                .value_name("charset")
                .help("The charset to draw the box with"),
        )
        .arg(
            Arg::with_name("all")
                .long("all")
                .help("Compare all charsets"),
        )
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
    if matches.is_present("all") {
        let charsets = vec!["thick", "thin", "double", "box", "rounded", "dot"];

        for charset_name in charsets {
            let charset = get_charset(charset_name);
            println!("{}:", charset_name);
            print_box(input.clone(), matches.value_of("title"), charset);
        }

        process::exit(0);
    }

    // It is safe to .unwrap here because Clap's default will ensure that there is always a value
    print_box(
        input,
        matches.value_of("title"),
        get_charset(
            matches
                .value_of("charset")
                .expect("Invalid charset recieved from Clap"),
        ),
    );
}

fn print_box(content: Vec<String>, title: Option<&str>, charset: Charset) {
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
