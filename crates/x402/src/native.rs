//! The native Nostr binding `nostr:openagents:1`: one purchase of one
//! capability operation, carried as private kind-3188 records between one
//! buyer and one provider. This module holds the record contract, the status
//! chain, the purchase ledger, and the provider and buyer steps; sealing,
//! relay transport, and wallets stay with the caller.
//!
//! Records are immutable. The provider's ledger keeps every record it issued
//! or admitted under `(buyer, purchase)` and is written before any record is
//! published and before any execution is dispatched, so a restart recovers
//! from the ledger rather than from the relay.

use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use nostr::contracts::{
    ContractError, digest_bytes, jcs, parse_artifact, parse_definition, parse_strict,
};
use nostr::x402::{PaymentRequirements, SupportedProfiles, binding_hash, native_binding};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::facilitator::Facilitator;
use crate::replay::ReplayStore;
use crate::server::Receiver;
use crate::wire::{PaymentPayload, PaymentRequired, ResourceInfo, SettlementResponse};

pub const PROFILE: &str = "nostr:openagents:1";
pub const RECORD_SCHEMA: &str = "openagents.x402-record.v1";
/// Inline artifact that carries opaque bytes: `{"v": ..., "bytes": base64}`.
pub const BYTES_SCHEMA: &str = "openagents.x402-bytes.v1";
pub const RECOVERY: &str = "native-record-v1";

pub const NATIVE_ONLY: SupportedProfiles = SupportedProfiles {
    http: false,
    mcp: false,
    native: true,
};

const RECORD_KEYS: [&str; 10] = [
    "v",
    "requires",
    "type",
    "purchase",
    "buyer",
    "provider",
    "issuer",
    "issued_at",
    "body",
    "meta",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordType {
    Request,
    Challenge,
    Claim,
    ClaimRejected,
    StatusQuery,
    Status,
}

impl RecordType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Challenge => "challenge",
            Self::Claim => "claim",
            Self::ClaimRejected => "claim_rejected",
            Self::StatusQuery => "status_query",
            Self::Status => "status",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "request" => Self::Request,
            "challenge" => Self::Challenge,
            "claim" => Self::Claim,
            "claim_rejected" => Self::ClaimRejected,
            "status_query" => Self::StatusQuery,
            "status" => Self::Status,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Offered,
    ClaimPending,
    Admitted,
    Running,
    Completed,
    Failed,
    Refused,
    Unknown,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Self::Offered => "offered",
            Self::ClaimPending => "claim_pending",
            Self::Admitted => "admitted",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Refused => "refused",
            Self::Unknown => "unknown",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "offered" => Self::Offered,
            "claim_pending" => Self::ClaimPending,
            "admitted" => Self::Admitted,
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "refused" => Self::Refused,
            "unknown" => Self::Unknown,
            _ => return None,
        })
    }

    pub fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Refused)
    }

    /// The transitions the status table admits.
    fn admits(self, next: Phase) -> bool {
        use Phase::{
            Admitted, ClaimPending, Completed, Failed, Offered, Refused, Running, Unknown,
        };
        match self {
            Offered => matches!(next, ClaimPending | Refused),
            ClaimPending => matches!(next, Admitted | Refused | Unknown),
            Admitted => matches!(next, Running | Failed | Unknown),
            Running => matches!(next, Completed | Failed | Unknown),
            Unknown => matches!(next, Completed | Failed | Refused),
            Completed | Failed | Refused => false,
        }
    }
}

/// One record's JCS bytes with the provenance a reference needs. `event` is
/// `None` until the record has been sealed and its event id is known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signed {
    pub bytes: Vec<u8>,
    pub event: Option<EventRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRef {
    pub id: String,
    pub pubkey: String,
}

impl Signed {
    pub fn new(value: &Value) -> Result<Self, &'static str> {
        Ok(Self {
            bytes: jcs(value).map_err(|_| "record is not canonical JSON")?,
            event: None,
        })
    }

    pub fn with_event(mut self, id: &str, pubkey: &str) -> Self {
        self.event = Some(EventRef {
            id: id.to_owned(),
            pubkey: pubkey.to_owned(),
        });
        self
    }

    pub fn value(&self) -> Result<Value, &'static str> {
        parse_strict(&self.bytes).map_err(|_| "record bytes are not strict JSON")
    }

    pub fn digest(&self) -> String {
        digest_bytes(&self.bytes)
    }

    /// The `ArtifactRef` another record uses to name this one.
    pub fn reference(&self, schema: &str) -> Value {
        let mut artifact = json!({
            "digest": self.digest(),
            "size": self.bytes.len(),
            "media_type": "application/json",
            "schema": schema,
        });
        if let Some(event) = &self.event {
            artifact["event"] = json!({"id": event.id, "pubkey": event.pubkey, "kind": 3188});
        }
        artifact
    }

    /// Confirm `reference` names exactly these bytes. An event id in the
    /// reference must match a known event id; a reference without one binds
    /// by digest and size alone.
    pub fn matches(&self, reference: &Value) -> Result<(), &'static str> {
        let parsed = parse_artifact(reference).map_err(|_| "artifact reference is malformed")?;
        if parsed.digest != self.digest() || parsed.size != self.bytes.len() as u64 {
            return Err("artifact reference names other bytes");
        }
        if let (Some(want), Some(have)) = (parsed.event, &self.event)
            && (want.id != have.id || want.pubkey != have.pubkey)
        {
            return Err("artifact reference names another event");
        }
        Ok(())
    }
}

/// A parsed record envelope. `body` is checked by the per-type parsers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub kind: RecordType,
    pub purchase: String,
    pub buyer: String,
    pub provider: String,
    pub issuer: String,
    pub issued_at: u64,
    pub body: Value,
}

/// Build a record value. The caller seals it; `issuer` must be the sealing key.
pub fn record(
    kind: RecordType,
    purchase: &str,
    buyer: &str,
    provider: &str,
    issuer: &str,
    issued_at: u64,
    body: Value,
) -> Value {
    json!({
        "v": RECORD_SCHEMA,
        "requires": [],
        "type": kind.name(),
        "purchase": purchase,
        "buyer": buyer,
        "provider": provider,
        "issuer": issuer,
        "issued_at": issued_at,
        "body": body,
    })
}

/// Parse record bytes that arrived from `signer`. The issuer role is checked
/// against the record type: buyers issue requests, claims, and status queries;
/// providers issue the rest.
pub fn parse_record(bytes: &[u8], signer: &str) -> Result<Record, &'static str> {
    let value = parse_strict(bytes).map_err(|_| "record is not strict JSON")?;
    let object = value.as_object().ok_or("record is not an object")?;
    if object
        .keys()
        .any(|key| !RECORD_KEYS.contains(&key.as_str()))
    {
        return Err("record has an unknown field");
    }
    if object.get("v").and_then(Value::as_str) != Some(RECORD_SCHEMA) {
        return Err("record schema is not openagents.x402-record.v1");
    }
    match object.get("requires") {
        Some(Value::Array(items)) if items.is_empty() => {}
        _ => return Err("record requires something this version does not provide"),
    }
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .and_then(RecordType::parse)
        .ok_or("record type is unknown")?;
    let field = |name: &str| -> Result<&str, &'static str> {
        object
            .get(name)
            .and_then(Value::as_str)
            .ok_or("record field is missing")
    };
    let purchase = field("purchase")?;
    let buyer = field("buyer")?;
    let provider = field("provider")?;
    let issuer = field("issuer")?;
    if !is_hex(purchase, 64) || !is_hex(buyer, 64) || !is_hex(provider, 64) {
        return Err("record identity is not 64 hex");
    }
    if buyer == provider {
        return Err("buyer and provider are one key");
    }
    if issuer != signer {
        return Err("record issuer is not its signer");
    }
    let expected = match kind {
        RecordType::Request | RecordType::Claim | RecordType::StatusQuery => buyer,
        RecordType::Challenge | RecordType::ClaimRejected | RecordType::Status => provider,
    };
    if issuer != expected {
        return Err("record issuer holds the wrong role");
    }
    let issued_at = object
        .get("issued_at")
        .and_then(Value::as_u64)
        .ok_or("record issued_at is not a nonnegative integer")?;
    let body = object.get("body").ok_or("record body is missing")?;
    if !body.is_object() {
        return Err("record body is not an object");
    }
    Ok(Record {
        kind,
        purchase: purchase.to_owned(),
        buyer: buyer.to_owned(),
        provider: provider.to_owned(),
        issuer: issuer.to_owned(),
        issued_at,
        body: body.clone(),
    })
}

fn is_hex(text: &str, len: usize) -> bool {
    text.len() == len
        && text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// The request body, parsed. `capability` is kept as sent for the challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub capability_id: String,
    pub operation: String,
    pub input: Value,
    pub ceiling_msat: u64,
    pub fee_ceiling_msat: u64,
    pub valid_until: u64,
    pub execute_until: u64,
    pub recover_until: u64,
}

const REQUEST_KEYS: [&str; 10] = [
    "capability",
    "operation",
    "input",
    "context",
    "account",
    "ceiling_msat",
    "fee_ceiling_msat",
    "valid_until",
    "execute_until",
    "recover_until",
];

pub fn parse_request(body: &Value, issued_at: u64) -> Result<Request, &'static str> {
    let object = closed(body, &REQUEST_KEYS, "request")?;
    let capability = parse_definition(get(object, "capability")?)
        .map_err(|_| "request capability is malformed")?;
    let operation = get(object, "operation")?
        .as_str()
        .filter(|text| !text.is_empty())
        .ok_or("request operation is missing")?;
    let input = get(object, "input")?;
    parse_artifact(input).map_err(|_| "request input is malformed")?;
    if !get(object, "context")?.is_object() {
        return Err("request context is not an object");
    }
    match get(object, "account")? {
        Value::Null => {}
        Value::String(text) if !text.is_empty() => {}
        _ => return Err("request account is malformed"),
    }
    let number = |name: &str| -> Result<u64, &'static str> {
        get(object, name)?
            .as_u64()
            .ok_or("request bound is not a nonnegative integer")
    };
    let ceiling_msat = number("ceiling_msat")?;
    let fee_ceiling_msat = number("fee_ceiling_msat")?;
    let valid_until = number("valid_until")?;
    let execute_until = number("execute_until")?;
    let recover_until = number("recover_until")?;
    if ceiling_msat == 0 {
        return Err("request ceiling is zero");
    }
    if !(issued_at < valid_until && valid_until <= execute_until && execute_until <= recover_until)
    {
        return Err("request windows are out of order");
    }
    Ok(Request {
        capability_id: capability.id,
        operation: operation.to_owned(),
        input: input.clone(),
        ceiling_msat,
        fee_ceiling_msat,
        valid_until,
        execute_until,
        recover_until,
    })
}

/// A status body, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub seq: u64,
    pub prev: Option<String>,
    pub phase: Phase,
    pub settlement: Option<SettlementResponse>,
    pub output: Option<Value>,
    pub cause: Option<String>,
    pub recover_until: u64,
}

const STATUS_KEYS: [&str; 10] = [
    "request",
    "seq",
    "prev",
    "claim",
    "phase",
    "settlement",
    "run",
    "output",
    "cause",
    "recover_until",
];

pub fn parse_status(body: &Value) -> Result<Status, &'static str> {
    let object = closed(body, &STATUS_KEYS, "status")?;
    parse_artifact(get(object, "request")?).map_err(|_| "status request is malformed")?;
    let seq = get(object, "seq")?
        .as_u64()
        .ok_or("status seq is not an integer")?;
    let prev = match get(object, "prev")? {
        Value::Null => None,
        Value::String(text) if text.starts_with("sha256:") && is_hex(&text[7..], 64) => {
            Some(text.clone())
        }
        _ => return Err("status prev is malformed"),
    };
    if (seq == 0) != prev.is_none() {
        return Err("status seq and prev disagree");
    }
    let phase = get(object, "phase")?
        .as_str()
        .and_then(Phase::parse)
        .ok_or("status phase is unknown")?;
    let settlement = match get(object, "settlement")? {
        Value::Null => None,
        value => Some(
            serde_json::from_value(value.clone()).map_err(|_| "status settlement is malformed")?,
        ),
    };
    let output = match get(object, "output")? {
        Value::Null => None,
        value => {
            parse_artifact(value).map_err(|_| "status output is malformed")?;
            Some(value.clone())
        }
    };
    let cause = match get(object, "cause")? {
        Value::Null => None,
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        _ => return Err("status cause is malformed"),
    };
    let recover_until = get(object, "recover_until")?
        .as_u64()
        .ok_or("status recover_until is not an integer")?;
    let claim = get(object, "claim")?;
    let run = get(object, "run")?;
    let ok = match phase {
        Phase::Offered => claim.is_null() && settlement.is_none() && output.is_none(),
        Phase::ClaimPending => !claim.is_null() && settlement.is_none(),
        Phase::Admitted => {
            !claim.is_null()
                && settlement
                    .as_ref()
                    .is_some_and(|s: &SettlementResponse| s.success)
                && !run.is_null()
        }
        Phase::Running => !claim.is_null() && settlement.is_some() && !run.is_null(),
        Phase::Completed => settlement.is_some() && !run.is_null() && output.is_some(),
        Phase::Failed | Phase::Refused => cause.is_some(),
        Phase::Unknown => cause.is_some() && output.is_none(),
    };
    if !ok {
        return Err("status phase lacks its evidence");
    }
    Ok(Status {
        seq,
        prev,
        phase,
        settlement,
        output,
        cause,
        recover_until,
    })
}

fn closed<'a>(
    body: &'a Value,
    keys: &[&str],
    what: &'static str,
) -> Result<&'a Map<String, Value>, &'static str> {
    let object = body.as_object().ok_or(what)?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err("body does not carry exactly its fields");
    }
    Ok(object)
}

fn get<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a Value, &'static str> {
    object.get(key).ok_or("body field is missing")
}

/// The native request hash: `native_binding` over the request record's digest.
pub fn request_hash(
    buyer: &str,
    provider: &str,
    purchase: &str,
    request: &Signed,
) -> Result<String, &'static str> {
    let binding = native_binding(buyer, provider, purchase, &request.digest())
        .map_err(|_| "native binding is invalid")?;
    binding_hash(&binding).map_err(|_| "native binding does not hash")
}

/// `nostr:<npub>` names the provider; authority comes from the bound request.
pub fn resource_url(provider: &str) -> Result<String, &'static str> {
    let bytes: [u8; 32] = hex_vec(provider)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or("provider key is not 32 bytes")?;
    Ok(format!("nostr:{}", nostr::nip19::encode_npub(&bytes)))
}

fn hex_vec(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}

/// Wrap opaque bytes as the inline `openagents.x402-bytes.v1` artifact.
pub fn bytes_artifact(bytes: &[u8]) -> Value {
    json!({"v": BYTES_SCHEMA, "bytes": STANDARD.encode(bytes)})
}

/// Unwrap an `openagents.x402-bytes.v1` artifact.
pub fn artifact_bytes(value: &Value) -> Result<Vec<u8>, &'static str> {
    let object = closed(value, &["v", "bytes"], "bytes artifact")?;
    if get(object, "v")?.as_str() != Some(BYTES_SCHEMA) {
        return Err("bytes artifact schema is unknown");
    }
    let text = get(object, "bytes")?
        .as_str()
        .ok_or("bytes artifact is not text")?;
    STANDARD
        .decode(text)
        .map_err(|_| "bytes artifact is not base64")
}

/// The provider's durable view of one purchase.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Purchase {
    pub buyer: String,
    pub purchase: String,
    pub request: Signed,
    pub request_hash: String,
    pub valid_until: u64,
    pub execute_until: u64,
    pub recover_until: u64,
    pub challenge: Option<Signed>,
    pub required: Option<PaymentRequired>,
    pub claim: Option<Signed>,
    pub settlement: Option<SettlementResponse>,
    pub run: Option<Value>,
    /// Every status the provider issued, oldest first, as signed bytes.
    pub statuses: Vec<Signed>,
}

impl Purchase {
    pub fn phase(&self) -> Result<Phase, &'static str> {
        let last = self.statuses.last().ok_or("purchase has no status")?;
        let value = last.value()?;
        let body = value.get("body").ok_or("status has no body")?;
        Ok(parse_status(body)?.phase)
    }

    fn last_ref(&self) -> Option<(u64, String)> {
        let last = self.statuses.last()?;
        let value = last.value().ok()?;
        let status = parse_status(value.get("body")?).ok()?;
        Some((status.seq, last.digest()))
    }
}

/// One JSON file per `(buyer, purchase)`, replaced atomically by rename.
pub struct PurchaseStore {
    dir: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("purchase store: {0}")]
    Io(#[from] std::io::Error),
    #[error("purchase store: {0}")]
    Encode(#[from] serde_json::Error),
}

impl PurchaseStore {
    pub fn open(dir: &Path) -> Result<Self, StoreError> {
        fs::create_dir_all(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    fn path(&self, buyer: &str, purchase: &str) -> PathBuf {
        self.dir.join(format!("{buyer}-{purchase}.json"))
    }

    pub fn get(&self, buyer: &str, purchase: &str) -> Result<Option<Purchase>, StoreError> {
        match fs::read(self.path(buyer, purchase)) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn put(&self, purchase: &Purchase) -> Result<(), StoreError> {
        let path = self.path(&purchase.buyer, &purchase.purchase);
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, serde_json::to_vec(purchase)?)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Create the ledger entry exactly once.
    fn create(&self, purchase: &Purchase) -> Result<bool, StoreError> {
        let path = self.path(&purchase.buyer, &purchase.purchase);
        let mut file = match fs::File::create_new(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        use std::io::Write;
        file.write_all(&serde_json::to_vec(purchase)?)?;
        file.sync_all()?;
        Ok(true)
    }

    pub fn list(&self) -> Result<Vec<Purchase>, StoreError> {
        let mut out = Vec::new();
        for item in fs::read_dir(&self.dir)? {
            let path = item?.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            out.push(serde_json::from_slice(&fs::read(path)?)?);
        }
        Ok(out)
    }
}

/// What the provider sells: one operation of one capability at one price.
pub struct Offer {
    pub capability_id: String,
    pub operation: String,
    pub network: String,
    pub amount_msat: u64,
    pub timeout_secs: u32,
    pub description: String,
}

/// Records the provider must publish next, as unsigned record values in order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Emit {
    pub records: Vec<Value>,
}

/// Why a record was not admitted. `emit` carries any `claim_rejected` or
/// `refused` record the provider owes the buyer.
#[derive(Debug)]
pub struct Refusal {
    pub cause: &'static str,
    pub emit: Emit,
}

pub struct Provider<'a, S: ReplayStore> {
    pub pubkey: String,
    pub offer: Offer,
    pub receiver: &'a dyn Receiver,
    pub facilitator: &'a Facilitator<S>,
    pub store: &'a PurchaseStore,
    pub skew: u64,
}

/// The evidence a status carries; absent fields are `null`.
#[derive(Default)]
struct Evidence {
    claim: Value,
    settlement: Value,
    run: Value,
    output: Value,
    cause: Option<&'static str>,
}

impl<S: ReplayStore> Provider<'_, S> {
    fn status(&self, purchase: &Purchase, phase: Phase, evidence: Evidence, now: u64) -> Value {
        let Evidence {
            claim,
            settlement,
            run,
            output,
            cause,
        } = evidence;
        let (seq, prev) = match purchase.last_ref() {
            Some((seq, digest)) => (seq + 1, Value::String(digest)),
            None => (0, Value::Null),
        };
        record(
            RecordType::Status,
            &purchase.purchase,
            &purchase.buyer,
            &self.pubkey,
            &self.pubkey,
            now,
            json!({
                "request": purchase.request.reference(RECORD_SCHEMA),
                "seq": seq,
                "prev": prev,
                "claim": claim,
                "phase": phase.name(),
                "settlement": settlement,
                "run": run,
                "output": output,
                "cause": cause,
                "recover_until": purchase.recover_until,
            }),
        )
    }

    fn push_status(&self, purchase: &mut Purchase, status: &Value) -> Result<(), &'static str> {
        let signed = Signed::new(status)?;
        let phase = parse_status(status.get("body").ok_or("status body")?)?.phase;
        if let Some(current) = purchase.statuses.last() {
            let current = parse_status(current.value()?.get("body").ok_or("status body")?)?.phase;
            if !current.admits(phase) {
                return Err("status transition is not admitted");
            }
        }
        purchase.statuses.push(signed);
        Ok(())
    }

    fn persist(&self, purchase: &Purchase) -> Result<(), &'static str> {
        self.store
            .put(purchase)
            .map_err(|_| "purchase ledger write failed")
    }

    /// Answer a request with a challenge and the `offered` status, or refuse.
    /// A repeated identical request returns the stored challenge; a different
    /// request under the same purchase is an idempotency conflict.
    pub fn offer(&self, request: &Signed, now: u64) -> Result<Emit, Refusal> {
        let refuse = |cause: &'static str| Refusal {
            cause,
            emit: Emit::default(),
        };
        let value = request.value().map_err(refuse)?;
        let bytes = jcs(&value).map_err(|_| refuse("request is not canonical"))?;
        let incoming =
            parse_record(&bytes, value["issuer"].as_str().unwrap_or("")).map_err(refuse)?;
        if incoming.kind != RecordType::Request {
            return Err(refuse("record is not a request"));
        }
        if incoming.provider != self.pubkey {
            return Err(refuse("request names another provider"));
        }
        let parsed = parse_request(&incoming.body, incoming.issued_at).map_err(refuse)?;
        if let Some(existing) = self
            .store
            .get(&incoming.buyer, &incoming.purchase)
            .map_err(|_| refuse("purchase ledger read failed"))?
        {
            if existing.request.digest() != request.digest() {
                return Err(refuse("idempotency_conflict"));
            }
            let mut records = Vec::new();
            if let Some(challenge) = &existing.challenge {
                records.push(challenge.value().map_err(refuse)?);
            }
            for status in &existing.statuses {
                records.push(status.value().map_err(refuse)?);
            }
            return Ok(Emit { records });
        }
        let hash = request_hash(
            &incoming.buyer,
            &incoming.provider,
            &incoming.purchase,
            request,
        )
        .map_err(refuse)?;
        let mut purchase = Purchase {
            buyer: incoming.buyer.clone(),
            purchase: incoming.purchase.clone(),
            request: request.clone(),
            request_hash: hash.clone(),
            valid_until: parsed.valid_until,
            execute_until: parsed.execute_until,
            recover_until: parsed.recover_until,
            challenge: None,
            required: None,
            claim: None,
            settlement: None,
            run: None,
            statuses: Vec::new(),
        };
        let refused = |purchase: &mut Purchase, cause: &'static str| -> Refusal {
            let status = self.status(
                purchase,
                Phase::Refused,
                Evidence {
                    cause: Some(cause),
                    ..Evidence::default()
                },
                now,
            );
            let mut emit = Emit::default();
            if self.push_status(purchase, &status).is_ok()
                && self.store.create(purchase).is_ok_and(|created| created)
            {
                emit.records.push(status);
            }
            Refusal { cause, emit }
        };
        if parsed.capability_id != self.offer.capability_id
            || parsed.operation != self.offer.operation
        {
            return Err(refused(&mut purchase, "operation_not_offered"));
        }
        if now >= parsed.valid_until {
            return Err(refused(&mut purchase, "valid_until_passed"));
        }
        if self.offer.amount_msat > parsed.ceiling_msat {
            return Err(refused(&mut purchase, "ceiling_below_price"));
        }
        let mut request_hash_bytes = [0u8; 32];
        for (i, byte) in hex_vec(&hash)
            .unwrap_or_default()
            .iter()
            .enumerate()
            .take(32)
        {
            request_hash_bytes[i] = *byte;
        }
        let expiry = u32::try_from(parsed.valid_until.saturating_sub(now))
            .unwrap_or(u32::MAX)
            .min(self.offer.timeout_secs);
        let invoice =
            match self
                .receiver
                .invoice(self.offer.amount_msat, request_hash_bytes, expiry)
            {
                Ok(invoice) => invoice,
                Err(_) => return Err(refused(&mut purchase, "invoice_unavailable")),
            };
        let required = PaymentRequired {
            x402_version: 2,
            error: None,
            resource: ResourceInfo {
                url: resource_url(&self.pubkey).map_err(refuse)?,
                description: Some(self.offer.description.clone()),
                mime_type: None,
                rest: Map::new(),
            },
            accepts: vec![PaymentRequirements {
                scheme: "exact".into(),
                network: self.offer.network.clone(),
                amount: self.offer.amount_msat.to_string(),
                asset: "BTC".into(),
                pay_to: self.receiver.pay_to(),
                max_timeout_seconds: u64::from(expiry),
                extra: {
                    let mut extra = Map::new();
                    extra.insert("assetTransferMethod".into(), Value::String("bolt11".into()));
                    extra.insert("paymentFlow".into(), Value::String("upfront".into()));
                    extra.insert("requestHash".into(), Value::String(hash));
                    extra.insert(
                        "requestBindingProfile".into(),
                        Value::String(PROFILE.into()),
                    );
                    extra.insert("requestBindingParams".into(), Value::Object(Map::new()));
                    extra.insert("invoice".into(), Value::String(invoice));
                    extra
                },
            }],
            extensions: None,
        };
        let challenge = record(
            RecordType::Challenge,
            &incoming.purchase,
            &incoming.buyer,
            &self.pubkey,
            &self.pubkey,
            now,
            json!({
                "request": request.reference(RECORD_SCHEMA),
                "required": serde_json::to_value(&required).map_err(|_| refuse("challenge encode"))?,
            }),
        );
        purchase.challenge = Some(Signed::new(&challenge).map_err(refuse)?);
        purchase.required = Some(required);
        let status = self.status(&purchase, Phase::Offered, Evidence::default(), now);
        self.push_status(&mut purchase, &status).map_err(refuse)?;
        if !self
            .store
            .create(&purchase)
            .map_err(|_| refuse("purchase ledger write failed"))?
        {
            return Err(refuse("idempotency_conflict"));
        }
        Ok(Emit {
            records: vec![challenge, status],
        })
    }

    /// Note the event ids of records this provider published, so later
    /// references and status `prev` chains can name them.
    pub fn published(
        &self,
        buyer: &str,
        purchase: &str,
        record_digest: &str,
        event_id: &str,
    ) -> Result<(), &'static str> {
        let Some(mut entry) = self
            .store
            .get(buyer, purchase)
            .map_err(|_| "purchase ledger read failed")?
        else {
            return Err("unknown_purchase");
        };
        let event = EventRef {
            id: event_id.to_owned(),
            pubkey: self.pubkey.clone(),
        };
        let mut hit = false;
        for signed in entry.challenge.iter_mut().chain(entry.statuses.iter_mut()) {
            if signed.digest() == record_digest {
                signed.event = Some(event.clone());
                hit = true;
            }
        }
        if hit {
            self.persist(&entry)?;
        }
        Ok(())
    }

    /// Settle a claim. On success the ledger holds the settlement, the
    /// `admitted` status, and the run intent before this returns; the caller
    /// publishes the statuses and then dispatches the execution.
    pub fn claim(&self, claim: &Signed, now: u64) -> Result<Admitted, Refusal> {
        let refuse = |cause: &'static str| Refusal {
            cause,
            emit: Emit::default(),
        };
        let value = claim.value().map_err(refuse)?;
        let bytes = jcs(&value).map_err(|_| refuse("claim is not canonical"))?;
        let incoming =
            parse_record(&bytes, value["issuer"].as_str().unwrap_or("")).map_err(refuse)?;
        if incoming.kind != RecordType::Claim || incoming.provider != self.pubkey {
            return Err(refuse("record is not a claim for this provider"));
        }
        let Some(mut purchase) = self
            .store
            .get(&incoming.buyer, &incoming.purchase)
            .map_err(|_| refuse("purchase ledger read failed"))?
        else {
            return Err(refuse("unknown_purchase"));
        };
        let reject = |purchase: &Purchase, cause: &'static str| -> Refusal {
            let status = purchase
                .statuses
                .last()
                .map(|status| status.reference(RECORD_SCHEMA));
            let body = json!({
                "request": purchase.request.reference(RECORD_SCHEMA),
                "claim": claim.reference(RECORD_SCHEMA),
                "cause": cause,
                "status": status,
            });
            Refusal {
                cause,
                emit: Emit {
                    records: vec![record(
                        RecordType::ClaimRejected,
                        &purchase.purchase,
                        &purchase.buyer,
                        &self.pubkey,
                        &self.pubkey,
                        now,
                        body,
                    )],
                },
            }
        };
        let body = match closed(
            &incoming.body,
            &["request", "challenge", "payment"],
            "claim",
        ) {
            Ok(body) => body,
            Err(cause) => return Err(reject(&purchase, cause)),
        };
        if purchase.request.matches(&body["request"]).is_err() {
            return Err(reject(&purchase, "request_mismatch"));
        }
        let Some(challenge) = &purchase.challenge else {
            return Err(reject(&purchase, "purchase_refused"));
        };
        if challenge.matches(&body["challenge"]).is_err() {
            return Err(reject(&purchase, "challenge_mismatch"));
        }
        let challenge_value = challenge.value().map_err(refuse)?;
        if challenge_value["body"]["request"] != body["request"] {
            return Err(reject(&purchase, "request_mismatch"));
        }
        let payload: PaymentPayload = match serde_json::from_value(body["payment"].clone()) {
            Ok(payload) => payload,
            Err(_) => return Err(reject(&purchase, "payment_malformed")),
        };
        let phase = purchase.phase().map_err(refuse)?;
        match phase {
            Phase::Offered | Phase::ClaimPending => {}
            Phase::Admitted | Phase::Running | Phase::Completed => {
                return Err(reject(&purchase, "purchase_already_admitted"));
            }
            Phase::Failed | Phase::Refused | Phase::Unknown => {
                return Err(reject(&purchase, "purchase_terminal"));
            }
        }
        if now >= purchase.valid_until {
            return Err(reject(&purchase, "valid_until_passed"));
        }
        let Some(required) = purchase.required.clone() else {
            return Err(reject(&purchase, "purchase_refused"));
        };
        let requirements = &required.accepts[0];
        let pending = self.status(
            &purchase,
            Phase::ClaimPending,
            Evidence {
                claim: claim.reference(RECORD_SCHEMA),
                ..Evidence::default()
            },
            now,
        );
        self.push_status(&mut purchase, &pending).map_err(refuse)?;
        purchase.claim = Some(claim.clone());
        self.persist(&purchase).map_err(refuse)?;
        let key = format!("{}:{}:{}", self.pubkey, purchase.buyer, purchase.purchase);
        let mut rejected_emit = vec![pending.clone()];
        let admission = match self.facilitator.settle(requirements, &payload, &key, now) {
            Ok(admission) => admission,
            Err(response) => {
                let cause: &'static str = match response.error_reason.as_deref() {
                    Some("duplicate_settlement") => "duplicate_settlement",
                    Some("invalid_exact_lnbtc_request_binding") => "request_binding_mismatch",
                    _ => "settlement_failed",
                };
                let mut refusal = reject(&purchase, cause);
                rejected_emit.append(&mut refusal.emit.records);
                refusal.emit.records = rejected_emit;
                return Err(refusal);
            }
        };
        let run = json!({
            "v": "openagents.x402-run.v1",
            "started_by": claim.reference(RECORD_SCHEMA),
            "admitted_at": now,
            "execute_until": purchase.execute_until,
        });
        purchase.settlement = Some(admission.response.clone());
        purchase.run = Some(run.clone());
        let admitted = self.status(
            &purchase,
            Phase::Admitted,
            Evidence {
                claim: claim.reference(RECORD_SCHEMA),
                settlement: serde_json::to_value(&admission.response).unwrap_or(Value::Null),
                run,
                ..Evidence::default()
            },
            now,
        );
        self.push_status(&mut purchase, &admitted).map_err(refuse)?;
        self.persist(&purchase).map_err(refuse)?;
        Ok(Admitted {
            buyer: purchase.buyer.clone(),
            purchase: purchase.purchase.clone(),
            input: purchase.request.value().map_err(refuse)?["body"]["input"].clone(),
            execute_until: purchase.execute_until,
            emit: Emit {
                records: vec![pending, admitted],
            },
        })
    }

    /// Move an admitted purchase to `running` (`Ok(None)` when it already is),
    /// then to `completed` or `failed` through [`Provider::finish`].
    pub fn start(&self, buyer: &str, purchase: &str, now: u64) -> Result<Emit, &'static str> {
        let mut entry = self.load(buyer, purchase)?;
        if entry.phase()? != Phase::Admitted {
            return Err("purchase is not admitted");
        }
        if now >= entry.execute_until {
            return self.finish(buyer, purchase, Err("execute_until_passed"), now);
        }
        let status = self.status(
            &entry,
            Phase::Running,
            Evidence {
                claim: entry
                    .claim
                    .as_ref()
                    .map(|c| c.reference(RECORD_SCHEMA))
                    .unwrap_or(Value::Null),
                settlement: serde_json::to_value(&entry.settlement).unwrap_or(Value::Null),
                run: entry.run.clone().unwrap_or(Value::Null),
                ..Evidence::default()
            },
            now,
        );
        self.push_status(&mut entry, &status)?;
        self.persist(&entry)?;
        Ok(Emit {
            records: vec![status],
        })
    }

    /// Record the terminal outcome. `Ok(output)` is the published output
    /// artifact's reference; `Err(cause)` is a stable failure cause.
    pub fn finish(
        &self,
        buyer: &str,
        purchase: &str,
        outcome: Result<Value, &'static str>,
        now: u64,
    ) -> Result<Emit, &'static str> {
        let mut entry = self.load(buyer, purchase)?;
        let claim = entry
            .claim
            .as_ref()
            .map(|c| c.reference(RECORD_SCHEMA))
            .unwrap_or(Value::Null);
        let settlement = serde_json::to_value(&entry.settlement).unwrap_or(Value::Null);
        let run = entry.run.clone().unwrap_or(Value::Null);
        let status = match outcome {
            Ok(output) => {
                parse_artifact(&output).map_err(|_| "output reference is malformed")?;
                self.status(
                    &entry,
                    Phase::Completed,
                    Evidence {
                        claim,
                        settlement,
                        run,
                        output,
                        ..Evidence::default()
                    },
                    now,
                )
            }
            Err(cause) => self.status(
                &entry,
                Phase::Failed,
                Evidence {
                    claim,
                    settlement,
                    run,
                    cause: Some(cause),
                    ..Evidence::default()
                },
                now,
            ),
        };
        self.push_status(&mut entry, &status)?;
        self.persist(&entry)?;
        Ok(Emit {
            records: vec![status],
        })
    }

    /// Answer a status query with every status issued so far.
    pub fn statuses(&self, query: &Signed) -> Result<Emit, &'static str> {
        let value = query.value()?;
        let bytes = jcs(&value).map_err(|_| "query is not canonical")?;
        let incoming = parse_record(&bytes, value["issuer"].as_str().unwrap_or(""))?;
        if incoming.kind != RecordType::StatusQuery || incoming.provider != self.pubkey {
            return Err("record is not a status query for this provider");
        }
        let entry = self.load(&incoming.buyer, &incoming.purchase)?;
        entry.request.matches(&incoming.body["request"])?;
        Ok(Emit {
            records: entry
                .statuses
                .iter()
                .map(Signed::value)
                .collect::<Result<_, _>>()?,
        })
    }

    fn load(&self, buyer: &str, purchase: &str) -> Result<Purchase, &'static str> {
        self.store
            .get(buyer, purchase)
            .map_err(|_| "purchase ledger read failed")?
            .ok_or("unknown_purchase")
    }
}

/// A settled purchase the caller may now execute.
#[derive(Debug)]
pub struct Admitted {
    pub buyer: String,
    pub purchase: String,
    pub input: Value,
    pub execute_until: u64,
    pub emit: Emit,
}

/// Buyer side.
pub mod buyer {
    use super::{
        Emit, NATIVE_ONLY, PROFILE, RECORD_SCHEMA, Record, RecordType, Signed, closed,
        parse_record, parse_request, record, request_hash, resource_url,
    };
    use nostr::x402::{PaymentRequirements, decode_invoice, validate_challenge};
    use serde_json::{Value, json};

    use crate::wire::{PaymentPayload, PaymentRequired};

    /// The request record for one operation, with input already published.
    #[allow(clippy::too_many_arguments)]
    pub fn request(
        purchase: &str,
        buyer: &str,
        provider: &str,
        capability: Value,
        operation: &str,
        input: Value,
        ceiling_msat: u64,
        fee_ceiling_msat: u64,
        now: u64,
        valid_secs: u64,
        execute_secs: u64,
        recover_secs: u64,
    ) -> Result<Value, &'static str> {
        let body = json!({
            "capability": capability,
            "operation": operation,
            "input": input,
            "context": {},
            "account": null,
            "ceiling_msat": ceiling_msat,
            "fee_ceiling_msat": fee_ceiling_msat,
            "valid_until": now + valid_secs,
            "execute_until": now + valid_secs + execute_secs,
            "recover_until": now + valid_secs + execute_secs + recover_secs,
        });
        parse_request(&body, now)?;
        Ok(record(
            RecordType::Request,
            purchase,
            buyer,
            provider,
            buyer,
            now,
            body,
        ))
    }

    /// The one requirement of a challenge that answers `request`.
    pub struct Terms {
        pub requirements: PaymentRequirements,
        pub invoice: String,
        pub amount_msat: u64,
        pub expires_at: u64,
    }

    pub fn check_challenge(
        challenge: &Record,
        request: &Signed,
        now: u64,
        skew: u64,
    ) -> Result<Terms, &'static str> {
        let request_value = request.value()?;
        let request_record = parse_record(
            &request.bytes,
            request_value["issuer"].as_str().unwrap_or(""),
        )?;
        if challenge.kind != RecordType::Challenge
            || challenge.purchase != request_record.purchase
            || challenge.buyer != request_record.buyer
            || challenge.provider != request_record.provider
        {
            return Err("challenge does not answer this purchase");
        }
        let body = closed(&challenge.body, &["request", "required"], "challenge")?;
        request.matches(&body["request"])?;
        let parsed = parse_request(&request_record.body, request_record.issued_at)?;
        let required: PaymentRequired = serde_json::from_value(body["required"].clone())
            .map_err(|_| "challenge required is malformed")?;
        if required.x402_version != 2 || required.accepts.len() != 1 {
            return Err("challenge carries other than one x402 v2 requirement");
        }
        if required.resource.url != resource_url(&challenge.provider)? {
            return Err("challenge resource is not the provider");
        }
        let requirements = required.accepts[0].clone();
        let hash = request_hash(
            &request_record.buyer,
            &request_record.provider,
            &request_record.purchase,
            request,
        )?;
        if requirements
            .extra
            .get("requestBindingProfile")
            .and_then(Value::as_str)
            != Some(PROFILE)
        {
            return Err("challenge binding profile is not native");
        }
        let checked = validate_challenge(&requirements, &hash, now, skew, NATIVE_ONLY)
            .map_err(|_| "challenge fails x402 validation")?;
        let invoice_text = requirements
            .extra
            .get("invoice")
            .and_then(Value::as_str)
            .ok_or("challenge has no invoice")?
            .to_owned();
        let invoice = decode_invoice(&invoice_text).map_err(|_| "invoice decode")?;
        let amount_msat = invoice.amount_msat();
        if amount_msat > parsed.ceiling_msat {
            return Err("price exceeds the request ceiling");
        }
        let _ = checked;
        Ok(Terms {
            requirements,
            invoice: invoice_text,
            amount_msat,
            expires_at: invoice.created_at() + invoice.expiry_seconds(),
        })
    }

    /// The claim record after paying.
    pub fn claim(
        challenge: &Record,
        request: &Signed,
        challenge_signed: &Signed,
        payload: &PaymentPayload,
        now: u64,
    ) -> Result<Value, &'static str> {
        Ok(record(
            RecordType::Claim,
            &challenge.purchase,
            &challenge.buyer,
            &challenge.provider,
            &challenge.buyer,
            now,
            json!({
                "request": request.reference(RECORD_SCHEMA),
                "challenge": challenge_signed.reference(RECORD_SCHEMA),
                "payment": serde_json::to_value(payload).map_err(|_| "payment encode")?,
            }),
        ))
    }

    pub fn status_query(request: &Signed, record_of: &Record, now: u64) -> Emit {
        Emit {
            records: vec![record(
                RecordType::StatusQuery,
                &record_of.purchase,
                &record_of.buyer,
                &record_of.provider,
                &record_of.buyer,
                now,
                json!({"request": request.reference(RECORD_SCHEMA), "status": null}),
            )],
        }
    }
}

impl From<ContractError> for Refusal {
    fn from(_: ContractError) -> Self {
        Refusal {
            cause: "contract",
            emit: Emit::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::FileReplayStore;
    use nostr::x402::MAINNET;
    use nostr::x402::test_invoice::{fields, signed};

    // The node id of the test signer's key, [1; 32].
    const PAYEE: &str = "031b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078f";
    // `fields` pins payment hash sha256([7; 32]) and 300 s expiry.
    const PREIMAGE: [u8; 32] = [7; 32];
    const NOW: u64 = 1_700_000_000;
    // The x coordinates of 2G and 3G: valid x-only keys with no known holder.
    const BUYER: &str = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
    const PROVIDER: &str = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";

    struct Signing;

    impl Receiver for Signing {
        fn pay_to(&self) -> String {
            PAYEE.into()
        }
        fn invoice(&self, _: u64, hash: [u8; 32], _: u32) -> Result<String, String> {
            Ok(signed("lnbc250n", fields(hash), false, false))
        }
    }

    struct Bench {
        dir: PathBuf,
        facilitator: Facilitator<FileReplayStore>,
        store: PurchaseStore,
    }

    impl Bench {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("oa-x402-native-{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self {
                facilitator: Facilitator::with_profiles(
                    FileReplayStore::open(&dir.join("replay")).unwrap(),
                    60,
                    NATIVE_ONLY,
                ),
                store: PurchaseStore::open(&dir.join("purchases")).unwrap(),
                dir,
            }
        }
        fn provider(&self) -> Provider<'_, FileReplayStore> {
            Provider {
                pubkey: PROVIDER.into(),
                offer: Offer {
                    capability_id: format!("{PROVIDER}:openagents/echo"),
                    operation: "echo".into(),
                    network: MAINNET.into(),
                    amount_msat: 25_000,
                    timeout_secs: 300,
                    description: "echo".into(),
                },
                receiver: &Signing,
                facilitator: &self.facilitator,
                store: &self.store,
                skew: 60,
            }
        }
    }

    impl Drop for Bench {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn capability() -> Value {
        json!({"id": format!("{PROVIDER}:openagents/echo"), "artifact": {"digest": format!("sha256:{}", "11".repeat(32)), "size": 12, "media_type": "application/json", "schema": "openagents.capability-definition.v1"}})
    }

    fn input() -> Value {
        let inline = bytes_artifact(b"hello");
        let bytes = jcs(&inline).unwrap();
        json!({"digest": digest_bytes(&bytes), "size": bytes.len(), "media_type": "application/json", "schema": BYTES_SCHEMA})
    }

    fn request(purchase: &str, ceiling: u64) -> Signed {
        let value = buyer::request(
            purchase,
            BUYER,
            PROVIDER,
            capability(),
            "echo",
            input(),
            ceiling,
            1_000,
            NOW,
            600,
            600,
            3_600,
        )
        .unwrap();
        Signed::new(&value)
            .unwrap()
            .with_event(&"aa".repeat(32), BUYER)
    }

    fn purchase_id(n: u8) -> String {
        format!("{n:02x}").repeat(32)
    }

    fn record_of(value: &Value) -> Record {
        parse_record(&jcs(value).unwrap(), value["issuer"].as_str().unwrap()).unwrap()
    }

    fn phase_of(value: &Value) -> Phase {
        parse_status(&value["body"]).unwrap().phase
    }

    fn pay(terms: &buyer::Terms) -> PaymentPayload {
        let mut payload = Map::new();
        payload.insert("preimage".into(), json!(hex::encode(PREIMAGE)));
        PaymentPayload {
            x402_version: 2,
            resource: None,
            accepted: terms.requirements.clone(),
            payload,
            extensions: None,
        }
    }

    fn offer_and_claim(bench: &Bench, purchase: &str) -> (Signed, Record, Signed, Value) {
        let provider = bench.provider();
        let request = request(purchase, 30_000);
        let emit = provider.offer(&request, NOW).unwrap();
        assert_eq!(emit.records.len(), 2);
        let challenge = record_of(&emit.records[0]);
        assert_eq!(challenge.kind, RecordType::Challenge);
        assert_eq!(phase_of(&emit.records[1]), Phase::Offered);
        let challenge_signed = Signed::new(&emit.records[0]).unwrap();
        let terms = buyer::check_challenge(&challenge, &request, NOW, 60).unwrap();
        assert_eq!(terms.amount_msat, 25_000);
        let claim = buyer::claim(
            &challenge,
            &request,
            &challenge_signed,
            &pay(&terms),
            NOW + 1,
        )
        .unwrap();
        (request, challenge, challenge_signed, claim)
    }

    #[test]
    fn resource_url_is_the_provider_npub() {
        assert!(resource_url(PROVIDER).unwrap().starts_with("nostr:npub1"));
        assert!(resource_url("zz").is_err());
    }

    #[test]
    fn bytes_artifact_round_trips() {
        assert_eq!(artifact_bytes(&bytes_artifact(b"abc")).unwrap(), b"abc");
        assert!(artifact_bytes(&json!({"v": BYTES_SCHEMA, "bytes": "!"})).is_err());
    }

    #[test]
    fn records_check_issuer_role_and_shape() {
        let value = request(&purchase_id(1), 30_000).value().unwrap();
        assert!(
            parse_record(&jcs(&value).unwrap(), PROVIDER).is_err(),
            "issuer must sign"
        );
        let mut wrong = value.clone();
        wrong["type"] = json!("challenge");
        assert!(
            parse_record(&jcs(&wrong).unwrap(), BUYER).is_err(),
            "buyers issue no challenge"
        );
        let mut extra = value.clone();
        extra["note"] = json!(1);
        assert!(parse_record(&jcs(&extra).unwrap(), BUYER).is_err());
        let record = record_of(&value);
        assert!(parse_request(&record.body, record.issued_at).is_ok());
        let mut body = record.body.clone();
        body["valid_until"] = json!(NOW);
        assert!(
            parse_request(&body, NOW).is_err(),
            "valid_until must follow issuance"
        );
    }

    #[test]
    fn offer_claim_run_and_finish() {
        let bench = Bench::new("happy");
        let provider = bench.provider();
        let purchase = purchase_id(1);
        let (request, challenge, _, claim) = offer_and_claim(&bench, &purchase);
        assert_eq!(
            challenge.body["required"]["resource"]["url"],
            json!(resource_url(PROVIDER).unwrap())
        );
        assert_eq!(
            challenge.body["required"]["accepts"][0]["extra"]["requestHash"],
            json!(request_hash(BUYER, PROVIDER, &purchase, &request).unwrap())
        );

        let admitted = provider
            .claim(&Signed::new(&claim).unwrap(), NOW + 1)
            .unwrap();
        let phases: Vec<_> = admitted.emit.records.iter().map(phase_of).collect();
        assert_eq!(phases, vec![Phase::ClaimPending, Phase::Admitted]);
        assert_eq!(admitted.input, input());
        let ledger = bench.store.get(BUYER, &purchase).unwrap().unwrap();
        assert!(ledger.run.is_some() && ledger.settlement.as_ref().unwrap().success);
        assert_eq!(ledger.phase().unwrap(), Phase::Admitted);

        let running = provider.start(BUYER, &purchase, NOW + 2).unwrap();
        assert_eq!(phase_of(&running.records[0]), Phase::Running);
        let done = provider
            .finish(BUYER, &purchase, Ok(input()), NOW + 3)
            .unwrap();
        let status = parse_status(&done.records[0]["body"]).unwrap();
        assert_eq!(status.phase, Phase::Completed);
        assert_eq!(status.seq, 4);
        assert!(status.prev.is_some());
        assert!(
            provider
                .finish(BUYER, &purchase, Err("late"), NOW + 4)
                .is_err(),
            "terminal"
        );

        let query = buyer::status_query(&request, &challenge, NOW + 5)
            .records
            .remove(0);
        let statuses = provider.statuses(&Signed::new(&query).unwrap()).unwrap();
        assert_eq!(statuses.records.len(), 5);
        let mut prev: Option<String> = None;
        for (seq, status) in statuses.records.iter().enumerate() {
            let parsed = parse_status(&status["body"]).unwrap();
            assert_eq!(parsed.seq as usize, seq);
            assert_eq!(parsed.prev, prev);
            prev = Some(Signed::new(status).unwrap().digest());
        }
    }

    #[test]
    fn same_request_reoffers_and_changed_request_conflicts() {
        let bench = Bench::new("idem");
        let provider = bench.provider();
        let purchase = purchase_id(2);
        let first = provider.offer(&request(&purchase, 30_000), NOW).unwrap();
        let again = provider
            .offer(&request(&purchase, 30_000), NOW + 5)
            .unwrap();
        assert_eq!(first, again, "the stored challenge is returned");
        let changed = provider
            .offer(&request(&purchase, 40_000), NOW)
            .unwrap_err();
        assert_eq!(changed.cause, "idempotency_conflict");
        assert!(changed.emit.records.is_empty());
    }

    #[test]
    fn refuses_below_price_with_a_terminal_status() {
        let bench = Bench::new("cheap");
        let provider = bench.provider();
        let refusal = provider
            .offer(&request(&purchase_id(3), 10_000), NOW)
            .unwrap_err();
        assert_eq!(refusal.cause, "ceiling_below_price");
        assert_eq!(phase_of(&refusal.emit.records[0]), Phase::Refused);
        let entry = bench.store.get(BUYER, &purchase_id(3)).unwrap().unwrap();
        assert_eq!(entry.phase().unwrap(), Phase::Refused);
    }

    #[test]
    fn second_claim_is_rejected_and_proof_is_consumed_once() {
        let bench = Bench::new("twice");
        let provider = bench.provider();
        let purchase = purchase_id(4);
        let (_, _, _, claim) = offer_and_claim(&bench, &purchase);
        let signed = Signed::new(&claim).unwrap();
        provider.claim(&signed, NOW + 1).unwrap();
        let again = provider.claim(&signed, NOW + 2).unwrap_err();
        assert_eq!(again.cause, "purchase_already_admitted");
        let rejected = record_of(&again.emit.records[0]);
        assert_eq!(rejected.kind, RecordType::ClaimRejected);
        assert!(
            rejected.body["status"].is_object(),
            "authoritative status is named"
        );

        // The same proof on a second purchase is a duplicate settlement.
        let other = purchase_id(5);
        let (_, _, _, claim) = offer_and_claim(&bench, &other);
        let refusal = provider
            .claim(&Signed::new(&claim).unwrap(), NOW + 3)
            .unwrap_err();
        assert_eq!(refusal.cause, "duplicate_settlement");
        assert_eq!(
            bench
                .store
                .get(BUYER, &other)
                .unwrap()
                .unwrap()
                .phase()
                .unwrap(),
            Phase::ClaimPending
        );
    }

    #[test]
    fn buyer_refuses_a_challenge_for_another_request() {
        let bench = Bench::new("swap");
        let provider = bench.provider();
        let purchase = purchase_id(6);
        let request = request(&purchase, 30_000);
        let emit = provider.offer(&request, NOW).unwrap();
        let challenge = record_of(&emit.records[0]);
        let other = self::request(&purchase_id(7), 30_000);
        assert!(buyer::check_challenge(&challenge, &other, NOW, 60).is_err());
        assert!(
            buyer::check_challenge(&challenge, &request, NOW + 10_000, 60).is_err(),
            "expired"
        );
    }

    #[test]
    fn status_chain_rejects_bad_evidence() {
        let ok = json!({"request": input(), "seq": 0, "prev": null, "claim": null, "phase": "offered", "settlement": null, "run": null, "output": null, "cause": null, "recover_until": 1});
        assert!(parse_status(&ok).is_ok());
        let mut bad = ok.clone();
        bad["seq"] = json!(1);
        assert!(parse_status(&bad).is_err(), "seq 1 needs prev");
        let mut bad = ok.clone();
        bad["phase"] = json!("completed");
        assert!(
            parse_status(&bad).is_err(),
            "completed needs settlement, run, output"
        );
        assert!(!Phase::Completed.admits(Phase::Running));
        assert!(Phase::Unknown.admits(Phase::Failed));
    }
}
