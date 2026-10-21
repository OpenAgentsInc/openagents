//! `rhai FILE`: run a Rhai script. `print` goes to standard output; an
//! error goes to standard error with its line, and the exit code is 1.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(file) = std::env::args().nth(1) else {
        eprintln!("usage: rhai FILE");
        return ExitCode::from(2);
    };
    let path = Path::new(&file);
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("rhai: {file}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let engine = kiln_tools::engine(&kiln_tools::script_dir(path));
    match engine.run(&source) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{file}: {}", kiln_tools::describe(&error));
            ExitCode::FAILURE
        }
    }
}
