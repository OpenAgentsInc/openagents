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

use anyhow::Result;
use tokio::task::LocalSet;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let local = LocalSet::new();
    match local
        .run_until(pylon_tui::run_pylon_tui_with_args(args))
        .await
    {
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
