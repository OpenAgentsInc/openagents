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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match pylon::parse_args(args) {
        Ok(cli) => cli,
        Err(error) if error.to_string() == pylon::usage() => {
            println!("{}", pylon::usage());
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("{error}");
            eprintln!("{}", pylon::usage());
            std::process::exit(1);
        }
    };

    if let Some(output) = pylon::run_cli(cli).await? {
        println!("{output}");
    }
    Ok(())
}
