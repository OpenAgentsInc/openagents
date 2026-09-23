//! Coder One's hill-climbing studies, as the Gym reads them.
//!
//! `coder-one study run` writes each study under
//! `~/.openagents/coder-one/studies/<id>/`: the frozen plan (`study.json`),
//! every proposal and candidate (`candidates.jsonl`), every trial
//! (`trials.jsonl`), the selection it committed before reading held-out
//! evidence (`selection.json`), and the result (`result.json`). A study can
//! be retained in the checkout under `bench/terminal-bench/studies/`
//! without its trials. This view reads the results: candidates by tier,
//! development and held-out results, frontier membership, promotions, and
//! the study's whole spend.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The schema of a study result.
pub const RESULT_SCHEMA: &str = "openagents.coder-one.study-result.v1";
/// The schema of `gym coder study --json`.
pub const SCHEMA: &str = "openagents.gym.coder-study.v1";

/// One study's result, and where it was read.
#[derive(Clone, Debug, PartialEq)]
pub struct Study {
    pub dir: PathBuf,
    /// `local` or `retained`.
    pub source: String,
    pub result: Value,
}

impl Study {
    #[must_use]
    pub fn id(&self) -> &str {
        self.result["study"].as_str().unwrap_or("?")
    }
}

/// The local study directory and the checkout's retained one.
#[must_use]
pub fn default_dirs() -> Vec<(PathBuf, &'static str)> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push((
            PathBuf::from(home).join(".openagents/coder-one/studies"),
            "local",
        ));
    }
    dirs.push((
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/studies"),
        "retained",
    ));
    dirs
}

/// Every study result under `dirs`, one per study id, local first.
#[must_use]
pub fn load(dirs: &[(PathBuf, &str)]) -> (Vec<Study>, Vec<String>) {
    let mut studies: Vec<Study> = Vec::new();
    let mut errors = Vec::new();
    for (dir, source) in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            let file = path.join("result.json");
            if !file.is_file() {
                continue;
            }
            let value = std::fs::read_to_string(&file)
                .map_err(|error| error.to_string())
                .and_then(|text| {
                    serde_json::from_str::<Value>(&text).map_err(|error| error.to_string())
                });
            match value {
                Ok(result) if result["schema"] == RESULT_SCHEMA => {
                    let study = Study {
                        dir: path.clone(),
                        source: (*source).to_string(),
                        result,
                    };
                    if !studies.iter().any(|s| s.id() == study.id()) {
                        studies.push(study);
                    }
                }
                Ok(_) => errors.push(format!("{}: not a study result", file.display())),
                Err(error) => errors.push(format!("{}: {error}", file.display())),
            }
        }
    }
    (studies, errors)
}

fn short(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| "none".to_string(), |d| d.chars().take(12).collect())
}

fn number(value: &Value, digits: usize) -> String {
    value
        .as_f64()
        .map_or_else(|| "—".to_string(), |v| format!("{v:.digits$}"))
}

/// A candidate's tier: how far successive halving took it.
fn tier_of(candidate: &Value) -> &'static str {
    if candidate["selected"] == true {
        "selected"
    } else if candidate["mini"]["passed"] == true {
        "mini ✓"
    } else if !candidate["mini"].is_null() {
        "mini ✗"
    } else if candidate["promoted"] == true {
        "promoted"
    } else if candidate["survived_search"] == true {
        "selection"
    } else {
        "search"
    }
}

/// The candidates in rank order: by selection J, then search J.
fn ranked(result: &Value) -> Vec<&Value> {
    let mut candidates: Vec<&Value> = result["candidates"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    let key = |c: &Value| {
        (
            c.pointer("/selection/j")
                .and_then(Value::as_f64)
                .unwrap_or(f64::NEG_INFINITY),
            c.pointer("/search/j")
                .and_then(Value::as_f64)
                .unwrap_or(f64::NEG_INFINITY),
        )
    };
    candidates.sort_by(|a, b| {
        let (a1, a2) = key(a);
        let (b1, b2) = key(b);
        b1.total_cmp(&a1).then(b2.total_cmp(&a2))
    });
    candidates
}

/// One study's lines: the plan, the operators, the rungs, the candidates
/// by tier, confirmation, the higher tiers, and spend. With `all`, every
/// candidate; otherwise the best 12 plus the baseline.
#[must_use]
pub fn lines(study: &Study, all: bool) -> Vec<String> {
    let r = &study.result;
    let mut out = vec![
        format!(
            "study {} · {} · {} · {} ({})",
            study.id(),
            r["component"].as_str().unwrap_or("?"),
            r["status"].as_str().unwrap_or("?"),
            study.source,
            study.dir.display()
        ),
        format!("objective: {}", r["objective"].as_str().unwrap_or("?")),
    ];
    let split = &r["split"];
    let names = |v: &Value| {
        v.as_array()
            .map(|t| {
                t.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default()
    };
    out.push(format!(
        "development: {} — search {} · selection {} briefings",
        names(&split["development_tasks"]),
        split["search"],
        split["selection"]
    ));
    out.push(format!(
        "held out: {} — {} briefings, read once after the selection was committed",
        names(&split["held_out_tasks"]),
        split["confirmation"]
    ));
    out.push(String::new());
    out.push(
        "operators          proposals  built  dup  refused  best search J  best selection J"
            .to_string(),
    );
    for op in r["operators"].as_array().into_iter().flatten() {
        out.push(format!(
            "  {:<16} {:>9}  {:>5}  {:>3}  {:>7}  {:>13}  {:>16}",
            op["operator"].as_str().unwrap_or("?"),
            op["proposals"].to_string(),
            op["built"].to_string(),
            op["duplicates"].to_string(),
            op["refused"].to_string(),
            number(&op["best_search"]["j"], 4),
            number(&op["best_selection"]["j"], 4),
        ));
        if let Some(algorithm) = op["algorithm"].as_str() {
            out.push(format!("    {algorithm}"));
        }
    }
    out.push(String::new());
    out.push("rungs (successive halving)".to_string());
    for rung in r["rungs"].as_array().into_iter().flatten() {
        out.push(format!(
            "  {}/{}: {} evaluated → {} kept{}",
            rung["tier"].as_str().unwrap_or("?"),
            rung["phase"].as_str().unwrap_or("?"),
            rung["evaluated"],
            if rung["kept"].is_null() {
                "—".to_string()
            } else {
                rung["kept"].to_string()
            },
            if rung["run"] == false {
                " (not run)"
            } else {
                ""
            },
        ));
    }
    let frontier = r["frontier"].as_object();
    out.push(String::new());
    out.push(format!(
        "candidates by tier ({} built; ★ baseline, ◆ on a task frontier: count of tasks)",
        r["candidates"].as_array().map_or(0, Vec::len)
    ));
    out.push(
        "  tier       id            operator  search J  select J   chars  label cov  frontier  changes"
            .to_string(),
    );
    let ranked = ranked(r);
    let shown = if all { ranked.len() } else { 12 };
    for (i, c) in ranked.iter().enumerate() {
        let baseline = c["baseline"] == true;
        if i >= shown && !baseline {
            continue;
        }
        let tasks = c["frontier_tasks"].as_array().map_or(0, Vec::len);
        out.push(format!(
            "  {:<9}  {:<12}{} {:<8}  {:>8}  {:>8}  {:>6}  {:>9}  {:>8}  {}",
            tier_of(c),
            short(&c["id"]),
            if baseline { "★" } else { " " },
            c["operator"].as_str().unwrap_or("?"),
            number(&c["search"]["j"], 4),
            number(&c["selection"]["j"], 4),
            number(&c["search"]["chars_mean"], 0),
            number(&c["search"]["label_coverage"], 3),
            if tasks > 0 {
                format!("◆ {tasks}")
            } else {
                String::new()
            },
            c["label"].as_str().unwrap_or(""),
        ));
    }
    if !all && ranked.len() > shown {
        out.push(format!(
            "  … {} more; gym coder study {} --all shows every candidate",
            ranked.len() - shown,
            study.id()
        ));
    }
    if let Some(frontier) = frontier {
        out.push(String::new());
        out.push("per-task frontier (replay quality against briefing characters)".to_string());
        for (task, ids) in frontier {
            let ids = ids.as_array().map_or(0, Vec::len);
            out.push(format!("  {task}: {ids} on the frontier"));
        }
    }
    out.push(String::new());
    out.push(format!(
        "baseline {} · selected {}",
        short(&r["baseline"]),
        short(&r["selected"])
    ));
    match r.get("confirmation").filter(|c| !c.is_null()) {
        Some(confirmation) => {
            out.push(format!(
                "held-out: selected J {} · baseline J {} · ΔJ {} (95% {}–{}) · chars {} · label coverage {}",
                number(&confirmation["selected_result"]["j"], 4),
                number(&confirmation["baseline_result"]["j"], 4),
                number(&confirmation["paired"]["mean"], 4),
                number(&confirmation["paired"]["low"], 4),
                number(&confirmation["paired"]["high"], 4),
                number(&confirmation["deltas"]["chars_mean"], 1),
                number(&confirmation["deltas"]["label_coverage"], 3),
            ));
            for (task, delta) in confirmation["paired"]["per_task"]
                .as_object()
                .into_iter()
                .flatten()
            {
                out.push(format!("  {task}: ΔJ {}", number(delta, 4)));
            }
            out.push(confirmation["statement"].as_str().unwrap_or("").to_string());
        }
        None => out.push(
            "No candidate beat the baseline on selection, so nothing was confirmed.".to_string(),
        ),
    }
    out.push(String::new());
    for tier in r["higher_tiers"].as_array().into_iter().flatten() {
        out.push(format!(
            "tier {}: {}",
            tier["tier"].as_str().unwrap_or("?"),
            if tier["run"] == true {
                "run".to_string()
            } else {
                format!("not run — {}", tier["why"].as_str().unwrap_or(""))
            }
        ));
    }
    let spend = &r["spend"];
    out.push(format!(
        "spend: ${} · {} model or Jev calls · {} ms of work · accounting {}",
        number(&spend["spend_usd"], 2),
        spend["calls"],
        spend["wall_ms"],
        if spend["complete"] == true {
            "complete"
        } else {
            "incomplete"
        }
    ));
    for entry in r["spend_entries"].as_array().into_iter().flatten() {
        out.push(format!(
            "  {:<8} {:<7} {:>7} ms  ${}  {}",
            entry["category"].as_str().unwrap_or("?"),
            entry["tier"].as_str().unwrap_or("?"),
            entry["wall_ms"].to_string(),
            number(
                &(entry["spend_microusd"].as_f64().map(|v| v / 1e6).into()),
                2
            ),
            entry["note"].as_str().unwrap_or(""),
        ));
    }
    for limitation in r["limitations"].as_array().into_iter().flatten() {
        if let Some(text) = limitation.as_str() {
            out.push(format!("limitation: {text}"));
        }
    }
    out
}

/// The TUI's lines: the newest study's, or a note when there is none.
#[must_use]
pub fn view_lines(studies: &[Study], errors: &[String]) -> Vec<String> {
    let mut out = match studies.last() {
        Some(study) => {
            let mut lines = Vec::new();
            if studies.len() > 1 {
                lines.push(format!(
                    "{} studies; showing the latest. gym coder study lists them all.",
                    studies.len()
                ));
            }
            lines.extend(self::lines(study, false));
            lines
        }
        None => vec![
            "No study recorded. Run: coder-one study run evidence.pack [--through mini]"
                .to_string(),
        ],
    };
    for error in errors {
        out.push(format!("error: {error}"));
    }
    out
}

/// A study as `--json` reports it.
#[must_use]
pub fn study_value(study: &Study) -> Value {
    let r = &study.result;
    let candidates: Vec<Value> = ranked(r)
        .into_iter()
        .map(|c| {
            json!({
                "id": c["id"],
                "label": c["label"],
                "operator": c["operator"],
                "tier": tier_of(c),
                "reached": c["reached"],
                "baseline": c["baseline"],
                "development": { "search": c["search"], "selection": c["selection"] },
                "mini": c["mini"],
                "frontier_tasks": c["frontier_tasks"],
                "promoted": {
                    "search_to_selection": c["survived_search"],
                    "selection_to_mini": c["promoted"],
                    "finalist": c["finalist"],
                    "selected": c["selected"],
                },
                "wall_ms": c["wall_ms"],
                "spend_usd": c["spend_usd"],
            })
        })
        .collect();
    json!({
        "study": study.id(),
        "source": study.source,
        "dir": study.dir.display().to_string(),
        "component": r["component"],
        "status": r["status"],
        "objective": r["objective"],
        "acceptance": r["acceptance"],
        "split": r["split"],
        "baseline": r["baseline"],
        "selected": r["selected"],
        "operators": r["operators"],
        "rungs": r["rungs"],
        "promotions": r["promotions"],
        "frontier": r["frontier"],
        "candidates": candidates,
        "held_out": r["confirmation"],
        "higher_tiers": r["higher_tiers"],
        "spend": r["spend"],
        "spend_entries": r["spend_entries"],
        "limitations": r["limitations"],
    })
}

const HELP: &str = "gym coder study [ID] [--all] [--dir DIR] [--json]

Coder One's hill-climbing studies: candidates by tier, development and
held-out results, frontier membership, promotions, and total spend. Without
an ID it shows the latest study and lists the rest. Studies are read from
~/.openagents/coder-one/studies and bench/terminal-bench/studies, or from
each --dir given. --all lists every candidate.";

/// Runs `gym coder study …`.
///
/// # Errors
///
/// Returns a message for an unknown option or study.
pub fn command(args: &[String], out: &mut impl Write) -> Result<i32, String> {
    let mut dirs: Vec<(PathBuf, &str)> = Vec::new();
    let mut json_output = false;
    let mut all = false;
    let mut query = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--json" => json_output = true,
            "--all" => all = true,
            "--dir" => dirs.push((
                PathBuf::from(iter.next().ok_or("--dir needs a value")?),
                "given",
            )),
            "--help" | "-h" | "help" => {
                writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
                return Ok(0);
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other}\n{HELP}"));
            }
            other => query = Some(other.to_string()),
        }
    }
    if dirs.is_empty() {
        dirs = default_dirs();
    }
    let (studies, errors) = load(&dirs);
    let chosen = match &query {
        Some(q) => Some(
            studies
                .iter()
                .find(|s| s.id() == q || s.id().starts_with(q.as_str()))
                .ok_or_else(|| format!("no study matches {q}"))?,
        ),
        None => studies.last(),
    };
    let write =
        |out: &mut dyn Write, text: &str| writeln!(out, "{text}").map_err(|e| e.to_string());
    if json_output {
        let value = json!({
            "schema": SCHEMA,
            "studies": studies.iter().map(|s| json!({
                "study": s.id(),
                "source": s.source,
                "component": s.result["component"],
                "status": s.result["status"],
                "candidates": s.result["candidates"].as_array().map_or(0, Vec::len),
                "selected": s.result["selected"],
                "beats_baseline": s.result.pointer("/confirmation/beats_baseline"),
                "spend_usd": s.result.pointer("/spend/spend_usd"),
            })).collect::<Vec<_>>(),
            "study": chosen.map(study_value),
            "errors": errors,
        });
        write(
            out,
            &serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?,
        )?;
        return Ok(0);
    }
    if studies.len() > 1 {
        write(out, "studies:")?;
        for s in &studies {
            write(
                out,
                &format!(
                    "  {}  {}  {} candidates  {}",
                    s.id(),
                    s.source,
                    s.result["candidates"].as_array().map_or(0, Vec::len),
                    match s.result.pointer("/confirmation/beats_baseline") {
                        Some(Value::Bool(true)) => "beats the baseline on held-out evidence",
                        Some(Value::Bool(false)) =>
                            "does not beat the baseline on held-out evidence",
                        _ => "nothing confirmed",
                    }
                ),
            )?;
        }
        write(out, "")?;
    }
    match chosen {
        Some(study) => {
            for line in lines(study, all) {
                write(out, &line)?;
            }
        }
        None => write(out, &view_lines(&[], &errors).join("\n"))?,
    }
    for error in &errors {
        write(out, &format!("error: {error}"))?;
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        json!({
            "schema": RESULT_SCHEMA,
            "study": "evidence-pack-test",
            "component": "evidence.pack",
            "status": "completed",
            "objective": "J",
            "baseline": "b".repeat(64),
            "selected": "a".repeat(64),
            "split": { "development_tasks": ["t1"], "held_out_tasks": ["t2"], "search": 2, "selection": 2, "confirmation": 1 },
            "operators": [{ "operator": "grid", "algorithm": "grid search", "proposals": 2, "built": 2, "duplicates": 0, "refused": 0, "best_search": { "j": 0.6 }, "best_selection": { "j": 0.61 } }],
            "rungs": [{ "tier": "replay", "phase": "search", "evaluated": 2, "kept": 1 }],
            "frontier": { "t1": ["a".repeat(64)] },
            "candidates": [
                { "id": "a".repeat(64), "label": "cap=8000", "operator": "grid", "search": { "j": 0.6, "chars_mean": 5000.0, "label_coverage": 0.2 }, "selection": { "j": 0.61 }, "survived_search": true, "promoted": true, "finalist": true, "selected": true, "baseline": false, "mini": { "passed": true }, "frontier_tasks": ["t1"] },
                { "id": "b".repeat(64), "label": "baseline", "operator": "baseline", "search": { "j": 0.5, "chars_mean": 7000.0, "label_coverage": 0.2 }, "selection": { "j": 0.52 }, "survived_search": false, "promoted": false, "finalist": false, "selected": false, "baseline": true, "mini": null, "frontier_tasks": [] },
            ],
            "confirmation": {
                "selected_result": { "j": 0.47 }, "baseline_result": { "j": 0.47 },
                "paired": { "mean": 0.0001, "low": 0.0, "high": 0.0002, "per_task": { "t2": 0.0001 } },
                "deltas": { "chars_mean": -10.0, "label_coverage": 0.0 },
                "beats_baseline": false,
                "statement": "The selected candidate does not beat the baseline beyond the noise floor on held-out evidence.",
            },
            "higher_tiers": [{ "tier": "screen", "run": false, "why": "gated" }],
            "spend": { "spend_usd": 0.0, "calls": 0, "wall_ms": 1000, "complete": true },
            "spend_entries": [{ "category": "tool", "tier": "replay", "wall_ms": 900, "spend_microusd": 0, "note": "search rung" }],
            "limitations": ["Replay scores packing, not task outcomes."],
        })
    }

    #[test]
    fn the_study_view_shows_tiers_results_frontier_promotions_and_spend() {
        let dir = tempfile::tempdir().unwrap();
        let study_dir = dir.path().join("evidence-pack-test");
        std::fs::create_dir_all(&study_dir).unwrap();
        std::fs::write(study_dir.join("result.json"), fixture().to_string()).unwrap();
        let (studies, errors) = load(&[(dir.path().to_path_buf(), "given")]);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(studies.len(), 1);
        let text = lines(&studies[0], false).join("\n");
        assert!(text.contains("selected   aaaaaaaaaaaa"), "{text}");
        assert!(text.contains("★ baseline"), "{text}");
        assert!(text.contains("◆ 1"), "{text}");
        assert!(text.contains("does not beat the baseline"), "{text}");
        assert!(text.contains("tier screen: not run"), "{text}");
        assert!(text.contains("spend: $0.00"), "{text}");
        let mut out = Vec::new();
        command(
            &[
                "--dir".into(),
                dir.path().display().to_string(),
                "--json".into(),
            ],
            &mut out,
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(value["schema"], SCHEMA);
        assert_eq!(value["study"]["candidates"][0]["tier"], "selected");
        assert_eq!(value["study"]["held_out"]["beats_baseline"], false);
        assert_eq!(value["studies"][0]["spend_usd"], 0.0);
    }

    #[test]
    fn the_retained_first_study_reads_back() {
        let (studies, _) = load(&[(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/studies"),
            "retained",
        )]);
        let study = studies
            .iter()
            .find(|s| s.result["component"] == "evidence.pack")
            .expect("the retained evidence.pack study");
        assert!(study.result["candidates"].as_array().unwrap().len() > 200);
        assert!(study.result["confirmation"]["statement"].is_string());
        let text = lines(study, false).join("\n");
        assert!(text.contains("held out:"), "{text}");
    }
}
