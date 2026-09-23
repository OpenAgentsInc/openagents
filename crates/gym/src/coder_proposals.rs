//! `gym coder proposals`: the changes `coder-one ask` proposed, a person's
//! decision on each, and what measuring it showed.
//!
//! An ask's answer may carry typed proposals: a policy or check merge
//! patch on a checked-in manifest, a question-set change, a new mini-task,
//! or a code change. `coder-one ask` validates each and writes it under
//! `~/.openagents/coder-one/proposals/<id>/proposal.json`
//! (`openagents.coder-one.proposal.v1`). This view lists them, shows one
//! as finding → change → result, and records a person's decision:
//!
//! ```text
//! gym coder proposals [ID|latest] [--json]
//! gym coder proposals approve ID [--note TEXT]
//! gym coder proposals reject ID [--note TEXT]
//! ```
//!
//! A decision (`openagents.coder-one.proposal-decision.v1`) names the
//! digest of the proposal it decided, so `coder-one proposal run` refuses a
//! proposal edited after its approval. Nothing runs without one. The result
//! (`openagents.coder-one.proposal-result.v1`) holds the mini-task stage
//! and, when it started, the live stage's experiment, which this view
//! reads from `tbench experiment`'s status file.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::coder_policy::canonical;
use crate::runs::clip_words;
use crate::terminal_bench_experiment::{self as experiment, Report};

/// A proposal's schema.
pub const PROPOSAL_SCHEMA: &str = "openagents.coder-one.proposal.v1";
/// A decision's schema.
pub const DECISION_SCHEMA: &str = "openagents.coder-one.proposal-decision.v1";
/// This view's JSON schema.
pub const SCHEMA: &str = "openagents.gym.coder-proposals.v1";

const HELP: &str = "\
gym coder proposals [ID|latest] [--dir PATH] [--experiments-dir PATH] [--json]
gym coder proposals approve ID [--note TEXT] [--dir PATH]
gym coder proposals reject ID [--note TEXT] [--dir PATH]

The changes `coder-one ask` proposed, newest first: the ID, the kind, the
status (refused, proposed, approved, rejected, measured, or live), the ask it
came from, and the title. ID (or a prefix, or `latest`) shows one proposal as
finding → change → result: the ask's question and the runs it rests on, the
change and its digests, the decision, the mini-task stage, and the live
experiment's passes per arm.

approve and reject record a person's decision, with an optional note, bound
to the proposal's digest. Only an approved proposal runs, with `coder-one
proposal run ID`. A refused proposal can't be approved, and a decision can't
change once a stage ran. Proposals are read from
~/.openagents/coder-one/proposals unless --dir names another directory.";

/// Where proposals are kept.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/proposals"))
}

/// A proposal's digest: SHA-256 of its canonical JSON without `digest`.
#[must_use]
pub fn digest(proposal: &Value) -> String {
    let mut value = proposal.clone();
    if let Some(object) = value.as_object_mut() {
        object.remove("digest");
    }
    Sha256::digest(canonical(&value).as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// One proposal with its decision and result, as read from its directory.
#[derive(Clone, Debug)]
pub struct Entry {
    pub dir: PathBuf,
    pub proposal: Value,
    pub decision: Option<Value>,
    pub result: Option<Value>,
}

fn read(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

impl Entry {
    fn load(dir: &Path) -> Option<Self> {
        let proposal = read(&dir.join("proposal.json"))?;
        (proposal["schema"] == PROPOSAL_SCHEMA).then(|| Entry {
            dir: dir.to_path_buf(),
            decision: read(&dir.join("decision.json")),
            result: read(&dir.join("result.json")),
            proposal,
        })
    }

    #[must_use]
    pub fn id(&self) -> &str {
        self.proposal["id"].as_str().unwrap_or("?")
    }

    /// `refused`, `proposed`, `approved`, `rejected`, `measured` (the mini
    /// stage ran), or `live` (the experiment started).
    #[must_use]
    pub fn status(&self) -> &'static str {
        if self.proposal["valid"] != true {
            return "refused";
        }
        let verdict = self
            .decision
            .as_ref()
            .filter(|d| d["proposal_digest"] == self.proposal["digest"])
            .and_then(|d| d["verdict"].as_str());
        match verdict {
            Some("rejected") => "rejected",
            Some("approved") => match &self.result {
                Some(result)
                    if !result["live"].is_null() && result["live"]["plan_only"] != true =>
                {
                    "live"
                }
                Some(_) => "measured",
                None => "approved",
            },
            _ => "proposed",
        }
    }
}

/// Every proposal under `dir`, newest first.
#[must_use]
pub fn load(dir: &Path) -> Vec<Entry> {
    let mut entries: Vec<Entry> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| Entry::load(&entry.path()))
        .collect();
    entries.sort_by(|a, b| {
        b.proposal["created_at"]
            .as_str()
            .cmp(&a.proposal["created_at"].as_str())
            .then_with(|| b.id().cmp(a.id()))
    });
    entries
}

/// The proposal an ID, a prefix, or `latest` names.
///
/// # Errors
///
/// Returns a message when none, or more than one, matches.
pub fn find(entries: &[Entry], query: &str) -> Result<usize, String> {
    if query == "latest" {
        return if entries.is_empty() {
            Err("there are no proposals".to_owned())
        } else {
            Ok(0)
        };
    }
    if let Some(index) = entries.iter().position(|e| e.id() == query) {
        return Ok(index);
    }
    let found: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.id().starts_with(query))
        .map(|(index, _)| index)
        .collect();
    match found.as_slice() {
        [one] => Ok(*one),
        [] => Err(format!("no proposal {query}")),
        many => Err(format!(
            "{} proposals start with {query}; give more of the ID",
            many.len()
        )),
    }
}

/// Records a person's decision on the proposal `query` names under `dir`.
///
/// # Errors
///
/// Returns a message when the proposal isn't found, is refused and would
/// be approved, changed since it was written, or already ran a stage.
pub fn decide(
    dir: &Path,
    query: &str,
    verdict: &str,
    note: &str,
    by: &str,
) -> Result<Value, String> {
    if !matches!(verdict, "approved" | "rejected") {
        return Err(format!("a decision is approved or rejected, not {verdict}"));
    }
    let entries = load(dir);
    let entry = &entries[find(&entries, query)?];
    let sealed = entry.proposal["digest"].as_str().unwrap_or_default();
    if digest(&entry.proposal) != sealed {
        return Err(format!(
            "{} changed after it was written; its digest doesn't match, so it can't be decided",
            entry.id()
        ));
    }
    if verdict == "approved" && entry.proposal["valid"] != true {
        return Err(format!(
            "{} didn't validate ({}), so it can't be approved",
            entry.id(),
            entry.proposal["problems"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    if entry.result.is_some() {
        return Err(format!(
            "{} already ran its mini stage, so its decision stands",
            entry.id()
        ));
    }
    let decision = json!({
        "schema": DECISION_SCHEMA,
        "proposal": entry.id(),
        "proposal_digest": sealed,
        "verdict": verdict,
        "note": note,
        "by": by,
        "at": iso_now(),
    });
    let path = entry.dir.join("decision.json");
    let partial = entry.dir.join("decision.json.partial");
    std::fs::write(
        &partial,
        serde_json::to_string_pretty(&decision).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("cannot write {}: {error}", partial.display()))?;
    std::fs::rename(&partial, &path)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    Ok(decision)
}

/// Who decides: `$USER`, or `operator`.
#[must_use]
pub fn who() -> String {
    std::env::var("USER")
        .ok()
        .filter(|user| !user.trim().is_empty())
        .unwrap_or_else(|| "operator".to_owned())
}

fn iso_now() -> String {
    let ms = crate::runs::now_ms();
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// The live stage's experiment report, when its status file exists.
#[must_use]
pub fn live_report(entry: &Entry, experiments: &Path) -> Option<Report> {
    let id = entry.result.as_ref()?["live"]["experiment"].as_str()?;
    Report::read(&experiment::status_path(id, experiments)).ok()
}

fn live_json(report: &Report) -> Value {
    json!({
        "state": report.state,
        "quota_used_usd": report.quota_used_usd,
        "budget_usd": report.budget_usd,
        "arms": report.summaries.iter().map(|s| json!({
            "arm": s.arm, "scheduled": s.scheduled, "graded": s.graded,
            "passes": s.passes, "interval": [s.interval.0, s.interval.1],
            "not_run": s.not_run, "quota_usd": s.quota_usd,
        })).collect::<Vec<_>>(),
        "paired": report.paired.iter().map(|p| json!({
            "baseline": p.baseline, "arm": p.arm, "pairs": p.pairs,
            "arm_only": p.arm_only, "baseline_only": p.baseline_only,
            "mcnemar_p": p.mcnemar_p,
        })).collect::<Vec<_>>(),
    })
}

/// One proposal as the JSON carries it.
#[must_use]
pub fn summary(entry: &Entry, experiments: &Path) -> Value {
    let p = &entry.proposal;
    json!({
        "id": entry.id(),
        "status": entry.status(),
        "kind": p["kind"],
        "title": p["title"],
        "ask": p["ask"],
        "source_runs": p["source_runs"],
        "expected_tasks": p["expected_tasks"],
        "valid": p["valid"],
        "problems": p["problems"],
        "needs_code": p["needs_code"],
        "digest": p["digest"],
        "change": p["change"],
        "materialized": p["materialized"],
        "issue_draft": p["issue_draft"],
        "decision": entry.decision,
        "mini": entry.result.as_ref().map(|r| r["mini"].clone()),
        "live": entry.result.as_ref().map(|r| r["live"].clone()),
        "live_report": live_report(entry, experiments).map(|r| live_json(&r)),
        "dir": entry.dir.display().to_string(),
    })
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

fn list_lines(entries: &[Entry]) -> Vec<String> {
    let mut lines = vec![
        format!("Coder One proposals, newest first: {}", entries.len()),
        String::new(),
    ];
    for entry in entries {
        lines.push(format!(
            "{:<26} {:<9} {:<9} {:<19} {}",
            entry.id(),
            entry.proposal["kind"].as_str().unwrap_or("?"),
            entry.status(),
            entry.proposal["ask"]["id"].as_str().unwrap_or("?"),
            clip_words(entry.proposal["title"].as_str().unwrap_or(""), 70)
        ));
    }
    lines.push(String::new());
    lines.push("Columns: ID, kind, status, the ask it came from, title.".to_owned());
    lines.push(
        "See one: gym coder proposals ID. Decide: gym coder proposals approve|reject ID."
            .to_owned(),
    );
    lines
}

/// One proposal as finding → change → result.
#[must_use]
pub fn one_lines(entry: &Entry, experiments: &Path) -> Vec<String> {
    let p = &entry.proposal;
    let mut lines = vec![
        format!(
            "{} · {} · {}",
            entry.id(),
            p["kind"].as_str().unwrap_or("?"),
            entry.status()
        ),
        p["title"].as_str().unwrap_or("").to_owned(),
        String::new(),
        "Finding".to_owned(),
        format!(
            "  The ask {}: {}",
            p["ask"]["id"].as_str().unwrap_or("?"),
            p["ask"]["question"].as_str().unwrap_or("")
        ),
    ];
    if let Some(rationale) = p["rationale"].as_str().filter(|r| !r.is_empty()) {
        lines.push(format!("  {rationale}"));
    }
    lines.push(format!("  Runs: {}", strings(&p["source_runs"]).join(", ")));
    lines.push(format!(
        "  Tasks it expects to change: {}",
        strings(&p["expected_tasks"]).join(", ")
    ));
    lines.push(String::new());
    lines.push("Change".to_owned());
    let materialized = &p["materialized"];
    match p["kind"].as_str() {
        Some("policy" | "check") => {
            lines.push(format!(
                "  Patch on {}: {}",
                p["change"]["base"].as_str().unwrap_or("?"),
                canonical(&p["change"]["patch"])
            ));
            if !materialized.is_null() {
                lines.push(format!(
                    "  Digest {} → {} ({})",
                    materialized["base_digest"].as_str().map_or("?", short),
                    materialized["digest"].as_str().map_or("?", short),
                    materialized["name"].as_str().unwrap_or("?")
                ));
            }
        }
        Some("minitask") => lines.push(format!(
            "  A new mini-task: {}",
            materialized["minitask"].as_str().unwrap_or("?")
        )),
        _ => {}
    }
    for problem in strings(&p["problems"]) {
        lines.push(format!("  Refused: {problem}"));
    }
    if let Some(draft) = p["issue_draft"].as_object() {
        lines.push(format!(
            "  Drafted issue, for a person to file: {} (coder-one proposal issue {})",
            draft.get("title").and_then(Value::as_str).unwrap_or(""),
            entry.id()
        ));
    }
    match &entry.decision {
        Some(decision) => lines.push(format!(
            "  {} by {} at {}{}",
            match decision["verdict"].as_str() {
                Some("approved") => "Approved",
                Some("rejected") => "Rejected",
                _ => "Decided",
            },
            decision["by"].as_str().unwrap_or("?"),
            decision["at"].as_str().unwrap_or("?"),
            decision["note"]
                .as_str()
                .filter(|n| !n.is_empty())
                .map_or_else(String::new, |n| format!(": {n}"))
        )),
        None if p["valid"] == true => lines.push(format!(
            "  Waiting for a decision: gym coder proposals approve|reject {}",
            entry.id()
        )),
        None => {}
    }
    lines.push(String::new());
    lines.push("Result".to_owned());
    let Some(result) = &entry.result else {
        lines.push(if entry.status() == "approved" {
            format!("  Not measured yet: coder-one proposal run {}", entry.id())
        } else {
            "  Nothing ran.".to_owned()
        });
        return lines;
    };
    let mini = &result["mini"];
    lines.push(format!(
        "  Mini stage: {} in {:.1}s on {}",
        mini["verdict"].as_str().unwrap_or("?"),
        mini["milliseconds"].as_f64().unwrap_or(0.0) / 1000.0,
        if mini["kind"] == "minitask" {
            mini["minitask"].as_str().unwrap_or("?").to_owned()
        } else {
            strings(&mini["tasks"]).join(", ")
        }
    ));
    for finding in strings(&mini["findings"]) {
        lines.push(format!("    - {finding}"));
    }
    let live = &result["live"];
    if live.is_null() {
        lines.push(
            "  Live stage: not started (coder-one proposal run ID --live --quota-usd USD)"
                .to_owned(),
        );
        return lines;
    }
    lines.push(format!(
        "  Live stage: experiment {} {}, {} against {} on {}, {} attempts each, quota ${}",
        live["experiment"].as_str().unwrap_or("?"),
        if live["plan_only"] == true {
            "planned"
        } else {
            "started"
        },
        live["experiment"].as_str().unwrap_or("?"),
        live["baseline"].as_str().unwrap_or("?"),
        strings(&live["tasks"]).join(", "),
        live["attempts"],
        live["quota_usd"]
    ));
    match live_report(entry, experiments) {
        Some(report) => {
            lines.push(format!(
                "    {} · quota used ${:.2}",
                report.state, report.quota_used_usd
            ));
            for arm in &report.summaries {
                lines.push(format!(
                    "    {:<32} {} of {} graded passed ({:.0}–{:.0}%), {} not run",
                    arm.arm,
                    arm.passes,
                    arm.graded,
                    arm.interval.0 * 100.0,
                    arm.interval.1 * 100.0,
                    arm.not_run
                ));
            }
            for paired in &report.paired {
                lines.push(format!(
                    "    paired over {}: proposal alone passed {}, baseline alone {} (McNemar p = {:.3})",
                    paired.pairs, paired.arm_only, paired.baseline_only, paired.mcnemar_p
                ));
            }
            lines.push(format!(
                "    gym terminal-bench experiment report {} --markdown",
                report.id
            ));
        }
        None => lines.push("    No status yet from tbench experiment.".to_owned()),
    }
    lines
}

fn short(digest: &str) -> &str {
    digest.get(..12).unwrap_or(digest)
}

/// `gym coder proposals …`.
///
/// # Errors
///
/// Returns a message when an argument doesn't parse, the proposal isn't
/// found, or a decision can't be recorded.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let mut dir = default_dir();
    let mut experiments = experiment::default_dir();
    let mut json_output = false;
    let mut note = String::new();
    let mut positional: Vec<String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        let mut value = |name: &str| {
            index += 1;
            args.get(index)
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg {
            "--json" => json_output = true,
            "--dir" => dir = Some(PathBuf::from(value("--dir")?)),
            "--experiments-dir" => experiments = PathBuf::from(value("--experiments-dir")?),
            "--note" => note = value("--note")?,
            "--help" | "-h" | "help" => {
                writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
                return Ok(0);
            }
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => positional.push(other.to_owned()),
        }
        index += 1;
    }
    let dir = dir.ok_or("no --dir and no HOME to read proposals from")?;
    let print = |out: &mut dyn std::io::Write, lines: Vec<String>| {
        for line in lines {
            writeln!(out, "{line}").map_err(|error| error.to_string())?;
        }
        Ok::<(), String>(())
    };
    match positional.first().map(String::as_str) {
        Some(verb @ ("approve" | "reject")) => {
            let [_, id] = positional.as_slice() else {
                return Err(format!("gym coder proposals {verb} needs one proposal ID"));
            };
            let verdict = if verb == "approve" {
                "approved"
            } else {
                "rejected"
            };
            let decision = decide(&dir, id, verdict, &note, &who())?;
            if json_output {
                writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&decision).unwrap_or_default()
                )
                .map_err(|error| error.to_string())?;
            } else {
                let id = decision["proposal"].as_str().unwrap_or("?");
                writeln!(
                    out,
                    "{id} {verdict}.{}",
                    if verdict == "approved" {
                        format!(" Measure it with `coder-one proposal run {id}`.")
                    } else {
                        String::new()
                    }
                )
                .map_err(|error| error.to_string())?;
            }
            Ok(0)
        }
        Some(query) => {
            let entries = load(&dir);
            let entry = &entries[find(&entries, query)?];
            if json_output {
                let mut value = summary(entry, &experiments);
                value["schema"] = json!(SCHEMA);
                value["proposal"] = entry.proposal.clone();
                writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&value).unwrap_or_default()
                )
                .map_err(|error| error.to_string())?;
            } else {
                print(out, one_lines(entry, &experiments))?;
            }
            Ok(0)
        }
        None => {
            let entries = load(&dir);
            if json_output {
                let value = json!({
                    "schema": SCHEMA,
                    "dir": dir.display().to_string(),
                    "proposals": entries.iter().map(|e| summary(e, &experiments)).collect::<Vec<_>>(),
                });
                writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&value).unwrap_or_default()
                )
                .map_err(|error| error.to_string())?;
            } else {
                print(out, list_lines(&entries))?;
            }
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_proposal(root: &Path, id: &str, valid: bool) -> Value {
        let dir = root.join(id);
        std::fs::create_dir_all(&dir).unwrap();
        let mut proposal = json!({
            "schema": PROPOSAL_SCHEMA,
            "id": id,
            "ask": {"id": "ask-1790000000000", "question": "why did Luna fail?", "dir": "/a"},
            "created_at": format!("2026-09-23T00:00:0{}Z", id.len() % 10),
            "kind": "check",
            "title": "Run the behavior scenarios",
            "rationale": "The checks passed a wrong summary.",
            "source_runs": ["tb4--a--log-summary-date-ranges/log-summary-date-ranges__1"],
            "expected_tasks": ["log-summary-date-ranges"],
            "change": {"base": "tunable-luna-v2", "patch": {"policy": {"verify": {"behavior": true}}}},
            "materialized": {"base": "tunable-luna-v2", "base_digest": "aaaaaaaaaaaaaaaa", "digest": "bbbbbbbbbbbbbbbb", "name": "tunable-luna-v2-x"},
            "needs_code": false,
            "issue_draft": null,
            "valid": valid,
            "problems": if valid { json!([]) } else { json!(["the patch sets protected"]) },
        });
        proposal["digest"] = json!(digest(&proposal));
        std::fs::write(dir.join("proposal.json"), proposal.to_string()).unwrap();
        proposal
    }

    fn run(args: &[&str]) -> (Result<i32, String>, String) {
        let mut out = Vec::new();
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let code = command(&args, &mut out);
        (code, String::from_utf8(out).unwrap())
    }

    #[test]
    fn a_person_approves_or_rejects_and_the_view_shows_finding_change_result() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().display().to_string();
        write_proposal(root.path(), "prop-1790000000000-1", true);
        write_proposal(root.path(), "prop-1790000000000-2", false);

        let (code, text) = run(&["--dir", &dir]);
        assert_eq!(code, Ok(0));
        assert!(text.contains("prop-1790000000000-1"), "{text}");
        assert!(text.contains("proposed"), "{text}");
        assert!(text.contains("refused"), "{text}");

        // A refused proposal can't be approved.
        let (code, _) = run(&["approve", "prop-1790000000000-2", "--dir", &dir]);
        assert!(code.unwrap_err().contains("didn't validate"));

        let (code, text) = run(&[
            "approve",
            "prop-1790000000000-1",
            "--note",
            "worth three attempts",
            "--dir",
            &dir,
        ]);
        assert_eq!(code, Ok(0), "{text}");
        assert!(text.contains("approved"), "{text}");
        let decision: Value = serde_json::from_str(
            &std::fs::read_to_string(root.path().join("prop-1790000000000-1/decision.json"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(decision["schema"], DECISION_SCHEMA);
        assert_eq!(decision["note"], "worth three attempts");

        let (_, text) = run(&["prop-1790000000000-1", "--dir", &dir]);
        assert!(text.contains("Finding"), "{text}");
        assert!(text.contains("The ask ask-1790000000000"), "{text}");
        assert!(text.contains("Patch on tunable-luna-v2"), "{text}");
        assert!(text.contains("aaaaaaaaaaaa → bbbbbbbbbbbb"), "{text}");
        assert!(text.contains("Approved by"), "{text}");
        assert!(text.contains("Not measured yet"), "{text}");

        // A mini stage and a live experiment show as the result.
        let experiments = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(experiments.path().join("prop-1790000000000-1")).unwrap();
        let status = json!({
            "schema": experiment::STATUS_SCHEMA,
            "experiment": "prop-1790000000000-1",
            "profile": "tb4",
            "arms": [{"id": "base", "providers": []}, {"id": "prop-1790000000000-1", "providers": []}],
            "tasks": ["log-summary-date-ranges"],
            "attempts": 1,
            "state": "done",
            "trials": [
                {"arm": "base", "task": "log-summary-date-ranges", "attempt": 1, "state": "finished", "reward": 0.0, "job": "j1"},
                {"arm": "prop-1790000000000-1", "task": "log-summary-date-ranges", "attempt": 1, "state": "finished", "reward": 1.0, "job": "j2"},
            ],
        });
        std::fs::write(
            experiments.path().join("prop-1790000000000-1/status.json"),
            status.to_string(),
        )
        .unwrap();
        std::fs::write(
            root.path().join("prop-1790000000000-1/result.json"),
            json!({
                "schema": "openagents.coder-one.proposal-result.v1",
                "proposal": "prop-1790000000000-1",
                "mini": {"kind": "policy", "verdict": "unchanged", "tasks": ["log-severity"], "findings": [], "milliseconds": 900},
                "live": {"experiment": "prop-1790000000000-1", "baseline": "base", "tasks": ["log-summary-date-ranges"], "attempts": 3, "quota_usd": 40.0},
            })
            .to_string(),
        )
        .unwrap();
        let experiments_dir = experiments.path().display().to_string();
        let (_, text) = run(&[
            "prop-1790000000000-1",
            "--dir",
            &dir,
            "--experiments-dir",
            &experiments_dir,
        ]);
        assert!(text.contains("Mini stage: unchanged"), "{text}");
        assert!(
            text.contains("Live stage: experiment prop-1790000000000-1 started"),
            "{text}"
        );
        let (_, json_text) = run(&[
            "prop-1790000000000-1",
            "--json",
            "--dir",
            &dir,
            "--experiments-dir",
            &experiments_dir,
        ]);
        let value: Value = serde_json::from_str(&json_text).unwrap();
        assert_eq!(value["status"], "live");
        // The decision stands once a stage ran.
        let (code, _) = run(&["reject", "prop-1790000000000-1", "--dir", &dir]);
        assert!(code.unwrap_err().contains("decision stands"));
    }

    #[test]
    fn an_edited_proposal_can_not_be_decided() {
        let root = tempfile::tempdir().unwrap();
        let mut proposal = write_proposal(root.path(), "prop-1-1", true);
        proposal["expected_tasks"] = json!(["fix-git"]);
        std::fs::write(
            root.path().join("prop-1-1/proposal.json"),
            proposal.to_string(),
        )
        .unwrap();
        let error = decide(root.path(), "prop-1-1", "approved", "", "t").unwrap_err();
        assert!(error.contains("changed after it was written"), "{error}");
    }
}
