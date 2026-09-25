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
//! The boundary also confines what its commands read: that directory, the
//! paths in [`Bounds::readable`] (the task's instruction and the untouched
//! task files it may see, when a caller grants them), a scratch directory,
//! and the system's program directories. The candidate workspace, other
//! trials' records, and the rest of the host are out of sight, so a
//! `find /` finds none of them. A host that can't confine reads refuses
//! every command rather than run it with reads open.
//!
//! A task whose boundary is a task container can't confine reads that
//! way: the container holds the candidate. There the writer runs in a
//! fresh container of the task's image instead ([`super::contain`], set by
//! [`Bounds::container`]), and only `oracle.py` comes out. Without one, a
//! writer in a task container is refused before any model call.
//!
//! It writes `oracle.py`, which a host later runs as
//! `python3 oracle.py WORKDIR CASES` against a finished workspace. The
//! oracle prints one JSON line per case ([`PROTOCOL`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;
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
    /// own directory. Their reads are always confined too, so a task
    /// container, which can't confine reads, refuses the writer unless
    /// [`Bounds::container`] gives it a container of its own.
    pub isolation: Isolation,
    /// When set, the writer runs in a fresh container of this image, with
    /// no network and nothing from the host, instead of a boundary on the
    /// host ([`super::contain`]).
    pub container: Option<super::contain::Image>,
    /// What else the writer's commands may read, besides its own
    /// directory: the task's instruction and the untouched task files it
    /// may see. Never a candidate workspace or another trial's records.
    pub readable: Vec<PathBuf>,
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds {
            turns: 30,
            wall: Duration::from_secs(600),
            usd: 0.08,
            effort: Some("high".to_string()),
            isolation: Isolation::Boundary,
            container: None,
            readable: Vec::new(),
        }
    }
}

/// What the writer in its own container is told about where it is.
pub const CONTAINED: &str = "Your commands run in a fresh container of the task's image, with no \
network. The task's files there are untouched, as the image ships them: no solution has been \
written in that container. Test your oracle there, but keep everything you make in your working \
directory: only `oracle.py` is kept.";

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
/// left an `oracle.py`. With [`Bounds::container`], the session runs in a
/// fresh container of the task's image and `oracle.py` is copied out of
/// it. A writer that can't be confined is refused before any model call:
/// the record says why under `refused`, and no oracle comes back.
///
/// # Errors
///
/// A message when the directory can't be prepared.
#[allow(clippy::too_many_lines)]
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
    let cases = format!("{cases}\n");
    let spec_text = serde_json::to_string_pretty(spec).map_err(|e| e.to_string())?;
    let spec_text = format!("{spec_text}\n");
    let name = format!("oracle-writer-{}", spec.task);
    let refused = |why: String, isolation: &str| {
        crate::say::line(&format!("  microluna ▸ {why}"));
        json!({
            "name": name,
            "refused": why,
            "isolation": isolation,
            "turns": 0,
            "usd": 0.0,
        })
    };
    let contained = match &bounds.container {
        Some(image) => {
            let label = format!(
                "oracle-writer-{}-{}-{}",
                &spec.digest[..12.min(spec.digest.len())],
                std::process::id(),
                atif::now_ms()
            );
            match super::contain::WriterContainer::start(image, &label) {
                Ok(container) => Some(Arc::new(container)),
                Err(why) => return Ok((None, refused(why, super::contain::ISOLATION))),
            }
        }
        None if bounds.isolation == Isolation::TaskContainer => {
            return Ok((
                None,
                refused(
                    "The oracle writer did not run: in a task container its commands could \
                     read the candidate's files, and no separate container was given for it."
                        .to_string(),
                    Isolation::TaskContainer.word(),
                ),
            ));
        }
        None => None,
    };
    std::fs::write(dir.join("cases.json"), &cases).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("spec.json"), &spec_text).map_err(|e| e.to_string())?;
    let seal_dir = dir.with_extension("seal");
    let workspace = microluna::Workspace::new(dir).map_err(|e| e.to_string())?;
    let mut brief = brief(spec);
    let workspace = match &contained {
        Some(container) => {
            let put = container
                .put("cases.json", cases.as_bytes())
                .and_then(|()| container.put("spec.json", spec_text.as_bytes()));
            if let Err(error) = put {
                return Ok((
                    None,
                    refused(
                        format!(
                            "The oracle writer did not run: the spec couldn't be put in its \
                             container ({error})."
                        ),
                        super::contain::ISOLATION,
                    ),
                ));
            }
            brief.guidance = format!("{}\n\n{CONTAINED}", brief.guidance);
            let remote: Arc<dyn microluna::Remote> = container.clone();
            workspace.in_remote(remote)
        }
        None => {
            let seal = microluna::Seal::create(&seal_dir, true).map_err(|e| e.to_string())?;
            workspace
                .isolated_by(bounds.isolation)
                .sealed_by(seal)
                .confining_reads(bounds.readable.clone())
        }
    };
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
    let report = microluna::run(transport, &workspace, &brief, &config, &mut recorder).await;
    recorder.close(match report.ending {
        Ending::Finished => atif::log::ENDED,
        _ => atif::log::INTERRUPTED,
    });
    let _ = std::fs::remove_dir_all(&seal_dir);
    let copied = contained
        .as_ref()
        .map(|container| container.copy_out(&dir.join("oracle.py")));
    if let Some(container) = &contained {
        container.remove();
    }
    let isolation = if contained.is_some() {
        super::contain::ISOLATION
    } else {
        "boundary"
    };
    let mut record = json!({
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
        "isolation": isolation,
        "bounds": {
            "turns": bounds.turns,
            "wall_sec": bounds.wall.as_secs(),
            "usd": bounds.usd,
            "effort": bounds.effort,
            "reads": if contained.is_some() { "container" } else { "confined" },
            "readable": if contained.is_some() { Vec::new() } else { bounds.readable.clone() },
        },
    });
    if let Some(container) = &contained {
        record["container"] = container.describe();
    }
    let program = match copied {
        Some(Ok(program)) => program,
        Some(Err(error)) => {
            record["copy_out"] = json!(error);
            None
        }
        None => std::fs::read_to_string(dir.join("oracle.py")).ok(),
    };
    let Some(program) = program else {
        return Ok((None, record));
    };
    let mut files = std::collections::BTreeMap::new();
    files.insert("oracle.py".to_string(), program);
    files.insert("cases.json".to_string(), cases);
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
