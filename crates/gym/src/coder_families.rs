//! The family table a Coder One route reads, recomputed from its sources.
//!
//! A policy's `control.route.families` names task families by the words in
//! a task's instruction, and gives each executor profile's passes on each
//! family. Those rows are data read from the Terminal-Bench 4.0
//! leaderboard. This module recomputes them: it matches every task
//! instruction in a Terminal-Bench checkout against each family's words,
//! sums the leaderboard row each profile names over the matched tasks, and
//! compares the sums with the rows the policy carries. `gym coder families
//! --json` prints the recomputed rows in the policy's shape.
//!
//! The rows come from the same 66 tasks a suite run is scored on, so a
//! route that reads them is fitted in sample. The view says so.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::terminal_bench_reference::Reference;

/// The schema of `gym coder families --json`.
pub const SCHEMA: &str = "openagents.gym.coder-families.v1";

/// Whether `phrase` occurs in `lower` as whole words. The same rule as the
/// route's.
#[must_use]
pub fn has_phrase(lower: &str, phrase: &str) -> bool {
    let phrase = phrase.to_lowercase();
    if phrase.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(i) = lower[from..].find(&phrase) {
        let at = from + i;
        let end = at + phrase.len();
        let before = lower[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after = lower[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        if before && after {
            return true;
        }
        from = end;
    }
    false
}

fn words(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

/// Whether an instruction belongs to a family, by the family's `any` and
/// `all` words.
#[must_use]
pub fn matches(family: &Value, instruction: &str) -> bool {
    let lower = instruction.to_lowercase();
    words(&family["any"]).iter().any(|p| has_phrase(&lower, p))
        && family["all"]
            .as_array()
            .into_iter()
            .flatten()
            .all(|group| words(group).iter().any(|p| has_phrase(&lower, p)))
}

/// One profile's recomputed row on one family.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub profile: String,
    pub reference_row: Option<String>,
    pub passes: u64,
    pub trials: u64,
    pub mean_cost_usd: Option<f64>,
    /// The policy's row for the same profile, when it has one.
    pub policy: Option<(u64, u64)>,
}

impl Row {
    /// Whether the policy carries exactly the recomputed passes and trials.
    #[must_use]
    pub fn agrees(&self) -> bool {
        self.policy == Some((self.passes, self.trials))
    }
}

/// One family: the tasks it matched and each profile's row.
#[derive(Clone, Debug, PartialEq)]
pub struct Family {
    pub name: String,
    pub tasks: Vec<String>,
    pub rows: Vec<Row>,
}

/// The whole view.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub policy: String,
    pub source: String,
    pub min_trials: u64,
    pub min_gap: f64,
    pub families: Vec<Family>,
    /// Tasks no family matched.
    pub unmatched: Vec<String>,
    /// Tasks that matched more than one family; the first match wins.
    pub overlaps: Vec<(String, Vec<String>)>,
}

/// Reads every `<tasks>/<task>/instruction.md`, by task name.
///
/// # Errors
///
/// Returns a message when the directory can't be read.
pub fn instructions(tasks: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(tasks)
        .map_err(|error| format!("{}: {error}", tasks.display()))?
        .flatten()
    {
        let file = entry.path().join("instruction.md");
        if let Ok(text) = std::fs::read_to_string(&file) {
            out.insert(entry.file_name().to_string_lossy().into_owned(), text);
        }
    }
    Ok(out)
}

/// Builds the view from a policy manifest, the task instructions, and the
/// leaderboard reference.
///
/// # Errors
///
/// Returns a message when the policy has no family table.
pub fn view(
    policy_name: &str,
    policy: &Value,
    instructions: &BTreeMap<String, String>,
    reference: &Reference,
) -> Result<View, String> {
    let families = &policy["policy"]["control"]["route"]["families"];
    if !families.is_object() {
        return Err(format!("{policy_name} has no control.route.families table"));
    }
    let rows_by_label: BTreeMap<String, &crate::terminal_bench_reference::Entry> = reference
        .entries
        .iter()
        .map(|entry| (entry.label(), entry))
        .collect();
    let profiles: Vec<String> = families["profiles"]
        .as_object()
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default();
    let table = families["table"].as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    let mut matched_by: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for family in &table {
        let name = family["name"].as_str().unwrap_or("?").to_owned();
        let tasks: Vec<String> = instructions
            .iter()
            .filter(|(_, text)| matches(family, text))
            .map(|(task, _)| task.clone())
            .collect();
        for task in &tasks {
            matched_by
                .entry(task.clone())
                .or_default()
                .push(name.clone());
        }
        let mut rows = Vec::new();
        for profile in &profiles {
            let label = families["reference_rows"][profile]
                .as_str()
                .map(str::to_owned);
            let entry = label.as_ref().and_then(|l| rows_by_label.get(l));
            let (mut passes, mut trials, mut cost, mut costed) = (0, 0, 0.0, true);
            if let Some(entry) = entry {
                for task in &tasks {
                    if let Some(result) = entry.tasks.get(task) {
                        passes += result.successes;
                        trials += result.trials;
                        match result.cost_usd {
                            Some(usd) => cost += usd,
                            None => costed = false,
                        }
                    }
                }
            }
            let policy_row = family["outcomes"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|o| o["profile"] == profile.as_str())
                .map(|o| {
                    (
                        o["passes"].as_u64().unwrap_or(0),
                        o["trials"].as_u64().unwrap_or(0),
                    )
                });
            rows.push(Row {
                profile: profile.clone(),
                reference_row: label,
                passes,
                trials,
                mean_cost_usd: (costed && trials > 0)
                    .then(|| (cost / trials as f64 * 10_000.0).round() / 10_000.0),
                policy: policy_row,
            });
        }
        out.push(Family { name, tasks, rows });
    }
    let unmatched = instructions
        .keys()
        .filter(|task| !matched_by.contains_key(*task))
        .cloned()
        .collect();
    let overlaps = matched_by
        .into_iter()
        .filter(|(_, names)| names.len() > 1)
        .collect();
    Ok(View {
        policy: policy_name.to_owned(),
        source: families["source"].as_str().unwrap_or_default().to_owned(),
        min_trials: families["min_trials"].as_u64().unwrap_or(0),
        min_gap: families["min_gap"].as_f64().unwrap_or(0.0),
        families: out,
        unmatched,
        overlaps,
    })
}

impl View {
    /// The view as text.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("Route families · {}", self.policy),
            format!(
                "  source: {} · a row decides with at least {} trials and a lead of {:.2}",
                self.source, self.min_trials, self.min_gap
            ),
            "  The rows come from the tasks a suite run is scored on, so a route that reads them is fitted in sample.".to_owned(),
        ];
        for family in &self.families {
            lines.push(String::new());
            lines.push(format!(
                "{} · {} task(s): {}",
                family.name,
                family.tasks.len(),
                if family.tasks.is_empty() {
                    "none".to_owned()
                } else {
                    family.tasks.join(", ")
                }
            ));
            for row in &family.rows {
                lines.push(format!(
                    "  {:<8} {:>3}/{:<3} {:>9}  {:<34} policy {}",
                    row.profile,
                    row.passes,
                    row.trials,
                    row.mean_cost_usd
                        .map_or_else(|| "—".to_owned(), |usd| format!("${usd:.2}")),
                    row.reference_row.as_deref().unwrap_or("no reference row"),
                    match row.policy {
                        Some((p, t)) if row.agrees() => format!("{p}/{t}, agrees"),
                        Some((p, t)) => format!("{p}/{t}, DIFFERS"),
                        None => "has no row".to_owned(),
                    }
                ));
            }
        }
        lines.push(String::new());
        lines.push(format!(
            "{} task(s) match no family and keep the rule's pick.",
            self.unmatched.len()
        ));
        for (task, names) in &self.overlaps {
            lines.push(format!(
                "  {task} matches {}; the first wins",
                names.join(", ")
            ));
        }
        lines
    }

    /// The view as versioned JSON, with each family's rows in the policy's
    /// `outcomes` shape.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema": SCHEMA,
            "policy": self.policy,
            "source": self.source,
            "min_trials": self.min_trials,
            "min_gap": self.min_gap,
            "in_sample": true,
            "families": self.families.iter().map(|family| json!({
                "name": family.name,
                "tasks": family.tasks,
                "outcomes": family.rows.iter().map(|row| {
                    let mut value = json!({
                        "profile": row.profile,
                        "passes": row.passes,
                        "trials": row.trials,
                    });
                    if let Some(usd) = row.mean_cost_usd {
                        value["mean_cost_usd"] = json!(usd);
                    }
                    value
                }).collect::<Vec<_>>(),
                "policy_agrees": family.rows.iter().all(Row::agrees),
            })).collect::<Vec<_>>(),
            "unmatched": self.unmatched,
            "overlaps": self.overlaps.iter().map(|(task, names)| json!({ "task": task, "families": names })).collect::<Vec<_>>(),
        })
    }
}

const HELP: &str = "gym coder families [--policy PATH] [--tasks DIR] [--reference PATH] [--json]

Recomputes a Coder One policy's control.route.families table: the Terminal-Bench
tasks each family's words match, and each profile's passes over them in the
leaderboard row the policy names for it. Says whether the policy's rows agree.

  --policy PATH     a policy manifest (default crates/coder-one/policies/tunable-v4.json)
  --tasks DIR       Terminal-Bench task directories with instruction.md
                    (default ~/.openagents/terminal-bench/upstream/terminal-bench-v4.0.0/tasks)
  --reference PATH  the leaderboard reference (default bench/terminal-bench/reference/tb4-leaderboard.json)
  --json            print versioned JSON, with each family's rows in the policy's shape";

/// `gym coder families`.
///
/// # Errors
///
/// Returns a message for an unknown option, an unreadable input, or a
/// failed write.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut policy = root.join("crates/coder-one/policies/tunable-v4.json");
    let mut tasks = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".openagents/terminal-bench/upstream/terminal-bench-v4.0.0/tasks"));
    let mut reference = crate::terminal_bench_reference::default_path();
    let mut json_out = false;
    let mut iter = args.iter();
    while let Some(argument) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match argument.as_str() {
            "help" | "--help" | "-h" => {
                writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
                return Ok(0);
            }
            "--policy" => policy = PathBuf::from(value("--policy")?),
            "--tasks" => tasks = Some(PathBuf::from(value("--tasks")?)),
            "--reference" => reference = PathBuf::from(value("--reference")?),
            "--json" => json_out = true,
            other => return Err(format!("unknown option {other}\n\n{HELP}")),
        }
    }
    let tasks = tasks.ok_or("no --tasks directory and no HOME")?;
    let text = std::fs::read_to_string(&policy)
        .map_err(|error| format!("{}: {error}", policy.display()))?;
    let manifest: Value =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", policy.display()))?;
    let name = manifest["name"].as_str().unwrap_or("policy").to_owned();
    let view = view(
        &name,
        &manifest,
        &instructions(&tasks)?,
        &Reference::load(&reference)?,
    )?;
    if json_out {
        writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&view.to_json()).map_err(|error| error.to_string())?
        )
        .map_err(|error| error.to_string())?;
    } else {
        for line in view.lines() {
            writeln!(out, "{line}").map_err(|error| error.to_string())?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_bench_reference::{Entry, TaskResult};

    fn entry(label_model: &str, results: &[(&str, u64)]) -> Entry {
        Entry {
            rank: None,
            agent: "Claude Code".to_owned(),
            model: label_model.to_owned(),
            reasoning_effort: Some("xhigh".to_owned()),
            accuracy: None,
            successes: None,
            trials: None,
            total_cost_usd: None,
            consistent: true,
            tasks: results
                .iter()
                .map(|(task, passes)| {
                    (
                        (*task).to_owned(),
                        TaskResult {
                            successes: *passes,
                            trials: 5,
                            cost_usd: Some(5.0),
                            mean_agent_sec: None,
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn a_family_sums_its_matched_tasks_and_checks_the_policy() {
        let policy = json!({
            "name": "p",
            "policy": { "control": { "route": { "families": {
                "source": "test",
                "min_trials": 10,
                "min_gap": 0.2,
                "profiles": { "a": {}, "b": {} },
                "reference_rows": { "a": "Claude Code / A (xhigh)", "b": "Claude Code / B (xhigh)" },
                "table": [{
                    "name": "cad",
                    "any": ["step file"],
                    "all": [["schematic", "drawing"]],
                    "outcomes": [{ "profile": "a", "passes": 8, "trials": 10 }],
                }],
            }}}},
        });
        let instructions: BTreeMap<String, String> = [
            ("cad-model", "Write a STEP file for the 2d schematic."),
            ("cad-other", "Write a STEP file."),
            ("steps", "Count the stepfile."),
        ]
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
        let reference = Reference {
            entries: vec![
                entry("A", &[("cad-model", 4), ("cad-other", 5)]),
                entry("B", &[("cad-model", 5)]),
            ],
            ..Reference::default()
        };
        let view = view("p", &policy, &instructions, &reference).unwrap();
        let family = &view.families[0];
        assert_eq!(family.tasks, ["cad-model"]);
        assert_eq!((family.rows[0].passes, family.rows[0].trials), (4, 5));
        assert!(!family.rows[0].agrees());
        assert_eq!(family.rows[1].policy, None);
        assert_eq!(view.unmatched, ["cad-other", "steps"]);
        let json = view.to_json();
        assert_eq!(json["families"][0]["outcomes"][1]["passes"], 5);
        assert_eq!(json["families"][0]["outcomes"][1]["mean_cost_usd"], 1.0);
        assert!(view.lines().iter().any(|l| l.contains("DIFFERS")));
    }
}
