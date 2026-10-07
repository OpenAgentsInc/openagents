//! Bounded public Brainstorm HTTP observations. The caller admits the exact
//! outbound input; enablement alone does not establish disclosure authority.

mod client;
mod transport;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, io::Write, time::SystemTime};
use tokio::sync::watch;

pub use client::Client;

pub const DEFAULT_ORIGIN: &str = "https://api.brainstorm.world";
pub const DISCOVERY_PATH: &str = "/.well-known/open-ranking.json";
pub const HOUSE_PATH: &str = "/.well-known/nostr.json?name=_";
pub const SEARCH_PATH: &str = "/search/pubkeys";
pub const RANK_PATH: &str = "/rank/pubkeys";

/// Host-selected engineering bounds, not upstream service guarantees.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Limits {
    pub query_characters: usize,
    pub query_bytes: usize,
    pub search_results: usize,
    pub rank_subjects: usize,
    pub concurrent_operations: usize,
    pub deadline_ms: u64,
    pub response_bytes: usize,
    pub normalized_bytes: usize,
    pub context_bytes: usize,
    pub cache_ttl_seconds: u64,
    pub cache_entries: usize,
    pub cache_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            query_characters: 512,
            query_bytes: 1024,
            search_results: 10,
            rank_subjects: 20,
            concurrent_operations: 2,
            deadline_ms: 15_000,
            response_bytes: 256 * 1024,
            normalized_bytes: 64 * 1024,
            context_bytes: 8 * 1024,
            cache_ttl_seconds: 300,
            cache_entries: 16,
            cache_bytes: 256 * 1024,
        }
    }
}

impl Limits {
    pub fn validate(&self) -> Result<(), Error> {
        let maximum = Self::default();
        macro_rules! bound {
            ($($field:ident),+ $(,)?) => {
                $(if self.$field == 0 || self.$field > maximum.$field {
                    return Err(Error::InvalidConfiguration { field: stringify!($field).into() });
                })+
            };
        }
        bound!(
            query_characters,
            query_bytes,
            search_results,
            rank_subjects,
            concurrent_operations,
            deadline_ms,
            response_bytes,
            normalized_bytes,
            context_bytes,
            cache_ttl_seconds,
            cache_entries,
            cache_bytes
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    /// An origin, with no credentials, query, fragment, or path prefix.
    pub origin: String,
    pub limits: Limits,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            origin: DEFAULT_ORIGIN.into(),
            limits: Limits::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Error {
    #[error("Invalid Brainstorm configuration: {field}")]
    InvalidConfiguration { field: String },
    #[error("Invalid Brainstorm input: {field}")]
    InvalidInput { field: String },
    #[error("Brainstorm is disabled")]
    Disabled,
    #[error("Brainstorm lookup was cancelled")]
    Cancelled,
    #[error("Brainstorm lookup exceeded its deadline")]
    Timeout,
    #[error("Brainstorm transport failed")]
    Transport,
    #[error("Brainstorm refused an HTTP redirect")]
    RedirectRefused,
    #[error("Brainstorm response exceeds its byte limit")]
    ResponseTooLarge,
    #[error("Brainstorm output exceeds its byte limit")]
    OutputTooLarge,
    #[error("Brainstorm context exceeds its byte limit")]
    ContextTooLarge,
    #[error("Brainstorm response is malformed: {field}")]
    InvalidResponse { field: String },
    #[error("Brainstorm discovery is unavailable: {component}")]
    DiscoveryUnavailable {
        component: String,
        cause: Box<Error>,
    },
    #[error("Brainstorm does not advertise the required operation: {endpoint}")]
    UnsupportedOperation { endpoint: String },
    #[error("Brainstorm requires authentication")]
    AuthenticationRequired { status: u16 },
    #[error("Brainstorm is rate limited")]
    RateLimited { retry_after_seconds: Option<u64> },
    #[error("Brainstorm scores are computing")]
    Computing { retry_after_seconds: Option<u64> },
    #[error("Brainstorm perspective is unavailable")]
    PerspectiveUnavailable,
    #[error("Brainstorm service returned HTTP {status}")]
    Service { status: u16 },
}

/// Cancellation drops the in-flight HTTP future and queued semaphore wait.
#[derive(Clone, Debug)]
pub struct Cancellation(watch::Sender<bool>);

impl Default for Cancellation {
    fn default() -> Self {
        Self(watch::channel(false).0)
    }
}

impl Cancellation {
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }
    pub fn is_cancelled(&self) -> bool {
        *self.0.borrow()
    }
    pub(crate) async fn cancelled(&self) {
        let mut receiver = self.0.subscribe();
        let _ = receiver.wait_for(|cancelled| *cancelled).await;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    SearchPeople,
    Rank,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Algorithm {
    Relevance,
    Graperank,
}

impl Algorithm {
    pub(crate) fn wire(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Graperank => "graperank",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    /// The provider returned a nonzero score; this says nothing about corpus coverage.
    Reported,
    /// A computed zero and an absent subject cannot be distinguished by this API.
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Influence {
    pub value: f64,
    pub coverage: Coverage,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subject {
    pub pubkey: String,
    pub profile_url: String,
    pub relevance: Option<f64>,
    pub influence: Option<Influence>,
}

/// The separately discovered identity is not atomically bound to data responses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseIdentity {
    pub pubkey: String,
    pub origin: String,
    pub discovered_at_ms: u64,
    pub attribution: Attribution,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Attribution {
    SeparateHttpsObservation,
}

/// Digests describe the actual request body and response bytes, not signatures.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseEvidence {
    pub origin: String,
    pub endpoint: String,
    pub status: u16,
    pub requested_algorithm: Option<Algorithm>,
    pub fetched_at_ms: u64,
    pub expires_at_ms: u64,
    pub input_digest: String,
    pub output_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Discovery {
    pub house: HouseIdentity,
    pub search_supported: bool,
    pub rank_supported: bool,
    pub responses: Vec<ResponseEvidence>,
    pub expires_at_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Completeness {
    /// Complete for the bounded request, not for the upstream dataset.
    Bounded,
    /// One or more requested influence scores were unavailable.
    Partial,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub operation: Operation,
    pub configuration: Config,
    pub configuration_digest: String,
    pub input_digest: String,
    pub house: HouseIdentity,
    pub subjects: Vec<Subject>,
    pub responses: Vec<ResponseEvidence>,
    pub enrichment_error: Option<Error>,
    pub completeness: Completeness,
    pub expires_at_ms: u64,
}

impl Observation {
    pub fn is_fresh_at(&self, now_ms: u64) -> bool {
        now_ms < self.expires_at_ms
    }

    /// Preserve provenance while omitting subjects to fit the context allowance.
    /// The caller also accounts for other messages in its route's total budget.
    pub fn model_context(&self) -> Result<String, Error> {
        #[derive(Serialize)]
        struct Context<'a> {
            observation: &'a Observation,
            context_truncated: bool,
            omitted_subjects: usize,
        }
        let mut observation = self.clone();
        loop {
            let omitted = self.subjects.len() - observation.subjects.len();
            match bounded_json(
                &Context {
                    observation: &observation,
                    context_truncated: omitted != 0,
                    omitted_subjects: omitted,
                },
                self.configuration.limits.context_bytes.min(8 * 1024),
            ) {
                Ok(bytes) => return String::from_utf8(bytes).map_err(|_| Error::ContextTooLarge),
                Err(_) if !observation.subjects.is_empty() => {
                    observation.subjects.pop();
                }
                Err(_) => return Err(Error::ContextTooLarge),
            }
        }
    }

    pub fn cache_key(&self) -> CacheKey {
        CacheKey {
            configuration_digest: self.configuration_digest.clone(),
            operation: self.operation,
            input_digest: self.input_digest.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheKey {
    pub configuration_digest: String,
    pub operation: Operation,
    pub input_digest: String,
}

/// Optional, task-owned bounded storage. Client operations never cache queries.
pub struct TaskCache {
    limits: Limits,
    entries: VecDeque<(CacheKey, Observation, usize)>,
    bytes: usize,
}

impl TaskCache {
    pub fn new(limits: Limits) -> Result<Self, Error> {
        limits.validate()?;
        Ok(Self {
            limits,
            entries: VecDeque::new(),
            bytes: 0,
        })
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn insert(&mut self, mut observation: Observation, now_ms: u64) -> Result<bool, Error> {
        self.expire(now_ms);
        if let Some(expiry) = observation
            .responses
            .iter()
            .map(|response| response.expires_at_ms)
            .min()
        {
            observation.expires_at_ms = observation.expires_at_ms.min(expiry);
        }
        observation.expires_at_ms = observation
            .expires_at_ms
            .min(now_ms.saturating_add(self.limits.cache_ttl_seconds.saturating_mul(1000)));
        if !observation.is_fresh_at(now_ms) {
            return Ok(false);
        }
        let bytes = bounded_json(
            &observation,
            self.limits.normalized_bytes.min(self.limits.cache_bytes),
        )?
        .len();
        let key = observation.cache_key();
        if let Some(index) = self.entries.iter().position(|entry| entry.0 == key) {
            let old = self.entries.remove(index).expect("the cache index exists");
            self.bytes -= old.2;
        }
        while self.entries.len() >= self.limits.cache_entries
            || self.bytes + bytes > self.limits.cache_bytes
        {
            let old = self
                .entries
                .pop_front()
                .expect("bounded entry fits an empty cache");
            self.bytes -= old.2;
        }
        self.bytes += bytes;
        self.entries.push_back((key, observation, bytes));
        Ok(true)
    }

    pub fn get(&mut self, key: &CacheKey, now_ms: u64) -> Option<Observation> {
        self.expire(now_ms);
        self.entries
            .iter()
            .find(|entry| &entry.0 == key)
            .map(|entry| entry.1.clone())
    }

    fn expire(&mut self, now_ms: u64) {
        self.entries.retain(|entry| entry.1.is_fresh_at(now_ms));
        self.bytes = self.entries.iter().map(|entry| entry.2).sum();
    }
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn pubkey(text: &str) -> Option<String> {
    (text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| text.to_ascii_lowercase())
}

pub(crate) fn bounded_json(value: &impl Serialize, limit: usize) -> Result<Vec<u8>, Error> {
    struct Bounded {
        bytes: Vec<u8>,
        limit: usize,
    }
    impl Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
                return Err(std::io::Error::other("output byte limit"));
            }
            self.bytes.reserve_exact(bytes.len());
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Bounded {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| Error::OutputTooLarge)?;
    Ok(writer.bytes)
}

#[cfg(test)]
mod tests;
