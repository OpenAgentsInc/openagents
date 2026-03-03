#![allow(clippy::exit)]
#![allow(clippy::print_stderr)]
#![allow(clippy::print_stdout)]

use openagents_cad::cli::run_cli_env_args;

fn main() {
    let outcome = run_cli_env_args(std::env::args().collect());

    if !outcome.stdout.is_empty() {
        print!("{}", outcome.stdout);
    }
    if !outcome.stderr.is_empty() {
        eprint!("{}", outcome.stderr);
    }

    if outcome.exit_code != 0 {
        std::process::exit(outcome.exit_code);
    }
}
