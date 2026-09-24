//! Coder One's requirement maps, as the Gym reads them.
//!
//! `task.requirements` turns the task's own words into a requirement map:
//! every span of the instruction with the requirement it became or the
//! context it stayed, each requirement's kind, binding, and state, and how
//! much of the instruction the spans cover. An episode writes its map to
//! `artifacts/requirements.json`; an isolated suite run records each
//! fixture's map, and its recall and precision against hand labels, in its
//! invocation log. This module reads both.

use std::path::Path;

use serde_json::{Value, json};

use crate::terminal_bench::Records;
use crate::timeline;

/// The schema Coder One writes a map with.
pub const MAP_SCHEMA: &str = "openagents.coder-one.requirements.v1";

/// The schema of this report's JSON.
pub const SCHEMA: &str = "openagents.gym.coder-requirements.v1";

/// One fixture's output from the latest isolated suite of a component.
#[derive(Clone, Debug, PartialEq)]
pub struct SuiteOutput {
    pub fixture: String,
    pub jev_mode: String,
    pub output: Value,
    pub metrics: Value,
    /// The log the suite was read from.
    pub log: String,
}

/// Each fixture's output from the most recent suite of `component` in
/// `runs_dir`, one suite per Jev mode, the latest first.
#[must_use]
pub fn suite_outputs(runs_dir: &Path, component: &str) -> (Vec<SuiteOutput>, Vec<String>) {
    let mut errors = Vec::new();
    let mut paths: Vec<_> = std::fs::read_dir(runs_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".atif.jsonl"))
        })
        .collect();
    paths.sort();
    // (started, mode) -> outputs, keeping the latest suite per mode.
    let mut latest: std::collections::BTreeMap<String, (u64, Vec<SuiteOutput>)> =
        std::collections::BTreeMap::new();
    for path in paths {
        let timeline = match timeline::read_log(&path) {
            Ok(timeline) => timeline,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        for root in timeline
            .entries
            .iter()
            .filter(|entry| entry.component == "suite" && entry.name.as_deref() == Some(component))
        {
            let mode = root
                .output
                .get("jev_mode")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned();
            let started = root.started_at.unwrap_or(0);
            let outputs: Vec<SuiteOutput> = timeline
                .entries
                .iter()
                .filter(|child| {
                    child.parent.as_deref() == Some(root.id.as_str())
                        && child.component == component
                })
                .map(|child| SuiteOutput {
                    fixture: child.name.clone().unwrap_or_default(),
                    jev_mode: mode.clone(),
                    output: child.output.get("output").cloned().unwrap_or(Value::Null),
                    metrics: child.output.get("metrics").cloned().unwrap_or(Value::Null),
                    log: path.display().to_string(),
                })
                .collect();
            if latest.get(&mode).is_none_or(|(seen, _)| *seen <= started) {
                latest.insert(mode, (started, outputs));
            }
        }
    }
    let mut suites: Vec<(u64, Vec<SuiteOutput>)> = latest.into_values().collect();
    suites.sort_by_key(|suite| std::cmp::Reverse(suite.0));
    (
        suites.into_iter().flat_map(|(_, runs)| runs).collect(),
        errors,
    )
}

/// One requirement map and where it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// `episode` or `suite`.
    pub source: String,
    /// The attempt (`job / trial`) or the fixture.
    pub label: String,
    /// The task name, when known.
    pub task: String,
    /// The suite's Jev mode, for a suite map.
    pub jev_mode: Option<String>,
    pub map: Value,
    /// Recall and precision against labels, for a labeled fixture.
    pub score: Option<Value>,
}

impl Entry {
    fn count(&self, pointer: &str) -> usize {
        self.map
            .pointer(pointer)
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    }

    fn uncertain(&self) -> usize {
        self.map
            .get("requirements")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|r| r.get("binding").and_then(Value::as_str) == Some("uncertain"))
            .count()
    }

    fn named(&self, query: &str) -> bool {
        self.label.contains(query) || self.task.contains(query)
    }

    /// The map's heading line.
    #[must_use]
    pub fn heading(&self) -> String {
        let coverage = self
            .map
            .pointer("/coverage/fraction")
            .and_then(Value::as_f64)
            .map_or("—".to_owned(), |f| format!("{:.0}%", f * 100.0));
        let score = self.score.as_ref().map_or(String::new(), |score| {
            format!(
                " · recall {} precision {}",
                number(score.get("recall")),
                number(score.get("precision"))
            )
        });
        format!(
            "{} · {}{} · by {} · {} spans · {} requirements ({} uncertain) · coverage {coverage}{score}",
            self.label,
            self.source,
            self.jev_mode
                .as_deref()
                .map_or(String::new(), |mode| format!(" ({mode} Jev)")),
            self.map
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("?"),
            self.count("/spans"),
            self.count("/requirements"),
            self.uncertain(),
        )
    }

    /// Every span with what it became.
    #[must_use]
    pub fn span_lines(&self) -> Vec<String> {
        let requirements: Vec<&Value> = self
            .map
            .get("requirements")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .collect();
        self.map
            .get("spans")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|span| {
                let text = span
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                let id = span.get("id").and_then(Value::as_str).unwrap_or("?");
                let requirement = span.get("requirement").and_then(Value::as_str);
                let found = requirement
                    .and_then(|rid| requirements.iter().find(|r| r["id"].as_str() == Some(rid)));
                let (role, kind, binding, p) = match found {
                    Some(r) => (
                        r["id"].as_str().unwrap_or("?").to_owned(),
                        r["kind"].as_str().unwrap_or("?").to_owned(),
                        r["binding"].as_str().unwrap_or("?").to_owned(),
                        r.get("p").and_then(Value::as_f64),
                    ),
                    None => (
                        "—".to_owned(),
                        "context".to_owned(),
                        String::new(),
                        span.pointer("/kinds/context")
                            .and_then(Value::as_f64)
                            .map(|c| 1.0 - c),
                    ),
                };
                format!(
                    "  {id:<4} {role:<4} {kind:<11} {binding:<9} {:<6} {}",
                    p.map_or("—".to_owned(), |p| format!("p={p:.2}")),
                    clip(&text, 110)
                )
            })
            .collect()
    }
}

fn number(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_f64)
        .map_or("—".to_owned(), |v| format!("{v:.2}"))
}

fn clip(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_owned()
    } else {
        format!("{}…", text.chars().take(width - 1).collect::<String>())
    }
}

/// Every map the Gym can read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub entries: Vec<Entry>,
    pub errors: Vec<String>,
}

/// Reads maps from episode bundles in `records` and from the latest
/// `task.requirements` suites in `runs_dir`.
#[must_use]
pub fn report(runs_dir: Option<&Path>, records: &Records) -> Report {
    let mut report = Report::default();
    if let Some(dir) = runs_dir {
        let (outputs, errors) = suite_outputs(dir, "task.requirements");
        report.errors.extend(errors);
        for run in outputs {
            let Some(map) = run.output.get("map").cloned().filter(Value::is_object) else {
                continue;
            };
            let task = run
                .fixture
                .rsplit("--")
                .next()
                .unwrap_or(&run.fixture)
                .to_owned();
            report.entries.push(Entry {
                source: "suite".to_owned(),
                label: run.fixture.clone(),
                task,
                jev_mode: Some(run.jev_mode.clone()),
                map,
                score: run.output.get("score").cloned().filter(Value::is_object),
            });
        }
    }
    for attempt in &records.attempts {
        for evidence in attempt.evidence.iter().filter(|e| e.kind == "requirements") {
            let Some(path) = &evidence.path else { continue };
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            match serde_json::from_str::<Value>(&text) {
                Ok(map) if map.get("schema").and_then(Value::as_str) == Some(MAP_SCHEMA) => {
                    report.entries.push(Entry {
                        source: "episode".to_owned(),
                        label: format!("{} / {}", attempt.job, attempt.trial),
                        task: attempt.task.clone(),
                        jev_mode: None,
                        map,
                        score: None,
                    });
                }
                Ok(_) => report
                    .errors
                    .push(format!("{}: not a requirement map", path.display())),
                Err(error) => report.errors.push(format!("{}: {error}", path.display())),
            }
        }
    }
    report
}

impl Report {
    fn chosen(&self, query: Option<&str>) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| query.is_none_or(|q| entry.named(q)))
            .collect()
    }

    /// The report as versioned JSON.
    #[must_use]
    pub fn to_json(&self, query: Option<&str>) -> Value {
        json!({
            "schema": SCHEMA,
            "maps": self.chosen(query).iter().map(|entry| json!({
                "source": entry.source,
                "label": entry.label,
                "task": entry.task,
                "jev_mode": entry.jev_mode,
                "score": entry.score,
                "map": entry.map,
            })).collect::<Vec<_>>(),
            "errors": self.errors,
        })
    }

    /// The report as text: a summary row per map, then, for a query, each
    /// map's spans.
    #[must_use]
    pub fn lines(&self, query: Option<&str>) -> Vec<String> {
        let chosen = self.chosen(query);
        let mut lines = vec![format!(
            "Requirement maps · {} from suites · {} from episodes",
            chosen.iter().filter(|e| e.source == "suite").count(),
            chosen.iter().filter(|e| e.source == "episode").count(),
        )];
        if chosen.is_empty() {
            lines.push("  No requirement map recorded. Run `coder-one component suite task.requirements`, or an episode with deep Jev.".to_owned());
        }
        for entry in &chosen {
            lines.push(String::new());
            lines.push(entry.heading());
            if query.is_some() || chosen.len() <= 3 {
                lines.push("  span role kind        binding   p      text".to_owned());
                lines.extend(entry.span_lines());
                if let Some(score) = &entry.score {
                    for (name, key) in [
                        ("missed labels", "missed"),
                        ("unlabeled requirements", "unmatched"),
                    ] {
                        let items: Vec<&str> = score
                            .get(key)
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .collect();
                        if !items.is_empty() {
                            lines.push(format!("  {name}: {}", items.join(", ")));
                        }
                    }
                }
            }
        }
        for error in &self.errors {
            lines.push(format!("could not read: {error}"));
        }
        lines
    }
}

const HELP: &str = "\
gym coder requirements [QUERY]  requirement maps from Coder One's task.requirements

A QUERY narrows to maps whose fixture, attempt, or task contains it, and shows
each span with the requirement it became or the context it stayed.

  --runs-dir PATH      component run logs (default ~/.openagents/coder-one/components)
  --jobs-dir PATH      local Harbor jobs (default ~/.openagents/terminal-bench/jobs)
  --traces-dir PATH    retained checkout traces
  --no-runs, --no-jobs, --no-traces  leave out that source
  --json               print versioned JSON instead of text";

/// Where a Coder One record view reads from, and how it prints.
#[derive(Clone, Debug, Default)]
pub struct Sources {
    pub runs: Option<std::path::PathBuf>,
    pub jobs: Option<std::path::PathBuf>,
    pub traces: Option<std::path::PathBuf>,
    pub json: bool,
    pub positional: Vec<String>,
}

impl Sources {
    /// Parses the shared flags; `None` asks for help.
    ///
    /// # Errors
    ///
    /// Returns a message for an unknown option or a flag without a value.
    pub fn parse(args: &[String]) -> Result<Option<Self>, String> {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench");
        let mut sources = Sources {
            runs: crate::coder_components::default_runs_dir(),
            jobs: std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|home| home.join(".openagents/terminal-bench/jobs")),
            traces: Some(repo.join("traces")),
            json: false,
            positional: Vec::new(),
        };
        let mut index = 0;
        while index < args.len() {
            let argument = args[index].as_str();
            match argument {
                "help" | "--help" | "-h" => return Ok(None),
                "--json" => sources.json = true,
                "--no-runs" => sources.runs = None,
                "--no-jobs" => sources.jobs = None,
                "--no-traces" => sources.traces = None,
                "--runs-dir" | "--jobs-dir" | "--traces-dir" => {
                    let value = args
                        .get(index + 1)
                        .filter(|value| !value.starts_with("--"))
                        .ok_or_else(|| format!("{argument} needs a value"))?;
                    match argument {
                        "--runs-dir" => sources.runs = Some(value.into()),
                        "--jobs-dir" => sources.jobs = Some(value.into()),
                        _ => sources.traces = Some(value.into()),
                    }
                    index += 1;
                }
                other if other.starts_with('-') => return Err(format!("unknown option {other}")),
                other => sources.positional.push(other.to_owned()),
            }
            index += 1;
        }
        Ok(Some(sources))
    }
}

/// `gym coder requirements …`.
///
/// # Errors
///
/// Returns a message for an unknown option.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let options = Sources::parse(args)?;
    let Some(options) = options else {
        writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
        return Ok(0);
    };
    let records = Records::load(options.jobs.as_deref(), options.traces.as_deref(), None);
    let report = report(options.runs.as_deref(), &records);
    let query = options.positional.first().map(String::as_str);
    if options.json {
        serde_json::to_writer_pretty(&mut *out, &report.to_json(query))
            .map_err(|error| error.to_string())?;
        writeln!(out).map_err(|error| error.to_string())?;
    } else {
        for line in report.lines(query) {
            writeln!(out, "{line}").map_err(|error| error.to_string())?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> Value {
        json!({
            "schema": MAP_SCHEMA,
            "method": "jev",
            "spans": [
                {"id": "s1", "text": "You are given logs.", "requirement": null, "kind": "context", "kinds": {"context": 0.9}},
                {"id": "s2", "text": "Write /app/out.csv.", "requirement": "R1", "kind": "deliverable"},
            ],
            "requirements": [
                {"id": "R1", "spans": ["s2"], "kind": "deliverable", "binding": "yes", "state": "unobserved", "p": 0.97, "text": "Write /app/out.csv."}
            ],
            "coverage": {"fraction": 1.0},
        })
    }

    #[test]
    fn a_map_reads_as_spans_with_their_requirements() {
        let entry = Entry {
            source: "suite".into(),
            label: "labeled--log".into(),
            task: "log".into(),
            jev_mode: Some("recorded".into()),
            map: map(),
            score: Some(
                json!({"recall": 1.0, "precision": 0.5, "missed": [], "unmatched": ["R2"]}),
            ),
        };
        let heading = entry.heading();
        assert!(heading.contains("2 spans · 1 requirements (0 uncertain) · coverage 100%"));
        assert!(heading.contains("recall 1.00 precision 0.50"));
        let spans = entry.span_lines();
        assert!(spans[0].contains("context") && spans[0].contains("p=0.10"));
        assert!(spans[1].contains("R1") && spans[1].contains("deliverable"));
        let report = Report {
            entries: vec![entry],
            errors: vec![],
        };
        let text = report.lines(Some("log")).join("\n");
        assert!(text.contains("unlabeled requirements: R2"));
        assert_eq!(report.to_json(None)["schema"], json!(SCHEMA));
    }
}
