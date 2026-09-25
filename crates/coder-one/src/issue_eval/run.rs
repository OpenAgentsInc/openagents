//! One issue-flow run on an entry, recorded like a mini-task run.
//!
//! The run lays out a directory `gym coder minitasks --runs-dir` reads:
//!
//! ```text
//! <runs>/issue-eval-<entry>-<executor>-<ms>/
//!   manifest.json             kind "issue-eval": entry, part, outcome, grade, time, cost
//!   episode.atif.jsonl        every step the issue flow recorded
//!   repo/                     the scratch clone at the base commit, changes staged
//!   artifacts/                briefings, session streams, the reply, and the diff
//!   verification/grade.json   every check's result
//! ```
//!
//! The flow runs as [`crate::issue_turn::work`] runs it for a real issue,
//! loop, review, and pre-pull-request gate included, except that it
//! publishes nothing: no commit, push, or pull request. The grader runs
//! after the flow ends and is never shown to it.

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use atif::document::{Source, Step};
use serde_json::{Value, json};

use super::grade::{Grade, grade};
use super::{Entry, Part, Set, checkout};
use crate::delegate::{Agent, Credential};
use crate::issue_turn::{Fetched, Prepared, Reference};
use crate::record::Recorder;
use crate::terminal::{Progress, Request};

/// The schema a run's manifest carries: the mini-task run's, so the Gym's
/// mini-task view reads it.
pub const RUN_SCHEMA: &str = crate::minitask::RUN_SCHEMA;

/// How one run is set up.
pub struct Options {
    pub set: Set,
    pub id: String,
    /// The checkout the scratch clone fetches the base commit from.
    pub source: PathBuf,
    /// The directory runs are recorded under.
    pub out: PathBuf,
    /// Where the grader's Cargo builds go.
    pub target_dir: PathBuf,
    /// The Jev client for the flow's judgments; `None` runs without Jev.
    pub jev: Option<jev::Client>,
    /// Luna's model; `None` takes the flow's default.
    pub model: Option<String>,
    /// Scripted Microluna replies in place of the Codex login, for a test.
    pub script: Option<(String, Vec<microluna::Reply>)>,
    /// The Microluna manifest the flow runs under; `None` takes the issue
    /// flow's own, as `terminal::selected` picks it.
    pub policy: Option<crate::terminal::Selected>,
    /// Whether progress lines go to standard error.
    pub quiet: bool,
}

/// What a run left.
#[derive(Clone, Debug)]
pub struct Ran {
    pub dir: PathBuf,
    pub manifest: Value,
    pub grade: Grade,
}

fn millis(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Works entry `options.id` through the issue flow without publishing,
/// grades the checkout, and records the run under `options.out`.
///
/// # Errors
///
/// Returns a message when the run's directory or the scratch clone can't
/// be made. A flow that fails is a run with a failing grade, not an error.
pub async fn run(options: Options) -> Result<Ran, String> {
    let started = Instant::now();
    let at = atif::now_ms();
    let (entry, part, entry_digest) = options.set.find(&options.id)?.clone();
    let label = match &options.script {
        Some((name, _)) => format!("scripted-{name}"),
        None => format!(
            "microluna-{}",
            options.model.as_deref().unwrap_or("default")
        ),
    };
    let dir = options
        .out
        .join(format!("issue-eval-{}-{label}-{at}", entry.id));
    let repo = dir.join("repo");
    let artifacts = dir.join("artifacts");
    for sub in [&artifacts, &dir.join("verification")] {
        std::fs::create_dir_all(sub)
            .map_err(|error| format!("cannot create {}: {error}", sub.display()))?;
    }
    checkout(&options.source, &entry.base, None, &repo)?;

    let id = format!("issue-eval-{}-{at}", entry.id);
    let mut session = atif::Session::opening(
        &id,
        "none",
        "issue-eval",
        &repo.to_string_lossy(),
        &crate::episode::version(),
    );
    session.directive = "Work this issue.".to_string();
    let log_path = dir.join(crate::episode::INVOCATION_LOG);
    let log = atif::Log::create_at(&log_path, &session)
        .map_err(|error| format!("cannot create {}: {error}", log_path.display()))?;
    let recorder = Recorder::durable(log);
    recorder.push(Step::said(Source::User, &entry.request()));

    let inner = Request {
        workdir: repo.clone(),
        request: entry.request(),
        earlier: String::new(),
        resume: None,
        read_only: false,
        clarify: false,
        agent: Agent::Microluna,
        model: options.model.clone(),
        binary: None,
        credential: Credential::CodexAuthFile,
        jev: options.jev.clone(),
        artifacts: artifacts.clone(),
        script: options.script.as_ref().map(|(_, replies)| replies.clone()),
        issues: false,
        issue: true,
        review: false,
        policy: options
            .policy
            .as_ref()
            .map(|chosen| chosen.manifest.clone()),
    };
    let prepared = Prepared {
        inner,
        workdir: repo.clone(),
        source: None,
        branch: "issue-eval".to_string(),
        issue: Fetched {
            url: String::new(),
            title: entry.issue.title.clone(),
            body: entry.issue.body.clone(),
        },
    };
    let reference = Reference {
        repository: Some(entry.issue.repository.clone()),
        number: entry.issue.number,
    };
    let on: Rc<dyn Fn(Progress)> = if options.quiet {
        Rc::new(|_| {})
    } else {
        Rc::new(|progress| {
            if let Progress::Line(line) = progress {
                eprintln!("{line}");
            }
        })
    };
    // Standard output carries the result, so the flow's own progress
    // lines go where the turn's do.
    let quiet = options.quiet;
    let captured = crate::say::capture(Box::new(move |line| {
        if !quiet {
            eprintln!("{line}");
        }
    }));
    let answer =
        crate::issue_turn::work(prepared, reference, on, &Recorder::default(), false).await;
    drop(captured);
    let flow_ms = millis(started);
    for step in &answer.steps {
        recorder.push(step.clone());
    }
    let usage = crate::episode::usage(&answer.steps, true);
    let reply = answer.report.summary.result.clone().unwrap_or_default();
    let diff = super::run_in(&repo, "git", &["diff", "--cached", &entry.base]).unwrap_or_default();
    for (name, text) in [("reply.md", &reply), ("candidate.diff", &diff)] {
        crate::record::write_atomic(&artifacts.join(name), text.as_bytes())?;
    }

    let graded_at = Instant::now();
    let grade = grade(&options.set, &entry, &repo, &options.target_dir);
    let grading_ms = millis(graded_at);
    recorder.push(Step::said(
        Source::System,
        &format!("issue-eval grade: {} · {}", grade.verdict, grade.detail),
    ));
    recorder.finish(atif::log::ENDED);

    let outcome = match (&answer.report.status, answer.stuck) {
        (_, true) => "stuck",
        (crate::delegate::Status::Answered, false) => "finished",
        _ => "unfinished",
    };
    crate::record::write_atomic(
        &dir.join("verification/grade.json"),
        serde_json::to_string_pretty(&json!({
            "entry": entry.id,
            "grade": grade,
            "reward": grade.reward(),
        }))
        .map_err(|error| error.to_string())?
        .as_bytes(),
    )?;
    let policy = options
        .policy
        .clone()
        .unwrap_or_else(|| crate::terminal::selected(true));
    let manifest = manifest(&Manifested {
        id: &id,
        entry: &entry,
        part,
        entry_digest: &entry_digest,
        set_digest: &options.set.digest,
        label: &label,
        scripted: options.script.is_some(),
        policy: &policy,
        outcome,
        grade: &grade,
        usage: &usage,
        at,
        milliseconds: millis(started),
        flow_ms,
        grading_ms,
    });
    crate::record::write_atomic(
        &dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)
            .map_err(|error| error.to_string())?
            .as_bytes(),
    )?;
    Ok(Ran {
        dir,
        manifest,
        grade,
    })
}

struct Manifested<'a> {
    id: &'a str,
    entry: &'a Entry,
    part: Part,
    entry_digest: &'a str,
    set_digest: &'a str,
    label: &'a str,
    scripted: bool,
    policy: &'a crate::terminal::Selected,
    outcome: &'a str,
    grade: &'a Grade,
    usage: &'a Value,
    at: u64,
    milliseconds: u64,
    flow_ms: u64,
    grading_ms: u64,
}

/// A run's manifest, in the mini-task run's shape with the entry, the
/// part, and the cost added.
fn manifest(m: &Manifested) -> Value {
    let (fix_passed, fix_total) = m.grade.group("fix_tests");
    let (deliverables_passed, deliverables_total) = m.grade.group("deliverables");
    json!({
        "schema": RUN_SCHEMA,
        "kind": "issue-eval",
        "id": m.id,
        "task": {
            "id": m.entry.id,
            "family": format!("issue-eval {} · {}", m.part.word(), m.entry.category),
            "part": m.part.word(),
            "category": m.entry.category,
            "issue": m.entry.issue.number,
            "base": m.entry.base,
            "fix": m.entry.fix.head,
            "entry_digest": m.entry_digest,
            "set_digest": m.set_digest,
        },
        "executor": {
            "kind": if m.scripted { "scripted" } else { "microluna" },
            "label": m.label,
        },
        "policy": {
            "source": m.policy.source,
            "name": m.policy.name(),
            "digest": m.policy.digest,
        },
        "outcome": m.outcome,
        "grade": {
            "verdict": m.grade.verdict,
            "detail": m.grade.detail,
            "passed": m.grade.passed,
            "total": m.grade.total,
            "fix_tests": format!("{fix_passed}/{fix_total}"),
            "deliverables": format!("{deliverables_passed}/{deliverables_total}"),
            "checks": m.grade.checks,
            "changed": m.grade.changed,
        },
        "reward": m.grade.reward(),
        "cost": {
            "luna_usd": m.usage.pointer("/components/delegate/cost_usd"),
            "jev_usd": m.usage.pointer("/components/jev/cost_usd"),
            "total_usd": m.usage.pointer("/cost/amount_usd"),
            "lower_bound_usd": m.usage.pointer("/cost/lower_bound_usd"),
        },
        "usage": m.usage,
        "started_at": atif::document::iso(m.at),
        "milliseconds": m.milliseconds,
        "flow_milliseconds": m.flow_ms,
        "grading_milliseconds": m.grading_ms,
        "version": crate::episode::version(),
        "files": {
            "invocation_log": crate::episode::INVOCATION_LOG,
            "grade": "verification/grade.json",
            "diff": "artifacts/candidate.diff",
            "reply": "artifacts/reply.md",
            "workdir": "repo",
            "artifacts": "artifacts",
        },
    })
}

/// Where issue-flow evaluation runs are recorded:
/// `~/.openagents/coder-one/issue-evals`.
#[must_use]
pub fn default_runs_dir() -> Option<PathBuf> {
    crate::credentials::openagents_dir().map(|dir| dir.join("coder-one").join("issue-evals"))
}

/// Where the grader's Cargo builds go by default:
/// `~/.openagents/coder-one/issue-evals/target`, or `CARGO_TARGET_DIR`.
#[must_use]
pub fn default_target_dir() -> Option<PathBuf> {
    std::env::var_os("CARGO_TARGET_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| default_runs_dir().map(|dir| dir.join("target")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue_eval::{default_set_dir, load, run_in, source_of};
    use microluna::fake::call;
    use std::path::Path;

    fn usage() -> microluna::TokenUsage {
        microluna::TokenUsage {
            input: 1_000,
            cached: 0,
            output: 20,
            reasoning: 0,
        }
    }

    /// Scripted Luna writes the real fix for a docs entry; the flow runs
    /// its loop, review, and gate, publishes nothing, and the grader
    /// passes the staged change. The run reads as a mini-task run.
    #[test]
    fn a_scripted_run_works_an_entry_without_publishing_and_grades_it() {
        let Ok(source) = source_of(Path::new(env!("CARGO_MANIFEST_DIR"))) else {
            eprintln!("skipped: not in a git checkout");
            return;
        };
        let set = load(&default_set_dir()).unwrap();
        let (entry, _, _) = set.find("9450").unwrap().clone();
        // A shallow or partial clone of this repository may not hold the
        // entry's commits; the case is about the flow, not the clone.
        let Ok(fixed) = run_in(
            &source,
            "git",
            &[
                "show",
                &format!("{}:docs/coder/delegate.md", entry.fix.head),
            ],
        ) else {
            eprintln!("skipped: this clone doesn't hold {}", entry.fix.head);
            return;
        };
        let probe = tempfile::tempdir().unwrap();
        let (work, artifacts) = (probe.path().join("work"), probe.path().join("artifacts"));
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(&artifacts).unwrap();
        if let Err(why) = crate::terminal::boundary(true, &work, &artifacts) {
            eprintln!("skipped: {why}");
            return;
        }
        let replies = vec![
            call(
                "c1",
                "write_file",
                &json!({ "path": "docs/coder/delegate.md", "contents": fixed }),
                usage(),
            ),
            call(
                "c2",
                "finish",
                &json!({ "status": "done", "summary": "Rewrote the stale bullet.", "answer": "Done." }),
                usage(),
            ),
        ];
        let out = tempfile::tempdir().unwrap();
        let options = Options {
            set,
            id: "9450".to_string(),
            source: source.clone(),
            out: out.path().to_path_buf(),
            target_dir: out.path().join("target"),
            jev: None,
            model: None,
            script: Some(("fix".to_string(), replies)),
            policy: None,
            quiet: true,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let ran = runtime.block_on(run(options)).unwrap();
        assert_eq!(ran.grade.verdict, "passed", "{:#?}", ran.grade);
        assert_eq!(ran.manifest["schema"], RUN_SCHEMA);
        assert_eq!(ran.manifest["kind"], "issue-eval");
        assert_eq!(ran.manifest["task"]["part"], "development");
        assert!(ran.manifest["cost"]["jev_usd"].is_number());
        // Nothing was committed, pushed, or opened: the checkout still
        // sits on the base commit with the change staged.
        let repo = ran.dir.join("repo");
        let head = run_in(&repo, "git", &["rev-parse", "HEAD"]).unwrap();
        assert_eq!(head.trim(), entry.base);
        let staged = run_in(&repo, "git", &["diff", "--cached", "--name-only"]).unwrap();
        assert_eq!(staged.trim(), "docs/coder/delegate.md");
        assert!(run_in(&repo, "git", &["remote"]).unwrap().trim().is_empty());
        // The scratch clone holds no history after the base.
        let log = run_in(&repo, "git", &["log", "--all", "--format=%H"]).unwrap();
        assert_eq!(log.trim(), entry.base);
        // The Gym's mini-task reader reads the run.
        let text = std::fs::read_to_string(ran.dir.join("manifest.json")).unwrap();
        let manifest: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(manifest["grade"]["verdict"], "passed");
        assert!(ran.dir.join(crate::episode::INVOCATION_LOG).is_file());
        assert!(ran.dir.join("artifacts/candidate.diff").is_file());
    }
}
