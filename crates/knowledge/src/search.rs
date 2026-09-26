//! Finding the entries that bear on a query: BM25 over each entry's
//! [`Entry::search_text`], combined with cosine similarity over embeddings
//! when an embedder is available.
//!
//! Both scores are scaled to 0 through 1 across the base before they're
//! averaged: BM25 by dividing by the best entry's score, and cosine
//! similarity by its range. When there's no embedder, or the embeddings
//! call fails, the ranking is BM25 alone, and the result says why.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;

use crate::{Base, Entry, digest};

/// BM25's term-frequency saturation.
const K1: f64 = 1.2;
/// BM25's length normalization.
const B: f64 = 0.75;

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
    let documents: Vec<Vec<String>> = entries.iter().map(|e| words(&e.search_text())).collect();
    let count = documents.len() as f64;
    if documents.is_empty() {
        return Vec::new();
    }
    let average = documents.iter().map(Vec::len).sum::<usize>() as f64 / count;
    let terms: HashSet<String> = words(query).into_iter().collect();
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

/// Turns text into vectors.
pub trait Embed {
    /// The model's name, which keys the cache.
    fn model(&self) -> &str;

    /// One vector per input, in order, and the call's cost in dollars.
    fn embed(
        &self,
        inputs: Vec<String>,
    ) -> impl std::future::Future<Output = Result<(Vec<Vec<f32>>, f64), String>>;
}

/// Embeddings through `crates/openrouter`.
pub struct OpenRouterEmbedder {
    pub client: openrouter::Client,
    pub model: String,
}

impl OpenRouterEmbedder {
    /// An embedder with the key from `OPENROUTER_API_KEY` or
    /// `~/.openagents/openrouter.json`, and the default model.
    ///
    /// # Errors
    ///
    /// No key, or the HTTP client can't start.
    pub fn from_env() -> Result<Self, String> {
        let config = openrouter::Config::from_env().map_err(|e| e.to_string())?;
        Ok(OpenRouterEmbedder {
            client: openrouter::Client::new(config).map_err(|e| e.to_string())?,
            model: openrouter::EMBEDDING_MODEL.to_string(),
        })
    }
}

impl Embed for OpenRouterEmbedder {
    fn model(&self) -> &str {
        &self.model
    }

    async fn embed(&self, inputs: Vec<String>) -> Result<(Vec<Vec<f32>>, f64), String> {
        let request = openrouter::EmbeddingRequest::new(&self.model, inputs);
        let reply = self
            .client
            .embeddings(&request)
            .await
            .map_err(|e| e.to_string())?;
        Ok((reply.vectors, reply.usage.cost.unwrap_or(0.0)))
    }
}

/// One entry's place in a search.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Hit {
    pub id: String,
    /// The combined score, 0 through 1.
    pub score: f64,
    /// BM25 scaled by the best entry's score.
    pub lexical: f64,
    /// Cosine similarity, before scaling, when embeddings were used.
    pub semantic: Option<f64>,
}

/// A search's result.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Search {
    /// The best entries first.
    pub hits: Vec<Hit>,
    /// Dollars the embeddings cost.
    pub usd: f64,
    /// Why the ranking is lexical only, when it is.
    pub lexical_only: Option<String>,
}

/// Cached vectors: model, then entry digest.
type Cache = HashMap<String, HashMap<String, Vec<f32>>>;

/// A base, an optional embedder, and the entry embeddings cached on disk.
pub struct Retriever<E: Embed = OpenRouterEmbedder> {
    pub base: Base,
    embedder: Option<E>,
    /// Why there's no embedder, when there isn't one.
    missing: String,
    cache_path: Option<PathBuf>,
    cache: Mutex<Cache>,
    /// Query vectors by the query's digest, for this process only.
    queries: Mutex<HashMap<String, Vec<f32>>>,
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
        }
    }

    /// The embedder, when there is one.
    #[must_use]
    pub fn embedder(&self) -> Option<&E> {
        self.embedder.as_ref()
    }

    /// The `limit` best entries for `query`.
    pub async fn search(&self, query: &str, limit: usize) -> Search {
        let entries = &self.base.entries;
        let raw = bm25(entries, query);
        let best = raw.iter().copied().fold(0.0_f64, f64::max);
        let lexical: Vec<f64> = raw
            .iter()
            .map(|s| if best > 0.0 { s / best } else { 0.0 })
            .collect();
        let (semantic, usd, lexical_only) = match &self.embedder {
            None => (None, 0.0, Some(self.missing.clone())),
            Some(embedder) => match self.similarities(embedder, query).await {
                Ok((similarities, usd)) => (Some(similarities), usd, None),
                Err(error) => (
                    None,
                    0.0,
                    Some(format!("the embeddings call failed: {error}")),
                ),
            },
        };
        let scaled: Option<Vec<f64>> = semantic.as_ref().map(|s| {
            let low = s.iter().copied().fold(f64::INFINITY, f64::min);
            let high = s.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            s.iter()
                .map(|v| {
                    if high > low {
                        (v - low) / (high - low)
                    } else {
                        0.0
                    }
                })
                .collect()
        });
        let mut hits: Vec<Hit> = entries
            .iter()
            .enumerate()
            .map(|(i, entry)| Hit {
                id: entry.id.clone(),
                score: match &scaled {
                    Some(scaled) => (lexical[i] + scaled[i]) / 2.0,
                    None => lexical[i],
                },
                lexical: lexical[i],
                semantic: semantic.as_ref().map(|s| s[i]),
            })
            .collect();
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
    async fn similarities(&self, embedder: &E, query: &str) -> Result<(Vec<f64>, f64), String> {
        let model = embedder.model().to_string();
        let query: String = query.chars().take(QUERY_CHARS).collect();
        let query_key = digest(query.as_bytes());
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
        let mut inputs: Vec<String> = missing.iter().map(|e| e.search_text()).collect();
        if cached_query.is_none() {
            inputs.push(query.clone());
        }
        let mut usd = 0.0;
        let mut query_vector = cached_query;
        if !inputs.is_empty() {
            let (mut vectors, cost) = embedder.embed(inputs).await?;
            usd = cost;
            if query_vector.is_none() {
                query_vector = vectors.pop();
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
        let similarities = self
            .base
            .entries
            .iter()
            .map(|e| {
                known
                    .get(&e.digest)
                    .map_or(0.0, |v| cosine(v, &query_vector))
            })
            .collect();
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
