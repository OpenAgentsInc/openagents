//! Submit and inspect changes in the shared landing queue (`/v1/land`).

use crate::{CancellationToken, Client, Error, Session};
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// A branch already pushed to its repository's `origin`, ready to land.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LandSubmit {
    pub repo: String,
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Where a submitted change stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LandState {
    #[default]
    Queued,
    Landing,
    Landed,
    Bounced,
    Withdrawn,
}

/// Which checks the integrator runs for a change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LandLane {
    Fast,
    Code,
}

/// A submitted change and the results of its landing attempts, when requested.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct LandEntry {
    pub id: String,
    pub branch: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<u64>,
    #[serde(default)]
    pub close: bool,
    pub author: String,
    pub machine: String,
    pub summary: String,
    pub head: String,
    pub submitted_at: u64,
    pub state: LandState,
    pub updated_at: u64,
    #[serde(default)]
    pub tries: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<LandLane>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_for: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub records: Vec<LandRecord>,
}

/// The checks, result, and progress notes from one landing attempt.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct LandRecord {
    pub entry: String,
    pub number: u32,
    pub worker: String,
    pub started_at: u64,
    pub ended_at: u64,
    pub outcome: String,
    pub target_before: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default)]
    pub landing: Vec<String>,
    #[serde(default)]
    pub checks: Vec<String>,
    #[serde(default)]
    pub problems: Vec<String>,
    #[serde(default)]
    pub repaired: bool,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub lane: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub slot: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regenerated: Vec<String>,
}

/// The cloud computer that runs the integrator.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct LandInstance {
    pub project: String,
    pub zone: String,
    pub name: String,
}

/// A change the integrator is currently checking.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct LandBusy {
    pub slot: String,
    pub lane: String,
    pub entry: String,
    pub since: u64,
}

/// The integrator's latest report of its work and capacity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct LandHeartbeat {
    pub machine: String,
    pub at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<LandInstance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<LandBusy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<u32>,
    #[serde(default)]
    pub draining: bool,
}

/// Whether the integrator has sent a recent heartbeat, and its latest report.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct LandIntegrator {
    pub running: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat: Option<LandHeartbeat>,
}

/// The integrator and changes waiting, landing, or returned to their authors.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct LandStatus {
    #[serde(default)]
    pub queue: String,
    pub integrator: LandIntegrator,
    #[serde(default)]
    pub waiting: Vec<LandEntry>,
    #[serde(default)]
    pub landing: Vec<LandEntry>,
    #[serde(default)]
    pub failed: Vec<LandEntry>,
}

/// A signed-in client's access to the shared landing queue.
pub struct Land<'a> {
    client: &'a Client,
    session: &'a Session,
}

impl Client {
    /// Returns the landing client for `session` on this deployment.
    pub fn land<'a>(&'a self, session: &'a Session) -> Result<Land<'a>, Error> {
        self.check_session(session)?;
        Ok(Land {
            client: self,
            session,
        })
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
}

impl Land<'_> {
    /// Submits a pushed branch. Reuse `request_id` and the same body after an uncertain response.
    pub async fn submit(
        &self,
        request_id: &str,
        entry: LandSubmit,
        cancel: &CancellationToken,
    ) -> Result<LandEntry, Error> {
        self.client.check_session(self.session)?;
        if !valid_id(request_id) {
            return Err(Error::InvalidReference);
        }
        let body = serde_json::to_value(entry).map_err(|_| Error::InvalidReference)?;
        tokio::select! { biased;
            _ = cancel.cancelled() => Err(Error::Cancelled),
            result = self.client.request_with_idempotency(
                Method::POST, &["land"], body, self.session.token(), request_id,
            ) => result,
        }
    }

    /// Reads the integrator's state and waiting, active, and failed changes.
    pub async fn status(&self, cancel: &CancellationToken) -> Result<LandStatus, Error> {
        self.client.check_session(self.session)?;
        tokio::select! { biased;
            _ = cancel.cancelled() => Err(Error::Cancelled),
            result = self.client.request(
                Method::GET, &["land"], &[], None, Some(self.session.token()),
            ) => result,
        }
    }

    /// Fetches a change and all recorded landing attempts.
    pub async fn show(&self, id: &str, cancel: &CancellationToken) -> Result<LandEntry, Error> {
        self.client.check_session(self.session)?;
        if !valid_id(id) {
            return Err(Error::InvalidReference);
        }
        let path = ["land", id];
        tokio::select! { biased;
            _ = cancel.cancelled() => Err(Error::Cancelled),
            result = self.client.request(
                Method::GET, &path, &[], None, Some(self.session.token()),
            ) => result,
        }
    }

    /// Removes a waiting change from the queue.
    pub async fn withdraw(&self, id: &str, cancel: &CancellationToken) -> Result<(), Error> {
        self.client.check_session(self.session)?;
        if !valid_id(id) {
            return Err(Error::InvalidReference);
        }
        let path = ["land", id];
        tokio::select! { biased;
            _ = cancel.cancelled() => Err(Error::Cancelled),
            result = self.client.request_empty(
                Method::DELETE, &path, self.session.token(),
            ) => result,
        }
    }
}
