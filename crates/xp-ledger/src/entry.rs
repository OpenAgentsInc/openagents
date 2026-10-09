//! A knowledge entry's document: one Markdown file with YAML front matter.
//! The ledger reads an entry's `provenance.written_from` runs to refuse
//! in-sample evidence, so the parser lives here, and `knowledge`
//! re-exports it.

use std::fmt;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::front::{self, Value};

/// What an entry holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A standard definition, its variants, and how to tell them apart.
    Method,
    /// An input or state that breaks common code.
    EdgeCase,
    /// A mistake agents make, how to notice it, and how to avoid it.
    Slip,
    /// How a class of environment behaves.
    Environment,
    /// How to use a command or library correctly.
    Tool,
    /// A fact about the OpenAgents product, written from its public
    /// documents, for the chat's product knowledge base
    /// (`knowledge/openagents/`).
    Product,
}

impl Kind {
    /// The kind a name like `edge-case` names.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "method" => Kind::Method,
            "edge-case" => Kind::EdgeCase,
            "slip" => Kind::Slip,
            "environment" => Kind::Environment,
            "tool" => Kind::Tool,
            "product" => Kind::Product,
            _ => return None,
        })
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Method => "method",
            Kind::EdgeCase => "edge-case",
            Kind::Slip => "slip",
            Kind::Environment => "environment",
            Kind::Tool => "tool",
            Kind::Product => "product",
        })
    }
}

/// How far an entry is trusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Not yet reviewed or measured; shown only when candidates are asked for.
    Candidate,
    /// Shown by default.
    Admitted,
    /// Wrong or harmful; never shown, kept so earlier runs can be explained.
    Withdrawn,
}

impl Status {
    /// The status a name like `admitted` names.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "candidate" => Status::Candidate,
            "admitted" => Status::Admitted,
            "withdrawn" => Status::Withdrawn,
            _ => return None,
        })
    }

    /// Whether an entry with this status is shown, given whether candidates
    /// are.
    #[must_use]
    pub fn shown(self, candidates: bool) -> bool {
        match self {
            Status::Admitted => true,
            Status::Candidate => candidates,
            Status::Withdrawn => false,
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Status::Candidate => "candidate",
            Status::Admitted => "admitted",
            Status::Withdrawn => "withdrawn",
        })
    }
}

/// One entry.
#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    pub id: String,
    pub version: u32,
    pub kind: Kind,
    pub title: String,
    /// One or two sentences: what the entry is about and when it applies.
    pub summary: String,
    pub tags: Vec<String>,
    pub applies_when: String,
    pub status: Status,
    pub author: String,
    /// Run IDs the entry was written from, or `reference`.
    pub written_from: Vec<String>,
    /// Sources for its definitions.
    pub cites: Vec<String>,
    /// Admission records, one line each: reviews, measurements, demotions,
    /// and withdrawals.
    pub evidence: Vec<String>,
    /// A short, complete answer in the OpenAgents voice, which a `product`
    /// entry may carry so the chat can show it as it is. `None` for every
    /// other kind of entry, and for a product entry without one.
    pub answer: Option<String>,
    /// Components shown under the answer, as OpenUI Lang statements
    /// (`openui-lang`, #11187): buttons that start a flow, copyable
    /// commands, numbered steps. Written as a literal (`|`) block; only a
    /// product entry with an answer carries them.
    pub ui: Option<String>,
    /// The Markdown after the front matter.
    pub body: String,
    /// `sha256:` and the hex SHA-256 of the whole file.
    pub digest: String,
}

/// The front-matter keys an entry may have.
const KEYS: &[&str] = &[
    "id",
    "version",
    "kind",
    "title",
    "summary",
    "tags",
    "applies_when",
    "status",
    "author",
    "provenance",
    "evidence",
    "answer",
    "ui",
];

/// `sha256:` and the hex SHA-256 of `bytes`.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

/// Whether `id` is lowercase letters, digits, dots, and hyphens, starting
/// with a letter.
pub fn valid_id(id: &str) -> bool {
    id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

impl Entry {
    /// Parses and validates one entry file.
    ///
    /// # Errors
    ///
    /// Malformed front matter, a missing or empty required field, an
    /// unknown key, kind, or status, or an empty body.
    pub fn parse(text: &str) -> Result<Self, String> {
        let (front, body) = front::split(text)?;
        let fields = front::parse(front)?;
        if let Some((key, _)) = fields.iter().find(|(k, _)| !KEYS.contains(&k.as_str())) {
            return Err(format!("unknown key `{key}`"));
        }
        let get = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, v)| v);
        let text_of = |key: &str| -> Result<String, String> {
            let value = get(key)
                .and_then(Value::text)
                .map(str::trim)
                .unwrap_or_default();
            if value.is_empty() {
                return Err(format!("`{key}` is missing or empty"));
            }
            Ok(value.to_string())
        };
        let list_of = |value: Option<&Value>, key: &str| -> Result<Vec<String>, String> {
            match value {
                None => Ok(Vec::new()),
                Some(value) => value.list().ok_or(format!("`{key}` must be a list")),
            }
        };
        let id = text_of("id")?;
        if !valid_id(&id) {
            return Err(format!(
                "the id `{id}` must be lowercase letters, digits, dots, and hyphens"
            ));
        }
        let version = text_of("version")?
            .parse::<u32>()
            .map_err(|_| "`version` must be a whole number".to_string())?;
        let kind = text_of("kind")?;
        let kind = Kind::parse(&kind).ok_or(format!(
            "unknown kind `{kind}`: use method, edge-case, slip, environment, tool, or product"
        ))?;
        let status = text_of("status")?;
        let status = Status::parse(&status).ok_or(format!(
            "unknown status `{status}`: use candidate, admitted, or withdrawn"
        ))?;
        let (written_from, cites) = match get("provenance") {
            Some(Value::Map(map)) => {
                let find = |key: &str| map.iter().find(|(k, _)| k == key).map(|(_, v)| v);
                (
                    list_of(find("written_from"), "provenance.written_from")?,
                    list_of(find("cites"), "provenance.cites")?,
                )
            }
            None => (Vec::new(), Vec::new()),
            Some(_) => return Err("`provenance` must hold written_from and cites".to_string()),
        };
        let body = body.trim().to_string();
        if body.is_empty() {
            return Err("the entry has no body after the front matter".to_string());
        }
        Ok(Entry {
            id,
            version,
            kind,
            title: text_of("title")?,
            summary: text_of("summary")?,
            tags: list_of(get("tags"), "tags")?,
            applies_when: text_of("applies_when")?,
            status,
            author: text_of("author")?,
            written_from,
            cites,
            evidence: list_of(get("evidence"), "evidence")?,
            answer: get("answer")
                .and_then(Value::text)
                .map(str::trim)
                .filter(|a| !a.is_empty())
                .map(str::to_string),
            ui: get("ui")
                .and_then(Value::text)
                .map(str::trim)
                .filter(|ui| !ui.is_empty())
                .map(str::to_string),
            body,
            digest: digest(text.as_bytes()),
        })
    }

    /// The text retrieval matches on: the title, summary, tags, and
    /// `applies_when`.
    #[must_use]
    pub fn search_text(&self) -> String {
        format!(
            "{}\n{}\n{}\n{}",
            self.title,
            self.summary,
            self.tags.join(", "),
            self.applies_when
        )
    }

    /// The first 12 hex digits of the digest, for display.
    #[must_use]
    pub fn short_digest(&self) -> &str {
        let hex = self.digest.trim_start_matches("sha256:");
        &hex[..hex.len().min(12)]
    }
}

/// The task a run ID names: the ID without its trailing `-<digits>`. A
/// bare task name is returned as it is.
#[must_use]
pub fn task_of(run: &str) -> String {
    match run.rsplit_once('-') {
        Some((task, stamp)) if !stamp.is_empty() && stamp.chars().all(|c| c.is_ascii_digit()) => {
            task.to_string()
        }
        _ => run.to_string(),
    }
}

/// Columns a folded line is wrapped at.
const WIDTH: usize = 76;

impl Entry {
    /// The entry as a file: front matter, then the body.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::from("---\n");
        out.push_str(&format!("id: {}\n", self.id));
        out.push_str(&format!("version: {}\n", self.version));
        out.push_str(&format!("kind: {}\n", self.kind));
        out.push_str(&format!("title: {}\n", inline(&self.title)));
        out.push_str(&format!("summary: >-\n{}\n", folded(&self.summary)));
        out.push_str(&format!("tags: {}\n", flow(&self.tags)));
        out.push_str(&format!(
            "applies_when: >-\n{}\n",
            folded(&self.applies_when)
        ));
        if let Some(answer) = &self.answer {
            out.push_str(&format!("answer: >-\n{}\n", folded(answer)));
        }
        if let Some(ui) = &self.ui {
            out.push_str("ui: |\n");
            for line in ui.lines() {
                if line.trim().is_empty() {
                    out.push('\n');
                } else {
                    out.push_str(&format!("  {line}\n"));
                }
            }
        }
        out.push_str(&format!("status: {}\n", self.status));
        out.push_str(&format!("author: {}\n", inline(&self.author)));
        out.push_str("provenance:\n");
        out.push_str(&format!(
            "  written_from:{}\n",
            block(&self.written_from, 4)
        ));
        out.push_str(&format!("  cites:{}\n", block(&self.cites, 4)));
        out.push_str(&format!("evidence:{}\n", block(&self.evidence, 2)));
        out.push_str("---\n\n");
        out.push_str(self.body.trim());
        out.push('\n');
        out
    }
}

/// A scalar on one line, quoted when YAML would read it differently.
fn inline(text: &str) -> String {
    let text = text.replace('\n', " ");
    let plain = !text.is_empty()
        && text.trim() == text
        && !text.contains(": ")
        && !text.contains(" #")
        && !text.ends_with(':')
        && !text.starts_with(|c: char| "[]{}>|'\"#-&*!%@`,?".contains(c));
    if plain {
        text
    } else if !text.contains('"') {
        format!("\"{text}\"")
    } else if !text.contains('\'') {
        format!("'{text}'")
    } else {
        format!("\"{}\"", text.replace('"', "'"))
    }
}

/// A `[a, b]` list of simple words, or a block list when an item needs
/// quoting.
fn flow(items: &[String]) -> String {
    let simple = items.iter().all(|i| {
        !i.is_empty()
            && i.chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    });
    if simple {
        format!("[{}]", items.join(", "))
    } else {
        block(items, 2)
    }
}

/// A block list indented by `indent`, or ` []` when empty. The result
/// starts with the separator after the key's colon.
pub fn block(items: &[String], indent: usize) -> String {
    if items.is_empty() {
        return " []".to_string();
    }
    let pad = " ".repeat(indent);
    items
        .iter()
        .map(|i| format!("\n{pad}- {}", inline(i)))
        .collect()
}

/// A `>-` block body: each paragraph wrapped at [`WIDTH`], paragraphs
/// separated by a blank line, which folds back to one newline.
fn folded(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for (n, paragraph) in text.split('\n').enumerate() {
        if n > 0 {
            lines.push(String::new());
        }
        let mut line = String::new();
        for word in paragraph.split(' ') {
            if !line.is_empty() && line.len() + 1 + word.len() > WIDTH {
                lines.push(format!("  {line}"));
                line.clear();
            } else if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        lines.push(format!("  {line}"));
    }
    lines.join("\n")
}
