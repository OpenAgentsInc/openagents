//! `grid-foreman`: the Grid Foreman's host (#12445).
//!
//! ```text
//! grid-foreman serve --state DIR [--relay URL] [--site URL] [--seconds N] [--auto-start]
//! grid-foreman new-key --state DIR     makes DIR/key (0600) and prints its public key
//! grid-foreman pubkey --state DIR      prints the public key of DIR/key
//! ```
//!
//! The secret key is never printed.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use grid_foreman::host;

fn usage() -> ExitCode {
    eprintln!(
        "usage: grid-foreman serve --state DIR [--relay URL] [--site URL] [--seconds N] [--auto-start]\n       grid-foreman new-key --state DIR\n       grid-foreman pubkey --state DIR"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().cloned() else {
        return usage();
    };
    let option = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let Some(state) = option("--state").map(PathBuf::from) else {
        return usage();
    };
    let result = match command.as_str() {
        "serve" => {
            let run_for = match option("--seconds").map(|s| s.parse::<u64>()) {
                Some(Ok(n)) => Some(Duration::from_secs(n)),
                Some(Err(_)) => return usage(),
                None => None,
            };
            host::serve(&host::Options {
                state,
                relay: option("--relay").unwrap_or_else(|| host::RELAY.into()),
                site: option("--site").unwrap_or_else(|| host::SITE.into()),
                run_for,
                auto_start: args.iter().any(|a| a == "--auto-start"),
            })
        }
        "new-key" => host::new_key(&state.join(host::KEY_FILE)).map(|pubkey| println!("{pubkey}")),
        "pubkey" => host::load_key(&state).map(|id| println!("{}", id.signer.pubkey())),
        _ => return usage(),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("grid-foreman: {error}");
            ExitCode::FAILURE
        }
    }
}
