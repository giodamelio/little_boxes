use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::prelude::*;
use std::io::{self, BufReader, IsTerminal};
use std::path::PathBuf;
use std::process;

use anyhow::{Context, Result, anyhow};
use clap::ArgMatches;

mod cli;
use self::cli::cli;
mod draw_box;
use self::draw_box::DrawBox;
mod charset;
use self::charset::{Charset, get_charset};

fn get_input(matches: &ArgMatches) -> Result<Vec<String>> {
    if let Some(file_path) = matches.get_one::<PathBuf>("file") {
        let file = File::open(file_path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => {
                anyhow!("File {:?} does not exist", file_path)
            }
            _ => anyhow::Error::new(e).context(format!("Failed to open {:?}", file_path)),
        })?;

        let metadata = file
            .metadata()
            .with_context(|| format!("Failed to stat {:?}", file_path))?;

        if metadata.is_dir() {
            anyhow::bail!("{:?} is a directory, it must be a file", file_path);
        }

        BufReader::new(file)
            .lines()
            .collect::<io::Result<Vec<_>>>()
            .with_context(|| format!("Failed to read {:?}", file_path))
    } else {
        io::stdin()
            .lock()
            .lines()
            .collect::<io::Result<Vec<_>>>()
            .context("Failed to read stdin")
    }
}

// NO_COLOR only disables colour when non-empty, per https://no-color.org
fn should_color(when: &str, is_tty: bool, no_color: Option<&OsStr>, term: Option<&OsStr>) -> bool {
    match when {
        "always" => true,
        "never" => false,
        "auto" => {
            is_tty && no_color.is_none_or(OsStr::is_empty) && term != Some(OsStr::new("dumb"))
        }
        other => unreachable!("Invalid --color value {:?} recieved from Clap", other),
    }
}

fn run() -> Result<()> {
    let matches = cli().get_matches();

    let input = get_input(&matches)?;

    let color = should_color(
        matches
            .get_one::<String>("color")
            .expect("Clap's default ensures --color always has a value"),
        io::stdout().is_terminal(),
        env::var_os("NO_COLOR").as_deref(),
        env::var_os("TERM").as_deref(),
    );
    let border_sgr = matches
        .get_one::<String>("border-color")
        .filter(|_| color)
        .map(String::as_str);

    // Compare all the charsets
    if let Some(true) = matches.get_one::<bool>("all") {
        let charsets = vec!["thick", "thin", "double", "box", "rounded", "dot", "ascii"];

        for charset_name in charsets {
            let charset = get_charset(charset_name);
            println!("{}:", charset_name);
            print_box(
                input.clone(),
                matches.get_one::<String>("title"),
                charset,
                border_sgr,
            )?;
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
        border_sgr,
    )
}

fn print_box(
    content: Vec<String>,
    title: Option<&String>,
    charset: Charset,
    border_sgr: Option<&str>,
) -> Result<()> {
    DrawBox::new(content, title.map(String::as_str), charset)
        .border_sgr(border_sgr)
        .render(&mut io::stdout().lock())
        .context("Failed to write to stdout")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn os(value: &str) -> Option<&OsStr> {
        Some(OsStr::new(value))
    }

    #[test]
    fn test_auto_colors_a_plain_terminal() {
        assert!(should_color("auto", true, None, os("xterm-256color")));
        assert!(should_color("auto", true, None, None));
    }

    #[test]
    fn test_auto_needs_a_terminal() {
        assert!(!should_color("auto", false, None, os("xterm-256color")));
    }

    #[test]
    fn test_auto_only_honours_non_empty_no_color() {
        assert!(should_color("auto", true, os(""), os("xterm")));
        assert!(!should_color("auto", true, os("1"), os("xterm")));
        assert!(!should_color("auto", true, os("0"), os("xterm")));
    }

    #[test]
    fn test_auto_skips_dumb_terminals() {
        assert!(!should_color("auto", true, None, os("dumb")));
        assert!(should_color("auto", true, None, os("")));
    }

    #[test]
    fn test_always_overrides_everything() {
        assert!(should_color("always", false, os("1"), os("dumb")));
        assert!(should_color("always", true, None, None));
    }

    #[test]
    fn test_never_overrides_everything() {
        assert!(!should_color("never", true, None, os("xterm")));
        assert!(!should_color("never", false, os(""), None));
    }
}
