//! Account-wide events that need an agent's or person's attention.
use crate::{CancellationToken, Client, Error, Session};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A working run with no output for this long is stalled, in milliseconds.
pub const STALL_AFTER_MS: i64 = 20 * 60 * 1000;
/// A live run holding issues with no progress for this long raises
/// [`Attention::ClaimNoProgress`], in milliseconds.
pub const CLAIM_IDLE_AFTER_MS: i64 = 20 * 60 * 1000;

/// A machine resource that can run low.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    /// Free disk space, in bytes.
    Disk,
    /// Free memory, in bytes.
    Memory,
}

/// What needs attention. Times are Unix milliseconds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Attention {
    /// A working run has shown no output since `since`.
    RunStalled {
        /// Run ID.
        run: String,
        /// The run's last output time.
        since: i64,
    },
    /// A run is waiting for an answer.
    RunAwaitingInput {
        /// Run ID.
        run: String,
        /// The run's note, or empty.
        question: String,
    },
    /// A run reported an error.
    RunErrored {
        /// Run ID.
        run: String,
        /// The run's note, or empty.
        reason: String,
    },
    /// A live run holds an issue without progress since `since`.
    ClaimNoProgress {
        /// Issue number.
        issue: u64,
        /// The run's harness session ID, or its run ID.
        holder: String,
        /// The run's last progress time, or when it was first seen without any.
        since: i64,
    },
    /// A landing failed.
    LandingFailed {
        /// Landing queue entry or landing run ID.
        entry: String,
        /// The issue it was landing, when known.
        issue: Option<u64>,
        /// Why, or empty.
        reason: String,
    },
    /// A work run finished.
    WorkFinished {
        /// Work run ID.
        work: String,
        /// Issue number.
        issue: u64,
        /// `done`, `failed`, or `cancelled`.
        outcome: String,
    },
    /// A computer is low on a resource.
    MachineLow {
        /// Computer identifier.
        computer: String,
        /// Which resource.
        resource: Resource,
        /// What remains, in bytes.
        value: u64,
    },
    /// A kind this SDK version does not know.
    #[serde(other)]
    Unknown,
}

/// One event in an account's attention stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttentionEvent {
    /// Cursor position; strictly increasing per account, starting at 1.
    pub seq: u64,
    /// Unix milliseconds when the server recorded the event.
    pub at: i64,
    /// Agent definition ID of the run's saved agent, when it has one.
    #[serde(default)]
    pub agent: Option<String>,
    /// What needs attention.
    #[serde(flatten)]
    pub attention: Attention,
}

impl Client {
    /// Follow the account's attention events after cursor `after` (0 for all
    /// retained events). After a dropped connection, call again with
    /// [`AttentionStream::cursor`].
    pub async fn attention(
        &self,
        session: &Session,
        after: u64,
        cancel: &CancellationToken,
    ) -> Result<AttentionStream, Error> {
        self.check_session(session)?;
        let segments = ["attention"];
        let query = [("after", after.to_string())];
        let future = self.send_events(&segments, &query, session.token());
        let response = tokio::select! {biased; _=cancel.cancelled()=>return Err(Error::Cancelled),result=future=>result?};
        if !response.status().is_success() {
            Client::decode::<Value>(response, &segments, 1024 * 1024).await?;
            return Err(Error::InvalidResponse);
        }
        if response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_none_or(|v| !v.starts_with("text/event-stream"))
        {
            return Err(Error::InvalidResponse);
        }
        Ok(AttentionStream {
            response,
            buffer: Vec::new(),
            cursor: after,
            cancel: cancel.clone(),
        })
    }
}

/// Attention events in order, read with [`AttentionStream::next`].
pub struct AttentionStream {
    response: reqwest::Response,
    buffer: Vec<u8>,
    cursor: u64,
    cancel: CancellationToken,
}
impl AttentionStream {
    /// The last event's `seq`, or the starting cursor; resume from here.
    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Reads the next event. Returns `None` when the server closes the stream;
    /// reconnect with [`AttentionStream::cursor`].
    pub async fn next(&mut self) -> Result<Option<AttentionEvent>, Error> {
        loop {
            if self.cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            if let Some(end) = self.buffer.windows(2).position(|w| w == b"\n\n") {
                let raw: Vec<u8> = self.buffer.drain(..end + 2).collect();
                let frame = std::str::from_utf8(&raw).map_err(|_| Error::InvalidResponse)?;
                let data = frame
                    .lines()
                    .filter_map(|l| l.strip_prefix("data:").map(str::trim_start))
                    .collect::<Vec<_>>()
                    .join("\n");
                if data.is_empty() {
                    continue;
                }
                let event: AttentionEvent =
                    serde_json::from_str(&data).map_err(|_| Error::InvalidResponse)?;
                if event.seq <= self.cursor {
                    return Err(Error::InvalidResponse);
                }
                self.cursor = event.seq;
                return Ok(Some(event));
            }
            let chunk = tokio::select! {biased; _=self.cancel.cancelled()=>return Err(Error::Cancelled),result=self.response.chunk()=>result.map_err(|e|Error::Network{timeout:e.is_timeout()})?};
            let Some(chunk) = chunk else {
                return Ok(None);
            };
            if chunk.len() > 1024 * 1024 - self.buffer.len().min(1024 * 1024) {
                return Err(Error::TooLarge);
            }
            self.buffer
                .extend(chunk.iter().copied().filter(|b| *b != b'\r'));
        }
    }
}
