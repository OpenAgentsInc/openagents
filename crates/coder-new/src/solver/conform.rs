//! The conformance check between a recorded solver run and the
//! `solve-issue` program: `coder solver conform --run DIR` reads the run's
//! `summary.json`, unrolls the program's steps the way `drive()` walked
//! them (intake, brief, then a code/check pair per round, land, done), and
//! says whether the recorded hands, rounds, and outcome are the path the
//! program would take. The program replaces `drive()` only when every
//! recorded run conforms.

use std::path::Path;

use serde_json::{Value, json};

/// Where the shipped program lives, relative to the repository root.
pub const PROGRAM: &str = "programs/solve-issue.json";

/// The step names of a program, in document order.
///
/// # Errors
/// The file is missing or is not a program document.
pub fn program_steps(bytes: &[u8]) -> Result<Vec<String>, String> {
    let doc: Value = serde_json::from_slice(bytes).map_err(|e| format!("not a program: {e}"))?;
    let steps = doc["definition"]["steps"]
        .as_array()
        .ok_or("program has no definition.steps")?;
    steps
        .iter()
        .map(|step| {
            step["name"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "a step has no name".to_owned())
        })
        .collect()
}

/// The program path a recorded run took: each hand mapped onto the step
/// that carries it. A Tester turn is the `check` of its round; the last
/// Tester turn of a landed run is `land`, and a landed run ends in `done`.
#[must_use]
pub fn recorded_path(summary: &Value) -> Vec<String> {
    let hands: Vec<&str> = summary["solver"]["hands"]
        .as_array()
        .map(|hands| {
            hands
                .iter()
                .filter_map(|turn| turn["hand"].as_str())
                .collect()
        })
        .unwrap_or_default();
    let landed = summary["outcome"]["status"] == "landed"
        || summary["solver"]["pull_request"]
            .as_str()
            .is_some_and(|url| !url.is_empty());
    let last_tester = hands.iter().rposition(|hand| *hand == "tester");
    let mut path = Vec::new();
    for (i, hand) in hands.iter().enumerate() {
        let step = match *hand {
            "intaker" => "intake",
            "briefer" => "brief",
            "coder" => "code",
            "tester" if landed && Some(i) == last_tester => "land",
            "tester" => "check",
            other => other,
        };
        path.push(step.to_owned());
    }
    if landed {
        path.push("done".to_owned());
    }
    path
}

/// The program's path over `rounds` coding rounds: the steps before
/// `code` once, `code` and `check` once per round, then the steps after
/// `check` once. A landed run with `rounds` hand-backs has `rounds + 1`
/// coding rounds, and its final `check` is `land`.
#[must_use]
pub fn program_path(steps: &[String], coding_rounds: usize, landed: bool) -> Vec<String> {
    let code = steps.iter().position(|s| s == "code");
    let check = steps.iter().position(|s| s == "check");
    let (Some(code), Some(check)) = (code, check) else {
        return steps.to_vec();
    };
    let mut path: Vec<String> = steps[..code].to_vec();
    for round in 0..coding_rounds {
        path.push(steps[code].clone());
        let last = round + 1 == coding_rounds;
        if last && landed {
            path.extend(steps[check + 1..].iter().cloned());
        } else {
            path.push(steps[check].clone());
        }
    }
    path
}

/// One run judged against the program.
#[must_use]
pub fn judge(run: &Path, summary: &Value, steps: &[String]) -> Value {
    let recorded = recorded_path(summary);
    let landed = recorded.last().is_some_and(|s| s == "done");
    let coding_rounds = recorded.iter().filter(|s| *s == "code").count();
    let expected = program_path(steps, coding_rounds, landed);
    let conforms = recorded == expected;
    let why = if conforms {
        "recorded hands walk the program's steps".to_owned()
    } else {
        format!("recorded {recorded:?} differs from the program's {expected:?}")
    };
    json!({
        "run": run.display().to_string(),
        "issue": summary["issue"],
        "outcome": summary["outcome"]["status"],
        "rounds": summary["solver"]["rounds"],
        "coding_rounds": coding_rounds,
        "recorded": recorded,
        "program": expected,
        "conforms": conforms,
        "why": why,
    })
}

/// `coder solver conform --run DIR [--run DIR ...] [--program FILE]`, one
/// JSON line per run; exits with an error when any run does not conform.
///
/// # Errors
/// An option is unknown, the program does not parse, a run has no
/// `summary.json`, or a run does not conform.
pub fn command(args: &[String]) -> Result<String, String> {
    let mut runs: Vec<std::path::PathBuf> = Vec::new();
    let mut program = std::path::PathBuf::from(PROGRAM);
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--run" => runs.push(args.next().ok_or("--run needs a directory")?.into()),
            "--program" => program = args.next().ok_or("--program needs a file")?.into(),
            other => {
                return Err(format!(
                    "unknown option {other}; coder solver conform takes --run DIR [--program FILE]"
                ));
            }
        }
    }
    if runs.is_empty() {
        return Err("coder solver conform needs at least one --run DIR".into());
    }
    let bytes = std::fs::read(&program)
        .map_err(|e| format!("cannot read program {}: {e}", program.display()))?;
    let steps = program_steps(&bytes)?;
    let mut out = String::new();
    let mut failed = 0;
    for run in &runs {
        let summary: Value = std::fs::read(run.join("summary.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .ok_or_else(|| format!("{} has no readable summary.json", run.display()))?;
        let row = judge(run, &summary, &steps);
        if row["conforms"] != true {
            failed += 1;
        }
        out.push_str(&row.to_string());
        out.push('\n');
    }
    if failed > 0 {
        return Err(format!(
            "{out}{failed} run(s) do not conform to {}",
            program.display()
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps() -> Vec<String> {
        ["intake", "brief", "code", "check", "land", "done"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    }

    fn summary(hands: &[&str], status: &str) -> Value {
        json!({
            "issue": 7,
            "outcome": {"status": status},
            "solver": {
                "hands": hands.iter().map(|h| json!({"hand": h})).collect::<Vec<_>>(),
                "rounds": hands.iter().filter(|h| **h == "tester").count().saturating_sub(1),
            },
        })
    }

    #[test]
    fn a_landed_run_with_one_hand_back_walks_the_program() {
        let s = summary(
            &["intaker", "briefer", "coder", "tester", "coder", "tester"],
            "landed",
        );
        let row = judge(Path::new("r"), &s, &steps());
        assert_eq!(row["conforms"], true, "{row}");
        assert_eq!(
            row["program"],
            json!(["intake", "brief", "code", "check", "code", "land", "done"])
        );
    }

    #[test]
    fn a_run_that_skipped_the_briefer_does_not_conform() {
        let s = summary(&["intaker", "coder", "tester"], "landed");
        let row = judge(Path::new("r"), &s, &steps());
        assert_eq!(row["conforms"], false);
    }

    #[test]
    fn a_failed_run_ends_at_its_last_check() {
        let s = summary(&["intaker", "briefer", "coder", "tester"], "check_failed");
        let row = judge(Path::new("r"), &s, &steps());
        assert_eq!(row["conforms"], true, "{row}");
        assert_eq!(row["program"], json!(["intake", "brief", "code", "check"]));
    }

    #[test]
    fn the_shipped_program_names_the_hands() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(PROGRAM);
        let bytes = std::fs::read(path).unwrap();
        assert_eq!(program_steps(&bytes).unwrap(), steps());
    }
}
