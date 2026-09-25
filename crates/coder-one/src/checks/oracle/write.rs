//! The oracle writer: a separate Luna session that sees only the [`Spec`].
//!
//! The session's workspace is a fresh directory holding `spec.json` and
//! `cases.json`, and nothing from the task's workspace. Its brief carries
//! the stated definition, the input and output formats, the heads of the
//! input files, the stated parameters, and the cases. It never sees the
//! implementation or a candidate. Its commands run in a writing boundary
//! on that directory with no network, so it can test the oracle on inputs
//! it builds itself.
//!
//! It writes `oracle.py`, which a host later runs as
//! `python3 oracle.py WORKDIR CASES` against a finished workspace. The
//! oracle prints one JSON line per case ([`PROTOCOL`]).

use std::path::Path;
use std::time::Duration;

use microluna::{Brief, Config, Ending, Evidence, Isolation, Transport};
use serde_json::{Value, json};

use super::{Oracle, Source, Spec};

/// What the writer is asked to do. General: it names no task.
pub const TASK: &str = "Write an independent oracle program for a programming task. Someone \
else is solving the task; you never see their code, and you must not guess how it works. Your \
oracle decides, from the task's stated definition alone, whether a finished solution is correct.\n\n\
The evidence below holds everything the task states about what a correct result is: the \
sentences that define it, the input and output formats, the first lines of the input files, the \
stated parameter values, and the cases to check. Compute every expected value on your own, by \
the most direct route the definition allows: a brute-force computation, a formula evaluated \
step by step, or a reference program the task provides. Never copy or import the solution's \
code.\n\n\
An oracle that checks only that an output exists or has the right shape isn't enough: for each \
case, recompute the values or properties the definition states and compare them with the \
solution's. Where the definition states a constraint rather than a unique answer, check every \
stated constraint on the solution's result. Read the input files' structure from their first \
lines below, and make the oracle fail with a clear message, not guess, when a file doesn't have \
that structure.";

/// The oracle's interface.
pub const PROTOCOL: &str = "Write `oracle.py` in your working directory. It will be run as\n\n\
    python3 oracle.py WORKDIR CASES\n\n\
where WORKDIR is the task's working directory holding a finished solution, and CASES is a JSON \
file shaped like `cases.json` here. For each case, in order:\n\n\
1. Get the solution's result through the interface the task states: run the stated command, or \
read the stated output file, under WORKDIR. Bound every run of the solution to 120 seconds.\n\
2. Compute the correct result yourself from the task's definition and the task's input files \
under WORKDIR. For a parameter case, use that parameter value; when the task fixes the value \
and the interface can't change it, check the solution's result at that value. For a boundary \
case, build the boundary input in a temporary copy of WORKDIR, run the solution there, and \
check it against the rule the case's sentence states.\n\
3. Print exactly one line of JSON to standard output:\n\
   {\"case\": ID, \"verdict\": \"passed\" | \"failed\" | \"could_not_run\", \"expected\": \
TEXT, \"observed\": TEXT, \"detail\": TEXT}\n\n\
A missing or malformed output is `failed`, not `could_not_run`. Use `could_not_run` only when \
the case can't be checked at all through the stated interface. Compare numbers with a \
tolerance only when the task states one or the values are floating point. Use only the Python \
standard library, and numpy when it's importable. Never change files under WORKDIR except \
outputs the task says the solution writes, and prefer a temporary copy. Print nothing else to \
standard output.\n\n\
Before you finish, test the oracle: build a small WORKDIR of your own in this directory with a \
correct and an incorrect result, and check that it prints `passed` for the first and `failed` \
for the second. Then call finish.";

/// The writer session's bounds.
#[derive(Clone, Debug)]
pub struct Bounds {
    pub turns: usize,
    pub wall: Duration,
    /// The session's spend bound, at list price.
    pub usd: f64,
    /// The reasoning effort.
    pub effort: Option<String>,
    /// How the session's commands are confined: a writing boundary on its
    /// own directory, or the task's container when that is the boundary.
    pub isolation: Isolation,
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds {
            turns: 30,
            wall: Duration::from_secs(600),
            usd: 0.08,
            effort: Some("high".to_string()),
            isolation: Isolation::Boundary,
        }
    }
}

/// The brief the writer reads: the task and the protocol, then the spec's
/// parts as evidence.
#[must_use]
pub fn brief(spec: &Spec) -> Brief {
    let mut evidence = vec![Evidence {
        label: "The task's stated definition of a correct result".to_string(),
        text: if spec.definition.is_empty() {
            "(No sentence was picked as a definition; use the formats and the cases.)".to_string()
        } else {
            spec.definition
                .iter()
                .map(|s| format!("- {s}"))
                .collect::<Vec<_>>()
                .join("\n")
        },
    }];
    evidence.push(Evidence {
        label: "Input and output formats, as the task states them".to_string(),
        text: spec
            .formats
            .iter()
            .map(|s| format!("- {s}"))
            .collect::<Vec<_>>()
            .join("\n"),
    });
    if !spec.inputs.is_empty() {
        evidence.push(Evidence {
            label: "The first lines of the input files the task names".to_string(),
            text: spec
                .inputs
                .iter()
                .map(|i| format!("{}:\n```\n{}\n```", i.path, i.head))
                .collect::<Vec<_>>()
                .join("\n\n"),
        });
    }
    if !spec.parameters.is_empty() {
        evidence.push(Evidence {
            label: "Stated parameter values".to_string(),
            text: spec
                .parameters
                .iter()
                .map(|p| format!("- {} = {} (from: {})", p.name, p.value, p.sentence))
                .collect::<Vec<_>>()
                .join("\n"),
        });
    }
    if !spec.references.is_empty() {
        evidence.push(Evidence {
            label: "Reference programs the task provides".to_string(),
            text: spec
                .references
                .iter()
                .map(|r| format!("- {r}"))
                .collect::<Vec<_>>()
                .join("\n"),
        });
    }
    evidence.push(Evidence {
        label: "The cases, as in cases.json".to_string(),
        text: serde_json::to_string_pretty(&spec.cases_file()).unwrap_or_default(),
    });
    Brief {
        task: format!(
            "{TASK}\n\nThe task's working directory is {}.",
            spec.workdir
        ),
        guidance: PROTOCOL.to_string(),
        evidence,
        state: Vec::new(),
    }
}

/// Runs the writer session in `dir`, which it creates empty but for
/// `spec.json` and `cases.json`, and returns the oracle when the session
/// left an `oracle.py`.
///
/// # Errors
///
/// A message when the directory can't be prepared.
pub async fn write<T: Transport>(
    transport: &T,
    spec: &Spec,
    dir: &Path,
    bounds: &Bounds,
    traces: Option<&Path>,
) -> Result<(Option<Oracle>, Value), String> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let cases = serde_json::to_string_pretty(&spec.cases_file()).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("cases.json"), format!("{cases}\n")).map_err(|e| e.to_string())?;
    let spec_text = serde_json::to_string_pretty(spec).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("spec.json"), format!("{spec_text}\n")).map_err(|e| e.to_string())?;
    let seal_dir = dir.with_extension("seal");
    let seal = microluna::Seal::create(&seal_dir, true).map_err(|e| e.to_string())?;
    let workspace = microluna::Workspace::new(dir)
        .map_err(|e| e.to_string())?
        .isolated_by(bounds.isolation)
        .sealed_by(seal);
    let name = format!("oracle-writer-{}", spec.task);
    let config = Config {
        max_turns: bounds.turns,
        deadline: Some(bounds.wall),
        spend_usd: Some(bounds.usd),
        effort: bounds.effort.clone(),
        ..Config::luna(&format!(
            "oracle-writer-{}",
            &spec.digest[..16.min(spec.digest.len())]
        ))
    };
    let mut recorder = microluna::Recorder::new();
    let mut trace = None;
    if let Some(traces) = traces {
        let path = traces.join(format!("{name}.atif.jsonl"));
        let session = atif::Session::opening(
            &name,
            &config.model,
            "codex-login",
            &dir.display().to_string(),
            &crate::episode::version(),
        );
        if let Ok(log) = atif::Log::create_at(&path, &session) {
            recorder = recorder.logging(log);
            trace = Some(path);
        }
    }
    let report = microluna::run(transport, &workspace, &brief(spec), &config, &mut recorder).await;
    recorder.close(match report.ending {
        Ending::Finished => atif::log::ENDED,
        _ => atif::log::INTERRUPTED,
    });
    let _ = std::fs::remove_dir_all(&seal_dir);
    let record = json!({
        "name": name,
        "model": config.model,
        "ending": match &report.ending {
            Ending::Finished => "finished".to_string(),
            Ending::Stopped => "stopped".to_string(),
            Ending::TurnLimit => "turn_limit".to_string(),
            Ending::Deadline => "deadline".to_string(),
            Ending::Transport(why) => format!("transport: {why}"),
            Ending::Host(why) => format!("host: {why}"),
        },
        "summary": report.finish.as_ref().map(|f| f.summary.clone()),
        "turns": report.turns,
        "calls": report.calls,
        "usd": report.cost_usd.unwrap_or_default(),
        "input_tokens": report.usage.input,
        "cached_tokens": report.usage.cached,
        "output_tokens": report.usage.output,
        "milliseconds": report.milliseconds,
        "trace": trace,
        "bounds": {
            "turns": bounds.turns,
            "wall_sec": bounds.wall.as_secs(),
            "usd": bounds.usd,
            "effort": bounds.effort,
        },
    });
    let Ok(program) = std::fs::read_to_string(dir.join("oracle.py")) else {
        return Ok((None, record));
    };
    let mut files = std::collections::BTreeMap::new();
    files.insert("oracle.py".to_string(), program);
    files.insert("cases.json".to_string(), format!("{cases}\n"));
    let oracle = Oracle {
        schema: String::new(),
        task: spec.task.clone(),
        source: Source::Written,
        command: None,
        origin: None,
        files,
        spec: Some(spec.digest.clone()),
        writer: record.clone(),
        digest: String::new(),
    }
    .sealed();
    Ok((Some(oracle), record))
}
