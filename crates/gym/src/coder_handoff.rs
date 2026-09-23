//! `control.handoff`: composition patterns compared as policies on the
//! mini-tasks.
//!
//! `coder-one handoff compare --out
//! bench/terminal-bench/handoff/minitask-patterns.json` runs the single-pass
//! Luna and Opus manifests and the escalate, planner-worker, steer, and race
//! manifests on every mini-task with scripted executors, and records each
//! cell's grade, episode time, modeled cost, handoffs, and branches. This
//! module reads that report into the outcome matrix's shape: a policy per
//! row, a task per column, and the pass count, cost, time, and objective
//! per policy.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The schema of the report this module reads.
pub const COMPARE_SCHEMA: &str = "openagents.coder-one.handoff-compare.v1";

/// The schema of this module's JSON.
pub const SCHEMA: &str = "openagents.gym.coder-handoff.v1";

/// The checked-in comparison.
#[must_use]
pub fn default_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/terminal-bench/handoff/minitask-patterns.json")
}

/// A pattern comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub path: PathBuf,
    pub value: Value,
}

/// Reads a comparison.
///
/// # Errors
///
/// Returns a message when the file is missing or isn't a comparison.
pub fn load(path: &Path) -> Result<Report, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    if value.get("schema").and_then(Value::as_str) != Some(COMPARE_SCHEMA) {
        return Err(format!(
            "{} is not a {COMPARE_SCHEMA} report",
            path.display()
        ));
    }
    Ok(Report {
        path: path.to_path_buf(),
        value,
    })
}

impl Report {
    fn tasks(&self) -> Vec<String> {
        let mut tasks: Vec<String> = Vec::new();
        for cell in self.value["cells"].as_array().into_iter().flatten() {
            if let Some(task) = cell["task"].as_str()
                && !tasks.iter().any(|t| t == task)
            {
                tasks.push(task.to_owned());
            }
        }
        tasks
    }

    fn cell(&self, policy: &str, task: &str) -> Option<&Value> {
        self.value["cells"]
            .as_array()?
            .iter()
            .find(|cell| cell["policy"] == policy && cell["task"] == task)
    }

    /// The comparison as text rows.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let tasks = self.tasks();
        let objective = &self.value["objective"];
        let mut lines = vec![
            format!(
                "Mini-task patterns · control.handoff as policies · {} policies × {} tasks · scripted executors, modeled cost",
                self.value["policies"].as_array().map_or(0, Vec::len),
                tasks.len()
            ),
            format!(
                "  J runtime = mean $ + λ · mean s, λ = ${}/s; J offline adds (1 − pass rate) · ${} · deadline {} s",
                objective["usd_per_second"].as_f64().unwrap_or(0.0),
                objective["fail_usd"].as_f64().unwrap_or(0.0),
                self.value["deadline_ms"].as_u64().unwrap_or(0) / 1_000
            ),
            format!(
                "  {:<34} {:<15} {}   pass   mean $  mean s  J runtime  J offline",
                "policy",
                "pattern",
                tasks
                    .iter()
                    .map(|task| format!("{:<14}", clip(task, 13)))
                    .collect::<String>()
            ),
        ];
        for policy in self.value["policies"].as_array().into_iter().flatten() {
            let name = policy["policy"].as_str().unwrap_or_default();
            let cells: String = tasks
                .iter()
                .map(|task| {
                    let text = self.cell(name, task).map_or("—".to_owned(), |cell| {
                        format!(
                            "{} {:>4.0}s",
                            match cell["passed"].as_bool() {
                                Some(true) => "✓",
                                Some(false) => "✗",
                                None => "?",
                            },
                            cell["ms"].as_f64().unwrap_or(0.0) / 1_000.0
                        )
                    });
                    format!("{text:<14}")
                })
                .collect();
            lines.push(format!(
                "  {:<34} {:<15} {}  {:>2}/{:<2}  {:>7.4}  {:>6.1}  {:>9.4}  {:>9}",
                clip(name, 34),
                policy["pattern"].as_str().unwrap_or_default(),
                cells,
                policy["passed"].as_u64().unwrap_or(0),
                policy["graded"].as_u64().unwrap_or(0),
                policy["mean_usd"].as_f64().unwrap_or(0.0),
                policy["mean_seconds"].as_f64().unwrap_or(0.0),
                policy["j_runtime"].as_f64().unwrap_or(0.0),
                policy["j_offline"]
                    .as_f64()
                    .map_or("—".to_owned(), |j| format!("{j:.4}")),
            ));
        }
        for note in self.value["notes"].as_array().into_iter().flatten() {
            if let Some(note) = note.as_str() {
                lines.push(format!("  note: {note}"));
            }
        }
        lines
    }

    /// The comparison as versioned JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema": SCHEMA,
            "source": self.path.display().to_string(),
            "objective": self.value["objective"],
            "rates": self.value["rates"],
            "notes": self.value["notes"],
            "policies": self.value["policies"],
            "cells": self.value["cells"].as_array().map(|cells| cells.iter().map(|cell| json!({
                "policy": cell["policy"],
                "pattern": cell["pattern"],
                "task": cell["task"],
                "passed": cell["passed"],
                "ms": cell["ms"],
                "usd": cell["usd"],
                "handoffs": cell["handoffs"],
                "branches": cell["branches"].as_array().map(|branches| branches.iter().map(|b| json!({
                    "role": b["role"],
                    "tier": b["tier"],
                    "ms": b["ms"],
                    "usd": b["usd"],
                    "status": b["status"],
                    "stopped_by": b["stopped_by"],
                    "reaped": b["reaped"],
                    "won": b["won"],
                })).collect::<Vec<_>>()),
            })).collect::<Vec<_>>()),
        })
    }
}

fn clip(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

/// `gym coder handoff`.
///
/// # Errors
///
/// Returns a message for a bad argument or an unreadable report.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let mut json_output = false;
    let mut path = default_path();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--json" => json_output = true,
            "--report" => path = PathBuf::from(rest.next().ok_or("--report needs a value")?),
            "help" | "--help" | "-h" => {
                writeln!(
                    out,
                    "gym coder handoff [--report PATH] [--json]\n\ncontrol.handoff's patterns compared as policies on the mini-tasks: each\ncell's grade and episode time, and each policy's pass count, modeled cost,\ntime, and objective, beside the single-pass Luna and Opus policies.\n\n  --report PATH  a comparison (default bench/terminal-bench/handoff/minitask-patterns.json,\n                 written by coder-one handoff compare)\n  --json         print versioned JSON instead of text"
                )
                .map_err(|error| error.to_string())?;
                return Ok(0);
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    let report = load(&path)?;
    if json_output {
        serde_json::to_writer_pretty(&mut *out, &report.to_json())
            .map_err(|error| error.to_string())?;
        writeln!(out).map_err(|error| error.to_string())?;
    } else {
        for line in report.lines() {
            writeln!(out, "{line}").map_err(|error| error.to_string())?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checked_in_comparison_reads_as_a_matrix_of_patterns() {
        let report = load(&default_path()).unwrap();
        let lines = report.lines();
        assert!(lines[0].contains("control.handoff"), "{}", lines[0]);
        for pattern in ["single", "escalate", "planner-worker", "steer", "race"] {
            assert!(
                lines.iter().any(|line| line.contains(pattern)),
                "{pattern} missing"
            );
        }
        let value = report.to_json();
        assert_eq!(value["schema"], SCHEMA);
        assert_eq!(value["policies"].as_array().unwrap().len(), 6);
        let mut out = Vec::new();
        assert_eq!(command(&["--json".to_owned()], &mut out).unwrap(), 0);
    }
}
