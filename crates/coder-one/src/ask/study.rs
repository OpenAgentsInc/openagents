//! `coder-one ask study`: the ask measured on questions whose answers are
//! already written down.
//!
//! A question set (`openagents.coder-one.ask-questions.v1`) holds each
//! question with the tasks, and where they're known the runs, that the
//! written answer names. The study asks every question with one executor,
//! then scores each answer by code:
//!
//! - **citation validity**: the share of the answer's citations the check
//!   held;
//! - **task recall**: the share of the expected tasks the answer cites a
//!   run of;
//! - **run recall**: the share of the expected runs the answer cites;
//! - **cost and time**: what the ask reported, Jev included.
//!
//! The result (`openagents.coder-one.ask-study.v1`) carries a digest of the
//! implementation, so two studies of different probes, questions, or
//! executors compare row by row. `gym coder asks --studies` reads it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::executor::Which;
use super::{JevChoice, Options, Progress};

/// The question set's schema.
pub const SET_SCHEMA: &str = "openagents.coder-one.ask-questions.v1";

/// A study result's schema.
pub const RESULT_SCHEMA: &str = "openagents.coder-one.ask-study.v1";

/// The usage.
pub const USAGE: &str = "\
usage: coder-one ask study [--set FILE] [--executor luna|opus] [--model MODEL]
                           [--only ID]... [--budget USD] [--out DIR] [--retain DIR]
                           [--gym PATH] [--no-jev] [--json]

Asks every question in the set and scores each answer by code: citation
validity, recall of the expected tasks and runs, cost, and time. --set
defaults to bench/terminal-bench/asks/questions-v1.json. The result goes to
~/.openagents/coder-one/ask-studies/<id>/result.json unless --out names
another directory, and --retain DIR also writes it to DIR/<id>/result.json,
such as bench/terminal-bench/asks/studies. --budget caps each question.";

/// One question and the answer written down for it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Question {
    pub id: String,
    pub question: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub answer: String,
    #[serde(default)]
    pub expected_tasks: Vec<String>,
    #[serde(default)]
    pub expected_runs: Vec<String>,
    #[serde(default)]
    pub note: String,
}

/// A question set.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Set {
    pub schema: String,
    pub set: String,
    #[serde(default)]
    pub summary: String,
    pub questions: Vec<Question>,
}

impl Set {
    /// Reads a question set.
    ///
    /// # Errors
    ///
    /// Returns a message when the file doesn't read, has another schema, or
    /// repeats an id.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let set: Set = serde_json::from_str(&text)
            .map_err(|error| format!("{} is not a question set: {error}", path.display()))?;
        if set.schema != SET_SCHEMA {
            return Err(format!("{} is not a {SET_SCHEMA} file", path.display()));
        }
        let ids: BTreeSet<&str> = set.questions.iter().map(|q| q.id.as_str()).collect();
        if ids.len() != set.questions.len() {
            return Err(format!("{} repeats a question id", path.display()));
        }
        Ok(set)
    }

    /// The set's digest, over its questions exactly as written.
    #[must_use]
    pub fn digest(&self) -> String {
        atif::digest(&json!({ "set": self.set, "questions": self.questions }))
    }
}

/// The share of `expected` found in `cited`, with what was found and
/// missed; `None` when nothing was expected.
fn recall(
    expected: &[String],
    cited: &BTreeSet<String>,
) -> (Option<f64>, Vec<String>, Vec<String>) {
    if expected.is_empty() {
        return (None, Vec::new(), Vec::new());
    }
    let (found, missed): (Vec<String>, Vec<String>) = expected
        .iter()
        .cloned()
        .partition(|item| cited.contains(item));
    (
        Some(found.len() as f64 / expected.len() as f64),
        found,
        missed,
    )
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// Scores one ask's record against its question.
#[must_use]
pub fn score(question: &Question, record: &Value) -> Value {
    let cited = record["cited_runs"].as_array().cloned().unwrap_or_default();
    let runs: BTreeSet<String> = cited
        .iter()
        .filter_map(|c| c["run"].as_str().map(str::to_string))
        .collect();
    let tasks: BTreeSet<String> = cited
        .iter()
        .filter_map(|c| c["task"].as_str().map(str::to_string))
        .filter(|task| !task.is_empty())
        .collect();
    let (task_recall, found_tasks, missed_tasks) = recall(&question.expected_tasks, &tasks);
    let (run_recall, found_runs, missed_runs) = recall(&question.expected_runs, &runs);
    let citations = &record["citations"];
    json!({
        "id": question.id,
        "question": question.question,
        "ask": record["id"],
        "status": record["status"],
        "answered": record["answer"].is_string(),
        "citations": citations["citations"],
        "valid_citations": citations["valid_citations"],
        "validity": citations["validity"],
        "claims": citations["claims"],
        "verified_claims": citations["verified"],
        "task_recall": task_recall.map(round3),
        "tasks_found": found_tasks,
        "tasks_missed": missed_tasks,
        "run_recall": run_recall.map(round3),
        "runs_found": found_runs,
        "runs_missed": missed_runs,
        "cited_runs": runs.len(),
        "cost_usd": record["cost"]["usd"],
        "jev_usd": record["cost"]["jev_usd"],
        "executor_usd": record["cost"]["executor_usd"],
        "milliseconds": record["milliseconds"],
    })
}

/// The means and sums over scored questions.
#[must_use]
pub fn totals(rows: &[Value]) -> Value {
    let mean = |key: &str| {
        let values: Vec<f64> = rows.iter().filter_map(|row| row[key].as_f64()).collect();
        (!values.is_empty()).then(|| round3(values.iter().sum::<f64>() / values.len() as f64))
    };
    let sum = |key: &str| {
        let values: Vec<f64> = rows.iter().filter_map(|row| row[key].as_f64()).collect();
        (!values.is_empty()).then(|| values.iter().sum::<f64>())
    };
    let citations: f64 = rows.iter().filter_map(|r| r["citations"].as_f64()).sum();
    let valid: f64 = rows
        .iter()
        .filter_map(|r| r["valid_citations"].as_f64())
        .sum();
    let times: Vec<u64> = rows
        .iter()
        .filter_map(|r| r["milliseconds"].as_u64())
        .collect();
    json!({
        "questions": rows.len(),
        "answered": rows.iter().filter(|r| r["answered"] == true).count(),
        "citations": citations,
        "valid_citations": valid,
        "citation_validity": (citations > 0.0).then(|| round3(valid / citations)),
        "mean_validity": mean("validity"),
        "mean_task_recall": mean("task_recall"),
        "mean_run_recall": mean("run_recall"),
        "cost_usd": sum("cost_usd").map(|usd| (usd * 10_000.0).round() / 10_000.0),
        "jev_usd": sum("jev_usd").map(|usd| (usd * 10_000.0).round() / 10_000.0),
        "mean_milliseconds": mean("milliseconds").map(f64::round),
        "max_milliseconds": times.iter().max(),
        "under_a_minute": times.iter().filter(|ms| **ms < 60_000).count(),
    })
}

/// Where studies are kept: `~/.openagents/coder-one/ask-studies`.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    crate::credentials::openagents_dir().map(|dir| dir.join("coder-one/ask-studies"))
}

/// The checkout's question set.
#[must_use]
pub fn default_set() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/terminal-bench/asks/questions-v1.json")
}

/// The study's lines as text: one per question, then the totals.
#[must_use]
pub fn text(result: &Value) -> String {
    let mut out = format!(
        "Ask study {} · {} · {} {}\n\n",
        result["study"].as_str().unwrap_or("?"),
        result["set"].as_str().unwrap_or("?"),
        result["executor"].as_str().unwrap_or("?"),
        result["model"].as_str().unwrap_or("")
    );
    out.push_str(&format!(
        "{:<32} {:>9} {:>8} {:>7} {:>8} {:>7}\n",
        "question", "citations", "tasks", "runs", "cost", "time"
    ));
    let fraction = |row: &Value, key: &str| {
        row[key]
            .as_f64()
            .map_or_else(|| "—".to_string(), |v| format!("{v:.2}"))
    };
    for row in result["rows"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "{:<32} {:>9} {:>8} {:>7} {:>8} {:>6.1}s{}\n",
            super::clip(row["id"].as_str().unwrap_or("?"), 32),
            format!("{}/{}", row["valid_citations"], row["citations"]),
            fraction(row, "task_recall"),
            fraction(row, "run_recall"),
            row["cost_usd"]
                .as_f64()
                .map_or_else(|| "—".to_string(), |usd| format!("${usd:.4}")),
            row["milliseconds"].as_f64().unwrap_or(0.0) / 1000.0,
            if row["answered"] == true {
                ""
            } else {
                "  no answer"
            }
        ));
    }
    let t = &result["totals"];
    out.push_str(&format!(
        "\n{} of {} answered · citations {}/{} valid ({}) · task recall {} · run recall {} · ${} · mean {:.1}s, {} under a minute\n",
        t["answered"],
        t["questions"],
        t["valid_citations"],
        t["citations"],
        t["citation_validity"],
        t["mean_task_recall"],
        t["mean_run_recall"],
        t["cost_usd"],
        t["mean_milliseconds"].as_f64().unwrap_or(0.0) / 1000.0,
        t["under_a_minute"],
    ));
    if let Some(dir) = result["dir"].as_str() {
        out.push_str(&format!("Recorded: {dir}\n"));
    }
    out
}

/// `coder-one ask study …`.
///
/// # Errors
///
/// Returns a message when the arguments or the set don't read, or the
/// result can't be written.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let mut set_path = default_set();
    let mut which = Which::Luna;
    let mut model: Option<String> = None;
    let mut only: Vec<String> = Vec::new();
    let mut budget = super::DEFAULT_BUDGET_USD;
    let mut out = default_dir();
    let mut retain: Option<PathBuf> = None;
    let mut gym: Option<PathBuf> = None;
    let mut jev = JevChoice::Live;
    let mut json_out = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value\n\n{USAGE}"))
        };
        match arg.as_str() {
            "--set" => set_path = PathBuf::from(value("--set")?),
            "--executor" => which = Which::parse(&value("--executor")?)?,
            "--model" => model = Some(value("--model")?),
            "--only" => only.push(value("--only")?),
            "--budget" => {
                budget = value("--budget")?
                    .trim_start_matches('$')
                    .parse()
                    .map_err(|_| "--budget takes dollars".to_string())?;
            }
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--retain" => retain = Some(PathBuf::from(value("--retain")?)),
            "--gym" => gym = Some(PathBuf::from(value("--gym")?)),
            "--no-jev" => jev = JevChoice::Off,
            "--json" => json_out = true,
            "--help" | "-h" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    let set = Set::load(&set_path)?;
    let questions: Vec<&Question> = set
        .questions
        .iter()
        .filter(|q| only.is_empty() || only.contains(&q.id))
        .collect();
    if questions.is_empty() {
        return Err("no question in the set matches --only".to_string());
    }
    let started_at = atif::now_ms();
    let study = format!("ask-study-{started_at}");
    let model = model.unwrap_or_else(|| which.agent().default_model().to_string());
    let implementation = json!({
        "battery": "gym-v1",
        "relevance": "ask-relevance-v1",
        "briefing": { "version": "ask briefing v1", "cap": super::brief::CAP },
        "allowlist": "ask-allowlist-v1",
        "executor": which.word(),
        "model": model,
        "jev": match jev { JevChoice::Off => "off", _ => "live" },
        "budget_usd": budget,
    });
    let progress = Progress {
        events: false,
        quiet: true,
    };
    let mut rows = Vec::new();
    for (n, question) in questions.iter().enumerate() {
        eprintln!(
            "  study ▸ {}/{} {}: {}",
            n + 1,
            questions.len(),
            question.id,
            super::clip(&question.question, 90)
        );
        let options = Options {
            question: question.question.clone(),
            scope: super::Scope::Gym,
            executor: which,
            model: Some(model.clone()),
            budget_usd: budget,
            deadline: super::DEFAULT_DEADLINE,
            json: true,
            events: false,
            rank: false,
            run: None,
            context: Vec::new(),
            gym: gym.clone(),
            repo: None,
            out: None,
            jev: jev.clone(),
            jev_record: None,
            answer_file: None,
            claims: Vec::new(),
            proposals: None,
            record_proposals: false,
        };
        let row = match super::run(options, &progress).await {
            Ok((record, _)) => score(question, &record),
            Err(why) => json!({
                "id": question.id,
                "question": question.question,
                "answered": false,
                "error": why,
            }),
        };
        eprintln!(
            "  study ▸ {}: citations {}/{}, task recall {}, run recall {}, {}, {:.1}s",
            question.id,
            row["valid_citations"],
            row["citations"],
            row["task_recall"],
            row["run_recall"],
            row["cost_usd"]
                .as_f64()
                .map_or_else(|| "cost unknown".to_string(), |usd| format!("${usd:.4}")),
            row["milliseconds"].as_f64().unwrap_or(0.0) / 1000.0
        );
        rows.push(row);
    }
    let dir = out.ok_or("HOME is not set; pass --out")?.join(&study);
    // A checkout's set is named from the checkout's root, so a retained
    // result names no home directory.
    let set_named = {
        let path = set_path.to_string_lossy().into_owned();
        path.find("bench/")
            .map_or(path.clone(), |at| path[at..].to_string())
    };
    let mut result = json!({
        "schema": RESULT_SCHEMA,
        "study": study,
        "set": set.set,
        "set_digest": set.digest(),
        "set_path": set_named,
        "executor": which.word(),
        "model": model,
        "implementation": implementation,
        "implementation_digest": atif::digest(&implementation),
        "started_at": atif::document::iso(started_at),
        "version": crate::episode::version(),
        "rows": rows,
        "totals": totals(&rows),
        "dir": dir.to_string_lossy(),
    });
    let text_body = serde_json::to_string_pretty(&result).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
    crate::record::write_atomic(&dir.join("result.json"), text_body.as_bytes())?;
    if let Some(retain) = retain {
        let kept = retain.join(result["study"].as_str().unwrap_or("study"));
        std::fs::create_dir_all(&kept)
            .map_err(|error| format!("cannot create {}: {error}", kept.display()))?;
        result["dir"] = json!(kept.to_string_lossy());
        crate::record::write_atomic(
            &kept.join("result.json"),
            serde_json::to_string_pretty(&result)
                .map_err(|error| error.to_string())?
                .as_bytes(),
        )?;
    }
    if json_out {
        println!("{text_body}");
    } else {
        print!("{}", text(&result));
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_scores_on_validity_and_recall_of_tasks_and_runs() {
        let question = Question {
            id: "q".to_string(),
            question: "Which runs?".to_string(),
            source: String::new(),
            answer: String::new(),
            expected_tasks: vec!["a".to_string(), "b".to_string()],
            expected_runs: vec![
                "tb4--a/a__1".to_string(),
                "tb4--b/b__1".to_string(),
                "tb4--c/c__1".to_string(),
            ],
            note: String::new(),
        };
        let record = json!({
            "id": "ask-1",
            "status": "answered",
            "answer": "…",
            "citations": {"citations": 4, "valid_citations": 3, "validity": 0.75, "claims": 2, "verified": 1},
            "cited_runs": [
                {"cited": "tb4--a/a__1", "run": "tb4--a/a__1", "task": "a"},
                {"cited": "tb4--z", "run": "tb4--z/z__9", "task": "z"},
                {"cited": "nope/x", "run": null, "task": null},
            ],
            "cost": {"usd": 0.01, "jev_usd": 0.001, "executor_usd": 0.009},
            "milliseconds": 42_000,
        });
        let row = score(&question, &record);
        assert_eq!(row["task_recall"], 0.5);
        assert_eq!(row["tasks_missed"], json!(["b"]));
        assert_eq!(row["run_recall"], 0.333);
        assert_eq!(row["runs_found"], json!(["tb4--a/a__1"]));
        assert_eq!(row["validity"], 0.75);
        let unanswered = json!({"id": "q2", "answered": false, "error": "no gym"});
        let t = totals(&[row, unanswered]);
        assert_eq!(t["questions"], 2);
        assert_eq!(t["answered"], 1);
        assert_eq!(t["citation_validity"], 0.75);
        assert_eq!(t["mean_task_recall"], 0.5);
        assert_eq!(t["under_a_minute"], 1);
    }

    #[test]
    fn the_checkout_question_set_reads_and_digests() {
        let set = Set::load(&default_set()).unwrap();
        assert!(
            (10..=15).contains(&set.questions.len()),
            "{}",
            set.questions.len()
        );
        assert!(set.questions.iter().all(|q| !q.expected_tasks.is_empty()));
        assert_eq!(set.digest().len(), 64);
    }
}
