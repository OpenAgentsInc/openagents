//! The Gym knowledge source behind the chat router's Gym and eval routes
//! ([`crate::router::seams::GymKb`]): verified records only, and retrieval
//! over them by embedding similarity and a typed Jev judgment.
//!
//! # What reaches the corpus
//!
//! Every record is admitted by code that checks it; nothing a model or a
//! message says becomes a record:
//!
//! - **Published results and checks**: `3189` events with the
//!   `oa:ext-eval:v1` marker, fetched from the relay ([`fetch_results`])
//!   and admitted only when `nostr::eval_ext::parse_publication` accepts
//!   them: the signature, both markers, the inline report against its
//!   digest and the `x` tag, the report under the profile, and every tag
//!   against what the report names. A check counts toward a result only as
//!   `nostr::eval_ext::linkage` reads it (same suite, subject, and lock; a
//!   different trainer).
//! - **Published test sets** (`eval-suite` releases) and **adoptions**
//!   (`coder-defaults` releases) through a [`ReleaseReader`], which needs
//!   the releases' manifest bytes; until an artifact fetcher is wired it
//!   is [`PendingReleases`], and a test set is read from the verified
//!   results that ran it instead.
//! - **The app's changelog**, `CHANGELOG` in
//!   `crates/openagents-mobile/src/account.rs`, compiled in and read by
//!   [`changelog`].
//! - **Our product notes**: the admitted entries of
//!   `knowledge/openagents/` tagged `gym` (notes) or `tool` (the tool
//!   catalog), which [`knowledge::product`] already checked for sources.
//!
//! # Retrieval
//!
//! For `gym.news`, candidates are the [`CANDIDATES`] items nearest the
//! latest message by embedding cosine similarity together with the
//! [`NEWEST`] newest dated records (recency is a record's field, not a
//! word), and one Jev request asks a `relevant_N` Noul for each. Items at
//! or above the router's relevance floor are kept, most relevant first,
//! at most [`crate::router::gym::MAX_NEWS`]. There is no word-matching
//! fallback: without embeddings or the judge, the lookup fails and the
//! router answers as if there were no records.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::future::BoxFuture;
use jev::{Answer, Noul, NoulCriteria, Questions, RetryPolicy};
use knowledge::product::Corpus;
use knowledge::search::{Embed, Embedder, cosine};
use nostr::domain::Event;
use nostr::eval_ext::{self, Linkage, Publication};
use serde_json::{Value, json};

use crate::generate::{Message, Role};
use crate::product_kb::Judge;
use crate::router::RELEVANCE_FLOOR;
use crate::router::gym::{
    AdoptionRecord, Checks, EventPointer, Grounding, Item, MAX_NEWS, Note, Records, Release,
    ResultRecord, SuiteRecord, Tool,
};
use crate::router::seams::{GymKb, GymLookup, SeamError};

/// The relevance question set's identity, for evidence.
pub const SET: &str = "gym-news-relevance-v1";

/// How many nearest items Jev judges.
pub const CANDIDATES: usize = 8;

/// How many of the newest dated records are judged besides the nearest.
pub const NEWEST: usize = 4;

/// How many of the newest app builds the corpus holds.
pub const BUILDS: usize = 3;

/// The earlier turns Jev reads besides the latest message.
pub const EARLIER_TURNS: usize = 4;

/// The longest earlier turn Jev reads, in characters.
pub const TURN_CHARS: usize = 400;

/// NIP-EVAL's publication kind.
pub const PUBLICATION_KIND: u16 = 3189;

/// The most results one relay read admits.
pub const MAX_RESULTS: usize = 500;

/// How long one relay read may take.
pub const FETCH_BUDGET: Duration = Duration::from_secs(15);

/// How often the worker reads the relay for new results.
pub const REFRESH: Duration = Duration::from_secs(600);

/// The product note tag for a Gym note.
pub const GYM_TAG: &str = "gym";

/// The product note tag for a tool in the catalog.
pub const TOOL_TAG: &str = "tool";

/// The catalog's default tool (`CHK-07`): first in [`Records::tools`].
pub const DEFAULT_TOOL: &str = "openagents.tool-project-map";

/// The changelog's path, which a build item cites.
pub const CHANGELOG_PATH: &str = "crates/openagents-mobile/src/account.rs";

/// The changelog source, compiled in.
const CHANGELOG_SOURCE: &str = include_str!("../../openagents-mobile/src/account.rs");

/// How long the Jev request may take.
pub const JUDGE_BUDGET: Duration = Duration::from_millis(1_500);

// ---------------------------------------------------------------------------
// The changelog
// ---------------------------------------------------------------------------

/// The string literal starting at `text`'s first `"`, unescaped, and the
/// rest after it.
fn literal(text: &str) -> Option<(String, &str)> {
    let start = text.find('"')?;
    let mut out = String::new();
    let mut chars = text[start + 1..].char_indices();
    while let Some((at, c)) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some((_, 'n')) => out.push(' '),
                Some((_, escaped)) => out.push(escaped),
                None => return None,
            },
            '"' => return Some((out, &text[start + 1 + at + 1..])),
            c => out.push(c),
        }
    }
    None
}

/// The value of the first `field: "…"` in `block`.
fn field(block: &str, field: &str) -> Option<String> {
    let at = block.find(&format!("{field}: \""))?;
    literal(&block[at + field.len()..]).map(|(value, _)| value)
}

/// The app's builds, newest first, from `CHANGELOG` in `source` (the
/// OpenAgents app's `account.rs`): each release's version, build, title,
/// and its items' titles. This reads a file of this repository for its
/// fixed shape; it reads no message.
#[must_use]
pub fn changelog_of(source: &str) -> Vec<Release> {
    let Some(start) = source.find("pub const CHANGELOG: &[Release] = &[") else {
        return Vec::new();
    };
    let body = &source[start..];
    // The list ends at the first line that closes it at the top level.
    let end = body.find("\n];").unwrap_or(body.len());
    let body = &body[..end];
    body.split("Release {")
        .skip(1)
        .filter_map(|block| {
            let (head, items) = block.split_once("items:").unwrap_or((block, ""));
            Some(Release {
                version: field(head, "version")?,
                build: field(head, "build")?,
                title: field(head, "title")?,
                items: items
                    .split("Item {")
                    .skip(1)
                    .filter_map(|item| field(item, "title"))
                    .collect(),
                source: CHANGELOG_PATH.to_string(),
            })
        })
        .collect()
}

/// The compiled-in changelog's builds, newest first.
#[must_use]
pub fn changelog() -> Vec<Release> {
    changelog_of(CHANGELOG_SOURCE)
}

// ---------------------------------------------------------------------------
// Product notes: Gym notes and the tool catalog
// ---------------------------------------------------------------------------

fn path_of(entry: &knowledge::Entry) -> String {
    format!("knowledge/{}/{}.md", knowledge::product::DIR, entry.id)
}

/// The tool catalog: the corpus's entries tagged `tool`, the default tool
/// first, then by id.
#[must_use]
pub fn tools(corpus: &Corpus) -> Vec<Tool> {
    let mut tools: Vec<Tool> = corpus
        .base
        .entries
        .iter()
        .filter(|entry| entry.tags.iter().any(|tag| tag == TOOL_TAG))
        .map(|entry| Tool {
            id: entry.id.clone(),
            name: entry.title.clone(),
            line: entry.summary.clone(),
            source: path_of(entry),
            slugs: entry
                .tags
                .iter()
                .filter(|tag| !matches!(tag.as_str(), TOOL_TAG | GYM_TAG))
                .cloned()
                .collect(),
        })
        .collect();
    tools.sort_by(|a, b| {
        (a.id != DEFAULT_TOOL)
            .cmp(&(b.id != DEFAULT_TOOL))
            .then(a.id.cmp(&b.id))
    });
    tools
}

/// The Gym notes: the corpus's entries tagged `gym` and not `tool`.
#[must_use]
pub fn notes(corpus: &Corpus) -> Vec<Note> {
    corpus
        .base
        .entries
        .iter()
        .filter(|entry| {
            entry.tags.iter().any(|tag| tag == GYM_TAG)
                && !entry.tags.iter().any(|tag| tag == TOOL_TAG)
        })
        .map(|entry| Note {
            id: format!("{}@{}", entry.id, entry.version),
            title: entry.title.clone(),
            summary: entry.summary.clone(),
            source: path_of(entry),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Published records
// ---------------------------------------------------------------------------

fn pointer(event: &Event) -> EventPointer {
    EventPointer {
        id: event.id.clone(),
        pubkey: event.pubkey.clone(),
        kind: event.kind,
    }
}

/// The catalog tool a subject is, by its DefinitionRef's package or
/// component slug among the tool notes' slugs, and the name to show: the
/// catalog's, else the package's.
#[must_use]
pub fn tool_of(
    tools: &[Tool],
    subject: &nostr::contracts::DefinitionRef,
) -> (Option<String>, String) {
    let (package, component) = subject
        .id
        .split_once(':')
        .and_then(|(_, rest)| rest.split_once('/'))
        .unwrap_or(("", ""));
    match tools.iter().find(|tool| {
        tool.slugs
            .iter()
            .any(|slug| slug == package || slug == component)
    }) {
        Some(tool) => (Some(tool.id.clone()), tool.name.clone()),
        None => (None, package.replace(['-', '_'], " ")),
    }
}

/// Reads published test sets and adoptions from their NIP-EXT releases,
/// which needs each release's manifest bytes.
pub trait ReleaseReader: Send + Sync {
    /// A published test set.
    ///
    /// # Errors
    ///
    /// Why it is not one.
    fn suite(&self, event: &Event, tools: &[Tool]) -> Result<SuiteRecord, String>;
    /// An adoption.
    ///
    /// # Errors
    ///
    /// Why it is not one.
    fn adoption(&self, event: &Event, tools: &[Tool]) -> Result<AdoptionRecord, String>;
}

/// No artifact fetcher yet: releases are not read, and a test set is read
/// from the verified results that ran it.
#[derive(Clone, Copy, Debug, Default)]
pub struct PendingReleases;

const PENDING: &str =
    "reading a release needs its manifest's bytes, and no artifact fetcher is wired";

impl ReleaseReader for PendingReleases {
    fn suite(&self, _: &Event, _: &[Tool]) -> Result<SuiteRecord, String> {
        Err(PENDING.to_string())
    }
    fn adoption(&self, _: &Event, _: &[Tool]) -> Result<AdoptionRecord, String> {
        Err(PENDING.to_string())
    }
}

/// What [`admit`] did with a batch of events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Admitted {
    pub results: Vec<ResultRecord>,
    pub suites: Vec<SuiteRecord>,
    pub adoptions: Vec<AdoptionRecord>,
    /// Refused events: id and why, naming a check, never content.
    pub refused: Vec<(String, String)>,
}

/// Admits `results` (`3189` events) that `nostr::eval_ext` verifies,
/// counts each original result's checks as `nostr::eval_ext::linkage`
/// reads them, and admits `suites` and `adoptions` through `releases`.
#[must_use]
pub fn admit(
    tools: &[Tool],
    results: &[Event],
    releases: &dyn ReleaseReader,
    suites: &[Event],
    adoptions: &[Event],
) -> Admitted {
    let mut admitted = Admitted::default();
    let mut read: Vec<(Publication, EventPointer)> = Vec::new();
    for event in results.iter().take(MAX_RESULTS) {
        match eval_ext::parse_publication(event) {
            Ok(publication) => {
                if !read.iter().any(|(seen, _)| seen.id == publication.id) {
                    read.push((publication, pointer(event)));
                }
            }
            Err(why) => admitted.refused.push((event.id.clone(), why.to_string())),
        }
    }
    for (publication, event) in &read {
        let mut checked = Checks::default();
        for (check, _) in &read {
            match eval_ext::linkage(publication, check) {
                Linkage::Confirm => checked.confirmed += 1,
                Linkage::Dispute => checked.disputed += 1,
                Linkage::NotACheck => {}
            }
        }
        let subject = publication.report.subject.definition.clone();
        let (tool, tool_name) = tool_of(tools, &subject);
        admitted.results.push(ResultRecord {
            publication: event.clone(),
            tool,
            tool_name,
            trainer: publication.trainer().to_string(),
            suite: publication.suite_release.clone(),
            cases: publication.report.profile.cases.len() as u64,
            subject,
            headline: publication.report.profile.headline,
            verdict: publication.verdict(),
            report: publication.report_ref.clone(),
            checks: publication.checks.clone(),
            checked,
            at: publication.created_at,
        });
    }
    for event in suites {
        match releases.suite(event, tools) {
            Ok(suite) => admitted.suites.push(suite),
            Err(why) => admitted.refused.push((event.id.clone(), why)),
        }
    }
    for event in adoptions {
        match releases.adoption(event, tools) {
            Ok(adoption) => admitted.adoptions.push(adoption),
            Err(why) => admitted.refused.push((event.id.clone(), why)),
        }
    }
    admitted
        .results
        .sort_by_key(|record| std::cmp::Reverse(record.at));
    admitted
        .suites
        .sort_by_key(|record| std::cmp::Reverse(record.at));
    admitted
        .adoptions
        .sort_by_key(|record| std::cmp::Reverse(record.at));
    admitted
}

/// The relay filter for published results.
#[must_use]
pub fn filter() -> Value {
    json!({
        "kinds": [PUBLICATION_KIND],
        "#t": [eval_ext::PROFILE_MARKER],
        "limit": MAX_RESULTS,
    })
}

/// Reads the published results from the relay at `url`, as `identity`:
/// one subscription with [`filter`], until the relay's end of stored
/// events or [`FETCH_BUDGET`]. The events are unverified here; [`admit`]
/// checks each one.
///
/// # Errors
///
/// The connection or the subscription fails.
pub async fn fetch_results(
    url: &str,
    identity: &crate::relay::Identity,
) -> Result<Vec<Event>, String> {
    use futures_util::StreamExt;
    let mut socket = crate::relay::connect(url, identity)
        .await
        .map_err(|error| error.to_string())?;
    let id = "gym-results";
    crate::relay::send(&mut socket, json!(["REQ", id, filter()]))
        .await
        .map_err(|error| error.to_string())?;
    let mut events = Vec::new();
    let reading = async {
        while let Some(frame) = socket.next().await {
            let Ok(tokio_tungstenite::tungstenite::Message::Text(text)) = frame else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            match (value[0].as_str(), value[1].as_str()) {
                (Some("EVENT"), Some(sub)) if sub == id => {
                    if let Ok(event) = serde_json::from_value::<Event>(value[2].clone()) {
                        events.push(event);
                        if events.len() >= MAX_RESULTS {
                            break;
                        }
                    }
                }
                (Some("EOSE" | "CLOSED"), Some(sub)) if sub == id => break,
                _ => {}
            }
        }
    };
    let finished = tokio::time::timeout(FETCH_BUDGET, reading).await;
    let _ = crate::relay::send(&mut socket, json!(["CLOSE", id])).await;
    let _ = socket.close(None).await;
    finished.map_err(|_| format!("no end of stored events in {} s", FETCH_BUDGET.as_secs()))?;
    Ok(events)
}

// ---------------------------------------------------------------------------
// The seam
// ---------------------------------------------------------------------------

/// The Gym's records and their retrieval.
pub struct GymKnowledge<E: Embed = Embedder> {
    records: Mutex<Records>,
    embedder: E,
    recipient: String,
    judge: Arc<dyn Judge>,
    releases: Arc<dyn ReleaseReader>,
    /// Item vectors by item id and the model that made them.
    vectors: Mutex<HashMap<String, Arc<Vec<f32>>>>,
}

impl GymKnowledge<Embedder> {
    /// The committed product corpus's tools and Gym notes, the compiled-in
    /// changelog, embeddings from the product KB's configuration, and
    /// `judge`; releases through [`PendingReleases`].
    ///
    /// # Errors
    ///
    /// The corpus does not load, or no embedder is set up.
    pub fn from_env(judge: Arc<dyn Judge>) -> Result<Self, String> {
        let dir = knowledge::product::default_dir();
        let root = knowledge::product::repository();
        let corpus = Corpus::load(&dir, root.join("knowledge").exists().then_some(&*root))?;
        let embedder = crate::product_kb::embedder_from_env()?;
        let recipient = crate::codebase::embedding_recipient(embedder.provider).to_string();
        Ok(GymKnowledge::new(
            &corpus,
            embedder,
            recipient,
            judge,
            Arc::new(PendingReleases),
        ))
    }
}

impl<E: Embed> GymKnowledge<E> {
    /// The Gym's records from `corpus` and the changelog, before any
    /// published record is admitted.
    pub fn new(
        corpus: &Corpus,
        embedder: E,
        recipient: impl Into<String>,
        judge: Arc<dyn Judge>,
        releases: Arc<dyn ReleaseReader>,
    ) -> Self {
        let records = Records {
            tools: tools(corpus),
            releases: changelog().into_iter().take(BUILDS).collect(),
            notes: notes(corpus),
            ..Records::default()
        };
        GymKnowledge {
            records: Mutex::new(records),
            embedder,
            recipient: recipient.into(),
            judge,
            releases,
            vectors: Mutex::new(HashMap::new()),
        }
    }

    /// The records as of now.
    #[must_use]
    pub fn records(&self) -> Records {
        self.records
            .lock()
            .map(|records| records.clone())
            .unwrap_or_default()
    }

    /// Reads the relay at `url` for published results and admits the
    /// verified ones in place of the last read's.
    ///
    /// # Errors
    ///
    /// The relay read fails; the records keep the last read's.
    pub async fn refresh(
        &self,
        url: &str,
        identity: &crate::relay::Identity,
    ) -> Result<Admitted, String> {
        let events = fetch_results(url, identity).await?;
        let tools = self.records().tools;
        let admitted = admit(&tools, &events, self.releases.as_ref(), &[], &[]);
        self.publish(&admitted);
        Ok(admitted)
    }

    /// Replaces the published records with what `admitted` holds.
    pub fn publish(&self, admitted: &Admitted) {
        if let Ok(mut records) = self.records.lock() {
            records.results = admitted.results.clone();
            records.suites = admitted.suites.clone();
            records.adoptions = admitted.adoptions.clone();
        }
    }

    /// The embedding provider, named for a person.
    #[must_use]
    pub fn recipient(&self) -> &str {
        &self.recipient
    }

    /// Vectors for `items`, embedding only the ones not yet embedded.
    async fn vectors(&self, items: &[Item]) -> Result<Vec<Arc<Vec<f32>>>, SeamError> {
        let key = |item: &Item| format!("{}\n{}", self.embedder.model(), item.id());
        let missing: Vec<&Item> = {
            let known = self
                .vectors
                .lock()
                .map_err(|_| SeamError::Failed("the vector cache is poisoned".to_string()))?;
            items
                .iter()
                .filter(|item| !known.contains_key(&key(item)))
                .collect()
        };
        if !missing.is_empty() {
            let (vectors, _) = self
                .embedder
                .embed(missing.iter().map(|item| item.text()).collect())
                .await
                .map_err(|e| SeamError::Failed(format!("embedding the records failed: {e}")))?;
            if vectors.len() != missing.len() {
                return Err(SeamError::Failed(format!(
                    "{} vectors came back for {} records",
                    vectors.len(),
                    missing.len()
                )));
            }
            if let Ok(mut known) = self.vectors.lock() {
                for (item, vector) in missing.iter().zip(vectors) {
                    known.insert(key(item), Arc::new(vector));
                }
            }
        }
        let known = self
            .vectors
            .lock()
            .map_err(|_| SeamError::Failed("the vector cache is poisoned".to_string()))?;
        items
            .iter()
            .map(|item| {
                known
                    .get(&key(item))
                    .cloned()
                    .ok_or_else(|| SeamError::Failed("a record has no vector".to_string()))
            })
            .collect()
    }

    /// Embeds every record now, so the first question does not wait.
    ///
    /// # Errors
    ///
    /// The embedding call fails.
    pub async fn warm(&self) -> Result<(), SeamError> {
        self.vectors(&self.records().items()).await.map(|_| ())
    }

    /// The candidates for `message`: the [`CANDIDATES`] nearest items and
    /// the [`NEWEST`] newest dated records, nearest first, each once.
    ///
    /// # Errors
    ///
    /// An embedding call fails.
    pub async fn candidates(&self, message: &str, items: &[Item]) -> Result<Vec<Item>, SeamError> {
        let vectors = self.vectors(items).await?;
        let (mut query, _) = self
            .embedder
            .embed(vec![
                message
                    .chars()
                    .take(knowledge::search::QUERY_CHARS)
                    .collect(),
            ])
            .await
            .map_err(|e| SeamError::Failed(format!("embedding the message failed: {e}")))?;
        let query = query
            .pop()
            .ok_or_else(|| SeamError::Failed("no vector came back for the message".to_string()))?;
        let mut ranked: Vec<(usize, f64)> = vectors
            .iter()
            .enumerate()
            .map(|(n, vector)| (n, cosine(vector, &query)))
            .collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut chosen: Vec<usize> = ranked.iter().take(CANDIDATES).map(|(n, _)| *n).collect();
        // `items` lists dated records newest first.
        for (n, item) in items.iter().enumerate() {
            if item.at().is_some()
                && !chosen.contains(&n)
                && chosen.iter().filter(|m| items[**m].at().is_some()).count() < NEWEST
            {
                chosen.push(n);
            }
        }
        Ok(chosen.into_iter().map(|n| items[n].clone()).collect())
    }

    /// The items relevant to `lookup`, with Jev's relevance, most relevant
    /// first (newer first on a tie), at most [`MAX_NEWS`].
    ///
    /// # Errors
    ///
    /// An embedding call or the Jev request fails; the error names no
    /// message text.
    pub async fn news(&self, lookup: &GymLookup) -> Result<Vec<(Item, f64)>, SeamError> {
        let items = self.records().items();
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let candidates = self.candidates(&lookup.message, &items).await?;
        let request =
            jev::SystemOneRequest::new(state(lookup, &candidates), questions(&candidates))
                .retry(RetryPolicy {
                    max_retries: 0,
                    budget: Some(JUDGE_BUDGET),
                    ..RetryPolicy::default()
                })
                .timeout(JUDGE_BUDGET);
        let response = self
            .judge
            .judge(request)
            .await
            .map_err(|e| SeamError::Failed(format!("the relevance judgment failed: {e}")))?;
        Ok(kept(&response, candidates))
    }
}

/// The key a candidate has in the state and its question id.
fn key(n: usize) -> String {
    format!("item_{}", n + 1)
}

/// What Jev reads: the latest message, the earlier turns, and the
/// candidates' texts.
#[must_use]
pub fn state(lookup: &GymLookup, items: &[Item]) -> Value {
    let earlier: Vec<Value> = lookup
        .transcript
        .iter()
        .rev()
        .skip_while(|m| m.role == Role::User && m.text.trim() == lookup.message.trim())
        .take(EARLIER_TURNS)
        .collect::<Vec<&Message>>()
        .into_iter()
        .rev()
        .map(|m| {
            json!({
                "role": match m.role { Role::User => "user", Role::Assistant => "assistant" },
                "text": m.text.chars().take(TURN_CHARS).collect::<String>(),
            })
        })
        .collect();
    let records: serde_json::Map<String, Value> = items
        .iter()
        .enumerate()
        .map(|(n, item)| {
            (
                key(n),
                json!({ "kind": item.kind(), "record": item.text() }),
            )
        })
        .collect();
    json!({
        "product": "The OpenAgents Gym, where people test tools on Coder, check each other's results, and earn XP",
        "latest_message": lookup.message,
        "earlier_turns": earlier,
        "records": records,
    })
}

/// One `relevant_N` Noul per candidate.
#[must_use]
pub fn questions(items: &[Item]) -> Questions {
    let mut questions = Questions::new();
    for n in 0..items.len() {
        let id = key(n);
        questions = questions.with(
            format!("relevant_{}", n + 1),
            Noul::with_criteria(
                format!(
                    "Would a good reply to the user's latest message, about what is new or in \
                     progress in the Gym, mention {id} in the state's records?"
                ),
                NoulCriteria::new()
                    .when_true(format!(
                        "Yes: {id} is news the user asked about, or part of what is new or in \
                         progress that they asked for"
                    ))
                    .when_false(format!(
                        "No: {id} is about something the user did not ask about, or only shares \
                         words with the message"
                    )),
            ),
        );
    }
    questions
}

/// The candidates at or above the relevance floor, most relevant first,
/// then newer first, at most [`MAX_NEWS`].
#[must_use]
pub fn kept(response: &jev::SystemOneResponse, candidates: Vec<Item>) -> Vec<(Item, f64)> {
    let relevance = |n: usize| match response.answers.get(&format!("relevant_{}", n + 1)) {
        Some(Answer::Noul(noul)) if noul.noul.is_finite() => noul.noul.clamp(0.0, 1.0),
        _ => 0.0,
    };
    let mut kept: Vec<(Item, f64)> = candidates
        .into_iter()
        .enumerate()
        .map(|(n, item)| (item, relevance(n)))
        .filter(|(_, p)| *p >= RELEVANCE_FLOOR)
        .collect();
    kept.sort_by(|(a, pa), (b, pb)| pb.total_cmp(pa).then(b.at().cmp(&a.at())));
    kept.truncate(MAX_NEWS);
    kept
}

impl GymKb for GymKnowledge<Embedder> {
    fn available(&self) -> bool {
        let records = self.records();
        !records.tools.is_empty() || !records.items().is_empty()
    }

    fn recipients(&self) -> Vec<String> {
        vec![self.recipient.clone()]
    }

    fn tools(&self) -> Vec<Tool> {
        self.records().tools
    }

    fn ground<'a>(&'a self, lookup: &'a GymLookup) -> BoxFuture<'a, Result<Grounding, SeamError>> {
        Box::pin(async move {
            let news = if lookup.route == crate::router::RouteId::GymNews {
                self.news(lookup).await?
            } else {
                Vec::new()
            };
            Ok(Grounding {
                records: self.records(),
                news,
            })
        })
    }
}

#[cfg(test)]
mod tests;
