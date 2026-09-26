//! Closed bounded Gym board and recipe-control messages.
use crate::{ErrorCode, RelayPolicy, Result, error, pubkey, random_id};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use nostr::{contracts, domain::Event};
use secp256k1::{SecretKey, XOnlyPublicKey, rand::RngCore};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;
use std::str::FromStr;

pub const MAX_BODY: usize = 128 * 1024;
pub const MAX_RUNS: usize = 64;
pub const MAX_RECIPES: usize = 16;
pub const GRANT_SCHEMA: &str = "openagents.gym-grant.v1";
pub const CONNECTION_SCHEMA: &str = "openagents.gym-connection.v1";
pub const REQUEST_SCHEMA: &str = "openagents.gym-request.v1";
pub const REPLY_SCHEMA: &str = "openagents.gym-reply.v1";
pub const SNAPSHOT_SCHEMA: &str = "openagents.gym-board.v1";
pub const REQUEST_LIFETIME: u64 = 60;
pub const GRANT_LIFETIME: u64 = 30 * 24 * 60 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Evaluation,
    Agent,
    Training,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    Unknown,
    Stale,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub step: u64,
    pub value: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    pub name: String,
    pub unit: String,
    pub points: Vec<Point>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub id: String,
    pub title: String,
    pub category: Category,
    pub status: Status,
    pub completed: Option<u64>,
    pub total: Option<u64>,
    pub cost_usd: Option<f64>,
    pub elapsed_ms: Option<u64>,
    pub metrics: Vec<Metric>,
    pub source: String,
    pub provenance: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    pub wall_ms: u64,
    pub max_starts: u32,
    pub spend_limit_usd: Option<f64>,
    pub spend_enforced: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub id: String,
    pub title: String,
    pub revision: String,
    pub budget: Budget,
    pub detail: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub observed_at: u64,
    pub runs: Vec<Run>,
    pub recipes: Vec<Recipe>,
    pub notices: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchReceipt {
    pub request_id: String,
    pub run_id: String,
    pub recipe_id: String,
    pub revision: String,
    pub status: Status,
    pub submitted_at: u64,
    pub finished_at: Option<u64>,
    pub exit_code: Option<i32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub v: String,
    pub host: String,
    pub client: String,
    pub relay: String,
    pub grant: String,
    pub observe: bool,
    pub sources_digest: String,
    pub recipes: Vec<Recipe>,
    pub issued_at: u64,
    pub expires_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub v: String,
    pub host: String,
    pub client: String,
    pub relay: String,
    pub grant: String,
    pub expires_at: u64,
    pub authorization: Event,
}
impl Connection {
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() > 64 * 1024 {
            return Err(error(ErrorCode::Bounds, "Gym connection exceeds its bound"));
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(
                text.trim()
                    .strip_prefix("gym-connect:")
                    .ok_or_else(|| error(ErrorCode::Malformed, "expected a Gym connection code"))?,
            )
            .map_err(|_| error(ErrorCode::Malformed, "invalid Gym connection encoding"))?;
        decode(&bytes)
    }
    pub fn encode(&self) -> Result<String> {
        Ok(format!(
            "gym-connect:{}",
            URL_SAFE_NO_PAD.encode(encoded(self)?)
        ))
    }
    pub fn verify(&self, secret: &SecretKey, now: u64, policy: RelayPolicy) -> Result<Grant> {
        if self.v != CONNECTION_SCHEMA || self.client != pubkey(secret) {
            return Err(error(
                ErrorCode::Forbidden,
                "Gym connection belongs to another client or schema",
            ));
        }
        let grant: Grant = open(
            &self.authorization,
            secret,
            &self.host,
            &self.client,
            GRANT_SCHEMA,
        )?;
        grant.validate(policy)?;
        fresh(grant.issued_at, grant.expires_at, now)?;
        if grant.host != self.host
            || grant.client != self.client
            || grant.relay != self.relay
            || grant.grant != self.grant
            || grant.expires_at != self.expires_at
            || self.authorization.tag_values("h").collect::<Vec<_>>() != [self.grant.as_str()]
        {
            return Err(error(
                ErrorCode::Forbidden,
                "Gym connection differs from its signed grant",
            ));
        }
        Ok(grant)
    }
}
impl Grant {
    pub fn validate(&self, policy: RelayPolicy) -> Result<()> {
        digest(&self.sources_digest)?;
        public(&self.host)?;
        public(&self.client)?;
        identity(&self.grant)?;
        policy.validate(&self.relay)?;
        window(self.issued_at, self.expires_at, GRANT_LIFETIME)?;
        if self.v != GRANT_SCHEMA
            || self.host == self.client
            || !self.observe
            || self.recipes.len() > MAX_RECIPES
        {
            return Err(error(
                ErrorCode::Forbidden,
                "invalid Gym authority or profile",
            ));
        }
        for (i, recipe) in self.recipes.iter().enumerate() {
            recipe.validate()?;
            if self.recipes[..i].iter().any(|r| r.id == recipe.id) {
                return Err(error(ErrorCode::Malformed, "duplicate Gym recipe"));
            }
        }
        Ok(())
    }
}
impl Recipe {
    pub fn validate(&self) -> Result<()> {
        slug(&self.id)?;
        digest(&self.revision)?;
        text(&self.title, 160)?;
        text(&self.detail, 1024)?;
        if self.budget.wall_ms == 0
            || self.budget.wall_ms > 24 * 60 * 60 * 1000
            || self.budget.max_starts == 0
            || self.budget.max_starts > 32
            || self.budget.spend_enforced
            || self.budget.spend_limit_usd.is_some()
        {
            return Err(error(
                ErrorCode::Unsupported,
                "recipe requires unsupported bounds or monetary enforcement",
            ));
        }
        Ok(())
    }
}
impl Run {
    pub fn validate(&self) -> Result<()> {
        identity(&self.id)?;
        text(&self.title, 160)?;
        text(&self.source, 160)?;
        text(&self.provenance, 1024)?;
        if self.cost_usd.is_some_and(|n| !n.is_finite() || n < 0.0)
            || self.completed.zip(self.total).is_some_and(|(a, b)| a > b)
            || self.metrics.len() > 4
        {
            return Err(error(ErrorCode::Malformed, "invalid Gym measurement"));
        }
        for metric in &self.metrics {
            text(&metric.name, 64)?;
            text(&metric.unit, 32)?;
            if metric.points.len() > 64
                || metric.points.iter().any(|p| !p.value.is_finite())
                || metric.points.windows(2).any(|p| p[0].step >= p[1].step)
            {
                return Err(error(
                    ErrorCode::Bounds,
                    "invalid bounded Gym metric series",
                ));
            }
        }
        Ok(())
    }
}
impl Snapshot {
    pub fn validate(&self, now: u64) -> Result<()> {
        if self.observed_at > now
            || self.runs.len() > MAX_RUNS
            || self.recipes.len() > MAX_RECIPES
            || self.notices.len() > 16
        {
            return Err(error(ErrorCode::Bounds, "Gym snapshot exceeds its bounds"));
        }
        for (i, run) in self.runs.iter().enumerate() {
            run.validate()?;
            if self.runs[..i].iter().any(|r| r.id == run.id) {
                return Err(error(ErrorCode::Conflict, "duplicate Gym run"));
            }
        }
        for (i, r) in self.recipes.iter().enumerate() {
            r.validate()?;
            if self.recipes[..i].iter().any(|old| old.id == r.id) {
                return Err(error(ErrorCode::Conflict, "duplicate Gym recipe"));
            }
        }
        for notice in &self.notices {
            text(notice, 256)?;
        }
        encoded(self)?;
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Query {
    Snapshot,
    Launch {
        request_id: String,
        recipe_id: String,
        revision: String,
    },
}
impl Query {
    pub fn validate(&self) -> Result<()> {
        if let Self::Launch {
            request_id,
            recipe_id,
            revision,
        } = self
        {
            identity(request_id)?;
            slug(recipe_id)?;
            digest(revision)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub v: String,
    pub request: String,
    pub grant: String,
    pub authorization: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub query: Query,
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        if self.v != REQUEST_SCHEMA {
            return Err(error(ErrorCode::Unsupported, "unsupported Gym request"));
        }
        identity(&self.request)?;
        identity(&self.grant)?;
        identity(&self.authorization)?;
        window(self.issued_at, self.expires_at, REQUEST_LIFETIME)?;
        self.query.validate()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Response {
    Snapshot(Box<Snapshot>),
    Launch(LaunchReceipt),
    Refused(ErrorCode),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub v: String,
    pub request: String,
    pub request_event: String,
    pub grant: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub response: Response,
}

pub(crate) fn identity(id: &str) -> Result<()> {
    coder_connect::protocol::identity(id)
}
pub(crate) fn digest(id: &str) -> Result<()> {
    identity(
        id.strip_prefix("sha256:")
            .ok_or_else(|| error(ErrorCode::Malformed, "expected a SHA-256 digest"))?,
    )
}
pub(crate) fn public(id: &str) -> Result<()> {
    identity(id)?;
    XOnlyPublicKey::from_str(id)
        .map(|_| ())
        .map_err(|_| error(ErrorCode::Malformed, "invalid public key"))
}
pub(crate) fn slug(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
    {
        return Err(error(ErrorCode::Malformed, "invalid Gym recipe identifier"));
    }
    Ok(())
}
pub(crate) fn text(value: &str, max: usize) -> Result<()> {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(error(
            ErrorCode::Bounds,
            "Gym display text exceeds its bound",
        ));
    }
    Ok(())
}
pub(crate) fn window(start: u64, end: u64, max: u64) -> Result<()> {
    if end <= start || end - start > max || end > 9_007_199_254_740_991 {
        return Err(error(ErrorCode::Malformed, "invalid Gym lifetime"));
    }
    Ok(())
}
pub(crate) fn fresh(start: u64, end: u64, now: u64) -> Result<()> {
    if start > now || end <= now {
        return Err(error(ErrorCode::Expired, "Gym grant or request expired"));
    }
    Ok(())
}
pub(crate) fn encoded(value: &impl Serialize) -> Result<Vec<u8>> {
    let value = serde_json::to_value(value)
        .map_err(|_| error(ErrorCode::Malformed, "Gym serialization failed"))?;
    let bytes = contracts::jcs(&value)
        .map_err(|_| error(ErrorCode::Malformed, "Gym canonical serialization failed"))?;
    if bytes.len() > MAX_BODY {
        return Err(error(ErrorCode::Bounds, "Gym body exceeds its byte bound"));
    }
    Ok(bytes)
}
pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let value = contracts::parse_strict_bounded(bytes, MAX_BODY)
        .map_err(|_| error(ErrorCode::Malformed, "invalid bounded Gym JSON"))?;
    serde_json::from_value(value).map_err(|_| error(ErrorCode::Malformed, "unsupported Gym fields"))
}
pub(crate) fn seal(
    value: &impl Serialize,
    schema: &str,
    secret: &SecretKey,
    to: &str,
    mailbox: &str,
    issued: u64,
    expires: u64,
) -> Result<Event> {
    let bytes = encoded(value)?;
    let inline: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| error(ErrorCode::Malformed, "Gym JSON"))?;
    let body = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":{"digest":contracts::digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":schema},"inline":inline,"issued_at":issued,"retain_until":expires});
    let mut nonce = [0; 32];
    secp256k1::rand::rng().fill_bytes(&mut nonce);
    nostr::private_artifact::seal(
        &body,
        secret,
        &XOnlyPublicKey::from_str(to)
            .map_err(|_| error(ErrorCode::Malformed, "invalid Gym recipient"))?,
        mailbox,
        issued,
        nonce,
    )
    .map_err(|_| error(ErrorCode::Malformed, "Gym envelope cannot be sealed"))
}
pub(crate) fn open<T: DeserializeOwned>(
    event: &Event,
    secret: &SecretKey,
    signer: &str,
    to: &str,
    schema: &str,
) -> Result<T> {
    if event.content.len() > 400 * 1024 {
        return Err(error(ErrorCode::Bounds, "Gym event exceeds its bound"));
    }
    let opened = nostr::private_artifact::open(event, secret).map_err(|_| {
        error(
            ErrorCode::Forbidden,
            "invalid Gym signature, encryption, or recipient",
        )
    })?;
    if opened.signer() != signer
        || opened.recipient() != to
        || opened.artifact().schema.as_deref() != Some(schema)
        || opened.artifact().media_type != "application/json"
        || event.created_at != opened.body().issued_at
    {
        return Err(error(ErrorCode::Forbidden, "Gym envelope identity differs"));
    }
    let value: serde_json::Value = decode(
        opened
            .inline_bytes()
            .ok_or_else(|| error(ErrorCode::Unavailable, "missing Gym inline bytes"))?,
    )?;
    if value["issued_at"].as_u64() != Some(opened.body().issued_at)
        || value["expires_at"].as_u64() != Some(opened.body().retain_until)
    {
        return Err(error(
            ErrorCode::Forbidden,
            "Gym body and envelope lifetimes differ",
        ));
    }
    serde_json::from_value(value)
        .map_err(|_| error(ErrorCode::Malformed, "unsupported Gym envelope body"))
}

pub fn new_request_id() -> String {
    random_id()
}
