//! `write PATH`: write standard input to PATH, creating its directories.

use std::io::Read;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: write PATH < content");
        return ExitCode::from(2);
    };
    let mut content = Vec::new();
    if let Err(error) = std::io::stdin().read_to_end(&mut content) {
        eprintln!("write: reading input: {error}");
        return ExitCode::FAILURE;
    }
    if let Some(parent) = std::path::Path::new(&path).parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        eprintln!("write: {}: {error}", parent.display());
        return ExitCode::FAILURE;
    }
    match std::fs::write(&path, &content) {
        Ok(()) => {
            println!("wrote {path} ({} bytes)", content.len());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("write: {path}: {error}");
            ExitCode::FAILURE
        }
    }
}
