//! `grep [-i] TEXT [PATH...]`: print `path:line:text` for each line that
//! contains TEXT, searching directories recursively. Exits 1 when nothing
//! matches, as grep does.

use std::path::Path;
use std::process::ExitCode;

fn search(path: &Path, needle: &str, fold: bool, hits: &mut usize) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut entries: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        entries.sort();
        for entry in entries {
            search(&entry, needle, fold, hits);
        }
        return;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let shown = path.display().to_string();
    let shown = shown.strip_prefix("./").unwrap_or(&shown);
    for (number, line) in text.lines().enumerate() {
        let found = if fold {
            line.to_lowercase().contains(needle)
        } else {
            line.contains(needle)
        };
        if found {
            *hits += 1;
            println!("{shown}:{}:{line}", number + 1);
        }
    }
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let fold = args.first().is_some_and(|a| a == "-i");
    if fold {
        args.remove(0);
    }
    let Some(needle) = args.first().cloned() else {
        eprintln!("usage: grep [-i] TEXT [PATH...]");
        return ExitCode::from(2);
    };
    let needle = if fold { needle.to_lowercase() } else { needle };
    let mut paths: Vec<String> = args.into_iter().skip(1).collect();
    if paths.is_empty() {
        paths.push(".".into());
    }
    let mut hits = 0;
    for path in &paths {
        search(Path::new(path), &needle, fold, &mut hits);
    }
    if hits == 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
