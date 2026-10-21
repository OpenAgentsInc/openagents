//! `cat PATH...`: print files.

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut failed = false;
    let mut out = std::io::stdout().lock();
    for path in std::env::args().skip(1) {
        match std::fs::read(&path) {
            Ok(bytes) => {
                let _ = out.write_all(&bytes);
            }
            Err(error) => {
                eprintln!("cat: {path}: {error}");
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
