//! `control.monitor`: trigger precision, stale answers, and cost, from
//! Coder One's replay of every retained native stream.
//!
//! `coder-one component replay control.monitor --out
//! bench/terminal-bench/monitor/replay.json` feeds each retained stream
//! through the monitor one event at a time and labels each judgment by
//! hindsight. This module reads that report: the rules alone over every
//! stream, and the rules beside Jev over the streams Jev answered.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The schema of the report this module reads.
pub const REPLAY_SCHEMA: &str = "openagents.coder-one.monitor-replay.v1";

/// The schema of this module's JSON.
pub const SCHEMA: &str = "openagents.gym.coder-monitor.v1";

/// The questions, in report order, with `intervene` first.
pub const QUESTIONS: &[&str] = &[
    "intervene",
    "stalled",
    "repeating",
    "rereading",
    "claims_done",
];

/// The checked-in replay report.
#[must_use]
pub fn default_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/monitor/replay.json")
}

/// A replay report.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub path: PathBuf,
    pub value: Value,
}

/// Reads a replay report.
///
/// # Errors
///
/// Returns a message when the file is missing or isn't a replay report.
pub fn load(path: &Path) -> Result<Report, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    if value.get("schema").and_then(Value::as_str) != Some(REPLAY_SCHEMA) {
        return Err(format!(
            "{} is not a {REPLAY_SCHEMA} report",
            path.display()
        ));
    }
    Ok(Report {
        path: path.to_path_buf(),
        value,
    })
}

fn ratio(value: &Value) -> String {
    value
        .as_f64()
        .map_or("—".to_owned(), |ratio| format!("{ratio:.2}"))
}

fn cell(counts: &Value) -> String {
    if counts.is_null() {
        return format!("{:>4} {:>5} {:>5}", "—", "—", "—");
    }
    format!(
        "{:>4} {:>5} {:>5}",
        counts["flagged"].as_u64().unwrap_or(0),
        ratio(&counts["precision"]),
        ratio(&counts["recall"])
    )
}

impl Report {
    /// The report as text rows.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let value = &self.value;
        let totals = &value["totals"];
        let compared = &value["compared"];
        let mut lines = vec![
            format!(
                "control.monitor · shadow-mode replay of {} retained streams · {} triggers ({})",
                totals["streams"].as_u64().unwrap_or(0),
                totals["triggers"].as_u64().unwrap_or(0),
                totals["by_trigger"]
                    .as_object()
                    .map(|counts| counts
                        .iter()
                        .map(|(kind, n)| format!("{kind} {n}"))
                        .collect::<Vec<_>>()
                        .join(" · "))
                    .unwrap_or_default()
            ),
            format!(
                "  Jev: {} answered of {} asks ({} live) · stale {} ({}) · cost ${:.4} at the published rate, {} of the ${:.2} the executors spent · {} characters a request",
                totals["jev_answered"].as_u64().unwrap_or(0),
                totals["jev_requests"].as_u64().unwrap_or(0),
                totals["live_requests"].as_u64().unwrap_or(0),
                totals["stale"].as_u64().unwrap_or(0),
                ratio(&totals["stale_rate"]),
                totals["cost_usd"].as_f64().unwrap_or(0.0),
                totals["monitor_share_of_delegate"]
                    .as_f64()
                    .map_or("—".to_owned(), |share| format!("{:.1}%", share * 100.0)),
                totals["delegate_usd"].as_f64().unwrap_or(0.0),
                totals["mean_state_chars"].as_u64().unwrap_or(0),
            ),
            format!(
                "  trigger precision by question: flagged, precision, and recall measured on hindsight labels · Jev compared on {} judgments it answered",
                compared["triggers"].as_u64().unwrap_or(0)
            ),
            "  question       labels   rules alone, all streams   rules, answered   Jev, answered"
                .to_owned(),
        ];
        for question in QUESTIONS {
            lines.push(format!(
                "  {:<13} {:>7}   {:<24}   {:<15}   {}",
                question,
                totals["labels"][question].as_u64().unwrap_or(0),
                cell(&totals["rules"][question]),
                cell(&compared["rules"][question]),
                cell(&compared["jev"][question]),
            ));
        }
        if let Some(families) = value["by_family"].as_object() {
            lines.push(
                "  by model family: triggers, rules' intervene precision, Jev asks, cost"
                    .to_owned(),
            );
            for (family, score) in families {
                lines.push(format!(
                    "    {:<8} {:>5} triggers · intervene precision {} · {} asks · ${:.4}",
                    family,
                    score["triggers"].as_u64().unwrap_or(0),
                    ratio(&score["rules"]["intervene"]["precision"]),
                    score["jev_requests"].as_u64().unwrap_or(0),
                    score["cost_usd"].as_f64().unwrap_or(0.0),
                ));
            }
        }
        for note in value["notes"].as_array().into_iter().flatten() {
            if let Some(note) = note.as_str() {
                lines.push(format!("  note: {note}"));
            }
        }
        lines
    }

    /// The report as versioned JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema": SCHEMA,
            "source": self.path.display().to_string(),
            "totals": self.value["totals"],
            "compared": self.value["compared"],
            "by_family": self.value["by_family"],
            "params": self.value["params"],
            "notes": self.value["notes"],
            "streams": self.value["replayed"].as_array().map(|streams| streams.iter().map(|s| json!({
                "trace": s["trace"],
                "stream": s["stream"],
                "task": s["task"],
                "arm": s["arm"],
                "family": s["family"],
                "passed": s["passed"],
                "jev": s["jev"],
                "triggers": s["score"]["triggers"],
                "labels": s["score"]["labels"],
                "stale": s["score"]["stale"],
                "monitor_usd": s["monitor_usd"],
                "delegate_usd": s["delegate_usd"],
            })).collect::<Vec<_>>()),
        })
    }
}

/// `gym coder monitor`.
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
                    "gym coder monitor [--report PATH] [--json]\n\ncontrol.monitor replayed over every retained native stream, observing\nwithout acting: triggers, trigger precision per question for the rules\nalone and for Jev, stale answers, and the monitor's cost beside the\nexecutors' spend.\n\n  --report PATH  a replay report (default bench/terminal-bench/monitor/replay.json,\n                 written by coder-one component replay control.monitor)\n  --json         print versioned JSON instead of text"
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
    fn the_checked_in_replay_reports_precision_and_cost() {
        let report = load(&default_path()).unwrap();
        let lines = report.lines();
        assert!(lines[0].contains("retained streams"), "{}", lines[0]);
        assert!(lines.iter().any(|line| line.starts_with("  stalled")));
        assert!(lines.iter().any(|line| line.contains("stale")));
        let value = report.to_json();
        assert_eq!(value["schema"], SCHEMA);
        assert!(value["totals"]["triggers"].as_u64().unwrap() > 0);
        assert!(value["compared"]["jev_answered"].as_u64().unwrap() > 0);
        let mut out = Vec::new();
        assert_eq!(command(&["--json".to_owned()], &mut out).unwrap(), 0);
        let parsed: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(parsed["schema"], SCHEMA);
    }
}
