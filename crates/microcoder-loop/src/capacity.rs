//! Model-provider capacity: which providers a host can reach, and which of
//! them refused work for a usage or rate limit, until when.
//!
//! A provider's usage-limit refusal is durable state, not a per-call error.
//! The Codex backend answers HTTP 429 `usage_limit_reached` with the reset
//! time, and every request before then fails the same way. This module
//! records each refusal in `capacity.json` in the task store directory,
//! one entry per provider, and answers whether a provider has capacity now.
//! The auto-start policy reads it to choose a route before it starts a
//! task, Coder's delegate door reads it to choose a provider before a
//! turn, and the Microcoder loop's failover ([`crate::failover`]) writes it
//! when a generation is refused, so a later task or turn does not repeat a
//! request that cannot succeed. `coder::task::capacity` re-exports this
//! module, so the task store and the loop read one book.
//!
//! The book holds no credential, prompt, or response text: the provider,
//! the kind of limit, when it was observed, and when it resets. A refusal
//! whose reset the provider did not report holds for
//! [`UNKNOWN_RESET_HOLD`] seconds. A missing or unreadable book means no
//! recorded refusal: the next request finds out again.
//!
//! [`probe`] reports whether a provider has a usable local login, without
//! a network request and without reading any secret into a log.

use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The book in the task store directory.
pub const FILE: &str = "capacity.json";
/// The book's format.
pub const SCHEMA: &str = "openagents.coder.provider-capacity.v1";
/// How long a Codex or Claude refusal with no reported reset holds, in
/// seconds.
pub const UNKNOWN_RESET_HOLD: u64 = 30 * 60;
/// How long a cloud (Vertex) refusal with no reported reset holds, in
/// seconds. The cloud's throttling is per minute, so a half-hour hold
/// would pass it over for no reason.
pub const VERTEX_UNKNOWN_RESET_HOLD: u64 = 5 * 60;
/// The latest reset the book accepts: 31 days after the refusal. A later
/// reported time is held to this bound.
pub const MAX_HOLD: u64 = 31 * 24 * 60 * 60;
/// The result ending a repository run records when no admitted provider
/// had capacity. The task owner keeps it as the run's `ending`.
pub const NO_CAPACITY_ENDING: &str = "no_capacity";

/// Where the cloud fallback ([`Provider::Vertex`]) sends a step: the
/// OpenAgents relay, where the OpenAgents cloud worker answers NIP-CJ
/// conversation jobs. The model credential stays on the worker.
pub const CLOUD_ENDPOINT: &str = "wss://relay.openagents.com";

/// The variable that turns the cloud fallback off on a host: `off`. Any
/// other value, or none, leaves it on.
pub const CLOUD_VAR: &str = "CODER_CLOUD";

/// The worker's day: a caller's daily quota resets at UTC midnight.
const DAY: u64 = 86_400;

/// How long a cloud rate limit or busy worker with no reported wait holds:
/// the worker's per-key window is one minute.
pub const CLOUD_UNKNOWN_RETRY: u64 = 60;

/// The endpoint a grant names for a Devin route: the local `devin acp`
/// process, which reaches Devin's service with its own login. It is not a
/// URL, and no request goes to it from Microcoder.
pub const DEVIN_ENDPOINT: &str = "local:devin-acp";

/// The endpoint a grant names for an OpenCode route: the local `opencode
/// acp` process, which reaches the route's own provider with OpenCode's
/// login. It is not a URL, and no request goes to it from Microcoder.
pub const OPENCODE_ENDPOINT: &str = "local:opencode-acp";

/// The endpoint a grant names for a Grok Build route: the local
/// `grok agent stdio` process, which uses Grok Build's own login. It is
/// not a URL, and no request goes to it from Microcoder.
pub const GROK_ENDPOINT: &str = "local:grok-acp";

/// The endpoint a grant names for a Claude route that runs as one lean
/// Claude Code session (#10246) instead of Microcoder's step loop: the
/// local `claude -p` process, briefed by Jev, with six tools, the trimmed
/// system prompt, and the five-minute prompt cache. It uses Claude Code's
/// own login; it is not a URL, and no request goes to it from Microcoder.
pub const CLAUDE_SESSION_ENDPOINT: &str = "local:claude-code-session";

/// A model provider a repository run can generate through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    /// The operator's Codex login (ChatGPT backend).
    Codex,
    /// The operator's Claude Code login, through the `claude` binary.
    Claude,
    /// Vertex through the OpenAgents cloud: Coder's no-setup fallback.
    /// The host sends each step to the OpenAgents cloud worker over
    /// [`CLOUD_ENDPOINT`], signed by the host's own Nostr key; the worker
    /// holds the model credential and meters each caller key server-side.
    /// Nothing on the host configures it and no Google credential is read.
    Vertex,
    /// The local Devin CLI over ACP (`devin acp`), with its own login. It
    /// is a whole coding agent, not a model a loop step generates through:
    /// a repository run on a Devin route hands Devin the turn.
    Devin,
    /// OpenCode over ACP (`opencode acp`), with OpenCode's own logins. Like
    /// Devin it is a whole coding agent: a repository run on an OpenCode
    /// route hands OpenCode the turn. A route names OpenCode's own
    /// `provider/model`, and one refusal holds every OpenCode route.
    #[serde(rename = "opencode")]
    OpenCode,
    /// Grok Build over ACP (`grok agent stdio`), with its own login. Like
    /// Devin it is a whole coding agent: a repository run on a Grok Build
    /// route hands Grok Build the turn.
    Grok,
}

impl Provider {
    /// Every provider, in a fixed order.
    pub const ALL: [Provider; 6] = [
        Provider::Codex,
        Provider::Claude,
        Provider::Vertex,
        Provider::Devin,
        Provider::OpenCode,
        Provider::Grok,
    ];

    /// The providers with a usage endpoint a probe can ask.
    pub const PROBED: [Provider; 2] = [Provider::Codex, Provider::Claude];

    /// The provider an adapter configuration names, or `None` for one
    /// without durable capacity, such as `synthetic` fixtures. The names
    /// are the configuration's closed set, not free text.
    #[must_use]
    pub fn from_config(name: &str) -> Option<Provider> {
        match name {
            "codex" => Some(Provider::Codex),
            "claude" => Some(Provider::Claude),
            "vertex" => Some(Provider::Vertex),
            "devin" => Some(Provider::Devin),
            "opencode" => Some(Provider::OpenCode),
            "grok" => Some(Provider::Grok),
            _ => None,
        }
    }

    /// The name an adapter configuration uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Provider::Codex => "codex",
            Provider::Claude => "claude",
            Provider::Vertex => "vertex",
            Provider::Devin => "devin",
            Provider::OpenCode => "opencode",
            Provider::Grok => "grok",
        }
    }

    /// The generation endpoint a repository grant names for this provider.
    #[must_use]
    pub const fn endpoint(self) -> &'static str {
        match self {
            Provider::Codex => codex_transport::codex::BASE_URL,
            Provider::Claude => "https://api.anthropic.com",
            Provider::Vertex => CLOUD_ENDPOINT,
            Provider::Devin => DEVIN_ENDPOINT,
            Provider::OpenCode => OPENCODE_ENDPOINT,
            Provider::Grok => GROK_ENDPOINT,
        }
    }

    /// How long a refusal that reported no reset holds, in seconds.
    #[must_use]
    pub const fn unknown_hold(self) -> u64 {
        match self {
            Provider::Codex
            | Provider::Claude
            | Provider::Devin
            | Provider::OpenCode
            | Provider::Grok => UNKNOWN_RESET_HOLD,
            Provider::Vertex => VERTEX_UNKNOWN_RESET_HOLD,
        }
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What kind of limit refused the work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The login's plan used its allowance for a window, such as Codex's
    /// `usage_limit_reached`.
    UsageLimit,
    /// The provider refused with HTTP 429 and no typed quota detail.
    RateLimit,
}

/// The OpenAgents cloud worker's capacity codes: the closed set of NIP-CJ
/// refusal codes that mean "not now" rather than "not this request".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloudCode {
    /// The caller key sent too many jobs in the last minute.
    RateLimited,
    /// The caller key, or every caller together, used the day's jobs.
    QuotaExhausted,
    /// Every one of the worker's job slots is taken.
    Busy,
}

impl CloudCode {
    /// The capacity code `code` names exactly, or `None` for any other
    /// code, such as `limit_exceeded` (a request too large to take).
    #[must_use]
    pub fn parse(code: &str) -> Option<CloudCode> {
        match code {
            "rate_limited" => Some(CloudCode::RateLimited),
            "quota_exhausted" => Some(CloudCode::QuotaExhausted),
            "busy" => Some(CloudCode::Busy),
            _ => None,
        }
    }
}

/// One provider's refusal, as the book keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refusal {
    pub provider: Provider,
    pub kind: Kind,
    /// When the refusal was observed, in Unix seconds.
    pub observed_at: u64,
    /// When the provider said the limit resets, in Unix seconds, if it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<u64>,
    /// Until when the provider counts as having no capacity: `resets_at`,
    /// or [`UNKNOWN_RESET_HOLD`] after the refusal when the provider did
    /// not say. Never more than [`MAX_HOLD`] after the refusal.
    pub until: u64,
    /// The plan the provider named, such as `pro`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// The allowance window in minutes, when the provider named one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_minutes: Option<u64>,
    /// The fingerprint of the login that was refused
    /// ([`crate::account`]), when it has one. A reader drops the refusal
    /// once a different login is signed in (#10105).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

impl Refusal {
    /// A refusal observed at `now`, holding until the reported reset.
    #[must_use]
    pub fn new(provider: Provider, kind: Kind, now: u64, resets_at: Option<u64>) -> Refusal {
        let until = resets_at
            .filter(|at| *at > now)
            .unwrap_or(now + provider.unknown_hold())
            .min(now + MAX_HOLD);
        Refusal {
            provider,
            kind,
            observed_at: now,
            resets_at,
            until,
            plan: None,
            window_minutes: None,
            account: None,
        }
    }

    /// The refusal a Codex error response carries: a typed usage limit
    /// (`usage_limit_reached`) with its reset, else `None`. Ordinary 429s
    /// are retried by the transport and are not recorded.
    #[must_use]
    pub fn codex(status: u16, body: &str, now: u64) -> Option<Refusal> {
        let limit = codex_transport::codex::UsageLimit::parse(status, body)?;
        let resets_at = limit
            .resets_at
            .or_else(|| limit.resets_in_seconds.map(|s| now.saturating_add(s)));
        let mut refusal = Refusal::new(Provider::Codex, Kind::UsageLimit, now, resets_at);
        refusal.plan = limit.plan_type;
        refusal.window_minutes = limit.window_minutes;
        Some(refusal)
    }

    /// The refusal a Claude Code result carries: an error result whose API
    /// status is 429, or whose stream's last `rate_limit_event` is
    /// `rejected`. The rejected event's `resetsAt` is the reset, and its
    /// window (five-hour or seven-day) makes it a usage limit. Without a
    /// rejected event the reset is unknown and it holds for
    /// [`UNKNOWN_RESET_HOLD`], unless [`Refusal::with_probed_reset`] finds one.
    #[must_use]
    pub fn claude(
        is_error: bool,
        api_error_status: Option<u16>,
        rate_limit: Option<&crate::claude::RateLimit>,
        now: u64,
    ) -> Option<Refusal> {
        let rejected = rate_limit.filter(|limit| limit.rejected());
        if !is_error || (api_error_status != Some(429) && rejected.is_none()) {
            return None;
        }
        let window = rejected.and_then(|limit| limit.window);
        let minutes = window.and_then(crate::claude::LimitWindow::minutes);
        let kind = if minutes.is_some() {
            Kind::UsageLimit
        } else {
            Kind::RateLimit
        };
        let mut refusal = Refusal::new(
            Provider::Claude,
            kind,
            now,
            rejected.and_then(|limit| limit.resets_at),
        );
        refusal.window_minutes = minutes;
        Some(refusal)
    }

    /// This refusal with the reset a usage probe read, when the provider
    /// did not report one and `reading` is fresh at the refusal: the reset
    /// of the window the reading shows at its limit (the latest, when
    /// several are). Otherwise the refusal is unchanged.
    #[must_use]
    pub fn with_probed_reset(mut self, book: &crate::usage::Book) -> Refusal {
        if self.resets_at.is_some() {
            return self;
        }
        let Some(reset) = book
            .reading(self.provider, self.observed_at)
            .and_then(crate::usage::Reading::limiting_reset)
        else {
            return self;
        };
        let probed = Refusal::new(
            self.provider,
            Kind::UsageLimit,
            self.observed_at,
            Some(reset),
        );
        if probed.resets_at.is_some_and(|at| at > self.observed_at) {
            self.kind = probed.kind;
            self.resets_at = probed.resets_at;
            self.until = probed.until;
        }
        self
    }

    /// The refusal a Vertex error response carries: HTTP 429 with the
    /// `google.rpc.Status` code `RESOURCE_EXHAUSTED`, as an object or the
    /// one-element list the OpenAI-compatible endpoint answers with. A
    /// `google.rpc.QuotaFailure` detail makes it a usage limit (a quota),
    /// otherwise a rate limit. The reset is the `google.rpc.RetryInfo`
    /// detail's `retryDelay`, else the `Retry-After` header, else
    /// [`VERTEX_UNKNOWN_RESET_HOLD`]. Only these typed fields are read,
    /// never the message.
    #[must_use]
    pub fn vertex(status: u16, retry_after: Option<u64>, body: &str, now: u64) -> Option<Refusal> {
        let error = VertexError::parse(body)?;
        if status != 429 || error.status != Code::ResourceExhausted {
            return None;
        }
        let quota = error
            .details
            .iter()
            .any(|detail| detail.kind == DetailType::QuotaFailure && !detail.violations.is_empty());
        let delay = error
            .details
            .iter()
            .filter(|detail| detail.kind == DetailType::RetryInfo)
            .find_map(|detail| detail.retry_delay.as_deref().and_then(duration_seconds));
        let resets_at = delay.or(retry_after).map(|s| now.saturating_add(s));
        let kind = if quota {
            Kind::UsageLimit
        } else {
            Kind::RateLimit
        };
        Some(Refusal::new(Provider::Vertex, kind, now, resets_at))
    }

    /// The refusal an OpenCode error carries: an `APIError` whose HTTP
    /// status is 429, which OpenCode reports only after its own retries
    /// ran out. The reset is the provider's `retry-after-ms` or
    /// `retry-after` header (seconds), else [`UNKNOWN_RESET_HOLD`]. Only
    /// the error's name, status, and those two headers are read, never
    /// the message. OpenCode saves the error on the failed assistant
    /// message, and `opencode run --format json` prints it in its `error`
    /// event.
    #[must_use]
    pub fn opencode(error: &Value, now: u64) -> Option<Refusal> {
        #[derive(Deserialize)]
        struct Error {
            name: String,
            #[serde(default)]
            data: Data,
        }
        #[derive(Default, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Data {
            #[serde(default)]
            status_code: Option<u16>,
            #[serde(default)]
            response_headers: std::collections::BTreeMap<String, Value>,
        }
        let error: Error = serde_json::from_value(error.clone()).ok()?;
        if error.name != "APIError" || error.data.status_code != Some(429) {
            return None;
        }
        let header = |name: &str| {
            error
                .data
                .response_headers
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .and_then(|(_, value)| value.as_str())
                .and_then(|value| value.trim().parse::<f64>().ok())
                .filter(|value| value.is_finite() && *value >= 0.0)
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let delay = header("retry-after-ms")
            .map(|ms| (ms / 1000.0).ceil() as u64)
            .or_else(|| header("retry-after").map(|s| s.ceil() as u64));
        Some(Refusal::new(
            Provider::OpenCode,
            Kind::RateLimit,
            now,
            delay.map(|s| now.saturating_add(s)),
        ))
    }

    /// The refusal the OpenAgents cloud worker's typed NIP-CJ code carries
    /// ([`CloudCode`]), for [`Provider::Vertex`]: `rate_limited` and `busy`
    /// are rate limits, `quota_exhausted` is a usage limit. The reset is
    /// the worker's `retry_after_ms`, else [`CLOUD_UNKNOWN_RETRY`] for a
    /// rate limit, else the next UTC midnight, when the worker's daily
    /// quota resets. Any other code is not a capacity refusal: the caller
    /// reports it as the step's error.
    #[must_use]
    pub fn cloud(code: &str, retry_after_ms: Option<u64>, now: u64) -> Option<Refusal> {
        let code = CloudCode::parse(code)?;
        let told = retry_after_ms.map(|ms| now.saturating_add(ms.div_ceil(1_000).max(1)));
        let (kind, resets_at) = match code {
            CloudCode::RateLimited | CloudCode::Busy => (
                Kind::RateLimit,
                told.unwrap_or(now.saturating_add(CLOUD_UNKNOWN_RETRY)),
            ),
            CloudCode::QuotaExhausted => (
                Kind::UsageLimit,
                told.unwrap_or((now / DAY + 1).saturating_mul(DAY)),
            ),
        };
        Some(Refusal::new(Provider::Vertex, kind, now, Some(resets_at)))
    }

    /// Whether the refusal still holds at `now`.
    #[must_use]
    pub fn holds(&self, now: u64) -> bool {
        now < self.until
    }
}

/// A `google.rpc.Code` name, as far as a refusal needs it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
enum Code {
    #[serde(rename = "RESOURCE_EXHAUSTED")]
    ResourceExhausted,
    #[default]
    #[serde(other)]
    Other,
}

/// A `google.rpc.Status` detail's `@type`, as far as a refusal needs it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
enum DetailType {
    #[serde(rename = "type.googleapis.com/google.rpc.QuotaFailure")]
    QuotaFailure,
    #[serde(rename = "type.googleapis.com/google.rpc.RetryInfo")]
    RetryInfo,
    #[default]
    #[serde(other)]
    Other,
}

/// A Google API error's typed fields (`google.rpc.Status`).
#[derive(Deserialize)]
struct VertexError {
    #[serde(default)]
    status: Code,
    #[serde(default)]
    details: Vec<VertexDetail>,
}

#[derive(Deserialize)]
struct VertexDetail {
    #[serde(rename = "@type", default)]
    kind: DetailType,
    #[serde(default, rename = "retryDelay")]
    retry_delay: Option<String>,
    #[serde(default)]
    violations: Vec<serde::de::IgnoredAny>,
}

impl VertexError {
    /// `{"error":{...}}`, or `[{"error":{...}}]` as the OpenAI-compatible
    /// endpoint answers.
    fn parse(body: &str) -> Option<VertexError> {
        #[derive(Deserialize)]
        struct Envelope {
            error: VertexError,
        }
        // The list comes first: serde reads a struct from a sequence too,
        // so `One` would take a list and miss its fields.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Body {
            List(Vec<Envelope>),
            One(Envelope),
        }
        match serde_json::from_str::<Body>(body).ok()? {
            Body::One(envelope) => Some(envelope.error),
            Body::List(list) => list.into_iter().next().map(|envelope| envelope.error),
        }
    }
}

/// Whole seconds in a protobuf JSON duration such as `41s` or `0.5s`,
/// rounded up.
fn duration_seconds(text: &str) -> Option<u64> {
    let number = text.strip_suffix('s')?;
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let whole: u64 = whole.parse().ok()?;
    let partial = fraction.bytes().any(|b| b != b'0');
    Some(whole + u64::from(partial))
}

/// Every recorded refusal, at most one per provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub schema: String,
    pub refusals: Vec<Refusal>,
}

impl Default for Book {
    fn default() -> Self {
        Book {
            schema: SCHEMA.into(),
            refusals: Vec::new(),
        }
    }
}

impl Book {
    /// The book in `dir`, less each refusal observed on another login than
    /// the one signed in now ([`crate::account`], #10105) and each refusal
    /// a later usage reading in the same directory lifts
    /// ([`Book::lifted_by`]). Missing, unreadable, or malformed is empty.
    #[must_use]
    pub fn load(dir: &Path) -> Book {
        Book::load_with(dir, crate::account::identify)
    }

    /// [`Book::load`] with `identify` naming each provider's login now.
    #[must_use]
    pub fn load_with(dir: &Path, identify: crate::account::Identify) -> Book {
        let mut book = std::fs::read(dir.join(FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Book>(&bytes).ok())
            .filter(|book| book.schema == SCHEMA)
            .unwrap_or_default();
        book.refusals.retain(|refusal| {
            crate::account::applies(
                refusal.account.as_deref(),
                crate::account::current(dir, refusal.provider, identify).as_deref(),
            )
        });
        book.lifted_by(&crate::usage::Book::load_with(dir, identify))
    }

    /// The book without the refusals `usage` lifts
    /// ([`crate::usage::Book::lifts`]): a usage limit that a probe reading
    /// taken after it shows is over. Every reader loads the book this way,
    /// so routing, the start card's reason, and the fallback list agree
    /// with the engine readings (#10073).
    #[must_use]
    pub fn lifted_by(mut self, usage: &crate::usage::Book) -> Book {
        self.refusals.retain(|refusal| !usage.lifts(refusal));
        self
    }

    /// The refusal that keeps `provider` from work at `now`, if one holds.
    #[must_use]
    pub fn blocking(&self, provider: Provider, now: u64) -> Option<&Refusal> {
        self.refusals
            .iter()
            .find(|refusal| refusal.provider == provider && refusal.holds(now))
    }

    /// Whether `provider` has capacity at `now` as far as the book knows.
    #[must_use]
    pub fn has_capacity(&self, provider: Provider, now: u64) -> bool {
        self.blocking(provider, now).is_none()
    }

    /// The earliest time one of `providers` has capacity again, if every
    /// one of them is blocked at `now`.
    #[must_use]
    pub fn earliest_reset(&self, providers: &[Provider], now: u64) -> Option<u64> {
        providers
            .iter()
            .map(|provider| self.blocking(*provider, now).map(|r| r.until))
            .collect::<Option<Vec<u64>>>()
            .and_then(|until| until.into_iter().min())
    }

    fn merge(&mut self, refusal: Refusal, now: u64) {
        self.refusals
            .retain(|kept| kept.provider != refusal.provider && kept.holds(now));
        self.refusals.push(refusal);
        self.refusals.sort_by_key(|refusal| refusal.provider);
    }
}

/// Record `refusal` in the book in `dir`, replacing the provider's earlier
/// entry and dropping entries that no longer hold. The refusal keeps the
/// fingerprint of the login signed in now ([`crate::account`]), so it
/// stops holding once another login is (#10105). The file is `0600`,
/// written under an exclusive lock so concurrent runs do not lose entries.
///
/// # Errors
/// Reports a failed read or write.
pub fn record(dir: &Path, refusal: Refusal) -> Result<Book, String> {
    record_with(dir, refusal, crate::account::identify)
}

/// [`record`] with `identify` naming each provider's login now.
///
/// # Errors
/// Reports a failed read or write.
pub fn record_with(
    dir: &Path,
    mut refusal: Refusal,
    identify: crate::account::Identify,
) -> Result<Book, String> {
    let now = refusal.observed_at;
    let path = dir.join(FILE);
    let mut file = open_private(dir, &path)?;
    if refusal.account.is_none() {
        refusal.account = crate::account::current(dir, refusal.provider, identify);
    }
    file.lock()
        .map_err(|_| format!("cannot lock {}", path.display()))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|_| format!("cannot read {}", path.display()))?;
    let mut book = serde_json::from_slice::<Book>(&bytes)
        .ok()
        .filter(|book| book.schema == SCHEMA)
        .unwrap_or_default();
    book.merge(refusal, now);
    let bytes = serde_json::to_vec_pretty(&book).map_err(|e| e.to_string())?;
    file.set_len(0)
        .and_then(|()| file.rewind())
        .and_then(|()| file.write_all(&bytes))
        .and_then(|()| file.sync_all())
        .map_err(|_| format!("cannot write {}", path.display()))?;
    Ok(book)
}

/// Opens `path` in `dir` for reading and writing, creating both where
/// missing: the directory `0700` and the file `0600` on Unix, and on
/// Windows a directory made the user's alone, whose files inherit that.
///
/// # Errors
/// Reports a directory or file that cannot be made or opened.
pub(crate) fn open_private(dir: &Path, path: &Path) -> Result<std::fs::File, String> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .map_err(|_| format!("cannot create {}", dir.display()))?;
        options.mode(0o600);
    }
    #[cfg(windows)]
    private_fs::create_dir_all(dir).map_err(|_| format!("cannot create {}", dir.display()))?;
    options
        .open(path)
        .map_err(|_| format!("cannot open {}", path.display()))
}

/// Whether a provider can be used from this host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Connection {
    /// A usable local login was found.
    Connected,
    /// No usable login: why, without secret material.
    Missing(String),
}

impl Connection {
    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(self, Connection::Connected)
    }
}

/// Whether `provider` has a usable login on this host, decided locally:
///
/// - **Codex**: `$CODEX_HOME/auth.json` or `~/.codex/auth.json` holds a
///   ChatGPT login whose access token is not about to expire, as
///   `codex_transport::codex::Login::load` requires before a request.
/// - **Claude**: a `claude` binary (`CLAUDE_BIN`, else
///   [`crate::claude::locate`]: `PATH`, the folders Claude Code, npm, and
///   Homebrew install it in, or the login shell's `PATH`) and a Claude Code sign-in: the account record in
///   `~/.claude.json`, or `~/.claude/.credentials.json` (both under
///   `CLAUDE_CONFIG_DIR` when it is set). The credential itself, in the
///   macOS keychain or that file, is not read.
///
/// - **Devin**: a `devin` binary (`DEVIN_BIN`, `PATH`, or
///   `~/.local/bin/devin`) and Devin's stored CLI login,
///   `~/.local/share/devin/credentials.toml` (or under `XDG_DATA_HOME`),
///   which is checked for presence and never read.
///
/// - **OpenCode**: an `opencode` binary (`OPENCODE_BIN`, `PATH`,
///   `~/.opencode/bin`, or `~/.local/bin`). Each route's own provider login
///   is OpenCode's to find; `acp_client::opencode::login` names a stored or
///   configured one by name without reading it.
///
/// - **Grok Build**: a `grok` binary (`GROK_BIN`, `PATH`, `~/.local/bin`, or
///   `~/.grok/bin`) and a login: `$GROK_HOME/auth.json` or
///   `~/.grok/auth.json` (presence and size only, never read), or a
///   non-empty `XAI_API_KEY` (presence only, never logged).
///
/// - **Vertex** (through the OpenAgents cloud): always connected, since it
///   needs nothing on this host but the host's own Nostr key, unless
///   `CODER_CLOUD=off` turns it off. No token file or Google credential is
///   read: the credential stays on the cloud worker.
///
/// A connected provider can still refuse: the login may have been revoked.
/// That refusal then ends the generation as any other error does.
#[must_use]
pub fn probe(provider: Provider) -> Connection {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match provider {
        Provider::Codex => {
            let Some(path) = codex_transport::codex::Login::default_path() else {
                return Connection::Missing("HOME is not set".into());
            };
            match codex_transport::codex::Login::load(&path) {
                Ok(_) => Connection::Connected,
                Err(error) => Connection::Missing(error.to_string()),
            }
        }
        Provider::Vertex => cloud_connection(std::env::var(CLOUD_VAR).ok().as_deref()),
        Provider::OpenCode => {
            let variable = |name: &str| std::env::var_os(name);
            if acp_client::opencode::binary(&variable).is_none() {
                Connection::Missing(
                    "no opencode binary in OPENCODE_BIN, PATH, ~/.opencode/bin, or ~/.local/bin"
                        .into(),
                )
            } else {
                Connection::Connected
            }
        }
        Provider::Devin => {
            let variable = |name: &str| std::env::var_os(name);
            if acp_client::devin::binary(&variable).is_none() {
                return Connection::Missing(
                    "no devin binary in DEVIN_BIN, PATH, or ~/.local/bin".into(),
                );
            }
            if acp_client::devin::signed_in(&variable) {
                Connection::Connected
            } else {
                Connection::Missing("the Devin CLI is not signed in; run `devin auth login`".into())
            }
        }
        Provider::Grok => {
            let variable = |name: &str| std::env::var_os(name);
            if acp_client::grok::binary(&variable).is_none() {
                return Connection::Missing(
                    "no grok binary in GROK_BIN, PATH, ~/.local/bin, or ~/.grok/bin".into(),
                );
            }
            if acp_client::grok::signed_in(&variable) {
                Connection::Connected
            } else {
                Connection::Missing(
                    "Grok Build is not signed in; run `grok` and log in, or set XAI_API_KEY".into(),
                )
            }
        }
        Provider::Claude => {
            if claude_binary().is_none() {
                return Connection::Missing(
                    "no claude binary in CLAUDE_BIN, PATH, the install folders, or the login shell's PATH"
                        .into(),
                );
            }
            let Some(home) = home else {
                return Connection::Missing("HOME is not set".into());
            };
            if claude_signed_in(&home) {
                Connection::Connected
            } else {
                Connection::Missing("Claude Code is not signed in; run `claude` and log in".into())
            }
        }
    }
}

/// Whether the cloud fallback is on, from `CODER_CLOUD`'s value: only the
/// exact value `off` turns it off. It reads no file and no credential.
#[must_use]
pub fn cloud_connection(setting: Option<&str>) -> Connection {
    match setting.map(str::trim) {
        Some("off") => Connection::Missing(format!("{CLOUD_VAR}=off turns the cloud fallback off")),
        _ => Connection::Connected,
    }
}

fn claude_binary() -> Option<PathBuf> {
    if let Some(named) = std::env::var_os("CLAUDE_BIN").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(named)).filter(|path| path.is_file());
    }
    crate::claude::locate()
}

/// Claude Code keeps the signed-in account's metadata (not the credential)
/// in `~/.claude.json` as `oauthAccount`; `CLAUDE_CONFIG_DIR` moves both
/// it and the credentials file, as it does for Claude Code itself.
fn claude_signed_in(home: &Path) -> bool {
    #[derive(Deserialize)]
    struct State {
        #[serde(default, rename = "oauthAccount")]
        oauth_account: Option<serde::de::IgnoredAny>,
    }
    let config = std::env::var_os("CLAUDE_CONFIG_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from);
    let (credentials, state) = match &config {
        Some(dir) => (dir.join(".credentials.json"), dir.join(".claude.json")),
        None => (
            home.join(".claude/.credentials.json"),
            home.join(".claude.json"),
        ),
    };
    if credentials.is_file() {
        return true;
    }
    std::fs::read(state)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<State>(&bytes).ok())
        .is_some_and(|state| state.oauth_account.is_some())
}

/// `YYYY-MM-DD HH:MM UTC` for Unix seconds, for host-built text.
#[must_use]
pub fn utc(seconds: u64) -> String {
    format!(
        "{} {:02}:{:02} UTC",
        nostr::git_sign::utc_date(seconds),
        seconds % 86_400 / 3_600,
        seconds % 3_600 / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The owner's Mac on 2026-09-30 (#10073): Codex refused on 09-28 with
    /// a week's limit until 10-03, and the host's probe two days later read
    /// its window at 8% with a new reset. The later reading lifts the
    /// refusal for every reader; a reading at the limit, one older than
    /// the refusal, or a rate limit does not.
    #[test]
    fn a_later_reading_under_the_limit_lifts_a_usage_refusal() {
        use crate::usage::{self, Entry, Reading, Window, WindowName};
        let dir = tempfile::tempdir().unwrap();
        let refused = Refusal::new(
            Provider::Codex,
            Kind::UsageLimit,
            1_790_574_244,
            Some(1_791_050_823),
        );
        record(dir.path(), refused.clone()).unwrap();
        let now = 1_790_819_309;
        assert!(!Book::load(dir.path()).has_capacity(Provider::Codex, now));
        let reading = |at: u64, used: f64| usage::Book {
            schema: usage::SCHEMA.into(),
            entries: vec![Entry {
                provider: Provider::Codex,
                attempted_at: at,
                next_probe_at: at + 60,
                reading: Some(Reading {
                    provider: Provider::Codex,
                    observed_at: at,
                    windows: vec![Window {
                        window: WindowName::Primary,
                        used_fraction: used,
                        resets_at: Some(1_791_337_387),
                        length_seconds: Some(604_800),
                    }],
                    limit_reached: false,
                    plan: Some("pro".into()),
                    account: None,
                }),
                failure: None,
            }],
        };
        let write = |book: &usage::Book| {
            std::fs::write(
                dir.path().join(usage::FILE),
                serde_json::to_vec(book).unwrap(),
            )
            .unwrap();
        };
        write(&reading(now, 0.08));
        assert!(Book::load(dir.path()).has_capacity(Provider::Codex, now));
        write(&reading(now, 0.97));
        assert!(!Book::load(dir.path()).has_capacity(Provider::Codex, now));
        write(&reading(1_790_574_000, 0.08));
        assert!(!Book::load(dir.path()).has_capacity(Provider::Codex, now));
        let mut rate = Book::default();
        rate.refusals
            .push(Refusal::new(Provider::Codex, Kind::RateLimit, 1_000, None));
        let fresh = reading(2_000, 0.08);
        assert!(!rate.lifted_by(&fresh).has_capacity(Provider::Codex, 1_500));
        let mut usage_limit = Book::default();
        usage_limit.refusals.push(refused);
        assert!(
            usage_limit
                .lifted_by(&reading(now, 0.08))
                .refusals
                .is_empty()
        );
    }

    fn login_a(_: Provider) -> Option<String> {
        Some("acct-a".into())
    }
    fn login_b(_: Provider) -> Option<String> {
        Some("acct-b".into())
    }
    fn no_login(_: Provider) -> Option<String> {
        None
    }

    /// The owner on 2026-10-01 (#10105): Claude refused on an exhausted
    /// account, the owner signed in to another, and the hold kept passing
    /// Claude over. A hold keeps the fingerprint of the login it refused;
    /// once another login is signed in it no longer holds, and signing back
    /// in to the refused one brings it back while it lasts.
    #[test]
    fn a_hold_stops_holding_once_another_login_is_signed_in() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_790_900_000;
        let refusal = Refusal::new(Provider::Claude, Kind::UsageLimit, now, Some(now + 86_400));
        let book = record_with(dir.path(), refusal, login_a).unwrap();
        let kept = &book.refusals[0];
        let fingerprint = kept.account.clone().unwrap();
        assert_eq!(fingerprint.len(), 32);
        assert!(!fingerprint.contains("acct"));
        let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(!text.contains("acct-a"));
        assert!(!Book::load_with(dir.path(), login_a).has_capacity(Provider::Claude, now + 60));
        assert!(Book::load_with(dir.path(), login_b).has_capacity(Provider::Claude, now + 60));
        // Signed out, or a login with no identity: the hold keeps its
        // meaning and ends by time, as before fingerprints.
        assert!(!Book::load_with(dir.path(), no_login).has_capacity(Provider::Claude, now + 60));
        // A hold recorded before fingerprints holds for every login.
        let dir = tempfile::tempdir().unwrap();
        record_with(
            dir.path(),
            Refusal::new(Provider::Codex, Kind::UsageLimit, now, Some(now + 600)),
            no_login,
        )
        .unwrap();
        assert!(!Book::load_with(dir.path(), login_b).has_capacity(Provider::Codex, now + 60));
    }

    const BODY: &str = r#"{"error":{"type":"usage_limit_reached","message":"The usage limit has been reached","plan_type":"pro","resets_at":1791050823,"eligible_promo":null,"limit_window_minutes":10080,"resets_in_seconds":478613}}"#;

    #[test]
    fn a_codex_usage_limit_records_its_reset_and_blocks_until_then() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_790_572_210;
        let refusal = Refusal::codex(429, BODY, now).unwrap();
        assert_eq!(refusal.kind, Kind::UsageLimit);
        assert_eq!(refusal.resets_at, Some(1_791_050_823));
        assert_eq!(refusal.until, 1_791_050_823);
        assert_eq!(refusal.plan.as_deref(), Some("pro"));
        record(dir.path(), refusal).unwrap();
        let book = Book::load(dir.path());
        assert!(!book.has_capacity(Provider::Codex, now + 60));
        assert!(book.has_capacity(Provider::Claude, now + 60));
        assert!(book.has_capacity(Provider::Codex, 1_791_050_823));
        assert_eq!(
            book.earliest_reset(&[Provider::Codex], now),
            Some(1_791_050_823)
        );
        assert_eq!(
            book.earliest_reset(&[Provider::Codex, Provider::Claude], now),
            None
        );
        // Private, and it holds nothing from the body but typed fields.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(dir.path().join(FILE)).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }
        let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(!text.contains("message"));
    }

    #[test]
    fn an_opencode_rate_limit_holds_every_opencode_route_until_its_retry_after() {
        let dir = tempfile::tempdir().unwrap();
        let limited: Value =
            serde_json::from_str(include_str!("../fixtures/opencode/rate-limited.error.json"))
                .unwrap();
        let now = 1_790_631_000;
        let refusal = Refusal::opencode(&limited, now).unwrap();
        assert_eq!(refusal.provider, Provider::OpenCode);
        assert_eq!(refusal.kind, Kind::RateLimit);
        assert_eq!(refusal.resets_at, Some(now + 120));
        record(dir.path(), refusal).unwrap();
        let book = Book::load(dir.path());
        assert!(!book.has_capacity(Provider::OpenCode, now + 60));
        assert!(book.has_capacity(Provider::Codex, now + 60));
        assert!(book.has_capacity(Provider::OpenCode, now + 120));
        // A refusal that isn't a capacity limit, as recorded live.
        let disabled: Value = serde_json::from_str(include_str!(
            "../fixtures/opencode/model-access-disabled.error.json"
        ))
        .unwrap();
        assert_eq!(Refusal::opencode(&disabled, now), None);
        // No retry header: the unknown hold; milliseconds win when given.
        let mut bare = limited.clone();
        bare["data"]["responseHeaders"] = serde_json::json!({});
        assert_eq!(
            Refusal::opencode(&bare, now).unwrap().until,
            now + UNKNOWN_RESET_HOLD
        );
        bare["data"]["responseHeaders"] = serde_json::json!({"Retry-After-Ms": "1500"});
        assert_eq!(
            Refusal::opencode(&bare, now).unwrap().resets_at,
            Some(now + 2)
        );
        assert_eq!(
            Refusal::opencode(&serde_json::json!({"name": "ProviderAuthError"}), now),
            None
        );
        assert_eq!(Provider::from_config("opencode"), Some(Provider::OpenCode));
        assert_eq!(Provider::OpenCode.endpoint(), OPENCODE_ENDPOINT);
    }

    #[test]
    fn a_reset_in_seconds_or_none_still_bounds_the_hold() {
        let body = r#"{"error":{"type":"usage_limit_reached","resets_in_seconds":600}}"#;
        let refusal = Refusal::codex(429, body, 1_000).unwrap();
        assert_eq!(refusal.until, 1_600);
        let body = r#"{"error":{"type":"usage_limit_reached"}}"#;
        let refusal = Refusal::codex(429, body, 1_000).unwrap();
        assert_eq!(refusal.until, 1_000 + UNKNOWN_RESET_HOLD);
        let body = r#"{"error":{"type":"usage_limit_reached","resets_at":99999999999}}"#;
        let refusal = Refusal::codex(429, body, 1_000).unwrap();
        assert_eq!(refusal.until, 1_000 + MAX_HOLD);
        // An ordinary rate limit is not recorded.
        let body = r#"{"error":{"type":"rate_limit_exceeded"}}"#;
        assert_eq!(Refusal::codex(429, body, 1_000), None);
        // Claude: only an error result with API status 429 or a rejected
        // limit event; without a reported reset it holds 30 minutes.
        let claude = Refusal::claude(true, Some(429), None, 1_000).unwrap();
        assert_eq!(
            (claude.kind, claude.until),
            (Kind::RateLimit, 1_000 + UNKNOWN_RESET_HOLD)
        );
        assert_eq!(Refusal::claude(true, Some(529), None, 1_000), None);
        assert_eq!(Refusal::claude(false, Some(429), None, 1_000), None);
    }

    #[test]
    fn a_claude_refusal_records_the_reset_its_stream_reported() {
        let stdout = include_str!("../fixtures/claude/session-limit.stream.jsonl");
        let report = crate::claude::Report::parse(stdout).unwrap();
        let now = 1_790_162_000;
        let refusal = Refusal::claude(
            report.is_error,
            report.api_error_status,
            report.rate_limit.as_ref(),
            now,
        )
        .unwrap();
        assert_eq!(refusal.kind, Kind::UsageLimit);
        assert_eq!(refusal.resets_at, Some(1_790_164_200));
        assert_eq!(refusal.until, 1_790_164_200);
        assert_eq!(refusal.window_minutes, Some(300));
        // A probe reading doesn't override a reported reset.
        let probed = refusal.clone().with_probed_reset(&probe_book(now));
        assert_eq!(probed, refusal);
        // A rejected event alone is a refusal, even without status 429.
        let rejected = report.rate_limit.as_ref();
        assert!(Refusal::claude(true, None, rejected, now).is_some());
        assert!(Refusal::claude(false, None, rejected, now).is_none());
    }

    /// A usage book holding the recorded Claude probe, observed at `now`,
    /// with its five-hour window at its limit.
    fn probe_book(now: u64) -> crate::usage::Book {
        let body = include_str!("../fixtures/usage/claude-oauth-usage.json");
        let mut reading = crate::usage::parse_claude(body.as_bytes(), now).unwrap();
        reading.windows[0].used_fraction = 1.0;
        reading.windows[0].resets_at = Some(now + 3_600);
        crate::usage::Book {
            entries: vec![crate::usage::Entry {
                provider: Provider::Claude,
                attempted_at: now,
                next_probe_at: now + 60,
                reading: Some(reading),
                failure: None,
            }],
            ..crate::usage::Book::default()
        }
    }

    #[test]
    fn without_a_reported_reset_a_fresh_probe_reading_supplies_it() {
        let now = 1_790_572_210;
        let refusal = Refusal::claude(true, Some(429), None, now + 120).unwrap();
        let probed = refusal.clone().with_probed_reset(&probe_book(now));
        assert_eq!(probed.kind, Kind::UsageLimit);
        assert_eq!(probed.resets_at, Some(now + 3_600));
        assert_eq!(probed.until, now + 3_600);
        // A stale reading, or one with no window at its limit, leaves the
        // 30-minute hold.
        let stale = refusal
            .clone()
            .with_probed_reset(&probe_book(now - crate::usage::STALE_AFTER));
        assert_eq!(stale.until, now + 120 + UNKNOWN_RESET_HOLD);
        let mut calm = probe_book(now);
        calm.entries[0].reading.as_mut().unwrap().windows[0].used_fraction = 0.5;
        let unchanged = refusal.clone().with_probed_reset(&calm);
        assert_eq!(unchanged, refusal);
        assert_eq!(
            refusal
                .with_probed_reset(&crate::usage::Book::default())
                .until,
            now + 120 + UNKNOWN_RESET_HOLD
        );
    }

    #[test]
    fn a_vertex_quota_refusal_records_its_retry_delay() {
        let quota = include_str!("../fixtures/vertex/quota-exceeded.json");
        let refusal = Refusal::vertex(429, None, quota, 1_000).unwrap();
        assert_eq!(refusal.provider, Provider::Vertex);
        assert_eq!(refusal.kind, Kind::UsageLimit);
        assert_eq!(refusal.resets_at, Some(1_041));
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), refusal).unwrap();
        let book = Book::load(dir.path());
        assert!(!book.has_capacity(Provider::Vertex, 1_040));
        assert!(book.has_capacity(Provider::Vertex, 1_041));
        assert!(book.has_capacity(Provider::Codex, 1_040));
    }

    #[test]
    fn a_vertex_rate_limit_takes_retry_after_or_a_short_hold() {
        let limited = include_str!("../fixtures/vertex/resource-exhausted.openai.json");
        let refusal = Refusal::vertex(429, Some(90), limited, 1_000).unwrap();
        assert_eq!((refusal.kind, refusal.until), (Kind::RateLimit, 1_090));
        let refusal = Refusal::vertex(429, None, limited, 1_000).unwrap();
        assert_eq!(refusal.until, 1_000 + VERTEX_UNKNOWN_RESET_HOLD);
        // Other statuses and shapes are not refusals.
        assert_eq!(Refusal::vertex(500, None, limited, 1_000), None);
        let denied = r#"{"error":{"code":403,"status":"PERMISSION_DENIED"}}"#;
        assert_eq!(Refusal::vertex(429, None, denied, 1_000), None);
        assert_eq!(Refusal::vertex(429, None, "Too Many Requests", 1_000), None);
        assert_eq!(duration_seconds("0.5s"), Some(1));
        assert_eq!(duration_seconds("12s"), Some(12));
        assert_eq!(duration_seconds("12"), None);
        assert_eq!(duration_seconds("-1s"), None);
    }

    #[test]
    fn recording_replaces_a_providers_entry_and_drops_expired_ones() {
        let dir = tempfile::tempdir().unwrap();
        record(
            dir.path(),
            Refusal::new(Provider::Claude, Kind::RateLimit, 100, Some(200)),
        )
        .unwrap();
        record(
            dir.path(),
            Refusal::new(Provider::Codex, Kind::UsageLimit, 150, Some(900)),
        )
        .unwrap();
        record(
            dir.path(),
            Refusal::new(Provider::Codex, Kind::UsageLimit, 160, Some(1_000)),
        )
        .unwrap();
        let book = Book::load(dir.path());
        assert_eq!(book.refusals.len(), 2);
        assert_eq!(book.blocking(Provider::Codex, 170).unwrap().until, 1_000);
        // Claude's entry expired by the next write and is dropped.
        let book = record(
            dir.path(),
            Refusal::new(Provider::Codex, Kind::UsageLimit, 300, Some(1_000)),
        )
        .unwrap();
        assert_eq!(book.refusals.len(), 1);
        // A malformed book reads as empty and is replaced on the next record.
        std::fs::write(dir.path().join(FILE), b"{").unwrap();
        assert!(Book::load(dir.path()).refusals.is_empty());
        record(
            dir.path(),
            Refusal::new(Provider::Claude, Kind::RateLimit, 1, None),
        )
        .unwrap();
        assert_eq!(Book::load(dir.path()).refusals.len(), 1);
    }

    #[test]
    fn utc_text_names_the_date_and_minute() {
        assert_eq!(utc(1_791_050_823), "2026-10-03 18:07 UTC");
    }

    #[test]
    fn the_cloud_worker_codes_are_capacity_refusals_with_their_waits() {
        // A rate limit takes the worker's wait, rounded up to a second.
        let limited = Refusal::cloud("rate_limited", Some(1_500), 1_000).unwrap();
        assert_eq!(limited.provider, Provider::Vertex);
        assert_eq!(limited.kind, Kind::RateLimit);
        assert_eq!(limited.until, 1_002);
        // Without a wait it holds one minute, the worker's window.
        let busy = Refusal::cloud("busy", None, 1_000).unwrap();
        assert_eq!(busy.kind, Kind::RateLimit);
        assert_eq!(busy.until, 1_000 + CLOUD_UNKNOWN_RETRY);
        // A spent day is a usage limit until the worker says, else UTC
        // midnight, when its day resets.
        let spent = Refusal::cloud("quota_exhausted", Some(3_600_000), 90_000).unwrap();
        assert_eq!(spent.kind, Kind::UsageLimit);
        assert_eq!(spent.until, 93_600);
        let spent = Refusal::cloud("quota_exhausted", None, 90_000).unwrap();
        assert_eq!(spent.until, 172_800);
        // Codes that don't mean "not now" are not capacity refusals.
        for code in [
            "limit_exceeded",
            "not_admitted",
            "internal",
            "RATE_LIMITED",
            "",
        ] {
            assert_eq!(Refusal::cloud(code, Some(1_000), 1_000), None, "{code}");
        }
    }

    #[test]
    fn the_cloud_fallback_needs_nothing_on_the_host() {
        // The invariant: the cloud fallback is connected from the setting
        // alone, reading no token file and no Google credential; only
        // CODER_CLOUD=off turns it off.
        assert_eq!(cloud_connection(None), Connection::Connected);
        assert_eq!(cloud_connection(Some("on")), Connection::Connected);
        assert!(!cloud_connection(Some("off")).is_connected());
        assert!(!cloud_connection(Some(" off ")).is_connected());
        assert_eq!(Provider::Vertex.endpoint(), CLOUD_ENDPOINT);
        assert!(CLOUD_ENDPOINT.starts_with("wss://"));
        // The probe itself reads only the setting.
        if std::env::var_os(CLOUD_VAR).is_none() {
            assert_eq!(probe(Provider::Vertex), Connection::Connected);
        }
    }
}
