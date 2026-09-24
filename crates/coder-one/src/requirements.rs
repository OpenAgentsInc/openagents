//! `task.requirements`: a requirement map from the task's own words, each
//! entry tied to its source span.
//!
//! The extractor works in three steps:
//!
//! 1. [`split`] cuts the instruction into identified spans: sentences, list
//!    items, checkbox lines, short lines, code blocks, and blocks of format
//!    rows. Together they cover every non-space character of the
//!    instruction; [`Coverage`] says so, and a map that did not would say
//!    that too.
//! 2. [`extract`] reads each span's exact paths, commands, formats, and
//!    constants mechanically.
//! 3. Jev reads each span in the context of the whole instruction and says
//!    what it states: a deliverable, a behavior, a constraint, a check, or
//!    context. For a span that looks like an example, it also says whether
//!    the example is exhaustive.
//!
//! A span Jev reads as binding becomes a requirement. A span it is unsure
//! of stays a requirement marked `uncertain`, and a span it reads as
//! context stays in the map as context. No instruction text is dropped.
//! Without Jev, [`mechanical`] keeps every span that states or implies
//! something to do, marked `unjudged`.

use std::collections::BTreeMap;

use jev::{Choice, Noul, Questions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::component::evidence::issue_state;
use crate::component::jev::{Ask, Asked, JevMode};
use crate::record::{Implementation, Recorder};

/// The schema of a requirement map.
pub const SCHEMA: &str = "openagents.coder-one.requirements.v1";

/// The extractor's tunable parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Params {
    /// A binding probability at or above this makes a requirement.
    pub yes: f64,
    /// A binding probability at or above this, and below `yes`, keeps the
    /// span as an uncertain requirement.
    pub keep: f64,
    /// The most spans one Jev request reads.
    pub batch: usize,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            yes: 0.5,
            keep: 0.2,
            batch: 20,
        }
    }
}

/// The question Jev answers for each span.
pub const KIND_QUESTION: &str = "Read the span `spans[{i}].text` as part of the whole task in `issue.body`. What does this span state about the finished work?";

/// The exhaustiveness question for a span that looks like an example.
pub const EXHAUSTIVE_QUESTION: &str = "The span `spans[{i}].text` gives an example, a sample, or a format. Must the finished work match it completely, every row, key, or value it shows, rather than treat it as one illustration of a pattern?";

/// The kinds a span can state.
pub const KINDS: [(&str, &str); 5] = [
    (
        "deliverable",
        "An artifact the finished work must produce or change: a file, a function, an installed package, a merge, or a recovered value.",
    ),
    (
        "behavior",
        "How the delivered work must behave or what result it must compute.",
    ),
    (
        "constraint",
        "A limit on how the work is done or what the result looks like: an exact name, path, format, value, version, or something to leave unchanged.",
    ),
    (
        "check",
        "A test, command, or snippet the finished work must pass or run.",
    ),
    (
        "context",
        "Background, a fact about the environment, a hint, a permission, or advice that the finished work is not checked against.",
    ),
];

/// The extractor's identity and parameters, digested.
#[must_use]
pub fn implementation(params: Params, jev: bool) -> Implementation {
    Implementation::new(
        "task.requirements",
        if jev {
            "span kinds by Jev"
        } else {
            "span kinds by rule"
        },
        &json!({
            "params": params,
            "kind_question": KIND_QUESTION,
            "exhaustive_question": EXHAUSTIVE_QUESTION,
            "kinds": KINDS.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            "splitter": "sentences, list items, checkboxes, short lines, code blocks, format-row blocks",
        }),
    )
}

/// How a span was cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form {
    Sentence,
    /// A short line that is not a sentence, such as a heading or one item
    /// of an unmarked list.
    Line,
    ListItem,
    Checkbox,
    /// A fenced code block.
    Code,
    /// Consecutive format rows, such as a CSV header and its rows.
    Block,
}

/// One identified span of the instruction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub id: String,
    /// Byte offsets into the instruction.
    pub start: usize,
    pub end: usize,
    pub form: Form,
    pub text: String,
}

/// What a span names exactly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extracted {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub formats: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constants: Vec<String>,
}

impl Extracted {
    fn is_empty(&self) -> bool {
        self.paths.is_empty()
            && self.commands.is_empty()
            && self.formats.is_empty()
            && self.constants.is_empty()
    }
}

/// What a span states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Deliverable,
    Behavior,
    Constraint,
    Check,
    Context,
}

impl Kind {
    fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "deliverable" => Kind::Deliverable,
            "behavior" => Kind::Behavior,
            "constraint" => Kind::Constraint,
            "check" => Kind::Check,
            "context" => Kind::Context,
            _ => return None,
        })
    }

    /// The kind as the map spells it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Kind::Deliverable => "deliverable",
            Kind::Behavior => "behavior",
            Kind::Constraint => "constraint",
            Kind::Check => "check",
            Kind::Context => "context",
        }
    }
}

/// How sure the map is that a requirement binds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Binding {
    /// Jev read it as binding.
    Yes,
    /// Jev was unsure; kept so no task text is lost.
    Uncertain,
    /// No Jev answer; kept by rule.
    Unjudged,
}

/// What is known about a requirement's satisfaction. Extraction leaves
/// every requirement `unobserved`; a check that observes it changes that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Unobserved,
    Observed,
    Contradicted,
    Unverifiable,
}

/// One requirement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Requirement {
    /// `R1`, `R2`, … in instruction order.
    pub id: String,
    /// The spans it comes from.
    pub spans: Vec<String>,
    pub kind: Kind,
    pub binding: Binding,
    pub state: State,
    /// Jev's probability that the span binds: one minus its probability of
    /// context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p: Option<f64>,
    /// Jev's probability that an example span is exhaustive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exhaustive: Option<f64>,
    /// The span's text, verbatim.
    pub text: String,
    #[serde(default, skip_serializing_if = "Extracted::is_empty")]
    pub extracted: Extracted,
}

/// One span and what the map made of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Placed {
    #[serde(flatten)]
    pub span: Span,
    /// The requirement it became, or `None` when it is context.
    pub requirement: Option<String>,
    pub kind: Kind,
    /// Jev's probability of each kind, when Jev answered.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub kinds: BTreeMap<String, f64>,
    /// Whether Jev answered for this span.
    pub answered: bool,
}

/// How much of the instruction the spans cover and how it was placed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Coverage {
    /// Non-space characters in the instruction.
    pub chars: usize,
    /// Non-space characters inside some span.
    pub covered: usize,
    pub fraction: f64,
    pub spans: usize,
    pub requirement_spans: usize,
    pub context_spans: usize,
    /// Spans Jev did not answer for.
    pub unanswered_spans: usize,
}

/// The requirement map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequirementMap {
    pub schema: String,
    /// SHA-256 of the instruction, hex.
    pub instruction_sha256: String,
    /// `jev` or `rule`.
    pub method: String,
    pub spans: Vec<Placed>,
    pub requirements: Vec<Requirement>,
    pub coverage: Coverage,
}

impl RequirementMap {
    /// The requirements as short texts, for the closing check and the
    /// briefing: every kept requirement, clipped.
    #[must_use]
    pub fn criteria(&self, max: usize) -> Vec<String> {
        self.requirements
            .iter()
            .take(max)
            .map(|r| {
                crate::judge::clip(
                    &r.text.split_whitespace().collect::<Vec<_>>().join(" "),
                    300,
                )
            })
            .collect()
    }

    /// The map as a record.
    #[must_use]
    pub fn record(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

// ---------------------------------------------------------------------------
// Splitting
// ---------------------------------------------------------------------------

/// Cuts `text` into spans that together cover every non-space character.
#[must_use]
pub fn split(text: &str) -> Vec<Span> {
    let mut raw: Vec<(usize, usize, Form)> = Vec::new();
    let lines = lines(text);
    let mut i = 0;
    while i < lines.len() {
        let (start, end) = lines[i];
        let line = &text[start..end];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            i += 1;
            continue;
        }
        if trimmed.starts_with("```") {
            let mut j = i + 1;
            while j < lines.len() && !text[lines[j].0..lines[j].1].trim().starts_with("```") {
                j += 1;
            }
            let last = j.min(lines.len() - 1);
            raw.push((start, lines[last].1, Form::Code));
            i = last + 1;
            continue;
        }
        if let Some(form) = item(trimmed) {
            // An item runs on over indented lines that start no item.
            let indent = indent_of(line);
            let mut j = i + 1;
            while j < lines.len() {
                let next = &text[lines[j].0..lines[j].1];
                if next.trim().is_empty()
                    || item(next.trim()).is_some()
                    || indent_of(next) <= indent
                {
                    break;
                }
                j += 1;
            }
            raw.push((start, lines[j - 1].1, form));
            i = j;
            continue;
        }
        if format_row(trimmed) {
            let mut j = i + 1;
            while j < lines.len() && format_row(text[lines[j].0..lines[j].1].trim()) {
                j += 1;
            }
            raw.push((start, lines[j - 1].1, Form::Block));
            i = j;
            continue;
        }
        // A paragraph: prose lines until a blank line or another form. A
        // line ends its segment when it ends with a colon, or when it ends
        // without punctuation before a capitalized line, as the items of
        // an unmarked list do.
        let mut j = i;
        while j < lines.len() {
            let current = text[lines[j].0..lines[j].1].trim();
            let other =
                current.starts_with("```") || item(current).is_some() || format_row(current);
            if current.is_empty() || (j > i && other) {
                break;
            }
            j += 1;
        }
        let mut segment = start;
        for k in i..j {
            let current = text[lines[k].0..lines[k].1].trim();
            let next = if k + 1 < j {
                text[lines[k + 1].0..lines[k + 1].1].trim()
            } else {
                ""
            };
            let breaks = k + 1 == j
                || current.ends_with(':')
                || (!current.ends_with(['.', ',', ';', '?', '!'])
                    && next.starts_with(|c: char| c.is_uppercase()));
            if breaks {
                let before = raw.len();
                sentences(text, segment, lines[k].1, &mut raw);
                let one_line = segment == lines[k].0;
                if raw.len() == before + 1 && one_line && !current.ends_with(['.', '?', '!']) {
                    raw[before].2 = Form::Line;
                }
                segment = lines.get(k + 1).map_or(lines[k].1, |&(s, _)| s);
            }
        }
        i = j.max(i + 1);
    }
    raw.sort_by_key(|(start, ..)| *start);
    raw.dedup_by_key(|(start, ..)| *start);
    raw.into_iter()
        .filter_map(|(start, end, form)| {
            let slice = &text[start..end];
            let lead = slice.len() - slice.trim_start().len();
            let trail = slice.len() - slice.trim_end().len();
            let (start, end) = (start + lead, end - trail);
            (start < end).then_some((start, end, form))
        })
        .enumerate()
        .map(|(n, (start, end, form))| Span {
            id: format!("s{}", n + 1),
            start,
            end,
            form,
            text: text[start..end].to_string(),
        })
        .collect()
}

/// Each line's byte range, without its newline.
fn lines(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    for (index, c) in text.char_indices() {
        if c == '\n' {
            out.push((start, index));
            start = index + 1;
        }
    }
    if start < text.len() {
        out.push((start, text.len()));
    }
    out
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Whether a trimmed line starts a list item, and which kind.
fn item(line: &str) -> Option<Form> {
    for mark in ["- [ ]", "- [x]", "- [X]", "* [ ]", "* [x]"] {
        if line.starts_with(mark) {
            return Some(Form::Checkbox);
        }
    }
    if line.starts_with("- ") || line.starts_with("* ") || line.starts_with("+ ") {
        return Some(Form::ListItem);
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && digits <= 3 {
        let rest = &line[digits..];
        if rest.starts_with(". ") || rest.starts_with(") ") {
            return Some(Form::ListItem);
        }
    }
    None
}

/// Whether a trimmed line reads as a format row: no spaces, and fields
/// split by commas, pipes, or tabs.
fn format_row(line: &str) -> bool {
    !line.is_empty()
        && !line.contains(' ')
        && (line.matches(',').count() >= 1 || line.contains('|') || line.contains('\t'))
        && !line.ends_with('.')
}

/// Splits `text[start..end]` into sentences and pushes their ranges.
fn sentences(text: &str, start: usize, end: usize, out: &mut Vec<(usize, usize, Form)>) {
    let slice = &text[start..end];
    let bytes: Vec<(usize, char)> = slice.char_indices().collect();
    let mut from = 0;
    for (k, &(index, c)) in bytes.iter().enumerate() {
        if !matches!(c, '.' | '?' | '!') {
            continue;
        }
        let Some(&(next_index, next)) = bytes.get(k + 1) else {
            continue;
        };
        if !next.is_whitespace() {
            continue;
        }
        let after = slice[next_index..].trim_start();
        let Some(first) = after.chars().next() else {
            continue;
        };
        if !(first.is_uppercase() || "`\"'(".contains(first) || first.is_ascii_digit()) {
            continue;
        }
        // Not after an abbreviation such as "e.g." or a lone initial.
        let word = slice[from..index]
            .rsplit(|ch: char| ch.is_whitespace() || ch == '(')
            .next()
            .unwrap_or("")
            .to_lowercase();
        if ["e.g", "i.e", "etc", "vs", "cf", "dr", "mr", "ms", "no"].contains(&word.as_str())
            || word.len() == 1
        {
            continue;
        }
        out.push((start + from, start + index + c.len_utf8(), Form::Sentence));
        from = next_index;
    }
    if slice[from..].trim().is_empty() {
        return;
    }
    out.push((start + from, end, Form::Sentence));
}

// ---------------------------------------------------------------------------
// Mechanical extraction
// ---------------------------------------------------------------------------

/// Programs whose name opens a command.
const PROGRAMS: &[&str] = &[
    "git", "pip", "pip3", "python", "python3", "pytest", "bash", "sh", "make", "cargo", "npm",
    "node", "sqlite3", "curl", "wget", "apt", "apt-get", "cd", "ls", "cat", "gcc", "cmake", "go",
    "uv", "docker", "rustc", "javac", "java",
];

/// File extensions that mark a relative path.
const EXTENSIONS: &[&str] = &[
    "py", "csv", "json", "jsonl", "txt", "log", "db", "md", "toml", "yaml", "yml", "sh", "c",
    "cpp", "h", "rs", "js", "ts", "html", "so", "pyx", "ini", "cfg", "sql", "xml", "bib",
];

/// The exact paths, commands, formats, and constants a span names.
#[must_use]
pub fn extract(text: &str) -> Extracted {
    let mut out = Extracted::default();
    let push = |list: &mut Vec<String>, value: &str| {
        let value = value.trim();
        if !value.is_empty() && !list.iter().any(|seen| seen == value) {
            list.push(value.to_string());
        }
    };
    // Inline code spans.
    let pieces: Vec<&str> = text.split('`').collect();
    let fenced = text.trim_start().starts_with("```");
    if !fenced {
        for code in pieces.iter().skip(1).step_by(2) {
            let first = code.split_whitespace().next().unwrap_or("");
            if code.contains(' ') && (PROGRAMS.contains(&first) || first.starts_with("./")) {
                push(&mut out.commands, code);
            } else if looks_like_path(code) {
                push(&mut out.paths, code);
            } else {
                push(&mut out.constants, code);
            }
        }
    } else {
        let language = text
            .trim_start()
            .trim_start_matches('`')
            .split_whitespace()
            .next()
            .unwrap_or("");
        if ["sh", "bash", "shell", "console"].contains(&language) {
            for line in text
                .lines()
                .skip(1)
                .filter(|l| !l.trim_start().starts_with("```"))
            {
                push(&mut out.commands, line.trim().trim_start_matches("$ "));
            }
        }
    }
    // Paths, dates, versions, placeholders, and uppercase constants in the
    // prose.
    for token in text.split(|c: char| c.is_whitespace() || "`'\"(),;[]{}".contains(c)) {
        let token = token.trim_end_matches(['.', ':', ';', '?', '!']);
        if token.is_empty() {
            continue;
        }
        if token.contains("://") {
            push(&mut out.constants, token);
        } else if looks_like_path(token) {
            push(&mut out.paths, token);
        } else if token.contains("YYYY") || token.contains("MM-DD") || token.contains('<') {
            push(&mut out.formats, token);
        } else if is_date(token) || is_version(token) || is_upper_constant(token) {
            push(&mut out.constants, token);
        }
    }
    // A block of format rows, or a JSON sample, is a format.
    let trimmed = text.trim();
    if trimmed.lines().count() > 1 && trimmed.lines().all(|l| format_row(l.trim())) {
        push(&mut out.formats, trimmed.lines().next().unwrap_or_default());
    }
    for opener in ["[{", "{\""] {
        if let Some(at) = trimmed.find(opener) {
            let sample: String = trimmed[at..].chars().take(120).collect();
            push(&mut out.formats, &sample);
        }
    }
    out
}

fn looks_like_path(token: &str) -> bool {
    if token.contains(' ') || token.len() < 3 || token.contains("://") {
        return false;
    }
    if (token.starts_with('/') || token.starts_with("~/") || token.starts_with("./"))
        && token.chars().skip(1).any(|c| c.is_alphanumeric())
    {
        return true;
    }
    let name = token.rsplit('/').next().unwrap_or(token);
    name.rsplit_once('.').is_some_and(|(stem, extension)| {
        !stem.is_empty()
            && EXTENSIONS.contains(&extension)
            && stem
                .chars()
                .all(|c| c.is_alphanumeric() || "_-.<>".contains(c))
    })
}

/// A word such as `ERROR` or `CWE-89`: uppercase letters, digits, `_`,
/// and `-`, with at least two letters.
fn is_upper_constant(token: &str) -> bool {
    token.len() >= 3
        && token
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c == '-' || c.is_ascii_digit())
        && token.chars().filter(char::is_ascii_uppercase).count() >= 2
}

fn is_date(token: &str) -> bool {
    let b = token.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

fn is_version(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    parts.len() >= 2
        && parts.len() <= 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Whether a span looks like an example, a sample, or a format.
#[must_use]
pub fn example_like(span: &Span) -> bool {
    let lower = span.text.to_lowercase();
    matches!(span.form, Form::Block | Form::Code)
        || [
            "e.g.",
            "for example",
            "such as",
            "format",
            "structure",
            "example",
            "sample",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
}

/// The kind a rule gives a span, without Jev.
#[must_use]
pub fn rule_kind(span: &Span, extracted: &Extracted) -> Kind {
    let lower = span.text.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    if span.form == Form::Checkbox {
        return if has(&["test", "pass", "verify"]) {
            Kind::Check
        } else {
            Kind::Deliverable
        };
    }
    if span.form == Form::Code {
        return Kind::Check;
    }
    if span.form == Form::Block {
        return Kind::Constraint;
    }
    if has(&[
        "test",
        "should pass",
        "verify",
        "pytest",
        "must run",
        "should run",
    ]) {
        return Kind::Check;
    }
    if has(&[
        "must not",
        "do not",
        "don't",
        "should not",
        "exactly",
        "only ",
        "without",
        "never",
        "remain",
        "untouched",
        "no need to change",
        "format",
        "structure",
    ]) {
        return Kind::Constraint;
    }
    let first = lower.split_whitespace().next().unwrap_or("");
    let imperative = [
        "create",
        "write",
        "implement",
        "fix",
        "recover",
        "make",
        "add",
        "update",
        "ensure",
        "install",
        "put",
        "call",
        "build",
        "compile",
        "merge",
        "clean",
        "identify",
        "count",
        "generate",
        "produce",
        "save",
        "report",
        "find",
        "use",
    ];
    if has(&[
        "write ", "create ", "save ", "output", "generate", "produce", "put it", "put the",
    ]) && !extracted.paths.is_empty()
    {
        return Kind::Deliverable;
    }
    if imperative.contains(&first.trim_matches(|c: char| !c.is_alphanumeric()))
        || has(&[
            " should ",
            " must ",
            " needs to ",
            "make sure",
            "please",
            "i want",
            "can you",
        ])
    {
        return Kind::Behavior;
    }
    Kind::Context
}

// ---------------------------------------------------------------------------
// Jev
// ---------------------------------------------------------------------------

/// The Jev request for one batch of spans. Question IDs name the span
/// (`kind_s3`, `exhaustive_s3`); the wording names its index in the batch.
#[must_use]
pub fn request(title: &str, body: &str, batch: &[Span]) -> (Value, Questions) {
    let mut questions = Questions::new();
    for (i, span) in batch.iter().enumerate() {
        let mut choice = Choice::new(
            KIND_QUESTION.replace("{i}", &i.to_string()),
            indexmap::IndexMap::new(),
        );
        for (name, description) in KINDS {
            choice = choice.option(name, description);
        }
        questions = questions.with(format!("kind_{}", span.id), choice);
        if example_like(span) {
            questions = questions.with(
                format!("exhaustive_{}", span.id),
                Noul::new(EXHAUSTIVE_QUESTION.replace("{i}", &i.to_string())),
            );
        }
    }
    let state = json!({
        "issue": issue_state(title, body),
        "spans": batch.iter().map(|s| json!({ "id": s.id, "text": s.text })).collect::<Vec<_>>(),
    });
    (state, questions)
}

/// A Choice answer's probabilities, or its pick at probability one.
fn probabilities(answer: &Value) -> BTreeMap<String, f64> {
    let mut out: BTreeMap<String, f64> = answer
        .get("probabilities")
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_f64()?)))
                .collect()
        })
        .unwrap_or_default();
    if out.is_empty()
        && let Some(pick) = answer.get("choice").and_then(Value::as_str)
    {
        out.insert(pick.to_string(), 1.0);
    }
    out
}

/// Whether `text` is one Markdown ATX heading line, such as `## Goal`.
fn heading(text: &str) -> bool {
    let text = text.trim();
    let hashes = text.chars().take_while(|&c| c == '#').count();
    !text.contains('\n')
        && (1..=6).contains(&hashes)
        && text[hashes..].starts_with(' ')
        && !text[hashes..].trim().is_empty()
}

/// Builds the map from spans and whatever answers came back, keyed by
/// question ID. A span without an answer is placed by rule and marked
/// `unjudged` when the rule keeps it.
#[must_use]
pub fn decide(body: &str, spans: &[Span], answers: &Value, params: Params) -> RequirementMap {
    let mut placed = Vec::new();
    let mut requirements: Vec<Requirement> = Vec::new();
    let mut unanswered = 0;
    // Whether the spans now are the items under a lead-in such as "the
    // following date ranges:", which the rule reads as binding.
    let mut under_lead_in = false;
    for span in spans {
        let item = matches!(span.form, Form::Line | Form::ListItem | Form::Checkbox);
        let extracted = extract(&span.text);
        let kinds = answers
            .get(format!("kind_{}", span.id))
            .map(probabilities)
            .unwrap_or_default();
        let exhaustive = answers
            .get(format!("exhaustive_{}", span.id))
            .and_then(|a| a.get("noul"))
            .and_then(Value::as_f64);
        let answered = !kinds.is_empty();
        let (kind, binding, p) = if heading(&span.text) {
            // A heading names the section under it and is never a
            // requirement itself: "## Goal" once became one, and a session
            // spent itself on it.
            (Kind::Context, None, None)
        } else if answered {
            let context = kinds.get("context").copied().unwrap_or(0.0);
            let p = 1.0 - context;
            let best = kinds
                .iter()
                .filter(|(name, _)| name.as_str() != "context")
                .max_by(|a, b| a.1.total_cmp(b.1))
                .and_then(|(name, _)| Kind::parse(name))
                .unwrap_or(Kind::Behavior);
            if p >= params.yes {
                (best, Some(Binding::Yes), Some(p))
            } else if p >= params.keep {
                (best, Some(Binding::Uncertain), Some(p))
            } else {
                (Kind::Context, None, Some(p))
            }
        } else {
            unanswered += 1;
            let mut kind = rule_kind(span, &extracted);
            if kind == Kind::Context && item && under_lead_in {
                kind = Kind::Constraint;
            }
            let binding = (kind != Kind::Context).then_some(Binding::Unjudged);
            (kind, binding, None)
        };
        let requirement = binding.map(|binding| {
            let id = format!("R{}", requirements.len() + 1);
            requirements.push(Requirement {
                id: id.clone(),
                spans: vec![span.id.clone()],
                kind,
                binding,
                state: State::Unobserved,
                p: p.map(round),
                exhaustive: exhaustive.map(round),
                text: span.text.clone(),
                extracted,
            });
            id
        });
        if !item {
            let lower = span.text.to_lowercase();
            under_lead_in = span.text.trim_end().ends_with(':')
                || lower.contains("the following")
                || lower.contains("as follows");
        }
        placed.push(Placed {
            span: span.clone(),
            requirement,
            kind,
            kinds: kinds.into_iter().map(|(k, v)| (k, round(v))).collect(),
            answered,
        });
    }
    let chars = body.chars().filter(|c| !c.is_whitespace()).count();
    let mut inside = vec![false; body.len()];
    for span in spans {
        for flag in &mut inside[span.start..span.end] {
            *flag = true;
        }
    }
    let covered = body
        .char_indices()
        .filter(|(i, c)| !c.is_whitespace() && inside[*i])
        .count();
    let requirement_spans = placed.iter().filter(|p| p.requirement.is_some()).count();
    RequirementMap {
        schema: SCHEMA.to_string(),
        instruction_sha256: crate::ops::hex(&Sha256::digest(body.as_bytes())),
        method: if unanswered == spans.len() {
            "rule".to_string()
        } else {
            "jev".to_string()
        },
        coverage: Coverage {
            chars,
            covered,
            fraction: if chars == 0 {
                1.0
            } else {
                round(covered as f64 / chars as f64)
            },
            spans: spans.len(),
            requirement_spans,
            context_spans: placed.len() - requirement_spans,
            unanswered_spans: unanswered,
        },
        spans: placed,
        requirements,
    }
}

/// The map by rule alone, with no Jev.
#[must_use]
pub fn mechanical(body: &str) -> RequirementMap {
    decide(body, &split(body), &Value::Null, Params::default())
}

/// Extracts the map with Jev in `mode`, one request per batch of spans,
/// each recorded under `component`.
pub async fn extract_with(
    title: &str,
    body: &str,
    params: Params,
    mode: &JevMode,
    recorder: &Recorder,
    deadline: Option<crate::deadline::Deadline>,
) -> (RequirementMap, Vec<Asked>) {
    let spans = split(body);
    let mut answers = serde_json::Map::new();
    let mut asked = Vec::new();
    for (n, batch) in spans.chunks(params.batch.max(1)).enumerate() {
        let (state, questions) = request(title, body, batch);
        let one = crate::component::jev::ask(
            mode,
            recorder,
            Ask {
                component: "task.requirements",
                name: "jev_requirements",
                id: format!("jev_requirements-{}", n + 1),
                state,
                questions,
                parent: None,
                deadline: deadline.clone(),
            },
        )
        .await;
        if let Some(Value::Object(map)) = &one.answers {
            answers.extend(map.clone());
        }
        asked.push(one);
    }
    (decide(body, &spans, &Value::Object(answers), params), asked)
}

fn round(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

// ---------------------------------------------------------------------------
// Labels and scoring
// ---------------------------------------------------------------------------

/// One hand-labeled requirement: text that must appear in the span that
/// states it, and its kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    pub id: String,
    pub anchor: String,
    pub kind: Kind,
}

/// Recall and precision of a map against labels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Score {
    /// Labels some kept requirement states.
    pub recall: f64,
    /// Kept requirements that state some label.
    pub precision: f64,
    /// The same over requirements Jev read as binding (`yes`) only.
    pub recall_binding: f64,
    pub precision_binding: f64,
    /// Of matched pairs, how many share the label's kind.
    pub kind_agreement: Option<f64>,
    pub missed: Vec<String>,
    pub unmatched: Vec<String>,
}

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Scores `map` against `labels`. A requirement states a label when its
/// span text holds the label's anchor.
#[must_use]
pub fn score(map: &RequirementMap, labels: &[Label]) -> Score {
    let states = |r: &Requirement, l: &Label| squash(&r.text).contains(&squash(&l.anchor));
    let measure = |kept: &[&Requirement]| -> (f64, f64, Vec<String>, Vec<String>) {
        let missed: Vec<String> = labels
            .iter()
            .filter(|l| !kept.iter().any(|r| states(r, l)))
            .map(|l| l.id.clone())
            .collect();
        let unmatched: Vec<String> = kept
            .iter()
            .filter(|r| !labels.iter().any(|l| states(r, l)))
            .map(|r| r.id.clone())
            .collect();
        let recall = if labels.is_empty() {
            1.0
        } else {
            (labels.len() - missed.len()) as f64 / labels.len() as f64
        };
        let precision = if kept.is_empty() {
            if labels.is_empty() { 1.0 } else { 0.0 }
        } else {
            (kept.len() - unmatched.len()) as f64 / kept.len() as f64
        };
        (round(recall), round(precision), missed, unmatched)
    };
    let all: Vec<&Requirement> = map.requirements.iter().collect();
    let binding: Vec<&Requirement> = map
        .requirements
        .iter()
        .filter(|r| r.binding != Binding::Uncertain)
        .collect();
    let (recall, precision, missed, unmatched) = measure(&all);
    let (recall_binding, precision_binding, ..) = measure(&binding);
    let pairs: Vec<(Kind, Kind)> = labels
        .iter()
        .filter_map(|l| {
            map.requirements
                .iter()
                .find(|r| states(r, l))
                .map(|r| (l.kind, r.kind))
        })
        .collect();
    Score {
        recall,
        precision,
        recall_binding,
        precision_binding,
        kind_agreement: (!pairs.is_empty()).then(|| {
            round(pairs.iter().filter(|(a, b)| a == b).count() as f64 / pairs.len() as f64)
        }),
        missed,
        unmatched,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "You are given multiple log files stored in /app/logs. Each log file name follows the pattern YYYY-MM-DD_<source>.log (e.g., 2025-08-10_db.log), indicating the date. Your task is to count each severity within the following date ranges:\nToday (the current date)\nLast 7 days (including today)\n\nThe severity levels to count are exactly: ERROR, WARNING, and INFO.\nWrite a CSV file /app/summary.csv with the following structure (including the header):\nperiod,severity,count\ntoday,ERROR,<count>\n\nThe current date is 2025-08-12. Use this as the reference date.\n";

    #[test]
    fn a_markdown_heading_is_never_a_requirement() {
        assert!(heading("## Goal"));
        assert!(heading("# #9597 Docs: explain mini-tasks"));
        assert!(!heading("#9597 needs a fix."));
        assert!(!heading("##"));
        let body = "## Goal\nWrite the report to out.txt.";
        let spans = split(body);
        let answers = json!({
            "kind_s1": {"choice": "deliverable", "probabilities": {"deliverable": 0.9, "context": 0.1}},
            "kind_s2": {"choice": "deliverable", "probabilities": {"deliverable": 0.9, "context": 0.1}},
        });
        let map = decide(body, &spans, &answers, Params::default());
        assert_eq!(map.requirements.len(), 1, "{:?}", map.requirements);
        assert!(map.requirements[0].text.contains("out.txt"));
    }

    #[test]
    fn spans_cover_every_character_and_follow_the_text() {
        let spans = split(LOG);
        let map = mechanical(LOG);
        assert_eq!(map.coverage.fraction, 1.0, "{:#?}", spans);
        let texts: Vec<&str> = spans.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.contains(&"Today (the current date)"));
        assert!(texts.contains(&"Last 7 days (including today)"));
        assert!(
            texts
                .iter()
                .any(|t| t.starts_with("period,severity,count\ntoday"))
        );
        assert!(texts.contains(&"The current date is 2025-08-12."));
        // "e.g." does not end a sentence.
        assert!(
            texts
                .iter()
                .any(|t| t.contains("(e.g., 2025-08-10_db.log), indicating"))
        );
        for span in &spans {
            assert_eq!(&LOG[span.start..span.end], span.text);
        }
    }

    #[test]
    fn lists_checkboxes_and_code_blocks_are_their_own_spans() {
        let text = "Please\n  1. recover the secret and write it to a /app/secret.txt file.\n  2. clean up.\n- [ ] Add a flag\n```python\nimport x\nx.run()\n```\nDone.";
        let spans = split(text);
        let forms: Vec<Form> = spans.iter().map(|s| s.form).collect();
        assert!(forms.contains(&Form::Checkbox));
        assert!(forms.contains(&Form::Code));
        assert_eq!(forms.iter().filter(|f| **f == Form::ListItem).count(), 2);
        assert_eq!(mechanical(text).coverage.fraction, 1.0);
    }

    #[test]
    fn extraction_finds_paths_commands_formats_and_constants() {
        let found = extract(
            "You should clone it with `git clone --depth 1 https://x/y.git` to `/app/pyknotid`, write /app/summary.csv as YYYY-MM-DD rows, count ERROR and INFO, and use Numpy 2.3.0 from 2025-08-12.",
        );
        assert_eq!(found.commands, ["git clone --depth 1 https://x/y.git"]);
        assert!(found.paths.contains(&"/app/pyknotid".to_string()));
        assert!(found.paths.contains(&"/app/summary.csv".to_string()));
        assert!(found.formats.contains(&"YYYY-MM-DD".to_string()));
        for constant in ["ERROR", "INFO", "2.3.0", "2025-08-12"] {
            assert!(
                found.constants.contains(&constant.to_string()),
                "{constant}"
            );
        }
    }

    #[test]
    fn jev_answers_place_spans_and_uncertain_ones_stay() {
        let body = "Write /app/out.txt. The sky is blue. Maybe keep it short.";
        let spans = split(body);
        assert_eq!(spans.len(), 3);
        let answers = json!({
            "kind_s1": {"choice": "deliverable", "probabilities": {"deliverable": 0.9, "context": 0.1}},
            "kind_s2": {"choice": "context", "probabilities": {"context": 0.95, "behavior": 0.05}},
            "kind_s3": {"choice": "context", "probabilities": {"context": 0.7, "constraint": 0.3}},
        });
        let map = decide(body, &spans, &answers, Params::default());
        assert_eq!(map.requirements.len(), 2);
        assert_eq!(map.requirements[0].binding, Binding::Yes);
        assert_eq!(map.requirements[0].kind, Kind::Deliverable);
        assert_eq!(map.requirements[1].binding, Binding::Uncertain);
        assert_eq!(map.requirements[1].kind, Kind::Constraint);
        assert_eq!(map.coverage.context_spans, 1);
        assert_eq!(map.coverage.fraction, 1.0);
        assert_eq!(map.method, "jev");
    }

    #[test]
    fn checkbox_issues_still_yield_their_items() {
        let map = mechanical("Intro\n- [ ] Add a flag\n  - [x] Test it\n- plain item\n");
        let texts: Vec<&str> = map.requirements.iter().map(|r| r.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("Add a flag")));
        assert!(texts.iter().any(|t| t.contains("Test it")));
    }

    /// The labeled suite, with recorded Jev: every instruction fully
    /// covered, and recall and precision reported for every labeled task.
    #[tokio::test(flavor = "current_thread")]
    async fn the_labeled_suite_scores_recall_and_precision_with_recorded_jev() {
        use crate::component::{JevChoice, find, fixtures_for, suite};
        let component = find("task.requirements").unwrap();
        let dirs = fixtures_for(&crate::component::default_fixtures(), component.id());
        assert!(dirs.len() >= 10, "{dirs:?}");
        let jev = suite(
            component.as_ref(),
            &dirs,
            &JevChoice::Recorded,
            &Recorder::default(),
            false,
        )
        .await
        .unwrap();
        let rule = suite(
            component.as_ref(),
            &dirs,
            &JevChoice::Off,
            &Recorder::default(),
            false,
        )
        .await
        .unwrap();
        for run in jev.runs.iter().chain(&rule.runs) {
            assert!(run.error.is_none(), "{}: {:?}", run.fixture, run.error);
            assert_eq!(run.metrics["coverage"], json!(1.0), "{}", run.fixture);
            assert!(run.metrics.contains_key("recall"), "{}", run.fixture);
            assert!(run.metrics.contains_key("precision"), "{}", run.fixture);
        }
        assert!(jev.runs.iter().all(|run| run.jev.get("miss").is_none()));
        let mean = |suite: &crate::component::Suite, metric: &str| {
            suite.summary()["metrics"][metric]["mean"].as_f64().unwrap()
        };
        assert!(mean(&jev, "recall") >= 0.95, "{}", mean(&jev, "recall"));
        assert!(mean(&jev, "precision") >= mean(&rule, "precision"));
    }

    #[test]
    fn scoring_counts_anchors_in_kept_requirements() {
        let body = "Write /app/out.txt. The sky is blue.";
        let map = mechanical(body);
        let labels = vec![
            Label {
                id: "L1".into(),
                anchor: "/app/out.txt".into(),
                kind: Kind::Deliverable,
            },
            Label {
                id: "L2".into(),
                anchor: "absent text".into(),
                kind: Kind::Check,
            },
        ];
        let score = score(&map, &labels);
        assert_eq!(score.recall, 0.5);
        assert_eq!(score.missed, ["L2"]);
    }
}
