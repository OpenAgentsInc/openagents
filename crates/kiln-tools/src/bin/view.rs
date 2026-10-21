//! `view PATH [START [END]]`: print lines START to END of a file (1-based,
//! inclusive), each after its number, at most 80 lines a call. Without END
//! it prints 40 lines from START; without START, from the top.

use std::process::ExitCode;

const MOST: usize = 80;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: view PATH [START [END]]");
        return ExitCode::from(2);
    };
    let number = |i: usize| args.get(i).and_then(|a| a.parse::<usize>().ok());
    let start = number(1).unwrap_or(1).max(1);
    let end = number(2).unwrap_or(start + 39).max(start);
    let end = end.min(start + MOST - 1);
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("view: {path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let total = text.lines().count();
    for (i, line) in text
        .lines()
        .enumerate()
        .skip(start - 1)
        .take(end + 1 - start)
    {
        println!("{:>4}: {line}", i + 1);
    }
    if end < total {
        println!("[{path} has {total} lines]");
    }
    ExitCode::SUCCESS
}
