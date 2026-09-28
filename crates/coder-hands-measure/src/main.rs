//! The command line of the hand measurement: `record`, `score`, and
//! `ask`.
//!
//! `record` is the part that needs you: it prompts for one gesture at a
//! time, counts you in, and writes every landmark line the camera
//! published with the prompt as its label. `score` replays that file
//! through the rules the desk runs and prints what they decided. `ask`
//! sends the windows the rules could not settle to the seam once and
//! keeps the answers beside the run, so a later score reads the file
//! rather than the network.
//!
//! The library beside this file is where each of the three lives, and
//! its documentation says how a run is recorded and what a score means.

use std::path::PathBuf;
use std::process::ExitCode;

use coder_hands_measure::replay::Replay;
use coder_hands_measure::run::Run;
use coder_hands_measure::{answers, ask, record, report, score};

/// What `--help` prints.
const USAGE: &str = "\
coder-hands-measure: measure the desk's hand gestures against a recorded run.

Usage:
  coder-hands-measure record [--out <file>] [--passes <n>]
      Prompt for each gesture in turn and write what the camera saw.
      The default is two passes of four gestures, about a minute.

  coder-hands-measure score <run> [--answers <file>] [--windows]
      Replay a run through the rules and print what they decided.
      With --windows, list the windows the rules could not settle first,
      and whether the trigger asked about each one or held it back.

  coder-hands-measure ask <run> [--answers <file>]
      Ask the seam about the windows the rules could not settle, once,
      and write the answers beside the run.

  coder-hands-measure --help
  coder-hands-measure --version
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.first().copied() {
        Some("--help" | "-h") | None => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("--version") => {
            println!("coder-hands-measure {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("record") => finish(do_record(&words[1..])),
        Some("score") => finish(do_score(&words[1..])),
        Some("ask") => finish(do_ask(&words[1..])),
        Some(other) => {
            eprintln!("coder-hands-measure: {other} names no command.\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

/// The exit code one command's result gives, with its complaint on the
/// error stream.
fn finish(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("coder-hands-measure: {error}");
            ExitCode::FAILURE
        }
    }
}

/// `record`.
fn do_record(words: &[&str]) -> Result<(), String> {
    let mut options = record::Options::default();
    let mut rest = words.iter();
    while let Some(word) = rest.next() {
        match *word {
            "--out" => options.out = Some(PathBuf::from(value(&mut rest, "--out")?)),
            "--passes" => {
                options.passes = value(&mut rest, "--passes")?
                    .parse()
                    .map_err(|_| "--passes takes a whole number".to_string())?;
            }
            other => return Err(format!("{other} is no option of record.\n\n{USAGE}")),
        }
    }
    record::record(&options).map(|_| ())
}

/// `score`.
fn do_score(words: &[&str]) -> Result<(), String> {
    let listing = words.contains(&"--windows");
    let words: Vec<&str> = words
        .iter()
        .copied()
        .filter(|w| *w != "--windows")
        .collect();
    let (path, answers_path) = run_and_answers(&words, "score")?;
    let run = Run::read(&path)?;
    let replay = Replay::of(&run);
    if listing {
        print!("{}", report::render_windows(&replay));
        println!();
    }
    let held = answers::read(&answers_path)?;
    let name = path.display().to_string();
    let scored = score::score(&name, &run, &replay, &held);
    print!("{}", report::render(&scored));
    Ok(())
}

/// `ask`.
fn do_ask(words: &[&str]) -> Result<(), String> {
    let (path, answers_path) = run_and_answers(words, "ask")?;
    let run = Run::read(&path)?;
    let replay = Replay::of(&run);
    let held = ask::ask(&replay, |text| println!("{text}"))?;
    if held.is_empty() {
        return Ok(());
    }
    answers::write(&answers_path, &held)?;
    println!(
        "Wrote {} answer(s) to {}.",
        held.len(),
        answers_path.display()
    );
    println!("Score it: coder-hands-measure score {}", path.display());
    Ok(())
}

/// The run a command reads and the answers file beside it.
fn run_and_answers(words: &[&str], command: &str) -> Result<(PathBuf, PathBuf), String> {
    let mut run = None;
    let mut answers_path = None;
    let mut rest = words.iter();
    while let Some(word) = rest.next() {
        match *word {
            "--answers" => {
                answers_path = Some(PathBuf::from(value(&mut rest, "--answers")?));
            }
            other if other.starts_with("--") => {
                return Err(format!("{other} is no option of {command}.\n\n{USAGE}"));
            }
            other => run = Some(PathBuf::from(other)),
        }
    }
    let run = run.ok_or_else(|| format!("{command} takes the run to read.\n\n{USAGE}"))?;
    let beside = answers_path.unwrap_or_else(|| answers::beside(&run));
    Ok((run, beside))
}

/// The word after an option, or the complaint that it is missing.
fn value<'a>(rest: &mut impl Iterator<Item = &'a &'a str>, name: &str) -> Result<String, String> {
    rest.next()
        .map(|word| (*word).to_string())
        .ok_or_else(|| format!("{name} takes a value"))
}
