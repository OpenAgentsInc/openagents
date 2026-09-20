//! `coderbench`: run an episode, or judge one somebody already ran.
//!
//! ```sh
//! coderbench run devin-fan-out-six
//! coderbench diff devin-fan-out-six runs/one.atif.jsonl
//! ```
//!
//! `run` reads the task, refuses when the machine does not hold what the
//! task requires, drives `coder -p`, reads back the trace it named, judges
//! it, and prints every fault. `diff` is the same path with the run step
//! removed, which is what an operator wants after a session they drove by
//! hand.
//!
//! Refusing before the run is the part worth keeping. A run at the wrong
//! commit, or without the executor the task delegates to, produces faults
//! that are about the machine, and somebody spends an afternoon reading
//! them as faults in the agent.
//!
//! `run` is also the only mode that can hand back a pass. It sees the exit
//! code, and it reads the checkout before and after, so it can say how the
//! episode ended and what it wrote. `diff` sees a file. A trace does not
//! carry either fact, so a hand-judged trace is `unverifiable` at best —
//! which is the honest answer, not a shortcoming to route around.

use std::path::{Path, PathBuf};
use std::time::Duration;

use coderbench::drive::{self, Outcome};
use coderbench::preflight;
use coderbench::{Observed, Task, Verdict, Workspace, load_task, observe};

/// The run took the path the task expects.
const EXIT_CLEAN: u8 = 0;
/// The run left the path, and every way it did is printed.
const EXIT_FAULTS: u8 = 1;
/// The machine does not hold what the task requires, so nothing ran.
const EXIT_REFUSED: u8 = 2;
/// There is nothing to judge: no trace, or one that will not read back.
const EXIT_NO_TRACE: u8 = 3;
/// The evidence a judgment needs is missing. Not a pass, and not a fault in
/// the agent.
const EXIT_UNVERIFIABLE: u8 = 4;
/// The command line was wrong. The code headless mode uses, for the same
/// reason: it sits outside the codes a judgment produces.
const EXIT_USAGE: u8 = 64;

const USAGE: &str = "\
coderbench — run a Coder episode and judge it against the path it owes.

Usage:
  coderbench run <TASK>            Run the task and judge what it did.
  coderbench diff <TASK> <TRACE>   Judge a trace somebody already has.

Options for run:
      --repository <DIR>  The checkout to run in. Default: this directory.
      --coder <PATH>      The coder binary. Default: CODERBENCH_CODER, the
                          binary beside this one, then PATH.
      --trace <PATH>      Where the run's trace lands. Default: a new file
                          under ~/.openagents/coderbench/. The path must
                          not exist: a session never writes over another
                          session's record.
      --timeout <SECS>    Override the task's own timeout.
  -h, --help              Show this text.

<TASK> is a task identifier, or a path to a task.json.

Exit codes:
  0   The run took the path the task expects, and the evidence shows it.
  1   The run left the path. Every fault is printed.
  2   The machine does not hold what the task requires. Nothing ran.
  3   There is no trace to judge.
  4   The evidence a judgment needs is missing. Not a pass.
  64  The command line was wrong.";

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let code = match parse(&arguments) {
        Ok(Command::Help) => {
            println!("{USAGE}");
            EXIT_CLEAN
        }
        Ok(Command::Run(options)) => run(&options),
        Ok(Command::Diff { task, trace }) => diff(&task, &trace),
        Err(why) => {
            eprintln!("coderbench: {why}");
            eprintln!("\n{USAGE}");
            EXIT_USAGE
        }
    };
    std::process::exit(i32::from(code));
}

/// What the command line asked for.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    Run(Options),
    Diff { task: String, trace: PathBuf },
    Help,
}

/// One run.
#[derive(Debug, PartialEq, Eq)]
struct Options {
    task: String,
    repository: Option<PathBuf>,
    coder: Option<PathBuf>,
    trace: Option<PathBuf>,
    timeout: Option<u64>,
}

/// Reads the command line.
fn parse(arguments: &[String]) -> Result<Command, String> {
    let mut free: Vec<String> = Vec::new();
    let mut repository = None;
    let mut coder = None;
    let mut trace = None;
    let mut timeout = None;

    let mut rest = arguments.iter();
    while let Some(argument) = rest.next() {
        let (flag, attached) = match argument.split_once('=') {
            Some((flag, value)) if flag.starts_with('-') => (flag, Some(value.to_string())),
            _ => (argument.as_str(), None),
        };
        let mut value = |flag: &str| -> Result<String, String> {
            match attached.clone() {
                Some(value) => Ok(value),
                None => rest
                    .next()
                    .filter(|next| !next.starts_with('-'))
                    .cloned()
                    .ok_or_else(|| format!("{flag} needs a value")),
            }
        };
        match flag {
            "-h" | "--help" => return Ok(Command::Help),
            "--repository" => repository = Some(PathBuf::from(value("--repository")?)),
            "--coder" => coder = Some(PathBuf::from(value("--coder")?)),
            "--trace" => trace = Some(PathBuf::from(value("--trace")?)),
            "--timeout" => {
                let seconds = value("--timeout")?;
                timeout = Some(
                    seconds
                        .parse::<u64>()
                        .map_err(|_| format!("--timeout takes seconds, not {seconds}"))?,
                );
            }
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(format!("unknown option {other}"));
            }
            _ => free.push(argument.clone()),
        }
    }

    match free.split_first() {
        None => Err("run a task, or diff a trace against one".to_string()),
        Some((verb, rest)) if verb == "run" => match rest {
            [task] => Ok(Command::Run(Options {
                task: task.clone(),
                repository,
                coder,
                trace,
                timeout,
            })),
            [] => Err("run needs a task".to_string()),
            _ => Err("run takes one task".to_string()),
        },
        Some((verb, rest)) if verb == "diff" => match rest {
            [task, trace] => Ok(Command::Diff {
                task: task.clone(),
                trace: PathBuf::from(trace),
            }),
            _ => Err("diff needs a task and a trace".to_string()),
        },
        Some((verb, _)) => Err(format!("no such command as {verb}")),
    }
}

/// Runs the task and judges what it did.
fn run(options: &Options) -> u8 {
    let task = match load_task(&options.task) {
        Ok(task) => task,
        Err(why) => return complain(&why, EXIT_USAGE),
    };
    let repository = match options
        .repository
        .clone()
        .map_or_else(std::env::current_dir, Ok)
    {
        Ok(repository) => drive::absolute(&repository),
        Err(why) => return complain(&format!("no directory to run in — {why}"), EXIT_USAGE),
    };

    println!("{} — {}", task.id, task.request);
    println!();

    // Every requirement is printed, met or not, because the faults below
    // mean one thing at the base commit and another thing anywhere else.
    let checked = preflight::check(&task, &repository);
    for one in &checked {
        println!(
            "  {} {:<44} {}",
            if one.met { "ok   " } else { "unmet" },
            one.requirement,
            one.found
        );
    }
    let unmet = preflight::unmet(&checked);
    if !unmet.is_empty() {
        println!();
        println!(
            "Refused before starting Coder. {} requirement{} of {} did not hold:",
            unmet.len(),
            if unmet.len() == 1 { "" } else { "s" },
            checked.len()
        );
        for one in unmet {
            println!("  {} — {}", one.requirement, one.found);
        }
        println!();
        println!(
            "A run this machine cannot hold up produces faults about the machine. \
             Nothing was started."
        );
        return EXIT_REFUSED;
    }

    let binary = match drive::find_coder(options.coder.as_deref()) {
        Ok(binary) => binary,
        Err(why) => return complain(&why, EXIT_USAGE),
    };
    let trace = drive::absolute(
        &options
            .trace
            .clone()
            .unwrap_or_else(|| fresh_trace(&task.id)),
    );
    let timeout = Duration::from_secs(options.timeout.unwrap_or(task.timeout_secs));

    println!();
    println!("  coder      {}", binary.display());
    println!("  trace      {}", trace.display());
    println!("  timeout    {} s", timeout.as_secs());
    println!();
    println!("Running…");

    // What the checkout looks like before the run, so what it looks like
    // afterwards means something. A task that forbids writes is graded
    // against this rather than against what the run says about itself.
    let before = preflight::worktree(&repository);

    let ran = match drive::coder(&binary, &repository, &task.request, &trace, timeout) {
        Ok(ran) => ran,
        Err(why) => return complain(&why, EXIT_NO_TRACE),
    };
    println!(
        "  {} in {:.1} s. Reply in {}, progress in {}.",
        ran.outcome,
        ran.seconds,
        ran.stdout.display(),
        ran.stderr.display()
    );
    if ran.outcome == Outcome::Usage {
        return complain(
            "coder refused the command line this built. The two are out of step.",
            EXIT_USAGE,
        );
    }

    // A turn that did not finish still recorded what it got to, and those
    // steps are worth judging. Only a missing trace stops the judgment.
    let mut run = match observe(&ran.trace) {
        Ok(run) => run,
        Err(why) => {
            return complain(&format!("nothing to judge — {why}"), EXIT_NO_TRACE);
        }
    };
    // How the turn ended is a grading fact, not a line of commentary. A
    // run that timed out with the expected names in its partial trace is
    // not a clean run.
    run.ending = ran.outcome.into();
    run.workspace = match (before, preflight::worktree(&repository)) {
        (Ok(before), Ok(after)) => Some(Workspace {
            changed: preflight::changed(&before, &after),
        }),
        // Neither reading is a list of writes on its own, so one of them
        // failing leaves the question open rather than answered.
        _ => None,
    };
    report(&task, &run, &ran.trace)
}

/// Judges a trace somebody already has.
fn diff(name: &str, trace: &Path) -> u8 {
    let task = match load_task(name) {
        Ok(task) => task,
        Err(why) => return complain(&why, EXIT_USAGE),
    };
    let run = match observe(trace) {
        Ok(run) => run,
        Err(why) => return complain(&why, EXIT_NO_TRACE),
    };
    println!("{} — {}", task.id, task.request);
    report(&task, &run, trace)
}

/// Prints what the run did, then every way it left the path.
fn report(task: &Task, run: &Observed, trace: &Path) -> u8 {
    let verified = run
        .delegations
        .iter()
        .filter(|delegation| delegation.verified())
        .count();
    println!();
    println!("What the trace holds:");
    println!("  trace          {}", trace.display());
    println!(
        "  program        {}",
        run.program.as_deref().unwrap_or("none selected")
    );
    println!(
        "  decisions      {}",
        if run.decisions.is_empty() {
            "none".to_string()
        } else {
            run.decisions.keys().cloned().collect::<Vec<_>>().join(", ")
        }
    );
    println!(
        "  checks         {}",
        if run.checks.is_empty() {
            "none".to_string()
        } else {
            run.checks
                .iter()
                .map(|check| check.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    println!(
        "  delegations    {} started, {verified} verified correct",
        run.delegations.len()
    );
    println!(
        "  writes         {}",
        match &run.workspace {
            Some(workspace) if workspace.changed.is_empty() =>
                "the workspace is unchanged".to_string(),
            Some(workspace) => format!(
                "{} changed in the workspace: {}",
                workspace.changed.len(),
                workspace.changed.join(", ")
            ),
            None => format!(
                "{} self-reported, and nobody looked at the workspace",
                run.writes.len()
            ),
        }
    );
    println!(
        "  ended          {}{}",
        run.ending,
        if run.closed {
            String::new()
        } else {
            ", with no end record".to_string()
        }
    );
    if run.unreadable_lines > 0 {
        println!("  unreadable     {} lines", run.unreadable_lines);
    }

    let judgment = task.judge(run);
    println!();
    if judgment.faults.is_empty() {
        println!("No faults. The run took the path {} expects.", task.id);
        return EXIT_CLEAN;
    }
    println!(
        "{}: {} fault{}, in the order the path takes:",
        judgment.verdict,
        judgment.faults.len(),
        if judgment.faults.len() == 1 { "" } else { "s" }
    );
    for (index, fault) in judgment.faults.iter().enumerate() {
        println!("  {:>2}. [{}] {}", index + 1, fault.verdict(), fault);
    }
    if judgment.verdict == Verdict::Unverifiable {
        println!();
        println!(
            "Nothing here says the run left the path. It says the evidence to show it \
             took the path is missing, which is not a pass."
        );
    }
    match judgment.verdict {
        Verdict::Passed => EXIT_CLEAN,
        Verdict::Unverifiable => EXIT_UNVERIFIABLE,
        Verdict::Failed => EXIT_FAULTS,
    }
}

/// A trace path no run has taken, under a directory that is not the
/// repository being measured.
fn fresh_trace(task: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    PathBuf::from(home)
        .join(".openagents")
        .join("coderbench")
        .join(format!("{task}-{stamp}.atif.jsonl"))
}

/// Says what went wrong and hands back the code to exit with.
fn complain(why: &str, code: u8) -> u8 {
    eprintln!("coderbench: {why}");
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_of(arguments: &[&str]) -> Result<Command, String> {
        let arguments: Vec<String> = arguments.iter().map(|a| (*a).to_string()).collect();
        parse(&arguments)
    }

    /// The flags read from either side of the task, and a run says what it
    /// runs where.
    #[test]
    fn a_run_reads_its_flags() {
        let expected = Command::Run(Options {
            task: "devin-fan-out-six".to_string(),
            repository: Some(PathBuf::from("/tmp/checkout")),
            coder: None,
            trace: Some(PathBuf::from("/tmp/one.jsonl")),
            timeout: Some(30),
        });
        for arguments in [
            vec![
                "run",
                "devin-fan-out-six",
                "--repository",
                "/tmp/checkout",
                "--trace",
                "/tmp/one.jsonl",
                "--timeout",
                "30",
            ],
            vec![
                "--trace=/tmp/one.jsonl",
                "run",
                "--timeout=30",
                "--repository=/tmp/checkout",
                "devin-fan-out-six",
            ],
        ] {
            assert_eq!(parse_of(&arguments).unwrap(), expected, "{arguments:?}");
        }
    }

    /// A diff takes a task and a trace, in that order.
    #[test]
    fn a_diff_takes_a_task_and_a_trace() {
        assert_eq!(
            parse_of(&["diff", "devin-fan-out-six", "runs/one.jsonl"]).unwrap(),
            Command::Diff {
                task: "devin-fan-out-six".to_string(),
                trace: PathBuf::from("runs/one.jsonl"),
            }
        );
    }

    /// A wrong command line says what is wrong with it.
    #[test]
    fn a_wrong_command_line_says_so() {
        for (arguments, expected) in [
            (vec![], "run a task"),
            (vec!["run"], "run needs a task"),
            (vec!["run", "one", "two"], "run takes one task"),
            (vec!["diff", "one"], "diff needs a task and a trace"),
            (vec!["judge", "one"], "no such command as judge"),
            (
                vec!["run", "one", "--timeout", "soon"],
                "--timeout takes seconds",
            ),
            (vec!["run", "one", "--verbose"], "unknown option --verbose"),
        ] {
            let error = parse_of(&arguments).unwrap_err();
            assert!(
                error.contains(expected),
                "{arguments:?} said {error:?}, wanted {expected:?}"
            );
        }
    }
}
