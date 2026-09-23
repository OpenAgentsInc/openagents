//! The Terminal-Bench 4.0 leaderboard as a reference beside this host's arms.
//!
//! `tbench reference` fetches the public leaderboard's rows from the Harbor
//! Hub into `bench/terminal-bench/reference/tb4-leaderboard.json`: per row,
//! the headline metrics and, per task, the successes over the trials, the
//! mean agent time, and the cost where the source job's aggregate matches
//! the row. This module reads that file so `gym terminal-bench overview`,
//! `gym terminal-bench compare`, and `gym coder matrix --profile tb4` can
//! show the leaderboard's rows beside the arms run here. It never reaches
//! the network.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The schema `tbench reference` writes.
pub const SCHEMA: &str = "openagents.tbench.reference.v1";

/// The job profile whose attempts the reference describes.
pub const PROFILE: &str = "tb4";

/// One task's result for one leaderboard row.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskResult {
    pub successes: u64,
    pub trials: u64,
    pub cost_usd: Option<f64>,
    pub mean_agent_sec: Option<f64>,
}

impl TaskResult {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "successes": self.successes,
            "trials": self.trials,
            "cost_usd": self.cost_usd,
            "mean_agent_sec": self.mean_agent_sec,
        })
    }
}

/// One leaderboard row.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub rank: Option<u64>,
    pub agent: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub accuracy: Option<f64>,
    pub successes: Option<u64>,
    pub trials: Option<u64>,
    pub total_cost_usd: Option<f64>,
    /// Whether the per-task counts add up to the row's own metrics.
    pub consistent: bool,
    pub tasks: BTreeMap<String, TaskResult>,
}

impl Entry {
    /// `Claude Code / Fable 5.1 (max)`.
    #[must_use]
    pub fn label(&self) -> String {
        match &self.reasoning_effort {
            Some(effort) => format!("{} / {} ({effort})", self.agent, self.model),
            None => format!("{} / {}", self.agent, self.model),
        }
    }

    #[must_use]
    pub fn summary_json(&self) -> Value {
        json!({
            "rank": self.rank,
            "label": self.label(),
            "agent": self.agent,
            "model": self.model,
            "reasoning_effort": self.reasoning_effort,
            "accuracy": self.accuracy,
            "successes": self.successes,
            "trials": self.trials,
            "total_cost_usd": self.total_cost_usd,
            "per_task_consistent": self.consistent,
        })
    }

    /// The row's line in a text view.
    #[must_use]
    pub fn summary_line(&self) -> String {
        format!(
            "#{} {}: {}/{} ({}) · {}",
            self.rank.map_or("?".to_owned(), |rank| rank.to_string()),
            self.label(),
            self.successes.map_or("?".to_owned(), |n| n.to_string()),
            self.trials.map_or("?".to_owned(), |n| n.to_string()),
            self.accuracy
                .map_or("?".to_owned(), |accuracy| format!("{accuracy:.1}%")),
            self.total_cost_usd
                .map_or("cost unknown".to_owned(), |cost| format!("${cost:.2}"))
        )
    }
}

/// The whole reference file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reference {
    pub leaderboard: String,
    pub fetched_at: String,
    pub dataset_ref: String,
    pub entries: Vec<Entry>,
}

/// Where the checked reference lives.
#[must_use]
pub fn default_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/terminal-bench/reference/tb4-leaderboard.json")
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

impl Reference {
    /// Reads the reference file.
    ///
    /// # Errors
    ///
    /// Returns a message for a missing or unreadable file, or another schema.
    pub fn load(path: &Path) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let value: Value =
            serde_json::from_str(&raw).map_err(|error| format!("{}: {error}", path.display()))?;
        Self::from_json(&value).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// Parses a reference document.
    ///
    /// # Errors
    ///
    /// Returns a message when the schema isn't the reference schema.
    pub fn from_json(value: &Value) -> Result<Self, String> {
        if value.get("schema").and_then(Value::as_str) != Some(SCHEMA) {
            return Err(format!("not a {SCHEMA} document"));
        }
        let entries = value
            .get("entries")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|entry| {
                let metrics = entry.get("metrics").cloned().unwrap_or(Value::Null);
                let tasks = entry
                    .get("tasks")
                    .and_then(Value::as_object)
                    .into_iter()
                    .flatten()
                    .map(|(task, cell)| {
                        (
                            task.clone(),
                            TaskResult {
                                successes: cell["successes"].as_u64().unwrap_or(0),
                                trials: cell["trials"].as_u64().unwrap_or(0),
                                cost_usd: cell["cost_usd"].as_f64(),
                                mean_agent_sec: cell["mean_agent_sec"].as_f64(),
                            },
                        )
                    })
                    .collect();
                Entry {
                    rank: entry["rank"].as_u64(),
                    agent: text(entry, "agent").unwrap_or_else(|| "?".to_owned()),
                    model: text(entry, "model").unwrap_or_else(|| "?".to_owned()),
                    reasoning_effort: text(entry, "reasoning_effort"),
                    accuracy: metrics["accuracy"].as_f64(),
                    successes: metrics["successes"].as_u64(),
                    trials: metrics["n_trials"].as_u64(),
                    total_cost_usd: metrics["total_cost_usd"].as_f64(),
                    consistent: entry["per_task"]["consistent"].as_bool().unwrap_or(false),
                    tasks,
                }
            })
            .collect();
        Ok(Self {
            leaderboard: text(value, "leaderboard").unwrap_or_default(),
            fetched_at: text(value, "fetched_at").unwrap_or_default(),
            dataset_ref: text(value, "dataset_ref").unwrap_or_default(),
            entries,
        })
    }

    /// Loads the checked reference, or `None` when it isn't there.
    #[must_use]
    pub fn checked() -> Option<Self> {
        Self::load(&default_path()).ok()
    }

    /// Every row's result on one task, best rank first.
    #[must_use]
    pub fn task(&self, task: &str) -> Vec<(&Entry, &TaskResult)> {
        self.entries
            .iter()
            .filter_map(|entry| entry.tasks.get(task).map(|result| (entry, result)))
            .collect()
    }

    /// The task names the reference covers.
    #[must_use]
    pub fn tasks(&self) -> Vec<String> {
        let mut tasks: Vec<String> = self
            .entries
            .iter()
            .flat_map(|entry| entry.tasks.keys().cloned())
            .collect();
        tasks.sort();
        tasks.dedup();
        tasks
    }

    /// The per-task rows as JSON: `[{label, rank, successes, trials, …}]`.
    #[must_use]
    pub fn task_json(&self, task: &str) -> Value {
        Value::Array(
            self.task(task)
                .into_iter()
                .map(|(entry, result)| {
                    let mut value = result.to_json();
                    value["label"] = json!(entry.label());
                    value["rank"] = json!(entry.rank);
                    value
                })
                .collect(),
        )
    }

    /// The document's header and every row's headline metrics.
    #[must_use]
    pub fn summary_json(&self) -> Value {
        json!({
            "schema": SCHEMA,
            "profile": PROFILE,
            "leaderboard": self.leaderboard,
            "dataset_ref": self.dataset_ref,
            "fetched_at": self.fetched_at,
            "entries": self.entries.iter().map(Entry::summary_json).collect::<Vec<_>>(),
        })
    }
}

/// One task's reference row as a short text cell: `3/5 $12.40 1,204 s`.
#[must_use]
pub fn cell_text(result: &TaskResult) -> String {
    format!(
        "{}/{} {} {}",
        result.successes,
        result.trials,
        result
            .cost_usd
            .map_or("cost ?".to_owned(), |cost| format!("${cost:.2}")),
        result
            .mean_agent_sec
            .map_or("—".to_owned(), |seconds| format!("{seconds:.0} s")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> Value {
        json!({
            "schema": SCHEMA,
            "leaderboard": "https://hub.example/leaderboards/x",
            "fetched_at": "2026-09-23T00:00:00+00:00",
            "dataset_ref": "v4.0.0",
            "entries": [
                {
                    "rank": 2,
                    "agent": "Claude Code",
                    "model": "Fable 5.1",
                    "reasoning_effort": "max",
                    "metrics": {"accuracy": 57.88, "successes": 191, "n_trials": 330, "total_cost_usd": 6243.5},
                    "per_task": {"consistent": true},
                    "tasks": {
                        "cad-model": {"successes": 5, "trials": 5, "cost_usd": 23.8, "mean_agent_sec": 1011.1},
                        "bun-sourcemap-leak": {"successes": 0, "trials": 5, "cost_usd": null, "mean_agent_sec": null}
                    }
                }
            ]
        })
    }

    #[test]
    fn a_reference_document_reads_per_task_rows() {
        let reference = Reference::from_json(&document()).unwrap();
        assert_eq!(reference.entries.len(), 1);
        let rows = reference.task("cad-model");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0.label(), "Claude Code / Fable 5.1 (max)");
        assert_eq!(cell_text(rows[0].1), "5/5 $23.80 1011 s");
        assert_eq!(
            cell_text(&reference.task("bun-sourcemap-leak")[0].1.clone()),
            "0/5 cost ? —"
        );
        assert!(reference.task("absent").is_empty());
        assert_eq!(reference.tasks(), ["bun-sourcemap-leak", "cad-model"]);
        assert!(
            reference.entries[0]
                .summary_line()
                .contains("191/330 (57.9%) · $6243.50")
        );
        assert_eq!(
            reference.task_json("cad-model")[0]["label"],
            "Claude Code / Fable 5.1 (max)"
        );
    }

    #[test]
    fn another_schema_is_refused() {
        assert!(Reference::from_json(&json!({"schema": "other"})).is_err());
    }

    #[test]
    fn the_checked_reference_covers_every_tb4_task() {
        let reference = Reference::load(&default_path()).unwrap();
        assert!(reference.entries.len() >= 13, "{}", reference.entries.len());
        assert_eq!(reference.tasks().len(), 66);
        for entry in &reference.entries {
            let trials: u64 = entry.tasks.values().map(|t| t.trials).sum();
            assert_eq!(Some(trials), entry.trials, "{}", entry.label());
        }
    }
}
