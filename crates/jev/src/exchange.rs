//! Another way for a call to reach the door: an [`Exchange`] carries each
//! attempt instead of this crate's HTTP client.
//!
//! A client built with [`crate::Config::exchange`] prepares every call
//! exactly as it would for HTTP — the body, the headers, the retry policy,
//! the per-attempt timeout, and the call budget — and hands each attempt to
//! the exchange. The exchange answers with a status, headers, and a body,
//! and the client reads that answer the way it reads an HTTP response, so a
//! caller sees the same types and the same errors whichever way the call
//! went.
//!
//! OpenAgents uses this for the hosted decision service: a computer with no
//! TypeSafe key sends its judgments as NIP-CJ decision jobs to a worker that
//! holds the key (`crates/jev-hosted`). No key crosses an exchange: a client
//! with one refuses to build.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

/// One attempt, as an exchange receives it.
#[derive(Debug, Clone)]
pub struct Call {
    /// The HTTP method the attempt would use, such as `POST`.
    pub method: String,
    /// The route, such as `/v1/systemone`.
    pub path: String,
    /// The JSON body, when the route takes one.
    pub body: Option<Vec<u8>>,
    /// The caller's idempotency key, when it set one: every attempt of one
    /// call shares it.
    pub idempotency_key: Option<String>,
    /// This attempt's number, one-based.
    pub attempt: u32,
    /// How long this attempt may take: the call's timeout, capped by what
    /// its budget leaves.
    pub timeout: Duration,
}

/// What an exchange answered: the parts of an HTTP response the client
/// reads.
#[derive(Debug, Clone)]
pub struct Reply {
    /// The status, as HTTP would carry it: 200 for an answer, 429 for a
    /// quota or rate refusal, 503 for a door that could not answer.
    pub status: u16,
    /// Response headers, lowercase names.
    pub headers: Vec<(String, String)>,
    /// The body. An error body is `{"error": {"code", "message"}}`.
    pub body: Vec<u8>,
}

/// Why an attempt got no reply.
#[derive(Debug, Clone)]
pub enum Failure {
    /// The attempt's timeout ran out first.
    Timeout,
    /// The exchange could not reach the service, with what it saw.
    Unreachable(String),
}

/// The boxed future an exchange returns.
pub type Pending<'a> = Pin<Box<dyn Future<Output = Result<Reply, Failure>> + Send + 'a>>;

/// A carrier for a client's attempts.
pub trait Exchange: Send + Sync + fmt::Debug {
    /// Carry one attempt and return its reply.
    fn exchange(&self, call: Call) -> Pending<'_>;

    /// What carries the call, for evidence: a short line such as
    /// `hosted decision service <worker> on <relay>`. It never holds a
    /// secret.
    fn service(&self) -> String;
}
