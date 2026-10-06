//! `retail-qualify`: the retail cloud's acceptance run, funded-qualification
//! plan, and launch gate. See `docs/cloud/retail-qualification.md`.

use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("usage: retail-qualify accept [--out PATH]");
    ExitCode::from(2)
}

fn write(json: &str, out: Option<&String>) -> ExitCode {
    match out {
        Some(path) => match std::fs::write(path, format!("{json}\n")) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("cannot write {path}: {error}");
                ExitCode::FAILURE
            }
        },
        None => {
            println!("{json}");
            ExitCode::SUCCESS
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1));
    match args.first().map(String::as_str) {
        Some("accept") => {
            let receipt = retail_qualify::acceptance::run();
            let Ok(json) = serde_json::to_string_pretty(&receipt) else {
                return ExitCode::FAILURE;
            };
            let code = write(&json, out);
            if receipt.passed {
                code
            } else {
                ExitCode::FAILURE
            }
        }
        _ => usage(),
    }
}
