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
    /// `(width, height, coordinate space)` for images.
    pub geometry: Option<(u64, u64, String)>,
    /// `(clock, start_ms, end_ms)` for audio and video.
    pub interval: Option<(String, u64, u64)>,
}

pub trait Source {
    /// Capture from the host-resolved resource. Platform permission is the
    /// adapter's own check; a plan cannot supply it.
    fn capture(&mut self, scope: &str, id: &str, media: &str) -> Result<Captured>;
}

/// Deterministic synthetic screen and microphone.
#[derive(Default)]
pub struct SyntheticSource {
    pub clock_ms: u64,
    /// Fault: the platform denied capture permission.
    pub denied: bool,
}

impl Source for SyntheticSource {
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
    /// The reply was lost after dispatch.
    Unknown,
}

pub trait Device {
    /// Current device generation and focus/navigation version.
    fn state(&self) -> (u64, u64);
    fn apply(&mut self, operation: &str, input: &[u8]) -> Applied;
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
    fn state(&self) -> (u64, u64) {
        (self.generation, self.focus)
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
