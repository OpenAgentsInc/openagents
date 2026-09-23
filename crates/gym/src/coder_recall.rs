//! Check recall on the labeled Terminal-Bench set.
//!
//! `coder-one checks recall` replays Coder One's checks over every graded
//! retained trial and writes a summary
//! (`openagents.coder-one.check-recall.v1`) to
//! `~/.openagents/coder-one/checks-recall/summary.json`: for the episode's
//! own first check and for each replay arm, how many verifier failures the
//! checks flagged (recall) and how many verifier passes they flagged
//! (false alarms), then one row per trial. This module reads that summary
//! and renders it.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The schema a recall summary carries.
pub const SCHEMA: &str = "openagents.coder-one.check-recall.v1";

/// The schema of this module's JSON.
pub const VIEW_SCHEMA: &str = "openagents.gym.coder-recall.v1";

const HELP: &str = "\
gym coder recall [--dir PATH] [--failures] [--json]

Check recall on the labeled Terminal-Bench set that `coder-one checks recall`
writes: for the episodes' own first checks and each replay arm, the verifier
failures the checks flagged and the verifier passes they flagged, then one
row per graded trial with the scenarios that failed.

  --dir PATH    the recall directory (default ~/.openagents/coder-one/checks-recall)
  --failures    only the trials the verifier failed
  --json        print versioned JSON instead of text";

/// Where `coder-one checks recall` writes by default.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/checks-recall"))
}

/// Reads the summary in `dir`.
///
/// # Errors
///
/// Returns why the summary can't be read.
pub fn load(dir: &Path) -> Result<Value, String> {
    let path = dir.join("summary.json");
    let text = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "cannot read {}: {e}; run `coder-one checks recall` first",
            path.display()
        )
    })?;
    let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if value["schema"] != SCHEMA {
        return Err(format!("{} isn't a {SCHEMA} summary", path.display()));
    }
    Ok(value)
}

fn columns(summary: &Value) -> Vec<String> {
    summary["columns"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c.as_str().map(str::to_string))
        .collect()
}

/// The summary as text lines: totals first, then one row per trial.
#[must_use]
pub fn lines(summary: &Value, failures_only: bool) -> Vec<String> {
    let columns = columns(summary);
    let mut lines = vec!["Check recall on the labeled Terminal-Bench set".to_string()];
    for total in summary["totals"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  {:<8} flags {:>2} of {:>2} failures ({:>3.0}%) · {:>2} of {:>2} passes ({:>3.0}%)",
            total["arm"].as_str().unwrap_or_default(),
            total["flagged_failures"].as_u64().unwrap_or(0),
            total["failures"].as_u64().unwrap_or(0),
            total["recall"].as_f64().unwrap_or(0.0) * 100.0,
            total["flagged_passes"].as_u64().unwrap_or(0),
            total["passes"].as_u64().unwrap_or(0),
            total["false_alarm_rate"].as_f64().unwrap_or(0.0) * 100.0,
        ));
    }
    lines.push(String::new());
    lines.push(format!(
        "  {:<30} {:<4} {:>6}  {}  scenarios that failed",
        "task",
        "arm",
        "reward",
        columns
            .iter()
            .map(|c| format!("{c:<8}"))
            .collect::<String>()
    ));
    let last = columns.last().cloned().unwrap_or_default();
    for row in summary["rows"].as_array().into_iter().flatten() {
        let failed = row["reward"].as_f64().is_some_and(|r| r < 1.0);
        if failures_only && !failed {
            continue;
        }
        let marks: String = columns
            .iter()
            .map(|c| {
                format!(
                    "{:<8}",
                    if row["flagged"][c] == true {
                        "flags"
                    } else {
                        "·"
                    }
                )
            })
            .collect();
        let why: Vec<&str> = row["failed"][&last]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        lines.push(format!(
            "  {:<30} {:<4} {:>6}  {marks}  {}",
            row["task"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(30)
                .collect::<String>(),
            row["arm"]
                .as_str()
                .unwrap_or_default()
                .rsplit('-')
                .next()
                .unwrap_or_default(),
            row["reward"]
                .as_f64()
                .map_or("—".to_string(), |r| format!("{r:.1}")),
            why.join(" ")
        ));
    }
    let excluded = summary["excluded"].as_array().map_or(0, Vec::len);
    if excluded > 0 {
        lines.push(String::new());
        lines.push(format!(
            "  {excluded} trial(s) left out: the agent never ran, or the verifier left no reward."
        ));
    }
    lines
}

/// `gym coder recall`.
///
/// # Errors
///
/// Returns a message for a bad argument or an unreadable summary.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let mut dir = default_dir();
    let mut failures_only = false;
    let mut json_output = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "help" | "--help" | "-h" => {
                writeln!(out, "{HELP}").map_err(|e| e.to_string())?;
                return Ok(0);
            }
            "--json" => json_output = true,
            "--failures" => failures_only = true,
            "--dir" => {
                dir = Some(
                    args.get(index + 1)
                        .ok_or("--dir needs a value")?
                        .clone()
                        .into(),
                );
                index += 1;
            }
            other => return Err(format!("unknown option {other}\n\n{HELP}")),
        }
        index += 1;
    }
    let dir = dir.ok_or("no --dir and no HOME")?;
    let summary = load(&dir)?;
    if json_output {
        let mut value = json!({ "schema": VIEW_SCHEMA, "dir": dir, "summary": summary });
        if failures_only && let Some(rows) = value["summary"]["rows"].as_array_mut() {
            rows.retain(|r| r["reward"].as_f64().is_some_and(|x| x < 1.0));
        }
        serde_json::to_writer_pretty(&mut *out, &value).map_err(|e| e.to_string())?;
        writeln!(out).map_err(|e| e.to_string())?;
    } else {
        for line in lines(&summary, failures_only) {
            writeln!(out, "{line}").map_err(|e| e.to_string())?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary() -> Value {
        json!({
            "schema": SCHEMA,
            "columns": ["episode", "v7"],
            "totals": [
                { "arm": "episode", "failures": 2, "flagged_failures": 0, "recall": 0.0, "passes": 1, "flagged_passes": 1, "false_alarm_rate": 1.0 },
                { "arm": "v7", "failures": 2, "flagged_failures": 1, "recall": 0.5, "passes": 1, "flagged_passes": 0, "false_alarm_rate": 0.0 },
            ],
            "rows": [
                { "task": "html-js-filter", "arm": "coder-one-tunable-v2", "reward": 0.0, "flagged": { "episode": false, "v7": true }, "failed": { "v7": ["behavior.filter-preserves"] } },
                { "task": "gsea-proteomics", "arm": "coder-one-tunable-v2", "reward": 0.0, "flagged": { "episode": false, "v7": false }, "failed": { "v7": [] } },
                { "task": "roy-polymorph-cn", "arm": "coder-one-tunable-v2", "reward": 1.0, "flagged": { "episode": true, "v7": false }, "failed": { "v7": [] } },
            ],
            "excluded": [ { "job": "j", "trial": "t", "why": "Harbor recorded NonZeroAgentExitCodeError" } ],
        })
    }

    #[test]
    fn the_view_shows_recall_false_alarms_and_the_failed_scenarios() {
        let text = lines(&summary(), false).join("\n");
        assert!(text.contains("v7       flags  1 of  2 failures ( 50%) ·  0 of  1 passes (  0%)"));
        assert!(text.contains("behavior.filter-preserves"));
        assert!(text.contains("roy-polymorph-cn"));
        assert!(text.contains("1 trial(s) left out"));
        let failures = lines(&summary(), true).join("\n");
        assert!(!failures.contains("roy-polymorph-cn"));
    }

    #[test]
    fn the_command_reads_the_summary_and_prints_json() {
        let dir = std::env::temp_dir().join(format!("gym-recall-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("summary.json"), summary().to_string()).unwrap();
        let mut out = Vec::new();
        let code = command(
            &[
                "--dir".to_string(),
                dir.to_string_lossy().into_owned(),
                "--json".to_string(),
                "--failures".to_string(),
            ],
            &mut out,
        )
        .unwrap();
        assert_eq!(code, 0);
        let value: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(value["schema"], VIEW_SCHEMA);
        assert_eq!(value["summary"]["rows"].as_array().unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(load(Path::new("/nonexistent-recall")).is_err());
    }
}
