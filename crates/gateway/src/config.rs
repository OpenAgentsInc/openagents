//! The gateway's deployment document: where to listen, where the
//! registry lives, and which endpoint stands behind each door.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::classify::BackendLimits;

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

fn default_in_flight() -> usize {
    64
}

fn default_questions() -> u64 {
    256
}

fn default_options() -> u64 {
    4096
}

impl Config {
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
        if self.reservation_ttl_secs * 1000 < self.forward_timeout_ms {
            return Err(format!(
                "{}: reservation_ttl_secs ({}s) is shorter than forward_timeout_ms ({}ms) — \
                 a reservation would expire while its forward still ran",
                name.display(),
                self.reservation_ttl_secs,
                self.forward_timeout_ms
            ));
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
    }

    #[test]
    fn a_reservation_deadline_shorter_than_the_forward_is_refused() {
        let mut broken = config();
        broken.forward_timeout_ms = 600_000;
        broken.reservation_ttl_secs = 60;
        assert!(broken.check(Path::new("gateway.json")).is_err());
    }
}
