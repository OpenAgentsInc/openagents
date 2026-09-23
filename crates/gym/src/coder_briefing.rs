//! Coder One's briefings, as the Gym reads them: what went in, what was
//! left out and why, and how many characters each requirement got.
//!
//! Three sources feed the view:
//!
//! - An episode packed by requirement coverage writes
//!   `artifacts/briefing-pack.json`: every item's fate, its delivered and
//!   original size, the requirements it informs, and the route to what
//!   was left out.
//! - `coder-one component replay evidence.pack` writes a replay report of
//!   every retained briefing, the first packer beside the coverage packer
//!   on the same evidence.
//! - An isolated `evidence.pack` suite records, per fixture, both packers'
//!   measures and the coverage packer's record.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::coder_requirements::{Sources, suite_outputs};
use crate::terminal_bench::Records;

/// The schema of an episode's pack record.
pub const PACK_SCHEMA: &str = "openagents.coder-one.briefing-pack.v1";
/// The schema of a replay report.
pub const REPLAY_SCHEMA: &str = "openagents.coder-one.pack-replay.v1";
/// The schema of this report's JSON.
pub const SCHEMA: &str = "openagents.gym.coder-briefing.v1";

/// One briefing's pack record and where it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct Packed {
    /// `episode` or `suite`.
    pub source: String,
    pub label: String,
    /// The coverage packer's record.
    pub record: Value,
    /// Before-and-after measures, for a suite fixture.
    pub metrics: Value,
}

/// Everything the view shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub packs: Vec<Packed>,
    /// The latest replay report, and its path.
    pub replay: Option<(PathBuf, Value)>,
    pub errors: Vec<String>,
}

/// The newest replay report in `dir`.
fn latest_replay(dir: &Path) -> Option<(PathBuf, Value)> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                let name = name.to_string_lossy();
                name.starts_with("pack-replay-") && name.ends_with(".json")
            })
        })
        .collect();
    paths.sort();
    let path = paths.pop()?;
    let value: Value = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
    (value.get("schema").and_then(Value::as_str) == Some(REPLAY_SCHEMA)).then_some((path, value))
}

/// Reads pack records from episodes and suites, and the latest replay.
#[must_use]
pub fn report(runs_dir: Option<&Path>, records: &Records) -> Report {
    let mut report = Report::default();
    if let Some(dir) = runs_dir {
        report.replay = latest_replay(dir);
        let (outputs, errors) = suite_outputs(dir, "evidence.pack");
        report.errors.extend(errors);
        for run in outputs {
            let Some(record) = run.output.get("after").cloned().filter(Value::is_object) else {
                continue;
            };
            report.packs.push(Packed {
                source: format!("suite ({} Jev)", run.jev_mode),
                label: run.fixture,
                record,
                metrics: run.metrics,
            });
        }
    }
    for attempt in &records.attempts {
        for evidence in attempt
            .evidence
            .iter()
            .filter(|e| e.kind == "briefing_pack")
        {
            let Some(path) = &evidence.path else { continue };
            match std::fs::read_to_string(path)
                .map_err(|error| error.to_string())
                .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|e| e.to_string()))
            {
                Ok(value) if value.get("schema").and_then(Value::as_str) == Some(PACK_SCHEMA) => {
                    report.packs.push(Packed {
                        source: "episode".to_owned(),
                        label: format!("{} / {}", attempt.job, attempt.trial),
                        record: value.get("record").cloned().unwrap_or(Value::Null),
                        metrics: Value::Null,
                    });
                }
                Ok(_) => report
                    .errors
                    .push(format!("{}: not a briefing pack record", path.display())),
                Err(error) => report.errors.push(format!("{}: {error}", path.display())),
            }
        }
    }
    report
}

fn number(value: &Value) -> String {
    match value {
        Value::Null => "—".to_owned(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

fn clip(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_owned()
    } else {
        format!("{}…", text.chars().take(width - 1).collect::<String>())
    }
}

impl Packed {
    fn named(&self, query: &str) -> bool {
        self.label.contains(query)
    }

    fn heading(&self) -> String {
        let items = self.record["items"].as_array().cloned().unwrap_or_default();
        let count = |state: &str| items.iter().filter(|i| i["state"] == state).count();
        format!(
            "{} · {} · {} of {} characters · {} complete, {} trimmed, {} duplicate, {} left out · {} requirements without evidence",
            self.label,
            self.source,
            number(&self.record["chars"]),
            number(&self.record["cap"]),
            count("complete"),
            count("trimmed"),
            count("duplicate"),
            count("omitted"),
            self.record["uncovered"].as_array().map_or(0, Vec::len),
        )
    }

    fn item_lines(&self) -> Vec<String> {
        let mut lines = vec!["  state      shown/total  p     informs      item".to_owned()];
        for item in self.record["items"].as_array().into_iter().flatten() {
            let informs: Vec<&str> = item["informs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            lines.push(format!(
                "  {:<10} {:>5}/{:<5}  {:<5} {:<12} {}",
                item["state"].as_str().unwrap_or("?"),
                number(&item["delivered_chars"]),
                number(&item["original_chars"]),
                item["p"]
                    .as_f64()
                    .map_or("—".to_owned(), |p| format!("{p:.2}")),
                clip(&informs.join(","), 12),
                clip(item["label"].as_str().unwrap_or(""), 90),
            ));
            let why = [item["reason"].as_str(), item["route"].as_str()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("; ");
            if !why.is_empty() && item["state"] != "complete" {
                lines.push(format!("             {}", clip(&why, 120)));
            }
        }
        if let Some(bytes) = self.record["bytes_per_requirement"].as_object() {
            let row: Vec<String> = bytes
                .iter()
                .map(|(id, n)| format!("{id} {}", number(n)))
                .collect();
            lines.push(format!("  characters per requirement: {}", row.join(" · ")));
        }
        if self.metrics.is_object() {
            let m = &self.metrics;
            lines.push(format!(
                "  before → after: selected dropped {} → {}, duplicate bytes {} → {}, left out {} → {}, data characters {} → {}",
                number(&m["before_selected_dropped"]),
                number(&m["after_selected_dropped"]),
                number(&m["before_duplicate_bytes"]),
                number(&m["after_duplicate_bytes"]),
                number(&m["before_omitted"]),
                number(&m["after_omitted"]),
                number(&m["before_data_chars"]),
                number(&m["after_data_chars"]),
            ));
            if m.get("after_jev_uncovered_requirements").is_some() {
                lines.push(format!(
                    "  requirements without evidence: {} by rule, {} with Jev coverage judgments",
                    number(&m["after_uncovered_requirements"]),
                    number(&m["after_jev_uncovered_requirements"]),
                ));
            }
        }
        lines
    }
}

impl Report {
    fn chosen(&self, query: Option<&str>) -> Vec<&Packed> {
        self.packs
            .iter()
            .filter(|p| query.is_none_or(|q| p.named(q)))
            .collect()
    }

    fn replay_rows(&self, query: Option<&str>) -> Vec<&Value> {
        self.replay
            .as_ref()
            .and_then(|(_, value)| value["replayed"].as_array())
            .into_iter()
            .flatten()
            .filter(|row| {
                query.is_none_or(|q| {
                    row["trace"].as_str().is_some_and(|t| t.contains(q))
                        || row["episode"].as_str().is_some_and(|e| e.contains(q))
                })
            })
            .collect()
    }

    /// The report as versioned JSON.
    #[must_use]
    pub fn to_json(&self, query: Option<&str>) -> Value {
        json!({
            "schema": SCHEMA,
            "replay": self.replay.as_ref().map(|(path, value)| json!({
                "path": path.display().to_string(),
                "totals": value["totals"],
                "attempts": self.replay_rows(query),
            })),
            "packs": self.chosen(query).iter().map(|p| json!({
                "source": p.source,
                "label": p.label,
                "record": p.record,
                "metrics": p.metrics,
            })).collect::<Vec<_>>(),
            "errors": self.errors,
        })
    }

    /// The report as text.
    #[must_use]
    pub fn lines(&self, query: Option<&str>) -> Vec<String> {
        let mut lines = Vec::new();
        match &self.replay {
            Some((path, value)) => {
                let totals = &value["totals"];
                lines.push(format!(
                    "Briefing replay · {} retained briefings · {} rebuilt exactly · {}",
                    number(&totals["replayed"]),
                    number(&totals["reproduces"]),
                    path.display()
                ));
                lines.push(format!("  {:<50} {:>10} {:>10}", "", "before", "after"));
                for (label, key) in [
                    ("Jev-selected items dropped", "selected_dropped"),
                    ("  of them larger than the cap", "selected_over_cap_dropped"),
                    ("items left out", "omitted_items"),
                    ("duplicate listing bytes", "duplicate_bytes"),
                    ("data-file characters delivered", "data_chars"),
                    (
                        "episodes dropping selected items",
                        "episodes_dropping_selected",
                    ),
                    (
                        "  while keeping duplicate listings",
                        "episodes_dropping_selected_while_duplicates_kept",
                    ),
                ] {
                    lines.push(format!(
                        "  {label:<50} {:>10} {:>10}",
                        number(&totals["before"][key]),
                        number(&totals["after"][key])
                    ));
                }
                let rows = self.replay_rows(query);
                if query.is_some() {
                    lines.push(String::new());
                    lines.push("  attempt                                                        selected dropped   duplicate bytes   data chars".to_owned());
                    for row in rows {
                        lines.push(format!(
                            "  {:<62} {:>4} → {:<4}   {:>6} → {:<6}  {:>6} → {:<6}",
                            clip(
                                &format!(
                                    "{} / {}",
                                    row["trace"].as_str().unwrap_or(""),
                                    row["episode"].as_str().unwrap_or("")
                                ),
                                62
                            ),
                            number(&row["before"]["selected_dropped"]),
                            number(&row["after"]["selected_dropped"]),
                            number(&row["before"]["duplicate_bytes"]),
                            number(&row["after"]["duplicate_bytes"]),
                            number(&row["before"]["data_chars"]),
                            number(&row["after"]["data_chars"]),
                        ));
                    }
                }
            }
            None => lines.push(
                "No briefing replay recorded. Run `coder-one component replay evidence.pack`."
                    .to_owned(),
            ),
        }
        let chosen = self.chosen(query);
        for pack in &chosen {
            lines.push(String::new());
            lines.push(pack.heading());
            if query.is_some() || chosen.len() <= 2 {
                lines.extend(pack.item_lines());
            }
        }
        for error in &self.errors {
            lines.push(format!("read error: {error}"));
        }
        lines
    }
}

const HELP: &str = "\
gym coder briefing [QUERY]  what each briefing delivered, trimmed, and left out

Shows the latest `coder-one component replay evidence.pack` totals (the first
packer beside the coverage packer on every retained briefing), each episode's
pack record, and each evidence.pack suite fixture. A QUERY narrows to
attempts and fixtures whose name contains it and lists their items.

  --runs-dir PATH      component runs and replay reports (default ~/.openagents/coder-one/components)
  --jobs-dir PATH      local Harbor jobs (default ~/.openagents/terminal-bench/jobs)
  --traces-dir PATH    retained checkout traces
  --no-runs | --no-jobs | --no-traces
  --json               print versioned JSON instead of text";

/// `gym coder briefing …`.
///
/// # Errors
///
/// Returns a message for an unknown option.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let Some(options) = Sources::parse(args)? else {
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

    #[test]
    fn a_pack_record_reads_as_items_with_their_fates() {
        let pack = Packed {
            source: "episode".into(),
            label: "job / trial".into(),
            record: json!({
                "chars": 9000, "cap": 12000,
                "items": [
                    {"label": "logs/a.log", "state": "trimmed", "delivered_chars": 900, "original_chars": 8000, "p": 0.9, "informs": ["R2"], "reason": "its share", "route": "read lines 13–100 with `sed -n '13,100p' logs/a.log`"},
                    {"label": "$ ls -la /app/logs", "state": "duplicate", "delivered_chars": 0, "original_chars": 6000, "p": 0.9, "informs": [], "reason": "every entry is already listed"},
                ],
                "bytes_per_requirement": {"R1": 0, "R2": 900},
                "uncovered": ["R1"],
            }),
            metrics: Value::Null,
        };
        assert!(pack.heading().contains(
            "0 complete, 1 trimmed, 1 duplicate, 0 left out · 1 requirements without evidence"
        ));
        let lines = pack.item_lines().join("\n");
        assert!(lines.contains("sed -n '13,100p' logs/a.log"));
        assert!(lines.contains("characters per requirement: R1 0 · R2 900"));
        let report = Report {
            packs: vec![pack],
            replay: Some((
                PathBuf::from("pack-replay-1.json"),
                json!({"schema": REPLAY_SCHEMA, "totals": {"replayed": 1, "reproduces": 1, "before": {"selected_dropped": 3}, "after": {"selected_dropped": 0}}, "replayed": []}),
            )),
            errors: vec![],
        };
        let text = report.lines(None).join("\n");
        assert!(text.contains("Jev-selected items dropped"));
        assert_eq!(report.to_json(None)["schema"], json!(SCHEMA));
    }
}
