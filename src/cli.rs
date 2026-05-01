use std::path::PathBuf;

use clap::{Command, arg, command, value_parser};

pub fn cli() -> Command {
    command!()
        .arg(arg!(-t --title <TITLE> "Add a title to the box").required(false))
        .arg(
            arg!(-c --charset <CHARSET> "The charset to draw the box with")
                .value_parser(["thick", "thin", "double", "box", "rounded", "dot", "ascii"])
                .default_value("thick"),
        )
        .arg(
            arg!(-f --file <FILE> "Read input from a file instead of stdin")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(arg!(--all "Compare all charsets"))
}
