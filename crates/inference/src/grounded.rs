//! Grounded values (#11114): values a reader may act on (prices, model
//! names, links, citations) are never retyped by the model. Each tool
//! result in a turn gets a stable id (`r1`, `r2`, ...) in a [`Ledger`];
//! the model writes a reference, and the server fills the value in from
//! the result itself.
//!
//! The references, in plain Markdown:
//!
//! | Written | Becomes |
//! | --- | --- |
//! | `{r3.price}` | the value at `price` in result `r3`, as text |
//! | `{r1.results[0].title}` | an array element's field |
//! | `{r2.models["google/gemini-3.8-flash"].input.price_usd}` | a key that is not a plain name |
//! | `{cite:r1.results[0]}` | a link chip, `[title](url)`, from that object's own `title` and `url` |
//!
//! [`resolve`] fills every reference outside code. A reference that does
//! not resolve (no such result, no such field, a field that is not one
//! value, a citation without a URL) is dropped from the text and kept as a
//! [`Diagnostic`]. Unlike a renderer that patches values in after the
//! model has finished, the model is meant to see what its references
//! returned: [`Resolved::feedback`] is the note to hand back to it, every
//! value it was given and every reference that was dropped and why.
//!
//! [`untraced`] is the golden check: every number and URL in a grounded
//! answer must trace to a value in one of the turn's results.
//!
//! The parsing here is deterministic over bounded shapes (result ids,
//! field paths, numbers, URLs) after the answer is written; nothing here
//! routes or classifies a message.

use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::hosted::SearchResult;
use crate::rates::{Card, Kind};

/// Where a result came from.
pub mod source {
    /// A hosted web search ([`crate::hosted`]).
    pub const WEB_SEARCH: &str = "web_search";
    /// The public rate card and model catalog ([`crate::rates`]).
    pub const RATE_CARD: &str = "rate_card";
    /// A docs or knowledge-base search.
    pub const DOCS_SEARCH: &str = "docs_search";
    /// Repository, issue, or pull-request data.
    pub const REPO: &str = "repo";
    /// A wallet balance or payment record.
    pub const WALLET: &str = "wallet";
}

/// The most field paths [`Ledger::brief`] lists for one result.
pub const BRIEF_PATHS: usize = 40;

/// One tool result in a turn.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Grounded {
    /// The result's id in the turn: `r1`, `r2`, ...
    pub id: String,
    /// Where it came from: a [`source`] word.
    pub source: String,
    /// The result as the tool returned it.
    pub value: Value,
}

/// The turn's tool results, each with a stable id in the order recorded.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Ledger {
    results: Vec<Grounded>,
}

impl Ledger {
    /// An empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one result and returns its id. Ids are never reused in a
    /// turn: the `n`th result recorded is `rn`.
    pub fn record(&mut self, source: &str, value: Value) -> String {
        let id = format!("r{}", self.results.len() + 1);
        self.results.push(Grounded {
            id: id.clone(),
            source: source.to_owned(),
            value,
        });
        id
    }

    /// Records a web search's results as `{"query", "results": [{title,
    /// url, snippet}]}`.
    pub fn record_search(&mut self, query: &str, results: &[SearchResult]) -> String {
        self.record(
            source::WEB_SEARCH,
            json!({ "query": query, "results": results }),
        )
    }

    /// Records the rate card, so prices and model names in an answer are
    /// references to it: `models` holds each model's list row by its id
    /// (the first list row when several providers serve it), and `rows`
    /// every row, promotions included.
    pub fn record_rates(&mut self, card: &Card) -> String {
        let mut models = Map::new();
        for row in &card.rows {
            if row.kind == Kind::List && !models.contains_key(&row.model) {
                models.insert(
                    row.model.clone(),
                    serde_json::to_value(row).unwrap_or(Value::Null),
                );
            }
        }
        self.record(
            source::RATE_CARD,
            json!({
                "unit": card.unit,
                "charged_in": card.charged_in,
                "models": models,
                "rows": card.rows,
            }),
        )
    }

    /// The result with `id`.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Grounded> {
        self.results.iter().find(|result| result.id == id)
    }

    /// Every result, in the order recorded.
    #[must_use]
    pub fn results(&self) -> &[Grounded] {
        &self.results
    }

    /// Whether the turn has no results.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// The value at `path` (`models["x"].input.price_usd`) in result `id`.
    #[must_use]
    pub fn lookup(&self, id: &str, path: &[Segment]) -> Option<&Value> {
        let mut at = &self.get(id)?.value;
        for segment in path {
            at = match segment {
                Segment::Key(key) => at.get(key.as_str())?,
                Segment::Index(index) => at.get(*index)?,
            };
        }
        Some(at)
    }

    /// The note the model reads before answering: how to reference a
    /// value, then each result's id, source, and field paths (at most
    /// [`BRIEF_PATHS`] each).
    #[must_use]
    pub fn brief(&self) -> String {
        let mut note = String::from(
            "Tool results have ids. Do not copy a price, number, model name, or link from \
             them: write a reference and it is filled in from the result. `{r1.path}` writes \
             the value at that path; `{cite:r1.results[0]}` writes a link to that item's \
             title and url. A reference that does not resolve is left out.\n",
        );
        for result in &self.results {
            let mut paths = Vec::new();
            leaf_paths(&result.value, &mut Vec::new(), &mut paths);
            let shown = paths.len().min(BRIEF_PATHS);
            note.push_str(&format!("- {} ({}): ", result.id, result.source));
            note.push_str(&paths[..shown].join(", "));
            if paths.len() > shown {
                note.push_str(&format!(", and {} more", paths.len() - shown));
            }
            note.push('\n');
        }
        note
    }
}

/// One step of a field path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Segment {
    /// `.name` or `["key"]`.
    Key(String),
    /// `[3]`.
    Index(usize),
}

/// Writes a path as a reference would: `.name` for a plain name,
/// `["key"]` otherwise, `[n]` for an index.
#[must_use]
pub fn path_text(path: &[Segment]) -> String {
    let mut text = String::new();
    for segment in path {
        match segment {
            Segment::Key(key) if is_name(key) => {
                text.push('.');
                text.push_str(key);
            }
            Segment::Key(key) => text.push_str(&format!("[\"{key}\"]")),
            Segment::Index(index) => text.push_str(&format!("[{index}]")),
        }
    }
    text
}

fn is_name(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn leaf_paths(value: &Value, at: &mut Vec<Segment>, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                at.push(Segment::Key(key.clone()));
                leaf_paths(child, at, out);
                at.pop();
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                at.push(Segment::Index(index));
                leaf_paths(child, at, out);
                at.pop();
            }
        }
        _ => {
            let path = path_text(at);
            out.push(path.strip_prefix('.').unwrap_or(&path).to_owned());
        }
    }
}

/// A reference as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    /// A citation (`{cite:...}`) rather than a value.
    pub cite: bool,
    /// The result id.
    pub id: String,
    pub path: Vec<Segment>,
}

impl Reference {
    /// The reference as it would be written, braces included.
    #[must_use]
    pub fn text(&self) -> String {
        format!(
            "{{{}{}{}}}",
            if self.cite { "cite:" } else { "" },
            self.id,
            path_text(&self.path)
        )
    }
}

/// Reads a reference body (between the braces): `cite:` optionally, then
/// `r` and digits, then path segments. `None` when it is not one, so the
/// braces are left as written.
#[must_use]
pub fn parse_reference(body: &str) -> Option<Reference> {
    let (cite, rest) = match body.strip_prefix("cite:") {
        Some(rest) => (true, rest),
        None => (false, body),
    };
    let digits = rest.strip_prefix('r')?;
    let end = digits
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(digits.len());
    if end == 0 {
        return None;
    }
    let id = format!("r{}", &digits[..end]);
    let mut rest = &digits[end..];
    let mut path = Vec::new();
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('.') {
            let end = after
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(after.len());
            if end == 0 {
                return None;
            }
            path.push(Segment::Key(after[..end].to_owned()));
            rest = &after[end..];
        } else if let Some(after) = rest.strip_prefix("[\"") {
            let end = after.find("\"]")?;
            path.push(Segment::Key(after[..end].to_owned()));
            rest = &after[end + 2..];
        } else if let Some(after) = rest.strip_prefix('[') {
            let end = after.find(']')?;
            path.push(Segment::Index(after[..end].parse().ok()?));
            rest = &after[end + 1..];
        } else {
            return None;
        }
    }
    Some(Reference { cite, id, path })
}

/// Why a reference was dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// The turn has no result with that id.
    NoSuchResult,
    /// The result has no value at that path.
    NoSuchField,
    /// The value is an object, an array, or null, not one value to write.
    NotAValue,
    /// A citation's object has no `http(s)` `url`.
    NoUrl,
}

impl Reason {
    /// The reason in words, for the model.
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Reason::NoSuchResult => "there is no result with that id",
            Reason::NoSuchField => "that result has no value at that path",
            Reason::NotAValue => "that path holds a list or an object, not one value",
            Reason::NoUrl => "that item has no url to cite",
        }
    }
}

/// A reference that did not resolve.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    /// The reference as written.
    pub reference: String,
    pub reason: Reason,
}

/// A reference that resolved, and what it wrote.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Resolution {
    /// The reference as written.
    pub reference: String,
    /// The text it became.
    pub value: String,
}

/// An answer with its references filled in.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Resolved {
    /// The answer as the reader sees it.
    pub text: String,
    pub resolutions: Vec<Resolution>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Resolved {
    /// The note handed back to the model: what each reference wrote and
    /// which were dropped and why. `None` when the answer had none.
    #[must_use]
    pub fn feedback(&self) -> Option<String> {
        if self.resolutions.is_empty() && self.diagnostics.is_empty() {
            return None;
        }
        let mut note = String::from("Your references resolved as follows.\n");
        for resolution in &self.resolutions {
            note.push_str(&format!(
                "- {} wrote: {}\n",
                resolution.reference, resolution.value
            ));
        }
        for diagnostic in &self.diagnostics {
            note.push_str(&format!(
                "- {} was left out: {}\n",
                diagnostic.reference,
                diagnostic.reason.words()
            ));
        }
        Some(note)
    }
}

fn resolve_one(reference: &Reference, ledger: &Ledger) -> Result<String, Reason> {
    if ledger.get(&reference.id).is_none() {
        return Err(Reason::NoSuchResult);
    }
    let value = ledger
        .lookup(&reference.id, &reference.path)
        .ok_or(Reason::NoSuchField)?;
    if reference.cite {
        let url = value
            .get("url")
            .and_then(Value::as_str)
            .filter(|url| url.starts_with("https://") || url.starts_with("http://"))
            .ok_or(Reason::NoUrl)?;
        let title = value
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .unwrap_or(url);
        return Ok(format!(
            "[{}]({})",
            title.replace('[', "\\[").replace(']', "\\]"),
            url.replace(' ', "%20").replace(')', "%29")
        ));
    }
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Number(number) => Ok(number.to_string()),
        Value::Bool(flag) => Ok(flag.to_string()),
        _ => Err(Reason::NotAValue),
    }
}

/// Fills every reference in `text` from `ledger`, outside fenced code and
/// code spans. Braces that are not a reference's shape are left as
/// written.
#[must_use]
pub fn resolve(text: &str, ledger: &Ledger) -> Resolved {
    let mut resolved = Resolved::default();
    for_each_prose(text, &mut resolved.text, |prose, out| {
        let mut rest = prose;
        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let after = &rest[open + 1..];
            let reference = after
                .find('}')
                .and_then(|close| Some((close, parse_reference(&after[..close])?)));
            match reference {
                Some((close, reference)) => {
                    match resolve_one(&reference, ledger) {
                        Ok(value) => {
                            out.push_str(&value);
                            resolved.resolutions.push(Resolution {
                                reference: reference.text(),
                                value,
                            });
                        }
                        Err(reason) => resolved.diagnostics.push(Diagnostic {
                            reference: reference.text(),
                            reason,
                        }),
                    }
                    rest = &after[close + 1..];
                }
                None => {
                    out.push('{');
                    rest = after;
                }
            }
        }
        out.push_str(rest);
    });
    resolved
}

/// Copies `text` into `out`, handing each run of prose (outside fenced
/// code blocks and code spans) to `prose` and copying code as written.
fn for_each_prose(text: &str, out: &mut String, mut prose: impl FnMut(&str, &mut String)) {
    let mut fence: Option<String> = None;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let marker: String = trimmed
            .chars()
            .take_while(|c| *c == '`' || *c == '~')
            .collect();
        let is_fence =
            marker.len() >= 3 && marker.chars().all(|c| c == marker.as_bytes()[0] as char);
        match &fence {
            Some(open) => {
                out.push_str(line);
                if is_fence
                    && marker.starts_with(open.as_str())
                    && trimmed[marker.len()..].trim().is_empty()
                {
                    fence = None;
                }
                continue;
            }
            None if is_fence => {
                fence = Some(marker);
                out.push_str(line);
                continue;
            }
            None => {}
        }
        // Code spans: a run of backticks up to the same run.
        let mut rest = line;
        while let Some(tick) = rest.find('`') {
            prose(&rest[..tick], out);
            let run = rest[tick..].chars().take_while(|c| *c == '`').count();
            let ticks = &rest[tick..tick + run];
            let after = &rest[tick + run..];
            match after.find(ticks) {
                Some(close) => {
                    out.push_str(&rest[tick..tick + run + close + run]);
                    rest = &after[close + run..];
                }
                None => {
                    out.push_str(&rest[tick..]);
                    rest = "";
                }
            }
        }
        prose(rest, out);
    }
}

/// The kind of value a golden check found untraced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Found {
    /// A number in prose.
    Number,
    Url,
}

/// A number or URL in an answer that no result holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Untraced {
    pub kind: Found,
    pub text: String,
}

/// The URLs in `text`: from `http://` or `https://` to whitespace or a
/// closing bracket, quote, or angle, without trailing sentence
/// punctuation.
#[must_use]
pub fn urls(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = ["https://", "http://"]
        .iter()
        .filter_map(|scheme| rest.find(*scheme))
        .min()
    {
        let tail = &rest[start..];
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, ')' | ']' | '>' | '"' | '\'' | '<'))
            .unwrap_or(tail.len());
        let url = tail[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        found.push(url.to_owned());
        rest = &tail[end..];
    }
    found
}

/// The numbers in `text` with URLs removed: digit runs with `,`
/// thousands separators and one `.` decimal point, separators dropped.
#[must_use]
pub fn numbers(text: &str) -> Vec<String> {
    let mut without = text.to_owned();
    for url in urls(text) {
        without = without.replace(&url, " ");
    }
    let chars: Vec<char> = without.chars().collect();
    let mut found = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        if !chars[at].is_ascii_digit() {
            at += 1;
            continue;
        }
        let mut number = String::new();
        while at < chars.len() {
            let c = chars[at];
            let separator_then_digit =
                matches!(c, ',' | '.') && chars.get(at + 1).is_some_and(char::is_ascii_digit);
            if c.is_ascii_digit() {
                number.push(c);
            } else if c == '.' && separator_then_digit && !number.contains('.') {
                number.push('.');
            } else if c == ',' && separator_then_digit && !number.contains('.') {
                // A thousands separator: dropped.
            } else {
                break;
            }
            at += 1;
        }
        found.push(number);
    }
    found
}

fn same_number(left: &str, right: &str) -> bool {
    match (left.parse::<f64>(), right.parse::<f64>()) {
        (Ok(left), Ok(right)) => (left - right).abs() <= f64::EPSILON * left.abs().max(1.0),
        _ => left == right,
    }
}

fn leaves(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => map.values().for_each(|child| leaves(child, out)),
        Value::Array(items) => items.iter().for_each(|child| leaves(child, out)),
        Value::String(text) => out.push(text.clone()),
        Value::Number(number) => out.push(number.to_string()),
        Value::Bool(_) | Value::Null => {}
    }
}

/// Drops ordered-list markers (`1. `, `2) `) at the start of lines, which
/// are structure rather than values.
fn without_list_markers(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| {
            let indent = line.len() - line.trim_start().len();
            let body = &line[indent..];
            let digits = body.chars().take_while(char::is_ascii_digit).count();
            let marker = digits > 0
                && matches!(body[digits..].chars().next(), Some('.' | ')'))
                && body[digits + 1..].starts_with(' ');
            if marker {
                format!("{}{}", &line[..indent], &body[digits + 1..])
            } else {
                line.to_owned()
            }
        })
        .collect()
}

/// The golden check: every number and URL in a grounded answer's prose
/// (code left out) that no result in `ledger` holds. Empty means the
/// answer traces.
#[must_use]
pub fn untraced(answer: &str, ledger: &Ledger) -> Vec<Untraced> {
    let mut prose = String::new();
    for_each_prose(
        &without_list_markers(answer),
        &mut String::new(),
        |run, _| {
            prose.push_str(run);
            prose.push(' ');
        },
    );
    let mut held = Vec::new();
    for result in ledger.results() {
        leaves(&result.value, &mut held);
    }
    let held_urls: Vec<String> = held.iter().flat_map(|leaf| urls(leaf)).collect();
    let held_numbers: Vec<String> = held.iter().flat_map(|leaf| numbers(leaf)).collect();
    let bare = |url: &str| url.trim_end_matches('/').to_owned();
    let mut missing = Vec::new();
    for url in urls(&prose) {
        if !held_urls.iter().any(|held| bare(held) == bare(&url)) {
            missing.push(Untraced {
                kind: Found::Url,
                text: url,
            });
        }
    }
    for number in numbers(&prose) {
        if !held_numbers.iter().any(|held| same_number(held, &number)) {
            missing.push(Untraced {
                kind: Found::Number,
                text: number,
            });
        }
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search() -> Vec<SearchResult> {
        vec![
            SearchResult {
                title: "Rust 1.92 released".into(),
                url: "https://blog.rust-lang.org/2026/09/18/Rust-1.92.0/".into(),
                snippet: "Rust 1.92 ships on September 18".into(),
            },
            SearchResult {
                title: "The [Cargo] book".into(),
                url: "https://doc.rust-lang.org/cargo/".into(),
                snippet: String::new(),
            },
        ]
    }

    fn ledger() -> Ledger {
        let mut ledger = Ledger::new();
        assert_eq!(ledger.record_search("rust release", &search()), "r1");
        assert_eq!(
            ledger.record(
                source::RATE_CARD,
                json!({ "models": { "google/gemini-3.8-flash": {
                    "model": "google/gemini-3.8-flash",
                    "input": { "price_usd": "0.315" },
                    "output": { "price_usd": "2.625" },
                } } }),
            ),
            "r2"
        );
        assert_eq!(
            ledger.record(
                source::WALLET,
                json!({ "balance_sats": 12_500, "ok": true })
            ),
            "r3"
        );
        ledger
    }

    #[test]
    fn ids_are_stable_and_in_order() {
        let ledger = ledger();
        let ids: Vec<&str> = ledger.results().iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["r1", "r2", "r3"]);
        assert_eq!(ledger.get("r3").unwrap().source, source::WALLET);
        assert!(ledger.get("r4").is_none());
    }

    #[test]
    fn references_parse_names_indexes_and_keys() {
        let reference =
            parse_reference("r2.models[\"google/gemini-3.8-flash\"].input.price_usd").unwrap();
        assert!(!reference.cite);
        assert_eq!(reference.id, "r2");
        assert_eq!(
            reference.path,
            [
                Segment::Key("models".into()),
                Segment::Key("google/gemini-3.8-flash".into()),
                Segment::Key("input".into()),
                Segment::Key("price_usd".into()),
            ]
        );
        assert_eq!(
            reference.text(),
            "{r2.models[\"google/gemini-3.8-flash\"].input.price_usd}"
        );
        let cite = parse_reference("cite:r1.results[1]").unwrap();
        assert!(cite.cite);
        assert_eq!(cite.path[1], Segment::Index(1));
        assert_eq!(parse_reference("r12").unwrap().id, "r12");
        // Not a reference's shape: left as written.
        for body in [
            "",
            "r",
            "x1",
            "r1.",
            "r1[x]",
            "r1 price",
            "\"a\": 1",
            "result3.price",
        ] {
            assert!(parse_reference(body).is_none(), "{body}");
        }
    }

    #[test]
    fn an_answer_fills_values_and_citations_from_the_results() {
        let ledger = ledger();
        let answer = "Rust {r1.results[0].title} is out ({cite:r1.results[0]}). \
                      Flash costs ${r2.models[\"google/gemini-3.8-flash\"].input.price_usd} \
                      per million input tokens. You hold {r3.balance_sats} sats. \
                      See {cite:r1.results[1]}.";
        let resolved = resolve(answer, &ledger);
        assert_eq!(
            resolved.text,
            "Rust Rust 1.92 released is out ([Rust 1.92 released]\
             (https://blog.rust-lang.org/2026/09/18/Rust-1.92.0/)). \
             Flash costs $0.315 per million input tokens. You hold 12500 sats. \
             See [The \\[Cargo\\] book](https://doc.rust-lang.org/cargo/)."
        );
        assert!(resolved.diagnostics.is_empty());
        assert_eq!(resolved.resolutions.len(), 5);
        assert_eq!(resolved.resolutions[3].value, "12500");
    }

    #[test]
    fn a_reference_that_does_not_resolve_is_dropped_and_reported() {
        let ledger = ledger();
        let resolved = resolve(
            "a{r9.price}b{r1.nope}c{r1.results}d{cite:r3}e{r3.ok}f",
            &ledger,
        );
        assert_eq!(resolved.text, "abcdetruef");
        let reasons: Vec<Reason> = resolved.diagnostics.iter().map(|d| d.reason).collect();
        assert_eq!(
            reasons,
            [
                Reason::NoSuchResult,
                Reason::NoSuchField,
                Reason::NotAValue,
                Reason::NoUrl
            ]
        );
        // The model sees what each reference wrote and what was dropped.
        let feedback = resolved.feedback().unwrap();
        assert!(feedback.contains("{r3.ok} wrote: true"), "{feedback}");
        assert!(
            feedback.contains("{r9.price} was left out: there is no result with that id"),
            "{feedback}"
        );
        assert_eq!(resolve("plain", &ledger).feedback(), None);
    }

    #[test]
    fn code_and_other_braces_are_left_as_written() {
        let ledger = ledger();
        let answer = "Use `{r3.balance_sats}` in a template, not {r3.balance_sats}.\n\
                      ```json\n{\"r1\": {r1.query}}\n```\n\
                      A set {a, b} and {r1.query}.";
        let resolved = resolve(answer, &ledger);
        assert_eq!(
            resolved.text,
            "Use `{r3.balance_sats}` in a template, not 12500.\n\
             ```json\n{\"r1\": {r1.query}}\n```\n\
             A set {a, b} and rust release."
        );
        assert_eq!(resolved.resolutions.len(), 2);
    }

    #[test]
    fn the_brief_lists_each_result_and_its_paths() {
        let brief = ledger().brief();
        assert!(
            brief.contains("- r1 (web_search): query, results[0].title"),
            "{brief}"
        );
        assert!(
            brief.contains("models[\"google/gemini-3.8-flash\"].input.price_usd"),
            "{brief}"
        );
        assert!(brief.contains("- r3 (wallet): balance_sats, ok"), "{brief}");
    }

    #[test]
    fn rate_card_rows_are_referenced_by_model() {
        let card = crate::rates::Card::published(None);
        let mut ledger = Ledger::new();
        let id = ledger.record_rates(&card);
        let row = card
            .rows
            .iter()
            .find(|row| row.kind == Kind::List)
            .expect("the published card has a list row");
        let reference = format!("{{{id}.models[\"{}\"].input.price_usd}}", row.model);
        let resolved = resolve(&reference, &ledger);
        assert!(resolved.diagnostics.is_empty(), "{resolved:?}");
        assert_eq!(resolved.text, row.input.price_usd);
        let name = resolve(
            &format!("{{{id}.models[\"{}\"].model}}", row.model),
            &ledger,
        );
        assert_eq!(name.text, row.model);
    }

    /// The golden check: a grounded answer's numbers and URLs all trace to
    /// a result once its references are filled; a retyped price or an
    /// invented link does not.
    #[test]
    fn every_number_and_url_in_a_grounded_answer_traces() {
        let ledger = ledger();
        let grounded = resolve(
            "1. Flash: ${r2.models[\"google/gemini-3.8-flash\"].output.price_usd} per \
             million output tokens.\n2. Balance: {r3.balance_sats} sats.\n\
             Source: {cite:r1.results[0]}\n",
            &ledger,
        );
        assert!(grounded.diagnostics.is_empty());
        assert!(untraced(&grounded.text, &ledger).is_empty(), "{grounded:?}");
        // Formatting does not matter: 12,500 is 12500.
        assert!(untraced("You hold 12,500 sats.", &ledger).is_empty());
        let retyped = "Flash costs $2.60 per million; see https://example.com/pricing.";
        assert_eq!(
            untraced(retyped, &ledger),
            [
                Untraced {
                    kind: Found::Url,
                    text: "https://example.com/pricing".into()
                },
                Untraced {
                    kind: Found::Number,
                    text: "2.60".into()
                },
            ]
        );
        // Code is not checked.
        assert!(untraced("Run `sleep 30` first.", &ledger).is_empty());
    }

    #[test]
    fn numbers_and_urls_read_bounded_shapes() {
        assert_eq!(
            numbers("$1,234.50 and 7, then 3.8 and v2"),
            ["1234.50", "7", "3.8", "2"]
        );
        assert_eq!(
            urls("see (https://a.example/x?y=1). and <http://b.example/>, ok"),
            ["https://a.example/x?y=1", "http://b.example/"]
        );
        assert!(numbers("https://a.example/2026/09").is_empty());
    }
}
