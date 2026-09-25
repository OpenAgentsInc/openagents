//! The pattern-provenance registry (issue #9655): one file per pattern
//! component under [`PATTERNS_DIR`], recording where the pattern was
//! learned and whether it was admitted.
//!
//! A pattern's source tasks never count as evidence for it
//! (`docs/coder/design/pattern-components.md`, "Provenance and
//! admission"). The check joins each pattern's declared source tasks with
//! the tasks of every annotated lexical match that names the pattern, and
//! lists the union as the tasks that pattern may not count as evidence.
//! An admitted pattern must have been measured on other tasks, and its
//! trigger and output must be of the kinds the design admits.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::files_under;
use super::lexicon::Match;

/// A pattern file's schema.
pub const PATTERN_SCHEMA: &str = "openagents.pattern.v1";
/// The registry, relative to the repository root.
pub const PATTERNS_DIR: &str = "patterns";
/// What can start a pattern: code enumerating the workspace's structure,
/// a typed judgment over candidates, a fitted phrase list, or an
/// unconditional instruction.
pub const TRIGGER_KINDS: [&str; 4] = ["structural", "semantic", "phrase-list", "instruction"];
/// What a pattern produces: evidence, an executed check, or an
/// instruction to the model.
pub const OUTPUT_KINDS: [&str; 3] = ["evidence", "check", "instruction"];
/// Where a pattern stands: measured nowhere but its sources, admitted
/// into a policy, or retired.
pub const STATUSES: [&str; 3] = ["candidate", "admitted", "retired"];

/// One task a pattern was learned from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceTask {
    pub task: String,
    pub how: String,
    pub commits: Vec<String>,
}

/// One measurement in a pattern's admission record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Measurement {
    pub task: String,
    pub result: String,
    pub evidence: String,
}

/// One pattern file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Pattern {
    pub id: String,
    pub path: String,
    pub digest: String,
    pub trigger: String,
    pub output: String,
    pub status: String,
    pub source_tasks: Vec<SourceTask>,
    pub measured_on: Vec<Measurement>,
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// Parses one pattern file's text. `name` is its path, for messages and
/// for the id check: a file's stem is its pattern's id.
///
/// # Errors
/// The file isn't a complete pattern.
pub fn parse(name: &str, bytes: &[u8]) -> Result<Pattern, String> {
    let doc: Value =
        serde_json::from_slice(bytes).map_err(|error| format!("{name} isn't JSON: {error}"))?;
    if doc["schema"].as_str() != Some(PATTERN_SCHEMA) {
        return Err(format!("{name} isn't {PATTERN_SCHEMA}"));
    }
    let text = |pointer: &str| {
        doc.pointer(pointer)
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let Some(id) = text("/id") else {
        return Err(format!("{name} has no id"));
    };
    let stem = Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if stem != id {
        return Err(format!("{name}: the id {id:?} must be the file's name"));
    }
    for field in [
        "/name",
        "/summary",
        "/trigger/detail",
        "/output/detail",
        "/admission/record",
    ] {
        if text(field).is_none_or(|t| t.trim().is_empty()) {
            return Err(format!("{name}: {field} is missing"));
        }
    }
    let pick = |pointer: &str, allowed: &[&str]| -> Result<String, String> {
        let value = text(pointer).unwrap_or_default();
        if allowed.contains(&value.as_str()) {
            Ok(value)
        } else {
            Err(format!(
                "{name}: {pointer} is {value:?}; use one of {allowed:?}"
            ))
        }
    };
    let trigger = pick("/trigger/kind", &TRIGGER_KINDS)?;
    let output = pick("/output/kind", &OUTPUT_KINDS)?;
    let status = pick("/admission/status", &STATUSES)?;
    let mut source_tasks = Vec::new();
    for source in doc["source_tasks"].as_array().into_iter().flatten() {
        let (Some(task), Some(how)) = (source["task"].as_str(), source["how"].as_str()) else {
            return Err(format!("{name}: every source task needs a task and how"));
        };
        source_tasks.push(SourceTask {
            task: task.to_owned(),
            how: how.to_owned(),
            commits: strings(&source["commits"]),
        });
    }
    if source_tasks.is_empty() {
        return Err(format!(
            "{name}: a pattern names the tasks it was learned from"
        ));
    }
    let mut measured_on = Vec::new();
    for measurement in doc
        .pointer("/admission/measured_on")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let field = |key: &str| measurement[key].as_str().map(str::to_owned);
        let (Some(task), Some(result), Some(evidence)) =
            (field("task"), field("result"), field("evidence"))
        else {
            return Err(format!(
                "{name}: every measurement needs a task, a result, and evidence"
            ));
        };
        measured_on.push(Measurement {
            task,
            result,
            evidence,
        });
    }
    let digest = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(Pattern {
        id,
        path: name.to_owned(),
        digest: format!("sha256:{digest}"),
        trigger,
        output,
        status,
        source_tasks,
        measured_on,
    })
}

/// Reads every pattern under [`PATTERNS_DIR`].
///
/// # Errors
/// A pattern file can't be read or isn't complete.
pub fn load(root: &Path) -> Result<Vec<Pattern>, String> {
    let mut files: Vec<PathBuf> = Vec::new();
    files_under(
        &root.join(PATTERNS_DIR),
        &|path| path.extension().is_some_and(|ext| ext == "json"),
        &mut files,
    );
    let mut out = Vec::new();
    for path in files {
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("can't read {}: {error}", path.display()))?;
        let name = super::relative(root, &path);
        out.push(parse(&name, &bytes)?);
    }
    Ok(out)
}

/// The tasks a pattern may not count as evidence: its source tasks and
/// the tasks of every annotated lexical match that names it.
#[must_use]
pub fn not_evidence(pattern: &Pattern, matches: &[Match]) -> BTreeSet<String> {
    let mut tasks: BTreeSet<String> = pattern
        .source_tasks
        .iter()
        .map(|source| source.task.clone())
        .collect();
    for found in matches {
        if found.provenance.as_ref().and_then(|p| p.pattern.as_deref()) == Some(pattern.id.as_str())
        {
            tasks.insert(found.task.clone());
        }
    }
    tasks
}

/// What is wrong with the registry against the matches: an admitted
/// pattern measured only on tasks it may not count, or of a kind the
/// design doesn't admit, and a provenance entry naming no pattern.
#[must_use]
pub fn problems(patterns: &[Pattern], matches: &[Match], named: &[String]) -> Vec<String> {
    let ids: BTreeSet<&str> = patterns.iter().map(|p| p.id.as_str()).collect();
    let mut out = Vec::new();
    for pattern in named {
        if !ids.contains(pattern.as_str()) {
            out.push(format!(
                "a provenance entry names pattern {pattern:?}, which {PATTERNS_DIR}/ doesn't hold"
            ));
        }
    }
    for pattern in patterns {
        if pattern.status != "admitted" {
            continue;
        }
        if pattern.trigger == "phrase-list" || pattern.output == "instruction" {
            out.push(format!(
                "{}: an admitted pattern needs a structural or semantic trigger and evidence \
                 or a check as output, not a {} trigger and {} output",
                pattern.id, pattern.trigger, pattern.output
            ));
        }
        let excluded = not_evidence(pattern, matches);
        let counted: Vec<&Measurement> = pattern
            .measured_on
            .iter()
            .filter(|m| !excluded.contains(&m.task))
            .collect();
        if counted.is_empty() {
            out.push(format!(
                "{}: admitted, but no measurement is on a task other than {}",
                pattern.id,
                excluded.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        for measurement in &pattern.measured_on {
            if excluded.contains(&measurement.task) {
                out.push(format!(
                    "{}: its admission record counts {}, which it may not count as evidence",
                    pattern.id, measurement.task
                ));
            }
        }
    }
    out
}

/// The registry's part of a check report.
#[must_use]
pub fn report(patterns: &[Pattern], matches: &[Match], problems: &[String]) -> Value {
    let entries: Vec<Value> = patterns
        .iter()
        .map(|pattern| {
            json!({
                "id": pattern.id,
                "path": pattern.path,
                "digest": pattern.digest,
                "status": pattern.status,
                "trigger": pattern.trigger,
                "output": pattern.output,
                "source_tasks": pattern.source_tasks.iter().map(|s| &s.task).collect::<Vec<_>>(),
                "measured_on": pattern.measured_on.iter().map(|m| &m.task).collect::<Vec<_>>(),
                "not_evidence": not_evidence(pattern, matches),
            })
        })
        .collect();
    let mut by_status: BTreeMap<&str, usize> = BTreeMap::new();
    for pattern in patterns {
        *by_status.entry(pattern.status.as_str()).or_default() += 1;
    }
    json!({
        "patterns": entries,
        "by_status": by_status,
        "problems": problems,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contamination::lexicon::Provenance;

    fn pattern_json(status: &str, measured: &[&str]) -> Vec<u8> {
        json!({
            "schema": PATTERN_SCHEMA,
            "id": "defended-comment-suspects",
            "name": "Defended comments are defect suspects",
            "summary": "A comment that defends a departure is a suspect.",
            "trigger": {"kind": "semantic", "detail": "Every comment on a selected function."},
            "output": {"kind": "evidence", "detail": "Ranked suspects."},
            "source_tasks": [{"task": "drift-task", "how": "v13 read its comments",
                              "commits": ["2544c9ed77"]}],
            "admission": {
                "status": status,
                "record": "Measured on retained workspaces.",
                "measured_on": measured.iter().map(|task| json!({
                    "task": task, "result": "ranked the defect first", "evidence": "doc.md"
                })).collect::<Vec<_>>(),
            },
        })
        .to_string()
        .into_bytes()
    }

    fn annotated(task: &str) -> Match {
        Match {
            kind: "entry",
            phrase: "accounts for".to_owned(),
            task: task.to_owned(),
            location: "src/m.rs:3".to_owned(),
            list: Some("src/m.rs::MARKS".to_owned()),
            provenance: Some(Provenance {
                phrase: "accounts for".to_owned(),
                task: task.to_owned(),
                paths: vec!["src/m.rs".to_owned()],
                relation: "coincident".to_owned(),
                commits: Vec::new(),
                pattern: Some("defended-comment-suspects".to_owned()),
                note: "n".to_owned(),
            }),
        }
    }

    #[test]
    fn a_pattern_file_parses_and_its_id_is_its_name() {
        let bytes = pattern_json("candidate", &[]);
        let pattern = parse("patterns/defended-comment-suspects.json", &bytes).expect("parse");
        assert_eq!(pattern.status, "candidate");
        assert!(pattern.digest.starts_with("sha256:"));
        assert!(parse("patterns/other.json", &bytes).is_err());
    }

    #[test]
    fn source_tasks_and_annotated_matches_are_not_evidence() {
        let bytes = pattern_json("candidate", &[]);
        let pattern = parse("patterns/defended-comment-suspects.json", &bytes).expect("parse");
        let tasks = not_evidence(&pattern, &[annotated("family-task")]);
        assert_eq!(
            tasks,
            BTreeSet::from(["drift-task".to_owned(), "family-task".to_owned()])
        );
    }

    #[test]
    fn an_admitted_pattern_must_be_measured_on_other_tasks() {
        let matches = [annotated("family-task")];
        let only_sources = parse(
            "patterns/defended-comment-suspects.json",
            &pattern_json("admitted", &["drift-task", "family-task"]),
        )
        .expect("parse");
        let found = problems(&[only_sources], &matches, &[]);
        assert_eq!(found.len(), 3, "{found:?}");
        let held_out = parse(
            "patterns/defended-comment-suspects.json",
            &pattern_json("admitted", &["held-out-task"]),
        )
        .expect("parse");
        assert!(problems(std::slice::from_ref(&held_out), &matches, &[]).is_empty());
        let unknown = problems(&[held_out], &matches, &["missing".to_owned()]);
        assert_eq!(unknown.len(), 1);
    }
}
