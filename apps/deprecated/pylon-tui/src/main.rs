#![allow(
    clippy::exit,
    reason = "CLI help flow exits immediately after printing usage."
)]
#![allow(
    clippy::print_stdout,
    reason = "CLI utility intentionally writes operational output to stdout."
)]
#![allow(
    clippy::print_stderr,
    reason = "CLI utility intentionally writes usage/errors to stderr."
)]

use anyhow::{Context, Result};
use tokio::task::LocalSet;

fn main() -> Result<()> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let args = match pylon::strip_startup_thread_limit_args(raw_args) {
        Ok((args, limit)) => {
            pylon::apply_startup_thread_limit(limit);
            args
        }
        Err(error) => {
            eprintln!("{error}");
            eprintln!("{}", pylon_tui::usage());
            std::process::exit(1);
        }
    };
    if matches!(args.as_slice(), [flag] if flag == "--version" || flag == "-V") {
        println!("pylon-tui {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build pylon-tui runtime")?;
    let local = LocalSet::new();
    match local.block_on(&runtime, pylon_tui::run_pylon_tui_with_args(args)) {
        Ok(()) => Ok(()),
        Err(error) if error.to_string() == pylon_tui::usage() => {
            println!("{}", pylon_tui::usage());
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("{error}");
            eprintln!("{}", pylon_tui::usage());
            std::process::exit(1);
        }
    }
}
