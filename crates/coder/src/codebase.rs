//! Codebase knowledge: answers about how OpenAgents is built, grounded on
//! this public repository at a pinned commit, with `path:line` citations.
//!
//! This is the chat router's `codebase.kb` route
//! (`docs/coder/design/2026-09-28-chat-router.md`, "Codebase knowledge").
//! A question goes through four steps:
//!
//! 1. **Retrieve.** The question is embedded and the index built by
//!    [`knowledge::codebase`] returns the [`CANDIDATES`] nearest chunks
//!    by cosine similarity, at most [`PER_PATH`] from one file.
//! 2. **Judge.** One Jev request reads the question and the candidates and
//!    answers independent Nouls: `needs_live` (does this need running code,
//!    a live checkout, or more source than the index holds?) and one
//!    `relevant_N` per candidate. Code keeps the candidates at
//!    [`RELEVANT`] or above, at most [`KEEP`].
//! 3. **Escalate or answer.** At `needs_live` ≥ [`LIVE_FLOOR`], or with
//!    nothing relevant, the reply is [`Reply::Escalate`]: the router turns
//!    it into a `work.dispatch` offer, so Coder reads or runs the code on a
//!    computer. Otherwise the chat model answers from the kept excerpts
//!    alone ([`prompt`]), citing them.
//! 4. **Check.** [`finish`] reads every `path:line` or `path:start-end` the
//!    model wrote and keeps as sources only those inside a kept excerpt's
//!    range, then appends a sources line naming the commit. A citation
//!    outside what the model was shown is reported, never shown as a
//!    source.
//!
//! No step reads the question by keyword: retrieval is embedding
//! similarity, the escalation and relevance decisions are Noul
//! probabilities, and citation parsing runs only on the model's answer
//! after the route is chosen, for a bounded field (a path and line
//! numbers), which `AGENTS.md` allows.
//!
//! Where the index lives and how it is refreshed:
//! `docs/coder/design/codebase-kb.md`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use jev::{Answer, Noul, NoulCriteria, Questions, RetryPolicy, SystemOneResponse};
use knowledge::codebase::{Chunk, Index};
use knowledge::search::Embed;
use serde_json::{Value, json};

use crate::generate::{Generate, Message, Meta, Role};

/// The question set's identity, for evidence.
pub const SET: &str = "codebase-kb-v1";

/// The environment variable naming the index file.
pub const INDEX_VAR: &str = "CODER_CODEBASE_KB";

/// Chunks retrieved for the judge.
pub const CANDIDATES: usize = 10;

/// The most retrieved chunks from one file.
pub const PER_PATH: usize = 2;

/// The least relevance at which a candidate is kept.
pub const RELEVANT: f64 = 0.5;

/// The most excerpts the model reads.
pub const KEEP: usize = 6;

/// The `needs_live` probability at which the question goes to Coder.
pub const LIVE_FLOOR: f64 = 0.6;

/// Characters of an excerpt shown to the judge.
const JUDGE_CHARS: usize = 1_200;

/// How long the judgment may take.
pub const BUDGET: Duration = Duration::from_millis(4_000);

/// The index's default place: `~/.cache/openagents/codebase-kb/codebase-kb.gz`.
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".cache/openagents/codebase-kb")
            .join("codebase-kb.gz")
    })
}

/// The index file [`INDEX_VAR`] names, else [`default_path`].
#[must_use]
pub fn configured_path() -> Option<PathBuf> {
    std::env::var_os(INDEX_VAR)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(default_path)
}

/// The embedder for questions and builds: the AI Gateway with the chat
/// worker's own door key when one is set, so the worker needs no second
/// credential, else OpenAI or OpenRouter. All three serve OpenAI's
/// `text-embedding-3-small`, so an index built through one is read through
/// any.
///
/// # Errors
///
/// None of the three has a key.
pub fn embedder() -> Result<knowledge::search::Embedder, String> {
    match std::env::var(EMBEDDINGS_VAR).ok().as_deref().map(str::trim) {
        Some("vertex") => return knowledge::search::Embedder::vertex(),
        None | Some("" | "gateway") => {}
        Some(other) => {
            return Err(format!(
                "{EMBEDDINGS_VAR} is `vertex` or `gateway`, not `{other}`"
            ));
        }
    }
    knowledge::search::Embedder::gateway().or_else(|gateway| {
        knowledge::search::Embedder::from_env().map_err(|other| format!("{gateway}; {other}"))
    })
}

/// The variable that picks the embedder an index is built with
/// ([`embedder`]): `vertex` for Google's `text-embedding-005` on Vertex AI,
/// unset or `gateway` for OpenAI's `text-embedding-3-small`. Reading an
/// index needs none: [`embedder_for`] takes the model the index was built
/// with.
pub const EMBEDDINGS_VAR: &str = "CODER_CODEBASE_EMBEDDINGS";

/// The embedder that reads questions into an index built by `model`:
/// Vertex AI for a Vertex-built index, else [`embedder`]'s OpenAI model,
/// so a worker never pairs an index with another model's questions.
///
/// # Errors
///
/// The embedder for that model cannot be set up.
pub fn embedder_for(model: &str) -> Result<knowledge::search::Embedder, String> {
    if model.starts_with("vertex/") {
        knowledge::search::Embedder::vertex()
    } else {
        knowledge::search::Embedder::gateway().or_else(|gateway| {
            knowledge::search::Embedder::from_env().map_err(|other| format!("{gateway}; {other}"))
        })
    }
}

/// Why a question goes to Coder instead of being answered here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Escalation {
    /// The judge read the question as needing running code, a live
    /// checkout, or more source than the index holds; its probability.
    NeedsLive(f64),
    /// No retrieved excerpt was relevant.
    NothingRelevant,
}

impl Escalation {
    /// The word evidence carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Escalation::NeedsLive(_) => "needs_live",
            Escalation::NothingRelevant => "nothing_relevant",
        }
    }
}

/// One cited range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Citation {
    pub path: String,
    pub start: u32,
    pub end: u32,
}

impl Citation {
    /// `path:start-end`, or `path:line`.
    #[must_use]
    pub fn cite(&self) -> String {
        if self.start == self.end {
            format!("{}:{}", self.path, self.start)
        } else {
            format!("{}:{}-{}", self.path, self.start, self.end)
        }
    }
}

/// What the codebase route says.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    /// An answer from the index.
    Grounded {
        /// The model's answer with the sources line appended.
        text: String,
        /// Citations inside the excerpts the model was shown.
        citations: Vec<Citation>,
        /// Citations the model wrote that lie outside them.
        unsupported: Vec<Citation>,
        /// The full commit the excerpts were read at.
        commit: String,
    },
    /// The question needs Coder on a computer.
    Escalate { why: Escalation, commit: String },
}

/// One kept excerpt.
#[derive(Clone, Debug, PartialEq)]
pub struct Excerpt {
    pub chunk: Chunk,
    /// Cosine similarity to the question.
    pub similarity: f64,
    /// The judge's probability that it helps.
    pub relevance: f64,
}

/// The judge's reading.
#[derive(Clone, Debug, PartialEq)]
pub struct Judged {
    /// The probability the question needs a computer; 1 when unanswered,
    /// so a missing reading never answers from the index.
    pub needs_live: f64,
    /// Kept excerpts, most relevant first.
    pub kept: Vec<Excerpt>,
}

/// The judge's state: the question and the numbered candidates.
#[must_use]
pub fn state(question: &str, candidates: &[&Chunk]) -> Value {
    let excerpts: Vec<Value> = candidates
        .iter()
        .enumerate()
        .map(|(n, chunk)| {
            json!({
                "n": n + 1,
                "at": chunk.cite(),
                "title": chunk.title,
                "text": chunk.text.chars().take(JUDGE_CHARS).collect::<String>(),
            })
        })
        .collect();
    json!({
        "question": question,
        "repository": "OpenAgentsInc/openagents",
        "excerpts": excerpts,
    })
}

/// The questions: `needs_live` and one `relevant_N` per candidate.
#[must_use]
pub fn questions(candidates: usize) -> Questions {
    let mut questions = Questions::new().with(
        "needs_live",
        Noul::with_criteria(
            "The user asked a question about the OpenAgents software. To answer it well, would \
             someone need to run code, builds, or tests, check the current state of a machine, \
             service, or checkout, or trace through more source code than documentation and \
             doc comments hold?",
            NoulCriteria::new()
                .when_true(
                    "Yes: it needs running something, live state, or reading and tracing a lot \
                     of source",
                )
                .when_false("No: documentation and doc comments of the repository can answer it"),
        ),
    );
    for n in 1..=candidates {
        questions = questions.with(
            format!("relevant_{n}"),
            Noul::with_criteria(
                format!(
                    "Does excerpt {n} contain information that directly helps answer the \
                     user's question?"
                ),
                NoulCriteria::new()
                    .when_true(format!("Yes: excerpt {n} states part of the answer"))
                    .when_false(format!("No: excerpt {n} is about something else")),
            ),
        );
    }
    questions
}

/// The judge request.
#[must_use]
pub fn request(question: &str, candidates: &[&Chunk]) -> jev::SystemOneRequest {
    jev::SystemOneRequest::new(state(question, candidates), questions(candidates.len()))
        .retry(RetryPolicy {
            max_retries: 1,
            budget: Some(BUDGET),
            ..RetryPolicy::default()
        })
        .timeout(BUDGET)
}

fn noul(response: &SystemOneResponse, id: &str) -> Option<f64> {
    match response.answers.get(id) {
        Some(Answer::Noul(answer)) if answer.noul.is_finite() => Some(answer.noul),
        _ => None,
    }
}

/// Reads the judge's answer over `candidates` (chunk and similarity).
#[must_use]
pub fn judged(response: &SystemOneResponse, candidates: &[(&Chunk, f64)]) -> Judged {
    let mut kept: Vec<Excerpt> = candidates
        .iter()
        .enumerate()
        .filter_map(|(i, (chunk, similarity))| {
            let relevance = noul(response, &format!("relevant_{}", i + 1))?;
            (relevance >= RELEVANT).then(|| Excerpt {
                chunk: (*chunk).clone(),
                similarity: *similarity,
                relevance,
            })
        })
        .collect();
    kept.sort_by(|a, b| b.relevance.total_cmp(&a.relevance));
    kept.truncate(KEEP);
    Judged {
        needs_live: noul(response, "needs_live").unwrap_or(1.0),
        kept,
    }
}

/// The model's instructions and input for an answer from `kept`, read at
/// `commit`.
#[must_use]
pub fn prompt(question: &str, kept: &[Excerpt], commit: &str) -> (String, String) {
    let short = &commit[..commit.len().min(10)];
    let instructions = format!(
        "We are OpenAgents: speak as \"we\", never \"I\". Answer the user's question about the \
         OpenAgents repository (github.com/OpenAgentsInc/openagents) using ONLY the excerpts \
         below, read at commit {short}. Cite every fact with the excerpt's location exactly as \
         given, in the form path:start-end (or path:line for a line inside that range). Never \
         cite a location that is not listed. If the excerpts do not answer the question, say \
         what they do cover and that Coder can look further on a connected computer. Keep it \
         under 150 words; plain sentences, no headings."
    );
    let mut input = String::new();
    for (n, excerpt) in kept.iter().enumerate() {
        input.push_str(&format!(
            "[{}] {} ({})\n{}\n\n",
            n + 1,
            excerpt.chunk.cite(),
            excerpt.chunk.title,
            excerpt.chunk.text
        ));
    }
    input.push_str(&format!("Question: {question}"));
    (instructions, input)
}

/// Every `path:line` or `path:start-end` in `text`, where the path ends in
/// `.md` or `.rs`. A bounded-field parse of the model's answer, after the
/// route is chosen.
#[must_use]
pub fn citations(text: &str) -> Vec<Citation> {
    let path_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-');
    let bytes: Vec<char> = text.chars().collect();
    let mut out: Vec<Citation> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != ':' {
            i += 1;
            continue;
        }
        let mut a = i;
        while a > 0 && path_char(bytes[a - 1]) {
            a -= 1;
        }
        let path: String = bytes[a..i].iter().collect();
        let path = path.trim_start_matches(['.', '/']).to_string();
        let mut j = i + 1;
        let digits = |j: &mut usize| {
            let from = *j;
            while *j < bytes.len() && bytes[*j].is_ascii_digit() {
                *j += 1;
            }
            bytes[from..*j]
                .iter()
                .collect::<String>()
                .parse::<u32>()
                .ok()
        };
        let Some(start) = digits(&mut j) else {
            i += 1;
            continue;
        };
        let mut end = start;
        if j < bytes.len() && matches!(bytes[j], '-' | '–') {
            let mut k = j + 1;
            if let Some(e) = digits(&mut k) {
                end = e.max(start);
                j = k;
            }
        }
        if (path.ends_with(".md") || path.ends_with(".rs")) && path.contains(|c: char| c != '.') {
            let citation = Citation { path, start, end };
            if !out.contains(&citation) {
                out.push(citation);
            }
        }
        i = j;
    }
    out
}

/// Whether `citation` lies inside an excerpt the model was shown.
#[must_use]
pub fn supported(citation: &Citation, kept: &[Excerpt]) -> bool {
    kept.iter().any(|e| {
        e.chunk.path == citation.path
            && citation.start >= e.chunk.start
            && citation.end <= e.chunk.end
    })
}

/// The grounded reply: the model's text, its checked citations, and a
/// sources line naming the commit. When the model cited nothing inside
/// the excerpts, the sources are the excerpts themselves.
#[must_use]
pub fn finish(text: &str, kept: &[Excerpt], commit: &str) -> Reply {
    let (citations, unsupported): (Vec<Citation>, Vec<Citation>) = citations(text)
        .into_iter()
        .partition(|c| supported(c, kept));
    let short = &commit[..commit.len().min(10)];
    let listed: Vec<String> = if citations.is_empty() {
        kept.iter().map(|e| e.chunk.cite()).collect()
    } else {
        citations.iter().map(Citation::cite).collect()
    };
    let text = format!(
        "{}\n\nSources, as of commit {short}: {}",
        text.trim(),
        listed.join(", ")
    );
    Reply::Grounded {
        text,
        citations,
        unsupported,
        commit: commit.to_string(),
    }
}

/// Something that writes an answer from instructions and one input.
pub trait Compose: Send + Sync {
    /// The model's whole answer.
    fn compose(
        &self,
        instructions: &str,
        input: &str,
    ) -> impl std::future::Future<Output = Result<String, String>> + Send;
}

/// Any [`Generate`] door, such as the chat worker's, as a [`Compose`].
pub struct Door<'a, G: Generate>(pub &'a G);

impl<G: Generate> Compose for Door<'_, G> {
    async fn compose(&self, instructions: &str, input: &str) -> Result<String, String> {
        let messages = [Message {
            role: Role::User,
            text: input.to_string(),
        }];
        let mut sink = |_: &str| {};
        let mut meta = |_: Meta| {};
        self.0
            .generate(instructions, &messages, &mut sink, &mut meta)
            .await
            .map(|(text, _)| text)
            .map_err(|e| e.to_string())
    }
}

/// A chat model on OpenRouter, as a [`Compose`].
pub struct OpenRouter {
    pub client: openrouter::Client,
    pub model: String,
}

impl Compose for OpenRouter {
    async fn compose(&self, instructions: &str, input: &str) -> Result<String, String> {
        let request = openrouter::ChatRequest::new(
            &self.model,
            vec![
                openrouter::Message::system(instructions),
                openrouter::Message::user(input),
            ],
        )
        .max_tokens(600);
        let reply = self
            .client
            .chat(&request)
            .await
            .map_err(|e| e.to_string())?;
        reply
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .filter(|t| !t.trim().is_empty())
            .ok_or_else(|| "the model returned no text".to_string())
    }
}

/// A chat model on OpenAI's chat completions API, as a [`Compose`].
pub struct OpenAiChat {
    pub http: reqwest::Client,
    pub key: String,
    pub model: String,
}

impl OpenAiChat {
    /// `model` with the key from `OPENAI_API_KEY`, or `None` without one.
    #[must_use]
    pub fn from_env(model: &str) -> Option<Self> {
        let key = std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())?;
        Some(Self {
            http: reqwest::Client::new(),
            key,
            model: model.to_string(),
        })
    }
}

impl Compose for OpenAiChat {
    async fn compose(&self, instructions: &str, input: &str) -> Result<String, String> {
        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": instructions},
                {"role": "user", "content": input},
            ],
            "max_completion_tokens": 600,
        });
        let response = self
            .http
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.key)
            .json(&body)
            .timeout(Duration::from_secs(60))
            .send()
            .await
            .map_err(|e| format!("OpenAI: {e}"))?;
        let status = response.status();
        let value: Value = response.json().await.map_err(|e| format!("OpenAI: {e}"))?;
        if !status.is_success() {
            return Err(format!(
                "OpenAI returned HTTP {status}: {}",
                value["error"]["message"].as_str().unwrap_or("")
            ));
        }
        value["choices"][0]["message"]["content"]
            .as_str()
            .filter(|t| !t.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| "the model returned no text".to_string())
    }
}

/// How long each step took, in milliseconds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timing {
    pub embed_ms: u128,
    pub judge_ms: u128,
    pub compose_ms: u128,
}

/// One answered question with what led to it.
#[derive(Clone, Debug)]
pub struct Answered {
    pub reply: Reply,
    /// Every retrieved candidate's location and similarity, best first.
    pub retrieved: Vec<(String, f64)>,
    pub judged: Judged,
    pub timing: Timing,
}

/// The index and the embedder that reads questions into it.
pub struct Codebase<E: Embed> {
    /// Shared with every copy lent to a job on the caller's keys
    /// ([`Codebase::lent`]).
    pub index: std::sync::Arc<Index>,
    embedder: E,
}

impl<E: Embed> Codebase<E> {
    /// The codebase knowledge over `index`, embedding questions with
    /// `embedder`.
    ///
    /// # Errors
    ///
    /// The embedder is another model than the index's, so their vectors
    /// cannot be compared.
    pub fn new(index: Index, embedder: E) -> Result<Self, String> {
        if embedder.model() != index.model {
            return Err(format!(
                "the index holds {}'s vectors and the embedder is {}",
                index.model,
                embedder.model()
            ));
        }
        Ok(Self {
            index: std::sync::Arc::new(index),
            embedder,
        })
    }

    /// The same index, reading questions with `embedder` (BYOK: an
    /// embedder on the caller's keys, for one job).
    ///
    /// # Errors
    ///
    /// The embedder is another model than the index's.
    pub fn lent<F: Embed>(&self, embedder: F) -> Result<Codebase<F>, String> {
        if embedder.model() != self.index.model {
            return Err(format!(
                "the index holds {}'s vectors and the embedder is {}",
                self.index.model,
                embedder.model()
            ));
        }
        Ok(Codebase {
            index: self.index.clone(),
            embedder,
        })
    }

    /// Reads the index at `path`.
    ///
    /// # Errors
    ///
    /// As [`Index::read`] and [`Codebase::new`].
    pub fn open(path: &Path, embedder: E) -> Result<Self, String> {
        Self::new(Index::read(path)?, embedder)
    }

    /// The nearest chunks to `question`: indexes into the index's chunks
    /// and cosine similarities.
    ///
    /// # Errors
    ///
    /// The embeddings call fails.
    pub async fn retrieve(&self, question: &str) -> Result<Vec<(usize, f64)>, String> {
        let (vectors, _) = self
            .embedder
            .embed(vec![question.chars().take(2_000).collect()])
            .await
            .map_err(|e| format!("embedding the question: {e}"))?;
        let vector = vectors.into_iter().next().ok_or("no question vector")?;
        Ok(self.index.nearest(&vector, CANDIDATES, PER_PATH))
    }

    /// Retrieves and judges `question`: the candidates with their
    /// similarities, the judge's reading, and why it goes to Coder, if it
    /// does.
    ///
    /// # Errors
    ///
    /// Retrieval or the judge fails.
    pub async fn ground(&self, judge: &jev::Client, question: &str) -> Result<Grounded, String> {
        let mut timing = Timing::default();
        let started = Instant::now();
        let nearest = self.retrieve(question).await?;
        timing.embed_ms = started.elapsed().as_millis();
        let candidates: Vec<(&Chunk, f64)> = nearest
            .iter()
            .map(|(i, s)| (&self.index.chunks[*i], *s))
            .collect();
        let retrieved = candidates.iter().map(|(c, s)| (c.cite(), *s)).collect();
        let chunks: Vec<&Chunk> = candidates.iter().map(|(c, _)| *c).collect();
        let started = Instant::now();
        let response = judge
            .system_one(request(question, &chunks))
            .await
            .map_err(|e| format!("the judge: {e}"))?;
        timing.judge_ms = started.elapsed().as_millis();
        let judged = judged(&response, &candidates);
        let escalation = if judged.needs_live >= LIVE_FLOOR {
            Some(Escalation::NeedsLive(judged.needs_live))
        } else if judged.kept.is_empty() {
            Some(Escalation::NothingRelevant)
        } else {
            None
        };
        Ok(Grounded {
            retrieved,
            judged,
            escalation,
            timing,
        })
    }

    /// Answers `question`: retrieve, judge, then escalate or compose and
    /// check.
    ///
    /// # Errors
    ///
    /// Retrieval, the judge, or the model fails. The router falls back to
    /// its model tier on any error.
    pub async fn answer<C: Compose>(
        &self,
        judge: &jev::Client,
        compose: &C,
        question: &str,
    ) -> Result<Answered, String> {
        let Grounded {
            retrieved,
            judged,
            escalation,
            mut timing,
        } = self.ground(judge, question).await?;
        let commit = self.index.commit.clone();
        if let Some(why) = escalation {
            return Ok(Answered {
                reply: Reply::Escalate { why, commit },
                retrieved,
                judged,
                timing,
            });
        }
        let (instructions, input) = prompt(question, &judged.kept, &commit);
        let started = Instant::now();
        let text = compose.compose(&instructions, &input).await?;
        timing.compose_ms = started.elapsed().as_millis();
        Ok(Answered {
            reply: finish(&text, &judged.kept, &commit),
            retrieved,
            judged,
            timing,
        })
    }
}

/// Retrieval and judgment for one question.
#[derive(Clone, Debug)]
pub struct Grounded {
    /// Every retrieved candidate's location and similarity, best first.
    pub retrieved: Vec<(String, f64)>,
    pub judged: Judged,
    /// Why the question goes to Coder, when it does.
    pub escalation: Option<Escalation>,
    pub timing: Timing,
}

/// The name a person reads for where question text goes to be embedded.
#[must_use]
pub fn embedding_recipient(provider: knowledge::search::EmbeddingProvider) -> &'static str {
    use knowledge::search::EmbeddingProvider;
    match provider {
        EmbeddingProvider::Gateway => "the Vercel AI Gateway (OpenAI embeddings)",
        EmbeddingProvider::Openai => "OpenAI (embeddings)",
        EmbeddingProvider::Openrouter => "OpenRouter (OpenAI embeddings)",
        EmbeddingProvider::Vertex => "Google Vertex AI (embeddings)",
    }
}

/// The router's [`CodebaseKb`](crate::router::seams::CodebaseKb) seam: the
/// index, the embedder, and the judge.
///
/// A lookup's latest message (already redacted and bounded by the router)
/// is embedded and judged; the kept excerpts come back as passages whose
/// `id` and `source` are their `path:start-end` at the index's commit, so a
/// reply that cites a passage cites a line range. The grounding asks for
/// dispatch when the judge reads the question as needing a computer or no
/// excerpt is relevant.
pub struct Seam {
    kb: Codebase<knowledge::search::Embedder>,
    judge: std::sync::Arc<jev::Client>,
}

impl Seam {
    /// The seam over `kb`, judged by `judge`.
    #[must_use]
    pub fn new(
        kb: Codebase<knowledge::search::Embedder>,
        judge: std::sync::Arc<jev::Client>,
    ) -> Self {
        Self { kb, judge }
    }

    /// The seam from the environment: the index at [`configured_path`],
    /// [`embedder`], and `judge`.
    ///
    /// # Errors
    ///
    /// The reason the seam is not available.
    pub fn from_env(judge: std::sync::Arc<jev::Client>) -> Result<Self, String> {
        let path = configured_path().ok_or("no HOME and no CODER_CODEBASE_KB")?;
        let index = Index::read(&path)?;
        let embedder = embedder_for(&index.model)?;
        let kb = Codebase::new(index, embedder)?;
        Ok(Self::new(kb, judge))
    }

    /// The commit the index was read at.
    #[must_use]
    pub fn commit(&self) -> &str {
        &self.kb.index.commit
    }
}

/// The seam the worker holds: [`Seam`] when an index is there and a judge
/// is configured, else the no-op. With [`INDEX_VAR`] set, an index that
/// does not open is an error, so a misconfigured worker stops instead of
/// silently answering without it; without it, a missing default index is
/// the no-op.
///
/// # Errors
///
/// [`INDEX_VAR`] names an index that cannot be opened, or no embedder has
/// a key.
pub fn seam_from_env(
    judge: Option<std::sync::Arc<jev::Client>>,
) -> Result<std::sync::Arc<dyn crate::router::seams::CodebaseKb>, String> {
    let named = std::env::var_os(INDEX_VAR).is_some_and(|v| !v.is_empty());
    let present = configured_path().is_some_and(|path| path.exists());
    match judge {
        Some(judge) if named || present => Ok(std::sync::Arc::new(
            Seam::from_env(judge).map_err(|e| format!("the codebase index: {e}"))?,
        )),
        _ => Ok(std::sync::Arc::new(crate::router::seams::NoKb)),
    }
}

/// The passages a judged question grounds on, as the router reads them.
#[must_use]
pub fn passages(judged: &Judged) -> Vec<crate::router::seams::Passage> {
    judged
        .kept
        .iter()
        .map(|excerpt| crate::router::seams::Passage {
            id: excerpt.chunk.cite(),
            title: excerpt.chunk.title.clone(),
            text: excerpt.chunk.text.clone(),
            source: excerpt.chunk.cite(),
            relevance: excerpt.relevance,
            answer: None,
            off_computer: false,
            in_app: false,
        })
        .collect()
}

impl crate::router::seams::CodebaseKb for Seam {
    fn available(&self) -> bool {
        true
    }

    fn warm(&self) -> futures_util::future::BoxFuture<'_, ()> {
        Box::pin(async move {
            let _ = self.kb.embedder.embed(vec!["warm".to_string()]).await;
        })
    }

    fn recipients(&self) -> Vec<String> {
        vec![embedding_recipient(self.kb.embedder.provider).to_string()]
    }

    fn ground<'a>(
        &'a self,
        lookup: &'a crate::router::seams::Lookup,
    ) -> futures_util::future::BoxFuture<
        'a,
        Result<crate::router::seams::Grounding, crate::router::seams::SeamError>,
    > {
        Box::pin(async move {
            let grounded = self
                .kb
                .ground(&self.judge, &lookup.message)
                .await
                .map_err(crate::router::seams::SeamError::Failed)?;
            Ok(crate::router::seams::Grounding {
                passages: passages(&grounded.judged),
                commit: Some(self.kb.index.commit.clone()),
                needs_dispatch: grounded.escalation.is_some(),
            })
        })
    }

    fn on_their_keys(
        &self,
        theirs: &crate::router::seams::TheirKeys,
    ) -> Option<std::sync::Arc<dyn crate::router::seams::CodebaseKb>> {
        let kb = self.kb.lent(theirs.embedder()?).ok()?;
        Some(std::sync::Arc::new(Seam::new(kb, theirs.judge.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use knowledge::codebase::Source;

    fn chunk(path: &str, start: u32, end: u32) -> Chunk {
        Chunk {
            path: path.to_string(),
            start,
            end,
            source: Source::Doc,
            title: "T".to_string(),
            text: format!("text of {path}"),
        }
    }

    fn excerpt(path: &str, start: u32, end: u32) -> Excerpt {
        Excerpt {
            chunk: chunk(path, start, end),
            similarity: 0.5,
            relevance: 0.9,
        }
    }

    fn response(values: &[(&str, f64)]) -> SystemOneResponse {
        let answers: serde_json::Map<String, Value> = values
            .iter()
            .map(|(id, p)| ((*id).to_string(), json!({"type": "noul", "noul": p})))
            .collect();
        SystemOneResponse::decode(jev::RawResponse {
            status: 200,
            headers: Default::default(),
            bytes: json!({"model": "jev", "answers": answers})
                .to_string()
                .into_bytes(),
        })
        .expect("a readable response")
    }

    #[test]
    fn citations_are_read_from_prose_and_brackets() {
        let text = "The quota is in crates/coder/src/relay/quota.rs:12-30 (see also \
                    [docs/deployment/chat-worker.md:4]). Not a citation: http://x:80 or a.rs.";
        assert_eq!(
            citations(text),
            [
                Citation {
                    path: "crates/coder/src/relay/quota.rs".to_string(),
                    start: 12,
                    end: 30
                },
                Citation {
                    path: "docs/deployment/chat-worker.md".to_string(),
                    start: 4,
                    end: 4
                },
            ]
        );
    }

    #[test]
    fn only_citations_inside_the_shown_excerpts_become_sources() {
        let kept = [excerpt("docs/a.md", 10, 20)];
        let reply = finish(
            "We do this in docs/a.md:12-14 and docs/a.md:30, and docs/b.md:1.",
            &kept,
            "0123456789abcdef",
        );
        let Reply::Grounded {
            text,
            citations,
            unsupported,
            commit,
        } = reply
        else {
            panic!("grounded");
        };
        assert_eq!(citations.len(), 1);
        assert_eq!(unsupported.len(), 2);
        assert!(text.ends_with("Sources, as of commit 0123456789: docs/a.md:12-14"));
        assert_eq!(commit, "0123456789abcdef");
    }

    #[test]
    fn an_answer_with_no_valid_citation_lists_its_excerpts() {
        let kept = [
            excerpt("docs/a.md", 10, 20),
            excerpt("crates/x/src/lib.rs", 1, 9),
        ];
        let Reply::Grounded { text, .. } = finish("We do.", &kept, "abc") else {
            panic!("grounded");
        };
        assert!(
            text.ends_with("Sources, as of commit abc: docs/a.md:10-20, crates/x/src/lib.rs:1-9")
        );
    }

    #[test]
    fn the_judge_keeps_relevant_excerpts_and_a_missing_reading_escalates() {
        let a = chunk("docs/a.md", 1, 5);
        let b = chunk("docs/b.md", 1, 5);
        let candidates = [(&a, 0.6), (&b, 0.5)];
        let read = judged(
            &response(&[
                ("needs_live", 0.1),
                ("relevant_1", 0.3),
                ("relevant_2", 0.8),
            ]),
            &candidates,
        );
        assert_eq!(read.needs_live, 0.1);
        assert_eq!(read.kept.len(), 1);
        assert_eq!(read.kept[0].chunk.path, "docs/b.md");
        let silent = judged(&response(&[]), &candidates);
        assert_eq!(silent.needs_live, 1.0);
        assert!(silent.kept.is_empty());
    }

    #[test]
    fn the_set_asks_one_noul_per_candidate_and_the_prompt_lists_them() {
        let asked = questions(3);
        let ids: Vec<&str> = asked.iter().map(|(id, _)| id).collect();
        assert_eq!(
            ids,
            ["needs_live", "relevant_1", "relevant_2", "relevant_3"]
        );
        let (instructions, input) =
            prompt("where?", &[excerpt("docs/a.md", 3, 4)], "0123456789abcdef");
        assert!(instructions.contains("0123456789"));
        assert!(!instructions.contains(" I "));
        assert!(input.starts_with("[1] docs/a.md:3-4 (T)"));
        assert!(input.ends_with("Question: where?"));
    }

    #[test]
    fn passages_are_named_by_their_line_range_at_the_commit() {
        let judged = Judged {
            needs_live: 0.1,
            kept: vec![excerpt("crates/coder/src/relay/quota.rs", 1, 21)],
        };
        let passages = passages(&judged);
        assert_eq!(passages.len(), 1);
        assert_eq!(passages[0].id, "crates/coder/src/relay/quota.rs:1-21");
        assert_eq!(passages[0].source, passages[0].id);
        assert_eq!(passages[0].relevance, 0.9);
        assert!(passages[0].answer.is_none());
    }
}
