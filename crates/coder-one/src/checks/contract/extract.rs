//! Extraction by code: what a task states, as [`Item`]s.
//!
//! [`gather`] reads what the instruction points to from the untouched
//! workspace. [`draft`] turns the instruction and those files into items
//! with no model: it splits the text into sentences, lines, and fenced
//! blocks, finds the code spans in them, and reads each span by its form
//! (a path, a command, an identifier) and by the words around it (an
//! output verb before a path, a column list after it, a stated exit
//! status). Where the words don't settle whether a command is stated to
//! succeed, or whether a file holds an example's expected output, the
//! draft leaves a [`Pending`] question, and [`finish`] settles it with
//! Jev's Noul at [`THRESHOLD`], or leaves the item not executable.

use std::collections::{BTreeMap, BTreeSet};

use jev::{Noul, NoulCriteria, Questions};
use regex::Regex;
use serde_json::{Value, json};

use super::host::{Host, Stat};
use super::{COMMAND_SEC, EXAMPLE_SEC, Exit, Expect, Item, JsonTop, Kind, Plan, clip};
use crate::component::jev::{Ask, JevMode, ask};
use crate::record::Recorder;

/// A Noul at or above this says yes.
pub const THRESHOLD: f64 = 0.5;

/// The largest file read as text while making a plan.
pub const TEXT_MAX: usize = 256 * 1024;

/// The largest file the instruction points to that is read for commands.
pub const DOC_MAX: usize = 64 * 1024;

/// A path that exists nowhere, for a stated missing-file condition.
pub const MISSING_INPUT: &str = "/nonexistent/contract-check-input";

/// Programs whose name starts a command.
const PROGRAMS: &[&str] = &[
    "make", "python", "python3", "pytest", "cargo", "go", "npm", "npx", "node", "bun", "deno",
    "bash", "sh", "uv", "java", "javac", "gcc", "g++", "clang", "clang++", "ruby", "perl", "coqc",
    "dotnet", "mvn", "gradle", "ctest", "cmake", "julia", "Rscript", "rustc", "sqlite3", "psql",
    "lake", "ghc", "cabal", "stack", "swift", "tsc", "yarn", "pnpm", "php", "lua", "ocaml", "dune",
    "zig", "pip", "pip3", "tox", "nox", "mix", "elixir", "erl", "sbt", "scala", "kotlin",
];

/// Words that say a path is where output goes, when they're the nearest
/// cue before it.
const OUTPUT_CUES: &[&str] = &[
    "write",
    "writes",
    "written",
    "writing",
    "save",
    "saved",
    "saves",
    "saving",
    "create",
    "creates",
    "created",
    "build",
    "builds",
    "produce",
    "produces",
    "generate",
    "generates",
    "store",
    "stored",
    "emit",
    "emits",
    "into",
    "add",
    "put",
    "place",
    "output",
    "dump",
    "deliver",
];

/// Words that say a path is something else: an input, a reference, code
/// to edit, or a module to import from.
const OTHER_CUES: &[&str] = &[
    "read",
    "reads",
    "given",
    "located",
    "has",
    "have",
    "contains",
    "load",
    "loaded",
    "loads",
    "reference",
    "match",
    "matches",
    "against",
    "from",
    "under",
    "see",
    "available",
    "use",
    "uses",
    "using",
    "compare",
    "imports",
    "import",
    "fix",
    "repair",
    "modify",
    "edit",
    "keep",
    "preserve",
    "run",
    "runs",
    "inputs",
    "input",
    "is",
    "are",
    "in",
];

/// Words that don't tell a placeholder's file apart.
const GENERIC: &[&str] = &[
    "file",
    "path",
    "dir",
    "directory",
    "input",
    "output",
    "name",
    "the",
    "a",
    "arg",
    "argument",
    "filename",
    "txt",
    "json",
    "csv",
    "tsv",
    "data",
    "in",
    "out",
    "of",
];

/// Where a unit of text comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    /// A sentence, a list item, or a line.
    pub text: String,
    pub paragraph: usize,
}

/// A fenced block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub lang: String,
    pub body: String,
    /// The index of the unit right before the block.
    pub lead: Option<usize>,
}

/// Drops HTML comments.
#[must_use]
pub fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Splits a paragraph into sentences at `.`, `!`, or `?` followed by
/// whitespace, never inside a code span.
#[must_use]
pub fn sentences(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut current = String::new();
    let mut code = false;
    for (i, c) in chars.iter().enumerate() {
        current.push(*c);
        if *c == '`' {
            code = !code;
        }
        let end = matches!(c, '.' | '!' | '?')
            && !code
            && chars.get(i + 1).is_none_or(|n| n.is_whitespace());
        if end {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_string());
            }
            current.clear();
        }
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        out.push(trimmed.to_string());
    }
    out
}

fn bullet(line: &str) -> Option<&str> {
    for mark in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(mark) {
            return Some(rest);
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && (line[digits..].starts_with(". ") || line[digits..].starts_with(") ")) {
        return Some(&line[digits + 2..]);
    }
    None
}

/// Splits text into units and fenced blocks.
#[must_use]
pub fn segment(text: &str) -> (Vec<Unit>, Vec<Block>) {
    let text = strip_comments(text);
    let mut units: Vec<Unit> = Vec::new();
    let mut blocks = Vec::new();
    let mut paragraph = 0;
    let mut buffer = String::new();
    let mut fence: Option<(String, String)> = None;
    let flush = |buffer: &mut String, units: &mut Vec<Unit>, paragraph: usize| {
        for sentence in sentences(buffer) {
            units.push(Unit {
                text: sentence,
                paragraph,
            });
        }
        buffer.clear();
    };
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some((lang, body)) = &mut fence {
            if trimmed.starts_with("```") {
                blocks.push(Block {
                    lang: lang.clone(),
                    body: body.clone(),
                    lead: units.len().checked_sub(1),
                });
                fence = None;
            } else {
                body.push_str(line);
                body.push('\n');
            }
            continue;
        }
        if let Some(lang) = trimmed.strip_prefix("```") {
            flush(&mut buffer, &mut units, paragraph);
            fence = Some((lang.trim().to_lowercase(), String::new()));
            continue;
        }
        if trimmed.is_empty() {
            flush(&mut buffer, &mut units, paragraph);
            paragraph += 1;
            continue;
        }
        if trimmed.starts_with('#') {
            flush(&mut buffer, &mut units, paragraph);
            units.push(Unit {
                text: trimmed.trim_start_matches('#').trim().to_string(),
                paragraph,
            });
            continue;
        }
        if let Some(rest) = bullet(trimmed) {
            flush(&mut buffer, &mut units, paragraph);
            buffer.push_str(rest);
            continue;
        }
        if !buffer.is_empty() {
            buffer.push(' ');
        }
        buffer.push_str(trimmed);
    }
    flush(&mut buffer, &mut units, paragraph);
    (units, blocks)
}

/// The code spans in `text`, each with its byte offset.
#[must_use]
pub fn spans(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut open: Option<usize> = None;
    for (i, c) in text.char_indices() {
        if c == '`' {
            match open {
                Some(start) => {
                    let inner = &text[start + 1..i];
                    if !inner.trim().is_empty() {
                        out.push((start, inner.trim().to_string()));
                    }
                    open = None;
                }
                None => open = Some(i),
            }
        }
    }
    out
}

/// Whether `span` reads as a shell command.
#[must_use]
pub fn is_command(span: &str) -> bool {
    let mut words = span.split_whitespace();
    let Some(first) = words.next() else {
        return false;
    };
    if first.starts_with("./") && first.len() > 2 {
        return true;
    }
    PROGRAMS.contains(&first) && (words.next().is_some() || matches!(first, "make" | "pytest"))
}

/// Whether `span` is an identifier.
#[must_use]
pub fn is_identifier(span: &str) -> bool {
    let mut chars = span.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn path_chars(span: &str) -> bool {
    !span.is_empty()
        && span
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '~' | '+'))
}

fn has_extension(span: &str) -> bool {
    let name = span.rsplit('/').next().unwrap_or(span);
    name.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty()
            && (1..=12).contains(&ext.len())
            && ext.chars().all(|c| c.is_ascii_alphanumeric())
    })
}

/// The absolute path a span names, if it names one: an absolute path as
/// it is, or a relative one with a directory or an extension under
/// `workdir`.
#[must_use]
pub fn as_path(span: &str, workdir: &str) -> Option<String> {
    let span = span.trim_end_matches([',', ';', ':']);
    if !path_chars(span) || span.contains("..") && !span.starts_with('/') {
        return None;
    }
    if span.starts_with('/') {
        return (span.len() > 1).then(|| span.to_string());
    }
    if span.starts_with('~') || span.starts_with('.') && !span.starts_with("./") {
        return None;
    }
    let relative = span.trim_start_matches("./");
    (relative.contains('/') || has_extension(relative))
        .then(|| format!("{}/{relative}", workdir.trim_end_matches('/')))
}

fn words_lower(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '-'))
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Whether the words before a path make it an output: the nearest cue in
/// the last 15 words is an output verb, not negated. `None` when no cue
/// is there.
#[must_use]
pub fn is_output(before: &str) -> Option<bool> {
    // Only the words after the last earlier code span count: a cue
    // before another path or name belongs to that one.
    let plain = match before.rfind('`') {
        Some(at) if before.matches('`').count().is_multiple_of(2) => &before[at + 1..],
        _ => before,
    };
    let words = words_lower(plain);
    let start = words.len().saturating_sub(15);
    let recent = &words[start..];
    for (i, word) in recent.iter().enumerate().rev() {
        if OUTPUT_CUES.contains(&word.as_str()) {
            let negated = recent[i.saturating_sub(4)..i]
                .iter()
                .any(|w| matches!(w.as_str(), "not" | "don't" | "never" | "no"));
            return Some(!negated);
        }
        if OTHER_CUES.contains(&word.as_str()) {
            return Some(false);
        }
    }
    None
}

/// Whether a path in `units[index]` at byte `offset` is an output: by the
/// words before it, or, for a list item with no cue, by the lead-in line
/// that ends with a colon before the list.
fn output_at(units: &[Unit], index: usize, offset: usize) -> bool {
    if let Some(decided) = is_output(&units[index].text[..offset]) {
        return decided;
    }
    let mut at = index;
    while at > 0 && index - at < 12 {
        at -= 1;
        let text = units[at].text.trim_end();
        if text.ends_with(':') {
            return is_output(text).unwrap_or(false);
        }
        if !text.starts_with('`') {
            return false;
        }
    }
    false
}

pub(crate) fn tokens(name: &str) -> BTreeSet<String> {
    name.split(|c: char| !c.is_ascii_alphanumeric())
        .map(str::to_lowercase)
        .filter(|t| t.len() > 1 && !GENERIC.contains(&t.as_str()))
        .collect()
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub(crate) fn stem_tokens(path: &str) -> BTreeSet<String> {
    let name = file_name(path);
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    tokens(stem)
}

/// What the untouched workspace holds at each path the plan reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pristine {
    pub entries: BTreeMap<String, Entry>,
}

/// One path of the untouched workspace.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub stat: Stat,
    /// The text, for a file of at most [`TEXT_MAX`] bytes that is UTF-8.
    pub text: Option<String>,
}

impl Pristine {
    fn stat(&self, path: &str) -> Stat {
        self.entries
            .get(path.trim_end_matches('/'))
            .map_or(Stat::Missing, |e| e.stat)
    }

    fn text(&self, path: &str) -> Option<&str> {
        self.entries.get(path).and_then(|e| e.text.as_deref())
    }
}

/// Paths the instruction's code spans name.
fn mentioned(instruction: &str, workdir: &str) -> Vec<String> {
    let (units, _) = segment(instruction);
    let mut out = Vec::new();
    for unit in &units {
        for (_, span) in spans(&unit.text) {
            if let Some(path) = as_path(&span, workdir) {
                out.push(path);
            } else if is_command(&span) {
                for word in span.split_whitespace().skip(1) {
                    if let Some(path) = as_path(word, workdir) {
                        out.push(path);
                    }
                }
            }
        }
    }
    out
}

fn is_doc(path: &str) -> bool {
    let name = file_name(path).to_lowercase();
    name.starts_with("readme")
        || [".md", ".txt", ".rst", ".py", ".sh"]
            .iter()
            .any(|ext| name.ends_with(ext))
}

async fn fetch(host: &impl Host, path: &str, into: &mut Pristine) {
    let key = path.trim_end_matches('/').to_string();
    if into.entries.contains_key(&key) {
        return;
    }
    let stat = host.stat(&key).await.unwrap_or(Stat::Missing);
    let text = match stat {
        Stat::File(size) if size <= TEXT_MAX as u64 => host
            .read(&key, TEXT_MAX)
            .await
            .ok()
            .flatten()
            .and_then(|b| String::from_utf8(b).ok()),
        _ => None,
    };
    into.entries.insert(key, Entry { stat, text });
}

/// Reads, from the untouched workspace on `host`, every path the
/// instruction names, the `README` of every directory it names, and every
/// file a command in a named document names.
pub async fn gather(host: &impl Host, instruction: &str, workdir: &str) -> Pristine {
    let mut pristine = Pristine::default();
    for path in mentioned(instruction, workdir) {
        fetch(host, &path, &mut pristine).await;
    }
    let dirs: Vec<String> = pristine
        .entries
        .iter()
        .filter(|(_, e)| e.stat == Stat::Dir)
        .map(|(p, _)| p.clone())
        .collect();
    for dir in dirs {
        for name in ["README.md", "README", "README.txt", "readme.md"] {
            fetch(host, &format!("{dir}/{name}"), &mut pristine).await;
        }
    }
    let docs: Vec<(String, String)> = pristine
        .entries
        .iter()
        .filter(|(p, e)| is_doc(p) && matches!(e.stat, Stat::File(n) if n <= DOC_MAX as u64))
        .filter_map(|(p, e)| Some((p.clone(), e.text.clone()?)))
        .collect();
    for (_, text) in docs {
        for (command, _) in doc_commands(&text) {
            for word in command.split_whitespace().skip(1) {
                if let Some(path) = as_path(word, workdir) {
                    fetch(host, &path, &mut pristine).await;
                }
            }
        }
    }
    pristine
}

/// Commands a document states, each with the line it came from: lines of
/// a shell or unlabeled fenced block, `$ ` lines, and `Usage:` or
/// `command:` lines. A script contributes only `Usage:` lines from its
/// first 60 lines.
#[must_use]
pub fn doc_commands(text: &str) -> Vec<(String, String)> {
    let usage =
        Regex::new(r"(?i)^\s*(?:#\s*)?(?:usage|local command|command|run|reproduce)\s*:\s*(.*)$")
            .expect("a valid pattern");
    let mut out: Vec<(String, String)> = Vec::new();
    let mut fence: Option<String> = None;
    let mut after_usage = false;
    for (n, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(lang) = &fence {
            if trimmed.starts_with("```") {
                fence = None;
            } else if matches!(
                lang.as_str(),
                "" | "sh" | "bash" | "shell" | "console" | "zsh"
            ) {
                let command = trimmed.trim_start_matches("$ ").trim();
                if is_command(command) {
                    out.push((command.to_string(), line.to_string()));
                }
            }
            continue;
        }
        if let Some(lang) = trimmed.strip_prefix("```") {
            fence = Some(lang.trim().to_lowercase());
            continue;
        }
        if n > 60 && out.is_empty() && !text.contains("```") {
            // A long script: only its header documents usage.
        }
        if let Some(captures) = usage.captures(line) {
            let rest = captures[1].trim().trim_matches('`');
            if is_command(rest) {
                out.push((rest.to_string(), line.to_string()));
            }
            after_usage = rest.is_empty();
            continue;
        }
        if after_usage {
            let command = trimmed.trim_start_matches("$ ").trim();
            if is_command(command) {
                out.push((command.to_string(), line.to_string()));
                continue;
            }
            if trimmed.is_empty() {
                after_usage = false;
            }
        }
        if let Some(command) = trimmed.strip_prefix("$ ")
            && is_command(command.trim())
        {
            out.push((command.trim().to_string(), line.to_string()));
        }
    }
    out
}

/// A question the draft leaves for Jev.
#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    /// The question's ID in the request.
    pub id: String,
    /// The item it settles, by index in the draft.
    pub item: usize,
    pub ask: Asking,
}

/// What a pending question asks.
#[derive(Clone, Debug, PartialEq)]
pub enum Asking {
    /// Whether the task states that `command` should succeed.
    Succeeds { command: String, context: String },
    /// Whether `expected` holds the output of `command` on `input`.
    Pairs {
        command: String,
        input: String,
        expected: String,
        text: String,
    },
}

/// Items with the questions that settle some of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Draft {
    pub items: Vec<Item>,
    pub pending: Vec<Pending>,
}

fn item(kind: Kind, source: &str, span: &str) -> Item {
    Item {
        id: String::new(),
        kind,
        source: source.to_string(),
        span: clip(span, 300),
        command: None,
        path: None,
        expect: None,
        wall_sec: None,
        stated_bound: false,
        not_executable: None,
        decided_by: None,
    }
}

fn stated_seconds(units: &[Unit], program: &str) -> Option<u64> {
    let within = Regex::new(r"(?i)within\s+(\d+)\s*(s|secs?|seconds?|min|mins|minutes?)\b")
        .expect("a valid pattern");
    for unit in units {
        let lower = unit.text.to_lowercase();
        let about = lower.contains("invocation")
            || lower.contains("each run")
            || lower.contains("each call")
            || lower.contains("per run")
            || (!program.is_empty() && unit.text.contains(program));
        if !about {
            continue;
        }
        if let Some(c) = within.captures(&unit.text) {
            let n: u64 = c[1].parse().ok()?;
            let minutes = c[2].to_lowercase().starts_with('m');
            return Some(if minutes { n * 60 } else { n });
        }
    }
    None
}

const SUCCESS_CUES: &[&str] = &[
    "should pass",
    "must pass",
    "passes",
    "should succeed",
    "must succeed",
    "succeeds",
    "exit 0",
    "exits 0",
    "exit code 0",
    "exit status 0",
    "without errors",
    "exit successfully",
    "exits successfully",
];

const FAILURE_CUES: &[&str] = &[
    "should fail",
    "must fail",
    "exits non-zero",
    "exit non-zero",
];

fn quote(text: &str) -> String {
    crate::accept::runner::sh_quote(text)
}

/// The instruction's items, with no model.
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn draft(instruction: &str, workdir: &str, pristine: &Pristine) -> Draft {
    let (units, blocks) = segment(instruction);
    let mut draft = Draft::default();
    let mut commands_seen: BTreeSet<String> = BTreeSet::new();
    let mut outputs: BTreeSet<String> = BTreeSet::new();
    let placeholder = Regex::new(r"<([^<>]+)>").expect("a valid pattern");
    let named_files: Vec<String> = {
        let mut seen = BTreeSet::new();
        mentioned(instruction, workdir)
            .into_iter()
            .filter(|p| matches!(pristine.stat(p), Stat::File(_)))
            .filter(|p| seen.insert(p.clone()))
            .collect()
    };
    for (index, unit) in units.iter().enumerate() {
        let lower = unit.text.to_lowercase();
        let found = spans(&unit.text);
        // Outputs, with their formats.
        for (offset, span) in &found {
            if !span.starts_with('/') || is_command(span) {
                continue;
            }
            let Some(path) = as_path(span, workdir) else {
                continue;
            };
            if path.ends_with('/') || !output_at(&units, index, *offset) {
                continue;
            }
            outputs.insert(path.clone());
            let mut exists = item(Kind::Path, "instruction", &unit.text);
            exists.path = Some(path.clone());
            exists.expect = Some(Expect::Exists);
            if pristine.stat(&path) != Stat::Missing {
                exists.not_executable = Some(
                    "the untouched workspace already has it, so its presence can't tell a \
                     deliverable"
                        .to_string(),
                );
            }
            draft.items.push(exists);
            let after = &unit.text[offset + span.len() + 2..];
            if let Some(expect) = format_of(&path, &unit.text, after, index, &units, &blocks) {
                let mut format = item(Kind::Format, "instruction", &unit.text);
                format.path = Some(path.clone());
                format.expect = Some(expect);
                draft.items.push(format);
            }
        }
        // Names a module must provide.
        let negated = ["must not", "do not", "don't", "never"]
            .iter()
            .any(|n| lower.contains(n));
        let interface = [
            "import",
            "available",
            "export",
            "expose",
            "provide",
            "define",
            "remain",
        ]
        .iter()
        .any(|w| lower.contains(w));
        if interface && !negated {
            let mut names: Vec<String> = Vec::new();
            for (_, span) in &found {
                if is_identifier(span) {
                    names.push(span.clone());
                    continue;
                }
                if let Some(path) = as_path(span, workdir)
                    && path.ends_with(".py")
                    && !names.is_empty()
                {
                    let (dir, name) = path.rsplit_once('/').unwrap_or((workdir, &path));
                    let module = name.trim_end_matches(".py");
                    let code = format!(
                        "import importlib, sys\nsys.path.insert(0, {dir:?})\nm = importlib.import_module({module:?})\n\
                         missing = [n for n in {names:?} if not hasattr(m, n)]\n\
                         print('missing: ' + ', '.join(missing) if missing else 'all present')\n\
                         sys.exit(1 if missing else 0)"
                    );
                    let mut check = item(Kind::Interface, "instruction", &unit.text);
                    check.command = Some(format!("python3 -c {}", quote(&code)));
                    check.expect = Some(Expect::Exit { exit: Exit::Zero });
                    check.wall_sec = Some(60);
                    draft.items.push(check);
                    names.clear();
                }
            }
        }
        // Commands and examples.
        for (_, span) in &found {
            if !is_command(span) || !commands_seen.insert(span.clone()) {
                continue;
            }
            if placeholder.is_match(span) {
                examples(
                    &mut draft,
                    span,
                    unit,
                    &units,
                    &named_files,
                    pristine,
                    workdir,
                    &placeholder,
                );
                continue;
            }
            let mut command = item(Kind::Command, "instruction", &unit.text);
            command.command = Some(span.clone());
            let bound = stated_seconds(std::slice::from_ref(unit), "");
            command.wall_sec = Some(bound.unwrap_or(COMMAND_SEC));
            command.stated_bound = bound.is_some();
            if SUCCESS_CUES.iter().any(|c| lower.contains(c)) {
                command.expect = Some(Expect::Exit { exit: Exit::Zero });
            } else if FAILURE_CUES.iter().any(|c| lower.contains(c)) {
                command.expect = Some(Expect::Exit {
                    exit: Exit::NonZero,
                });
            } else {
                draft.pending.push(Pending {
                    id: String::new(),
                    item: draft.items.len(),
                    ask: Asking::Succeeds {
                        command: span.clone(),
                        context: unit.text.clone(),
                    },
                });
            }
            draft.items.push(command);
        }
    }
    // Shell blocks in the instruction.
    for block in &blocks {
        if !matches!(
            block.lang.as_str(),
            "sh" | "bash" | "shell" | "console" | "zsh"
        ) {
            continue;
        }
        for line in block.body.lines() {
            let command = line.trim().trim_start_matches("$ ").trim();
            if command.is_empty()
                || command.starts_with('#')
                || !commands_seen.insert(command.to_string())
            {
                continue;
            }
            doc_command(
                &mut draft,
                command,
                line,
                "instruction",
                workdir,
                pristine,
                &outputs,
                &placeholder,
            );
        }
    }
    // Documents the instruction points to.
    let docs: Vec<(String, String)> = pristine
        .entries
        .iter()
        .filter(|(p, e)| is_doc(p) && matches!(e.stat, Stat::File(n) if n <= DOC_MAX as u64))
        .filter_map(|(p, e)| Some((p.clone(), e.text.clone()?)))
        .collect();
    for (path, text) in docs {
        for (command, line) in doc_commands(&text) {
            if !commands_seen.insert(command.clone()) {
                continue;
            }
            doc_command(
                &mut draft,
                &command,
                &line,
                &path,
                workdir,
                pristine,
                &outputs,
                &placeholder,
            );
        }
    }
    for (n, item) in draft.items.iter_mut().enumerate() {
        item.id = format!("K{}", n + 1);
    }
    for (n, pending) in draft.pending.iter_mut().enumerate() {
        pending.id = format!("q{}", n + 1);
    }
    draft
}

/// The format an output path's sentence states, if it states one.
fn format_of(
    path: &str,
    text: &str,
    after: &str,
    index: usize,
    units: &[Unit],
    blocks: &[Block],
) -> Option<Expect> {
    let lower = text.to_lowercase();
    let ext = file_name(path)
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "json" => {
            let near: String = words_lower(after)
                .into_iter()
                .take(6)
                .collect::<Vec<_>>()
                .join(" ");
            let top = if near.contains("array") {
                Some(JsonTop::Array)
            } else if ["mapping", "object", "dictionary", "map", "dict"]
                .iter()
                .any(|w| near.split(' ').any(|x| x == *w))
            {
                Some(JsonTop::Object)
            } else {
                None
            };
            let example = |block: &Block| serde_json::from_str::<Value>(&block.body).ok();
            let shape = if ["format", "schema", "following", "structure", "like"]
                .iter()
                .any(|w| lower.contains(w))
            {
                let next = blocks
                    .iter()
                    .find(|b| b.lead == Some(index))
                    .and_then(example);
                let above = (lower.contains("above"))
                    .then(|| {
                        blocks
                            .iter()
                            .rev()
                            .filter(|b| b.lead.is_some_and(|l| l < index))
                            .find_map(example)
                    })
                    .flatten();
                next.or(above)
            } else {
                None
            };
            Some(Expect::Json { top, shape })
        }
        "csv" | "tsv" => {
            let delimiter = if ext == "tsv" { '\t' } else { ',' };
            let columns = columns_of(text, after);
            let ordered = !columns.is_empty() && lower.contains("order");
            Some(Expect::Table {
                delimiter,
                columns,
                ordered,
            })
        }
        "xlsx" | "docx" | "pptx" | "zip" => Some(Expect::Signature {
            format: "zip".to_string(),
        }),
        "npy" | "png" | "pdf" | "safetensors" => Some(Expect::Signature { format: ext }),
        "sqlite" | "sqlite3" => Some(Expect::Signature {
            format: "sqlite".to_string(),
        }),
        _ if lower.contains("per line") && lower.contains("one ") => {
            let _ = units;
            Some(Expect::Lines {
                sorted: ["alphabetical", "sorted", "ascending"]
                    .iter()
                    .any(|w| lower.contains(w)),
            })
        }
        _ => None,
    }
}

/// The columns a sentence states after an output path: the code spans
/// after `column`, `header`, or `headed`, or else a plain comma list after
/// a colon that follows `columns`.
fn columns_of(text: &str, after: &str) -> Vec<String> {
    let lower = after.to_lowercase();
    let Some(at) = ["column", "header", "headed"]
        .iter()
        .filter_map(|w| lower.find(w))
        .min()
    else {
        return Vec::new();
    };
    let rest = &after[at..];
    let coded: Vec<String> = spans(rest)
        .into_iter()
        .map(|(_, s)| s)
        .filter(|s| !s.starts_with('/'))
        .collect();
    if !coded.is_empty() {
        return coded;
    }
    let Some(colon) = rest.find(':') else {
        let _ = text;
        return Vec::new();
    };
    let list = rest[colon + 1..].trim().trim_end_matches('.');
    let names: Vec<String> = list
        .split(',')
        .map(|c| c.trim().trim_start_matches("and ").trim().to_string())
        .collect();
    if names.len() >= 2 && names.iter().all(|n| is_identifier(n)) {
        names
    } else {
        Vec::new()
    }
}

/// Items for a usage line with placeholders: the example run against the
/// file its placeholder names, stated output comparisons, and stated exit
/// statuses.
#[allow(clippy::too_many_arguments)]
fn examples(
    draft: &mut Draft,
    span: &str,
    unit: &Unit,
    units: &[Unit],
    named: &[String],
    pristine: &Pristine,
    workdir: &str,
    placeholder: &Regex,
) {
    let program = span
        .split_whitespace()
        .find(|w| has_extension(w))
        .map(file_name)
        .unwrap_or_default()
        .to_string();
    let bound = stated_seconds(units, &program);
    let wall = bound.unwrap_or(EXAMPLE_SEC);
    let all = units
        .iter()
        .map(|u| u.text.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    // Stated exit statuses.
    if all.contains("non-zero") || all.contains("nonzero") || all.contains("non zero") {
        let bare = placeholder.replace_all(span, "");
        let bare = bare.split_whitespace().collect::<Vec<_>>().join(" ");
        let conditions = [
            (
                [
                    "missing argument",
                    "argument is missing",
                    "no argument",
                    "without an argument",
                    "without arguments",
                ]
                .iter()
                .any(|c| all.contains(c)),
                bare,
            ),
            (
                [
                    "does not exist",
                    "doesn't exist",
                    "not exist",
                    "nonexistent",
                    "non-existent",
                    "missing file",
                ]
                .iter()
                .any(|c| all.contains(c)),
                placeholder.replace_all(span, MISSING_INPUT).to_string(),
            ),
        ];
        for (stated, command) in conditions {
            if stated {
                let mut exit = item(Kind::ExitCode, "instruction", &unit.text);
                exit.command = Some(command);
                exit.expect = Some(Expect::Exit {
                    exit: Exit::NonZero,
                });
                exit.wall_sec = Some(wall);
                exit.stated_bound = bound.is_some();
                draft.items.push(exit);
            }
        }
    }
    // The example itself, on the file the placeholder names.
    let mut chosen: Vec<(String, String)> = Vec::new();
    for capture in placeholder.captures_iter(span) {
        let wanted = tokens(&capture[1]);
        let best = named
            .iter()
            .map(|p| (stem_tokens(p).intersection(&wanted).count(), p))
            .filter(|(n, _)| *n > 0)
            .max_by_key(|(n, _)| *n)
            .map(|(_, p)| p.clone());
        match best {
            Some(path) => chosen.push((capture[0].to_string(), path)),
            None => {
                let mut example = item(Kind::Example, "instruction", &unit.text);
                example.command = Some(span.to_string());
                example.not_executable = Some(format!(
                    "the placeholder {} names no file the task provides",
                    &capture[0]
                ));
                draft.items.push(example);
                return;
            }
        }
    }
    let mut command = span.to_string();
    for (mark, path) in &chosen {
        command = command.replace(mark, &quote(path));
    }
    let _ = workdir;
    let Some((_, input)) = chosen.first().cloned() else {
        return;
    };
    let input_text = pristine.text(&input).map(str::to_string);
    if all.contains("same length as the input") || all.contains("same length as its input") {
        let mut length = item(Kind::Example, "instruction", &unit.text);
        length.command = Some(command.clone());
        length.wall_sec = Some(wall);
        length.stated_bound = bound.is_some();
        match &input_text {
            Some(text) => {
                length.expect = Some(Expect::StdoutLength {
                    reference: input.clone(),
                    chars: text.chars().count(),
                });
            }
            None => length.not_executable = Some(format!("{input} isn't readable text")),
        }
        draft.items.push(length);
    }
    let input_dir = input.rsplit_once('/').map_or("", |(d, _)| d);
    let input_tokens = stem_tokens(&input);
    let candidates: Vec<&String> = named
        .iter()
        .filter(|p| **p != input && !chosen.iter().any(|(_, c)| c == *p))
        .filter(|p| {
            p.rsplit_once('/').map_or("", |(d, _)| d) == input_dir
                || !stem_tokens(p).is_disjoint(&input_tokens)
        })
        .filter(|p| pristine.text(p).is_some())
        .take(3)
        .collect();
    let mut example = item(Kind::Example, "instruction", &unit.text);
    example.command = Some(command.clone());
    example.wall_sec = Some(wall);
    example.stated_bound = bound.is_some();
    if candidates.is_empty() {
        example.not_executable =
            Some("no file the task provides could hold the expected output".to_string());
    } else {
        for expected in candidates {
            draft.pending.push(Pending {
                id: String::new(),
                item: draft.items.len(),
                ask: Asking::Pairs {
                    command: command.clone(),
                    input: input.clone(),
                    expected: expected.clone(),
                    text: pristine.text(expected).unwrap_or_default().to_string(),
                },
            });
        }
    }
    draft.items.push(example);
}

/// An item for a command a document or a shell block states.
#[allow(clippy::too_many_arguments)]
fn doc_command(
    draft: &mut Draft,
    command: &str,
    line: &str,
    source: &str,
    workdir: &str,
    pristine: &Pristine,
    outputs: &BTreeSet<String>,
    placeholder: &Regex,
) {
    let mut entry = item(Kind::Command, source, line);
    entry.command = Some(command.to_string());
    entry.wall_sec = Some(COMMAND_SEC);
    let unresolved = command
        .split_whitespace()
        .skip(1)
        .filter(|w| !w.starts_with('-'))
        .filter_map(|w| as_path(w.trim_matches(['"', '\'']), workdir))
        .find(|p| pristine.stat(p) == Stat::Missing && !outputs.contains(p));
    if placeholder.is_match(command) || command.contains('[') || command.contains("...") {
        entry.not_executable = Some("a usage line with placeholders".to_string());
    } else if let Some(path) = unresolved {
        entry.not_executable = Some(format!(
            "it names {path}, which the untouched workspace doesn't have and the task doesn't \
             ask for"
        ));
    } else {
        draft.pending.push(Pending {
            id: String::new(),
            item: draft.items.len(),
            ask: Asking::Succeeds {
                command: command.to_string(),
                context: format!("{source}: {}", line.trim()),
            },
        });
    }
    draft.items.push(entry);
}

/// The Jev request that settles `pending`: one Noul per question.
#[must_use]
pub fn questions(pending: &[Pending]) -> Questions {
    let mut out = Questions::new();
    for p in pending {
        let noul = match &p.ask {
            Asking::Succeeds { command, context } => Noul::with_criteria(
                format!(
                    "The task in `task` mentions the shell command `{command}`, in this \
                     passage: \"{}\". Once the task is completed as asked, does the task \
                     state or clearly imply that running this command from the working \
                     directory should exit successfully?",
                    clip(context, 400)
                ),
                NoulCriteria::new()
                    .when_true(
                        "The task presents the command as a check that should pass or \
                         succeed once the work is done, such as a test suite to run or a \
                         reproducer that must stop failing.",
                    )
                    .when_false(
                        "The command is only a tool, a usage pattern, a way to see the \
                         original failure, or something the task doesn't say must succeed.",
                    ),
            ),
            Asking::Pairs {
                command,
                input,
                expected,
                ..
            } => Noul::with_criteria(
                format!(
                    "The task in `task` gives the example invocation `{command}`, whose input \
                     is the file {input}. Does the task state that the file {expected} holds \
                     the exact output this invocation should print, so that the printed \
                     output can be compared with that file?"
                ),
                NoulCriteria::new()
                    .when_true(format!(
                        "The task presents {expected} as the correct or corresponding output \
                         for {input}."
                    ))
                    .when_false(format!(
                        "The task doesn't pair the two files, or {expected} is something else, \
                         such as another input or a reference that isn't this program's output."
                    )),
            ),
        };
        out = out.with(p.id.clone(), noul);
    }
    out
}

/// The state the questions read: the task's words and nothing about any
/// candidate.
#[must_use]
pub fn state(instruction: &str) -> Value {
    json!({ "task": clip(strip_comments(instruction).trim(), 8000) })
}

/// Settles the draft's pending questions with `answers` (question ID to
/// Noul). An item whose question has no answer isn't executable.
#[must_use]
pub fn finish(mut draft: Draft, answers: &BTreeMap<String, f64>) -> Vec<Item> {
    let mut by_item: BTreeMap<usize, Vec<&Pending>> = BTreeMap::new();
    for p in &draft.pending {
        by_item.entry(p.item).or_default().push(p);
    }
    for (index, pending) in by_item {
        let item = &mut draft.items[index];
        let answered: Vec<(&Pending, f64)> = pending
            .iter()
            .filter_map(|p| Some((*p, *answers.get(&p.id)?)))
            .collect();
        item.decided_by = Some(json!(
            pending
                .iter()
                .map(|p| json!({ "question": p.id, "noul": answers.get(&p.id) }))
                .collect::<Vec<_>>()
        ));
        if answered.is_empty() {
            item.not_executable = Some(
                "the task's words don't settle what it should do, and Jev gave no answer"
                    .to_string(),
            );
            continue;
        }
        let best = answered
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .copied()
            .expect("at least one answer");
        if best.1 < THRESHOLD {
            item.not_executable = Some(match best.0.ask {
                Asking::Succeeds { .. } => format!("not stated to succeed (Jev {:.2})", best.1),
                Asking::Pairs { .. } => {
                    format!(
                        "no file is stated to hold the expected output (Jev {:.2})",
                        best.1
                    )
                }
            });
            continue;
        }
        match &best.0.ask {
            Asking::Succeeds { .. } => item.expect = Some(Expect::Exit { exit: Exit::Zero }),
            Asking::Pairs { expected, text, .. } => {
                item.expect = Some(Expect::StdoutEquals {
                    reference: expected.clone(),
                    text: text.clone(),
                });
            }
        }
    }
    draft.items
}

/// Makes a task's plan: gathers from the untouched workspace on `host`,
/// drafts, asks Jev what the words leave open, and seals the result.
/// `recorded` receives every live answer.
pub async fn plan(
    task: &str,
    instruction: &str,
    workdir: &str,
    host: &impl Host,
    mode: &JevMode,
    recorded: Option<&JevMode>,
    recorder: &Recorder,
) -> Plan {
    let pristine = gather(host, instruction, workdir).await;
    let draft = draft(instruction, workdir, &pristine);
    let mut answers = BTreeMap::new();
    let mut jev = Vec::new();
    if !draft.pending.is_empty() {
        let questions = questions(&draft.pending);
        let request = |mode| {
            ask(
                mode,
                recorder,
                Ask {
                    component: "verify.contract",
                    name: "jev_contract_span",
                    id: format!("contract-{task}"),
                    state: state(instruction),
                    questions: questions.clone(),
                    parent: None,
                    deadline: None,
                },
            )
        };
        let mut asked = match recorded {
            Some(replay) => request(replay).await,
            None => request(mode).await,
        };
        if !asked.answered() && recorded.is_some() && !matches!(mode, JevMode::Off) {
            asked = request(mode).await;
        }
        for p in &draft.pending {
            if let Some(p_yes) = asked.noul(&p.id) {
                answers.insert(p.id.clone(), p_yes);
            }
        }
        jev.push(json!({
            "key": asked.key,
            "how": asked.how,
            "questions": questions,
            "answers": asked.answers,
            "error": asked.error,
            "input_tokens": asked.input_tokens,
            "output_tokens": asked.output_tokens,
            "milliseconds": asked.milliseconds,
        }));
    }
    let items = finish(draft, &answers);
    Plan::seal(task, workdir, instruction, items, jev)
}
