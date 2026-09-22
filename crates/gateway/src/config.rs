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
