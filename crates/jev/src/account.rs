//! The caller's account surface — the session the bearer names, the
//! account and its workspaces, the monetary balance, and workspace
//! usage.
//!
//! Every route here is conditional: sessions, accounts, and usage mount
//! under the deployment's `accounts` document, and balance mounts only
//! under monetary admission. Where a route is absent the call is a
//! typed refusal — `not_a_session` for an `oak_` key's session read,
//! the generic 404 family elsewhere — never a fabricated document.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::Result;
use crate::client::Client;
use crate::error::Error;
use crate::options::CallOptions;
use crate::transport::parse_body;
use reqwest::Method;

/// The session routes.
const SESSION_PATH: &str = "/v1/session";
/// The account route.
const ACCOUNT_PATH: &str = "/v1/account";
/// The balance route.
const BALANCE_PATH: &str = "/v1/balance";
/// The workspaces route family.
const WORKSPACES_PATH: &str = "/v1/workspaces";

/// The account surface, scoped to its client.
#[derive(Debug)]
pub struct Account<'a> {
    client: &'a Client,
}

impl<'a> Account<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// `GET /v1/session` — the session the bearer token names. An
    /// `oak_` key has no session: the call answers `not_a_session`
    /// (400), which the caller reads as the credential kind, not a
    /// failure of the deployment.
    ///
    /// # Errors
    ///
    /// Returns the shared call errors and [`Error::ResponseValidation`]
    /// on an undecodable body.
    pub async fn session(&self) -> Result<SessionView> {
        self.read(SESSION_PATH, &CallOptions::new()).await
    }

    /// `GET /v1/account` — the caller's account and every workspace it
    /// belongs to: the workspace-switching menu.
    ///
    /// # Errors
    ///
    /// Returns the shared call errors; a credential with no account is
    /// [`Error::Api`] with `no_account` or `membership_required`.
    pub async fn details(&self) -> Result<AccountDetails> {
        self.read(ACCOUNT_PATH, &CallOptions::new()).await
    }

    /// `GET /v1/balance` — the named workspace's monetary position and
    /// the configured doors' price versions, under `X-Workspace-Id`.
    ///
    /// # Errors
    ///
    /// Returns the shared call errors; a workspace with no monetary
    /// account is [`Error::Api`] with `account_missing`, and a route
    /// under a deployment without monetary admission does not exist at
    /// all.
    pub async fn balance(&self, workspace: &str) -> Result<BalanceView> {
        self.read(BALANCE_PATH, &CallOptions::new().workspace(workspace)?)
            .await
    }

    /// `GET /v1/workspaces/{workspace}/usage` — the workspace's usage
    /// position: exact totals from its own receipts joined to the money
    /// and quota ledgers.
    ///
    /// # Errors
    ///
    /// Returns the shared call errors; a non-member credential is
    /// [`Error::Api`] with `not_member` or `forbidden`.
    pub async fn usage(&self, workspace: &str, query: &UsageQuery) -> Result<UsageView> {
        let mut path = format!("{WORKSPACES_PATH}/{workspace}/usage");
        let params = query.params();
        if !params.is_empty() {
            path.push('?');
            path.push_str(&params);
        }
        self.read(&path, &query.options).await
    }

    /// The read every account call shares.
    async fn read<T>(&self, path: &str, options: &CallOptions) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let raw = self
            .client
            .request_read(
                Method::GET,
                path,
                None,
                &options.headers,
                options.timeout,
                options.retry.clone(),
            )
            .await?;
        decode(&raw.bytes, raw.status, &raw.headers)
    }
}

/// The usage read's filters — each names a receipt field, so the list
/// the caller sees is the ledger's own narrowing.
#[derive(Debug, Clone, Default)]
pub struct UsageQuery {
    /// RFC 3339 lower bound on `resolved_at`.
    pub from: Option<String>,
    /// RFC 3339 upper bound on `resolved_at`.
    pub to: Option<String>,
    /// One key's calls only.
    pub key: Option<String>,
    /// One model's calls only.
    pub model: Option<String>,
    /// One outcome's calls only.
    pub outcome: Option<String>,
    /// One capacity lane's calls only.
    pub lane: Option<String>,
    /// One transport's calls only.
    pub transport: Option<String>,
    /// One job's calls only.
    pub job: Option<String>,
    /// One selection policy's calls only.
    pub policy: Option<String>,
    /// One capacity's calls only.
    pub capacity: Option<String>,
    /// The page size to ask for.
    pub limit: Option<u64>,
    /// The opaque cursor the previous page returned.
    pub cursor: Option<String>,
    /// The call's overrides.
    pub options: CallOptions,
}

impl UsageQuery {
    /// No filters — the workspace's whole window.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// RFC 3339 bounds on `resolved_at`.
    #[must_use]
    pub fn window(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.from = Some(from.into());
        self.to = Some(to.into());
        self
    }

    /// One model's calls only.
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// One outcome's calls only.
    #[must_use]
    pub fn outcome(mut self, outcome: impl Into<String>) -> Self {
        self.outcome = Some(outcome.into());
        self
    }

    /// The page size to ask for.
    #[must_use]
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Continue from a previous page's cursor.
    #[must_use]
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Options for this call.
    #[must_use]
    pub fn options(mut self, options: CallOptions) -> Self {
        self.options = options;
        self
    }

    /// The query string these filters encode.
    fn params(&self) -> String {
        let mut params = url::form_urlencoded::Serializer::new(String::new());
        for (name, value) in [
            ("from", &self.from),
            ("to", &self.to),
            ("key", &self.key),
            ("model", &self.model),
            ("outcome", &self.outcome),
            ("lane", &self.lane),
            ("transport", &self.transport),
            ("job", &self.job),
            ("policy", &self.policy),
            ("capacity", &self.capacity),
            ("cursor", &self.cursor),
        ] {
            if let Some(value) = value {
                params.append_pair(name, value);
            }
        }
        if let Some(limit) = self.limit {
            params.append_pair("limit", &limit.to_string());
        }
        params.finish()
    }
}

/// `GET /v1/session`'s answer.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SessionView {
    /// The session the bearer names.
    pub session: SessionInfo,
    /// The anonymous lane's counters, for a funded anonymous session.
    #[serde(default)]
    pub budget: Option<SessionBudget>,
}

/// One session as the service describes it.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SessionInfo {
    /// The session's id.
    pub id: String,
    /// `account` or `anonymous` — which lane the token runs in.
    pub kind: String,
    /// The account the session acts for, when it names one.
    #[serde(default)]
    pub account: Option<String>,
    /// When the session was created.
    #[serde(default)]
    pub created_at: Option<String>,
    /// When it ends.
    #[serde(default)]
    pub expires_at: Option<String>,
    /// `active`, `closed`, or `revoked` — its standing.
    #[serde(default)]
    pub state: Option<String>,
}

/// The funded anonymous lane's counters.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SessionBudget {
    /// The budget's id.
    pub id: String,
    /// Units left.
    #[serde(default)]
    pub remaining: Option<u64>,
    /// Units this session has drawn.
    #[serde(default)]
    pub session_drawn: Option<u64>,
    /// The per-session ceiling.
    #[serde(default)]
    pub session_cap: Option<u64>,
    /// When the budget ends.
    #[serde(default)]
    pub expires_at: Option<String>,
}

/// `GET /v1/account`'s answer.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AccountDetails {
    /// The caller's account.
    pub account: AccountInfo,
    /// Every workspace the account belongs to.
    #[serde(default)]
    pub workspaces: Vec<WorkspaceRef>,
}

/// The account record the caller may read.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AccountInfo {
    /// The account's id.
    pub id: String,
    /// Its display label.
    #[serde(default)]
    pub label: Option<String>,
    /// The principals bound to it — key ids and sessions.
    #[serde(default)]
    pub principals: Vec<Value>,
    /// When it was created.
    #[serde(default)]
    pub created: Option<String>,
}

/// One workspace as the member may see it.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct WorkspaceRef {
    /// The workspace's id.
    pub id: String,
    /// Its display name.
    #[serde(default)]
    pub name: Option<String>,
    /// `personal` or `organization`.
    #[serde(default)]
    pub kind: Option<String>,
    /// The tenant it runs under.
    #[serde(default)]
    pub tenant: Option<String>,
    /// The caller's role in it — `owner`, `admin`, or `member`.
    #[serde(default)]
    pub role: Option<String>,
    /// Its seat count.
    #[serde(default)]
    pub seats: Option<u64>,
    /// Its active member count.
    #[serde(default)]
    pub members: Option<u64>,
}

/// `GET /v1/balance`'s answer.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct BalanceView {
    /// The workspace the read is scoped to.
    pub workspace: String,
    /// Its monetary position.
    pub balance: Position,
    /// Each configured door's price version — name to `{version,
    /// policy, currency}`.
    #[serde(default)]
    pub prices: Map<String, Value>,
}

/// A workspace's monetary position, in the ledger's own units.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Position {
    /// The currency the amounts are in.
    #[serde(default)]
    pub currency: String,
    /// Everything ever credited.
    #[serde(default)]
    pub credited: u64,
    /// Currently reserved by outstanding holds.
    #[serde(default)]
    pub reserved: u64,
    /// Everything settled.
    #[serde(default)]
    pub settled: u64,
    /// Everything refunded.
    #[serde(default)]
    pub refunded: u64,
    /// What a new call may spend.
    #[serde(default)]
    pub available: u64,
    /// The configured spend limit's remainder.
    #[serde(default)]
    pub spend_remaining: u64,
}

/// `GET /v1/workspaces/{workspace}/usage`'s answer — totals the
/// workspace's own receipts prove, with the disclosure of what the
/// read cannot see.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct UsageView {
    /// The schema the report declares — `openagents.usage.v1`.
    pub v: String,
    /// The workspace the read is scoped to.
    pub workspace: String,
    /// The window the filters resolved to.
    #[serde(default)]
    pub window: Option<Value>,
    /// Calls by outcome.
    #[serde(default)]
    pub totals: UsageTotals,
    /// The measured work the calls carried.
    #[serde(default)]
    pub units: UsageUnits,
    /// The cost fields the ledgers prove.
    #[serde(default)]
    pub cost: UsageCost,
    /// Reservations still open — hold, reserved units, phase.
    #[serde(default)]
    pub outstanding: Vec<Value>,
    /// Per-model breakdown — `{model, calls, answered, retail}`.
    #[serde(default)]
    pub by_model: Vec<Value>,
    /// Per-key breakdown — `{key, calls}`.
    #[serde(default)]
    pub by_key: Vec<Value>,
    /// Per-lane breakdown — lane name to calls.
    #[serde(default)]
    pub by_lane: Map<String, Value>,
    /// Per-transport breakdown — transport name to calls.
    #[serde(default)]
    pub by_transport: Map<String, Value>,
    /// The billing entitlement the workspace runs under, when the
    /// deployment bills.
    #[serde(default)]
    pub entitlement: Option<Value>,
    /// What the read cannot attribute — sources, scale, lag, and the
    /// receipts outside its window.
    #[serde(default)]
    pub disclosure: Option<Value>,
}

/// Calls by outcome, from the workspace's own receipts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct UsageTotals {
    /// Every call in the window.
    #[serde(default)]
    pub calls: u64,
    /// Calls that answered.
    #[serde(default)]
    pub answered: u64,
    /// Calls the door refused.
    #[serde(default)]
    pub refused: u64,
    /// Calls the door could not reach.
    #[serde(default)]
    pub unavailable: u64,
    /// Calls never dispatched.
    #[serde(default)]
    pub unattempted: u64,
    /// Calls whose outcome the record cannot say.
    #[serde(default)]
    pub unknown: u64,
}

/// The measured work the calls carried.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct UsageUnits {
    /// Questions the calls asked.
    #[serde(default)]
    pub questions: u64,
    /// Request bytes the calls carried.
    #[serde(default)]
    pub input_bytes: u64,
    /// Calls whose quota reservation was measured.
    #[serde(default)]
    pub reservations_measured: u64,
    /// Calls with no measured reservation.
    #[serde(default)]
    pub unmeasured: u64,
}

/// The cost fields the money ledger proves, in its own units.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct UsageCost {
    /// The currency the amounts are in.
    #[serde(default)]
    pub currency: String,
    /// The retail total the calls priced at.
    #[serde(default)]
    pub retail: u64,
    /// What the provider side cost, where the deployment tracks it.
    #[serde(default)]
    pub provider_cost: Option<u64>,
    /// What the hosting side cost, where tracked.
    #[serde(default)]
    pub hosting_cost: Option<u64>,
    /// Refunded total.
    #[serde(default)]
    pub refunded: u64,
    /// Calls that priced.
    #[serde(default)]
    pub priced_calls: u64,
    /// Calls with no price — unmetered, never free by assumption.
    #[serde(default)]
    pub unpriced_calls: u64,
}

/// Decode one successful body, naming the route's shape on a mismatch.
fn decode<T>(bytes: &[u8], status: u16, headers: &reqwest::header::HeaderMap) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_slice::<T>(bytes).map_err(|error| Error::ResponseValidation {
        status,
        field_path: format!("account document: {error}"),
        body: parse_body(bytes).map(Box::new),
        request_id: headers
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
    })
}
