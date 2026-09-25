//! `accept.grade`: grade the session-written score script at the freeze,
//! line by line (issue #9635).
//!
//! The lean loop's first session writes `score.sh`, iterates against it,
//! and the host freezes it after the session. At the freeze, code splits
//! the script into check lines and Jev answers, for each, whether its
//! expected result follows from the task's words, from the task's own
//! program as the host ran it, or from the standard definition of a named
//! method ([`support`], the support question). A line that follows
//! from none of them is kept and marked `advisory`.
//!
//! The host then scores each candidate with an instrumented copy of the
//! frozen script that also reports each line's result, and keep-best ranks
//! candidates on the lines graded `follows` first and the full score
//! second ([`key`]). No line, of any grade, can stop the loop or reverse
//! an edit.
//!
//! What counts as a check line, by code:
//!
//! - a call to a function the script defines that appends to a list or
//!   adds to a counter, such as `check(x)` or `ck(x)`, including the
//!   `check("name", lambda: x)` form;
//! - `NAME.append(x)` where the script starts `NAME` as an empty list;
//! - `if x: NAME += 1`, a counter the script adds to.
//!
//! Only the Python the script runs in a heredoc (`python3 - <<'PY'`) is
//! read. A script with no such line, or whose checks are one list literal,
//! doesn't split; it's graded as one advisory unit and the record says so.

pub mod offline;
pub mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::accept::authority;
use crate::component::jev::JevMode;
use crate::record::{Implementation, Recorder};
use support::{Basis, Item};

/// The component ID.
pub const COMPONENT: &str = "accept.grade";

/// The record's schema, as the run card reads it
/// (`crates/gym/src/runs_card.rs`).
pub const SCHEMA: &str = "openagents.coder-one.check-grades.v1";

/// The record's file name, in the lean group's artifacts directory.
pub const FILE: &str = "check-grades.json";

/// The probability at or above which a line's expectation follows. Fixed
/// before any retained script was graded: no task was available to fit
/// it on that the held-out rules allow
/// (`docs/terminal-bench/2026-09-25-check-grades-offline.md`).
pub const THRESHOLD: f64 = 0.5;

/// Whether the offline measurement admitted the graded ranking, so a
/// manifest may switch it on. It didn't: on the one task with both
/// outcomes, the graded key separated passes from failures for 1 of 6
/// scripts, the same one the raw score separated
/// (`docs/terminal-bench/2026-09-25-check-grades-offline.md`).
pub const ADMITTED: bool = false;

/// The Jev decision's name.
pub const DECISION: &str = "jev_check_grade";

/// The Jev decision's name for #9629's two class questions.
pub const AUTHORITY_DECISION: &str = "jev_check_grade_authority";

/// What each instrumented check line writes to standard error.
pub const MARK: &str = "OA-CHECK";

/// The most lines of setup a check line carries as context.
pub const CONTEXT_LINES: usize = 12;

/// How a script split.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    /// Into check lines.
    Lines,
    /// Not at all: the script is graded whole, as one advisory unit.
    OneUnit,
}

/// A line's grade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    /// Its expectation follows from the task, the baseline, or a standard
    /// definition.
    Follows,
    /// Jev answered and it follows from none of them, or the script didn't
    /// split.
    Advisory,
    /// Jev didn't answer.
    Unknown,
}

/// How to wrap a check line's value for instrumentation: the byte span of
/// the expression in the script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    /// The expression is a lambda's body; the wrapper parenthesizes it.
    pub lambda: bool,
}

/// One check line the splitter found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub id: String,
    /// The 1-based line of the script the check starts on.
    pub line: usize,
    /// The check statement as written.
    pub text: String,
    /// The lines before it that set up its inputs.
    pub context: String,
    pub span: Span,
}

/// A script, split.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub split: Split,
    pub sites: Vec<Site>,
    /// Why the script didn't split, for [`Split::OneUnit`].
    pub reason: Option<String>,
    /// Where each Python heredoc's body starts, as a byte offset.
    bodies: Vec<usize>,
}

/// A logical Python statement: its byte span in the script.
#[derive(Clone, Copy, Debug)]
struct Statement {
    start: usize,
    end: usize,
    /// Its indentation, in columns, when it starts a line.
    indent: Option<usize>,
}

/// The heredoc bodies that a Python interpreter reads: each as the byte
/// span of its lines, without the delimiter.
fn python_bodies(script: &str) -> Vec<(usize, usize)> {
    static OPEN: OnceLock<Regex> = OnceLock::new();
    let open = OPEN.get_or_init(|| {
        Regex::new(r#"\bpython3?(?:\.\d+)?\b[^<\n]*<<-?\s*['"]?([A-Za-z_][A-Za-z0-9_]*)['"]?"#)
            .expect("the heredoc pattern")
    });
    let mut bodies = Vec::new();
    let mut offset = 0;
    let lines: Vec<&str> = script.split_inclusive('\n').collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let next = offset + line.len();
        if let Some(caps) = open.captures(line) {
            let delimiter = caps[1].to_string();
            let start = next;
            let mut end = start;
            let mut k = i + 1;
            let mut at = next;
            while k < lines.len() {
                if lines[k].trim() == delimiter {
                    break;
                }
                at += lines[k].len();
                end = at;
                k += 1;
            }
            bodies.push((start, end));
            offset = at + lines.get(k).map_or(0, |l| l.len());
            i = k + 1;
            continue;
        }
        offset = next;
        i += 1;
    }
    bodies
}

/// Splits a Python body into logical statements: at newlines outside
/// brackets and strings, and at `;` outside them. Comments are skipped.
fn statements(script: &str, start: usize, end: usize) -> Vec<Statement> {
    let bytes = script.as_bytes();
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    let mut quote: Option<(u8, bool)> = None;
    let mut i = start;
    let mut stmt_start = start;
    let mut line_start = true;
    let mut indent = Some(0);
    let mut col = 0;
    let push = |out: &mut Vec<Statement>, s: usize, e: usize, indent: Option<usize>| {
        let text = &script[s..e];
        let lead = text.len() - text.trim_start().len();
        if !text.trim().is_empty() {
            out.push(Statement {
                start: s + lead,
                end: s + text.trim_end().len(),
                indent,
            });
        }
    };
    while i < end {
        let c = bytes[i];
        if let Some((q, triple)) = quote {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                if triple {
                    if i + 2 < end && bytes[i + 1] == q && bytes[i + 2] == q {
                        quote = None;
                        i += 3;
                        continue;
                    }
                } else {
                    quote = None;
                }
            } else if c == b'\n' && !triple {
                quote = None;
            }
            i += 1;
            continue;
        }
        if line_start {
            if c == b' ' || c == b'\t' {
                col += 1;
                i += 1;
                continue;
            }
            line_start = false;
            if depth == 0 {
                indent = Some(col);
                stmt_start = i;
            }
        }
        match c {
            b'#' => {
                // A comment runs to the end of the line.
                let mut k = i;
                while k < end && bytes[k] != b'\n' {
                    k += 1;
                }
                if depth == 0 {
                    push(&mut out, stmt_start, i, indent);
                    stmt_start = k;
                }
                i = k;
                continue;
            }
            b'\'' | b'"' => {
                let triple = i + 2 < end && bytes[i + 1] == c && bytes[i + 2] == c;
                quote = Some((c, triple));
                i += if triple { 3 } else { 1 };
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b';' if depth == 0 => {
                push(&mut out, stmt_start, i, indent);
                stmt_start = i + 1;
                indent = None;
            }
            b'\n' => {
                if depth <= 0 {
                    depth = 0;
                    push(&mut out, stmt_start, i, indent);
                    stmt_start = i + 1;
                }
                line_start = true;
                col = 0;
            }
            _ => {}
        }
        i += 1;
    }
    if stmt_start < end {
        push(&mut out, stmt_start, end, indent);
    }
    out
}

/// The byte offset of the parenthesis that closes the one at `open`, or
/// `None` when it doesn't close before `end`.
fn closing(script: &str, open: usize, end: usize) -> Option<usize> {
    let bytes = script.as_bytes();
    let mut depth = 0;
    let mut quote: Option<u8> = None;
    let mut i = open;
    while i < end {
        let c = bytes[i];
        if let Some(q) = quote {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'\'' | b'"' => quote = Some(c),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// The byte offsets of the top-level commas between `start` and `end`.
fn top_commas(script: &str, start: usize, end: usize) -> Vec<usize> {
    let bytes = script.as_bytes();
    let mut depth = 0;
    let mut quote: Option<u8> = None;
    let mut out = Vec::new();
    let mut i = start;
    while i < end {
        let c = bytes[i];
        if let Some(q) = quote {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'\'' | b'"' => quote = Some(c),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => out.push(i),
            _ => {}
        }
        i += 1;
    }
    out
}

/// A span with its surrounding whitespace dropped.
fn trimmed(script: &str, start: usize, end: usize) -> (usize, usize) {
    let text = &script[start..end];
    let lead = text.len() - text.trim_start().len();
    (start + lead, start + text.trim_end().len())
}

/// The span of the value a recorder call records: its only argument, or
/// the body of a trailing lambda, or its last argument.
fn call_value(script: &str, open: usize, close: usize) -> Option<Span> {
    let commas = top_commas(script, open + 1, close);
    let from = commas.last().map_or(open + 1, |c| c + 1);
    let (start, end) = trimmed(script, from, close);
    if start >= end {
        return None;
    }
    let arg = &script[start..end];
    if let Some(rest) = arg.strip_prefix("lambda") {
        // `lambda: body` or `lambda x: body`: the body starts after the
        // first top-level colon.
        let colon = rest.find(':')?;
        let (body_start, body_end) = trimmed(script, start + 6 + colon + 1, end);
        return (body_start < body_end).then_some(Span {
            start: body_start,
            end: body_end,
            lambda: true,
        });
    }
    Some(Span {
        start,
        end,
        lambda: false,
    })
}

fn line_of(script: &str, at: usize) -> usize {
    script[..at].bytes().filter(|b| *b == b'\n').count() + 1
}

/// Splits `script` into check lines.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn split(script: &str) -> Parsed {
    static DEF: OnceLock<Regex> = OnceLock::new();
    static EMPTY_LIST: OnceLock<Regex> = OnceLock::new();
    static LIST_LITERAL: OnceLock<Regex> = OnceLock::new();
    static IF_COUNT: OnceLock<Regex> = OnceLock::new();
    let def = DEF.get_or_init(|| Regex::new(r"^def\s+([A-Za-z_]\w*)\s*\(").expect("def"));
    let empty_list = EMPTY_LIST
        .get_or_init(|| Regex::new(r"^([A-Za-z_]\w*)\s*=\s*\[\s*\]$").expect("empty list"));
    let list_literal =
        LIST_LITERAL.get_or_init(|| Regex::new(r"^([A-Za-z_]\w*)\s*=\s*\[").expect("list literal"));
    let if_count = IF_COUNT.get_or_init(|| {
        Regex::new(r"^if\s+(.+):\s*([A-Za-z_]\w*)\s*\+=\s*1\s*$").expect("if count")
    });
    let one = |reason: &str, bodies: Vec<usize>| Parsed {
        split: Split::OneUnit,
        sites: Vec::new(),
        reason: Some(reason.to_string()),
        bodies,
    };
    let bodies = python_bodies(script);
    if bodies.is_empty() {
        return one("the script runs no Python heredoc to split", Vec::new());
    }
    let starts: Vec<usize> = bodies.iter().map(|(s, _)| *s).collect();
    let mut sites = Vec::new();
    let mut literal_checks = false;
    for (body_start, body_end) in &bodies {
        let stmts = statements(script, *body_start, *body_end);
        // Recorders: top-level functions whose body appends or adds, and
        // lists started empty. Statements inside a function body are
        // never check lines.
        let mut recorders: BTreeSet<String> = BTreeSet::new();
        let mut lists: BTreeSet<String> = BTreeSet::new();
        let mut in_def: Option<(String, usize)> = None;
        let mut def_body: Vec<(String, String)> = Vec::new();
        let mut top: Vec<Statement> = Vec::new();
        for s in &stmts {
            let text = &script[s.start..s.end];
            if let Some((_, indent)) = &in_def {
                match s.indent {
                    Some(i) if i <= *indent => in_def = None,
                    _ => {
                        if let Some((name, _)) = &in_def {
                            def_body.push((name.clone(), text.to_string()));
                        }
                        continue;
                    }
                }
            }
            if s.indent == Some(0)
                && let Some(caps) = def.captures(text)
            {
                let name = caps[1].to_string();
                // A one-line def carries its body after the colon.
                if let Some((_, body)) = text.split_once("):") {
                    def_body.push((name.clone(), body.to_string()));
                }
                in_def = Some((name, 0));
                continue;
            }
            top.push(*s);
        }
        for (name, body) in &def_body {
            if body.contains(".append(") || body.contains("+=") {
                recorders.insert(name.clone());
            }
        }
        for s in &top {
            let text = &script[s.start..s.end];
            if let Some(caps) = empty_list.captures(text) {
                lists.insert(caps[1].to_string());
            } else if let Some(caps) = list_literal.captures(text) {
                let name = &caps[1];
                let used = script.contains(&format!("sum({name}"))
                    || script.contains(&format!("all({name}"));
                if used && !top_commas(script, s.start + caps[0].len(), s.end).is_empty() {
                    literal_checks = true;
                }
            }
        }
        let mut alternatives: Vec<String> = recorders.iter().map(|n| regex::escape(n)).collect();
        alternatives.extend(
            lists
                .iter()
                .map(|n| format!(r"{}\s*\.\s*append", regex::escape(n))),
        );
        let call = (!alternatives.is_empty())
            .then(|| Regex::new(&format!(r"(?:^|[^.\w])(?:{})\s*\(", alternatives.join("|"))).ok())
            .flatten();
        let mut previous_line = line_of(script, *body_start);
        for s in &top {
            let text = &script[s.start..s.end];
            let mut span = None;
            if let Some(call) = &call
                && let Some(caps) = call.captures(text)
            {
                let name_end = s.start + caps.get(0).map_or(0, |m| m.end());
                let open = name_end - 1;
                if let Some(close) = closing(script, open, s.end.max(open + 1)) {
                    span = call_value(script, open, close);
                }
            } else if let Some(caps) = if_count.captures(text) {
                let cond = caps.get(1).map_or((0, 0), |m| (m.start(), m.end()));
                let (start, end) = trimmed(script, s.start + cond.0, s.start + cond.1);
                if start < end {
                    span = Some(Span {
                        start,
                        end,
                        lambda: false,
                    });
                }
            }
            let Some(span) = span else {
                continue;
            };
            let line = line_of(script, s.start);
            let context: Vec<&str> = script
                .lines()
                .skip(previous_line.saturating_sub(1))
                .take(line.saturating_sub(previous_line))
                .collect();
            let context = context[context.len().saturating_sub(CONTEXT_LINES)..].join("\n");
            let prefix = script[..s.start]
                .rsplit('\n')
                .next()
                .unwrap_or_default()
                .trim();
            let context = if prefix.is_empty() {
                context
            } else {
                format!("{context}\n{prefix}").trim().to_string()
            };
            sites.push(Site {
                id: format!("c{}", sites.len() + 1),
                line,
                text: text.to_string(),
                context,
                span,
            });
            previous_line = line + 1;
        }
    }
    if literal_checks && sites.is_empty() {
        return one("the checks are one list literal", starts);
    }
    if sites.is_empty() {
        return one("no check line was found", starts);
    }
    Parsed {
        split: Split::Lines,
        sites,
        reason: None,
        bodies: starts,
    }
}

/// The helper the instrumented script defines at the top of each Python
/// heredoc. It writes each check's result to standard error and returns
/// the value unchanged, so the script scores exactly as before.
pub const PRELUDE: &str = "import sys as _oa_sys\n\
def _oa_mark(i, v):\n    \
try:\n        b = bool(v)\n    \
except Exception:\n        b = False\n    \
_oa_sys.stderr.write('OA-CHECK %s %d\\n' % (i, int(b)))\n    \
_oa_sys.stderr.flush()\n    \
return v\n";

/// An instrumented copy of `script` whose check lines report their
/// results, or `None` when it can't be made: the script didn't split, or a
/// heredoc body opens indented or with a `__future__` import.
#[must_use]
pub fn instrument(script: &str, parsed: &Parsed) -> Option<String> {
    if parsed.split != Split::Lines {
        return None;
    }
    let mut edits: Vec<(usize, String)> = Vec::new();
    for body in &parsed.bodies {
        let first = script[*body..]
            .lines()
            .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))?;
        if first.starts_with([' ', '\t']) || first.contains("__future__") {
            return None;
        }
        edits.push((*body, PRELUDE.to_string()));
    }
    for site in &parsed.sites {
        let (open, close) = if site.span.lambda {
            (format!("_oa_mark('{}', (", site.id), "))")
        } else {
            (format!("_oa_mark('{}', ", site.id), ")")
        };
        edits.push((site.span.start, open));
        edits.push((site.span.end, close.to_string()));
    }
    // Apply from the end so earlier offsets stay valid.
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
    let mut out = script.to_string();
    for (at, text) in edits {
        out.insert_str(at, &text);
    }
    Some(out)
}

/// Each check line's result from an instrumented run's output: a line
/// passes when it reported at least once and every report passed. A line
/// that never reported failed, when any line reported; with no reports
/// at all the results are unknown and the map is empty.
#[must_use]
pub fn results(output: &str, parsed: &Parsed) -> BTreeMap<String, bool> {
    let mut seen: BTreeMap<String, bool> = BTreeMap::new();
    for line in output.lines() {
        let Some(rest) = line.trim().strip_prefix(MARK) else {
            continue;
        };
        let mut parts = rest.split_whitespace();
        let (Some(id), Some(ok)) = (parts.next(), parts.next()) else {
            continue;
        };
        let ok = ok == "1";
        seen.entry(id.to_string())
            .and_modify(|all| *all &= ok)
            .or_insert(ok);
    }
    if seen.is_empty() {
        return seen;
    }
    parsed
        .sites
        .iter()
        .map(|site| {
            (
                site.id.clone(),
                seen.get(&site.id).copied().unwrap_or(false),
            )
        })
        .collect()
}

/// `output` without the instrumented lines' reports, for the score's tail.
#[must_use]
pub fn without_marks(output: &str) -> String {
    output
        .lines()
        .filter(|line| !line.trim_start().starts_with(MARK))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Jev's answer in the record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JevNote {
    pub how: String,
    pub error: Option<String>,
}

/// One line's result on one session's candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineResult {
    pub session: u32,
    pub passed: bool,
}

/// One graded line of the record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub id: String,
    pub line: usize,
    pub text: String,
    pub grade: Grade,
    pub basis: Option<Basis>,
    pub p: Option<f64>,
    pub jev: JevNote,
    #[serde(default)]
    pub results: Vec<LineResult>,
    /// Each basis's probability, beside the record's shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support: Option<Value>,
    /// The line's authority class (#9629) and the evidence it rests on,
    /// beside the record's shape. `grade` follows from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority: Option<authority::Classified>,
}

/// The check-grades record the run card reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grades {
    pub schema: String,
    /// The frozen check's path under `artifacts/`.
    pub check: String,
    pub check_digest: String,
    pub frozen_after_session: u32,
    pub split: Split,
    pub lines: Vec<Line>,
    /// Why the script didn't split, for [`Split::OneUnit`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Whether Jev read the task's baseline behavior (#9633); without it
    /// a line is graded on the task's words and the standard definition
    /// only.
    #[serde(default)]
    pub baseline: bool,
    /// The support question set's ID and digest, and the threshold.
    #[serde(default)]
    pub question_set: Value,
    #[serde(default)]
    pub threshold: f64,
    /// What Jev cost, in dollars.
    #[serde(default)]
    pub jev_usd: f64,
}

impl Grades {
    /// Appends one session's line results, when they're known.
    pub fn record(&mut self, session: u32, passed: &BTreeMap<String, bool>) {
        if passed.is_empty() {
            return;
        }
        for line in &mut self.lines {
            if let Some(ok) = passed.get(&line.id) {
                line.results.push(LineResult {
                    session,
                    passed: *ok,
                });
            }
        }
    }

    /// The IDs of the lines graded `follows`.
    #[must_use]
    pub fn supported_ids(&self) -> BTreeSet<&str> {
        self.lines
            .iter()
            .filter(|l| l.grade == Grade::Follows)
            .map(|l| l.id.as_str())
            .collect()
    }

    /// Passed and total over the lines graded `follows`, from one run's
    /// results, or `None` when the results are unknown.
    #[must_use]
    pub fn supported(&self, passed: &BTreeMap<String, bool>) -> Option<(u64, u64)> {
        if passed.is_empty() {
            return None;
        }
        let ids = self.supported_ids();
        let total = ids.len() as u64;
        let ok = ids
            .iter()
            .filter(|id| passed.get(**id).copied().unwrap_or(false))
            .count() as u64;
        Some((ok, total))
    }
}

/// A score as a fraction for ranking: -1 when unknown, and 0 for no lines.
#[must_use]
pub fn fraction(score: Option<(u64, u64)>) -> f64 {
    match score {
        None => -1.0,
        Some((_, 0)) => 0.0,
        Some((p, t)) => p as f64 / t as f64,
    }
}

/// The keep-best key: the lines graded `follows` first, the full score
/// second.
#[must_use]
pub fn key(supported: Option<(u64, u64)>, score: Option<(u64, u64)>) -> (f64, f64) {
    (fraction(supported), fraction(score))
}

/// Whether key `a` ranks above `b`, or, with `ties`, at least level.
#[must_use]
pub fn ahead(a: (f64, f64), b: (f64, f64), ties: bool) -> bool {
    match a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Equal => ties,
        std::cmp::Ordering::Less => false,
    }
}

/// Where a grading runs, and what it's told.
pub struct Freeze<'a> {
    /// The frozen check's path under `artifacts/`, as the record names it.
    pub check: String,
    pub script: &'a str,
    pub frozen_after_session: u32,
    pub task: &'a str,
    /// The task's baseline behavior (#9633), when the host has it.
    pub baseline: Option<&'a str>,
    /// Each check line's result on the untouched workspace, from the
    /// instrumented copy ([`results`]); `None` when it wasn't run there.
    /// It decides a line's authority class: green there is a guard.
    pub start: Option<&'a BTreeMap<String, bool>>,
}

/// Splits and grades a frozen script. Returns the record, the split, and
/// one record per Jev request.
pub async fn grade(
    jev: &JevMode,
    recorder: &Recorder,
    context: &support::Context<'_>,
    freeze: &Freeze<'_>,
) -> (Grades, Parsed, Vec<Value>) {
    let parsed = split(freeze.script);
    let set = support::support_set();
    let mut grades = Grades {
        schema: SCHEMA.to_string(),
        check: freeze.check.clone(),
        check_digest: crate::accept::sha256(freeze.script.as_bytes()),
        frozen_after_session: freeze.frozen_after_session,
        split: parsed.split,
        lines: Vec::new(),
        reason: parsed.reason.clone(),
        baseline: freeze.baseline.is_some(),
        question_set: json!({ "id": set.id, "digest": set.digest }),
        threshold: THRESHOLD,
        jev_usd: 0.0,
    };
    if parsed.split == Split::OneUnit {
        grades.lines.push(Line {
            id: "u1".to_string(),
            line: 1,
            text: crate::judge::clip(freeze.script.trim(), 600),
            grade: Grade::Advisory,
            basis: None,
            p: None,
            jev: JevNote {
                how: "skipped".to_string(),
                error: parsed.reason.clone(),
            },
            results: Vec::new(),
            support: None,
            authority: None,
        });
        return (grades, parsed, Vec::new());
    }
    let items: Vec<Item> = parsed
        .sites
        .iter()
        .map(|site| Item {
            id: site.id.clone(),
            text: site.text.clone(),
            context: site.context.clone(),
        })
        .collect();
    let asked = support::ask(jev, recorder, context, freeze.task, freeze.baseline, &items).await;
    grades.jev_usd = asked.usd;
    let mut calls = asked.calls;
    for (n, (site, support)) in parsed.sites.iter().zip(&asked.supports).enumerate() {
        let best = support.best();
        // #9629's evidence for the line: its run on the untouched
        // workspace, and the support answer in the place of the
        // faithfulness judgment. A line red there and supported is asked
        // #9629's two narrow questions, which tell an independently
        // supported expectation from a writer-derived one.
        let green_at_start = freeze
            .start
            .filter(|start| !start.is_empty())
            .map(|start| start.get(&site.id).copied().unwrap_or(false));
        let mut evidence = authority::Evidence {
            green_at_start,
            faithful: best.map(|(_, p)| p),
            faithful_from: best.map(|_| COMPONENT.to_string()),
            routes: authority::reference_routes(&format!("{}\n{}", site.context, site.text)),
            ..authority::Evidence::default()
        };
        if green_at_start == Some(false)
            && best.is_some_and(|(_, p)| crate::decision::GRADE_FAITHFUL.yes(p))
        {
            let answer = crate::component::jev::ask(
                jev,
                recorder,
                crate::component::jev::Ask {
                    component: context.component,
                    name: AUTHORITY_DECISION,
                    id: format!("{}-authority-{n}", context.id),
                    state: json!({
                        "task": crate::judge::clip(freeze.task.trim(), support::TASK_CHARS),
                        "test": crate::judge::clip(
                            &format!("{}\n{}", site.context, site.text),
                            2 * support::ITEM_CHARS,
                        ),
                    }),
                    questions: authority::questions(false),
                    parent: None,
                    deadline: context.deadline.clone(),
                },
            )
            .await;
            grades.jev_usd += answer.input_tokens.map_or(0.0, |t| {
                t as f64 * crate::component::jev::USD_PER_MILLION_INPUT / 1_000_000.0
            });
            calls.push(json!({
                "line": site.id,
                "how": answer.how,
                "error": answer.error,
                "input_tokens": answer.input_tokens,
            }));
            evidence.separate_route = answer.noul("separate_route");
            evidence.expected_correct = answer.noul("expected_correct");
            evidence.jev_key = Some(answer.key.clone());
        }
        let (grade, basis, classified) = line_grade(best, evidence);
        grades.lines.push(Line {
            id: site.id.clone(),
            line: site.line,
            text: site.text.clone(),
            grade,
            basis,
            p: best.map(|(_, p)| p),
            jev: JevNote {
                how: support.how.clone(),
                error: support.error.clone(),
            },
            results: Vec::new(),
            support: Some(json!({
                "task": support.task,
                "baseline": support.baseline,
                "standard": support.standard,
            })),
            authority: Some(classified),
        });
    }
    (grades, parsed, calls)
}

/// A line's grade, basis, and authority class from its best support
/// answer and its evidence. The class is #9629's ([`authority::classify`]),
/// with [`THRESHOLD`] as the faithfulness bound, and it sets the grade: a
/// class that may rank follows, and a guard or an unsupported line is
/// advisory. With no support answer the line is unknown.
#[must_use]
pub fn line_grade(
    best: Option<(Basis, f64)>,
    evidence: authority::Evidence,
) -> (Grade, Option<Basis>, authority::Classified) {
    let thresholds = authority::Thresholds {
        faithful_min: THRESHOLD,
        ..authority::Thresholds::default()
    };
    let (class, why) = authority::classify(&evidence, &thresholds);
    let (grade, basis) = match best {
        None => (Grade::Unknown, None),
        Some((basis, _)) if class.can_rank() => (Grade::Follows, Some(basis)),
        Some(_) => (Grade::Advisory, None),
    };
    (
        grade,
        basis,
        authority::Classified {
            class,
            why,
            evidence,
        },
    )
}

/// The implementation record: the question set, the threshold, and the
/// bounds.
#[must_use]
pub fn implementation() -> Implementation {
    let set = support::support_set();
    Implementation::new(
        COMPONENT,
        "check lines split by code, each graded by Jev's support question",
        &json!({
            "set": { "id": set.id, "digest": set.digest },
            "threshold": THRESHOLD,
            "batch": support::BATCH,
            "context_lines": CONTEXT_LINES,
        }),
    )
}

#[cfg(test)]
mod tests;
