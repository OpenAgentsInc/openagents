//! Embeddings from Google's `text-embedding-005` on Vertex AI, an opt-in
//! alternative to OpenAI's model for knowledge search.
//!
//! The request is Vertex AI's `predict` on the publisher model: one
//! instance per input, entries as `RETRIEVAL_DOCUMENT` and the query as
//! `RETRIEVAL_QUERY`. A request holds at most [`MAX_INPUTS`] inputs and
//! [`MAX_REQUEST_TOKENS`] tokens, so a long list goes out in several
//! requests. Vertex truncates an input past [`MAX_INPUT_TOKENS`] tokens.
//!
//! `text-embedding-005` is used rather than `gemini-embedding-001` because
//! it takes up to 250 inputs a request, where `gemini-embedding-001` takes
//! one, so indexing the base is a few calls rather than one per entry. Its
//! response also reports the billable characters its price is set in, so a
//! call's cost is known exactly.
//!
//! Vertex bills by character and reports the count in
//! `metadata.billableCharacterCount`, so a call's cost is that count at
//! [`USD_PER_THOUSAND_CHARACTERS`]. A response without the count leaves the
//! cost unknown.
//!
//! The bearer is a Google OAuth access token. It comes from the file named
//! by `VERTEX_TOKEN_FILE` when that's set, read before every request as the
//! `--provider vertex` model provider does. Otherwise it comes from
//! the Google credential the inference gateway reads
//! (`GOOGLE_APPLICATION_CREDENTIALS`, or the GCE metadata server when
//! `GCE_METADATA_HOST` is set), and failing that from
//! `gcloud auth print-access-token`, which honors `CLOUDSDK_CONFIG`, and is
//! reused for up to [`TOKEN_REUSE`]. The token is never printed.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::EmbedError;

/// The model on Vertex AI.
pub const MODEL: &str = "text-embedding-005";

/// The name that keys this model's vectors in the cache and names it in
/// run records. The provider is part of the name, so these vectors never
/// share a cache key with another provider's.
pub const CACHE_MODEL: &str = "vertex/text-embedding-005";

/// `text-embedding-005`'s list price in dollars per 1,000 billable input
/// characters for online requests, from Google Cloud's Vertex AI pricing
/// page (<https://cloud.google.com/vertex-ai/generative-ai/pricing>,
/// "Embeddings for Text (Excluding Gemini Embedding)", retrieved
/// 2026-09-27). Output is free.
pub const USD_PER_THOUSAND_CHARACTERS: f64 = 0.000_025;

/// Inputs a request may hold.
pub const MAX_INPUTS: usize = 250;

/// Tokens a request may hold across its inputs; more is an HTTP 400.
pub const MAX_REQUEST_TOKENS: usize = 20_000;

/// Tokens of one input that Vertex embeds; the rest is truncated.
pub const MAX_INPUT_TOKENS: usize = 2_048;

/// Google's Gemini embedding model, an alternative to [`MODEL`]: one input
/// a request, truncated (Matryoshka) to [`GEMINI_DIMENSIONS`] dimensions.
pub const GEMINI_MODEL: &str = "gemini-embedding-001";

/// The dimensions [`GEMINI_MODEL`] is asked for: [`MODEL`]'s 768.
pub const GEMINI_DIMENSIONS: u32 = 768;

/// The name that keys [`GEMINI_MODEL`]'s vectors in the cache.
pub const GEMINI_CACHE_MODEL: &str = "vertex/gemini-embedding-001@768";

/// The variable that picks the model: unset or [`MODEL`], or
/// [`GEMINI_MODEL`].
pub const MODEL_VAR: &str = "KB_VERTEX_MODEL";

/// The variable that sets how many times a refused or failed request is
/// retried (default 2), with backoff doubling from half a second to a
/// minute; a codebase build sets 8.
pub const RETRIES_VAR: &str = "KB_VERTEX_RETRIES";

/// The variable that names the Google Cloud project.
pub const PROJECT_VAR: &str = "KB_VERTEX_PROJECT";

/// The variable that names the region, default [`DEFAULT_LOCATION`].
pub const LOCATION_VAR: &str = "KB_VERTEX_LOCATION";

/// The variable that replaces the whole models URL, for a test endpoint.
pub const URL_VAR: &str = "KB_VERTEX_URL";

/// The variable that names a file holding an access token, shared with
/// the `--provider vertex` model provider.
pub const TOKEN_VAR: &str = "VERTEX_TOKEN_FILE";

/// The region used when [`LOCATION_VAR`] isn't set.
pub const DEFAULT_LOCATION: &str = "us-central1";

/// How long a token from `gcloud` is reused. Tokens last about an hour.
pub const TOKEN_REUSE: Duration = Duration::from_secs(30 * 60);

/// Where the access token comes from.
#[derive(Clone, Debug)]
pub enum Token {
    /// A file, read before every request.
    File(PathBuf),
    /// `gcloud auth print-access-token`.
    Gcloud,
    /// The inference gateway's Google credential
    /// (`inference::upstream::google::TokenSource`): a service-account key
    /// (`GOOGLE_APPLICATION_CREDENTIALS`) or the GCE metadata server
    /// (`GCE_METADATA_HOST`), its token cached until a minute before it
    /// expires. The chat worker's VM uses the metadata server.
    Google(inference::upstream::google::TokenSource),
}

/// What an input is for, which `text-embedding-005` embeds differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    /// A knowledge-base entry.
    Document,
    /// A search query.
    Query,
}

impl Task {
    fn wire(self) -> &'static str {
        match self {
            Task::Document => "RETRIEVAL_DOCUMENT",
            Task::Query => "RETRIEVAL_QUERY",
        }
    }
}

/// A client for `text-embedding-005` on Vertex AI.
#[derive(Debug)]
pub struct Vertex {
    /// The publisher models URL, such as
    /// `https://us-central1-aiplatform.googleapis.com/v1/projects/P/locations/us-central1/publishers/google/models`.
    pub base_url: String,
    pub token: Token,
    /// The model on Vertex AI: [`MODEL`] or [`GEMINI_MODEL`].
    pub model: String,
    /// Retries after a retryable status or a broken connection.
    pub retries: u32,
    /// A token from `gcloud` and when it was fetched.
    fetched: Mutex<Option<(String, Instant)>>,
}

/// The publisher models URL for `project` in `location`.
#[must_use]
pub fn models_url(project: &str, location: &str) -> String {
    format!(
        "https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models"
    )
}

fn var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

impl Vertex {
    /// A client for `base_url` with `token`.
    #[must_use]
    pub fn new(base_url: &str, token: Token) -> Self {
        Vertex {
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
            model: MODEL.to_string(),
            retries: 2,
            fetched: Mutex::new(None),
        }
    }

    /// The same client on `model` ([`MODEL`] or [`GEMINI_MODEL`]).
    #[must_use]
    pub fn on(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }

    /// The name this client's vectors are keyed by in caches and indexes.
    #[must_use]
    pub fn cache_model(&self) -> &'static str {
        if self.model == GEMINI_MODEL {
            GEMINI_CACHE_MODEL
        } else {
            CACHE_MODEL
        }
    }

    fn gemini(&self) -> bool {
        self.model == GEMINI_MODEL
    }

    /// A client configured by the environment: [`URL_VAR`], or else
    /// [`PROJECT_VAR`] (or `GOOGLE_CLOUD_PROJECT`) and [`LOCATION_VAR`];
    /// and [`TOKEN_VAR`], or else `gcloud`.
    ///
    /// # Errors
    ///
    /// No project is named.
    pub fn from_env() -> Result<Self, String> {
        let base_url = match var(URL_VAR) {
            Some(url) => url,
            None => {
                let project = var(PROJECT_VAR)
                    .or_else(|| var("VERTEX_PROJECT"))
                    .or_else(|| var("GOOGLE_CLOUD_PROJECT"))
                    .ok_or(format!(
                        "Vertex AI embeddings need a project: set {PROJECT_VAR}"
                    ))?;
                let location = var(LOCATION_VAR).unwrap_or_else(|| DEFAULT_LOCATION.to_string());
                models_url(&project, &location)
            }
        };
        let token = match var(TOKEN_VAR) {
            Some(path) => Token::File(path.into()),
            None => {
                let google = inference::upstream::google::TokenSource::from_env();
                if google.present() {
                    Token::Google(google)
                } else {
                    Token::Gcloud
                }
            }
        };
        let model = match var(MODEL_VAR).as_deref() {
            None | Some(MODEL) => MODEL,
            Some(GEMINI_MODEL) => GEMINI_MODEL,
            Some(other) => {
                return Err(format!(
                    "{MODEL_VAR} is {MODEL} or {GEMINI_MODEL}, not {other}"
                ));
            }
        };
        let mut vertex = Vertex::new(&base_url, token).on(model);
        // A long build (tens of thousands of chunks) meets Vertex's
        // per-minute quota (429); more retries ride it out.
        if let Some(retries) = var(RETRIES_VAR).and_then(|r| r.parse().ok()) {
            vertex.retries = retries;
        }
        Ok(vertex)
    }

    /// The bearer for the next request.
    async fn bearer(&self) -> Result<String, String> {
        match &self.token {
            Token::Google(google) => google.token().await.map(|token| token.expose().to_string()),
            Token::File(path) => {
                let token = std::fs::read_to_string(path).map_err(|e| {
                    format!("can't read the Vertex token at {}: {e}", path.display())
                })?;
                let token = token.trim().to_string();
                if token.is_empty() {
                    return Err(format!("the Vertex token at {} is empty", path.display()));
                }
                Ok(token)
            }
            Token::Gcloud => {
                let mut fetched = self.fetched.lock().map_err(|_| "the token lock broke")?;
                if let Some((token, at)) = fetched.as_ref()
                    && at.elapsed() < TOKEN_REUSE
                {
                    return Ok(token.clone());
                }
                let output = std::process::Command::new("gcloud")
                    .args(["auth", "print-access-token"])
                    .output()
                    .map_err(|e| format!("can't run gcloud for a Vertex token: {e}"))?;
                if !output.status.success() {
                    let why: String = String::from_utf8_lossy(&output.stderr)
                        .trim()
                        .chars()
                        .take(300)
                        .collect();
                    return Err(format!(
                        "gcloud auth print-access-token failed ({}): {why}",
                        output.status
                    ));
                }
                let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if token.is_empty() {
                    return Err("gcloud auth print-access-token printed no token".to_string());
                }
                *fetched = Some((token.clone(), Instant::now()));
                Ok(token)
            }
        }
    }

    /// One vector per input, in order, and the cost in dollars, or `None`
    /// when any request's cost is unknown. Inputs go out in as many
    /// requests as [`batches`] makes.
    ///
    /// # Errors
    ///
    /// The first failed request. A failure is `refused` only when no
    /// request was billed before it and Vertex answered it with an error
    /// status, or nothing was sent.
    pub async fn embed(
        &self,
        inputs: &[(String, Task)],
    ) -> Result<(Vec<Vec<f32>>, Option<f64>), EmbedError> {
        let mut vectors = Vec::with_capacity(inputs.len());
        let mut usd = Some(0.0);
        let ranges = if self.gemini() {
            // Gemini's embedding model takes one input a request.
            (0..inputs.len()).map(|i| i..i + 1).collect()
        } else {
            batches(inputs)
        };
        for (index, range) in ranges.into_iter().enumerate() {
            let earlier = index > 0;
            let (batch, cost) =
                self.request(&inputs[range.clone()])
                    .await
                    .map_err(|mut error| {
                        if earlier {
                            error.refused = false;
                            error.message = format!(
                                "{} (after {index} billed requests, so the cost is unknown)",
                                error.message
                            );
                        }
                        error
                    })?;
            vectors.extend(batch);
            usd = match (usd, cost) {
                (Some(a), Some(b)) => Some(a + b),
                _ => None,
            };
        }
        Ok((vectors, usd))
    }

    /// One `predict` request.
    async fn request(
        &self,
        inputs: &[(String, Task)],
    ) -> Result<(Vec<Vec<f32>>, Option<f64>), EmbedError> {
        // Nothing has been sent when there's no token or client.
        let refused = |message: String| EmbedError {
            message,
            refused: true,
        };
        let token = self.bearer().await.map_err(refused)?;
        let mut config = openrouter::Config::new(openrouter::ApiKey::new(&token));
        config.base_url.clone_from(&self.base_url);
        config.referer = None;
        config.title = None;
        config.retries = self.retries;
        let client = openrouter::Client::new(config).map_err(|e| refused(e.to_string()))?;
        let body = Predict {
            instances: inputs
                .iter()
                .map(|(content, task)| Instance {
                    content,
                    task_type: task.wire(),
                })
                .collect(),
            parameters: Parameters {
                auto_truncate: true,
                output_dimensionality: self.gemini().then_some(GEMINI_DIMENSIONS),
            },
        };
        let reply: Predicted = client
            .post_json(&format!("{}:predict", self.model), &body)
            .await
            .map_err(|e| EmbedError {
                refused: matches!(e, openrouter::Error::Api { .. } | openrouter::Error::NoKey),
                // The client is OpenRouter's, which names itself in errors.
                message: e.to_string().replace("OpenRouter", "Vertex AI"),
            })?;
        // Gemini's embedding model is priced by token, not character, and
        // its response carries no count: the cost stays unknown.
        let usd = reply
            .metadata
            .filter(|_| !self.gemini())
            .and_then(|m| m.billable_character_count)
            .filter(|count| *count > 0 || inputs.iter().all(|(c, _)| c.trim().is_empty()))
            .map(|count| count as f64 * USD_PER_THOUSAND_CHARACTERS / 1_000.0);
        let unread = |detail: String| EmbedError {
            // The request was answered, so it may have been billed.
            message: format!("Vertex AI's response couldn't be read: {detail}"),
            refused: false,
        };
        if reply.predictions.len() != inputs.len() {
            return Err(unread(format!(
                "{} vectors for {} inputs",
                reply.predictions.len(),
                inputs.len()
            )));
        }
        let vectors: Vec<Vec<f32>> = reply
            .predictions
            .into_iter()
            .map(|p| p.embeddings.values)
            .collect();
        if vectors.iter().any(Vec::is_empty) {
            return Err(unread("an empty vector".to_string()));
        }
        Ok((vectors, usd))
    }
}

/// An estimate of an input's tokens that errs high: three tokens per five
/// characters, plus one. Not capped where Vertex truncates an input: the
/// request limit counts an input's tokens before truncation (2026-10-10, a
/// codebase build's batch estimated under 20,000 was refused at 22,565).
fn estimated_tokens(text: &str) -> usize {
    (text.chars().count() * 3).div_ceil(5) + 1
}

/// Splits `inputs` into runs of at most [`MAX_INPUTS`] inputs whose
/// estimated tokens fit in [`MAX_REQUEST_TOKENS`], in order.
#[must_use]
pub fn batches<T>(inputs: &[(String, T)]) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut tokens = 0;
    for (i, (text, _)) in inputs.iter().enumerate() {
        let estimate = estimated_tokens(text);
        if i > start && (i - start >= MAX_INPUTS || tokens + estimate > MAX_REQUEST_TOKENS) {
            out.push(start..i);
            start = i;
            tokens = 0;
        }
        tokens += estimate;
    }
    if start < inputs.len() {
        out.push(start..inputs.len());
    }
    out
}

#[derive(Serialize)]
struct Predict<'a> {
    instances: Vec<Instance<'a>>,
    parameters: Parameters,
}

#[derive(Serialize)]
struct Instance<'a> {
    content: &'a str,
    task_type: &'static str,
}

#[derive(Serialize)]
struct Parameters {
    #[serde(rename = "autoTruncate")]
    auto_truncate: bool,
    #[serde(
        rename = "outputDimensionality",
        skip_serializing_if = "Option::is_none"
    )]
    output_dimensionality: Option<u32>,
}

#[derive(Deserialize)]
struct Predicted {
    #[serde(default)]
    predictions: Vec<Prediction>,
    #[serde(default)]
    metadata: Option<Metadata>,
}

#[derive(Deserialize)]
struct Prediction {
    embeddings: Values,
}

#[derive(Deserialize)]
struct Values {
    values: Vec<f32>,
}

#[derive(Deserialize)]
struct Metadata {
    #[serde(rename = "billableCharacterCount", default)]
    billable_character_count: Option<u64>,
}

#[cfg(test)]
mod tests;
