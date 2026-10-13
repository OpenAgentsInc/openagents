//! `hook-log report <log.jsonl> [--outcomes outcomes.jsonl]`

use hook_log::Decision;
use hook_log::report::{Outcome, join, parse_lines, render};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: hook-log report <log.jsonl> [--outcomes outcomes.jsonl]";
    if args.first().map(String::as_str) != Some("report") {
        eprintln!("{usage}");
        std::process::exit(2);
    }
    let mut log = None;
    let mut outcomes = None;
    let mut it = args.into_iter().skip(1);
    while let Some(a) = it.next() {
        if a == "--outcomes" {
            outcomes = it.next();
        } else {
            log = Some(a);
        }
    }
    let Some(log) = log else {
        eprintln!("{usage}");
        std::process::exit(2);
    };
    let read = |path: &str| {
        std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("hook-log: {path}: {e}");
            std::process::exit(1);
        })
    };
    let decisions: Vec<Decision> = parse_lines(&read(&log));
    let outcomes: Vec<Outcome> = outcomes.map_or_else(Vec::new, |p| parse_lines(&read(&p)));
    print!("{}", render(&join(decisions, &outcomes)));
}
