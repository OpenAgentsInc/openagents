//! Host-owned media, capture, and device adapters. Credentials stay inside
//! the broker; nothing here writes them to artifacts, logs, or results.
use crate::Result;
use serde_json::json;
use std::collections::BTreeMap;

/// How a revocation was observed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Revocation {
    Immediate,
    /// The broker cannot confirm; the token may still publish or record.
    Unknown,
}

/// A transport credential grant, bound to its exact scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    pub session: String,
    pub epoch: u64,
    pub participant: String,
    pub tracks: Vec<String>,
    pub rights: Vec<String>,
    pub expires_at: u64,
}

pub trait Broker {
    /// Mint a short-lived credential. The token is returned only to the
    /// caller for secure local delivery.
    fn mint(&mut self, grant: Grant) -> String;
    /// Check a presented token against the exact principal and use.
    fn redeem(
        &self,
        token: &str,
        participant: &str,
        track: &str,
        right: &str,
        now: u64,
    ) -> Result<Grant>;
    /// Revoke by the token's digest; the controller never stores tokens.
    fn revoke(&mut self, digest: &str) -> Revocation;
}

/// The digest under which a token is tracked.
pub fn token_digest(token: &str) -> String {
    nostr::contracts::digest_bytes(token.as_bytes())
}

/// In-process broker with explicit revocation behavior.
#[derive(Default)]
pub struct LocalBroker {
    tokens: BTreeMap<String, Grant>,
    /// Fault: revocations cannot be confirmed.
    pub revocation_unknown: bool,
}

impl Broker for LocalBroker {
    fn mint(&mut self, grant: Grant) -> String {
        let token: String = secp256k1::rand::random::<[u8; 32]>()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        self.tokens.insert(token_digest(&token), grant);
        token
    }
    fn redeem(
        &self,
        token: &str,
        participant: &str,
        track: &str,
        right: &str,
        now: u64,
    ) -> Result<Grant> {
        let grant = self
            .tokens
            .get(&token_digest(token))
            .ok_or("revoked: unknown or revoked credential")?;
        if grant.participant != participant {
            return Err("identity_mismatch: credential belongs to another principal".into());
        }
        if grant.expires_at <= now {
            return Err("stale: credential expired".into());
        }
        if !grant.tracks.iter().any(|t| t == track) || !grant.rights.iter().any(|r| r == right) {
            return Err("not_admitted: credential does not cover that use".into());
        }
        Ok(grant.clone())
    }
    fn revoke(&mut self, digest: &str) -> Revocation {
        if self.revocation_unknown {
            return Revocation::Unknown;
        }
        self.tokens.remove(digest);
        Revocation::Immediate
    }
}

/// One capture from a resolved source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Captured {
    pub bytes: Vec<u8>,
    /// Native anchor captured atomically with these bytes.
    pub anchor: Option<(u64, u64)>,
    pub native: serde_json::Value,
    /// `(width, height, coordinate space)` for images.
    pub geometry: Option<(u64, u64, String)>,
    /// `(clock, start_ms, end_ms)` for audio and video.
    pub interval: Option<(String, u64, u64)>,
}

pub struct CaptureContext<'a> {
    pub owner: &'a str,
    pub session: &'a str,
    pub plan: &'a serde_json::Value,
    pub generation: u64,
    pub focus: u64,
    pub max_bytes: usize,
    pub deadline: std::time::Instant,
}

pub trait Source {
    /// Capture from the host-resolved resource. Platform permission is the
    /// adapter's own check; a plan cannot supply it.
    fn capture(&mut self, scope: &str, id: &str, media: &str) -> Result<Captured>;
    /// Enforce caller, resource, bounds, and permission before capture. Legacy
    /// capture methods never receive an automatic fallback.
    fn capture_checked(
        &mut self,
        _scope: &str,
        _id: &str,
        _media: &str,
        _context: &CaptureContext<'_>,
    ) -> Result<Captured> {
        Err("unsupported_feature: checked capture is unavailable".into())
    }
}

/// Deterministic synthetic screen and microphone.
#[derive(Default)]
pub struct SyntheticSource {
    pub clock_ms: u64,
    /// Fault: the platform denied capture permission.
    pub denied: bool,
}

impl Source for SyntheticSource {
    fn capture_checked(
        &mut self,
        scope: &str,
        id: &str,
        media: &str,
        context: &CaptureContext<'_>,
    ) -> Result<Captured> {
        if std::time::Instant::now() >= context.deadline {
            return Err("stale: capture deadline".into());
        }
        let mut captured = self.capture(scope, id, media)?;
        if captured.bytes.len() > context.max_bytes {
            return Err("limit_exceeded: capture bytes".into());
        }
        captured.anchor = Some((context.generation, context.focus));
        Ok(captured)
    }
    fn capture(&mut self, scope: &str, id: &str, media: &str) -> Result<Captured> {
        if self.denied {
            return Err("not_admitted: platform capture permission denied".into());
        }
        let bytes =
            serde_json::to_vec(&json!({"scope":scope,"id":id,"media":media,"at":self.clock_ms}))
                .map_err(|e| e.to_string())?;
        Ok(match media {
            "image" | "video" => Captured {
                bytes,
                anchor: None,
                native: serde_json::Value::Null,
                geometry: Some((1280, 800, format!("{scope}:{id}"))),
                interval: (media == "video").then(|| {
                    (
                        "host-monotonic".to_string(),
                        self.clock_ms,
                        self.clock_ms + 1000,
                    )
                }),
            },
            _ => {
                let start = self.clock_ms;
                self.clock_ms += 2000;
                Captured {
                    bytes,
                    anchor: None,
                    native: serde_json::Value::Null,
                    geometry: None,
                    interval: Some(("host-monotonic".into(), start, start + 2000)),
                }
            }
        })
    }
}

/// What applying a device input observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Applied {
    Applied(Vec<u8>),
    /// The adapter proved that it sent no input.
    Refused(&'static str),
    /// The reply was lost after dispatch.
    Unknown,
}

/// Conditions checked by the native adapter immediately before sending input.
/// Reading `Device::state` earlier cannot replace this boundary check.
pub struct InputGuard {
    pub owner: String,
    pub session: String,
    pub plan: serde_json::Value,
    pub generation: u64,
    pub focus: u64,
    pub deadline: std::time::Instant,
    pub observation: serde_json::Value,
    pub binding: Vec<u8>,
    pub preconditions: Vec<Vec<u8>>,
}

/// The installed device implementation's exact input contract and binding.
pub struct InputOperation {
    pub implementation: Vec<u8>,
    pub binding: Vec<u8>,
    pub input_schema: serde_json::Value,
    pub observation_schema: serde_json::Value,
    pub preconditions_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub effects: serde_json::Value,
}

pub trait Device {
    /// Describe an installed operation without sending input or opening a device.
    fn input_operation(&self) -> Option<InputOperation> {
        None
    }
    /// Current device generation and focus/navigation version.
    fn state(&self) -> (u64, u64);
    fn apply(&mut self, operation: &str, input: &[u8]) -> Applied;
    /// Refuse unless this adapter can enforce every condition at the actual
    /// input boundary. The unchecked method is never a fallback.
    fn apply_checked(&mut self, _operation: &str, _input: &[u8], _guard: &InputGuard) -> Applied {
        Applied::Refused("unsupported_feature")
    }
}

/// Synthetic pointer/keyboard device. It counts applied inputs.
#[derive(Default)]
pub struct SyntheticDevice {
    pub generation: u64,
    pub focus: u64,
    pub applied: Vec<Vec<u8>>,
    /// Fault: lose the reply after applying.
    pub lose_reply: bool,
}

impl Device for SyntheticDevice {
    fn input_operation(&self) -> Option<InputOperation> {
        Some(InputOperation {
            implementation: b"synthetic-device guarded input v1".to_vec(),
            binding: b"pointer binding".to_vec(),
            input_schema: json!({"oneOf":[
                {"type":"object","additionalProperties":false,"required":["click"],
                    "properties":{"click":{"type":"array","minItems":2,"maxItems":2,
                        "prefixItems":[{"type":"integer","minimum":0,"maximum":1279},
                            {"type":"integer","minimum":0,"maximum":799}],"items":false}}},
                {"type":"object","additionalProperties":false,"required":["type"],
                    "properties":{"type":{"type":"string","maxLength":16384}}}]}),
            observation_schema: json!({"type":"object","required":["source","geometry"],"properties":{
                "source":{"const":{"scope":"display","id":"window-7"}},
                "geometry":{"type":"object","required":["width_px","height_px","coordinate_space"],
                    "properties":{"width_px":{"const":1280},"height_px":{"const":800},
                        "coordinate_space":{"const":"display:window-7"}}}}}),
            preconditions_schema: json!({"type":"array","maxItems":0}),
            output_schema: json!({"type":"object","additionalProperties":false,
                "required":["operation","applied"],"properties":{"operation":{"type":"string"},
                    "applied":{"type":"integer","minimum":1}}}),
            effects: json!({"reads":["artifact"],"writes":["synthetic-device"],"network":[],
                "process":false,"delegates":false,"spend":false}),
        })
    }
    fn state(&self) -> (u64, u64) {
        (self.generation, self.focus)
    }
    fn apply_checked(&mut self, operation: &str, input: &[u8], guard: &InputGuard) -> Applied {
        if std::time::Instant::now() >= guard.deadline
            || self.state() != (guard.generation, guard.focus)
        {
            return Applied::Refused("stale");
        }
        if !guard.preconditions.is_empty() {
            return Applied::Refused("unsupported_feature");
        }
        self.apply(operation, input)
    }
    fn apply(&mut self, operation: &str, input: &[u8]) -> Applied {
        self.applied.push(input.to_vec());
        if self.lose_reply {
            return Applied::Unknown;
        }
        Applied::Applied(
            serde_json::to_vec(&json!({"operation":operation,"applied":self.applied.len()}))
                .unwrap_or_default(),
        )
    }
}
