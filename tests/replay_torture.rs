use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;

use vte::{Params, Parser, Perform};

// Deterministic so a failure reproduces; xorshift avoids adding a dev-dependency
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a>(&mut self, items: &'a [String]) -> &'a str {
        &items[self.below(items.len())]
    }

    fn rgb(&mut self) -> String {
        format!(
            "{};{};{}",
            self.below(256),
            self.below(256),
            self.below(256)
        )
    }

    fn letters(&mut self) -> String {
        (0..1 + self.below(5))
            .map(|_| (b'a' + self.below(26) as u8) as char)
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Color {
    Basic(u16),
    Bright(u16),
    Indexed(u16),
    Rgb(u16, u16, u16),
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Style {
    bold: bool,
    dim: bool,
    italic: bool,
    underline: Option<u16>,
    slow_blink: bool,
    rapid_blink: bool,
    inverse: bool,
    strike: bool,
    overline: bool,
    vertical_align: Option<u16>,
    fg: Option<Color>,
    bg: Option<Color>,
    underline_color: Option<Color>,
}

impl Style {
    // A reference model of only the SGR codes the generators below emit, written independently of
    // the code under test, which replays bytes without interpreting them
    fn apply(&mut self, params: &Params) {
        let groups: Vec<Vec<u16>> = params.iter().map(<[u16]>::to_vec).collect();
        if groups.is_empty() {
            *self = Style::default();
            return;
        }

        let mut i = 0;
        while i < groups.len() {
            let group = &groups[i];
            match group.as_slice() {
                [0] => *self = Style::default(),
                [1] => self.bold = true,
                [2] => self.dim = true,
                [3] => self.italic = true,
                [4] => self.underline = Some(1),
                [4, 0] => self.underline = None,
                [4, style] => self.underline = Some(*style),
                [22] => {
                    self.bold = false;
                    self.dim = false;
                }
                [23] => self.italic = false,
                [24] => self.underline = None,
                // Blink rates kept apart and 73-75 modelled as mintty does, so a terminal that
                // reads codes more finely than the box does still sees the right style
                [5] => self.slow_blink = true,
                [6] => self.rapid_blink = true,
                [25] => {
                    self.slow_blink = false;
                    self.rapid_blink = false;
                }
                [7] => self.inverse = true,
                [27] => self.inverse = false,
                [9] => self.strike = true,
                [29] => self.strike = false,
                [53] => self.overline = true,
                [55] => self.overline = false,
                [code @ (73 | 74)] => self.vertical_align = Some(*code),
                [75] => self.vertical_align = None,
                [code @ 30..=37] => self.fg = Some(Color::Basic(code - 30)),
                [code @ 90..=97] => self.fg = Some(Color::Bright(code - 90)),
                [39] => self.fg = None,
                [code @ 40..=47] => self.bg = Some(Color::Basic(code - 40)),
                [code @ 100..=107] => self.bg = Some(Color::Bright(code - 100)),
                [49] => self.bg = None,
                [59] => self.underline_color = None,
                [code @ (38 | 48 | 58), 2, _, r, g, b] => {
                    *self.color_slot(*code) = Some(Color::Rgb(*r, *g, *b))
                }
                [code @ (38 | 48 | 58)] => {
                    let color = match groups[i + 1].as_slice() {
                        [5] => {
                            i += 2;
                            Color::Indexed(groups[i][0])
                        }
                        [2] => {
                            i += 4;
                            Color::Rgb(groups[i - 2][0], groups[i - 1][0], groups[i][0])
                        }
                        other => panic!("reference model does not handle {code};{other:?}"),
                    };
                    *self.color_slot(*code) = Some(color);
                }
                other => panic!("reference model does not handle SGR {other:?}"),
            }
            i += 1;
        }
    }

    fn color_slot(&mut self, code: u16) -> &mut Option<Color> {
        match code {
            38 => &mut self.fg,
            48 => &mut self.bg,
            58 => &mut self.underline_color,
            other => unreachable!("not a colour code: {other}"),
        }
    }
}

// What a terminal would show: every printed char with the style it was drawn in, split into lines
#[derive(Default)]
struct Screen {
    style: Style,
    lines: Vec<Vec<(char, Style)>>,
}

impl Perform for Screen {
    fn print(&mut self, c: char) {
        if self.lines.is_empty() {
            self.lines.push(Vec::new());
        }
        let style = self.style.clone();
        self.lines.last_mut().unwrap().push((c, style));
    }

    fn execute(&mut self, byte: u8) {
        if byte == b'\n' {
            self.lines.push(Vec::new());
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        assert!(
            action == 'm' && intermediates.is_empty(),
            "unexpected non-SGR CSI {action:?}"
        );
        self.style.apply(params);
    }
}

fn screen(bytes: &[u8]) -> Screen {
    let mut screen = Screen {
        lines: vec![Vec::new()],
        ..Screen::default()
    };
    Parser::new().advance(&mut screen, bytes);
    screen
}

fn run_ascii_box(input: &str) -> Vec<u8> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_little_boxes"))
        .args(["--charset", "ascii"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start little_boxes");

    let mut stdin = child.stdin.take().unwrap();
    let input = input.to_owned();
    // Written from a thread so a large output can't deadlock against a full stdin pipe
    let writer = thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child
        .wait_with_output()
        .expect("little_boxes did not finish");
    writer.join().unwrap().expect("failed to write stdin");
    assert!(output.status.success(), "little_boxes failed: {output:?}");
    output.stdout
}

// Checks the box against the input as if both were printed to a terminal, then returns the output
// size so each scenario can bound it
fn assert_box_preserves_styles(input: &str) -> usize {
    let output = run_ascii_box(input);
    let expected = screen(input.as_bytes());
    let actual = screen(&output);

    assert_eq!(
        actual.style,
        Style::default(),
        "the box left a style open after it"
    );

    // Content is generated from letters only, so anything else on a box row is border or padding
    let content_rows = &actual.lines[1..actual.lines.len() - 2];
    let input_rows = &expected.lines[..content_rows.len()];
    for (row, (actual_row, input_row)) in content_rows.iter().zip(input_rows).enumerate() {
        let letters: Vec<_> = actual_row
            .iter()
            .filter(|(c, _)| c.is_ascii_alphabetic())
            .cloned()
            .collect();
        assert_eq!(
            letters, *input_row,
            "row {row} content drawn in the wrong style"
        );

        for (c, style) in actual_row.iter().filter(|(c, _)| !c.is_ascii_alphabetic()) {
            assert_eq!(
                *style,
                Style::default(),
                "row {row}: border or padding {c:?} picked up a content style"
            );
        }
    }
    for row in [&actual.lines[0], &actual.lines[actual.lines.len() - 2]] {
        assert!(row.iter().all(|(_, style)| *style == Style::default()));
    }

    output.len()
}

fn vocabulary() -> Vec<String> {
    let mut sgr: Vec<String> = [
        "1",
        "2",
        "3",
        "4",
        "4:3",
        "4:0",
        "22",
        "23",
        "24",
        "39",
        "49",
        "59",
        "1;31",
        "2;3;44",
        "1;38;5;208",
        "4:3;58:2::0:200:255",
        "38;2;255;105;180",
        "48;2;0;0;128",
        "38:2::10:20:30",
        "22;23;24",
        "3;39;49",
    ]
    .iter()
    .map(|params| params.to_string())
    .collect();
    sgr.extend((30..=37).chain(40..=47).map(|code| code.to_string()));
    sgr.extend((0..256).step_by(17).map(|n| format!("38;5;{n}")));
    sgr.extend((0..256).step_by(23).map(|n| format!("48;5;{n}")));
    sgr.into_iter()
        .map(|params| format!("\u{1b}[{params}m"))
        .collect()
}

fn generate(
    rng: &mut Rng,
    lines: usize,
    max_tokens: usize,
    style_percent: usize,
    reset_percent: usize,
) -> String {
    let vocabulary = vocabulary();
    let mut input = String::new();
    for _ in 0..lines {
        for _ in 0..rng.below(max_tokens + 1) {
            if rng.chance(reset_percent) {
                input.push_str(if rng.chance(50) {
                    "\u{1b}[0m"
                } else {
                    "\u{1b}[m"
                });
            } else if rng.chance(style_percent) {
                input.push_str(rng.pick(&vocabulary));
            } else {
                input.push_str(&rng.letters());
            }
        }
        input.push('\n');
    }
    input
}

fn widest_line(input: &str) -> usize {
    input
        .lines()
        .map(|line| strip_ansi_escapes::strip_str(line).len())
        .max()
        .unwrap_or(0)
}

// A box row is at most its content, its padding, the borders and one replay of every distinct
// style in the vocabulary plus the closing resets
fn assert_size_bounded(input: &str, output_len: usize, replay_bound: usize) {
    let lines = input.lines().count();
    let per_row_overhead = widest_line(input) + 4 + replay_bound + "\u{1b}[0m".len();
    let bound = input.len() + (lines + 2) * per_row_overhead;
    assert!(
        output_len <= bound,
        "output {output_len} bytes exceeds the linear bound {bound} for {lines} lines"
    );
}

fn vocabulary_bytes() -> usize {
    vocabulary().iter().map(String::len).sum()
}

#[test]
fn torture_random_styles_and_text_with_rare_resets() {
    let mut rng = Rng(0x5eed_0001);
    let input = generate(&mut rng, 3000, 30, 40, 1);
    let output_len = assert_box_preserves_styles(&input);
    assert_size_bounded(&input, output_len, vocabulary_bytes());
}

#[test]
fn torture_styles_only_never_reset() {
    let mut rng = Rng(0x5eed_0002);
    let input = generate(&mut rng, 2000, 40, 95, 0);
    let output_len = assert_box_preserves_styles(&input);
    assert_size_bounded(&input, output_len, vocabulary_bytes());
}

#[test]
fn torture_same_style_on_every_line_never_reset() {
    let input = "\u{1b}[1mbold\n".repeat(20_000);
    let output_len = assert_box_preserves_styles(&input);
    assert_size_bounded(&input, output_len, "\u{1b}[1m".len());
}

#[test]
fn torture_one_huge_line_then_plain_lines() {
    let mut rng = Rng(0x5eed_0003);
    let vocabulary = vocabulary();
    let mut input = String::new();
    for _ in 0..100_000 {
        input.push_str(rng.pick(&vocabulary));
        if rng.chance(10) {
            input.push_str(&rng.letters());
        }
    }
    input.push('\n');
    for _ in 0..2000 {
        input.push_str(&rng.letters());
        input.push('\n');
    }
    let output_len = assert_box_preserves_styles(&input);
    assert_size_bounded(&input, output_len, vocabulary_bytes());
}

#[test]
fn torture_leading_reset_sequences_replace_the_replay() {
    let mut rng = Rng(0x5eed_0004);
    let mut input = String::new();
    for _ in 0..5000 {
        let params = match rng.below(3) {
            0 => format!("0;{}", 30 + rng.below(8)),
            1 => format!(";1;{}", 40 + rng.below(8)),
            _ => format!("0;38;5;{}", rng.below(256)),
        };
        input.push_str(&format!("\u{1b}[{params}m{}\n", rng.letters()));
    }
    let output_len = assert_box_preserves_styles(&input);
    assert_size_bounded(&input, output_len, "\u{1b}[0;38;5;255m".len());
}

// Every SGR is distinct, so only overwriting slots, not dropping repeats, keeps the replay short
fn generate_restyling(
    rng: &mut Rng,
    lines: usize,
    mut style: impl FnMut(&mut Rng) -> String,
) -> String {
    let mut input = String::new();
    for _ in 0..lines {
        for _ in 0..1 + rng.below(6) {
            input.push_str(&format!("\u{1b}[{}m", style(rng)));
            input.push_str(&rng.letters());
        }
        input.push('\n');
    }
    input
}

fn sgr_len(params: &str) -> usize {
    format!("\u{1b}[{params}m").len()
}

#[test]
fn torture_truecolor_gradient_never_reset() {
    let mut rng = Rng(0x5eed_0005);
    let input = generate_restyling(&mut rng, 2000, |rng| format!("38;2;{}", rng.rgb()));
    let output_len = assert_box_preserves_styles(&input);
    assert_size_bounded(&input, output_len, sgr_len("38;2;255;255;255"));
}

#[test]
fn torture_simultaneous_colour_gradients_never_reset() {
    let mut rng = Rng(0x5eed_0006);
    let input = generate_restyling(&mut rng, 2000, |rng| match rng.below(4) {
        0 => format!("38;2;{}", rng.rgb()),
        1 => format!("48;5;{}", rng.below(256)),
        2 => format!("58:2::{}", rng.rgb().replace(';', ":")),
        _ => format!("4:{}", 1 + rng.below(5)),
    });
    let output_len = assert_box_preserves_styles(&input);
    let replay_bound = sgr_len("38;2;255;255;255")
        + sgr_len("48;5;255")
        + sgr_len("58:2::255:255:255")
        + sgr_len("4:5");
    assert_size_bounded(&input, output_len, replay_bound);
}

// A kept sequence always has a slot nothing after it writes, so of all the sequences writing the
// same set of slots only the last can be kept
#[test]
fn torture_composite_sequences_never_reset() {
    let mut rng = Rng(0x5eed_0007);
    let input = generate_restyling(&mut rng, 2000, |rng| match rng.below(5) {
        0 | 1 => format!("1;38;2;{}", rng.rgb()),
        2 => format!("2;48;2;{}", rng.rgb()),
        3 => "22".to_string(),
        _ if rng.chance(50) => format!("3;58;5;{}", rng.below(256)),
        _ => "23;59".to_string(),
    });
    let output_len = assert_box_preserves_styles(&input);
    let replay_bound = sgr_len("1;38;2;255;255;255")
        + sgr_len("2;48;2;255;255;255")
        + sgr_len("22")
        + sgr_len("3;58;5;255");
    assert_size_bounded(&input, output_len, replay_bound);
}

#[test]
fn torture_gradient_mixed_with_unknown_sequences_never_reset() {
    let mut rng = Rng(0x5eed_0008);
    let known = [
        "5", "6", "25", "5;6", "7", "27", "9", "29", "53", "55", "100", "107",
    ]
    .map(String::from);
    // The box cannot tell which slots these write, so it keeps them all
    let unknown = ["73", "74", "75", "1;73", "25;74"].map(String::from);
    let input = generate_restyling(&mut rng, 2000, |rng| match rng.below(4) {
        0 => format!("38;2;{}", rng.rgb()),
        1 => format!("9{}", rng.below(8)),
        2 => rng.pick(&known).to_string(),
        _ => rng.pick(&unknown).to_string(),
    });
    let output_len = assert_box_preserves_styles(&input);
    let replay_bound = sgr_len("38;2;255;255;255")
        + known
            .iter()
            .chain(&unknown)
            .map(|params| sgr_len(params))
            .sum::<usize>();
    assert_size_bounded(&input, output_len, replay_bound);
}

#[test]
fn torture_open_styles_across_empty_lines() {
    let mut input = String::from("\u{1b}[1;4:3;58:2::1:2:3;38;2;9;8;7mopen\n");
    input.push_str(&"\n".repeat(5000));
    input.push_str("still\u{1b}[0mclosed\n");
    let output_len = assert_box_preserves_styles(&input);
    assert_size_bounded(&input, output_len, input.lines().next().unwrap().len());
}
