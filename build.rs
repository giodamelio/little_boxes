use std::env::var_os;
use std::fs::write;
use std::io::{ErrorKind, Result};
use std::path::PathBuf;

// This is a bit of a hack to not have to rewrite the cli in the build
#[path = "src/cli.rs"]
mod cli;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/cli.rs");

    let out_dir = PathBuf::from(var_os("OUT_DIR").ok_or(ErrorKind::NotFound)?);
    println!(
        "cargo:warning=manpages built at {:?}",
        out_dir.join("little_boxes.1")
    );

    let cmd = cli::cli();

    let man = clap_mangen::Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;

    write(out_dir.join("little_boxes.1"), buffer)?;

    Ok(())
}