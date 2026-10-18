//! Presence and progress for runs from any agent or harness.
pub mod commands;
pub use commands::{CommandOutcome, CommandReceipt, CommandRefusal, RunCommand, SessionCommand};

use crate::{CancellationToken, Client, Error, Session, sessions::SessionStatus};
use reqwest::Method;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

/// Minimum interval between heartbeats with the same status, in milliseconds.
pub const HEARTBEAT_INTERVAL_MS: i64 = 60_000;
/// A run is live for three minutes after its last accepted heartbeat.
pub const LIVE_WINDOW_MS: i64 = 180_000;

/// Where a run executes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    /// A chat in an app.
    AppChat,
    /// A terminal harness.
    Terminal,
    /// A cloud worker.
    CloudWork,
    /// A landing task.
    Landing,
}

/// A host's latest report. Output and progress times are Unix milliseconds;
/// neither is proof of liveness. The server records receipt time separately.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunHeartbeat {
    /// Stable run ID. Use a new ID after ending a run.
    pub run: String,
    /// Where this run executes.
    pub kind: RunKind,
    /// Harness identifier, such as `codex`.
    pub harness: String,
    /// Harness session ID, for joining issue claims.
    #[serde(default)]
    pub harness_session: Option<String>,
    /// Computer identifier.
    pub computer: String,
    /// Issue numbers this run holds; at most 32, all positive and distinct.
    #[serde(default)]
    pub issues: Vec<u64>,
    /// What the run is doing.
    pub status: SessionStatus,
    /// Unix milliseconds of the last model or tool output.
    pub last_output_at: i64,
    /// Unix milliseconds of the last commit, push, landing step, or test result.
    #[serde(default)]
    pub last_progress_at: Option<i64>,
    /// One plain line, fewer than 200 UTF-8 bytes.
    #[serde(default)]
    pub note: Option<String>,
    /// Agent definition ID, when this run belongs to a saved agent.
    #[serde(default)]
    pub agent: Option<String>,
}

/// Whether an identifier can be used in a run URL or report.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
        && !matches!(id, "." | "..")
}
impl RunHeartbeat {
    /// Whether the report meets the API's size and value limits.
    pub fn valid(&self) -> bool {
        valid_id(&self.run)
            && valid_id(&self.harness)
            && valid_id(&self.computer)
            && self.harness_session.as_deref().is_none_or(valid_id)
            && self.agent.as_deref().is_none_or(crate::agents::valid_id)
            && self.issues.len() <= 32
            && self.issues.iter().all(|n| *n > 0)
            && self
                .issues
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.issues.len()
            && !matches!(self.status, SessionStatus::Other(_))
            && self.last_output_at >= 0
            && self.last_progress_at.is_none_or(|at| at >= 0)
            && self.note.as_deref().is_none_or(|n| {
                n.len() < 200
                    && !n
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
            })
    }
}

/// Filters combined with AND. `live: Some(false)` selects ended or expired runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunFilter {
    /// Filter by recent server receipt time and whether the run ended.
    pub live: Option<bool>,
    /// Select runs holding this issue number.
    pub issue: Option<u64>,
}

/// Runs belonging to one signed-in account.
pub struct Runs<'a> {
    client: &'a Client,
    session: &'a Session,
}
impl Client {
    /// Access runs belonging to the session's account.
    pub fn runs<'a>(&'a self, session: &'a Session) -> Result<Runs<'a>, Error> {
        self.check_session(session)?;
        Ok(Runs {
            client: self,
            session,
        })
    }
}
impl Runs<'_> {
    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &[&str],
        query: &[(&str, String)],
        body: Option<Value>,
        cancel: &CancellationToken,
    ) -> Result<T, Error> {
        self.client.check_session(self.session)?;
        tokio::select! { biased;
            _ = cancel.cancelled() => Err(Error::Cancelled),
            result = self.client.request(method, path, query, body, Some(self.session.token())) => result,
        }
    }

    /// Report once a minute and on status changes. An exact retry within a minute succeeds
    /// without extending liveness. Ended IDs cannot receive heartbeats.
    pub async fn heartbeat(
        &self,
        beat: &RunHeartbeat,
        cancel: &CancellationToken,
    ) -> Result<(), Error> {
        if !beat.valid() {
            return Err(Error::InvalidReference);
        }
        let _: Value = self
            .send(
                Method::PUT,
                &["runs", &beat.run, "heartbeat"],
                &[],
                Some(serde_json::to_value(beat).map_err(|_| Error::InvalidReference)?),
                cancel,
            )
            .await?;
        Ok(())
    }

    /// List at most 512 reports, newest receipt first. Reports disappear after
    /// 24 hours without a heartbeat; ended runs use their end time.
    pub async fn list(
        &self,
        filter: RunFilter,
        cancel: &CancellationToken,
    ) -> Result<Vec<RunHeartbeat>, Error> {
        if filter.issue == Some(0) {
            return Err(Error::InvalidReference);
        }
        let mut query = Vec::new();
        if let Some(live) = filter.live {
            query.push(("live", live.to_string()));
        }
        if let Some(issue) = filter.issue {
            query.push(("issue", issue.to_string()));
        }
        #[derive(Deserialize)]
        struct Listing {
            runs: Vec<RunHeartbeat>,
        }
        let listing: Listing = self
            .send(Method::GET, &["runs"], &query, None, cancel)
            .await?;
        Ok(listing.runs)
    }

    /// Record that a known run has ended. Repeated requests succeed; an unknown ID is 404.
    pub async fn end(&self, run: &str, cancel: &CancellationToken) -> Result<(), Error> {
        if !valid_id(run) {
            return Err(Error::InvalidReference);
        }
        let _: Value = self
            .send(Method::POST, &["runs", run, "end"], &[], None, cancel)
            .await?;
        Ok(())
    }
}
