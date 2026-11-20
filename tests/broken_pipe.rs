use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;

// Far more output than a pipe buffer holds, so little_boxes is guaranteed to still be writing
// when the reader goes away
const LINES: usize = 200_000;

fn run_with_closed_stdout(args: &[&str]) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_little_boxes"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start little_boxes");

    let mut stdin = child.stdin.take().unwrap();
    let input: String = (1..=LINES).map(|n| format!("{n}\n")).collect();
    // Written from a thread so a large input can't deadlock against a full stdin pipe
    let writer = thread::spawn(move || stdin.write_all(input.as_bytes()));

    let mut stdout = child.stdout.take().unwrap();
    let mut head = [0; 64];
    stdout
        .read_exact(&mut head)
        .expect("little_boxes wrote nothing to stdout");
    drop(stdout);

    let output = child
        .wait_with_output()
        .expect("little_boxes did not finish");
    writer.join().unwrap().expect("failed to write stdin");
    assert_eq!(
        output.status.code(),
        Some(0),
        "little_boxes failed: {output:?}"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn closed_stdout_exits_quietly() {
    run_with_closed_stdout(&[]);
}

#[test]
fn closed_stdout_exits_quietly_with_all() {
    run_with_closed_stdout(&["--all"]);
}
