//! The Gym page's knowledge view: knowledge entries and studio plans,
//! opened at an exact version and cited with the next question (#10662).
//!
//! A search reads the local and trusted cached entries through
//! `openagents --json kb search TEXT --lexical`, which runs no model and
//! spends nothing. Opening an entry reads it through `kb show ID`; studio
//! plans come from `openagents --json studio goal list`. Nothing is copied
//! into a store here.
//!
//! Citing is the admission step: only an `admitted` entry whose shown
//! version and digest are exactly the ones the search found can be cited,
//! so a candidate, a withdrawn entry, or one that changed under the page
//! never becomes context. A studio plan is cited as studio memory, by the
//! digest of the plan as read, never as published knowledge. What is cited
//! is the request context's `cited` list, so the preview the page shows is
//! the text `Context::preview` sends.

use crate::context::Cited;
use serde::Deserialize;
use serde_json::Value;
use std::sync::mpsc::Receiver;

/// The most bytes of helper output the view reads.
pub const READ_MAX: usize = 2 * 1024 * 1024;
/// The most characters of an entry or plan a citation sends.
pub const CITED_CHARS: usize = 4_000;
/// The most citations one question carries.
pub const MAX_CITED: usize = 4;

/// An entry as `kb search` and `kb show` report it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub id: String,
    pub version: u32,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub author: String,
    pub written_from: Vec<String>,
    pub cites: Vec<String>,
    pub evidence: Vec<String>,
    pub body: String,
    pub digest: String,
}

/// One studio plan entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Step {
    pub id: String,
    pub seat: String,
    pub progress: Value,
    pub title: String,
}

/// A studio goal and its plan, and the digest of the goal as read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Goal {
    pub goal_id: String,
    pub status: String,
    pub text: String,
    pub entries: Vec<Step>,
    pub digest: String,
}

/// What the view lists: studio plans, then knowledge hits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Plan(Goal),
    Knowledge(Entry),
}

pub type HitsRead = Result<Vec<Entry>, String>;
pub type ShownRead = Result<(Entry, Option<u32>), String>;
pub type GoalsRead = Result<Vec<Goal>, String>;

fn last_json(bytes: &[u8]) -> Option<Value> {
    let text = String::from_utf8_lossy(bytes);
    let line = text.lines().rev().find(|line| !line.trim().is_empty())?;
    serde_json::from_str(line).ok()
}

fn error_of(stdout: &[u8], stderr: &[u8]) -> String {
    last_json(stdout)
        .or_else(|| last_json(stderr))
        .and_then(|value| value["error"].as_str().map(crate::ascii::ascii))
        .unwrap_or_else(|| "the answer was not readable".into())
}

/// Decodes `openagents --json kb search TEXT --lexical`.
#[must_use]
pub fn decode_hits(stdout: &[u8], stderr: &[u8]) -> HitsRead {
    if stdout.len() > READ_MAX {
        return Err("the search answered too much to show".into());
    }
    match last_json(stdout) {
        Some(value) if value.get("hits").is_some() => Ok(value["hits"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|hit| serde_json::from_value(hit["entry"].clone()).ok())
            .collect()),
        _ => Err(error_of(stdout, stderr)),
    }
}

/// Decodes `openagents --json kb show ID`: the entry for `id` and the
/// version of a newer candidate waiting beside it.
#[must_use]
pub fn decode_shown(stdout: &[u8], stderr: &[u8], id: &str) -> ShownRead {
    if stdout.len() > READ_MAX {
        return Err("the entry is too large to show".into());
    }
    match last_json(stdout) {
        Some(value) if value.get("entry").is_some() => {
            let entry: Entry = serde_json::from_value(value["entry"].clone())
                .map_err(|_| "the entry was not readable")?;
            if entry.id != id {
                return Err("the answer named another entry".into());
            }
            let pending = value["pending"]["version"]
                .as_u64()
                .and_then(|version| u32::try_from(version).ok());
            Ok((entry, pending))
        }
        _ => Err(error_of(stdout, stderr)),
    }
}

/// Decodes `openagents --json studio goal list`; each goal's digest is
/// over its exact JSON as answered.
#[must_use]
pub fn decode_goals(stdout: &[u8], stderr: &[u8]) -> GoalsRead {
    if stdout.len() > READ_MAX {
        return Err("the studio answered too much to show".into());
    }
    match last_json(stdout) {
        Some(value) if value.get("goals").is_some() => Ok(value["goals"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|goal| Goal {
                goal_id: goal["goal_id"].as_str().unwrap_or_default().to_owned(),
                status: word(&goal["status"]),
                text: goal["text"].as_str().unwrap_or_default().to_owned(),
                entries: serde_json::from_value(goal["entries"].clone()).unwrap_or_default(),
                digest: format!("sha256:{}", crate::proposals::digest(goal)),
            })
            .collect()),
        _ => Err(error_of(stdout, stderr)),
    }
}

fn word(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Object(map) => map.keys().next().cloned().unwrap_or_default(),
        _ => String::new(),
    }
}

fn bounded(text: &str) -> (String, bool) {
    let text = crate::ascii::ascii(text);
    match text.char_indices().nth(CITED_CHARS) {
        Some((at, _)) => (text[..at].to_owned(), true),
        None => (text, false),
    }
}

/// The admission step for a knowledge entry: `found` is what the search
/// listed and `shown` what was opened. Only the same admitted version and
/// bytes become a citation.
///
/// # Errors
///
/// Why the entry is not cited.
pub fn cite_entry(found: &Entry, shown: &Entry) -> Result<Cited, String> {
    if shown.id != found.id || shown.version != found.version || shown.digest != found.digest {
        return Err(format!(
            "{} changed since the search (now version {}); search again",
            found.id, shown.version
        ));
    }
    match shown.status.as_str() {
        "admitted" => {}
        "candidate" => {
            return Err(format!(
                "{} version {} is a candidate, not admitted context",
                shown.id, shown.version
            ));
        }
        "withdrawn" => return Err(format!("{} was withdrawn", shown.id)),
        other => return Err(format!("{} is {other}, not admitted", shown.id)),
    }
    let (text, truncated) = bounded(&shown.body);
    Ok(Cited {
        kind: "knowledge".into(),
        id: shown.id.clone(),
        version: shown.version.to_string(),
        digest: shown.digest.clone(),
        title: crate::ascii::ascii(&shown.title),
        status: shown.status.clone(),
        author: shown.author.clone(),
        text,
        truncated,
    })
}

/// A studio plan as a citation: studio memory at the digest it was read.
#[must_use]
pub fn cite_plan(goal: &Goal) -> Cited {
    let mut text = format!("Goal: {}\n", goal.text.trim());
    for step in &goal.entries {
        text.push_str(&format!(
            "- {} [{}] {} ({})\n",
            step.id,
            step.seat,
            step.title,
            word(&step.progress)
        ));
    }
    let (text, truncated) = bounded(&text);
    Cited {
        kind: "plan".into(),
        id: goal.goal_id.clone(),
        version: String::new(),
        digest: goal.digest.clone(),
        title: crate::ascii::ascii(goal.text.lines().next().unwrap_or_default()),
        status: goal.status.clone(),
        author: "studio".into(),
        text,
        truncated,
    }
}

/// The view's state.
#[derive(Default)]
pub struct Page {
    pub open: bool,
    pub query: Option<String>,
    pub hits: Option<HitsRead>,
    pub searching: Option<Receiver<HitsRead>>,
    pub goals: Option<GoalsRead>,
    pub goals_reading: Option<Receiver<GoalsRead>>,
    pub picked: usize,
    /// The item opened, and for an entry what `kb show` answered.
    pub opened: Option<Item>,
    pub shown: Option<ShownRead>,
    pub showing: Option<Receiver<ShownRead>>,
    /// What goes with the next question, in order.
    pub cited: Vec<Cited>,
    pub refusal: Option<String>,
}

impl Page {
    /// Everything listed: studio plans, then knowledge hits.
    #[must_use]
    pub fn items(&self) -> Vec<Item> {
        let mut out: Vec<Item> = match &self.goals {
            Some(Ok(goals)) => goals.iter().cloned().map(Item::Plan).collect(),
            _ => Vec::new(),
        };
        if let Some(Ok(hits)) = &self.hits {
            out.extend(hits.iter().cloned().map(Item::Knowledge));
        }
        out
    }

    /// Whether `id` at `digest` is cited.
    #[must_use]
    pub fn is_cited(&self, id: &str, digest: &str) -> bool {
        self.cited
            .iter()
            .any(|cited| cited.id == id && cited.digest == digest)
    }

    /// Cites or uncites the opened item. Returns the words to show.
    pub fn toggle(&mut self) -> String {
        let cited = match &self.opened {
            Some(Item::Plan(goal)) => Ok(cite_plan(goal)),
            Some(Item::Knowledge(found)) => match &self.shown {
                Some(Ok((shown, _))) => cite_entry(found, shown),
                Some(Err(why)) => Err(why.clone()),
                None => Err("the entry is still being read".into()),
            },
            None => return String::new(),
        };
        match cited {
            Ok(cited) if self.is_cited(&cited.id, &cited.digest) => {
                self.cited
                    .retain(|other| !(other.id == cited.id && other.digest == cited.digest));
                self.refusal = None;
                format!("Removed {} from the next question.", cited.id)
            }
            Ok(_) if self.cited.len() >= MAX_CITED => {
                format!("A question carries at most {MAX_CITED} citations.")
            }
            Ok(cited) => {
                let words = format!("Cited {} with the next question.", cited.id);
                self.cited.retain(|other| other.id != cited.id);
                self.cited.push(cited);
                self.refusal = None;
                words
            }
            Err(why) => {
                self.refusal = Some(crate::ascii::ascii(&why));
                format!("Not cited: {why}.")
            }
        }
    }
}

/// The view's text before wrapping.
#[must_use]
pub fn lines(page: &Page) -> Vec<(String, crate::paper::Tone)> {
    use crate::ascii::ascii;
    use crate::paper::Tone;
    let mut out = Vec::new();
    out.push((
        format!(
            "KNOWLEDGE AND PLANS  {} cited with the next question",
            page.cited.len()
        ),
        Tone::Loud,
    ));
    if page.cited.is_empty() {
        out.push((
            "Nothing is cited. Type words and ENTER to search knowledge (local and trusted, no \
             model); ENTER on an empty line opens the item picked, and ENTER again cites it."
                .into(),
            Tone::Quiet,
        ));
    } else {
        out.push((
            "SENT WITH THE NEXT QUESTION (open an item and press ENTER to remove it):".into(),
            Tone::Present,
        ));
        let preview = crate::context::Context {
            cited: page.cited.clone(),
            ..crate::context::Context::default()
        }
        .preview();
        for line in preview.lines() {
            out.push((format!("  | {line}"), Tone::Quiet));
        }
    }
    if let Some(why) = &page.refusal {
        out.push((format!("NOT CITED {why}"), Tone::Loud));
    }
    out.push((String::new(), Tone::Quiet));
    if let Some(item) = &page.opened {
        opened_lines(page, item, &mut out);
        return out;
    }
    match &page.goals {
        None => out.push(("STUDIO PLANS  [reading]".into(), Tone::Present)),
        Some(Err(why)) => out.push((format!("STUDIO PLANS  [unavailable] {why}"), Tone::Present)),
        Some(Ok(goals)) if goals.is_empty() => {
            out.push(("STUDIO PLANS  none".into(), Tone::Present));
        }
        Some(Ok(_)) => out.push((
            "STUDIO PLANS (studio memory, not published knowledge)".into(),
            Tone::Present,
        )),
    }
    let items = page.items();
    let plans = items
        .iter()
        .filter(|item| matches!(item, Item::Plan(_)))
        .count();
    for (index, item) in items.iter().enumerate() {
        if index == plans {
            out.push((String::new(), Tone::Quiet));
        }
        let picked = index == page.picked;
        let mark = if picked { ">" } else { " " };
        let tone = if picked { Tone::Loud } else { Tone::Present };
        match item {
            Item::Plan(goal) => out.push((
                format!(
                    "{mark} plan {}  {}  {} steps  {}{}",
                    ascii(&goal.goal_id),
                    ascii(&goal.status),
                    goal.entries.len(),
                    ascii(goal.text.lines().next().unwrap_or_default()),
                    if page.is_cited(&goal.goal_id, &goal.digest) {
                        "  [cited]"
                    } else {
                        ""
                    }
                ),
                tone,
            )),
            Item::Knowledge(entry) => out.push((
                format!(
                    "{mark} {} v{}  {}  by {}  {}{}",
                    ascii(&entry.id),
                    entry.version,
                    ascii(&entry.status),
                    ascii(&short_key(&entry.author)),
                    ascii(&entry.title),
                    if page.is_cited(&entry.id, &entry.digest) {
                        "  [cited]"
                    } else {
                        ""
                    }
                ),
                tone,
            )),
        }
    }
    match (&page.query, &page.hits) {
        (None, _) => out.push((
            "KNOWLEDGE  type words and ENTER to search".into(),
            Tone::Quiet,
        )),
        (Some(query), None) => out.push((
            format!("KNOWLEDGE  searching for {:?}", ascii(query)),
            Tone::Present,
        )),
        (Some(query), Some(Ok(hits))) if hits.is_empty() => out.push((
            format!("KNOWLEDGE  nothing matches {:?}", ascii(query)),
            Tone::Present,
        )),
        (Some(_), Some(Err(why))) => {
            out.push((format!("KNOWLEDGE  [unavailable] {why}"), Tone::Present));
        }
        _ => {}
    }
    out
}

fn short_key(author: &str) -> String {
    if author.len() == 64 && author.bytes().all(|b| b.is_ascii_hexdigit()) {
        format!("{}...", &author[..8])
    } else {
        author.to_owned()
    }
}

fn opened_lines(page: &Page, item: &Item, out: &mut Vec<(String, crate::paper::Tone)>) {
    use crate::ascii::ascii;
    use crate::paper::Tone;
    match item {
        Item::Plan(goal) => {
            let cited = page.is_cited(&goal.goal_id, &goal.digest);
            out.push((
                format!(
                    "PLAN {}  {}  digest {}  [{}]",
                    ascii(&goal.goal_id),
                    ascii(&goal.status),
                    short(&goal.digest),
                    if cited {
                        "cited; ENTER removes it"
                    } else {
                        "ENTER cites it"
                    }
                ),
                Tone::Loud,
            ));
            out.push((
                "Studio memory: the plan as the studio keeps it now, not published knowledge."
                    .into(),
                Tone::Quiet,
            ));
            out.push((format!("GOAL {}", ascii(goal.text.trim())), Tone::Present));
            for step in &goal.entries {
                out.push((
                    format!(
                        "  {} [{}] {}  {}",
                        ascii(&step.id),
                        ascii(&step.seat),
                        ascii(&step.title),
                        ascii(&word(&step.progress))
                    ),
                    Tone::Present,
                ));
            }
        }
        Item::Knowledge(found) => {
            let (shown, pending) = match &page.shown {
                None => {
                    out.push((format!("ENTRY {}  [reading]", ascii(&found.id)), Tone::Loud));
                    return;
                }
                Some(Err(why)) => {
                    out.push((
                        format!("ENTRY {}  [unavailable] {why}", ascii(&found.id)),
                        Tone::Loud,
                    ));
                    return;
                }
                Some(Ok((shown, pending))) => (shown, pending),
            };
            let cited = page.is_cited(&shown.id, &shown.digest);
            let action = match (shown.status.as_str(), cited) {
                (_, true) => "cited; ENTER removes it",
                ("admitted", false) => "ENTER cites it",
                _ => "not admitted; it can't be cited",
            };
            out.push((
                format!(
                    "ENTRY {} version {}  {}  [{action}]",
                    ascii(&shown.id),
                    shown.version,
                    ascii(&shown.status)
                ),
                Tone::Loud,
            ));
            if shown.version != found.version || shown.digest != found.digest {
                out.push((
                    format!(
                        "CHANGED since the search found version {}; search again to cite it.",
                        found.version
                    ),
                    Tone::Loud,
                ));
            }
            out.push((format!("TITLE {}", ascii(&shown.title)), Tone::Present));
            out.push((
                format!(
                    "AUTHOR {}  KIND {}  DIGEST {}",
                    ascii(&shown.author),
                    ascii(&shown.kind),
                    short(&shown.digest)
                ),
                Tone::Present,
            ));
            out.push((
                format!(
                    "WRITTEN FROM {}",
                    ascii(&if shown.written_from.is_empty() {
                        "not stated".to_owned()
                    } else {
                        shown.written_from.join(", ")
                    })
                ),
                Tone::Present,
            ));
            if let Some(next) = pending {
                out.push((
                    format!("Version {next} waits as a candidate; it is not what is cited."),
                    Tone::Quiet,
                ));
            }
            for line in &shown.cites {
                out.push((format!("CITES {}", ascii(line)), Tone::Quiet));
            }
            for line in &shown.evidence {
                out.push((format!("EVIDENCE {}", ascii(line)), Tone::Quiet));
            }
            out.push((String::new(), Tone::Quiet));
            for line in shown.body.lines() {
                out.push((format!("  {}", ascii(line)), Tone::Present));
            }
        }
    }
}

fn short(digest: &str) -> String {
    let hex = digest.trim_start_matches("sha256:");
    hex[..hex.len().min(12)].to_owned()
}
