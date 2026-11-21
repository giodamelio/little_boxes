use std::path::PathBuf;

use clap::{Arg, Command, arg, command, value_parser};

// The options of `draw`. They are also defined on the top-level command, which is how `draw`
// becomes the default: with no subcommand, clap parses them there instead.
fn draw_args() -> [Arg; 6] {
    [
        arg!(-t --title <TITLE> "Add a title to the box")
            .required(false)
            .value_parser(parse_title),
        arg!(-c --charset <CHARSET> "The charset to draw the box with")
            .value_parser(["thick", "thin", "double", "box", "rounded", "dot", "ascii"])
            .default_value("thick"),
        arg!(-f --file <FILE> "Read input from a file instead of stdin")
            .value_parser(value_parser!(PathBuf)),
        arg!(--all "Compare all charsets"),
        arg!(--"border-color" <SPEC> "Color the border: a name like red or bright-blue, a 256-color index 0-255, or #rrggbb")
            .value_parser(parse_border_color),
        arg!(--color <WHEN> "When to color the border")
            .value_parser(["auto", "always", "never"])
            .default_value("auto"),
    ]
}

pub fn cli() -> Command {
    command!()
        // So the top-level copies of draw's options cannot be mixed with another subcommand
        .args_conflicts_with_subcommands(true)
        .args(draw_args())
        .subcommand(
            Command::new("draw")
                .about("Draw a box around the input (the default)")
                .args(draw_args()),
        )
        .subcommand(
            Command::new("completions")
                .about("Print a shell completion script")
                .arg(
                    Arg::new("shell")
                        .value_name("SHELL")
                        .required(true)
                        .value_parser(["bash", "elvish", "fish", "nushell", "powershell", "zsh"]),
                ),
        )
        .subcommand(Command::new("man").about("Print the man page"))
}

// The title is drawn inside the top border, so a line break would split the border in two
pub fn parse_title(title: &str) -> Result<String, String> {
    if title.contains(['\n', '\r']) {
        return Err("the title must be a single line; remove its line breaks".to_string());
    }
    Ok(title.to_string())
}

const NAMED_COLORS: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

// Returns the SGR escape that starts the colour rather than a colour type, because build.rs
// includes this file on its own and would see any type only main.rs reads as dead code
pub fn parse_border_color(spec: &str) -> Result<String, String> {
    let (base, name) = match spec.strip_prefix("bright-") {
        Some(name) => (90, name),
        None => (30, spec),
    };
    if let Some(index) = NAMED_COLORS.iter().position(|&named| named == name) {
        return Ok(format!("\u{1b}[{}m", base + index));
    }

    // u8's parser accepts a leading '+', so digits are checked first
    if spec.bytes().all(|b| b.is_ascii_digit())
        && let Ok(index) = spec.parse::<u8>()
    {
        return Ok(format!("\u{1b}[38;5;{index}m"));
    }

    if let Some(hex) = spec.strip_prefix('#')
        && hex.len() == 6
        && hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap();
        return Ok(format!(
            "\u{1b}[38;2;{};{};{}m",
            channel(0),
            channel(2),
            channel(4)
        ));
    }

    Err(format!(
        "expected a color name ({}, or any of them prefixed with bright-), a 256-color index 0-255, or a hex color #rrggbb",
        NAMED_COLORS.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_title_accepts_single_line() {
        assert_eq!(parse_title("hello"), Ok("hello".to_string()));
        assert_eq!(parse_title(""), Ok(String::new()));
        assert_eq!(
            parse_title("\u{1b}[31mred\u{1b}[0m\tafter tab"),
            Ok("\u{1b}[31mred\u{1b}[0m\tafter tab".to_string())
        );
    }

    #[test]
    fn test_parse_title_rejects_line_breaks() {
        let err = parse_title("a\nb").unwrap_err();
        assert!(err.contains("single line"), "{}", err);
        assert!(parse_title("a\rb").is_err());
        assert!(parse_title("a\r\nb").is_err());
        assert!(parse_title("title\n").is_err());
    }

    // clap only checks the definition's consistency in debug builds, so assert it once here
    #[test]
    fn test_cli_definition_is_valid() {
        cli().debug_assert();
    }

    #[test]
    fn test_parse_named_colors() {
        assert_eq!(parse_border_color("black"), Ok("\u{1b}[30m".to_string()));
        assert_eq!(parse_border_color("red"), Ok("\u{1b}[31m".to_string()));
        assert_eq!(parse_border_color("white"), Ok("\u{1b}[37m".to_string()));
        assert_eq!(
            parse_border_color("bright-black"),
            Ok("\u{1b}[90m".to_string())
        );
        assert_eq!(
            parse_border_color("bright-white"),
            Ok("\u{1b}[97m".to_string())
        );
    }

    #[test]
    fn test_parse_index_colors() {
        assert_eq!(parse_border_color("0"), Ok("\u{1b}[38;5;0m".to_string()));
        assert_eq!(
            parse_border_color("255"),
            Ok("\u{1b}[38;5;255m".to_string())
        );
        assert!(parse_border_color("256").is_err());
        assert!(parse_border_color("-1").is_err());
        assert!(parse_border_color("+5").is_err());
    }

    #[test]
    fn test_parse_hex_colors() {
        assert_eq!(
            parse_border_color("#ff8000"),
            Ok("\u{1b}[38;2;255;128;0m".to_string())
        );
        assert_eq!(
            parse_border_color("#FFfF00"),
            Ok("\u{1b}[38;2;255;255;0m".to_string())
        );
        assert!(parse_border_color("#fff").is_err());
        assert!(parse_border_color("#ff80000").is_err());
        assert!(parse_border_color("#gg0000").is_err());
        assert!(parse_border_color("#+f0000").is_err());
        assert!(parse_border_color("ff8000").is_err());
    }

    #[test]
    fn test_parse_rejects_unknown_names() {
        let err = parse_border_color("purple").unwrap_err();
        assert!(err.contains("bright-"), "{}", err);
        assert!(err.contains("0-255"), "{}", err);
        assert!(err.contains("#rrggbb"), "{}", err);
        assert!(parse_border_color("bright-").is_err());
        assert!(parse_border_color("bright-bright-red").is_err());
        assert!(parse_border_color("Red").is_err());
        assert!(parse_border_color("").is_err());
    }
}
