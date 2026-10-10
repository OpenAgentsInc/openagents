//! A workshop agent's scored memory stream (`docs/verse/generative-agents.md`,
//! item 1): the briefing a terminal-mode request carries, chosen from her
//! memory entries and her newest journal rows by recency, importance, and
//! relevance.
//!
//! - **Recency** is `0.99^hours` since a briefing last carried the record,
//!   read from the selection receipts the journal already holds, or since
//!   the record was written when none carried it. Nothing new is stored.
//! - **Importance** is 1 to 10, set once per record: by [`rule`] when the
//!   record is a known case, else by Jev through a [`Judge`] and the
//!   `questions/memory-importance.json` set, else, until Jev scores it, by
//!   the kind's [`prior`]. Rule and Jev scores go to the sidecar
//!   `agents/NAME/scores.jsonl` ([`SCORE_SCHEMA`]).
//! - **Relevance** comes from a [`Relevance`]: cosine similarity over the
//!   knowledge embedder ([`Embedded`]), or BM25 ([`Lexical`]) when no
//!   embedder is set up or a call fails.
//!
//! `memory-stream` normalizes and combines the terms. Notes and accepted
//! preferences keep their standing place at the top; the briefing stays
//! within [`BRIEFING_MAX`] and the journal records what it carried.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::agent::{Entry, Kind, Store};
use super::agent_memory::{BRIEFING_MAX, Memory, MemoryEntry, MemoryKind, MemoryState};
use crate::questions::{Fill, Set};

#[path = "agent_recall_live.rs"]
mod live;
pub use live::{Embedded, JevJudge};

/// One score row's schema.
pub const SCORE_SCHEMA: &str = "openagents.agent-memory-score.v1";
/// The newest journal rows a briefing considers, beside every memory entry.
pub const JOURNAL_WINDOW: usize = 2000;
/// The most records Jev scores while one briefing is built; the rest keep
/// their prior until a later briefing.
pub const JEV_PER_BRIEFING: usize = 16;
/// The most scored records a briefing carries beside the standing ones.
pub const BRIEFING_RECORDS: usize = 40;

const SET_JSON: &str = include_str!("../../../../questions/memory-importance.json");

static SET: LazyLock<Set> = LazyLock::new(|| {
    let set: Set = serde_json::from_str(SET_JSON).expect("the memory-importance set parses");
    set.validate()
        .expect("the memory-importance set is one this host asks");
    set
});

/// The importance question set.
#[must_use]
pub fn importance_set() -> &'static Set {
    &SET
}

/// A record in the stream: `journal:POS` (1-based line) or `memory:ID`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ref {
    Journal(usize),
    Memory(u64),
}

impl fmt::Display for Ref {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Journal(pos) => write!(f, "journal:{pos}"),
            Self::Memory(id) => write!(f, "memory:{id}"),
        }
    }
}

impl Ref {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (kind, number) = text.split_once(':')?;
        match kind {
            "journal" => number.parse().ok().map(Self::Journal),
            "memory" => number.parse().ok().map(Self::Memory),
            _ => None,
        }
    }
}

/// What a record holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    Journal(Entry),
    Memory(MemoryEntry),
}

/// One record of the stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub reference: Ref,
    pub body: Body,
}

impl Record {
    /// When it was written, Unix seconds.
    #[must_use]
    pub fn at(&self) -> u64 {
        match &self.body {
            Body::Journal(entry) => entry.at,
            Body::Memory(entry) => entry.at,
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        match &self.body {
            Body::Journal(entry) => &entry.text,
            Body::Memory(entry) => &entry.text,
        }
    }

    /// The row's or entry's kind, as its file spells it.
    #[must_use]
    pub fn kind(&self) -> String {
        match &self.body {
            Body::Journal(entry) => kind_word(entry.kind),
            Body::Memory(entry) => entry.kind.word().to_string(),
        }
    }

    /// A note or an accepted preference: you wrote or accepted it, so it
    /// goes first in every briefing.
    #[must_use]
    pub fn standing(&self) -> bool {
        matches!(&self.body, Body::Memory(entry)
            if entry.state == MemoryState::Active
                && matches!(entry.kind, MemoryKind::Note | MemoryKind::Preference))
    }

    /// SHA-256 of the record's kind and text, so a score row never applies
    /// to another record that later takes the same reference.
    #[must_use]
    pub fn digest(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(self.kind().as_bytes());
        hash.update([0]);
        hash.update(self.text().as_bytes());
        hex(&hash.finalize())
    }

    /// The briefing line: kind, date, and text.
    #[must_use]
    pub fn line(&self) -> String {
        let status = match &self.body {
            Body::Journal(Entry {
                status: Some(code), ..
            }) => format!(", exit {code}"),
            _ => String::new(),
        };
        format!(
            "- ({}{status}, {}) {}\n",
            self.kind(),
            day(self.at()),
            self.text().replace('\n', " ")
        )
    }
}

fn kind_word(kind: Kind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A Unix time as a civil UTC date, such as "September 9, 2026".
#[must_use]
pub fn day(at: u64) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let stamp = gym::eval::utc_from_unix(at);
    let year = &stamp[0..4];
    let month: usize = stamp[5..7].parse().unwrap_or(1);
    let date: u32 = stamp[8..10].parse().unwrap_or(1);
    format!("{} {date}, {year}", MONTHS[month.clamp(1, 12) - 1])
}

/// Whether a note or preference entry in `memory` was kept from the
/// request `text`: the entry's text holds the request's.
fn kept_in_memory(text: &str, memory: &[MemoryEntry]) -> bool {
    let text = text.trim();
    !text.is_empty()
        && memory.iter().any(|e| {
            matches!(e.kind, MemoryKind::Note | MemoryKind::Preference) && e.text.contains(text)
        })
}

/// The records a briefing chooses from: every active memory entry, and the
/// journal rows in `journal` that say something of their own. Memory rows
/// (receipts and the lines that wrote, accepted, or forgot an entry) and
/// requests a note or preference entry was kept from (its text holds the
/// request's) stay out, so a candidate preference waits for you and a forgotten
/// entry isn't carried back in through the line that wrote it.
#[must_use]
pub fn candidates(journal: &[(usize, Entry)], memory: &[MemoryEntry]) -> Vec<Record> {
    let mut records: Vec<Record> = memory
        .iter()
        .filter(|e| e.state == MemoryState::Active)
        .map(|e| Record {
            reference: Ref::Memory(e.id),
            body: Body::Memory(e.clone()),
        })
        .collect();
    let skip = journal.len().saturating_sub(JOURNAL_WINDOW);
    records.extend(
        journal
            .iter()
            .skip(skip)
            .filter(|(_, e)| e.kind != Kind::Memory && !e.text.trim().is_empty())
            .filter(|(_, e)| e.kind != Kind::Request || !kept_in_memory(&e.text, memory))
            .map(|(pos, e)| Record {
                reference: Ref::Journal(*pos),
                body: Body::Journal(e.clone()),
            }),
    );
    records
}

const RECEIPT: &str = "the briefing carried ";

/// The selection receipt's journal text for `carried`, or `None` when it
/// carried nothing. A briefing of memory entries alone reads as it always
/// has: "the briefing carried memory entries 1, 3".
#[must_use]
pub fn receipt(carried: &[Ref]) -> Option<String> {
    let join = |pick: &dyn Fn(&Ref) -> Option<String>| -> String {
        carried
            .iter()
            .filter_map(pick)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let memory = join(&|r| match r {
        Ref::Memory(id) => Some(id.to_string()),
        Ref::Journal(_) => None,
    });
    let journal = join(&|r| match r {
        Ref::Journal(pos) => Some(pos.to_string()),
        Ref::Memory(_) => None,
    });
    let parts: Vec<String> = [
        (!memory.is_empty()).then(|| format!("memory entries {memory}")),
        (!journal.is_empty()).then(|| format!("journal rows {journal}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!parts.is_empty()).then(|| format!("{RECEIPT}{}", parts.join("; ")))
}

/// The records a receipt's text names.
#[must_use]
pub fn parse_receipt(text: &str) -> Vec<Ref> {
    let Some(rest) = text.strip_prefix(RECEIPT) else {
        return Vec::new();
    };
    let mut refs = Vec::new();
    for part in rest.split("; ") {
        let (make, list): (fn(u64) -> Ref, &str) =
            if let Some(list) = part.strip_prefix("memory entries ") {
                (Ref::Memory, list)
            } else if let Some(list) = part.strip_prefix("journal rows ") {
                (|n| Ref::Journal(n as usize), list)
            } else {
                continue;
            };
        refs.extend(
            list.split(", ")
                .filter_map(|n| n.trim().parse::<u64>().ok())
                .map(make),
        );
    }
    refs
}

/// When a briefing last carried each record, from the receipts in
/// `journal` written before `before`.
#[must_use]
pub fn last_carried(journal: &[(usize, Entry)], before: u64) -> HashMap<Ref, u64> {
    let mut last = HashMap::new();
    for (_, entry) in journal {
        if entry.kind != Kind::Memory || entry.at > before {
            continue;
        }
        for reference in parse_receipt(&entry.text) {
            let at = last.entry(reference).or_insert(entry.at);
            *at = (*at).max(entry.at);
        }
    }
    last
}

/// The rule table: importance for a record whose importance is a known
/// case, or `None` for Jev to judge.
#[must_use]
pub fn rule(record: &Record) -> Option<f64> {
    let text = record.text().to_ascii_lowercase();
    match &record.body {
        Body::Memory(entry) => match entry.kind {
            // You wrote or accepted it.
            MemoryKind::Note | MemoryKind::Preference => Some(8.0),
            // The host's line for a request that ran clean.
            MemoryKind::Outcome if text.ends_with("(ok exit 0)") => Some(1.0),
            MemoryKind::Outcome | MemoryKind::Project | MemoryKind::Insight => None,
        },
        Body::Journal(entry) => match entry.kind {
            Kind::Ran if entry.status == Some(0) => Some(1.0),
            Kind::Typed | Kind::Created | Kind::Migrated | Kind::Keyed => Some(1.0),
            Kind::Job if text.contains(" fired, occurrence ") => Some(1.0),
            Kind::Control if text.contains("stop") || text.contains("retire") => Some(10.0),
            _ => None,
        },
    }
}

/// The importance a record has until Jev scores it: a guess from its kind.
#[must_use]
pub fn prior(record: &Record) -> f64 {
    let text = record.text().to_ascii_lowercase();
    match &record.body {
        Body::Memory(entry) => match entry.kind {
            MemoryKind::Note | MemoryKind::Preference => 8.0,
            MemoryKind::Insight => 6.0,
            MemoryKind::Project => 5.0,
            MemoryKind::Outcome if text.contains("merged") || text.contains("rejected") => 7.0,
            MemoryKind::Outcome => 3.0,
        },
        Body::Journal(entry) => match entry.kind {
            Kind::Control => 8.0,
            Kind::Task if text.contains("merged") || text.contains("rejected") => 7.0,
            Kind::Rejected | Kind::Refused | Kind::Takeback => 5.0,
            Kind::Failed => 5.0,
            Kind::Ran => 4.0,
            Kind::Task | Kind::Proposed | Kind::Confirmed | Kind::Job => 3.0,
            Kind::Request | Kind::Report | Kind::Plan => 2.0,
            _ => 1.0,
        },
    }
}

/// Who set a record's importance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum By {
    Rule,
    Jev,
    /// The kind's prior, until Jev scores it; never written to the sidecar.
    Prior,
}

/// One sidecar row (`openagents.agent-memory-score.v1`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreRow {
    pub schema: String,
    pub v: u32,
    /// `journal:POS` or `memory:ID`.
    pub record: String,
    /// [`Record::digest`] when it was scored.
    pub digest: String,
    /// 1 through 10.
    pub importance: f64,
    pub by: By,
    /// Jev's question set and the digest of its wording.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_digest: Option<String>,
    /// Jev's probability for each level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probabilities: Option<BTreeMap<u32, f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Unix seconds.
    pub at: u64,
}

impl ScoreRow {
    fn new(record: &Record, importance: f64, by: By, at: u64) -> Self {
        Self {
            schema: SCORE_SCHEMA.into(),
            v: 1,
            record: record.reference.to_string(),
            digest: record.digest(),
            importance: memory_stream::clamp_importance(importance),
            by,
            set: None,
            set_digest: None,
            probabilities: None,
            model: None,
            at,
        }
    }
}

/// Jev's answer for one record.
#[derive(Clone, Debug, PartialEq)]
pub struct Judged {
    /// 1 through 10.
    pub importance: f64,
    pub probabilities: BTreeMap<u32, f64>,
    pub model: String,
}

/// Rates records' importance with a model.
pub trait Judge {
    /// One answer per record, in order.
    fn judge(&mut self, agent: &str, records: &[&Record]) -> Vec<Result<Judged, String>>;
}

/// The importance question's state for `record`.
#[must_use]
pub fn judge_state(agent: &str, record: &Record) -> serde_json::Value {
    let mut fields = serde_json::json!({
        "source": match record.reference {
            Ref::Journal(_) => "journal row",
            Ref::Memory(_) => "memory entry",
        },
        "kind": record.kind(),
        "date": day(record.at()),
        "text": record.text(),
    });
    match &record.body {
        Body::Journal(entry) => {
            if let Some(status) = entry.status {
                fields["status"] = status.into();
            }
            if let Some(from) = &entry.from {
                fields["from"] = from.clone().into();
            }
        }
        Body::Memory(entry) => {
            fields["author"] = serde_json::to_value(entry.author).unwrap_or_default();
        }
    }
    serde_json::json!({ "agent": agent, "record": fields })
}

/// A Score answer's probability-weighted level, `levels` of them, mapped
/// linearly onto 1 through 10: the lowest level is 1 and the highest 10.
#[must_use]
pub fn importance_from(probabilities: &BTreeMap<u32, f64>, score: f64, levels: usize) -> f64 {
    let mass: f64 = probabilities.values().sum();
    let position = if mass > 0.0 {
        probabilities
            .iter()
            .map(|(level, p)| f64::from(*level) * p)
            .sum::<f64>()
            / mass
    } else {
        score
    };
    let top = levels.saturating_sub(1).max(1) as f64;
    memory_stream::clamp_importance(1.0 + 9.0 * position / top)
}

/// The decide request for one record.
///
/// # Errors
/// When the state is larger than the set's policy admits.
pub fn judge_request(agent: &str, record: &Record) -> Result<jev::SystemOneRequest, String> {
    let state = judge_state(agent, record);
    let size = serde_json::to_vec(&state).map_or(usize::MAX, |b| b.len());
    if let Some(max) = SET.policy.state_max_bytes
        && size as u64 > max
    {
        return Err(format!("the state is {size} bytes, over the set's {max}"));
    }
    let questions = SET.build(&Fill::None)?;
    Ok(jev::SystemOneRequest::new(state, questions))
}

/// Rates relevance to a query.
pub trait Relevance {
    /// What the last ranking used, such as `bm25` or `cosine MODEL`.
    fn basis(&self) -> String;

    /// One relevance per text, in order, on the measure's own scale.
    fn relevance(&mut self, query: &str, texts: &[String]) -> Vec<f64>;
}

/// BM25 over the texts, as knowledge search ranks without embeddings.
#[derive(Clone, Copy, Debug, Default)]
pub struct Lexical;

impl Relevance for Lexical {
    fn basis(&self) -> String {
        "bm25".into()
    }

    fn relevance(&mut self, query: &str, texts: &[String]) -> Vec<f64> {
        knowledge::search::bm25_texts(texts, query)
    }
}

/// What a scored briefing asks: Jev, when one is set up, and a relevance.
pub struct Services {
    pub judge: Option<Box<dyn Judge>>,
    pub relevance: Box<dyn Relevance>,
}

impl Services {
    /// No model and no network: rule and prior importance, BM25 relevance.
    #[must_use]
    pub fn offline() -> Self {
        Self {
            judge: None,
            relevance: Box::new(Lexical),
        }
    }

    /// Jev from the decision profile, and the knowledge embedder with its
    /// vectors cached under `store`; either one missing falls back to
    /// priors or BM25.
    #[must_use]
    pub fn live(store: &Store) -> Self {
        if super::sales::privacy::model_available(store).is_err() {
            return Self::offline();
        }
        let judge = match crate::decision::from_env() {
            Ok(Some(client)) => JevJudge::new(client)
                .ok()
                .map(|judge| Box::new(judge) as Box<dyn Judge>),
            _ => None,
        };
        let relevance: Box<dyn Relevance> = match knowledge::search::Embedder::from_env() {
            Ok(embedder) => Box::new(Embedded::new(embedder, store.dir())),
            Err(_) => Box::new(Lexical),
        };
        Self { judge, relevance }
    }
}

/// Makes the [`Services`] for one agent's briefing.
pub type ServicesFactory = Arc<dyn Fn(&Store) -> Services + Send + Sync>;

/// How a host chooses a terminal request's briefing.
#[derive(Clone)]
pub enum Briefing {
    /// Words shared with the request, [`Memory::briefing`]: the baseline.
    WordOverlap,
    /// The scored stream, with the services the factory makes.
    Scored(ServicesFactory),
}

impl Briefing {
    /// The scored stream with live services, or offline ones in a unit test.
    #[must_use]
    pub fn default_scored() -> Self {
        if cfg!(test) {
            Self::Scored(Arc::new(|_: &Store| Services::offline()))
        } else {
            Self::Scored(Arc::new(Services::live))
        }
    }
}

impl fmt::Debug for Briefing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::WordOverlap => "WordOverlap",
            Self::Scored(_) => "Scored",
        })
    }
}

/// A briefing and the records it carried.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recall {
    pub text: String,
    pub carried: Vec<Ref>,
    /// What relevance used.
    pub basis: String,
    /// Score rows this briefing set, for the sidecar.
    pub scored: Vec<ScoreRow>,
}

/// What one briefing reads.
#[derive(Clone, Copy, Debug)]
pub struct Inputs<'a> {
    pub agent: &'a str,
    pub request: &'a str,
    pub workspace: &'a str,
    /// Unix seconds.
    pub now: u64,
    /// The journal with positions, oldest first.
    pub journal: &'a [(usize, Entry)],
    pub memory: &'a [MemoryEntry],
}

/// The candidates in score order, best first, and the score rows set while
/// ranking them.
#[derive(Clone, Debug, Default)]
pub struct Ranked {
    pub records: Vec<Record>,
    /// Indexes into `records`, best first.
    pub order: Vec<usize>,
    pub scored: Vec<ScoreRow>,
}

/// A record's importance as the stream reads it without asking Jev: the
/// sidecar's row when its digest still matches, else the rule, else the
/// kind's prior.
#[must_use]
pub fn importance_of(record: &Record, known: &HashMap<String, ScoreRow>) -> f64 {
    known
        .get(&record.reference.to_string())
        .filter(|row| row.digest == record.digest())
        .map(|row| row.importance)
        .or_else(|| rule(record))
        .unwrap_or_else(|| prior(record))
}

/// Ranks the candidates for `inputs`: importance (sidecar, rule, Jev, then
/// prior), recency from receipts, and relevance, normalized and summed
/// with equal weights.
#[must_use]
pub fn rank(
    inputs: &Inputs<'_>,
    known: &HashMap<String, ScoreRow>,
    services: &mut Services,
) -> Ranked {
    let records = candidates(inputs.journal, inputs.memory);
    let carried_at = last_carried(inputs.journal, inputs.now);
    let mut scored = Vec::new();
    let mut importance: Vec<Option<f64>> = records
        .iter()
        .map(|record| {
            if let Some(row) = known
                .get(&record.reference.to_string())
                .filter(|row| row.digest == record.digest())
            {
                return Some(row.importance);
            }
            let value = rule(record)?;
            scored.push(ScoreRow::new(record, value, By::Rule, inputs.now));
            Some(value)
        })
        .collect();
    if let Some(judge) = services.judge.as_mut() {
        // The newest unscored first.
        let mut pending: Vec<usize> = (0..records.len())
            .filter(|&i| importance[i].is_none())
            .collect();
        pending.sort_by_key(|&i| std::cmp::Reverse(records[i].at()));
        pending.truncate(JEV_PER_BRIEFING);
        let asked: Vec<&Record> = pending.iter().map(|&i| &records[i]).collect();
        for (&i, answer) in pending.iter().zip(judge.judge(inputs.agent, &asked)) {
            let Ok(judged) = answer else { continue };
            let mut row = ScoreRow::new(&records[i], judged.importance, By::Jev, inputs.now);
            row.set = Some(SET.id.clone());
            row.set_digest = Some(SET.digest());
            row.probabilities = Some(judged.probabilities);
            row.model = Some(judged.model);
            importance[i] = Some(row.importance);
            scored.push(row);
        }
    }
    let texts: Vec<String> = records.iter().map(|r| r.text().to_string()).collect();
    let query = format!("{}\n{}", inputs.request, inputs.workspace);
    let relevance = services.relevance.relevance(&query, &texts);
    let terms: Vec<memory_stream::Terms> = records
        .iter()
        .enumerate()
        .map(|(i, record)| memory_stream::Terms {
            recency: memory_stream::recency_since(
                inputs.now,
                carried_at
                    .get(&record.reference)
                    .copied()
                    .unwrap_or_else(|| record.at()),
            ),
            importance: importance[i].unwrap_or_else(|| prior(record)),
            relevance: relevance.get(i).copied().unwrap_or(0.0),
        })
        .collect();
    let scores = memory_stream::score(&terms, memory_stream::Weights::default());
    let order = memory_stream::ranked(&scores);
    Ranked {
        records,
        order,
        scored,
    }
}

/// The `k` best records for `inputs.request` at `inputs.now`, by score, with
/// no standing priority: what a reflection shows for one question. The
/// score rows set while ranking come back for the sidecar.
#[must_use]
pub fn retrieve(
    inputs: &Inputs<'_>,
    known: &HashMap<String, ScoreRow>,
    services: &mut Services,
    k: usize,
) -> (Vec<Record>, Vec<ScoreRow>) {
    let ranked = rank(inputs, known, services);
    let best = ranked
        .order
        .iter()
        .take(k)
        .map(|&i| ranked.records[i].clone())
        .collect();
    (best, ranked.scored)
}

/// The scored briefing: the candidates as [`rank`] orders them, standing
/// records first, then the best, within [`BRIEFING_RECORDS`] and
/// [`BRIEFING_MAX`].
#[must_use]
pub fn recall(
    inputs: &Inputs<'_>,
    known: &HashMap<String, ScoreRow>,
    services: &mut Services,
) -> Recall {
    let Ranked {
        records,
        order,
        scored,
    } = rank(inputs, known, services);
    let mut text = String::new();
    let mut carried = Vec::new();
    let mut ranked_taken = 0;
    let standing = order.iter().filter(|&&i| records[i].standing());
    let rest = order.iter().filter(|&&i| !records[i].standing());
    for (&i, is_standing) in standing.map(|i| (i, true)).chain(rest.map(|i| (i, false))) {
        if !is_standing && ranked_taken >= BRIEFING_RECORDS {
            break;
        }
        let line = records[i].line();
        if text.len() + line.len() > BRIEFING_MAX {
            continue;
        }
        text.push_str(&line);
        carried.push(records[i].reference);
        ranked_taken += usize::from(!is_standing);
    }
    Recall {
        text,
        carried,
        basis: services.relevance.basis(),
        scored,
    }
}

/// The sidecar `agents/NAME/scores.jsonl`.
#[derive(Clone, Debug)]
pub struct Scores {
    path: PathBuf,
}

impl Scores {
    #[must_use]
    pub fn of(store: &Store) -> Self {
        Self::at(store.dir())
    }

    #[must_use]
    pub fn at(dir: &Path) -> Self {
        Self {
            path: dir.join("scores.jsonl"),
        }
    }

    /// The newest row for each record. A line that doesn't read is skipped.
    ///
    /// # Errors
    /// When the file exists and cannot be read.
    pub fn load(&self) -> Result<HashMap<String, ScoreRow>, String> {
        let directory = self.path.parent().ok_or("score namespace is unavailable")?;
        let Some(text) = super::sales::privacy::read_agent_directory_text(directory, &self.path)?
        else {
            return Ok(HashMap::new());
        };
        Ok(text
            .lines()
            .filter_map(|line| serde_json::from_str::<ScoreRow>(line).ok())
            .filter(|row| row.schema == SCORE_SCHEMA && row.v == 1)
            .map(|row| (row.record.clone(), row))
            .collect())
    }

    /// Appends `rows`.
    ///
    /// # Errors
    /// When the file cannot be written.
    pub fn append(&self, rows: &[ScoreRow]) -> Result<(), String> {
        if rows.is_empty() {
            return Ok(());
        }
        let mut body = Vec::new();
        for row in rows {
            body.extend(serde_json::to_vec(row).map_err(|e| e.to_string())?);
            body.push(b'\n');
        }
        super::sales::privacy::append_agent_directory_text(
            self.path.parent().ok_or("score namespace is unavailable")?,
            &self.path,
            std::str::from_utf8(&body).map_err(|_| "score serialization is not UTF-8")?,
        )
    }
}

impl Memory {
    /// The scored briefing for `request` in `workspace` at `now`, from this
    /// agent's memory, journal, and score sidecar; new scores are appended
    /// to the sidecar.
    ///
    /// # Errors
    /// When the memory or the journal cannot be read.
    pub fn recall(
        &self,
        request: &str,
        workspace: &str,
        now: u64,
        services: &mut Services,
    ) -> Result<Recall, String> {
        let store = self.store();
        super::sales::privacy::model_available(store)?;
        super::sales::privacy::check_agent_copy(store, &format!("{request}\n{workspace}"))?;
        let journal = store.journal_rows()?;
        let memory = self.entries()?;
        let sidecar = Scores::of(store);
        let known = sidecar.load().unwrap_or_default();
        let recall = recall(
            &Inputs {
                agent: store.name(),
                request,
                workspace,
                now,
                journal: &journal,
                memory: &memory,
            },
            &known,
            services,
        );
        // A briefing still goes out when the sidecar can't be written.
        let _ = sidecar.append(&recall.scored);
        super::agent_engrams::write_through_scores(self, &recall.scored, now);
        Ok(recall)
    }
}

#[cfg(test)]
#[path = "agent_recall_tests.rs"]
mod tests;
