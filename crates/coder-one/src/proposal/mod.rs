//! Proposals: a finding from `coder-one ask`, turned into a typed change a
//! person approves and a targeted measurement tests.
//!
//! ```text
//! ask       the executor's `answer` may carry proposals, each typed
//! validate  code checks each one: a policy diff applies to a checked-in
//!           manifest and yields a valid, new digest; a mini-task parses
//! approve   a person approves or rejects it: `gym coder proposals approve`
//! mini      `coder-one proposal run ID` runs it on the cited tasks'
//!           mini-tasks first: seconds, and no model
//! live      with `--live`, a targeted `tbench experiment` on the cited
//!           tasks only: 3 matched attempts against the current policy,
//!           under a `--quota-usd` budget
//! ```
//!
//! A proposal is one of five kinds:
//!
//! | Kind | The change | Runs as |
//! | --- | --- | --- |
//! | `policy` | A JSON merge patch on a manifest in `crates/coder-one/policies/` | The mini stage, then `--live` |
//! | `check` | The same, confined to `policy.verify` | The mini stage, then `--live` |
//! | `minitask` | A new mini-task: files, a grader, and a good and a bad candidate | The mini stage: the grader passes the good one and fails the bad one |
//! | `questions` | A change to Jev's question set | Nothing: the set is pinned in code, so it's a drafted issue |
//! | `code` | Anything that needs Rust | Nothing: a drafted issue for a person to file |
//!
//! Each proposal names the runs it came from, which the ask's answer must
//! have read, and the tasks it expects to change, which must be those
//! runs' tasks, so a measurement never reaches a task the finding didn't.
//!
//! Proposals live under `~/.openagents/coder-one/proposals/<id>/`:
//!
//! ```text
//! proposal.json   openagents.coder-one.proposal.v1, written once by the ask
//! policy.json     the materialized manifest, for a valid policy or check
//! minitask.json   the mini-task, for a valid minitask proposal
//! decision.json   openagents.coder-one.proposal-decision.v1, from the Gym
//! result.json     openagents.coder-one.proposal-result.v1: each stage
//! mini/           the mini-task runs
//! ```
//!
//! The proposal carries a digest of itself, and a decision names the digest
//! it approved, so an edited proposal needs a new decision.

pub mod cli;
pub mod stage;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::policy::{self, Manifest};

/// A proposal's schema.
pub const SCHEMA: &str = "openagents.coder-one.proposal.v1";
/// A decision's schema.
pub const DECISION_SCHEMA: &str = "openagents.coder-one.proposal-decision.v1";
/// A result's schema.
pub const RESULT_SCHEMA: &str = "openagents.coder-one.proposal-result.v1";

pub const PROPOSAL_FILE: &str = "proposal.json";
pub const POLICY_FILE: &str = "policy.json";
pub const MINITASK_FILE: &str = "minitask.json";
pub const DECISION_FILE: &str = "decision.json";
pub const RESULT_FILE: &str = "result.json";

/// The most proposals one answer may carry.
pub const MAX_PROPOSALS: usize = 4;

/// The Terminal-Bench tasks each mini-task reproduces a failure family of.
pub const MINI_FOR_TASK: &[(&str, &str)] = &[
    ("log-summary-date-ranges", "log-severity"),
    ("headless-terminal", "interactive-terminal"),
    ("cancel-async-tasks", "cancel-cleanup"),
    ("fix-git", "git-recovery"),
];

/// The mini-task that reproduces `task`'s failure family, if one does.
#[must_use]
pub fn mini_for(task: &str) -> Option<&'static str> {
    MINI_FOR_TASK
        .iter()
        .find(|(t, _)| *t == task)
        .map(|(_, mini)| *mini)
}

/// Where proposals are kept: `~/.openagents/coder-one/proposals`.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/proposals"))
}

/// The checked-in manifests: `<repo>/crates/coder-one/policies` when the
/// repository has it, else this crate's own.
#[must_use]
pub fn policies_dir(repo: &Path) -> PathBuf {
    let checkout = repo.join("crates/coder-one/policies");
    if checkout.is_dir() {
        checkout
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("policies")
    }
}

/// A proposal's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Policy,
    Check,
    Questions,
    Minitask,
    Code,
}

impl Kind {
    /// Parses a kind's word.
    ///
    /// # Errors
    ///
    /// Returns a message naming the kinds.
    pub fn parse(word: &str) -> Result<Self, String> {
        match word.trim() {
            "policy" => Ok(Kind::Policy),
            "check" => Ok(Kind::Check),
            "questions" => Ok(Kind::Questions),
            "minitask" => Ok(Kind::Minitask),
            "code" => Ok(Kind::Code),
            other => Err(format!(
                "a proposal's kind is policy, check, questions, minitask, or code, not {other:?}"
            )),
        }
    }

    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Kind::Policy => "policy",
            Kind::Check => "check",
            Kind::Questions => "questions",
            Kind::Minitask => "minitask",
            Kind::Code => "code",
        }
    }

    /// Whether the change can't be materialized without Rust, so it
    /// becomes a drafted issue.
    #[must_use]
    pub fn needs_code(self) -> bool {
        matches!(self, Kind::Questions | Kind::Code)
    }
}

/// One proposal as the executor wrote it in its `answer`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Draft {
    pub kind: String,
    pub title: String,
    pub rationale: String,
    pub source_runs: Vec<String>,
    pub expected_tasks: Vec<String>,
    /// For `policy` and `check`: the manifest file's stem, such as
    /// `tunable-luna-v2`.
    pub base: String,
    /// For `policy` and `check`: a JSON merge patch (RFC 7386), as text.
    pub patch: String,
    /// For `questions`: the question set it changes.
    pub question_set: String,
    /// For `minitask`: the mini-task, as JSON text.
    pub minitask: String,
    /// For `questions` and `code`, and optional for the rest: what a person
    /// would change, as the body of an issue.
    pub issue: String,
}

/// The proposals an answer carries.
#[must_use]
pub fn drafts(answer: &Value) -> Vec<Draft> {
    answer["proposals"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| serde_json::from_value::<Draft>(p.clone()).ok())
        .filter(|d| !(d.kind.trim().is_empty() && d.title.trim().is_empty()))
        .collect()
}

/// A mini-task a proposal defines: files the task starts with, a public
/// instruction, a grader command, and two candidates, one that should pass
/// and one that shows the failure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MiniSpec {
    pub id: String,
    pub family: String,
    pub instruction: String,
    /// Files the task starts with, by relative path.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    /// A shell command run in the task directory; exit 0 passes.
    pub grader: String,
    /// The files a passing candidate writes.
    pub good: BTreeMap<String, String>,
    /// The files a candidate with the failure writes.
    pub bad: BTreeMap<String, String>,
}

/// The most characters a mini-task's files take together.
pub const MINI_MAX_CHARS: usize = 64 * 1024;

impl MiniSpec {
    /// Parses and checks a mini-task.
    ///
    /// # Errors
    ///
    /// Returns every problem found.
    pub fn parse(text: &str) -> Result<Self, Vec<String>> {
        let spec: MiniSpec = serde_json::from_str(text).map_err(|error| {
            vec![format!(
                "the mini-task isn't valid JSON of its shape: {error}"
            )]
        })?;
        let mut problems = Vec::new();
        if spec.id.is_empty()
            || !spec
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            problems.push(format!(
                "the mini-task's id {:?} must be lowercase letters, digits, and hyphens",
                spec.id
            ));
        }
        if crate::minitask::CATALOG.iter().any(|t| t.id == spec.id) {
            problems.push(format!("the mini-task {} already exists", spec.id));
        }
        if spec.instruction.trim().is_empty() {
            problems.push("the mini-task has no instruction".to_string());
        }
        if spec.grader.trim().is_empty() {
            problems.push("the mini-task has no grader command".to_string());
        }
        if spec.good.is_empty() || spec.bad.is_empty() {
            problems.push("the mini-task needs a good and a bad candidate".to_string());
        }
        if spec.good == spec.bad {
            problems.push("the good and bad candidates are the same".to_string());
        }
        let mut chars = spec.instruction.len() + spec.grader.len();
        for (path, body) in spec.files.iter().chain(&spec.good).chain(&spec.bad) {
            chars += body.len();
            let relative = Path::new(path);
            if path.is_empty()
                || relative.is_absolute()
                || relative
                    .components()
                    .any(|c| !matches!(c, std::path::Component::Normal(_)))
            {
                problems.push(format!(
                    "the path {path:?} must be relative, inside the task directory"
                ));
            }
        }
        if chars > MINI_MAX_CHARS {
            problems.push(format!(
                "the mini-task is {chars} characters; the most is {MINI_MAX_CHARS}"
            ));
        }
        if problems.is_empty() {
            Ok(spec)
        } else {
            Err(problems)
        }
    }
}

/// What a proposal is checked against.
#[derive(Clone, Debug)]
pub struct Context {
    pub ask_id: String,
    pub question: String,
    pub ask_dir: String,
    /// The manifests a policy or check patches.
    pub policies: PathBuf,
    /// Every run the ask read, as cited or resolved, with its task.
    pub run_tasks: BTreeMap<String, String>,
}

/// JSON with object keys sorted and no whitespace, then SHA-256.
#[must_use]
pub fn digest(value: &Value) -> String {
    let mut value = value.clone();
    if let Some(object) = value.as_object_mut() {
        object.remove("digest");
    }
    Sha256::digest(policy::canonical(&value).as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Applies a JSON merge patch (RFC 7386) to `target`.
pub fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(fields) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    let Some(object) = target.as_object_mut() else {
        return;
    };
    for (key, value) in fields {
        if value.is_null() {
            object.remove(key);
        } else {
            merge_patch(object.entry(key.clone()).or_insert(Value::Null), value);
        }
    }
}

/// Every leaf path a patch sets or removes, as `a.b.c`.
fn patch_paths(patch: &Value, prefix: &str, out: &mut Vec<String>) {
    match patch {
        Value::Object(fields) if !fields.is_empty() => {
            for (key, value) in fields {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                patch_paths(value, &path, out);
            }
        }
        _ => out.push(prefix.to_string()),
    }
}

/// A policy or check patch applied: the new manifest, or why not.
///
/// # Errors
///
/// Returns every problem found.
pub fn materialize(
    policies: &Path,
    base: &str,
    patch_text: &str,
    kind: Kind,
    name: &str,
    note: &str,
) -> Result<(Value, Value), Vec<String>> {
    let stem = base.trim().trim_end_matches(".json");
    if stem.is_empty() || stem.contains('/') || stem.contains("..") {
        return Err(vec![format!(
            "base {base:?} must name a manifest file in {}",
            policies.display()
        )]);
    }
    let file = policies.join(format!("{stem}.json"));
    let text = std::fs::read_to_string(&file).map_err(|_| {
        vec![format!(
            "no manifest {stem}.json in {}; the base must be a checked-in manifest",
            policies.display()
        )]
    })?;
    let base_value: Value = serde_json::from_str(&text)
        .map_err(|error| vec![format!("{} isn't JSON: {error}", file.display())])?;
    let base_manifest = Manifest::parse(&text).map_err(|error| vec![error])?;
    let patch: Value = serde_json::from_str(patch_text.trim())
        .map_err(|error| vec![format!("the patch isn't JSON: {error}")])?;
    if !patch.as_object().is_some_and(|p| !p.is_empty()) {
        return Err(vec![
            "the patch must be a JSON merge patch object that changes something".to_string(),
        ]);
    }
    let mut problems = Vec::new();
    let mut paths = Vec::new();
    patch_paths(&patch, "", &mut paths);
    for path in &paths {
        let top = path.split('.').next().unwrap_or_default();
        if top != "policy" {
            problems.push(format!(
                "the patch sets {path}; a proposal changes only fields under policy \
                 (protected, schema, name, note, and search are the host's)"
            ));
        } else if kind == Kind::Check && !path.starts_with("policy.verify") {
            problems.push(format!(
                "a check proposal changes only policy.verify, and this one sets {path}"
            ));
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let mut value = base_value;
    merge_patch(&mut value, &patch);
    value["name"] = json!(name);
    value["note"] = json!(note);
    let text = serde_json::to_string(&value).unwrap_or_default();
    let manifest = Manifest::parse(&text).map_err(|error| vec![error])?;
    manifest
        .validate()
        .map_err(|error| vec![format!("the patched manifest doesn't validate: {error}")])?;
    let (base_digest, new_digest) = (base_manifest.digest(), manifest.digest());
    if base_digest == new_digest {
        return Err(vec![
            "the patch leaves the manifest's digest unchanged, so it changes nothing that runs"
                .to_string(),
        ]);
    }
    let summary = json!({
        "base": stem,
        "base_file": file.display().to_string(),
        "base_name": base_manifest.name,
        "base_digest": base_digest,
        "name": name,
        "digest": new_digest,
        "fields": paths,
        "file": POLICY_FILE,
    });
    // The patched file as written, not a round trip through this build's
    // types, so an artifact built before a field existed still reads it.
    Ok((value, summary))
}

/// A drafted issue for a person to file.
#[must_use]
pub fn issue_draft(proposal: &Value, extra: &str) -> Value {
    let list = |key: &str| {
        proposal[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|s| format!("`{s}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut body = String::new();
    let rationale = proposal["rationale"].as_str().unwrap_or_default().trim();
    if !rationale.is_empty() {
        body.push_str(rationale);
        body.push_str("\n\n");
    }
    if !extra.trim().is_empty() {
        body.push_str("## Change\n\n");
        body.push_str(extra.trim());
        body.push_str("\n\n");
    }
    body.push_str("## Evidence\n\n");
    body.push_str(&format!(
        "- The ask `{}`: \"{}\"\n",
        proposal["ask"]["id"].as_str().unwrap_or("?"),
        proposal["ask"]["question"].as_str().unwrap_or("").trim()
    ));
    body.push_str(&format!("- Source runs: {}\n", list("source_runs")));
    body.push_str(&format!(
        "- Tasks it expects to change: {}\n",
        list("expected_tasks")
    ));
    body.push_str(&format!(
        "- Proposal `{}` (kind `{}`), drafted by `coder-one ask`. A person files it; \
         nothing merges on its own.\n",
        proposal["id"].as_str().unwrap_or("?"),
        proposal["kind"].as_str().unwrap_or("?")
    ));
    json!({
        "title": proposal["title"].as_str().unwrap_or("").trim(),
        "body": body,
    })
}

/// Checks one drafted proposal and builds its record, valid or not. The
/// second value is the file to write beside it, when there is one.
#[must_use]
pub fn validate(draft: &Draft, id: &str, context: &Context) -> (Value, Option<(String, Value)>) {
    let mut problems: Vec<String> = Vec::new();
    let kind = match Kind::parse(&draft.kind) {
        Ok(kind) => Some(kind),
        Err(why) => {
            problems.push(why);
            None
        }
    };
    if draft.title.trim().is_empty() {
        problems.push("the proposal has no title".to_string());
    }
    let source_runs: Vec<String> = dedup(&draft.source_runs);
    let expected_tasks: Vec<String> = dedup(&draft.expected_tasks);
    if source_runs.is_empty() {
        problems.push("the proposal names no source runs".to_string());
    }
    let mut source_tasks = BTreeSet::new();
    for run in &source_runs {
        match context.run_tasks.get(run) {
            Some(task) => {
                source_tasks.insert(task.clone());
            }
            None => problems.push(format!("the source run {run} isn't a run the ask read")),
        }
    }
    if expected_tasks.is_empty() {
        problems.push("the proposal names no tasks it expects to change".to_string());
    }
    for task in &expected_tasks {
        if !source_tasks.is_empty() && !source_tasks.contains(task) {
            problems.push(format!(
                "the task {task} isn't the task of any source run; a proposal expects to \
                 change only the tasks its runs came from"
            ));
        }
    }
    let mut record = json!({
        "schema": SCHEMA,
        "id": id,
        "ask": {
            "id": context.ask_id,
            "question": context.question,
            "dir": context.ask_dir,
        },
        "created_at": atif::document::iso(atif::now_ms()),
        "kind": kind.map_or_else(|| draft.kind.clone(), |k| k.word().to_string()),
        "title": draft.title.trim(),
        "rationale": draft.rationale.trim(),
        "source_runs": source_runs,
        "expected_tasks": expected_tasks,
        "mini_tasks": expected_tasks.iter().filter_map(|t| mini_for(t)).collect::<BTreeSet<_>>(),
        "change": Value::Null,
        "materialized": Value::Null,
        "needs_code": kind.is_some_and(Kind::needs_code),
        "issue_draft": Value::Null,
    });
    let mut file = None;
    match kind {
        Some(kind @ (Kind::Policy | Kind::Check)) => {
            record["change"] =
                json!({ "base": draft.base.trim(), "patch": parse_or_text(&draft.patch) });
            let name = format!("{}-{id}", draft.base.trim().trim_end_matches(".json"));
            let note = format!(
                "{} (proposal {id} from ask {}). {}",
                draft.title.trim(),
                context.ask_id,
                draft.rationale.trim()
            );
            match materialize(
                &context.policies,
                &draft.base,
                &draft.patch,
                kind,
                &name,
                &note,
            ) {
                Ok((manifest, summary)) => {
                    record["materialized"] = summary;
                    file = Some((POLICY_FILE.to_string(), manifest));
                }
                Err(more) => problems.extend(more),
            }
        }
        Some(Kind::Minitask) => {
            record["change"] = json!({ "minitask": parse_or_text(&draft.minitask) });
            match MiniSpec::parse(&draft.minitask) {
                Ok(spec) => {
                    record["materialized"] = json!({ "minitask": spec.id, "file": MINITASK_FILE });
                    file = Some((
                        MINITASK_FILE.to_string(),
                        serde_json::to_value(&spec).unwrap_or(Value::Null),
                    ));
                }
                Err(more) => problems.extend(more),
            }
        }
        Some(Kind::Questions) => {
            record["change"] =
                json!({ "question_set": draft.question_set.trim(), "change": draft.issue.trim() });
            if draft.question_set.trim() != policy::QUESTION_SETS {
                problems.push(format!(
                    "the question set {:?} isn't one this build has; it has {}",
                    draft.question_set.trim(),
                    policy::QUESTION_SETS
                ));
            }
            if draft.issue.trim().is_empty() {
                problems.push("a questions proposal says what to change in `issue`".to_string());
            }
        }
        Some(Kind::Code) => {
            record["change"] = json!({ "issue": draft.issue.trim() });
            if draft.issue.trim().is_empty() {
                problems.push("a code proposal says what to change in `issue`".to_string());
            }
        }
        None => {}
    }
    if kind.is_some_and(|k| k.needs_code() || k == Kind::Minitask) {
        let extra = match kind {
            Some(Kind::Minitask) => format!(
                "Add the mini-task `{}` to `crates/coder-one/src/minitask/` once `coder-one \
                 proposal run {id}` shows its grader passes the good candidate and fails the bad \
                 one. The spec is `{MINITASK_FILE}` beside the proposal.\n\n{}",
                record["materialized"]["minitask"].as_str().unwrap_or("?"),
                draft.issue.trim()
            ),
            _ => draft.issue.clone(),
        };
        record["issue_draft"] = issue_draft(&record, &extra);
    }
    if !problems.is_empty() {
        // An invalid proposal leaves nothing a run could pick up.
        file = None;
    }
    record["valid"] = json!(problems.is_empty());
    record["problems"] = json!(problems);
    let sealed = digest(&record);
    record["digest"] = json!(sealed);
    (record, file)
}

fn dedup(items: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let item = item.trim().to_string();
        if !item.is_empty() && !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

fn parse_or_text(text: &str) -> Value {
    serde_json::from_str(text.trim()).unwrap_or_else(|_| json!(text.trim()))
}

/// The ID of an ask's `index`th proposal, from 1.
#[must_use]
pub fn id_for(ask_id: &str, index: usize) -> String {
    format!(
        "prop-{}-{index}",
        ask_id.strip_prefix("ask-").unwrap_or(ask_id)
    )
}

/// Validates an answer's proposals and writes each under `root`. Returns
/// the records.
///
/// # Errors
///
/// Returns a message when a record can't be written.
pub fn record_all(answer: &Value, context: &Context, root: &Path) -> Result<Vec<Value>, String> {
    let mut records = Vec::new();
    for (index, draft) in drafts(answer).iter().take(MAX_PROPOSALS).enumerate() {
        let id = id_for(&context.ask_id, index + 1);
        let (mut record, file) = validate(draft, &id, context);
        let dir = root.join(&id);
        std::fs::create_dir_all(&dir)
            .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
        if let Some((name, value)) = &file {
            crate::record::write_atomic(
                &dir.join(name),
                serde_json::to_string_pretty(value)
                    .map_err(|error| error.to_string())?
                    .as_bytes(),
            )?;
        }
        crate::record::write_atomic(
            &dir.join(PROPOSAL_FILE),
            serde_json::to_string_pretty(&record)
                .map_err(|error| error.to_string())?
                .as_bytes(),
        )?;
        record["dir"] = json!(dir.display().to_string());
        records.push(record);
    }
    Ok(records)
}

/// Reads a proposal by ID, or by a prefix of one, under `root`.
///
/// # Errors
///
/// Returns a message when none, or more than one, matches.
pub fn load(root: &Path, query: &str) -> Result<(PathBuf, Value), String> {
    let exact = root.join(query).join(PROPOSAL_FILE);
    let path = if exact.is_file() {
        exact
    } else {
        let mut found: Vec<PathBuf> = std::fs::read_dir(root)
            .map_err(|error| format!("cannot read {}: {error}", root.display()))?
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(query))
            .map(|entry| entry.path().join(PROPOSAL_FILE))
            .filter(|path| path.is_file())
            .collect();
        found.sort();
        match found.len() {
            0 => return Err(format!("no proposal {query} under {}", root.display())),
            1 => found.remove(0),
            n => {
                return Err(format!(
                    "{n} proposals start with {query}; give more of the ID"
                ));
            }
        }
    };
    let value: Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("{} isn't JSON: {error}", path.display()))?;
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    Ok((dir, value))
}

/// Why a proposal may not run yet, or `None` when a person approved this
/// exact proposal.
#[must_use]
pub fn unapproved(dir: &Path, proposal: &Value) -> Option<String> {
    if proposal["valid"] != true {
        return Some("the proposal didn't validate, so it can't run".to_string());
    }
    let sealed = proposal["digest"].as_str().unwrap_or_default();
    if digest(proposal) != sealed {
        return Some(
            "the proposal changed after it was written; its digest doesn't match".to_string(),
        );
    }
    let Ok(text) = std::fs::read_to_string(dir.join(DECISION_FILE)) else {
        return Some(format!(
            "nobody has approved it: run `gym coder proposals approve {}`",
            proposal["id"].as_str().unwrap_or("ID")
        ));
    };
    let decision: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if decision["proposal_digest"].as_str() != Some(sealed) {
        return Some("the decision names another version of the proposal".to_string());
    }
    match decision["verdict"].as_str() {
        Some("approved") => None,
        Some("rejected") => Some("a person rejected it".to_string()),
        _ => Some("its decision is unreadable".to_string()),
    }
}

#[cfg(test)]
mod tests;
