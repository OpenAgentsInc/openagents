//! `verify.method_conformance`: well-known methods, checked by their
//! definitions (issue #9653).
//!
//! The registry lives in the repository's `methods/` directory, one file
//! per method, each digested: the method's standard definition in plain
//! language with its citations, how to call a candidate implementation
//! (Python functions first), executable property checks derived from the
//! definition, the entry's provenance, and its admission record.
//!
//! The component has three steps, and only the middle one asks a model:
//!
//! 1. **Find.** Code lists candidate functions in the workspace's Python
//!    source: module functions and methods of module classes, with one to
//!    six parameters and at most [`MAX_BODY_LINES`] lines. The bound is
//!    structural; no name or phrase list decides what's a candidate.
//! 2. **Identify.** One narrow Jev Choice per candidate picks the registry
//!    entry the function implements, or `none`
//!    (`questions/method-conformance.json`). Jev reads the function and
//!    the entries' definitions. It's never asked whether the function is
//!    correct.
//! 3. **Check.** Code runs the entry's property checks against the
//!    function ([`runner.py`](runner.py)) through a [`Host`]: a writing
//!    boundary, the task's own container, or a container with no network.
//!    Every failed property becomes a typed [`Failure`]: the method, the
//!    function, the property, and what was observed against what was
//!    expected.
//!
//! The lean loop's switch is `executor.microluna.lean.method_conformance`,
//! absent from every manifest. The offline measurement is
//! `docs/terminal-bench/2026-09-25-method-conformance.md`.

pub mod cli;
pub mod offline;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::contract::host::Host;
use crate::component::jev::{self as jev_component, JevMode};
use crate::record::{Implementation, Recorder};

/// The component ID.
pub const COMPONENT: &str = "verify.method_conformance";

/// The schema of one workspace's report.
pub const REPORT_SCHEMA: &str = "openagents.coder-one.method-conformance.v1";

/// The registry entry body version this reads.
pub const ENTRY_VERSION: u32 = 1;

/// Whether a policy may switch the component on. The offline measurement
/// decides; see `docs/terminal-bench/2026-09-25-method-conformance.md`.
pub const ADMITTED: bool = false;

/// The Python property runner the host runs once per candidate and entry.
pub const RUNNER: &str = include_str!("runner.py");

/// The line prefix the runner writes its result after.
pub const MARK: &str = "CONFORMANCE-RESULT:";

/// Source files read, at most.
pub const MAX_FILES: usize = 60;

/// Candidates per workspace, at most, in file order.
pub const MAX_CANDIDATES: usize = 60;

/// The longest function body a candidate may have, in lines. A
/// well-known method's implementation is short; a longer function is
/// orchestration that may call one.
pub const MAX_BODY_LINES: usize = 80;

/// Parameters a candidate may take, not counting `self` or `cls`.
pub const PARAMETERS: std::ops::RangeInclusive<usize> = 1..=6;

/// Candidates per Jev request.
pub const BATCH: usize = 6;

/// One candidate's check, all properties together, in seconds.
pub const CHECK_SEC: u64 = 60;

/// The registry files, by name, as the repository holds them.
pub const FILES: [(&str, &str); 11] = [
    (
        "cosine-distance.json",
        include_str!("../../../../../methods/cosine-distance.json"),
    ),
    (
        "cosine-similarity.json",
        include_str!("../../../../../methods/cosine-similarity.json"),
    ),
    (
        "debounce.json",
        include_str!("../../../../../methods/debounce.json"),
    ),
    (
        "euclidean-distance.json",
        include_str!("../../../../../methods/euclidean-distance.json"),
    ),
    (
        "ks-two-sample-statistic.json",
        include_str!("../../../../../methods/ks-two-sample-statistic.json"),
    ),
    (
        "l2-normalize.json",
        include_str!("../../../../../methods/l2-normalize.json"),
    ),
    (
        "levenshtein-distance.json",
        include_str!("../../../../../methods/levenshtein-distance.json"),
    ),
    (
        "mmd-squared-unbiased.json",
        include_str!("../../../../../methods/mmd-squared-unbiased.json"),
    ),
    (
        "pearson-correlation.json",
        include_str!("../../../../../methods/pearson-correlation.json"),
    ),
    (
        "population-stability-index.json",
        include_str!("../../../../../methods/population-stability-index.json"),
    ),
    (
        "softmax.json",
        include_str!("../../../../../methods/softmax.json"),
    ),
];

/// The question set's file text.
pub const QUESTION_SET: &str = include_str!("../../../../../questions/method-conformance.json");

/// What a registry entry's candidate returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultKind {
    Scalar,
    Vector,
    Sequence,
}

/// How the runner passes a property's inputs to a candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form {
    /// The JSON values as Python lists, numbers, and strings.
    Plain,
    /// Numeric lists as NumPy arrays of floats.
    Numpy,
    /// Each one-dimensional numeric list as a one-row NumPy matrix, and the
    /// result's first row as the result.
    Rows,
    /// A class built with no arguments whose method takes one sample at a
    /// time, for a sequence property.
    Stepwise,
}

/// How to call a candidate implementation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    /// The adapter's language; `python` is the only one so far.
    pub language: String,
    /// What the candidate takes, in words.
    pub inputs: String,
    pub result: ResultKind,
    /// The forms to try, in order; the first that the first property's
    /// call accepts is used for every property.
    pub forms: Vec<Form>,
    /// Values passed for required positional parameters the property
    /// doesn't supply, such as a kernel bandwidth or a sample count.
    #[serde(default)]
    pub fill: Vec<Value>,
}

/// One executable property derived from the definition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Property {
    pub id: String,
    /// The property, in words.
    pub says: String,
    /// The call's arguments.
    pub args: Vec<Value>,
    /// The result is this value, within the entry's tolerance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<Value>,
    /// The result equals the result for these arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_as: Option<Vec<Value>>,
    /// The result is greater than the result for these arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub greater_than: Option<Vec<Value>>,
    /// The result is below this number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub below: Option<f64>,
    /// The resulting sequence changes state this many times.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<u32>,
    /// Reduce a vector result before comparing: `sum`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduce: Option<String>,
}

impl Property {
    fn checks(&self) -> usize {
        usize::from(self.expect.is_some())
            + usize::from(self.same_as.is_some())
            + usize::from(self.greater_than.is_some())
            + usize::from(self.below.is_some())
            + usize::from(self.changes.is_some())
    }
}

/// Why an entry exists and where it came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub added: String,
    pub issue: String,
    pub why: String,
    pub pattern: String,
    /// The tasks the entry was learned from. They never count as evidence
    /// for it.
    pub source_tasks: Vec<String>,
}

/// Whether a policy may use the entry, and the measurement that says so.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub status: String,
    #[serde(default)]
    pub policies: Vec<String>,
    pub evidence: Vec<String>,
}

/// One well-known method.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub v: u32,
    pub id: String,
    pub slug: String,
    pub name: String,
    /// The standard definition, in plain language.
    pub definition: String,
    /// Textbooks or references, never a benchmark task.
    pub citations: Vec<String>,
    pub call: Call,
    pub tolerance: f64,
    pub properties: Vec<Property>,
    pub provenance: Provenance,
    pub admission: Admission,
}

impl Entry {
    /// Whether the entry is one this host runs.
    ///
    /// # Errors
    ///
    /// The first reason it isn't.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != ENTRY_VERSION {
            return Err(format!(
                "{}: body version {}, this reads {ENTRY_VERSION}",
                self.slug, self.v
            ));
        }
        if self.id != format!("openagents.method.{}.v{}", self.slug, self.v) {
            return Err(format!(
                "{}: the id {} doesn't name the slug",
                self.slug, self.id
            ));
        }
        if self.definition.trim().is_empty() || self.citations.is_empty() {
            return Err(format!("{}: a definition needs its citations", self.slug));
        }
        if self.call.language != "python" {
            return Err(format!(
                "{}: no adapter for {}",
                self.slug, self.call.language
            ));
        }
        if self.call.forms.is_empty() {
            return Err(format!("{}: no form to call a candidate in", self.slug));
        }
        if !(self.tolerance > 0.0 && self.tolerance < 1e-2) {
            return Err(format!(
                "{}: tolerance {} is out of range",
                self.slug, self.tolerance
            ));
        }
        if self.properties.len() < 3 {
            return Err(format!("{}: fewer than three properties", self.slug));
        }
        let mut seen = BTreeSet::new();
        for property in &self.properties {
            if !seen.insert(property.id.as_str()) {
                return Err(format!("{}: property {} twice", self.slug, property.id));
            }
            if property.checks() != 1 {
                return Err(format!(
                    "{}: property {} must state exactly one check",
                    self.slug, property.id
                ));
            }
            if property.changes.is_some() != (self.call.result == ResultKind::Sequence) {
                return Err(format!(
                    "{}: property {} counts changes only on a sequence result",
                    self.slug, property.id
                ));
            }
        }
        if self.admission.evidence.is_empty() {
            return Err(format!(
                "{}: an admission record names its evidence",
                self.slug
            ));
        }
        Ok(())
    }
}

/// An entry with its digest.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    pub entry: Entry,
    /// `atif::digest` of the file's JSON.
    pub digest: String,
}

/// The registry: every entry, in slug order, and a digest over them.
#[derive(Clone, Debug, PartialEq)]
pub struct Registry {
    pub entries: Vec<Loaded>,
    /// `atif::digest` of each slug's digest.
    pub digest: String,
}

impl Registry {
    /// Reads registry files.
    ///
    /// # Errors
    ///
    /// The first file that doesn't read or validate.
    pub fn from_texts<'a>(
        files: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<Self, String> {
        let mut entries = Vec::new();
        for (name, text) in files {
            let value: Value =
                serde_json::from_str(text).map_err(|e| format!("methods/{name}: {e}"))?;
            let entry: Entry = serde_json::from_value(value.clone())
                .map_err(|e| format!("methods/{name}: {e}"))?;
            entry
                .validate()
                .map_err(|e| format!("methods/{name}: {e}"))?;
            if name != format!("{}.json", entry.slug) {
                return Err(format!("methods/{name}: the file isn't named for its slug"));
            }
            entries.push(Loaded {
                entry,
                digest: atif::digest(&value),
            });
        }
        entries.sort_by(|a, b| a.entry.slug.cmp(&b.entry.slug));
        let digests: BTreeMap<&str, &str> = entries
            .iter()
            .map(|l| (l.entry.slug.as_str(), l.digest.as_str()))
            .collect();
        if digests.len() != entries.len() {
            return Err("two registry entries share a slug".to_string());
        }
        Ok(Registry {
            digest: atif::digest(&json!(digests)),
            entries,
        })
    }

    /// The entry named `slug`.
    #[must_use]
    pub fn get(&self, slug: &str) -> Option<&Loaded> {
        self.entries.iter().find(|l| l.entry.slug == slug)
    }
}

/// The registry this binary carries.
#[must_use]
pub fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Registry::from_texts(FILES).expect("the methods registry reads and validates")
    })
}

/// The identifying question set, as the repository holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestionSet {
    pub id: String,
    /// `atif::digest` of `{"per_finding": template}`, the digest the
    /// question-set registry in `crates/coder` computes.
    pub digest: String,
    pub instructions: String,
    /// The `none` option's wording.
    pub none: String,
    /// The template's decision block, outside the digest; empty when the
    /// file carries none, and the model's pick stands.
    pub decision: ::jev::Decision,
}

/// What the template writes the candidate's state path into.
pub const FINDING: &str = "{finding}";

/// The question set.
#[must_use]
pub fn question_set() -> &'static QuestionSet {
    static SET: OnceLock<QuestionSet> = OnceLock::new();
    SET.get_or_init(|| {
        let value: Value =
            serde_json::from_str(QUESTION_SET).expect("the method question set is JSON");
        let mut template = value["per_finding"].clone();
        let decision = ::jev::decision::split(&mut template)
            .expect("the method question set's decision block reads")
            .unwrap_or_default();
        QuestionSet {
            id: value["id"].as_str().unwrap_or_default().to_string(),
            digest: atif::digest(&json!({ "per_finding": template })),
            instructions: template["instructions"]
                .as_str()
                .expect("the method question set has instructions")
                .to_string(),
            none: template["criteria"]["none"]
                .as_str()
                .expect("the method question set words none")
                .to_string(),
            decision,
        }
    })
}

/// A function code found.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    /// Relative to the workspace.
    pub file: String,
    pub line: usize,
    /// `name`, or `Class.name` for a method.
    pub qualname: String,
    pub signature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docstring: Option<String>,
    /// The source, clipped.
    pub body: String,
    /// `atif::digest` of what Jev reads for it.
    pub digest: String,
}

impl Candidate {
    /// What Jev reads for this candidate.
    #[must_use]
    pub fn state(&self) -> Value {
        json!({
            "file": self.file,
            "function": self.qualname,
            "signature": self.signature,
            "docstring": self.docstring,
            "source": self.body,
        })
    }

    /// `file:line qualname`.
    #[must_use]
    pub fn label(&self) -> String {
        format!("{}:{} {}", self.file, self.line, self.qualname)
    }
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The parameters a signature names, less `self`, `cls`, `*args`,
/// `**kwargs`, and the bare `*` and `/` markers.
#[must_use]
pub fn parameters(signature: &str) -> Vec<String> {
    let Some(open) = signature.find('(') else {
        return Vec::new();
    };
    let mut depth = 0i32;
    let mut end = signature.len();
    for (i, c) in signature[open..].char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    end = open + i;
                    break;
                }
            }
            _ => {}
        }
    }
    let inner = &signature[open + 1..end.max(open + 1)];
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for c in inner.chars().chain(std::iter::once(',')) {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                let name = current
                    .split([':', '='])
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                current.clear();
                if name.is_empty()
                    || name == "self"
                    || name == "cls"
                    || name == "*"
                    || name == "/"
                    || name.starts_with('*')
                {
                    continue;
                }
                out.push(name);
            }
            _ => current.push(c),
        }
    }
    out
}

/// The class chain that encloses line `at` (0-based) of `lines`, or
/// `None` when a function encloses it.
fn enclosing(lines: &[&str], at: usize) -> Option<Vec<String>> {
    let mut indent = indent_of(lines[at]);
    let mut chain = Vec::new();
    for line in lines[..at].iter().rev() {
        if indent == 0 {
            break;
        }
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let here = indent_of(line);
        if here >= indent {
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
            return None;
        }
        if let Some(rest) = trimmed.strip_prefix("class ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            chain.push(name);
        } else {
            // An `if` or `try` block at module level still reaches the
            // module's names; any other block is treated the same.
        }
        indent = here;
    }
    chain.reverse();
    Some(chain)
}

/// Every candidate function in `workspace`'s Python source, in file
/// order, at most [`MAX_CANDIDATES`].
#[must_use]
pub fn candidates(workspace: &Path) -> Vec<Candidate> {
    let mut out = Vec::new();
    for path in crate::accept::source_files(workspace, MAX_FILES) {
        if !path.ends_with(".py") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(workspace.join(&path)) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        for f in crate::departures::python_functions(&path, &text) {
            let at = f.line - 1;
            let head = lines.get(at).map_or("", |l| l.trim_start());
            if head.starts_with("class ") {
                continue;
            }
            if f.name.starts_with("__") && f.name.ends_with("__") {
                continue;
            }
            let Some(chain) = enclosing(&lines, at) else {
                continue;
            };
            if !PARAMETERS.contains(&parameters(&f.signature).len()) {
                continue;
            }
            let length = lines[at..]
                .iter()
                .skip(1)
                .take_while(|l| l.trim().is_empty() || indent_of(l) > indent_of(lines[at]))
                .count()
                + 1;
            if length > MAX_BODY_LINES {
                continue;
            }
            let qualname = chain
                .into_iter()
                .chain(std::iter::once(f.name.clone()))
                .collect::<Vec<_>>()
                .join(".");
            let mut candidate = Candidate {
                file: path.clone(),
                line: f.line,
                qualname,
                signature: f.signature.clone(),
                docstring: f.docstring.clone(),
                body: f.body.clone(),
                digest: String::new(),
            };
            candidate.digest = atif::digest(&candidate.state());
            out.push(candidate);
            if out.len() >= MAX_CANDIDATES {
                return out;
            }
        }
    }
    out
}

/// One Jev request over a slice of candidates.
pub struct Request {
    /// Index of the request's first candidate.
    pub offset: usize,
    pub state: Value,
    pub questions: ::jev::Questions,
    /// Each question's ID, by candidate within the request.
    pub ids: Vec<String>,
}

/// The question for candidate `j` of a request.
#[must_use]
pub fn question(j: usize) -> ::jev::Choice {
    let set = question_set();
    let mut choice = ::jev::Choice::new(
        set.instructions
            .replace(FINDING, &format!("functions[{j}]"))
            .as_str(),
        indexmap::IndexMap::new(),
    );
    for loaded in &registry().entries {
        choice = choice.option(loaded.entry.slug.clone(), loaded.entry.name.clone());
    }
    choice.option("none", set.none.clone())
}

/// The requests that identify `candidates`, [`BATCH`] to a request.
#[must_use]
pub fn requests(candidates: &[Candidate]) -> Vec<Request> {
    let methods: Vec<Value> = registry()
        .entries
        .iter()
        .map(|l| {
            json!({
                "option": l.entry.slug,
                "name": l.entry.name,
                "definition": l.entry.definition,
            })
        })
        .collect();
    candidates
        .chunks(BATCH)
        .enumerate()
        .map(|(n, chunk)| {
            let mut questions = ::jev::Questions::new();
            let mut ids = Vec::new();
            for j in 0..chunk.len() {
                let id = format!("method_{j}");
                questions = questions.with(id.clone(), question(j));
                ids.push(id);
            }
            Request {
                offset: n * BATCH,
                state: json!({
                    "methods": methods,
                    "functions": chunk.iter().map(Candidate::state).collect::<Vec<_>>(),
                }),
                questions,
                ids,
            }
        })
        .collect()
}

/// What identifying produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Identified {
    /// Per candidate: the entry's slug, or `None` for `none` or no answer.
    pub methods: Vec<Option<String>>,
    /// Per candidate: whether Jev answered.
    pub answered: Vec<bool>,
    pub usd: f64,
    /// One record per request: how it was answered, its key, answers, and
    /// tokens, so a live run can be kept for replay.
    pub calls: Vec<Value>,
}

/// Where identifying runs.
pub struct Context<'a> {
    pub component: &'a str,
    pub id: String,
    pub deadline: Option<crate::deadline::Deadline>,
}

/// The recorded-answer key of a request, as [`jev_component::ask`]
/// computes it.
#[must_use]
pub fn request_key(state: &Value, questions: &::jev::Questions) -> String {
    let request =
        ::jev::SystemOneRequest::new(::jev::Entry::from(state.clone()), questions.clone());
    let body = request
        .body(crate::credentials::JEV_MODEL)
        .map(Value::Object)
        .unwrap_or_else(|_| json!({}));
    jev_component::key(
        body.get("state").unwrap_or(&Value::Null),
        body.get("questions").unwrap_or(&Value::Null),
    )
}

/// Has Jev pick the entry each candidate implements, or none.
pub async fn identify(
    jev: &JevMode,
    recorder: &Recorder,
    context: &Context<'_>,
    candidates: &[Candidate],
) -> Identified {
    identify_with(jev, None, recorder, context, candidates).await
}

/// [`identify`], answering from `replay`'s recorded answers where it has
/// the request and from `jev` otherwise.
pub async fn identify_with(
    jev: &JevMode,
    replay: Option<&JevMode>,
    recorder: &Recorder,
    context: &Context<'_>,
    candidates: &[Candidate],
) -> Identified {
    let mut out = Identified {
        methods: vec![None; candidates.len()],
        answered: vec![false; candidates.len()],
        ..Identified::default()
    };
    for (n, request) in requests(candidates).into_iter().enumerate() {
        let mode = match replay {
            Some(JevMode::Recorded(recorded))
                if recorded
                    .entries
                    .contains_key(&request_key(&request.state, &request.questions)) =>
            {
                replay.unwrap_or(jev)
            }
            _ => jev,
        };
        let asked = jev_component::ask(
            mode,
            recorder,
            jev_component::Ask {
                component: context.component,
                name: "jev_method_conformance",
                id: format!("{}-{n}", context.id),
                state: request.state,
                questions: request.questions,
                parent: None,
                deadline: context.deadline.clone(),
            },
        )
        .await;
        if asked.how == "live" {
            out.usd += asked.input_tokens.map_or(0.0, |t| {
                t as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0
            });
        }
        let decision = &question_set().decision;
        for (j, id) in request.ids.iter().enumerate() {
            let at = request.offset + j;
            if let Some(choice) = asked.decided_choice(id, decision) {
                out.answered[at] = true;
                if registry().get(choice).is_some() {
                    out.methods[at] = Some(choice.to_string());
                }
            }
        }
        out.calls.push(json!({
            "how": asked.how,
            "error": asked.error,
            "key": asked.key,
            "answers": asked.answers,
            "input_tokens": asked.input_tokens,
            "output_tokens": asked.output_tokens,
            "milliseconds": asked.milliseconds,
        }));
    }
    out
}

/// How one candidate's check ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Every property ran; each passed or failed.
    Ran,
    /// No form of the first property's call returned a result of the
    /// entry's kind.
    CouldNotCall,
    /// The candidate's module didn't import.
    ImportFailed,
    /// The host couldn't run the runner, or it wrote no result.
    HostFailed,
}

/// One property's outcome.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropertyResult {
    pub id: String,
    pub says: String,
    /// `passed` or `failed`.
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
}

/// One candidate checked against one entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checked {
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<Form>,
    #[serde(default)]
    pub properties: Vec<PropertyResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub milliseconds: u64,
}

impl Checked {
    /// The properties that failed.
    pub fn failed(&self) -> impl Iterator<Item = &PropertyResult> {
        self.properties.iter().filter(|p| p.status != "passed")
    }

    /// `pass` when every property ran and passed, `fail` when one
    /// failed, and `unknown` when the check couldn't run.
    #[must_use]
    pub fn verdict(&self) -> &'static str {
        match self.status {
            Status::Ran if self.failed().next().is_some() => "fail",
            Status::Ran => "pass",
            _ => "unknown",
        }
    }
}

/// The shell command that runs `entry`'s properties against `candidate`
/// from `workdir`.
#[must_use]
pub fn command(workdir: &str, candidate: &Candidate, entry: &Entry) -> String {
    let spec = json!({
        "workdir": workdir,
        "file": candidate.file,
        "qualname": candidate.qualname,
        "call": entry.call,
        "tolerance": entry.tolerance,
        "properties": entry.properties,
    });
    let runner = base64(RUNNER.as_bytes());
    let spec = base64(spec.to_string().as_bytes());
    format!(
        "P=$(command -v python3 || command -v python) && PYTHONDONTWRITEBYTECODE=1 exec \"$P\" -B -c \
         \"import base64;exec(base64.b64decode('{runner}'))\" '{spec}'"
    )
}

/// Standard base64 with padding, which carries the runner and its spec
/// through `sh -c` without quoting trouble.
#[must_use]
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn clip(text: &str) -> String {
    crate::judge::clip(text.trim(), 600)
}

/// Runs `entry`'s properties against `candidate` through `host`, whose
/// working directory is `workdir`.
pub async fn check<H: Host>(
    host: &H,
    workdir: &str,
    candidate: &Candidate,
    entry: &Entry,
) -> Checked {
    let started = Instant::now();
    let ran = host
        .run(
            &command(workdir, candidate, entry),
            Duration::from_secs(CHECK_SEC),
        )
        .await;
    let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let failed = |why: String| Checked {
        status: Status::HostFailed,
        form: None,
        properties: Vec::new(),
        error: Some(why),
        milliseconds,
    };
    if let Some(why) = ran.failed {
        return failed(why);
    }
    if ran.timed_out {
        return failed(format!("the check didn't finish within {CHECK_SEC} s"));
    }
    let Some(line) = ran.stdout.lines().rev().find_map(|l| l.strip_prefix(MARK)) else {
        return failed(format!(
            "the runner wrote no result (exit {:?}): {}",
            ran.exit,
            clip(&ran.stderr)
        ));
    };
    let value: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => return failed(format!("the runner's result didn't read: {error}")),
    };
    let status = match value["status"].as_str() {
        Some("ran") => Status::Ran,
        Some("could_not_call") => Status::CouldNotCall,
        Some("import_failed") => Status::ImportFailed,
        _ => Status::HostFailed,
    };
    let error = value["error"].as_str().map(str::to_string).or_else(|| {
        value["tried"].as_array().map(|tried| {
            tried
                .iter()
                .map(|t| {
                    format!(
                        "{}: {}",
                        t["form"].as_str().unwrap_or("?"),
                        t["error"].as_str().unwrap_or("?")
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        })
    });
    Checked {
        status,
        form: serde_json::from_value(value["form"].clone()).ok(),
        properties: serde_json::from_value(value["properties"].clone()).unwrap_or_default(),
        error,
        milliseconds,
    }
}

/// One failed property, as evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Failure {
    /// The entry's slug.
    pub method: String,
    pub method_name: String,
    pub method_digest: String,
    /// `file:line qualname`.
    pub function: String,
    pub property: String,
    pub says: String,
    pub observed: String,
    pub expected: String,
}

/// One candidate Jev tied to an entry, and its check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub file: String,
    pub line: usize,
    pub qualname: String,
    pub method: String,
    pub checked: Checked,
}

/// One workspace's report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub schema: String,
    pub component: String,
    pub registry: String,
    pub question_set: String,
    pub candidates: usize,
    /// Candidates Jev answered for.
    pub answered: usize,
    pub rows: Vec<Row>,
    pub failures: Vec<Failure>,
    pub jev_usd: f64,
    #[serde(default)]
    pub jev: Vec<Value>,
}

/// The failures in `rows`.
#[must_use]
pub fn failures(rows: &[Row]) -> Vec<Failure> {
    let mut out = Vec::new();
    for row in rows {
        let Some(loaded) = registry().get(&row.method) else {
            continue;
        };
        for property in row.checked.failed() {
            out.push(Failure {
                method: row.method.clone(),
                method_name: loaded.entry.name.clone(),
                method_digest: loaded.digest.clone(),
                function: format!("{}:{} {}", row.file, row.line, row.qualname),
                property: property.id.clone(),
                says: property.says.clone(),
                observed: property.observed.clone().unwrap_or_default(),
                expected: property.expected.clone().unwrap_or_default(),
            });
        }
    }
    out
}

/// Finds, identifies, and checks one workspace. Code reads the source at
/// `local`; `host` runs the checks from `workdir`, the same files.
pub async fn run<H: Host>(
    jev: &JevMode,
    recorder: &Recorder,
    context: &Context<'_>,
    local: &Path,
    host: &H,
    workdir: &str,
) -> Report {
    run_with(jev, None, recorder, context, local, host, workdir).await
}

/// [`run`], answering from `replay` where it holds the request.
pub async fn run_with<H: Host>(
    jev: &JevMode,
    replay: Option<&JevMode>,
    recorder: &Recorder,
    context: &Context<'_>,
    local: &Path,
    host: &H,
    workdir: &str,
) -> Report {
    let found = candidates(local);
    let identified = identify_with(jev, replay, recorder, context, &found).await;
    let mut rows = Vec::new();
    for (candidate, method) in found.iter().zip(&identified.methods) {
        let Some(slug) = method else {
            continue;
        };
        let Some(loaded) = registry().get(slug) else {
            continue;
        };
        let checked = check(host, workdir, candidate, &loaded.entry).await;
        rows.push(Row {
            file: candidate.file.clone(),
            line: candidate.line,
            qualname: candidate.qualname.clone(),
            method: slug.clone(),
            checked,
        });
    }
    Report {
        schema: REPORT_SCHEMA.to_string(),
        component: COMPONENT.to_string(),
        registry: registry().digest.clone(),
        question_set: question_set().digest.clone(),
        candidates: found.len(),
        answered: identified.answered.iter().filter(|a| **a).count(),
        failures: failures(&rows),
        rows,
        jev_usd: identified.usd,
        jev: identified.calls,
    }
}

/// The briefing section's name.
pub const LABEL: &str = "Well-known methods that fail their definitions";

/// What a session is told about the failures.
pub const NOTE: &str = "The host found functions that implement well-known methods and ran \
checks derived from each method's standard definition against them. Each line below is a check \
that failed: the method, the function, the property, and what the function returned against \
what the definition gives. A failed check is an observation, not an instruction; decide each one \
against the task.";

/// The most failing functions the briefing lists.
pub const MAX_LISTED: usize = 12;

/// The most failed properties the briefing details per function.
pub const MAX_DETAILS: usize = 3;

/// The briefing entry for `report`, or `None` when nothing failed.
#[must_use]
pub fn evidence(report: &Report) -> Option<microluna::Evidence> {
    if report.failures.is_empty() {
        return None;
    }
    let mut lines = Vec::new();
    for row in report.rows.iter().filter(|r| r.checked.verdict() == "fail") {
        let Some(loaded) = registry().get(&row.method) else {
            continue;
        };
        let failed: Vec<&PropertyResult> = row.checked.failed().collect();
        let details: Vec<String> = failed
            .iter()
            .take(MAX_DETAILS)
            .map(|p| {
                format!(
                    "`{}`: {} Observed {}; expected {}.",
                    p.id,
                    p.says,
                    p.observed.as_deref().unwrap_or("nothing"),
                    p.expected.as_deref().unwrap_or("nothing")
                )
            })
            .collect();
        lines.push(format!(
            "- `{}` ({}:{}) implements {} (`{}`) and fails {} of {} checks. {}",
            row.qualname,
            row.file,
            row.line,
            loaded.entry.name,
            row.method,
            failed.len(),
            row.checked.properties.len(),
            details.join(" ")
        ));
        if lines.len() >= MAX_LISTED {
            break;
        }
    }
    Some(microluna::Evidence {
        label: LABEL.to_string(),
        text: format!("{NOTE}\n\n{}", lines.join("\n")),
    })
}

/// The implementation record: the registry, the question set, and the
/// bounds, by digest or value.
#[must_use]
pub fn implementation() -> Implementation {
    let set = question_set();
    Implementation::new(
        COMPONENT,
        "registry entries identified by one Jev Choice, then checked by code",
        &json!({
            "registry": registry().digest,
            "entries": registry()
                .entries
                .iter()
                .map(|l| json!({ "slug": l.entry.slug, "digest": l.digest }))
                .collect::<Vec<_>>(),
            "question_set": { "id": set.id, "digest": set.digest },
            "runner": atif::digest(&json!(RUNNER)),
            "max_candidates": MAX_CANDIDATES,
            "max_body_lines": MAX_BODY_LINES,
            "batch": BATCH,
            "check_sec": CHECK_SEC,
        }),
    )
}

/// The lean loop's step: finds, identifies, and checks a scratch copy of
/// `workdir` before session 1. In a task container, which can't enforce a
/// writing boundary, the checks run unconfined in the copy; elsewhere,
/// inside a boundary on it. Returns the briefing section, the loop's
/// record, and Jev's cost.
pub async fn lean_step(
    jev: &JevMode,
    recorder: &Recorder,
    context: &Context<'_>,
    workdir: &Path,
    contained: bool,
) -> (Option<microluna::Evidence>, Value, f64) {
    let scratch = std::env::temp_dir().join(format!(
        "coder-one-conformance-{}-{}",
        std::process::id(),
        atif::now_ms()
    ));
    let copied = crate::handoff::copy_tree(workdir, &scratch);
    let record = |report: Option<&Report>, error: Option<String>| {
        json!({
            "kind": "lean.method_conformance",
            "implementation": implementation(),
            "report": report,
            "error": error,
        })
    };
    if let Err(error) = copied {
        let _ = std::fs::remove_dir_all(&scratch);
        return (None, record(None, Some(error)), 0.0);
    }
    let alias = scratch.display().to_string();
    let report = if contained {
        let host = super::contract::host::Contained {
            workdir: scratch.clone(),
        };
        run(jev, recorder, context, workdir, &host, &alias).await
    } else {
        let host = super::contract::host::Local {
            workdir: scratch.clone(),
        };
        run(jev, recorder, context, workdir, &host, &alias).await
    };
    let _ = std::fs::remove_dir_all(&scratch);
    let usd = report.jev_usd;
    (evidence(&report), record(Some(&report), None), usd)
}
