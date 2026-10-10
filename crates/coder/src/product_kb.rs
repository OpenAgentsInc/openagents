//! The product knowledge base behind the chat router's `product.kb` route
//! ([`crate::router::seams::ProductKb`]): the admitted entries in
//! `knowledge/openagents/` ([`knowledge::product`]), found by embedding
//! similarity and kept by a typed Jev judgment.
//!
//! A lookup takes two requests:
//!
//! 1. **Candidates by embedding.** The user's latest message is embedded
//!    and compared by cosine similarity with each entry's embedded search
//!    text (title, summary, tags, and `applies_when`); the [`CANDIDATES`]
//!    nearest are candidates. Entry vectors are made once per process and
//!    kept in memory. There is no word-matching fallback: without
//!    embeddings the seam fails and the router answers with the model
//!    alone, as it would with no knowledge base.
//! 2. **Relevance by Jev.** One System One request over the latest message,
//!    the earlier turns, and the candidates asks, for each candidate, a
//!    Noul (`relevant_N`: would a correct reply use this entry?) and, over
//!    the candidates that carry a reviewed answer, one Choice (`answer`:
//!    whose answer fully answers the message as asked, or `none`). The
//!    questions are independent, so they cost one round trip.
//!
//! The [`Grounding`] keeps the candidates at or above
//! [`crate::router::RELEVANCE_FLOOR`], most relevant first, at most
//! [`KEEP`]. A passage carries its entry's reviewed answer only when the
//! `answer` Choice picked that entry with probability at least
//! [`crate::router::KB_ANSWER_CONFIDENCE`]; the router then serves it
//! whole (T0) when its relevance also clears that bar and the message
//! needs no specifics. Otherwise the passages ground the model (T2):
//! [`knowledge::product::instructions`] tells it to answer only from them
//! and cite them, and [`knowledge::product::check_reply`] reads what it
//! cited.
//!
//! Nothing here reads the message for words: candidates are an embedding
//! ranking, and what is kept or served is a Choice's or a Noul's
//! probability over options this module lists, as `AGENTS.md` requires.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::future::BoxFuture;
use indexmap::IndexMap;
use jev::{Answer, Choice, Entry as JevEntry, Noul, NoulCriteria, Questions, RetryPolicy};
use knowledge::product::{self, Corpus, Reference};
use knowledge::search::{Embed, Embedder, EmbeddingProvider, cosine};
use serde_json::{Value, json};

use crate::generate::{Message, Role};
use crate::router::seams::{Grounding, Lookup, Passage, ProductKb, SeamError};
use crate::router::{KB_ANSWER_CONFIDENCE, RELEVANCE_FLOOR};

/// The relevance question set's identity, for evidence.
pub const SET: &str = "product-kb-relevance-v1";

/// How many nearest entries Jev judges.
pub const CANDIDATES: usize = 8;

/// The most passages a grounding keeps.
pub const KEEP: usize = 6;

/// The tag of an entry whose reviewed answer assumes the chat is not on a
/// computer ("we can't reach your computer"): a chat on the computer Coder
/// runs on never shows it whole (#10077).
pub const OFF_COMPUTER_TAG: &str = "off-computer";

/// The earlier turns Jev reads besides the latest message, newest last.
pub const EARLIER_TURNS: usize = 4;

/// The longest earlier turn Jev reads, in characters.
pub const TURN_CHARS: usize = 400;

/// The variable that picks the embedding provider: unset for
/// [`Embedder::house`] (Vertex AI on a Google credential, else OpenAI,
/// else OpenRouter), `vertex`, or
/// `gateway` (the Vercel AI Gateway, the chat worker's own door).
pub const EMBEDDINGS_VAR: &str = "OPENAGENTS_PRODUCT_KB_EMBEDDINGS";

/// The embedder [`EMBEDDINGS_VAR`] picks.
///
/// # Errors
///
/// An unknown choice, or the chosen embedder can't be set up.
pub fn embedder_from_env() -> Result<Embedder, String> {
    let choice = std::env::var(EMBEDDINGS_VAR)
        .ok()
        .filter(|c| !c.trim().is_empty());
    Embedder::chosen(choice.as_deref())
}

/// How long the Jev request may take. The router bounds the whole lookup
/// by [`crate::router::seams::KB_BUDGET`]; this leaves the embedding call
/// room in front of it.
pub const JUDGE_BUDGET: Duration = Duration::from_millis(1_500);

/// A typed judge: Jev behind [`jev::Client`], or a stand-in in tests.
pub trait Judge: Send + Sync {
    /// Answers one System One request.
    fn judge(
        &self,
        request: jev::SystemOneRequest,
    ) -> BoxFuture<'_, Result<jev::SystemOneResponse, String>>;
}

impl Judge for jev::Client {
    fn judge(
        &self,
        request: jev::SystemOneRequest,
    ) -> BoxFuture<'_, Result<jev::SystemOneResponse, String>> {
        Box::pin(async move { self.system_one(request).await.map_err(|e| e.to_string()) })
    }
}

/// The entries' vectors, in corpus order, and the model that made them.
struct Index {
    model: String,
    vectors: Vec<Vec<f32>>,
}

/// The product knowledge base: a corpus, an embedder, and a judge.
pub struct ProductKnowledge<E: Embed = Embedder> {
    corpus: Arc<Corpus>,
    embedder: E,
    /// The embedding provider, named for a person, for the privacy answer.
    recipient: String,
    judge: Arc<dyn Judge>,
    /// The entries' vectors, shared with every copy made for a job on the
    /// caller's keys ([`ProductKnowledge::lent`]).
    index: Arc<Mutex<Option<Arc<Index>>>>,
    /// Whether this copy may embed the corpus itself; a copy lent to a job
    /// on the caller's keys only reads the vectors we made, so they never
    /// pay to index our corpus.
    builds: bool,
}

/// One candidate and what the lookup found about it.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// The entry id, without its version.
    pub id: String,
    /// Cosine similarity of the message and the entry's search text.
    pub similarity: f64,
    /// Jev's `relevant_N`, or 0 when it did not answer.
    pub relevance: f64,
}

/// Everything a lookup found: the router's [`Grounding`], and the
/// candidates and the `answer` choice behind it, for evaluation.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub grounding: Grounding,
    /// The candidates, nearest first.
    pub candidates: Vec<Candidate>,
    /// The entry the `answer` Choice picked, and its probability; `None`
    /// when it picked `none` or was not asked.
    pub answer: Option<(String, f64)>,
    /// Milliseconds the embedding call took.
    pub embed_ms: u64,
    /// Milliseconds the Jev request took.
    pub judge_ms: u64,
}

/// The provider's name for a person.
#[must_use]
pub fn provider_name(provider: EmbeddingProvider) -> &'static str {
    match provider {
        EmbeddingProvider::Openai => "OpenAI",
        EmbeddingProvider::Openrouter => "OpenRouter",
        EmbeddingProvider::Vertex => "Google Vertex AI",
        EmbeddingProvider::Gateway => "the Vercel AI Gateway",
    }
}

impl ProductKnowledge<Embedder> {
    /// The committed corpus (or `OPENAGENTS_PRODUCT_KNOWLEDGE`), embeddings
    /// from [`embedder_from_env`], and `judge`.
    ///
    /// # Errors
    ///
    /// The corpus does not load or has no entries, or no embedder is set up.
    pub fn from_env(judge: Arc<dyn Judge>) -> Result<Self, String> {
        let dir = product::default_dir();
        let root = product::repository();
        let corpus = Corpus::load(&dir, root.join("knowledge").exists().then_some(&*root))?;
        let embedder = embedder_from_env()?;
        // Named as the codebase seam names the same provider, so the
        // privacy answer lists a service once.
        let recipient = crate::codebase::embedding_recipient(embedder.provider).to_string();
        Ok(ProductKnowledge::new(corpus, embedder, recipient, judge))
    }
}

impl<E: Embed> ProductKnowledge<E> {
    /// A knowledge base over `corpus`. `recipient` names the embedding
    /// provider for the privacy answer.
    pub fn new(
        corpus: Corpus,
        embedder: E,
        recipient: impl Into<String>,
        judge: Arc<dyn Judge>,
    ) -> Self {
        ProductKnowledge {
            corpus: Arc::new(corpus),
            embedder,
            recipient: recipient.into(),
            judge,
            index: Arc::new(Mutex::new(None)),
            builds: true,
        }
    }

    /// The same corpus and vectors for one job on the caller's own keys
    /// (BYOK): the message is embedded with `embedder` and judged by
    /// `judge`, both on their keys. The copy never embeds the corpus: until
    /// our vectors are made, its lookups fail and the router answers past
    /// the seam.
    pub fn lent<F: Embed>(
        &self,
        embedder: F,
        recipient: impl Into<String>,
        judge: Arc<dyn Judge>,
    ) -> ProductKnowledge<F> {
        ProductKnowledge {
            corpus: self.corpus.clone(),
            embedder,
            recipient: recipient.into(),
            judge,
            index: self.index.clone(),
            builds: false,
        }
    }

    /// The corpus.
    #[must_use]
    pub fn corpus(&self) -> &Corpus {
        &self.corpus
    }

    /// The embedding provider, named for a person.
    #[must_use]
    pub fn recipient(&self) -> &str {
        &self.recipient
    }

    /// The entries' vectors, made once.
    async fn index(&self) -> Result<Arc<Index>, SeamError> {
        if let Some(index) = self.index.lock().ok().and_then(|i| i.clone()) {
            return Ok(index);
        }
        if !self.builds {
            return Err(SeamError::Failed(
                "the corpus is not indexed yet".to_string(),
            ));
        }
        let texts: Vec<String> = self
            .corpus
            .base
            .entries
            .iter()
            .map(knowledge::Entry::search_text)
            .collect();
        let (vectors, _) = self
            .embedder
            .embed(texts)
            .await
            .map_err(|e| SeamError::Failed(format!("embedding the corpus failed: {e}")))?;
        if vectors.len() != self.corpus.base.entries.len() {
            return Err(SeamError::Failed(format!(
                "{} vectors came back for {} entries",
                vectors.len(),
                self.corpus.base.entries.len()
            )));
        }
        let index = Arc::new(Index {
            model: self.embedder.model().to_string(),
            vectors,
        });
        if let Ok(mut slot) = self.index.lock() {
            *slot = Some(index.clone());
        }
        Ok(index)
    }

    /// Makes the entries' vectors now, so the first lookup does not wait
    /// for them.
    ///
    /// # Errors
    ///
    /// The embedding call fails.
    pub async fn warm(&self) -> Result<(), SeamError> {
        self.index().await.map(|_| ())
    }

    /// The [`CANDIDATES`] entries nearest `message` by cosine similarity,
    /// nearest first, and the milliseconds the query's embedding took.
    ///
    /// # Errors
    ///
    /// An embedding call fails.
    pub async fn nearest(&self, message: &str) -> Result<(Vec<(usize, f64)>, u64), SeamError> {
        let index = self.index().await?;
        let started = std::time::Instant::now();
        let (mut vectors, _) = self
            .embedder
            .embed(vec![
                message
                    .chars()
                    .take(knowledge::search::QUERY_CHARS)
                    .collect(),
            ])
            .await
            .map_err(|e| SeamError::Failed(format!("embedding the message failed: {e}")))?;
        let embed_ms = started.elapsed().as_millis() as u64;
        if index.model != self.embedder.model() {
            return Err(SeamError::Failed(
                "the index is another model's".to_string(),
            ));
        }
        let query = vectors
            .pop()
            .ok_or_else(|| SeamError::Failed("no vector came back for the message".to_string()))?;
        let mut ranked: Vec<(usize, f64)> = index
            .vectors
            .iter()
            .enumerate()
            .map(|(n, vector)| (n, cosine(vector, &query)))
            .collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked.truncate(CANDIDATES);
        Ok((ranked, embed_ms))
    }

    /// Finds the entries relevant to `lookup`.
    ///
    /// # Errors
    ///
    /// An embedding call or the Jev request fails. The error names no
    /// message text.
    pub async fn find(&self, lookup: &Lookup) -> Result<Found, SeamError> {
        let (nearest, embed_ms) = self.nearest(&lookup.message).await?;
        let entries: Vec<&knowledge::Entry> = nearest
            .iter()
            .map(|(n, _)| &self.corpus.base.entries[*n])
            .collect();
        let request = jev::SystemOneRequest::new(state(lookup, &entries), questions(&entries))
            .retry(RetryPolicy {
                max_retries: 0,
                budget: Some(JUDGE_BUDGET),
                ..RetryPolicy::default()
            })
            .timeout(JUDGE_BUDGET);
        let started = std::time::Instant::now();
        let response = self
            .judge
            .judge(request)
            .await
            .map_err(|e| SeamError::Failed(format!("the relevance judgment failed: {e}")))?;
        let judge_ms = started.elapsed().as_millis() as u64;
        let similarities: Vec<f64> = nearest.iter().map(|(_, s)| *s).collect();
        let mut found = read(&response, &entries, &similarities);
        found.embed_ms = embed_ms;
        found.judge_ms = judge_ms;
        Ok(found)
    }
}

/// The key a candidate has in the state and in its question ids.
fn key(n: usize) -> String {
    format!("entry_{}", n + 1)
}

/// `text` cut to `chars` characters.
fn cut(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}

/// What Jev reads: the latest message, the earlier turns, and the
/// candidates, each with its title, summary, what it covers, and its
/// reviewed answer.
#[must_use]
pub fn state(lookup: &Lookup, entries: &[&knowledge::Entry]) -> Value {
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
                "text": cut(&m.text, TURN_CHARS),
            })
        })
        .collect();
    let candidates: serde_json::Map<String, Value> = entries
        .iter()
        .enumerate()
        .map(|(n, entry)| {
            (
                key(n),
                json!({
                    "title": entry.title,
                    "summary": entry.summary,
                    "covers": entry.applies_when,
                    // The answer as a reader gets it, its components as
                    // Markdown, so Jev judges the whole of it.
                    "answer": served(entry).map(|text| openui_lang::embed::fallback(&text)),
                }),
            )
        })
        .collect();
    json!({
        "product": "OpenAgents, an app for chatting with OpenAgents and commanding your own computers",
        "latest_message": lookup.message,
        "earlier_turns": earlier,
        "entries": candidates,
    })
}

/// The relevance questions for `entries`: a `relevant_N` Noul for each,
/// and an `answer` Choice over the ones with a reviewed answer.
#[must_use]
pub fn questions(entries: &[&knowledge::Entry]) -> Questions {
    let mut questions = Questions::new();
    for n in 0..entries.len() {
        let id = key(n);
        questions = questions.with(
            format!("relevant_{}", n + 1),
            Noul::with_criteria(
                format!(
                    "Would a correct reply to the user's latest message about OpenAgents use facts \
                     that {id} in the state's entries states?"
                ),
                NoulCriteria::new()
                    .when_true(format!(
                        "Yes: {id} documents something the reply needs to answer what the user asked"
                    ))
                    .when_false(format!(
                        "No: {id} is about something else, or only shares words or a topic with the \
                         message"
                    )),
            ),
        );
    }
    let mut options: IndexMap<String, Option<JevEntry>> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.answer.is_some())
        .map(|(n, entry)| {
            (
                key(n),
                Some(JevEntry::from(format!(
                    "The answer of {} (\"{}\") fully answers it",
                    key(n),
                    entry.title
                ))),
            )
        })
        .collect();
    if !options.is_empty() {
        options.insert(
            "none".to_string(),
            Some(JevEntry::from(
                "No single entry's answer fully and correctly answers the message as asked",
            )),
        );
        questions = questions.with(
            "answer",
            Choice::new(
                "Which entry's `answer`, if any, fully and correctly answers the user's latest \
                 message as asked, on its own, with nothing the user asked for missing?",
                options,
            ),
        );
    }
    questions
}

/// Reads a response into what was found: the passages at or above the
/// relevance floor, most relevant first, at most [`KEEP`], each carrying
/// its reviewed answer only when the `answer` Choice picked it at
/// [`KB_ANSWER_CONFIDENCE`] or more.
#[must_use]
pub fn read(
    response: &jev::SystemOneResponse,
    entries: &[&knowledge::Entry],
    similarities: &[f64],
) -> Found {
    let relevance = |n: usize| match response.answers.get(&format!("relevant_{}", n + 1)) {
        Some(Answer::Noul(noul)) if noul.noul.is_finite() => noul.noul.clamp(0.0, 1.0),
        _ => 0.0,
    };
    let answer =
        match response.answers.get("answer") {
            Some(Answer::Choice(choice)) => (0..entries.len())
                .find(|n| key(*n) == choice.choice)
                .map(|n| {
                    let p = choice
                        .probabilities
                        .get(&choice.choice)
                        .copied()
                        .unwrap_or(0.0);
                    (n, if p.is_finite() { p } else { 0.0 })
                }),
            _ => None,
        };
    let candidates: Vec<Candidate> = entries
        .iter()
        .enumerate()
        .map(|(n, entry)| Candidate {
            id: entry.id.clone(),
            similarity: similarities.get(n).copied().unwrap_or(0.0),
            relevance: relevance(n),
        })
        .collect();
    let mut kept: Vec<usize> = (0..entries.len())
        .filter(|n| relevance(*n) >= RELEVANCE_FLOOR)
        .collect();
    kept.sort_by(|a, b| relevance(*b).total_cmp(&relevance(*a)).then(a.cmp(b)));
    kept.truncate(KEEP);
    let passages = kept
        .into_iter()
        .map(|n| {
            let entry = entries[n];
            let reference = Reference::of(entry);
            let chosen = matches!(answer, Some((m, p)) if m == n && p >= KB_ANSWER_CONFIDENCE);
            Passage {
                id: reference.id,
                title: reference.title,
                text: reference.text,
                source: entry.cites.first().cloned().unwrap_or_default(),
                relevance: relevance(n),
                answer: if chosen { served(entry) } else { None },
                off_computer: entry.tags.iter().any(|tag| tag == OFF_COMPUTER_TAG),
                in_app: entry.tags.iter().any(|tag| tag == product::IN_APP_TAG),
            }
        })
        .collect();
    Found {
        grounding: Grounding {
            passages,
            commit: None,
            needs_dispatch: false,
        },
        candidates,
        answer: answer.map(|(n, p)| (entries[n].id.clone(), p)),
        embed_ms: 0,
        judge_ms: 0,
    }
}

/// What the chat shows when `entry`'s reviewed answer is served whole: the
/// answer, followed by its components when it has them (#11187).
#[must_use]
pub fn served(entry: &knowledge::Entry) -> Option<String> {
    let answer = entry.answer.as_deref()?;
    Some(match &entry.ui {
        Some(ui) => crate::router::bank::with_ui(answer, ui),
        None => answer.to_owned(),
    })
}

/// The references a grounded reply is given: the passages' entries.
#[must_use]
pub fn references(grounding: &Grounding) -> Vec<Reference> {
    grounding
        .passages
        .iter()
        .map(|passage| Reference {
            id: passage.id.clone(),
            title: passage.title.clone(),
            text: passage.text.clone(),
            source: passage.source.clone(),
        })
        .collect()
}

/// The grounded model's instructions for `grounding`
/// ([`knowledge::product::instructions`]).
#[must_use]
pub fn instructions(grounding: &Grounding) -> String {
    product::instructions(&references(grounding))
}

/// Which of `grounding`'s entries a grounded reply cited, and which ids it
/// made up ([`knowledge::product::check_reply`]).
#[must_use]
pub fn cited(reply: &str, grounding: &Grounding) -> product::Cited {
    product::check_reply(reply, &references(grounding))
}

/// A tidier that takes a grounded reply's `[openagents.…]` and
/// `[openagents.…@N]` citations out as it streams: the citations are for
/// [`cited`], never for the person reading.
#[must_use]
pub fn tidier() -> crate::router::gym::Tidy {
    crate::router::gym::Tidy::citing(product::PREFIX)
}

/// `reply` with its product citations taken out, as [`tidier`] streams it.
#[must_use]
pub fn tidy(reply: &str) -> String {
    crate::router::gym::tidy_citing(reply, product::PREFIX)
}

impl ProductKb for ProductKnowledge<Embedder> {
    fn available(&self) -> bool {
        !self.corpus.base.entries.is_empty()
    }

    fn recipients(&self) -> Vec<String> {
        vec![self.recipient.clone()]
    }

    fn ground<'a>(&'a self, lookup: &'a Lookup) -> BoxFuture<'a, Result<Grounding, SeamError>> {
        Box::pin(async move { self.find(lookup).await.map(|found| found.grounding) })
    }

    fn on_their_keys(
        &self,
        theirs: &crate::router::seams::TheirKeys,
    ) -> Option<Arc<dyn ProductKb>> {
        let embedder = theirs.embedder()?;
        let recipient = crate::codebase::embedding_recipient(embedder.provider).to_string();
        Some(Arc::new(self.lent(
            embedder,
            recipient,
            theirs.judge.clone(),
        )))
    }
}

#[cfg(test)]
mod eval;
#[cfg(test)]
mod tests;
