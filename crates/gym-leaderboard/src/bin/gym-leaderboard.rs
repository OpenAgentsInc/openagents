//! `gym-leaderboard`: generate or check the Gym's published results.
//!
//! ```text
//! gym-leaderboard build [--root DIR] [--out DIR] [--commit REV]
//! gym-leaderboard check [--root DIR] [--out DIR]
//! ```
//!
//! `build` regenerates `leaderboard.v1.json` and every trace bundle from
//! the committed evidence under `--root` (default: the current directory)
//! into `--out` (default: `bench/terminal-bench/published` under the root),
//! and appends the digest to `index.json` when it's new. `check`
//! regenerates in memory and exits 1 if any committed file differs or any
//! bundle matched a credential rule.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return usage();
    };
    let mut root = PathBuf::from(".");
    let mut out: Option<PathBuf> = None;
    let mut commit: Option<String> = None;
    while let Some(flag) = args.next() {
        let Some(value) = args.next() else {
            return usage();
        };
        match flag.as_str() {
            "--root" => root = PathBuf::from(value),
            "--out" => out = Some(PathBuf::from(value)),
            "--commit" => commit = Some(value),
            _ => return usage(),
        }
    }
    let out = out.unwrap_or_else(|| root.join(gym_leaderboard::PUBLISHED));
    let output = match gym_leaderboard::generate(&root) {
        Ok(output) => output,
        Err(e) => {
            eprintln!("gym-leaderboard: {e}");
            return ExitCode::FAILURE;
        }
    };
    match command.as_str() {
        "build" => {
            if let Err(e) = gym_leaderboard::write(&out, &output, commit.as_deref()) {
                eprintln!("gym-leaderboard: {e}");
                return ExitCode::FAILURE;
            }
            println!(
                "wrote {} boards and {} trace bundles to {} (digest {})",
                output.leaderboard.boards.len(),
                output.bundles.len(),
                out.display(),
                output.leaderboard.digest
            );
            for board in &output.leaderboard.boards {
                println!("  {}: {}", board.id, board.headline);
            }
            ExitCode::SUCCESS
        }
        "check" => {
            let problems = gym_leaderboard::check(&out, &output);
            if problems.is_empty() {
                println!(
                    "{} matches the evidence (digest {})",
                    out.display(),
                    output.leaderboard.digest
                );
                ExitCode::SUCCESS
            } else {
                for p in &problems {
                    eprintln!("gym-leaderboard: {p}");
                }
                ExitCode::FAILURE
            }
        }
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: gym-leaderboard build [--root DIR] [--out DIR] [--commit REV]\n       gym-leaderboard check [--root DIR] [--out DIR]"
    );
    ExitCode::from(2)
}
