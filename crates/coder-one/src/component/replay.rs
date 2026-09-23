//! Replays the briefing packer over retained episodes: the first packer's
//! briefing, rebuilt from its retained text, beside the coverage packer's
//! briefing from the same evidence.
//!
//! Each retained episode that delegated keeps its briefing text, the
//! briefing record's `included` and `omitted` lists, and `state.json`,
//! whose survey holds every surveyed file and probe output as the episode
//! read it. [`super::pack::parse`] reads the briefing back into the
//! packer's inputs; the survey restores the text of each item the briefing
//! left out. The first packer's rebuild must match the retained digest,
//! so the "before" numbers are the retained run's own.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::delegate::{Briefing, BriefingInputs};
use crate::pack::{Measure, Params, delivered_by_pack, delivered_by_sections, measure, pack};

/// The schema of a replay report.
pub const SCHEMA: &str = "openagents.coder-one.pack-replay.v1";

/// One retained episode's replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Replayed {
    /// The trace directory and the episode directory under it.
    pub trace: String,
    pub episode: String,
    pub task: String,
    /// Whether the first packer rebuilt the retained briefing exactly.
    pub reproduces: bool,
    /// Omitted items whose text the survey restored, and those it could
    /// not.
    pub restored: usize,
    pub unrestored: usize,
    pub before: Measure,
    pub after: Measure,
    /// Requirements no delivered item informs, after.
    pub uncovered_after: usize,
    pub requirements: usize,
    /// Requirements that name an exact path or constant, which the
    /// deterministic packer can tie evidence to, and how many of those no
    /// delivered item informs.
    pub keyed: usize,
    pub keyed_uncovered: usize,
}

/// The whole replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub schema: String,
    pub implementation: crate::record::Implementation,
    pub manifests: usize,
    /// Episodes that delegated and kept a briefing the first packer built.
    pub briefings: usize,
    /// Episodes whose briefing the coverage packer built, which the replay
    /// leaves out.
    #[serde(default)]
    pub coverage_packed: usize,
    /// Briefings that could not be read back, with why.
    pub skipped: Vec<(String, String)>,
    pub replayed: Vec<Replayed>,
    pub totals: Value,
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Whether the episode kept a briefing the first packer built.
///
/// An episode whose policy chose the coverage packer attaches
/// `artifacts/briefing-pack.json`, and its briefing has the coverage
/// packer's shape, which [`super::pack::parse`] doesn't read. Only the
/// first packer's briefings replay.
pub(crate) fn first_packer_briefing(dir: &Path, manifest: &Value) -> bool {
    manifest.pointer("/delegate/delegation/briefing").is_some()
        && !dir.join("artifacts/briefing-pack.json").exists()
        && manifest
            .pointer("/policy/manifest/policy/brief/packer")
            .and_then(Value::as_str)
            .is_none_or(|packer| packer == "sections")
}

/// Every episode directory under `traces` that holds a manifest.
pub(crate) fn episodes(traces: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![(traces.to_path_buf(), 0)];
    while let Some((dir, depth)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if path.join("manifest.json").is_file() && path.join("artifacts").is_dir() {
                out.push(path);
            } else if depth < 3 {
                pending.push((path, depth + 1));
            }
        }
    }
    out.sort();
    out
}

/// Restores the text the retained briefing left out, and the whole text of
/// every surveyed item, from the episode's survey. Returns the inputs the
/// first packer had (omitted items clipped as it clipped them) and the
/// inputs with each surveyed item whole, plus how many omitted items were
/// restored and how many were not.
pub(crate) fn restore(
    parsed: &BriefingInputs,
    omitted: &[String],
    state: &Value,
) -> (BriefingInputs, BriefingInputs, usize, usize) {
    let survey: Vec<(String, f64, f64, String)> = state["survey"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            Some((
                s["path"].as_str()?.to_string(),
                s["relevance"].as_f64().unwrap_or(0.0),
                s["edit"].as_f64().unwrap_or(0.0),
                s["content"].as_str()?.to_string(),
            ))
        })
        .collect();
    let omitted_paths: Vec<&str> = omitted
        .iter()
        .filter_map(|item| {
            item.strip_prefix("file ")?
                .rsplit_once(" (")
                .map(|(p, _)| p)
        })
        .collect();
    let mut first = parsed.clone();
    let mut whole = parsed.clone();
    let mut restored = 0;
    let mut unrestored = 0;
    for (index, (path, ..)) in parsed.files.iter().enumerate() {
        let found = survey.iter().find(|(p, ..)| p == path);
        if let Some((_, relevance, edit, content)) = found {
            whole.files[index].2.clone_from(content);
            if omitted_paths.contains(&path.as_str()) {
                // The briefing kept only an omitted item's name and size;
                // the survey holds its relevance and text.
                let cap = if *edit >= 0.8 { 16_000 } else { 4_000 };
                first.files[index].1 = Some(*relevance);
                first.files[index].2 = crate::judge::clip(content, cap);
                whole.files[index].1 = Some(*relevance);
                restored += 1;
            }
        } else if omitted_paths.contains(&path.as_str()) {
            unrestored += 1;
        }
    }
    (first, whole, restored, unrestored)
}

/// Replays one episode directory.
///
/// # Errors
///
/// Returns why the episode has no replayable briefing.
pub fn replay_episode(dir: &Path, params: Params) -> Result<Replayed, String> {
    let manifest = read_json(&dir.join("manifest.json")).ok_or("no manifest")?;
    let record = manifest
        .pointer("/delegate/delegation/briefing")
        .ok_or("no delegation")?;
    let strings = |key: &str| -> Vec<String> {
        record[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    };
    let included = strings("included");
    let omitted = strings("omitted");
    let cap = record["cap"].as_u64().ok_or("no cap")? as usize;
    let text = std::fs::read_to_string(dir.join("artifacts/delegate-1.briefing.md"))
        .map_err(|_| "no retained briefing text")?;
    let state = read_json(&dir.join("artifacts/state.json")).ok_or("no state.json")?;
    let parsed = super::pack::parse(&text, &included, &omitted)?;
    let rebuilt = Briefing::build(&parsed, cap);
    let reproduces = record["sha256"].as_str() == Some(rebuilt.sha256().as_str());
    let (first, whole, restored, unrestored) = restore(&parsed, &omitted, &state);
    let before = measure(&rebuilt, &delivered_by_sections(&first, &rebuilt));
    let map = crate::requirements::mechanical(&whole.instruction);
    let packed = pack(&whole, &map, None, Params { cap, ..params });
    let after = measure(&packed.briefing, &delivered_by_pack(&whole, &packed));
    let keyed: Vec<String> = map
        .requirements
        .iter()
        .filter(|r| r.kind != crate::requirements::Kind::Context)
        .filter(|r| !r.extracted.paths.is_empty() || !r.extracted.constants.is_empty())
        .map(|r| r.id.clone())
        .collect();
    let trace = dir
        .parent()
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Replayed {
        task: state
            .pointer("/issue/title")
            .and_then(Value::as_str)
            .map(|_| trace.rsplit("--").next().unwrap_or("").to_string())
            .unwrap_or_default(),
        trace,
        episode: dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        reproduces,
        restored,
        unrestored,
        before,
        after,
        uncovered_after: packed.record.uncovered.len(),
        requirements: packed.record.bytes_per_requirement.len(),
        keyed: keyed.len(),
        keyed_uncovered: keyed
            .iter()
            .filter(|id| packed.record.uncovered.contains(id))
            .count(),
    })
}

/// Replays every retained episode under `traces`.
#[must_use]
pub fn replay_tree(traces: &Path, params: Params) -> Report {
    let dirs = episodes(traces);
    let mut replayed = Vec::new();
    let mut skipped = Vec::new();
    let mut briefings = 0;
    let mut coverage_packed = 0;
    for dir in &dirs {
        let Some(manifest) = read_json(&dir.join("manifest.json")) else {
            continue;
        };
        if manifest.pointer("/delegate/delegation/briefing").is_none() {
            continue;
        }
        if !first_packer_briefing(dir, &manifest) {
            coverage_packed += 1;
            continue;
        }
        briefings += 1;
        match replay_episode(dir, params) {
            Ok(one) => replayed.push(one),
            Err(error) => skipped.push((dir.display().to_string(), error)),
        }
    }
    let sum = |f: &dyn Fn(&Measure) -> usize, after: bool| -> usize {
        replayed
            .iter()
            .map(|r| f(if after { &r.after } else { &r.before }))
            .sum()
    };
    let count = |f: &dyn Fn(&Measure) -> bool, after: bool| -> usize {
        replayed
            .iter()
            .filter(|r| f(if after { &r.after } else { &r.before }))
            .count()
    };
    let side = |after: bool| {
        json!({
            "selected": sum(&|m| m.selected, after),
            "selected_delivered": sum(&|m| m.selected_delivered, after),
            "selected_dropped": sum(&|m| m.selected_dropped, after),
            "selected_over_cap_dropped": sum(&|m| m.selected_over_cap, after),
            "omitted_items": sum(&|m| m.omitted, after),
            "duplicate_bytes": sum(&|m| m.duplicate_bytes, after),
            "data_chars": sum(&|m| m.data_chars, after),
            "chars": sum(&|m| m.chars, after),
            "over_cap": count(&|m| m.chars > m.cap, after),
            "episodes_dropping_selected_while_duplicates_kept": count(&|m| m.dropped_while_duplicates_kept, after),
            "episodes_dropping_selected": count(&|m| m.selected_dropped > 0, after),
        })
    };
    let totals = json!({
        "replayed": replayed.len(),
        "reproduces": replayed.iter().filter(|r| r.reproduces).count(),
        "restored_omissions": replayed.iter().map(|r| r.restored).sum::<usize>(),
        "unrestored_omissions": replayed.iter().map(|r| r.unrestored).sum::<usize>(),
        "before": side(false),
        "after": side(true),
        "uncovered_requirements_after": replayed.iter().map(|r| r.uncovered_after).sum::<usize>(),
        "requirements": replayed.iter().map(|r| r.requirements).sum::<usize>(),
        "keyed_requirements": replayed.iter().map(|r| r.keyed).sum::<usize>(),
        "keyed_uncovered_after": replayed.iter().map(|r| r.keyed_uncovered).sum::<usize>(),
    });
    Report {
        schema: SCHEMA.to_string(),
        implementation: crate::pack::implementation(params, false),
        manifests: dirs.len(),
        briefings,
        coverage_packed,
        skipped,
        replayed,
        totals,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn traces() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces")
    }

    /// The acceptance replay: on every retained briefing, no Jev-selected
    /// evidence is dropped while duplicate listings are kept, and nothing
    /// delivered exceeds the cap.
    #[test]
    fn every_retained_briefing_replays_without_dropping_selected_evidence() {
        let report = replay_tree(&traces(), Params::default());
        assert!(report.briefings >= 200, "{}", report.briefings);
        // The coverage packer's briefings (the pack and Luna-first tunable
        // arms) aren't the first packer's, so they stay out.
        assert!(report.coverage_packed > 0);
        assert!(
            report
                .replayed
                .iter()
                .all(|r| !r.trace.contains("-pack") && !r.trace.contains("-luna-v2")),
            "a coverage-packed briefing replayed"
        );
        // One early smoke briefing predates the section format the reader
        // parses, and the report names it.
        assert!(report.skipped.len() <= 1, "{:?}", report.skipped);
        assert_eq!(report.totals["reproduces"], json!(report.replayed.len()));
        for one in &report.replayed {
            assert!(!one.after.dropped_while_duplicates_kept, "{}", one.episode);
            assert!(one.after.chars <= one.after.cap, "{}", one.episode);
            assert_eq!(one.after.selected_over_cap, 0, "{}", one.episode);
        }
        // The v3 log-summary briefings lost their log excerpts before.
        let logs: Vec<&Replayed> = report
            .replayed
            .iter()
            .filter(|r| r.trace.contains("jevprobe3-luna--log-summary"))
            .collect();
        assert_eq!(logs.len(), 3);
        for one in logs {
            assert!(one.before.selected_dropped > 0);
            assert_eq!(one.after.selected_dropped, 0);
            assert!(one.after.data_chars > 0);
        }
    }

    /// Writes episodes in the shapes newer retained traces take, none of
    /// them a first-packer briefing the replays can read: a TB4 job with
    /// only a live ATIF log, a composition that never delegated, a
    /// coverage-packed briefing, an unreadable manifest, a raw ATIF
    /// trajectory, a retention record, and a stream of lines that aren't
    /// JSON.
    pub(crate) fn odd_tree(root: &Path) {
        let write = |path: &str, text: &str| {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write(
            "tb4--coder-one-tunable-v4--task/task__A/agent/live/episode.atif.jsonl",
            "{\"schema_version\":\"ATIF-v1.7\"}\n",
        );
        write("tb4--coder-one-tunable-v4--task/result.json", "{}");
        write(
            "panel--coder-one-tunable--t/t__B.episode/manifest.json",
            r#"{"contract":"x","composition":{"steps":[]}}"#,
        );
        write("panel--coder-one-tunable--t/t__B.episode/artifacts/x", "");
        write(
            "panel--coder-one-tunable--t/t__B.episode/retention.json",
            r#"{"kept":["manifest.json"]}"#,
        );
        write(
            "panel--coder-one-tunable--t/t__B.episode/trajectory.atif.json",
            r#"{"schema_version":"ATIF-v1.7","steps":[]}"#,
        );
        write(
            "extended--coder-one-tunable-luna-pack--t/t__C.episode/manifest.json",
            r#"{"policy":{"manifest":{"policy":{"brief":{"packer":"coverage-jev"}}}},
               "delegate":{"model":"luna","delegation":{"briefing":{"sha256":"0","cap":12000}}}}"#,
        );
        write(
            "extended--coder-one-tunable-luna-pack--t/t__C.episode/artifacts/briefing-pack.json",
            "{}",
        );
        write(
            "extended--coder-one-tunable-luna-pack--t/t__C.episode/artifacts/delegate-1.briefing.md",
            "## Evidence\n",
        );
        write(
            "extended--coder-one-tunable-luna-pack--t/t__C.episode/artifacts/delegate-1.stream.jsonl",
            "not json\n{\"type\":\"unknown\"}\n",
        );
        write(
            "extended--coder-one-tunable-luna-pack--t/t__C.episode/artifacts/state.json",
            r#"{"issue":{"title":"t","body":"Do t."},"survey":[{"path":7}]}"#,
        );
        write("smoke--x--t/t__D.episode/manifest.json", "not json");
        write(
            "smoke--x--t/t__D.episode/artifacts/delegate-1.stream.jsonl",
            "",
        );
    }

    #[test]
    fn newer_trace_shapes_replay_without_crashing() {
        let dir = tempfile::tempdir().unwrap();
        odd_tree(dir.path());
        let report = replay_tree(dir.path(), Params::default());
        assert_eq!(report.briefings, 0);
        assert_eq!(report.coverage_packed, 1);
        assert!(report.replayed.is_empty());
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    }
}
