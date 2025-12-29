use std::process;

use autopilot_service::cli::run_cli;

fn main() {
    // Check for --verbose or -v flag
    let args: Vec<String> = std::env::args().collect();
    let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");

    if let Err(err) = run_cli(verbose) {
        eprintln!("Error: {}", err);
        process::exit(1);
    }
}
