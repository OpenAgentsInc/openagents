//! `grep [-i] [-E] TEXT [PATH...]`: print `path:line:text` for each line
//! that contains TEXT, searching directories recursively. TEXT may hold
//! alternatives separated by `\|` (or `|` with `-E`). `-n`, `-r`, `-R`,
//! and `-F` are accepted and change nothing: lines are always numbered and
//! directories always searched. Exits 1 when nothing matches, as grep does.

use std::path::Path;
use std::process::ExitCode;

fn search(path: &Path, needles: &[String], fold: bool, hits: &mut usize) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut entries: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        entries.sort();
        for entry in entries {
            search(&entry, needles, fold, hits);
        }
        return;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let shown = path.display().to_string();
    let shown = shown.strip_prefix("./").unwrap_or(&shown);
    for (number, line) in text.lines().enumerate() {
        let hay = if fold {
            line.to_lowercase()
        } else {
            line.to_string()
        };
        let found = needles.iter().any(|n| hay.contains(n.as_str()));
        if found {
            *hits += 1;
            println!("{shown}:{}:{line}", number + 1);
        }
    }
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let (mut fold, mut extended) = (false, false);
    while let Some(flag) = args
        .first()
        .filter(|a| a.starts_with('-') && a.len() > 1)
        .cloned()
    {
        for c in flag.chars().skip(1) {
            match c {
                'i' => fold = true,
                'E' => extended = true,
                'n' | 'r' | 'R' | 'F' | 'H' => {}
                _ => {
                    eprintln!("grep: -{c} is not supported; use -i, -E, or plain text");
                    return ExitCode::from(2);
                }
            }
        }
        args.remove(0);
    }
    let Some(pattern) = args.first().cloned() else {
        eprintln!("usage: grep [-i] [-E] TEXT [PATH...]");
        return ExitCode::from(2);
    };
    let pattern = if fold {
        pattern.to_lowercase()
    } else {
        pattern
    };
    let separator = if extended { "|" } else { "\\|" };
    let needles: Vec<String> = pattern.split(separator).map(str::to_string).collect();
    let mut paths: Vec<String> = args.into_iter().skip(1).collect();
    if paths.is_empty() {
        paths.push(".".into());
    }
    let mut hits = 0;
    for path in &paths {
        search(Path::new(path), &needles, fold, &mut hits);
    }
    if hits == 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
