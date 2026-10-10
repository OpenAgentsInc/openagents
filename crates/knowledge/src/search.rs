//! Finding the entries that bear on a query: BM25 over each entry's
//! [`Entry::search_text`], combined with cosine similarity over embeddings
//! when an embedder is available.
//!
//! BM25 uses the fixed transform `s / (1 + s)` and is averaged with raw cosine
//! similarity. Semantic results must clear an absolute relevance floor;
//! being the nearest entry in an unrelated corpus is not enough. When there's
//! no embedder, or the embeddings call fails, the ranking is BM25 alone, and
//! the result says why.
//!
//! [`Embedder`] reaches `text-embedding-3-small` on OpenAI's API when an
//! OpenAI key is set up (`OPENAI_API_KEY` or `~/.openagents/openai.json`),
//! and otherwise through OpenRouter. Both give that model's vectors, so they
//! share the cache under the one model name.
//!
//! [`Embedder::vertex`] reaches Google's `text-embedding-005` on Vertex AI
//! instead. It is used only when asked for (`--embeddings vertex` on `kb`,
//! `--kb-embeddings vertex` on `microcoder`), and its vectors are cached
//! under [`vertex::CACHE_MODEL`], apart from any other model's. A query is
//! compared only with entry vectors from the model that embedded it; see
//! [`rank`].

pub mod vertex;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;

use crate::{Base, Entry, digest};

/// BM25's term-frequency saturation.
const K1: f64 = 1.2;
/// BM25's length normalization.
const B: f64 = 0.75;

/// Minimum raw cosine for semantic search. Corpus-relative normalization
/// makes even an unrelated nearest neighbor score 1. Keep admission on the
/// absolute scale instead; the corpus regression tests cover weak neighbors
/// and strong semantic-only matches separately.
pub const MIN_SEMANTIC_SIMILARITY: f64 = 0.4;

/// Characters of a query that are embedded, at most.
pub const QUERY_CHARS: usize = 12_000;

/// Lowercase alphanumeric words of two or more characters.
#[must_use]
pub fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 2)
        .map(str::to_lowercase)
        .collect()
}

/// Each entry's BM25 score for `query`, in base order. A query word counts
/// once however often it appears, so a long query isn't ruled by one
/// repeated word.
#[must_use]
pub fn bm25(entries: &[Entry], query: &str) -> Vec<f64> {
    let texts: Vec<String> = entries.iter().map(Entry::search_text).collect();
    bm25_texts(&texts, query)
}

/// [`bm25`] over plain texts, such as a plugin catalog's listings, in
/// their order.
#[must_use]
pub fn bm25_texts(texts: &[String], query: &str) -> Vec<f64> {
    let documents: Vec<Vec<String>> = texts.iter().map(|text| words(text)).collect();
    let count = documents.len() as f64;
    if documents.is_empty() {
        return Vec::new();
    }
    let average = documents.iter().map(Vec::len).sum::<usize>() as f64 / count;
    // Ignore grammatical words, while retaining technical names and commands.
    // This list applies only to query terms; documents retain their full text.
    let terms: HashSet<String> = words(query)
        .into_iter()
        .filter(|word| !is_stopword(word))
        .collect();
    let mut frequency: HashMap<&str, usize> = HashMap::new();
    for document in &documents {
        let unique: HashSet<&str> = document.iter().map(String::as_str).collect();
        for word in unique {
            *frequency.entry(word).or_default() += 1;
        }
    }
    documents
        .iter()
        .map(|document| {
            let length = document.len() as f64;
            terms
                .iter()
                .map(|term| {
                    let in_doc = document.iter().filter(|w| *w == term).count() as f64;
                    if in_doc == 0.0 {
                        return 0.0;
                    }
                    let n = frequency.get(term.as_str()).copied().unwrap_or(0) as f64;
                    let idf = (1.0 + (count - n + 0.5) / (n + 0.5)).ln();
                    idf * in_doc * (K1 + 1.0)
                        / (in_doc + K1 * (1.0 - B + B * length / average.max(1.0)))
                })
                .sum()
        })
        .collect()
}

/// Common English function words that don't establish query relevance.
fn is_stopword(word: &str) -> bool {
    matches!(
        word,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "been"
            | "being"
            | "but"
            | "by"
            | "can"
            | "could"
            | "did"
            | "do"
            | "does"
            | "for"
            | "from"
            | "had"
            | "has"
            | "have"
            | "how"
            | "i"
            | "if"
            | "in"
            | "into"
            | "is"
            | "it"
            | "its"
            | "me"
            | "my"
            | "of"
            | "on"
            | "or"
            | "our"
            | "should"
            | "so"
            | "than"
            | "that"
            | "the"
            | "their"
            | "them"
            | "there"
            | "these"
            | "they"
            | "this"
            | "those"
            | "to"
            | "was"
            | "we"
            | "were"
            | "what"
            | "when"
            | "where"
            | "which"
            | "who"
            | "why"
            | "will"
            | "with"
            | "would"
            | "you"
            | "your"
    )
}

/// The cosine similarity of two vectors, or 0 when either is all zeros.
#[must_use]
pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum();
    let norm = |v: &[f32]| v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
    let (na, nb) = (norm(a), norm(b));
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// Why an embeddings call failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbedError {
    pub message: String,
    /// Whether the provider refused the request with an error status, so
    /// it did no work and cost nothing. Any other failure, such as a broken
    /// connection, may have been billed.
    pub refused: bool,
}

impl From<String> for EmbedError {
    fn from(message: String) -> Self {
        EmbedError {
            message,
            refused: false,
        }
    }
}

impl From<&str> for EmbedError {
    fn from(message: &str) -> Self {
        EmbedError::from(message.to_string())
    }
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Turns text into vectors.
pub trait Embed {
    /// The model's name, which keys the cache.
    fn model(&self) -> &str;

    /// One vector per input, in order, and the call's cost in dollars, or
    /// `None` when the cost is unknown.
    fn embed(
        &self,
        inputs: Vec<String>,
    ) -> impl std::future::Future<Output = Result<(Vec<Vec<f32>>, Option<f64>), EmbedError>>;

    /// One vector per document, the query's vector when there's a query,
    /// and the cost, as [`Embed::embed`] gives them. A model that embeds
    /// queries apart from documents overrides this; by default the query
    /// is embedded last in the same call.
    fn embed_with_query(
        &self,
        documents: Vec<String>,
        query: Option<String>,
    ) -> impl std::future::Future<Output = Result<Embeddings, EmbedError>> {
        async move {
            let asked = query.is_some();
            let mut inputs = documents;
            inputs.extend(query);
            let (mut vectors, usd) = self.embed(inputs).await?;
            let query = if asked { vectors.pop() } else { None };
            Ok((vectors, query, usd))
        }
    }
}

/// The documents' vectors, the query's vector, and the cost in dollars or
/// `None` when that's unknown.
pub type Embeddings = (Vec<Vec<f32>>, Option<Vec<f32>>, Option<f64>);

/// OpenAI's API, for embeddings without OpenRouter.
pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

/// The Vercel AI Gateway's OpenAI-compatible API.
pub const GATEWAY_BASE_URL: &str = "https://ai-gateway.vercel.sh/v1";

/// The variables that hold an AI Gateway key, in the order they are read:
/// the gateway's own variable, then the chat worker's door key names.
/// `CODER_DOOR_KEY` is read only while `CODER_DOOR_URL` is unset or names
/// the gateway: a worker whose door is OpenRouter holds an OpenRouter key
/// there, which the gateway refuses with HTTP 401 ([`gateway_key`]).
pub const GATEWAY_KEY_VARS: [&str; 3] = [
    "AI_GATEWAY_API_KEY",
    "CODER_AI_GATEWAY_KEY",
    "CODER_DOOR_KEY",
];

/// The AI Gateway key [`GATEWAY_KEY_VARS`] holds, if any.
fn gateway_key() -> Option<String> {
    let door_is_gateway = std::env::var("CODER_DOOR_URL")
        .map(|url| url.trim().is_empty() || url.contains("ai-gateway.vercel.sh"))
        .unwrap_or(true);
    GATEWAY_KEY_VARS
        .iter()
        .filter(|name| **name != "CODER_DOOR_KEY" || door_is_gateway)
        .find_map(|name| std::env::var(name).ok().filter(|k| !k.trim().is_empty()))
}

/// The environment variable that holds an OpenAI API key.
pub const OPENAI_KEY_VAR: &str = "OPENAI_API_KEY";

/// `text-embedding-3-small`'s list price in dollars per million input
/// tokens, from OpenAI's pricing page (retrieved 2026-09-26).
pub const EMBEDDING_USD_PER_MILLION: f64 = 0.02;

/// `~/.openagents/openai.json`, which holds `{"api_key": "..."}` and must
/// be readable by its owner only.
#[must_use]
pub fn openai_key_file() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openagents/openai.json"))
}

/// Where embeddings come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingProvider {
    /// OpenAI's API directly, with an OpenAI key.
    Openai,
    /// OpenRouter, which forwards the same model to OpenAI.
    Openrouter,
    /// Vertex AI, with Google's `text-embedding-005`. Opt-in only.
    Vertex,
    /// The Vercel AI Gateway, which forwards the same OpenAI model; the
    /// chat worker's own door key reaches it.
    Gateway,
}

impl EmbeddingProvider {
    /// The provider's name in records.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            EmbeddingProvider::Openai => "openai",
            EmbeddingProvider::Openrouter => "openrouter",
            EmbeddingProvider::Vertex => "vertex",
            EmbeddingProvider::Gateway => "gateway",
        }
    }
}

impl std::fmt::Display for EmbeddingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Embeddings through an OpenAI-compatible endpoint (OpenAI's own API, or
/// OpenRouter) or through Vertex AI. OpenAI and OpenRouter both serve
/// OpenAI's `text-embedding-3-small`, so their vectors are the same model's
/// and share one cache key, the OpenRouter slug
/// [`openrouter::EMBEDDING_MODEL`]. Vertex AI's are another model's, keyed
/// by [`vertex::CACHE_MODEL`].
pub struct Embedder {
    pub provider: EmbeddingProvider,
    /// The model's name as the cache keys it and records name it.
    pub model: String,
    transport: Transport,
    /// Embedders of the same model on other providers, asked in order when
    /// this one fails ([`Embedder::with_backups`]): an embedding call never
    /// fails while one provider can answer it.
    backups: Vec<Embedder>,
}

/// How an [`Embedder`] reaches its model.
enum Transport {
    /// OpenAI's embeddings API, on OpenAI or OpenRouter.
    Compatible(openrouter::Client),
    Vertex(vertex::Vertex),
}

/// The embedding providers `--embeddings` and `--kb-embeddings` accept.
pub const EMBEDDINGS_CHOICES: &str = "vertex or gateway";

/// An OpenAI key from `OPENAI_API_KEY`, or else from `api_key` in
/// `~/.openagents/openai.json`, which is refused when its group or others
/// can read it.
fn openai_key() -> Result<String, String> {
    if let Ok(key) = std::env::var(OPENAI_KEY_VAR)
        && !key.trim().is_empty()
    {
        return Ok(key);
    }
    let path = openai_key_file().ok_or("no HOME to find ~/.openagents/openai.json in")?;
    key_from_file(&path)
}

/// `api_key` from a JSON key file readable by its owner only.
///
/// # Errors
///
/// No file, a file others can read, or no key in it.
pub fn key_from_file(path: &std::path::Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|_| {
        format!(
            "no OpenAI key: set {OPENAI_KEY_VAR}; or run openagents settings provider-key set openrouter"
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "{} is readable by others (mode {:o}); chmod 600 it",
                path.display(),
                mode & 0o777
            ));
        }
    }
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v["api_key"].as_str().map(str::to_string))
        .filter(|k| !k.trim().is_empty())
        .ok_or(format!("{} has no api_key", path.display()))
}

impl Embedder {
    /// An embedder for `text-embedding-3-small` on `client`, which reaches
    /// OpenAI or OpenRouter as `provider` says.
    #[must_use]
    pub fn compatible(client: openrouter::Client, provider: EmbeddingProvider) -> Self {
        Embedder {
            provider,
            model: openrouter::EMBEDDING_MODEL.to_string(),
            transport: Transport::Compatible(client),
            backups: Vec::new(),
        }
    }

    /// An embedder for `text-embedding-005` on Vertex AI through `client`.
    #[must_use]
    pub fn with_vertex(client: vertex::Vertex) -> Self {
        Embedder {
            provider: EmbeddingProvider::Vertex,
            model: client.cache_model().to_string(),
            transport: Transport::Vertex(client),
            backups: Vec::new(),
        }
    }

    /// An embedder on Vertex AI, configured as [`vertex::Vertex::from_env`]
    /// says.
    ///
    /// # Errors
    ///
    /// No project is named.
    pub fn vertex() -> Result<Self, String> {
        Ok(Embedder::with_vertex(vertex::Vertex::from_env()?))
    }

    /// An embedder on Vertex AI running `model` ([`vertex::MODEL`] or
    /// [`vertex::GEMINI_MODEL`]) whatever `KB_VERTEX_MODEL` says, so an
    /// index built with one model is read with that model.
    ///
    /// # Errors
    ///
    /// No project is named.
    pub fn vertex_on(model: &str) -> Result<Self, String> {
        Ok(Embedder::with_vertex(vertex::Vertex::from_env()?.on(model)))
    }

    /// The embedder a command asked for: `None` for the default
    /// ([`Embedder::from_env`]), `vertex`, or `gateway` (the Vercel AI
    /// Gateway with the chat worker's own door key).
    ///
    /// # Errors
    ///
    /// Another name, or the chosen embedder can't be set up.
    pub fn chosen(choice: Option<&str>) -> Result<Self, String> {
        if let Some(theirs) = Embedder::theirs(&model_access::current()) {
            return theirs;
        }
        match choice {
            None => Embedder::from_env(),
            Some("vertex") => Embedder::vertex(),
            Some("gateway") => Embedder::gateway(),
            Some(other) => Err(format!(
                "the embeddings provider can be {EMBEDDINGS_CHOICES}, not {other}"
            )),
        }
    }

    /// The embedder on the person's own key (BYOK, `model_access`), when
    /// `access` runs model calls on their keys: the same
    /// `text-embedding-3-small` on OpenRouter, else the Vercel AI Gateway,
    /// so caches stay valid. `None` under `ours`; under `mine` with no key
    /// that embeds, the one plain line, never a key of ours.
    #[must_use]
    pub fn theirs(access: &model_access::Access) -> Option<Result<Self, String>> {
        let doors = match access.chat(model_access::Use::Embeddings) {
            Ok(model_access::Doors::Ours) => return None,
            Ok(model_access::Doors::Theirs(doors)) => doors,
            Err(no_door) => return Some(Err(no_door.to_string())),
        };
        let door = doors.into_iter().next()?;
        let provider = match door.provider {
            model_access::Provider::Vercel => EmbeddingProvider::Gateway,
            _ => EmbeddingProvider::Openrouter,
        };
        let mut config = openrouter::Config::new(openrouter::ApiKey::new(door.key.expose()))
            .base_url(door.base_url);
        if provider == EmbeddingProvider::Gateway {
            config.title = None;
        }
        Some(
            openrouter::Client::new(config)
                .map(|client| Embedder::compatible(client, provider))
                .map_err(|e| e.to_string()),
        )
    }

    /// An embedder on OpenAI's API, with the key from `OPENAI_API_KEY` or
    /// `~/.openagents/openai.json`.
    ///
    /// # Errors
    ///
    /// No key, a key file others can read, or the HTTP client can't start.
    pub fn openai() -> Result<Self, String> {
        let mut config = openrouter::Config::new(openrouter::ApiKey::new(&openai_key()?))
            .base_url(OPENAI_BASE_URL);
        config.title = None;
        Ok(Embedder::compatible(
            openrouter::Client::new(config).map_err(|e| e.to_string())?,
            EmbeddingProvider::Openai,
        ))
    }

    /// An embedder on OpenRouter, with the key from `OPENROUTER_API_KEY` or
    /// `~/.openagents/openrouter.json`.
    ///
    /// # Errors
    ///
    /// No key, or the HTTP client can't start.
    pub fn openrouter() -> Result<Self, String> {
        let config = openrouter::Config::from_env().map_err(|e| e.to_string())?;
        Ok(Embedder::compatible(
            openrouter::Client::new(config).map_err(|e| e.to_string())?,
            EmbeddingProvider::Openrouter,
        ))
    }

    /// An embedder on the Vercel AI Gateway, with the key from
    /// [`GATEWAY_KEY_VARS`], and our other embedding providers behind it
    /// ([`Embedder::with_our_backups`]).
    ///
    /// # Errors
    ///
    /// No key, or the HTTP client can't start.
    pub fn gateway() -> Result<Self, String> {
        if let Some(theirs) = Embedder::theirs(&model_access::current()) {
            return theirs;
        }
        Embedder::gateway_alone().map(Embedder::with_our_backups)
    }

    /// The Vercel AI Gateway embedder with no backups.
    fn gateway_alone() -> Result<Self, String> {
        let key = gateway_key()
            .ok_or_else(|| format!("no AI Gateway key: set {}", GATEWAY_KEY_VARS.join(" or ")))?;
        let mut config =
            openrouter::Config::new(openrouter::ApiKey::new(&key)).base_url(GATEWAY_BASE_URL);
        config.title = None;
        Ok(Embedder::compatible(
            openrouter::Client::new(config).map_err(|e| e.to_string())?,
            EmbeddingProvider::Gateway,
        ))
    }

    /// OpenAI's API when an OpenAI key is set up, else OpenRouter, with our
    /// other embedding providers behind it ([`Embedder::with_our_backups`]).
    ///
    /// # Errors
    ///
    /// Neither has a key; the message gives both reasons.
    pub fn from_env() -> Result<Self, String> {
        if let Some(theirs) = Embedder::theirs(&model_access::current()) {
            return theirs;
        }
        Embedder::openai()
            .or_else(|openai| {
                Embedder::openrouter().map_err(|openrouter| {
                    let hint = "openagents settings provider-key set openrouter";
                    // Both providers can recommend the same setup command.
                    let openai = openai.replace(&format!("; or run {hint}"), "");
                    format!("{openai}; {openrouter}")
                })
            })
            .map(Embedder::with_our_backups)
    }

    /// The same embedder with `backups` asked in order when it fails. A
    /// backup that embeds with another model is left out: its vectors
    /// could not be compared with the cache's.
    #[must_use]
    pub fn with_backups(mut self, backups: Vec<Embedder>) -> Self {
        let model = self.model.clone();
        self.backups
            .extend(backups.into_iter().filter(|backup| backup.model == model));
        self
    }

    /// The same embedder with every other provider of ours that has a key
    /// here behind it, in the order OpenRouter, the Vercel AI Gateway,
    /// OpenAI. Never on a person's own keys: [`Embedder::theirs`] answers
    /// before any caller reaches this.
    #[must_use]
    pub fn with_our_backups(self) -> Self {
        let provider = self.provider;
        let backups = [
            Embedder::openrouter(),
            Embedder::gateway_alone(),
            Embedder::openai(),
        ]
        .into_iter()
        .filter_map(Result::ok)
        .filter(|backup| backup.provider != provider)
        .collect();
        self.with_backups(backups)
    }

    /// The providers this embedder asks, in order, named for a log line.
    #[must_use]
    pub fn chain(&self) -> Vec<EmbeddingProvider> {
        std::iter::once(self.provider)
            .chain(self.backups.iter().map(|backup| backup.provider))
            .collect()
    }

    /// How this embedder's costs are reached: OpenAI reports tokens and
    /// Vertex AI billable characters, which are priced at list price;
    /// OpenRouter reports what it billed.
    #[must_use]
    pub fn basis(&self) -> &'static str {
        match self.provider {
            EmbeddingProvider::Openai | EmbeddingProvider::Vertex | EmbeddingProvider::Gateway => {
                "list_price"
            }
            EmbeddingProvider::Openrouter => "billed",
        }
    }
}

impl Embedder {
    /// One embedding call on this embedder's own provider, no backups.
    async fn embed_here(
        &self,
        inputs: Vec<String>,
    ) -> Result<(Vec<Vec<f32>>, Option<f64>), EmbedError> {
        let client = match &self.transport {
            Transport::Compatible(client) => client,
            Transport::Vertex(vertex) => {
                let typed: Vec<(String, vertex::Task)> = inputs
                    .into_iter()
                    .map(|text| (text, vertex::Task::Document))
                    .collect();
                return vertex.embed(&typed).await;
            }
        };
        let wire = match self.provider {
            EmbeddingProvider::Openai => self.model.rsplit('/').next().unwrap_or(&self.model),
            EmbeddingProvider::Openrouter
            | EmbeddingProvider::Vertex
            | EmbeddingProvider::Gateway => &self.model,
        };
        let request = openrouter::EmbeddingRequest::new(wire, inputs);
        let reply = client.embeddings(&request).await.map_err(|e| EmbedError {
            refused: matches!(e, openrouter::Error::Api { .. } | openrouter::Error::NoKey),
            message: match self.provider {
                // The client is OpenRouter's, which names itself in errors.
                EmbeddingProvider::Openai => e.to_string().replace("OpenRouter", "OpenAI"),
                EmbeddingProvider::Gateway => e.to_string().replace("OpenRouter", "the AI Gateway"),
                EmbeddingProvider::Openrouter | EmbeddingProvider::Vertex => e.to_string(),
            },
        })?;
        let tokens = reply.usage.prompt_tokens.max(reply.usage.total_tokens);
        let list = (tokens > 0).then(|| tokens as f64 * EMBEDDING_USD_PER_MILLION / 1_000_000.0);
        let usd = match self.provider {
            EmbeddingProvider::Openai | EmbeddingProvider::Vertex | EmbeddingProvider::Gateway => {
                list
            }
            EmbeddingProvider::Openrouter => reply.usage.cost.or(list),
        };
        Ok((reply.vectors, usd))
    }
}

impl Embed for Embedder {
    fn model(&self) -> &str {
        &self.model
    }

    /// The call on this provider, then on each backup in order while the
    /// last one failed. When every provider fails, the first one's error
    /// stands; each switch is logged with the provider and its error.
    async fn embed(&self, inputs: Vec<String>) -> Result<(Vec<Vec<f32>>, Option<f64>), EmbedError> {
        let first = match self.embed_here(inputs.clone()).await {
            Ok(done) => return Ok(done),
            Err(error) => error,
        };
        let mut failed = (self.provider, first.message.clone());
        for backup in &self.backups {
            eprintln!(
                "embeddings: {} failed ({}); {} takes the call",
                failed.0,
                failed.1.chars().take(200).collect::<String>(),
                backup.provider
            );
            match backup.embed_here(inputs.clone()).await {
                Ok(done) => return Ok(done),
                Err(error) => failed = (backup.provider, error.message),
            }
        }
        Err(first)
    }

    async fn embed_with_query(
        &self,
        documents: Vec<String>,
        query: Option<String>,
    ) -> Result<Embeddings, EmbedError> {
        let asked = query.is_some();
        let (mut vectors, usd) = match &self.transport {
            Transport::Compatible(_) => {
                let mut inputs = documents;
                inputs.extend(query);
                self.embed(inputs).await?
            }
            Transport::Vertex(vertex) => {
                let mut typed: Vec<(String, vertex::Task)> = documents
                    .into_iter()
                    .map(|text| (text, vertex::Task::Document))
                    .collect();
                typed.extend(query.map(|text| (text, vertex::Task::Query)));
                vertex.embed(&typed).await?
            }
        };
        let query = if asked { vectors.pop() } else { None };
        Ok((vectors, query, usd))
    }
}

/// A query's vector and the model that embedded it.
#[derive(Clone, Debug, PartialEq)]
pub struct Embedded {
    pub model: String,
    pub vector: Vec<f32>,
}

/// Each entry's cosine similarity to `query`, from `index`, the entry
/// vectors `model` made keyed by entry digest. An entry with no vector
/// scores 0.
///
/// # Errors
///
/// `query` was embedded by another model than `model`, or a vector in
/// `index` has another length than the query's. Vectors from different
/// models can't be compared, so the ranking is refused rather than made.
pub fn rank(
    model: &str,
    index: &HashMap<String, Vec<f32>>,
    entries: &[Entry],
    query: &Embedded,
) -> Result<Vec<f64>, EmbedError> {
    if query.model != model {
        return Err(EmbedError::from(format!(
            "the query was embedded by {} but the index holds {model}'s vectors, \
             and vectors from different models can't be compared",
            query.model
        )));
    }
    entries
        .iter()
        .map(|e| match index.get(&e.digest) {
            None => Ok(0.0),
            Some(v) if v.len() == query.vector.len() => Ok(cosine(v, &query.vector)),
            Some(v) => Err(EmbedError::from(format!(
                "the cached {model} vector for {} has {} dimensions and the query's has {}, \
                 so they aren't the same model's",
                e.id,
                v.len(),
                query.vector.len()
            ))),
        })
        .collect()
}

/// One entry's place in a search.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Hit {
    pub id: String,
    /// The combined score, 0 through 1.
    pub score: f64,
    /// BM25 bounded by the fixed transform `s / (1 + s)`.
    pub lexical: f64,
    /// Raw cosine similarity when embeddings were used.
    pub semantic: Option<f64>,
}

/// A search's result.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Search {
    /// The best entries first.
    pub hits: Vec<Hit>,
    /// Dollars the embeddings cost, or `None` when that's unknown: the
    /// provider reported neither a cost nor tokens, or the call failed after
    /// it was sent. A search that called nothing costs `Some(0.0)`.
    pub usd: Option<f64>,
    /// Why the ranking is lexical only, when it is.
    pub lexical_only: Option<String>,
}

/// Cached vectors: model, then entry digest.
type Cache = HashMap<String, HashMap<String, Vec<f32>>>;

/// A base, an optional embedder, and the entry embeddings cached on disk.
pub struct Retriever<E: Embed = Embedder> {
    pub base: Base,
    embedder: Option<E>,
    /// Why there's no embedder, when there isn't one.
    missing: String,
    cache_path: Option<PathBuf>,
    cache: Mutex<Cache>,
    /// Query vectors by the query's digest, for this process only.
    queries: Mutex<HashMap<String, Vec<f32>>>,
    /// The first failed embeddings call's error. After one, searches rank
    /// by words alone and say why, instead of paying for retries each time.
    failed: Mutex<Option<String>>,
}

impl<E: Embed> Retriever<E> {
    /// A retriever that ranks by BM25 alone; `why` says why.
    #[must_use]
    pub fn lexical(base: Base, why: &str) -> Self {
        Retriever {
            base,
            embedder: None,
            missing: why.to_string(),
            cache_path: None,
            cache: Mutex::new(Cache::new()),
            queries: Mutex::new(HashMap::new()),
            failed: Mutex::new(None),
        }
    }

    /// A retriever that also ranks by embeddings, caching entry vectors in
    /// `cache_path` when one is given.
    #[must_use]
    pub fn new(base: Base, embedder: E, cache_path: Option<PathBuf>) -> Self {
        let cache = cache_path
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str::<Cache>(&text).ok())
            .unwrap_or_default();
        Retriever {
            base,
            embedder: Some(embedder),
            missing: String::new(),
            cache_path,
            cache: Mutex::new(cache),
            queries: Mutex::new(HashMap::new()),
            failed: Mutex::new(None),
        }
    }

    /// The embedder, when there is one.
    #[must_use]
    pub fn embedder(&self) -> Option<&E> {
        self.embedder.as_ref()
    }

    /// Why every search is lexical, when there's no embedder.
    #[must_use]
    pub fn lexical_reason(&self) -> Option<&str> {
        self.embedder.is_none().then_some(self.missing.as_str())
    }

    /// The `limit` best entries for `query`.
    pub async fn search(&self, query: &str, limit: usize) -> Search {
        let entries = &self.base.entries;
        let raw = bm25(entries, query);
        let lexical: Vec<f64> = raw.iter().map(|s| s / (1.0 + s)).collect();
        let failed = self.failed.lock().ok().and_then(|f| f.clone());
        let (semantic, usd, lexical_only) = match (&self.embedder, failed) {
            (None, _) => (None, Some(0.0), Some(self.missing.clone())),
            (Some(_), Some(error)) => (
                None,
                Some(0.0),
                Some(format!(
                    "embeddings are off for this process after a failed call: {error}"
                )),
            ),
            (Some(embedder), None) => match self.similarities(embedder, query).await {
                Ok((similarities, usd)) => (Some(similarities), usd, None),
                // A refused request cost nothing; whether any other failed
                // call was billed isn't known.
                Err(error) => {
                    if let Ok(mut failed) = self.failed.lock() {
                        *failed = Some(error.message.clone());
                    }
                    (
                        None,
                        error.refused.then_some(0.0),
                        Some(format!("the embeddings call failed: {error}")),
                    )
                }
            },
        };
        let mut hits: Vec<Hit> = entries
            .iter()
            .enumerate()
            .map(|(i, entry)| Hit {
                id: entry.id.clone(),
                score: match &semantic {
                    Some(semantic) => (lexical[i] + semantic[i].clamp(0.0, 1.0)) / 2.0,
                    None => lexical[i],
                },
                lexical: lexical[i],
                semantic: semantic.as_ref().map(|s| s[i]),
            })
            .collect();
        // Filter before limiting. Relative rank is not evidence of relevance.
        hits.retain(|hit| match hit.semantic {
            Some(similarity) => similarity.is_finite() && similarity >= MIN_SEMANTIC_SIMILARITY,
            None => hit.score > 0.0,
        });
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(limit);
        Search {
            hits,
            usd,
            lexical_only,
        }
    }

    /// Each entry's cosine similarity to `query`, embedding the query and
    /// any entry not yet cached in one call.
    async fn similarities(
        &self,
        embedder: &E,
        query: &str,
    ) -> Result<(Vec<f64>, Option<f64>), EmbedError> {
        let model = embedder.model().to_string();
        let query: String = query.chars().take(QUERY_CHARS).collect();
        // The query cache is keyed by model too, so a query vector is never
        // reused for another model's entries.
        let query_key = format!("{model}\n{}", digest(query.as_bytes()));
        let missing: Vec<&Entry> = {
            let cache = self.cache.lock().map_err(|_| "the cache lock broke")?;
            let known = cache.get(&model);
            self.base
                .entries
                .iter()
                .filter(|e| known.is_none_or(|k| !k.contains_key(&e.digest)))
                .collect()
        };
        let cached_query = self
            .queries
            .lock()
            .map_err(|_| "the cache lock broke")?
            .get(&query_key)
            .cloned();
        let documents: Vec<String> = missing.iter().map(|e| e.search_text()).collect();
        let mut usd = Some(0.0);
        let mut query_vector = cached_query.clone();
        if !documents.is_empty() || cached_query.is_none() {
            let asked = cached_query.is_none().then(|| query.clone());
            let (vectors, embedded, cost) = embedder.embed_with_query(documents, asked).await?;
            usd = cost;
            if vectors.len() != missing.len() {
                return Err(EmbedError::from(format!(
                    "{} vectors came back for {} entries",
                    vectors.len(),
                    missing.len()
                )));
            }
            if query_vector.is_none() {
                query_vector = embedded;
            }
            if !missing.is_empty() {
                let mut cache = self.cache.lock().map_err(|_| "the cache lock broke")?;
                let known = cache.entry(model.clone()).or_default();
                for (entry, vector) in missing.iter().zip(vectors) {
                    known.insert(entry.digest.clone(), vector);
                }
                self.save(&cache);
            }
        }
        let query_vector = query_vector.ok_or("no vector came back for the query")?;
        self.queries
            .lock()
            .map_err(|_| "the cache lock broke")?
            .insert(query_key, query_vector.clone());
        let cache = self.cache.lock().map_err(|_| "the cache lock broke")?;
        let known = cache.get(&model).ok_or("no cached vectors")?;
        let query = Embedded {
            model: model.clone(),
            vector: query_vector,
        };
        let similarities = rank(&model, known, &self.base.entries, &query)?;
        Ok((similarities, usd))
    }

    /// Writes the cache; a failure only costs a later call.
    fn save(&self, cache: &Cache) {
        let Some(path) = &self.cache_path else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string(cache) {
            let _ = std::fs::write(path, text);
        }
    }
}

#[cfg(test)]
mod byok_tests {
    use super::*;

    #[test]
    fn embeddings_on_the_persons_key_never_fall_back_to_ours() {
        use model_access::{Access, ApiKey, Keys, Provider};
        assert!(Embedder::theirs(&Access::ours()).is_none());
        let mut keys = Keys::none();
        keys.insert(Provider::Vercel, ApiKey::new("their-gateway"));
        let embedder = Embedder::theirs(&Access::theirs(keys)).unwrap().unwrap();
        assert_eq!(embedder.provider, EmbeddingProvider::Gateway);
        assert_eq!(embedder.model, openrouter::EMBEDDING_MODEL);
        let mut keys = Keys::none();
        keys.insert(Provider::TypeSafe, ApiKey::new("ts"));
        let refused = Embedder::theirs(&Access::theirs(keys)).unwrap();
        assert_eq!(
            refused.err().unwrap(),
            "Your keys can't use openai/text-embedding-3-small."
        );
    }
}
