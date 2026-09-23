//! Coder One's `verify.repair`: the conditional recovery study.
//!
//! `coder-one repair study` writes `study.json`
//! (`openagents.coder-one.repair-study.v1`) under
//! `~/.openagents/coder-one/repair/study-<ms>/`: each preserved candidate
//! and, per repair arm, the failures it recovered, the passing candidates
//! it damaged, and every dispatch's cost. The Components view shows the
//! latest study.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The schema a study carries.
pub const SCHEMA: &str = "openagents.coder-one.repair-study.v1";

/// Where studies are written by default.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/repair"))
}

/// The newest study under `dir`, by its directory name, with its path.
#[must_use]
pub fn latest(dir: &Path) -> Option<(PathBuf, Value)> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path().join("study.json"))
        .filter(|path| path.is_file())
        .collect();
    found.sort();
    found.into_iter().rev().find_map(|path| {
        let value: Value = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
        (value["schema"] == SCHEMA).then_some((path, value))
    })
}

fn money(value: &Value) -> String {
    value
        .as_f64()
        .map_or("unknown".to_owned(), |usd| format!("${usd:.4}"))
}

/// The study as text rows: one per arm.
#[must_use]
pub fn lines(path: &Path, study: &Value) -> Vec<String> {
    let mut lines = vec![format!(
        "verify.repair · conditional recovery study · trigger {} · same {} · other {} · {} candidates · {}",
        study["trigger"].as_str().unwrap_or("?"),
        study["same"].as_str().unwrap_or("?"),
        study["other"].as_str().unwrap_or("?"),
        study["candidates"].as_array().map_or(0, Vec::len),
        path.display()
    )];
    lines.push(format!(
        "  {:<13} {:<30} {:>20} {:>18} {:>8} {:>10} {:>12}",
        "arm",
        "brief · profile",
        "recovered failures",
        "damaged passes",
        "repairs",
        "cost",
        "per repair"
    ));
    for arm in study["arms"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  {:<13} {:<30} {:>20} {:>18} {:>8} {:>10} {:>12}",
            arm["arm"].as_str().unwrap_or_default(),
            format!(
                "{} · {}",
                arm["brief"].as_str().unwrap_or("—"),
                arm["profile"].as_str().unwrap_or("—")
            ),
            format!("{} of {}", arm["recovered"], arm["failed_candidates"]),
            format!("{} of {}", arm["damaged"], arm["passing_candidates"]),
            arm["repairs_run"].to_string(),
            money(&arm["cost_usd"]),
            if arm["cost_per_repair_usd"].is_null() {
                "—".to_owned()
            } else {
                money(&arm["cost_per_repair_usd"])
            },
        ));
    }
    lines
}

/// The study as the Components view's JSON: the arms and where it came
/// from.
#[must_use]
pub fn to_json(path: &Path, study: &Value) -> Value {
    json!({
        "path": path.display().to_string(),
        "trigger": study["trigger"],
        "same": study["same"],
        "other": study["other"],
        "candidates": study["candidates"],
        "arms": study["arms"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_latest_study_reads_and_renders_each_arm() {
        let dir = std::env::temp_dir().join(format!("gym-repair-{}", std::process::id()));
        for (name, recovered) in [("study-1", 1), ("study-2", 3)] {
            let sub = dir.join(name);
            std::fs::create_dir_all(&sub).unwrap();
            std::fs::write(
                sub.join("study.json"),
                json!({
                    "schema": SCHEMA, "trigger": "detected", "same": "scripted:fix-if-packet", "other": "scripted:fix",
                    "candidates": [{}, {}],
                    "arms": [{ "arm": "packet-same", "brief": "packet", "profile": "scripted:fix-if-packet",
                               "failed_candidates": 4, "recovered": recovered, "passing_candidates": 4, "damaged": 0,
                               "repairs_run": 3, "cost_usd": 0.0, "cost_per_repair_usd": 0.0 }],
                })
                .to_string(),
            )
            .unwrap();
        }
        let (path, study) = latest(&dir).unwrap();
        assert!(path.ends_with("study-2/study.json"));
        let text = lines(&path, &study).join("\n");
        assert!(text.contains("3 of 4"), "{text}");
        assert!(text.contains("0 of 4"), "{text}");
        assert!(text.contains("$0.0000"), "{text}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
