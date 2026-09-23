//! What the host reads before any model runs: a fixed probe battery over
//! the Gym, then Jev's judgments of which runs and which transcript steps
//! bear on the question.
//!
//! The battery is code's choice, not the executor's. For a question about
//! runs it is the learning order's top 40, the reason groups, the task
//! groups, and the outcome matrix, plus the run the operator selected.
//! Jev then reads the question alone and says which reasons it asks
//! about, so the host can fetch those runs too; reads the candidate runs
//! and says which bear on the question; and reads the top runs'
//! transcript steps and says which of those do. One Noul per item, the
//! same pattern the Jev probe uses for files.
//!
//! When a person has marked runs, the battery reads the marks too, and the
//! same Jev request that reads the reasons says whether the question asks
//! about marked runs. When it does, the marked runs come first among the
//! candidates and the briefing opens them.
//!
//! For a question about the repository, the battery is a search of the
//! Markdown files for the question's words, and Jev judges which files
//! bear on it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use atif::document::{Call, Outcome as CallOutcome, Step};
use jev::{Noul, NoulCriteria, Questions};
use serde_json::{Value, json};

use crate::component::jev::{Ask, Asked, JevMode, ask};
use crate::record::{Cost, Finish, Implementation, Outcome, Recorder, Start};

use super::Progress;

/// How long one host probe may run.
pub const PROBE_DEADLINE: Duration = Duration::from_secs(120);

/// The most runs Jev judges for relevance.
pub const MAX_CANDIDATES: usize = 60;

/// The most runs the briefing opens in full.
pub const MAX_OPENED: usize = 6;

/// The fewest runs the briefing opens when any candidate exists, so a
/// question Jev finds nothing for still gets evidence.
pub const MIN_OPENED: usize = 3;

/// The most transcript steps Jev judges across the opened runs.
pub const MAX_STEPS: usize = 72;

/// The most steps per run the briefing quotes.
pub const STEPS_PER_RUN: usize = 6;

/// The most reasons the question can pull runs in by.
pub const MAX_REASONS: usize = 3;

/// A yes at or above this probability selects an item.
pub const YES: f64 = 0.5;

/// The most files a repository question judges, and opens.
pub const MAX_FILES: usize = 30;
pub const OPENED_FILES: usize = 5;

/// Runs a program the host chose, bounded and inside the ask's boundary.
pub struct Reader<'a> {
    pub gym: Option<PathBuf>,
    pub cwd: PathBuf,
    pub boundary: Option<&'a coder_boundary::Boundary>,
}

/// One probe's result.
#[derive(Clone, Debug)]
pub struct Probe {
    /// The command in words, `gym runs --json`.
    pub command: String,
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub milliseconds: u64,
    pub bytes: u64,
}

impl Probe {
    /// The output read as JSON, when it is JSON.
    #[must_use]
    pub fn json(&self) -> Option<Value> {
        if !self.ok {
            return None;
        }
        serde_json::from_str(&self.stdout).ok()
    }
}

impl Reader<'_> {
    /// Runs `gym ARGS`.
    pub async fn gym(&self, args: Vec<String>) -> Probe {
        let Some(gym) = &self.gym else {
            return Probe {
                command: format!("gym {}", args.join(" ")),
                ok: false,
                stdout: String::new(),
                stderr: "no gym binary".to_string(),
                milliseconds: 0,
                bytes: 0,
            };
        };
        self.run(gym, "gym", args).await
    }

    /// Runs `program ARGS`, where `label` names the program in records.
    pub async fn run(&self, program: &Path, label: &str, args: Vec<String>) -> Probe {
        let command_text = format!("{label} {}", args.join(" "));
        let command = match self.boundary {
            Some(boundary) => match boundary.command(program, &args) {
                Ok(command) => command,
                Err(error) => {
                    return Probe {
                        command: command_text,
                        ok: false,
                        stdout: String::new(),
                        stderr: format!("cannot bound the probe: {error}"),
                        milliseconds: 0,
                        bytes: 0,
                    };
                }
            },
            None => {
                let mut command = std::process::Command::new(program);
                command.args(&args);
                command
            }
        };
        let mut command = command;
        command.current_dir(&self.cwd);
        let ended = supervise::Job::from_command(command)
            .bounded(supervise::Limits::within(PROBE_DEADLINE).keeping(8 * 1024 * 1024))
            .run()
            .await;
        Probe {
            command: command_text,
            ok: ended.ending.success() && !ended.stdout.truncated,
            stdout: ended.stdout.text,
            stderr: super::clip(&ended.stderr.text, 2_000),
            milliseconds: u64::try_from(ended.elapsed.as_millis()).unwrap_or(u64::MAX),
            bytes: ended.stdout.bytes + ended.stderr.bytes,
        }
    }
}

/// Records `probes` as calls under one `ask.probes` invocation.
pub fn record_probes(recorder: &Recorder, name: &str, probes: &[&Probe], progress: &Progress) {
    let invocation = recorder.enter(
        Start::new(
            "ask.probes",
            Implementation::new(
                "ask.probes",
                name,
                &json!({ "commands": probes.iter().map(|p| &p.command).collect::<Vec<_>>() }),
            ),
        )
        .named(name)
        .effect("observe"),
    );
    for (index, probe) in probes.iter().enumerate() {
        let mut extra = serde_json::Map::new();
        extra.insert("bytes".to_string(), json!(probe.bytes));
        extra.insert(
            "sha256".to_string(),
            json!(atif::digest(&json!(probe.stdout))),
        );
        recorder.push(
            Step::called(Call {
                id: format!("{invocation}-{index}"),
                name: "probe".to_string(),
                arguments: json!({ "command": probe.command }),
                output: if probe.ok {
                    super::clip(&probe.stdout, 1_500)
                } else {
                    format!("failed: {}", probe.stderr)
                },
                outcome: if probe.ok {
                    CallOutcome::Completed
                } else {
                    CallOutcome::Failed
                },
                milliseconds: probe.milliseconds,
                purpose: None,
                extra,
            })
            .taking(probe.milliseconds),
        );
        progress.line(&format!(
            "probe ▸ {} ({:.1}s, {}){}",
            probe.command,
            probe.milliseconds as f64 / 1000.0,
            size(probe.bytes),
            if probe.ok {
                String::new()
            } else {
                format!(" failed: {}", super::clip(probe.stderr.trim(), 200))
            }
        ));
    }
    let failed = probes.iter().filter(|p| !p.ok).count();
    recorder.end(
        &invocation,
        Finish::new(if failed == 0 {
            Outcome::Completed
        } else {
            Outcome::Failed
        })
        .summary(json!({ "probes": probes.len(), "failed": failed }))
        .cost(Cost::none()),
    );
}

fn size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{bytes} B")
    }
}

/// One run as the candidates list carries it: compact enough for forty
/// runs in one Jev state.
#[must_use]
pub fn run_summary(run: &Value) -> Value {
    let label = match (run["variant"].as_str(), run["model"].as_str()) {
        (Some(variant), _) => format!("{} · {variant}", run["agent"].as_str().unwrap_or("?")),
        (None, Some(model)) => format!("{} · {model}", run["agent"].as_str().unwrap_or("?")),
        _ => run["agent"].as_str().unwrap_or("?").to_string(),
    };
    let reasons: Vec<String> = run["learning"]["reasons"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| {
            format!(
                "{} {:.2}",
                r["id"].as_str().unwrap_or("?"),
                r["probability"].as_f64().unwrap_or(0.0)
            )
        })
        .collect();
    json!({
        "run": format!("{}/{}", run["job"].as_str().unwrap_or("?"), run["trial"].as_str().unwrap_or("?")),
        "task": run["task"],
        "agent": label,
        "outcome": run["outcome"],
        "tests": run["tests"].as_object().map(|t| format!("{}/{}", t["passed"], t["total"])),
        "cost_usd": run["cost_usd"].as_f64().map(|c| (c * 1000.0).round() / 1000.0),
        "learning": run["learning"]["learning"],
        "reasons": reasons,
        "marks": run["marks"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|mark| {
                let mut text = match mark["step"].as_u64() {
                    Some(step) => format!("step {step} {}", mark["verdict"].as_str().unwrap_or("?")),
                    None => mark["verdict"].as_str().unwrap_or("?").to_string(),
                };
                let tags: Vec<&str> = mark["tags"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                if !tags.is_empty() {
                    text.push_str(&format!(": {}", tags.join(", ")));
                }
                text
            })
            .collect::<Vec<_>>(),
    })
}

/// The Noul that asks whether a question is about runs a person marked.
#[must_use]
pub fn marks_noul() -> Noul {
    Noul::with_criteria(
        "Does the question in `question` ask about runs a person marked or judged: runs a person flagged as bad, cleared as fine, tagged with a judgment, or wrote a note on, as `marks` summarizes them?",
        NoulCriteria::new()
            .when_true("The question asks about marked runs, about which runs a person called bad or fine, or about what the marks or their notes say.")
            .when_false("The question is about runs in general, Jev's judgments, a task, or an agent, and would be answered the same way without anyone's marks."),
    )
}

/// Whether the question's words name marks, for when Jev can't say.
#[must_use]
pub fn names_marks(question: &str) -> bool {
    let lower = question.to_lowercase();
    ["mark", "flagged", "bad run", "cleared"]
        .iter()
        .any(|word| lower.contains(word))
}

/// The marks `gym runs marks --json` lists: bad first, then cleared, each
/// as its run.
#[must_use]
pub fn marked_runs(marks: &Value) -> Vec<String> {
    let mut runs: Vec<(bool, String)> = marks["marks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|mark| {
            Some((
                mark["verdict"].as_str() != Some("bad"),
                mark["run"].as_str()?.to_string(),
            ))
        })
        .collect();
    runs.sort();
    let mut seen = std::collections::HashSet::new();
    runs.into_iter()
        .filter(|(_, run)| seen.insert(run.clone()))
        .map(|(_, run)| run)
        .collect()
}

/// The reason groups a `gym runs group --by reason --json` probe found:
/// ID, tag, and count.
#[must_use]
pub fn reason_groups(groups: &Value) -> Vec<(String, String, u64)> {
    groups["groups"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| {
            Some((
                g["key"].as_str()?.to_string(),
                g["tag"].as_str().unwrap_or_default().to_string(),
                g["count"].as_u64().unwrap_or(0),
            ))
        })
        .collect()
}

/// The tasks a question names by their slug, longest first, from the
/// task groups.
#[must_use]
pub fn named_tasks(question: &str, tasks: &Value) -> Vec<String> {
    let lower = question.to_lowercase();
    let mut named: Vec<String> = tasks["groups"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| g["key"].as_str())
        .filter(|task| task.len() > 3 && lower.contains(&task.to_lowercase()))
        .map(str::to_string)
        .collect();
    named.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
    // A task whose name is part of a longer named task isn't named itself.
    let kept: Vec<String> = named
        .iter()
        .filter(|task| {
            !named
                .iter()
                .any(|other| other != *task && other.contains(task.as_str()))
        })
        .cloned()
        .collect();
    kept.into_iter().take(3).collect()
}

/// Jev's question set for which reasons a question asks about.
#[must_use]
pub fn reason_questions(count: usize) -> Questions {
    let mut questions = Questions::new();
    for index in 0..count {
        questions = questions.with(
            format!("reason_{index}"),
            Noul::with_criteria(
                format!(
                    "Does the question in `question` ask about runs that show the reason described in `reasons[{index}]`, whether it names the reason's tag or ID or describes the same thing in other words?"
                ),
                NoulCriteria::new()
                    .when_true("The question asks about runs with this reason: it names the tag or the ID, or describes the same behavior, such as a run claiming success it didn't earn for `unearned_success`.")
                    .when_false("The question is about something else, or would be answered the same way whether or not runs gave this reason."),
            ),
        );
    }
    questions
}

/// Jev's question set for which runs bear on a question.
#[must_use]
pub fn run_questions(count: usize) -> Questions {
    let mut questions = Questions::new();
    for index in 0..count {
        questions = questions.with(
            format!("run_{index}"),
            Noul::with_criteria(
                format!(
                    "Would a person answering the question in `question` need to read the run in `runs[{index}]`?"
                ),
                NoulCriteria::new()
                    .when_true("The run is one the question asks about, by its task, its agent, its outcome, or a reason Jev gave it, or its record is evidence for or against the answer.")
                    .when_false("The run doesn't bear on the question: another task, another agent, or a reason the question doesn't ask about."),
            ),
        );
    }
    questions
}

/// Jev's question set for which transcript steps bear on a question.
#[must_use]
pub fn step_questions(count: usize) -> Questions {
    let mut questions = Questions::new();
    for index in 0..count {
        questions = questions.with(
            format!("step_{index}"),
            Noul::with_criteria(
                format!(
                    "Does the transcript step in `steps[{index}]` show evidence that bears on the question in `question` for the run it belongs to?"
                ),
                NoulCriteria::new()
                    .when_true("The step shows what the question asks about for its run: a failure, a claim the agent made, a check, a decision, or a result that supports or contradicts an answer.")
                    .when_false("The step is routine work, such as reading a file or listing a directory, that says nothing about the question."),
            ),
        );
    }
    questions
}

/// Jev's question set for which repository files bear on a question.
#[must_use]
pub fn file_questions(count: usize) -> Questions {
    let mut questions = Questions::new();
    for index in 0..count {
        questions = questions.with(
            format!("file_{index}"),
            Noul::with_criteria(
                format!(
                    "Would reading the file in `files[{index}]` help answer the question in `question`?"
                ),
                NoulCriteria::new()
                    .when_true("The file's matching lines show it documents or decides what the question asks about.")
                    .when_false("The file only mentions the question's words in passing."),
            ),
        );
    }
    questions
}

/// Asks Jev one set, as a `ask.relevance` invocation.
pub async fn judge(
    mode: &JevMode,
    recorder: &Recorder,
    name: &str,
    state: Value,
    questions: Questions,
) -> Asked {
    ask(
        mode,
        recorder,
        Ask {
            component: "ask.relevance",
            name,
            id: format!("{name}-{}", atif::now_ms()),
            state,
            questions,
            parent: None,
            deadline: None,
        },
    )
    .await
}

/// The indexes Jev said yes to, strongest first, at most `most`; when Jev
/// has no answer, the first `fallback` in the order given.
#[must_use]
pub fn chosen(
    asked: &Asked,
    prefix: &str,
    count: usize,
    most: usize,
    fallback: usize,
) -> Vec<(usize, Option<f64>)> {
    if !asked.answered() {
        return (0..count.min(fallback)).map(|i| (i, None)).collect();
    }
    let mut scored: Vec<(usize, f64)> = (0..count)
        .filter_map(|i| Some((i, asked.noul(&format!("{prefix}_{i}"))?)))
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    scored
        .into_iter()
        .filter(|(_, p)| *p >= YES)
        .take(most)
        .map(|(i, p)| (i, Some(p)))
        .collect()
}

/// Every probability Jev gave, by index.
#[must_use]
pub fn probabilities(asked: &Asked, prefix: &str, count: usize) -> BTreeMap<usize, f64> {
    (0..count)
        .filter_map(|i| Some((i, asked.noul(&format!("{prefix}_{i}"))?)))
        .collect()
}

/// A transcript step as Jev and the briefing read it.
#[derive(Clone, Debug)]
pub struct StepText {
    pub run: String,
    pub step: u64,
    pub headline: String,
    pub body: String,
}

/// The steps of one run's `show --json` Jev may judge: every step when
/// there are few, else the first two and the last `keep - 2`, where the
/// work ends and the agent reports.
#[must_use]
pub fn steps_of(run: &str, shown: &Value, keep: usize) -> Vec<StepText> {
    let blocks: Vec<&Value> = shown["transcript"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    let pick: Vec<usize> = if blocks.len() <= keep {
        (0..blocks.len()).collect()
    } else {
        (0..2)
            .chain(blocks.len() - (keep - 2)..blocks.len())
            .collect()
    };
    pick.into_iter()
        .map(|index| {
            let block = blocks[index];
            StepText {
                run: run.to_string(),
                step: block["step"].as_u64().unwrap_or(index as u64 + 1),
                headline: block["headline"].as_str().unwrap_or_default().to_string(),
                body: block["body"].as_str().unwrap_or_default().to_string(),
            }
        })
        .collect()
}

/// Words from the question worth searching the repository for.
#[must_use]
pub fn keywords(question: &str) -> Vec<String> {
    const STOP: [&str; 40] = [
        "about", "after", "again", "also", "because", "been", "before", "being", "does", "doing",
        "each", "every", "find", "from", "have", "into", "just", "like", "made", "make", "many",
        "more", "most", "much", "only", "other", "same", "should", "some", "such", "tell", "than",
        "that", "their", "them", "then", "there", "these", "they", "this",
    ];
    const STOP2: [&str; 16] = [
        "those", "through", "what", "when", "where", "which", "while", "with", "would", "your",
        "runs", "why", "how", "were", "will", "share",
    ];
    let mut words: Vec<String> = question
        .split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
        .map(|w| w.trim_matches('-').to_lowercase())
        .filter(|w| w.len() >= 4 && !STOP.contains(&w.as_str()) && !STOP2.contains(&w.as_str()))
        .collect();
    words.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
    words.dedup();
    words.into_iter().take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_names_tasks_by_their_slug_and_the_longest_wins() {
        let tasks = json!({"groups": [
            {"key": "log-summary"},
            {"key": "log-summary-date-ranges"},
            {"key": "fix-git"},
            {"key": "cad"},
        ]});
        assert_eq!(
            named_tasks("Why did Luna fail log-summary-date-ranges?", &tasks),
            vec!["log-summary-date-ranges"]
        );
        assert!(named_tasks("Which runs failed on output paths?", &tasks).is_empty());
    }

    #[test]
    fn the_steps_jev_reads_keep_the_start_and_the_end() {
        let shown = json!({"transcript": (1..=20).map(|i| json!({"step": i, "headline": format!("h{i}"), "body": "b"})).collect::<Vec<_>>()});
        let steps = steps_of("j/t", &shown, 6);
        let numbers: Vec<u64> = steps.iter().map(|s| s.step).collect();
        assert_eq!(numbers, vec![1, 2, 17, 18, 19, 20]);
        assert_eq!(steps_of("j/t", &json!({"transcript": []}), 6).len(), 0);
    }

    #[test]
    fn a_run_summary_is_compact_and_keeps_the_reasons() {
        let run = json!({
            "job": "tb4--x", "trial": "t__1", "task": "t", "agent": "Coder One",
            "variant": "tunable-v6", "outcome": "failed",
            "tests": {"passed": 3, "failed": 1, "total": 4}, "cost_usd": 0.12345,
            "learning": {"learning": 0.7, "reasons": [{"id": "near_miss", "probability": 0.91}]},
        });
        let summary = run_summary(&run);
        assert_eq!(summary["run"], "tb4--x/t__1");
        assert_eq!(summary["agent"], "Coder One · tunable-v6");
        assert_eq!(summary["tests"], "3/4");
        assert_eq!(summary["reasons"][0], "near_miss 0.91");
        assert_eq!(summary["cost_usd"], 0.123);
    }

    #[test]
    fn keywords_drop_common_words() {
        let words = keywords("Which failures come down to output paths?");
        assert!(words.contains(&"failures".to_string()), "{words:?}");
        assert!(words.contains(&"output".to_string()), "{words:?}");
        assert!(!words.contains(&"which".to_string()), "{words:?}");
    }
}
