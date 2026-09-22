//! The gateway's deployment document: where to listen, where the
//! registry lives, and which endpoint stands behind each door.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::classify::BackendLimits;
use crate::money::Money;

/// The schema tag a gateway config carries.
pub const SCHEMA: &str = "openagents.gateway.v1";

/// One door's backend: where the request is forwarded once the binding
/// admits it.
///
/// The endpoint is deployment configuration, deliberately not part of
/// the registry manifest — which host serves a door is the operator's
/// business; which identity that host must publish is the binding's.
#[derive(Clone, Debug, Deserialize)]
pub struct Door {
    /// The backend's base URL, such as `http://127.0.0.1:9080`. The
    /// gateway calls `{endpoint}/v1/models` and `{endpoint}/v1/systemone`.
    /// Plain HTTP is expected — the backend binds a private interface
    /// and trusts only forwarded calls.
    pub endpoint: String,
    /// The bounds this backend declares for `POST /v1/classify`, when
    /// the operator has measured and declared them. Absent means the
    /// door serves `systemone` only — the facade never infers support
    /// from a missing declaration.
    #[serde(default)]
    pub classify: Option<BackendLimits>,
    /// The most per-input forwards one `POST /v1/classify` call may hold
    /// in flight against this backend at once. Default 1 — the call
    /// runs serially, as it always has. A value above one is the
    /// operator's explicit declaration that the backend takes that
    /// much item concurrency; the binding's declared
    /// `capacity.concurrency`, when it names one, still bounds every
    /// forward underneath it, and the process's `max_in_flight` bounds
    /// the whole. A bound that can never be reached is refused rather
    /// than silently capped.
    #[serde(default = "default_item_concurrency")]
    pub classify_item_concurrency: u64,
    /// How the backend serves batch work, when the operator has
    /// declared it: `native` packs items into one execution and names
    /// its bound, `caller-loop` is bounded independent calls with no
    /// batch bound. Absent means unknown — the facade still loops
    /// calls, but discovery reports no adapter capability rather than
    /// guessing one. A declared value is checked against the card the
    /// backend publishes at request time; a disagreement is an
    /// `identity_mismatch`, never a silent substitution.
    #[serde(default)]
    pub batching: Option<tenancy::backend::Batching>,
}

/// The parsed `gateway.json`.
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    /// The schema tag.
    pub v: String,
    /// The address the public listener binds, such as `0.0.0.0:443`.
    pub listen: String,
    /// The registry directory — `registry.json`, `keys.json`,
    /// `quota-ledger.jsonl`, and `receipts.jsonl` all live there.
    pub registry: PathBuf,
    /// Require an authenticated workspace membership on every public decision
    /// and discovery request. Legacy tenant-key admission is the default.
    /// Monetary admission requires it: a charge binds a workspace, never an
    /// anonymous or bearer-only call.
    #[serde(default)]
    pub require_workspace_membership: bool,
    /// Monetary admission — the explicit opt-in to charging workspaces.
    /// Absent means no ledger opens, no workspace is charged, and no
    /// balance route exists: the gateway behaves exactly as it did
    /// without the field.
    #[serde(default)]
    pub money: Option<Money>,
    /// The largest request body admitted, in bytes. Default 1 MiB —
    /// a decision request is state plus questions, never a bulk upload.
    #[serde(default = "default_body_max")]
    pub max_body_bytes: usize,
    /// The largest response body accepted from a backend, in bytes.
    /// Default 4 MiB.
    #[serde(default = "default_response_max")]
    pub max_response_bytes: usize,
    /// How long a forwarded call may run before the gateway declares it
    /// unavailable. Default two minutes.
    #[serde(default = "default_forward_timeout_ms")]
    pub forward_timeout_ms: u64,
    /// How long a `POST /v1/classify` call may run end to end — queue
    /// waits and forwards share the one deadline. Absent means the
    /// forward timeout governs. A value longer than `forward_timeout_ms`
    /// is refused: a single forward cannot outlive the client's own
    /// timeout, so the call deadline would promise more than a forward
    /// can deliver.
    #[serde(default)]
    pub classify_timeout_ms: Option<u64>,
    /// The most classify forwards one tenant may hold in flight at
    /// once, summed over every open `POST /v1/classify` call. Absent
    /// means a tenant is bounded only by the call's own fan-out, the
    /// door's slots, and the process's `max_in_flight`. A value is the
    /// fairness knob for mixed workloads: one tenant's thousand-input
    /// call cannot hold every door slot while another tenant's call
    /// waits. Items that cannot take a tenant slot inside the call's
    /// deadline report `unattempted`.
    #[serde(default)]
    pub max_tenant_classify_in_flight: Option<u32>,
    /// How long a reservation may stand unsettled before recovery
    /// orphans it. Default five minutes — comfortably longer than
    /// `forward_timeout_ms`, so a slow door is not mistaken for a dead
    /// one.
    #[serde(default = "default_ttl_secs")]
    pub reservation_ttl_secs: u64,
    /// How many forwards the gateway holds in flight at once, across
    /// every door. Default 64 — the bound that keeps one burst from
    /// starving the process.
    #[serde(default = "default_in_flight")]
    pub max_in_flight: usize,
    /// Admitted classification inputs, including waiting and running items.
    #[serde(default = "default_classify_inputs")]
    pub max_classify_inputs: u32,
    /// Per-tenant share of admitted classification inputs. Anonymous calls
    /// share one separate allowance; issuing another key creates no new share.
    #[serde(default = "default_classify_inputs")]
    pub max_classify_inputs_per_tenant: u32,
    /// The most questions one request may carry. Default 256 — the
    /// backend's own bounds are tighter still, but a request this shape
    /// is refused before it is authorized or reserved.
    #[serde(default = "default_questions")]
    pub max_questions: u64,
    /// Origins a browser caller may use with a credential, answered as
    /// `Access-Control-Allow-Origin` on credentialed routes and their
    /// preflights. Default empty — no credentialed cross-origin use.
    /// Public GET routes always answer `Access-Control-Allow-Origin: *`
    /// regardless of this list. Wildcards are not honored: an origin is
    /// admitted only by exact match.
    #[serde(default)]
    pub cors_origins: Vec<String>,
    /// The most options one request may total across its `choice` and
    /// `score` questions. Default 4096.
    #[serde(default = "default_options")]
    pub max_options: u64,
    /// Door name to its backend. A bound door missing here is a
    /// misconfiguration the gateway reports as `door_unavailable`
    /// rather than guessing an address.
    #[serde(default)]
    pub doors: BTreeMap<String, Door>,
    /// How long a terminal job's manifest, status, results, and delivery
    /// records stay before the retention sweep removes them. Default
    /// seven days — a durable job's results are a short-lived record,
    /// not an archive.
    #[serde(default = "default_job_retention_ms")]
    pub job_retention_ms: u64,
    /// How long a results-export cursor stays valid. Default one hour —
    /// a paginated read is a short-lived operation, not a bookmark.
    /// An expired cursor answers `cursor_expired`, never a quiet
    /// restart at the wrong offset.
    #[serde(default = "default_job_cursor_ttl_ms")]
    pub job_cursor_ttl_ms: u64,
    /// The absolute origin the public discovery documents fold into
    /// canonical links, sitemap entries, and card URLs, such as
    /// `https://api.example.com`. Absent means each request's own
    /// `Host` over plain HTTP — correct for direct local serving; set
    /// it when the deployment answers behind TLS or a reverse proxy
    /// under a public name.
    #[serde(default)]
    pub public_origin: Option<String>,
    /// The account, session, and key-management surface. Absent means
    /// the gateway mounts no account routes: callers are exactly the
    /// tenants the operator provisioned out of band, and a `sess_`
    /// token is not a credential the service knows.
    #[serde(default)]
    pub accounts: Option<Accounts>,
    /// Plans, checkout, subscriptions, and provider events — the
    /// billing surface. Absent means the gateway mounts no billing
    /// routes and no plan gates a door: workspaces call exactly as
    /// `money` alone admits. Present requires `accounts` and `money` —
    /// a subscription binds a workspace, and a grant needs the ledger
    /// behind it.
    #[serde(default)]
    pub billing: Option<Billing>,
    /// The versioned skill directory — bounded `SKILL.md` submissions,
    /// staged review, and a public catalog. Absent mounts no skill
    /// routes. Present requires `accounts`: a submission binds the
    /// account that sent it.
    #[serde(default)]
    pub skills: Option<Skills>,
}

/// The skill directory's deployment options: the submission bounds the
/// book enforces and the backend the decision-review stage calls.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Skills {
    /// The largest Markdown body a submission may carry, in bytes.
    /// Default 64 KiB — a skill document is instructions, not a corpus.
    #[serde(default = "default_skill_body")]
    pub max_body_bytes: usize,
    /// The submissions one author may open per day. Default 20.
    #[serde(default = "default_skill_daily")]
    pub submissions_per_day: u32,
    /// The under-review submissions one author may hold at once.
    /// Default 10 — the review queue stays shallow.
    #[serde(default = "default_skill_pending")]
    pub pending_per_author: usize,
    /// The quality score a decision review must reach to admit a
    /// version. Default 0.6 — recorded with the stage so a reopened
    /// store admits under the same declared bound.
    #[serde(default = "default_admit_score")]
    pub admit_score: f64,
    /// The backend the decision-review stage calls — a TypeSafe-shaped
    /// endpoint answering `POST {endpoint}/v1/systemone`.
    pub review: Review,
}

/// The decision-review backend: which endpoint answers the review's
/// pinned questions and which model it names.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    /// The endpoint root — the stage posts to `{endpoint}/v1/systemone`.
    pub endpoint: String,
    /// The model the review names in its request's `model` field.
    pub model: String,
    /// The review call's deadline in milliseconds. Default 30 seconds —
    /// a review that cannot answer in its window records an error and
    /// the submission stays under review for retry.
    #[serde(default = "default_review_timeout")]
    pub timeout_ms: u64,
}

fn default_skill_body() -> usize {
    65_536
}

fn default_skill_daily() -> u32 {
    20
}

fn default_skill_pending() -> usize {
    10
}

fn default_admit_score() -> f64 {
    0.6
}

fn default_review_timeout() -> u64 {
    30_000
}

/// The billing surface's deployment options: the published plan
/// catalog, the provider that attests payment, and the webhook
/// envelope that carries its events.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Billing {
    /// The versioned plan catalog — the price configuration checkout
    /// sells and `GET /v1/plans` publishes.
    pub plans: Vec<tenancy::billing::Plan>,
    /// The provider id whose events the webhook accepts. `sandbox`
    /// is the built-in provider — a journal of operator-emitted events
    /// beside the registry, for integration and staging. Any other
    /// value names a provider this build does not know and is refused.
    #[serde(default = "default_provider")]
    pub provider: String,
    /// The environment variable holding the webhook HMAC secret — a
    /// name, never the secret itself. Default `OPENAGENTS_BILLING_SECRET`.
    #[serde(default = "default_webhook_secret_env")]
    pub webhook_secret_env: String,
    /// How long a pending checkout stands before it expires, in
    /// seconds. Default one day — a payment link is not durable.
    #[serde(default = "default_checkout_ttl")]
    pub checkout_ttl_secs: u64,
    /// The accepted clock skew on a webhook signature timestamp, in
    /// seconds. Default five minutes — a signed event is fresh or it
    /// is refused.
    #[serde(default = "default_webhook_skew")]
    pub webhook_skew_secs: u64,
}

fn default_provider() -> String {
    "sandbox".to_string()
}

fn default_webhook_secret_env() -> String {
    "OPENAGENTS_BILLING_SECRET".to_string()
}

fn default_checkout_ttl() -> u64 {
    86_400
}

fn default_webhook_skew() -> u64 {
    300
}

/// The account surface's deployment options: sessions, self-serve
/// sign-up, and the funded anonymous lane.
///
/// Present-but-empty mounts the management routes over
/// `accounts.json` — members sign in with an `oak_` key and manage
/// their workspaces and keys — with sign-up and the anonymous lane
/// staying off.
#[derive(Clone, Debug, Deserialize)]
pub struct Accounts {
    /// The tenant self-serve sign-up provisions accounts onto: a
    /// `POST /v1/accounts` call creates the account, its personal
    /// workspace bound to this tenant, and its first `oak_` key.
    /// Absent disables sign-up; the rest of the surface still serves
    /// operator-provisioned accounts.
    #[serde(default)]
    pub signup_tenant: Option<String>,
    /// How long a session stands from issue, in seconds. Default eight
    /// hours — a workday, not a standing credential.
    #[serde(default = "default_session_ttl")]
    pub session_ttl_secs: u64,
    /// How long a recovery token stands from issue, in seconds.
    /// Default one hour — a delivered token is used or dead, never
    /// durable.
    #[serde(default = "default_recovery_ttl")]
    pub recovery_ttl_secs: u64,
    /// The operator-funded anonymous budget — the bounded public lane.
    /// Absent means `POST /v1/sessions` mints no anonymous sessions and
    /// the `shared` bindings' anonymous access stays exactly what it
    /// was.
    #[serde(default)]
    pub anonymous: Option<Anonymous>,
}

/// The operator-funded anonymous budget: a stated bound the public
/// lane draws down, plus the per-session cap that keeps one session
/// from draining it.
#[derive(Clone, Debug, Deserialize)]
pub struct Anonymous {
    /// The workspace reference the funding records against — a label
    /// for attribution, never a credential.
    #[serde(default = "default_anonymous_workspace")]
    pub workspace: String,
    /// How many requests the funding answers in total.
    pub bound: u64,
    /// The most one session may draw.
    #[serde(default = "default_anonymous_cap")]
    pub session_cap: u64,
    /// How long the funding stands, in seconds. Default one day —
    /// funding is a decision an operator renews, not a standing offer.
    #[serde(default = "default_anonymous_ttl")]
    pub ttl_secs: u64,
}

fn default_body_max() -> usize {
    1_048_576
}

fn default_response_max() -> usize {
    4_194_304
}

fn default_forward_timeout_ms() -> u64 {
    120_000
}

fn default_ttl_secs() -> u64 {
    300
}

fn default_classify_inputs() -> u32 {
    1024
}

fn default_in_flight() -> usize {
    64
}

fn default_item_concurrency() -> u64 {
    1
}

fn default_questions() -> u64 {
    256
}

fn default_options() -> u64 {
    4096
}

fn default_job_retention_ms() -> u64 {
    604_800_000
}

fn default_job_cursor_ttl_ms() -> u64 {
    3_600_000
}

fn default_session_ttl() -> u64 {
    28_800
}

fn default_recovery_ttl() -> u64 {
    3_600
}

fn default_anonymous_workspace() -> String {
    "public".to_string()
}

fn default_anonymous_cap() -> u64 {
    50
}

fn default_anonymous_ttl() -> u64 {
    86_400
}

impl Config {
    /// The deadline a classification call runs under: its own bound
    /// when the operator declared one, the forward timeout otherwise.
    #[must_use]
    pub fn classify_deadline_ms(&self) -> u64 {
        self.classify_timeout_ms.unwrap_or(self.forward_timeout_ms)
    }

    /// Read and check a config file.
    ///
    /// Refused: a schema tag this build does not know, a door named
    /// twice, an endpoint that is not an HTTP URL, or a reservation
    /// deadline shorter than the forward timeout — a reservation that
    /// expires before its forward can finish orphans live work.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let config: Self =
            serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        config.check(path)?;
        Ok(config)
    }

    /// The checks [`Config::load`] runs.
    pub fn check(&self, name: &Path) -> Result<(), String> {
        if self.v != SCHEMA {
            return Err(format!(
                "{}: schema `{}` is not `{SCHEMA}`",
                name.display(),
                self.v
            ));
        }
        if self.max_classify_inputs == 0
            || self.max_classify_inputs > 1_000_000
            || self.max_classify_inputs_per_tenant == 0
            || self.max_classify_inputs_per_tenant > self.max_classify_inputs
        {
            return Err("classification input limits must be positive, at most 1,000,000 globally, and per-tenant no larger than global".into());
        }
        if let Some(deadline) = self.classify_timeout_ms
            && deadline > self.forward_timeout_ms
        {
            return Err(format!(
                "{}: classify_timeout_ms ({deadline}ms) exceeds forward_timeout_ms ({}ms) — \
                 a call deadline cannot promise more than a forward can deliver",
                name.display(),
                self.forward_timeout_ms
            ));
        }
        if let Some(bound) = self.max_tenant_classify_in_flight
            && (bound == 0 || bound as usize > self.max_in_flight)
        {
            return Err(format!(
                "{}: max_tenant_classify_in_flight must be positive and no larger than \
                 the process's `max_in_flight` of {} — a bound that can never be reached \
                 is not a bound",
                name.display(),
                self.max_in_flight
            ));
        }
        if self.reservation_ttl_secs * 1000 < self.classify_deadline_ms() {
            return Err(format!(
                "{}: reservation_ttl_secs ({}s) is shorter than the longest call deadline \
                 ({}ms) — a reservation would expire while its work still ran",
                name.display(),
                self.reservation_ttl_secs,
                self.classify_deadline_ms()
            ));
        }
        if let Some(money) = &self.money {
            if !self.require_workspace_membership {
                return Err(format!(
                    "{}: `money` requires `require_workspace_membership` — a charge \
                     binds an authenticated workspace, never an anonymous or \
                     bearer-only call",
                    name.display()
                ));
            }
            for (door, priced) in &money.doors {
                if !self.doors.contains_key(door) {
                    return Err(format!(
                        "{}: money prices door `{door}`, which has no configured \
                         backend — the price names nothing the gateway can serve",
                        name.display()
                    ));
                }
                if priced.price.policy != crate::money::POLICY {
                    return Err(format!(
                        "{}: door `{door}`'s price names policy `{}`, which this \
                         build does not implement (`{}`)",
                        name.display(),
                        priced.price.policy,
                        crate::money::POLICY
                    ));
                }
                priced.price.quote(&priced.maximum_usage).map_err(|error| {
                    format!(
                        "{}: door `{door}`'s price cannot quote its declared \
                             maximum usage: {error}",
                        name.display()
                    )
                })?;
            }
        }
        if let Some(origin) = &self.public_origin {
            let scheme = origin
                .strip_prefix("https://")
                .or_else(|| origin.strip_prefix("http://"));
            let valid = scheme.is_some_and(|rest| {
                !rest.is_empty() && !rest.contains('/') && !rest.contains(['?', '#', ' '])
            });
            if !valid {
                return Err(format!(
                    "{}: public_origin `{origin}` must be an `http://` or `https://` \
                     origin with no path, query, or fragment — it names where the \
                     deployment answers, not a route",
                    name.display()
                ));
            }
        }
        if let Some(accounts) = &self.accounts {
            if accounts.session_ttl_secs == 0 || accounts.recovery_ttl_secs == 0 {
                return Err(format!(
                    "{}: session and recovery lifetimes must be positive — a \
                     credential that never stands or never ends is not a bound",
                    name.display()
                ));
            }
            if let Some(anonymous) = &accounts.anonymous
                && (anonymous.bound == 0
                    || anonymous.session_cap == 0
                    || anonymous.ttl_secs == 0
                    || anonymous.session_cap > anonymous.bound)
            {
                return Err(format!(
                    "{}: the anonymous budget must be positive, live, and capped \
                     no looser per session than in total — a bound that cannot \
                     be spent or can never run out is not a bound",
                    name.display()
                ));
            }
        }
        if let Some(billing) = &self.billing {
            if self.accounts.is_none() || self.money.is_none() {
                return Err(format!(
                    "{}: `billing` requires `accounts` and `money` — a subscription \
                     binds a workspace and its grants need the ledger behind them",
                    name.display()
                ));
            }
            if billing.provider != "sandbox" {
                return Err(format!(
                    "{}: billing provider `{}` is not one this build knows (`sandbox`)",
                    name.display(),
                    billing.provider
                ));
            }
            if billing.plans.is_empty() {
                return Err(format!(
                    "{}: `billing` needs at least one plan — checkout cannot sell a \
                     catalog it does not have",
                    name.display()
                ));
            }
            if billing.checkout_ttl_secs == 0 || billing.webhook_skew_secs == 0 {
                return Err(format!(
                    "{}: checkout_ttl_secs and webhook_skew_secs must be positive — \
                     a bound of zero is not a bound",
                    name.display()
                ));
            }
            let mut ids = std::collections::BTreeSet::new();
            for plan in &billing.plans {
                if !ids.insert(plan.id.clone()) {
                    return Err(format!(
                        "{}: two plans share id `{}` — a subscription cannot tell \
                         which offer it took",
                        name.display(),
                        plan.id
                    ));
                }
                if plan.id.is_empty() || plan.version.is_empty() {
                    return Err(format!(
                        "{}: a plan needs an id and a version — the version is what \
                         a subscription pins its terms to",
                        name.display()
                    ));
                }
                if plan.price.period_secs == 0 {
                    return Err(format!(
                        "{}: plan `{}` has a zero-second period — a period must be \
                         a length of time, not an instant",
                        name.display(),
                        plan.id
                    ));
                }
                if plan.price.currency.len() != 3
                    || !plan
                        .price
                        .currency
                        .bytes()
                        .all(|byte| byte.is_ascii_uppercase())
                {
                    return Err(format!(
                        "{}: plan `{}` currency `{}` must be a three-letter \
                         uppercase code, such as `USD` — the ledger's own \
                         currency contract",
                        name.display(),
                        plan.id,
                        plan.price.currency
                    ));
                }
                if let tenancy::billing::ModelAccess::Listed(doors) = &plan.models {
                    for door in doors {
                        if !self.doors.contains_key(door) {
                            return Err(format!(
                                "{}: plan `{}` names door `{door}`, which has no \
                                 configured backend — the plan's model list names \
                                 nothing the gateway can serve",
                                name.display(),
                                plan.id
                            ));
                        }
                    }
                }
            }
        }
        if let Some(skills) = &self.skills {
            if self.accounts.is_none() {
                return Err(format!(
                    "{}: `skills` requires `accounts` — a submission binds the \
                     account that sent it",
                    name.display()
                ));
            }
            if skills.max_body_bytes == 0 || skills.max_body_bytes > 1_048_576 {
                return Err(format!(
                    "{}: skills.max_body_bytes must be positive and at most 1 MiB",
                    name.display()
                ));
            }
            if skills.submissions_per_day == 0 || skills.pending_per_author == 0 {
                return Err(format!(
                    "{}: skills submission bounds must be positive — a bound of \
                     zero admits nothing",
                    name.display()
                ));
            }
            if !(0.0..=1.0).contains(&skills.admit_score) {
                return Err(format!(
                    "{}: skills.admit_score must sit in 0–1",
                    name.display()
                ));
            }
            if !(skills.review.endpoint.starts_with("http://")
                || skills.review.endpoint.starts_with("https://"))
            {
                return Err(format!(
                    "{}: skills.review.endpoint `{}` is not an HTTP URL — the \
                     review stage posts to it, so it must name one",
                    name.display(),
                    skills.review.endpoint
                ));
            }
            if skills.review.model.is_empty() {
                return Err(format!(
                    "{}: skills.review.model is empty — the review names a model \
                     it cannot omit",
                    name.display()
                ));
            }
            if skills.review.timeout_ms == 0 {
                return Err(format!(
                    "{}: skills.review.timeout_ms must be positive",
                    name.display()
                ));
            }
        }
        for (door, backend) in &self.doors {
            if !(backend.endpoint.starts_with("http://")
                || backend.endpoint.starts_with("https://"))
            {
                return Err(format!(
                    "{}: door `{door}` names endpoint `{}`, which is not an HTTP URL",
                    name.display(),
                    backend.endpoint
                ));
            }
            if backend.classify_item_concurrency == 0 {
                return Err(format!(
                    "{}: door `{door}` declares a classify item concurrency of zero — \
                     it admits no forwards at all",
                    name.display()
                ));
            }
            if backend.classify.is_none() && backend.classify_item_concurrency > 1 {
                return Err(format!(
                    "{}: door `{door}` declares a classify item concurrency but no classify \
                     bounds — the facade does not infer support it was not told about",
                    name.display()
                ));
            }
            if backend.classify_item_concurrency > self.max_in_flight as u64 {
                return Err(format!(
                    "{}: door `{door}` declares a classify item concurrency of {} above the \
                     process's `max_in_flight` of {} — a bound that can never be reached",
                    name.display(),
                    backend.classify_item_concurrency,
                    self.max_in_flight
                ));
            }
            if let Some(batching) = &backend.batching {
                match (batching.kind, batching.max_items) {
                    (tenancy::backend::BatchKind::Native, Some(items)) if items >= 2 => {}
                    (tenancy::backend::BatchKind::Native, _) => {
                        return Err(format!(
                            "{}: door `{door}` declares native batching without a `max_items` \
                             of two or more — one item is a call, not a batch",
                            name.display()
                        ));
                    }
                    (tenancy::backend::BatchKind::CallerLoop, Some(_)) => {
                        return Err(format!(
                            "{}: door `{door}` declares caller-loop batching with a `max_items` \
                             — there is no batch to bound",
                            name.display()
                        ));
                    }
                    (tenancy::backend::BatchKind::CallerLoop, None) => {}
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        serde_json::from_value(serde_json::json!({
            "v": SCHEMA,
            "listen": "127.0.0.1:8080",
            "registry": "/tmp/registry",
            "doors": {"kev-0.6b": {"endpoint": "http://127.0.0.1:9080"}},
        }))
        .unwrap()
    }

    #[test]
    fn a_valid_config_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gateway.json");
        std::fs::write(
            &path,
            format!(
                r#"{{"v": "{SCHEMA}", "listen": "127.0.0.1:8080",
                    "registry": "/tmp/registry",
                    "doors": {{"kev-0.6b": {{"endpoint": "http://127.0.0.1:9080"}}}}}}"#
            ),
        )
        .unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.max_body_bytes, 1_048_576);
        assert_eq!(loaded.doors["kev-0.6b"].endpoint, "http://127.0.0.1:9080");
        // An undeclared item concurrency is one — serial, as before.
        assert_eq!(loaded.doors["kev-0.6b"].classify_item_concurrency, 1);
    }

    #[test]
    fn an_unreachable_or_unbacked_item_concurrency_is_refused() {
        // Item concurrency without classify bounds infers support the
        // door never declared.
        let mut unbacked = config();
        unbacked
            .doors
            .get_mut("kev-0.6b")
            .unwrap()
            .classify_item_concurrency = 4;
        assert!(unbacked.check(Path::new("gateway.json")).is_err());

        // A bound above the process's own forward bound can never be
        // reached — the misconfiguration is refused, not capped.
        let mut unreachable = config();
        unreachable.max_in_flight = 8;
        let door = unreachable.doors.get_mut("kev-0.6b").unwrap();
        door.classify = Some(crate::classify::BackendLimits::product());
        door.classify_item_concurrency = 128;
        assert!(unreachable.check(Path::new("gateway.json")).is_err());

        // Zero admits no forwards at all.
        let mut zero = config();
        let door = zero.doors.get_mut("kev-0.6b").unwrap();
        door.classify = Some(crate::classify::BackendLimits::product());
        door.classify_item_concurrency = 0;
        assert!(zero.check(Path::new("gateway.json")).is_err());
    }

    #[test]
    fn a_reservation_deadline_shorter_than_the_forward_is_refused() {
        let mut broken = config();
        broken.forward_timeout_ms = 600_000;
        broken.reservation_ttl_secs = 60;
        assert!(broken.check(Path::new("gateway.json")).is_err());
    }
}
