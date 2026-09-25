//! `evidence.departures`: suspects widened from justifying comments to
//! departures (issue #9634).
//!
//! Three miners read the untouched workspace and each produce the same
//! likely-defect row (file, line, text, probability) with its kind:
//!
//! - **`rationale`**: comments that give a reason for a choice
//!   ([`crate::accept::rationale_choices`]), the v13 miner. Jev asks
//!   whether the behavior a comment justifies could cause a problem the
//!   task describes.
//! - **`docstring`**: every function with a docstring. Jev asks whether
//!   the code, or the way its call sites use it, departs from what the
//!   docstring says.
//! - **`standard-method`**: functions whose name, docstring, or imported
//!   names match a well-known method in `standard-methods.json` beside
//!   this module, a versioned, digested list that holds no task text.
//!   Jev asks whether the body departs from the method's standard
//!   definition.
//!
//! Code finds every candidate; Jev only ranks them. Each question's
//! wording lives in a digested question set under `questions/`, and every
//! request is recorded like any other Jev call. The miners never see a
//! verifier, a reference solution, or the task anatomy.
//!

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::component::jev::{self as jev_component, JevMode};
use crate::record::{Implementation, Recorder};

/// The component ID.
pub const COMPONENT: &str = "evidence.departures";

/// Candidates per source, at most, in workspace order.
pub const MAX_CANDIDATES: usize = 24;

/// Rows per source the briefing lists, at most.
pub const MAX_LISTED: usize = 8;

/// Candidates per Jev request for the docstring and standard-method
/// sources, whose evidence carries function bodies.
pub const BATCH: usize = 6;

/// The most characters of one function body a candidate carries.
pub const BODY_CHARS: usize = 2_500;

/// The call sites a docstring candidate carries, at most.
pub const MAX_CALLERS: usize = 4;

/// The sources a manifest may switch on beside `rationale`, which has its
/// own switch. A source is admitted only when the offline measurement on
/// the task anatomy's 18 tasks shows it raises recall without dropping
/// precision below the comment miner's
/// (`docs/terminal-bench/2026-09-25-departures-offline.md`). It admitted
/// neither: `docstring` listed nothing at 0.5, and `standard-method`
/// raised recall at a precision below the comment miner's.
pub const ADMITTED: &[Source] = &[];

/// Where a likely-defect row comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// A comment that justifies a choice.
    Rationale,
    /// A function whose code departs from its docstring.
    Docstring,
    /// A function that departs from the standard method it names.
    StandardMethod,
}

impl Source {
    /// Every source, in briefing order.
    pub const ALL: [Source; 3] = [Source::Rationale, Source::Docstring, Source::StandardMethod];

    /// The source as manifests and records spell it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Source::Rationale => "rationale",
            Source::Docstring => "docstring",
            Source::StandardMethod => "standard-method",
        }
    }

    /// The probability at or above which a row is listed. Every source
    /// keeps v13's 0.5: the offline measurement's fit tasks held no
    /// candidate to choose another on.
    #[must_use]
    pub fn threshold(self) -> f64 {
        match self {
            Source::Rationale => RATIONALE_P,
            Source::Docstring => DOCSTRING_P,
            Source::StandardMethod => STANDARD_METHOD_P,
        }
    }

    /// The state field the candidates go under.
    fn state_key(self) -> &'static str {
        match self {
            Source::Rationale => "comments",
            Source::Docstring => "functions",
            Source::StandardMethod => "implementations",
        }
    }

    /// The question ID prefix; `rationale` keeps v13's.
    fn question_prefix(self) -> &'static str {
        match self {
            Source::Rationale => "suspect",
            Source::Docstring => "docstring",
            Source::StandardMethod => "method",
        }
    }

    /// Candidates per request: v13 asked every comment in one.
    fn batch(self) -> usize {
        match self {
            Source::Rationale => MAX_CANDIDATES,
            Source::Docstring | Source::StandardMethod => BATCH,
        }
    }

    /// The Jev decision's name; `rationale` keeps v13's.
    fn decision(self) -> &'static str {
        match self {
            Source::Rationale => "jev_suspects",
            Source::Docstring => "jev_departures_docstring",
            Source::StandardMethod => "jev_departures_standard_method",
        }
    }

    /// The question set's file text.
    fn set_text(self) -> &'static str {
        match self {
            Source::Rationale => include_str!("../../../../questions/departure-rationale.json"),
            Source::Docstring => include_str!("../../../../questions/departure-docstring.json"),
            Source::StandardMethod => {
                include_str!("../../../../questions/departure-standard-method.json")
            }
        }
    }
}

/// v13's listing threshold for justifying comments.
pub const RATIONALE_P: f64 = 0.5;

/// The docstring source's listing threshold. The protocol's rule keeps
/// v13's value when the fit tasks hold no labeled hit, as they don't.
pub const DOCSTRING_P: f64 = 0.5;

/// The standard-method source's listing threshold, by the same rule.
pub const STANDARD_METHOD_P: f64 = 0.5;

/// One source's question set, as the repository holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestionSet {
    pub id: String,
    /// `atif::digest` of `{"per_finding": template}`, the digest the
    /// question-set registry in `crates/coder` computes.
    pub digest: String,
    /// The template's instructions, with `{finding}` where the
    /// candidate's state path goes.
    pub instructions: String,
}

/// What the template writes the candidate's state path into.
pub const FINDING: &str = "{finding}";

fn parse_set(text: &str) -> QuestionSet {
    let value: Value = serde_json::from_str(text).expect("a departure question set is JSON");
    let template = value
        .get("per_finding")
        .cloned()
        .expect("a departure question set is a per_finding template");
    let instructions = template
        .get("instructions")
        .and_then(Value::as_str)
        .expect("a departure question set has instructions")
        .to_string();
    QuestionSet {
        id: value["id"].as_str().unwrap_or_default().to_string(),
        digest: atif::digest(&json!({ "per_finding": template })),
        instructions,
    }
}

/// The question set a source asks from.
#[must_use]
pub fn question_set(source: Source) -> &'static QuestionSet {
    static SETS: OnceLock<BTreeMap<Source, QuestionSet>> = OnceLock::new();
    &SETS.get_or_init(|| {
        Source::ALL
            .iter()
            .map(|source| (*source, parse_set(source.set_text())))
            .collect()
    })[&source]
}

/// The question for candidate `j` of a request.
#[must_use]
pub fn question(source: Source, j: usize) -> String {
    question_set(source)
        .instructions
        .replace(FINDING, &format!("{}[{j}]", source.state_key()))
}

/// The standard-method list's file text.
pub const METHODS_TEXT: &str = include_str!("standard-methods.json");

/// The standard-method list's schema.
pub const METHODS_SCHEMA: &str = "openagents.coder-one.standard-methods.v1";

/// A versioned list of well-known methods.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MethodList {
    pub schema: String,
    pub version: u32,
    #[serde(default)]
    pub about: String,
    pub methods: Vec<Method>,
}

/// One well-known method.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Method {
    pub id: String,
    pub family: String,
    /// Phrases that name it, matched as whole words after normalization.
    pub names: Vec<String>,
    /// Its standard definition, in one or two sentences.
    pub definition: String,
}

/// The standard-method list and its digest.
#[must_use]
pub fn methods() -> &'static (MethodList, String) {
    static LIST: OnceLock<(MethodList, String)> = OnceLock::new();
    LIST.get_or_init(|| {
        let list: MethodList =
            serde_json::from_str(METHODS_TEXT).expect("standard-methods.json is a method list");
        let digest = atif::digest(&serde_json::to_value(&list).unwrap_or_default());
        (list, digest)
    })
}

/// A suspect code found, before Jev ranks it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub kind: Source,
    pub file: String,
    pub line: usize,
    /// The row's text: the comment, or the function and what it names.
    pub text: String,
    /// What Jev reads for this candidate.
    pub evidence: Value,
}

/// A likely-defect row: a candidate and Jev's probability that it names a
/// defect, `None` when Jev didn't answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub kind: Source,
    pub file: String,
    pub line: usize,
    pub text: String,
    pub p: Option<f64>,
}

impl Row {
    /// The row as the briefing shows it, kind first.
    #[must_use]
    pub fn line_text(&self) -> String {
        format!(
            "[{}] {}:{}: {} (p = {:.2})",
            self.kind.word(),
            self.file,
            self.line,
            self.text,
            self.p.unwrap_or(0.0)
        )
    }
}

/// The candidates one source finds in `workspace`.
#[must_use]
pub fn mine(source: Source, workspace: &Path) -> Vec<Candidate> {
    match source {
        Source::Rationale => rationale(workspace),
        Source::Docstring => docstrings(workspace),
        Source::StandardMethod => standard_methods(workspace, &methods().0),
    }
}

/// The v13 comments, as candidates whose evidence is the comment line
/// exactly as v13 put it in the state.
#[must_use]
pub fn rationale(workspace: &Path) -> Vec<Candidate> {
    crate::accept::rationale_choices(workspace)
        .into_iter()
        .take(MAX_CANDIDATES)
        .map(|comment| {
            let mut parts = comment.splitn(3, ':');
            let file = parts.next().unwrap_or_default().to_string();
            let line = parts
                .next()
                .and_then(|n| n.trim().parse().ok())
                .unwrap_or(0);
            let text = parts.next().unwrap_or_default().trim().to_string();
            Candidate {
                kind: Source::Rationale,
                file,
                line,
                text,
                evidence: Value::String(comment),
            }
        })
        .collect()
}

/// A function the extractor found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function {
    pub file: String,
    /// The 1-based line of its definition.
    pub line: usize,
    pub name: String,
    pub docstring: Option<String>,
    /// Its definition through the end of its parameter list.
    pub signature: String,
    /// Its source from the definition line, clipped to [`BODY_CHARS`].
    pub body: String,
}

/// Every function in the workspace's production source files, in file
/// order: Python by indentation, brace languages by matching braces.
#[must_use]
pub fn functions(workspace: &Path) -> Vec<Function> {
    let mut out = Vec::new();
    for path in crate::accept::source_files(workspace, 60) {
        let Ok(text) = std::fs::read_to_string(workspace.join(&path)) else {
            continue;
        };
        match path.rsplit_once('.').map(|(_, ext)| ext) {
            Some("py") => out.extend(python_functions(&path, &text)),
            Some("sh" | "rb") | None => {}
            Some(_) => out.extend(brace_functions(&path, &text)),
        }
    }
    out
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Python functions and classes: `def` and `class` lines, their
/// docstrings, and their bodies to the first line indented no deeper than
/// the definition.
#[must_use]
pub fn python_functions(path: &str, text: &str) -> Vec<Function> {
    static DEF: OnceLock<Regex> = OnceLock::new();
    let def = DEF.get_or_init(|| {
        Regex::new(r"^(\s*)(?:async\s+def|def|class)\s+([A-Za-z_]\w*)\s*[(:]")
            .expect("the def pattern")
    });
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(caps) = def.captures(line) else {
            continue;
        };
        let indent = caps[1].len();
        let name = caps[2].to_string();
        // The signature ends at the first line where the parentheses
        // close and the line ends with a colon.
        let mut depth: i32 = 0;
        let mut sig_end = i;
        for (k, l) in lines.iter().enumerate().skip(i).take(30) {
            let code = l.split('#').next().unwrap_or(l);
            for c in code.chars() {
                match c {
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' => depth -= 1,
                    _ => {}
                }
            }
            sig_end = k;
            if depth <= 0 && code.trim_end().ends_with(':') {
                break;
            }
        }
        let mut end = sig_end + 1;
        while end < lines.len() {
            let l = lines[end];
            if !l.trim().is_empty() && indent_of(l) <= indent {
                break;
            }
            end += 1;
        }
        while end > sig_end + 1 && lines[end - 1].trim().is_empty() {
            end -= 1;
        }
        let docstring = python_docstring(&lines[(sig_end + 1).min(lines.len())..end]);
        let body = crate::judge::clip(&lines[i..end].join("\n"), BODY_CHARS);
        out.push(Function {
            file: path.to_string(),
            line: i + 1,
            name,
            docstring,
            signature: lines[i..=sig_end.min(lines.len() - 1)].join(" "),
            body,
        });
    }
    out
}

/// The docstring a Python body opens with, if it opens with one.
fn python_docstring(body: &[&str]) -> Option<String> {
    let mut rest = body.iter().skip_while(|l| l.trim().is_empty());
    let first = rest.next()?.trim();
    let first = first.trim_start_matches(['r', 'R', 'u', 'U', 'b', 'B']);
    let quote = if first.starts_with("\"\"\"") {
        "\"\"\""
    } else if first.starts_with("'''") {
        "'''"
    } else {
        return None;
    };
    let opened = &first[3..];
    if let Some(end) = opened.find(quote) {
        return Some(opened[..end].trim().to_string());
    }
    let mut text = vec![opened.trim().to_string()];
    for l in rest.take(60) {
        if let Some(end) = l.find(quote) {
            text.push(l[..end].trim().to_string());
            break;
        }
        text.push(l.trim().to_string());
    }
    let joined = text.join("\n").trim().to_string();
    (!joined.is_empty()).then_some(joined)
}

/// Words that open a brace block but name no function.
const NOT_FUNCTIONS: [&str; 12] = [
    "if", "for", "while", "switch", "catch", "return", "else", "do", "sizeof", "function", "with",
    "match",
];

/// Functions in brace languages (JavaScript, TypeScript, Rust, Go, Java,
/// C, C++): a definition line, the doc comment right above it, and the
/// body to its matching brace.
#[must_use]
pub fn brace_functions(path: &str, text: &str) -> Vec<Function> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        [
            r"^\s*(?:export\s+)?(?:default\s+)?(?:async\s+)?function\s*\*?\s*([A-Za-z_$][\w$]*)\s*[(<]",
            r"^\s*(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*(?::[^=]+)?=\s*(?:async\s*)?(?:\([^)]*\)|[A-Za-z_$][\w$]*)\s*(?::[^=]+)?=>",
            r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+([A-Za-z_]\w*)",
            r"^\s*func\s+(?:\([^)]*\)\s*)?([A-Za-z_]\w*)\s*\(",
            r"^\s*(?:(?:public|private|protected|static|async|override|readonly|virtual|inline|extern|final)\s+)*(?:[A-Za-z_][\w:<>,\*&\[\]]*\s+[\*&]*)?([A-Za-z_]\w*)\s*\([^;{}]*\)\s*(?::\s*[^{;=]+)?(?:const\s*)?(?:noexcept\s*)?\{?\s*$",
        ]
        .iter()
        .map(|p| Regex::new(p).expect("a function pattern"))
        .collect()
    });
    let rust = path.ends_with(".rs");
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        let name = if trimmed.starts_with("//") || trimmed.starts_with('*') {
            None
        } else {
            patterns
                .iter()
                .find_map(|p| p.captures(line).map(|c| c[1].to_string()))
                .filter(|n| !NOT_FUNCTIONS.contains(&n.as_str()))
        };
        let Some(name) = name else {
            i += 1;
            continue;
        };
        // The body opens at the first brace within a few lines.
        let Some(open) = (i..lines.len().min(i + 8)).find(|&k| lines[k].contains('{')) else {
            i += 1;
            continue;
        };
        if lines[i..open].iter().any(|l| l.trim_end().ends_with(';')) {
            i += 1;
            continue;
        }
        let mut depth: i32 = 0;
        let mut end = open;
        'scan: for (k, l) in lines.iter().enumerate().skip(open).take(300) {
            let mut quote: Option<char> = None;
            let mut prev = ' ';
            for c in l.chars() {
                match quote {
                    Some(q) => {
                        if c == q && prev != '\\' {
                            quote = None;
                        }
                    }
                    None => match c {
                        // A Rust lifetime isn't a quote.
                        '\'' if rust => {}
                        '"' | '\'' | '`' => quote = Some(c),
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    },
                }
                prev = c;
            }
            end = k;
            if depth <= 0 {
                break 'scan;
            }
        }
        out.push(Function {
            file: path.to_string(),
            line: i + 1,
            name,
            docstring: doc_comment_above(&lines, i),
            signature: lines[i..=open].join(" "),
            body: crate::judge::clip(&lines[i..=end].join("\n"), BODY_CHARS),
        });
        i += 1;
    }
    out
}

/// The doc comment that ends right above line `i`: a `/** … */` block or
/// a run of `///` or `//` lines.
fn doc_comment_above(lines: &[&str], i: usize) -> Option<String> {
    let mut k = i;
    // Decorators and attributes sit between a doc comment and its item.
    while k > 0 {
        let t = lines[k - 1].trim();
        if t.starts_with('@') || t.starts_with("#[") {
            k -= 1;
        } else {
            break;
        }
    }
    if k == 0 {
        return None;
    }
    let above = lines[k - 1].trim();
    let mut text = Vec::new();
    if above.ends_with("*/") {
        let mut j = k;
        while j > 0 {
            j -= 1;
            let t = lines[j].trim();
            text.push(
                t.trim_start_matches("/**")
                    .trim_start_matches("/*")
                    .trim_end_matches("*/")
                    .trim_start_matches('*')
                    .trim()
                    .to_string(),
            );
            if t.starts_with("/*") || i - j > 60 {
                break;
            }
        }
    } else if above.starts_with("//") {
        let mut j = k;
        while j > 0 && lines[j - 1].trim().starts_with("//") && i - j < 60 {
            j -= 1;
            text.push(lines[j].trim().trim_start_matches('/').trim().to_string());
        }
    } else {
        return None;
    }
    text.reverse();
    let joined = text.join("\n").trim().to_string();
    (!joined.is_empty()).then_some(joined)
}

/// The first non-empty line of `text`, clipped.
fn first_line(text: &str, max: usize) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty());
    crate::judge::clip(line.unwrap_or_default(), max)
}

/// Lines in the workspace's source files that call `name`, other than the
/// definition itself, as `path:line: text`.
fn callers(files: &[(String, String)], function: &Function) -> Vec<String> {
    let Ok(call) = Regex::new(&format!(r"\b{}\s*\(", regex::escape(&function.name))) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (path, text) in files {
        for (n, line) in text.lines().enumerate() {
            if *path == function.file && n + 1 == function.line {
                continue;
            }
            let t = line.trim_start();
            if t.starts_with("def ") || t.starts_with("async def ") {
                continue;
            }
            if call.is_match(line) {
                out.push(format!(
                    "{path}:{}: {}",
                    n + 1,
                    crate::judge::clip(line.trim(), 200)
                ));
                if out.len() >= MAX_CALLERS {
                    return out;
                }
            }
        }
    }
    out
}

fn source_texts(workspace: &Path) -> Vec<(String, String)> {
    crate::accept::source_files(workspace, 60)
        .into_iter()
        .filter_map(|path| {
            std::fs::read_to_string(workspace.join(&path))
                .ok()
                .map(|text| (path, text))
        })
        .collect()
}

/// Every documented function, with its body and call sites.
#[must_use]
pub fn docstrings(workspace: &Path) -> Vec<Candidate> {
    let files = source_texts(workspace);
    functions(workspace)
        .into_iter()
        .filter(|f| f.docstring.is_some())
        .take(MAX_CANDIDATES)
        .map(|f| {
            let doc = f.docstring.clone().unwrap_or_default();
            let callers = callers(&files, &f);
            Candidate {
                kind: Source::Docstring,
                file: f.file.clone(),
                line: f.line,
                text: format!("`{}`: {}", f.name, first_line(&doc, 160)),
                evidence: json!({
                    "file": f.file,
                    "line": f.line,
                    "name": f.name,
                    "docstring": crate::judge::clip(&doc, 1_200),
                    "body": f.body,
                    "callers": callers,
                }),
            }
        })
        .collect()
}

/// `text` as space-separated lowercase words, camel case split, padded
/// with a space on each side for whole-word matching.
#[must_use]
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push(' ');
    let mut prev: Option<char> = None;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && prev.is_some_and(|p| p.is_ascii_lowercase()) {
                out.push(' ');
            }
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with(' ') {
            out.push(' ');
        }
        prev = Some(c);
    }
    if !out.ends_with(' ') {
        out.push(' ');
    }
    out
}

/// The names a source file imports: Python `import` and `from … import`,
/// and JavaScript or TypeScript `import … from`.
#[must_use]
pub fn imported_names(text: &str) -> BTreeSet<String> {
    static IMPORT: OnceLock<Vec<Regex>> = OnceLock::new();
    let patterns = IMPORT.get_or_init(|| {
        [
            r"^\s*from\s+[\w.]+\s+import\s+\(?([^)#]+)",
            r"^\s*import\s+([\w.,\s]+?)(?:\s+as\s+\w+)?\s*$",
            r"^\s*import\s+(?:type\s+)?\{([^}]+)\}\s+from",
            r"^\s*import\s+(?:\*\s+as\s+)?([A-Za-z_$][\w$]*)\s+from",
        ]
        .iter()
        .map(|p| Regex::new(p).expect("an import pattern"))
        .collect()
    });
    let mut out = BTreeSet::new();
    for line in text.lines() {
        for p in patterns {
            if let Some(caps) = p.captures(line) {
                for part in caps[1].split(',') {
                    let name = part.split_whitespace().next().unwrap_or_default();
                    let name = name.rsplit('.').next().unwrap_or(name);
                    if !name.is_empty() {
                        out.insert(name.to_string());
                    }
                }
            }
        }
    }
    out
}

/// The methods a function names per matched phrase, strongest first: 2
/// when its own name names one, 1 when its docstring, its parameters, or
/// an imported name its body uses does. At most [`METHODS_PER_FUNCTION`].
fn named_methods<'a>(list: &'a MethodList, name: &str, other: &str) -> Vec<(&'a Method, &'a str)> {
    let name = normalize(name);
    let other = normalize(other);
    let mut found: Vec<(&Method, &str, u8, usize)> = Vec::new();
    for (order, method) in list.methods.iter().enumerate() {
        let best = method
            .names
            .iter()
            .filter_map(|phrase| {
                let needle = normalize(phrase);
                if name.contains(&needle) {
                    Some((phrase.as_str(), 2))
                } else if other.contains(&needle) {
                    Some((phrase.as_str(), 1))
                } else {
                    None
                }
            })
            .max_by_key(|(_, strength)| *strength);
        if let Some((phrase, strength)) = best {
            found.push((method, phrase, strength, order));
        }
    }
    found.sort_by_key(|(_, _, strength, order)| (std::cmp::Reverse(*strength), *order));
    found
        .into_iter()
        .take(METHODS_PER_FUNCTION)
        .map(|(method, phrase, _, _)| (method, phrase))
        .collect()
}

/// The methods one standard-method candidate carries, at most.
pub const METHODS_PER_FUNCTION: usize = 3;

/// Functions and classes that name a well-known method, each with its
/// body and the standard definitions of the methods it names.
#[must_use]
pub fn standard_methods(workspace: &Path, list: &MethodList) -> Vec<Candidate> {
    let imports: BTreeMap<String, BTreeSet<String>> = source_texts(workspace)
        .into_iter()
        .map(|(path, text)| (path, imported_names(&text)))
        .collect();
    let mut out = Vec::new();
    for f in functions(workspace) {
        let doc = f.docstring.clone().unwrap_or_default();
        let used: Vec<&str> = imports.get(&f.file).map_or_else(Vec::new, |names| {
            names
                .iter()
                .filter(|n| f.body.contains(n.as_str()))
                .map(String::as_str)
                .collect()
        });
        // The parameters, without the definition's own name.
        let parameters = f.signature.split_once('(').map_or("", |(_, rest)| rest);
        let other = format!("{doc} {parameters} {}", used.join(" "));
        let named = named_methods(list, &f.name, &other);
        if named.is_empty() {
            continue;
        }
        let ids: Vec<String> = named
            .iter()
            .map(|(method, phrase)| format!("{} ({phrase})", method.id))
            .collect();
        out.push(Candidate {
            kind: Source::StandardMethod,
            file: f.file.clone(),
            line: f.line,
            text: format!("`{}` names {}", f.name, ids.join(", ")),
            evidence: json!({
                "file": f.file,
                "line": f.line,
                "name": f.name,
                "methods": named
                    .iter()
                    .map(|(method, _)| json!({
                        "name": method.id,
                        "family": method.family,
                        "definition": method.definition,
                    }))
                    .collect::<Vec<_>>(),
                "docstring": crate::judge::clip(&doc, 1_200),
                "body": f.body,
            }),
        });
        if out.len() >= MAX_CANDIDATES {
            break;
        }
    }
    out
}

/// One Jev request over a slice of candidates: the index of its first
/// candidate, its state, and its questions.
pub struct Request {
    pub offset: usize,
    pub state: Value,
    pub questions: ::jev::Questions,
    /// Each question's ID, by candidate within the request.
    pub ids: Vec<String>,
}

/// The requests that rank `candidates` of `source` against `task`. For
/// `rationale` this is v13's one request, word for word.
#[must_use]
pub fn requests(source: Source, task: &str, candidates: &[Candidate]) -> Vec<Request> {
    let task = crate::judge::clip(task.trim(), 4_000);
    candidates
        .chunks(source.batch().max(1))
        .enumerate()
        .map(|(n, chunk)| {
            let mut questions = ::jev::Questions::new();
            let mut ids = Vec::new();
            for j in 0..chunk.len() {
                let id = format!("{}_{j}", source.question_prefix());
                questions =
                    questions.with(id.clone(), ::jev::Noul::new(question(source, j).as_str()));
                ids.push(id);
            }
            let evidence: Vec<Value> = chunk.iter().map(|c| c.evidence.clone()).collect();
            let mut state = serde_json::Map::new();
            state.insert("task".to_string(), json!(task));
            state.insert(source.state_key().to_string(), json!(evidence));
            Request {
                offset: n * source.batch().max(1),
                state: Value::Object(state),
                questions,
                ids,
            }
        })
        .collect()
}

/// What ranking produced: every candidate as a row, what it cost, and one
/// record per request.
#[derive(Clone, Debug, Default)]
pub struct Ranked {
    pub rows: Vec<Row>,
    pub usd: f64,
    pub calls: Vec<Value>,
}

/// Where a ranking runs: the component that asks, the call ID prefix,
/// and the episode deadline.
pub struct Context<'a> {
    pub component: &'a str,
    pub id: String,
    pub deadline: Option<crate::deadline::Deadline>,
}

/// Mines `sources` in `workspace` and has Jev rank every candidate.
pub async fn rank(
    jev: &JevMode,
    recorder: &Recorder,
    context: &Context<'_>,
    task: &str,
    workspace: &Path,
    sources: &[Source],
) -> Ranked {
    let mut ranked = Ranked::default();
    for &source in sources {
        let candidates = mine(source, workspace);
        for (n, request) in requests(source, task, &candidates).into_iter().enumerate() {
            let asked = jev_component::ask(
                jev,
                recorder,
                jev_component::Ask {
                    component: context.component,
                    name: source.decision(),
                    id: format!("{}-{}-{n}", context.id, source.word()),
                    state: request.state,
                    questions: request.questions,
                    parent: None,
                    deadline: context.deadline.clone(),
                },
            )
            .await;
            ranked.usd += asked.input_tokens.map_or(0.0, |t| {
                t as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0
            });
            ranked.calls.push(json!({
                "source": source.word(),
                "how": asked.how,
                "error": asked.error,
                "input_tokens": asked.input_tokens,
            }));
            for (j, id) in request.ids.iter().enumerate() {
                let c = &candidates[request.offset + j];
                ranked.rows.push(Row {
                    kind: c.kind,
                    file: c.file.clone(),
                    line: c.line,
                    text: c.text.clone(),
                    p: asked.noul(id),
                });
            }
        }
    }
    ranked
}

/// The rows the briefing lists: per source, those at or above the
/// source's threshold, most likely first, at most [`MAX_LISTED`].
#[must_use]
pub fn listed(rows: &[Row]) -> Vec<Row> {
    listed_at(rows, &|source| source.threshold())
}

/// [`listed`] with the thresholds `at` gives, for the offline measurement.
#[must_use]
pub fn listed_at(rows: &[Row], at: &dyn Fn(Source) -> f64) -> Vec<Row> {
    let mut out = Vec::new();
    for source in Source::ALL {
        let mut mine: Vec<&Row> = rows
            .iter()
            .filter(|r| r.kind == source && r.p.is_some_and(|p| p >= at(source)))
            .collect();
        mine.sort_by(|a, b| b.p.unwrap_or(0.0).total_cmp(&a.p.unwrap_or(0.0)));
        out.extend(mine.into_iter().take(MAX_LISTED).cloned());
    }
    out
}

/// The briefing section's name, kept from v13.
pub const LABEL: &str = "Likely defects: comments that justify a choice";

/// What a session is told about the listed rows. It keeps v13's
/// instruction to decide each suspect explicitly.
pub const NOTE: &str = "Each row below is a place in the code that Jev read as a likely cause of \
a problem the task describes. Its kind says why it was found: `rationale` is a comment that gives \
a reason for a choice, `docstring` is a function whose code or callers may not do what its \
docstring says, and `standard-method` is a function that may depart from the standard definition \
of the method it names. A comment or a docstring is a claim, not a specification. Decide each one \
explicitly against the task: fix it when it causes a described problem or departs from the \
standard form, and say in your finish summary what you decided for each.";

/// The briefing entry for `listed`, or `None` when it's empty.
#[must_use]
pub fn evidence(listed: &[Row]) -> Option<microluna::Evidence> {
    (!listed.is_empty()).then(|| microluna::Evidence {
        label: LABEL.to_string(),
        text: format!(
            "{NOTE}\n\n{}",
            listed
                .iter()
                .map(Row::line_text)
                .collect::<Vec<_>>()
                .join("\n")
        ),
    })
}

/// The implementation record: the question sets, the method list, the
/// thresholds, and the bounds, each by digest or value.
#[must_use]
pub fn implementation(sources: &[Source]) -> Implementation {
    let sets: BTreeMap<&str, Value> = sources
        .iter()
        .map(|s| {
            let set = question_set(*s);
            (
                s.word(),
                json!({ "id": set.id, "digest": set.digest, "threshold": s.threshold() }),
            )
        })
        .collect();
    let (list, digest) = methods();
    Implementation::new(
        COMPONENT,
        "comment, docstring, and standard-method miners ranked by Jev",
        &json!({
            "sets": sets,
            "methods": { "version": list.version, "digest": digest },
            "max_candidates": MAX_CANDIDATES,
            "max_listed": MAX_LISTED,
            "batch": BATCH,
            "body_chars": BODY_CHARS,
        }),
    )
}

#[cfg(test)]
mod tests;
