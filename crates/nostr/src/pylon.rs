//! NIP-PYLON records: the pylon beacon (`30200`), the service receipt
//! (`3201`), and the pool aggregate (`30201`), with the reader's checks,
//! beacon freshness, aggregate recomputation, and the world projection.
//!
//! Everything here is pure: a reader passes the events it fetched and its
//! own clock, and gets a verified record or a refusal. Nothing in this
//! module grants access, admits a job, or moves money. A beacon is a claim,
//! a receipt is a buyer's claim, and an aggregate is arithmetic over claims
//! that any reader can repeat. See `nips/openagents/NIP-PYLON.md`.
//!
//! Check verdicts are NIP-32 labels ([`check`]); an aggregate counts them
//! only from the checkers its policy trusts, and a policy with
//! `exclude_failed` drops a pylon with a counted `check-fail` from
//! admission.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::contracts::jcs;
use crate::domain::{Event, MintedOwnerAttestation, RelaySigner, Tag, verify_owner_attestation};

pub mod check;
pub use check::{
    CHECK_KIND, CHECK_NAMESPACE, Check, Record, Standing, Verdict, bind_check, check_event,
    counted, parse_check, standings,
};

/// The beacon kind (addressable).
pub const BEACON_KIND: u16 = 30_200;
/// The pool aggregate kind (addressable).
pub const POOL_KIND: u16 = 30_201;
/// The service receipt kind (regular).
pub const RECEIPT_KIND: u16 = 3_201;

/// The beacon body version.
pub const BEACON_V: &str = "openagents.pylon-beacon.v1";
/// The receipt body version.
pub const RECEIPT_V: &str = "openagents.pylon-receipt.v1";
/// The pool aggregate body version.
pub const POOL_V: &str = "openagents.pylon-pool.v1";
/// The pool policy document version.
pub const POLICY_V: &str = "openagents.pylon-pool-policy.v1";

/// The beacon's `t` marker.
pub const BEACON_MARKER: &str = "oa:pylon-beacon:v1";
/// The receipt's `t` marker.
pub const RECEIPT_MARKER: &str = "oa:pylon-receipt:v1";
/// The pool aggregate's `t` marker.
pub const POOL_MARKER: &str = "oa:pylon-pool:v1";

/// A beacon or aggregate is valid for at most this long after its sample.
pub const MAX_VALIDITY_SECS: u64 = 300;
/// A beacon from further in the future than this is refused.
pub const MAX_FUTURE_SKEW_SECS: u64 = 30;
/// The longest aggregate window.
pub const MAX_WINDOW_SECS: u64 = 3_600;
/// The most beacons, receipts, or labels one aggregate may count.
pub const MAX_BEACONS: usize = 4_096;
/// The most receipts one aggregate may count.
pub const MAX_RECEIPTS: usize = 65_536;
/// The most slots one beacon may declare.
pub const MAX_SLOTS: u32 = 64;
/// The most services one beacon may declare.
pub const MAX_SERVICES: usize = 16;
/// The most pools one beacon may ask to join.
pub const MAX_POOLS: usize = 8;
/// The most `rate` slices an aggregate carries.
pub const MAX_RATE_SLICES: usize = 60;

const MEMORY_GB: &[u32] = &[8, 16, 32, 64, 128, 256, 512];

/// A pylon's availability as its beacon states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Online,
    Draining,
    Offline,
}

/// The hardware family of a pylon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    UnifiedMemory,
    Gpu,
    Cpu,
}

/// The coarse size band within a family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Small,
    Medium,
    Large,
    Xl,
}

impl Tier {
    /// The tier for a GPU with `vram_gb` of video memory.
    #[must_use]
    pub fn for_gpu(vram_gb: u32) -> Self {
        match vram_gb {
            0..=11 => Self::Small,
            12..=23 => Self::Medium,
            24..=47 => Self::Large,
            _ => Self::Xl,
        }
    }
}

/// The job lanes a service accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Lane {
    #[serde(rename = "cj-execution")]
    CjExecution,
    #[serde(rename = "cj-conversation")]
    CjConversation,
    #[serde(rename = "nip90-5050")]
    Nip90,
    /// NIP-DEC decision jobs (`25910`/`26910`/`27010`): the pylon answers
    /// `POST /v1/systemone`-shaped questions with probabilities.
    #[serde(rename = "cj-decision")]
    CjDecision,
}

/// `class` in a beacon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Class {
    pub family: Family,
    pub tier: Tier,
    pub memory_gb: u32,
}

/// `slots` in a beacon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slots {
    pub total: u32,
    pub free: u32,
}

/// One service a pylon serves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    pub capability: String,
    pub model: String,
    pub lanes: Vec<Lane>,
    pub offering: Option<String>,
    pub price_hint_msat: Option<u64>,
}

/// The `30200` beacon body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Beacon {
    pub v: String,
    pub requires: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    pub provider: String,
    pub pylon: String,
    pub label: String,
    pub status: Status,
    pub generation: u64,
    pub since: u64,
    pub observed_at: u64,
    pub valid_until: u64,
    pub class: Class,
    pub slots: Slots,
    pub services: Vec<Service>,
    pub settlement: Vec<String>,
    pub pools: Vec<String>,
}

/// How a receipt measures the work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnitKind {
    Tokens,
    Seconds,
    Jobs,
}

/// `units` in a receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Units {
    pub kind: UnitKind,
    pub count: u64,
}

/// How a job ended, from the buyer's side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Accepted,
    Rejected,
    Failed,
    Timeout,
}

/// `payment` in a receipt; null for free work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payment {
    pub profile: String,
    pub network: String,
    pub amount_msat: u64,
    pub payment_hash: String,
    pub preimage: String,
}

/// The `3201` receipt body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub v: String,
    pub requires: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    pub buyer: String,
    pub provider: String,
    pub pylon: String,
    pub lane: Lane,
    pub capability: String,
    pub request: String,
    pub request_digest: String,
    pub result_digest: Option<String>,
    pub started_at: u64,
    pub finished_at: u64,
    pub units: Units,
    pub outcome: Outcome,
    pub payment: Option<Payment>,
}

/// `{count, digest}` over one input set of an aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputSet {
    pub count: u64,
    pub digest: String,
}

/// The three input sets an aggregate counted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inputs {
    pub beacons: InputSet,
    pub receipts: InputSet,
    pub checks: InputSet,
}

/// `window` in an aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub from: u64,
    pub to: u64,
}

/// Pylons counted per family.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ByFamily {
    #[serde(rename = "unified-memory")]
    pub unified_memory: u64,
    pub gpu: u64,
    pub cpu: u64,
}

/// Jobs counted per outcome.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Jobs {
    pub accepted: u64,
    pub rejected: u64,
    pub failed: u64,
    pub timeout: u64,
}

/// Units counted per kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitTotals {
    pub tokens: u64,
    pub seconds: u64,
    pub jobs: u64,
}

/// Paid amounts per network; test networks never add to `bitcoin`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaidMsat {
    pub bitcoin: u64,
    pub testnet: u64,
    pub signet: u64,
    pub regtest: u64,
}

/// Check verdicts counted per result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckTotals {
    pub pass: u64,
    pub fail: u64,
    pub inconclusive: u64,
}

/// `totals` in an aggregate.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Totals {
    pub pylons_online: u64,
    pub slots_total: u64,
    pub slots_free: u64,
    pub by_family: ByFamily,
    pub jobs: Jobs,
    pub units: UnitTotals,
    pub paid_msat: PaidMsat,
    pub checks: CheckTotals,
}

/// The `30201` pool aggregate body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolAggregate {
    pub v: String,
    pub requires: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    pub aggregator: String,
    pub pool: String,
    pub policy: String,
    pub window: Window,
    pub inputs: Inputs,
    pub totals: Totals,
    pub rate: Vec<u64>,
    pub generated_at: u64,
    pub valid_until: u64,
}

/// A pool's policy document: which pylons it admits and whose receipts it
/// counts. `None` admits every pylon or counts every buyer that the
/// receipt rules allow. The aggregate carries the SHA-256 of its JCS bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolPolicy {
    pub v: String,
    pub pool: String,
    pub pylons: Option<BTreeSet<String>>,
    pub buyers: Option<BTreeSet<String>>,
    pub slices: u32,
    /// The checkers whose verdicts count. Empty counts no verdicts.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub checkers: BTreeSet<String>,
    /// Drop a pylon with a counted `check-fail` from admission: its
    /// beacons and receipts no longer count.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exclude_failed: bool,
}

impl PoolPolicy {
    /// A policy that admits every pylon and counts every buyer, with
    /// `slices` rate slices per window.
    #[must_use]
    pub fn open(pool: &str, slices: u32) -> Self {
        Self {
            v: POLICY_V.into(),
            pool: pool.into(),
            pylons: None,
            buyers: None,
            slices,
            checkers: BTreeSet::new(),
            exclude_failed: false,
        }
    }

    /// This policy, also counting verdicts from `checkers` and dropping
    /// pylons they fail from admission.
    #[must_use]
    pub fn checked(mut self, checkers: impl IntoIterator<Item = String>) -> Self {
        self.checkers.extend(checkers);
        self.exclude_failed = true;
        self
    }

    /// The policy's digest, as an aggregate names it.
    ///
    /// # Errors
    ///
    /// When the policy does not serialize.
    pub fn digest(&self) -> Result<String, String> {
        Ok(sha256_hex(&canonical(self)?))
    }

    fn validate(&self) -> Result<(), String> {
        if self.v != POLICY_V {
            return Err("unknown pool policy version".into());
        }
        slug(&self.pool, "pool")?;
        if self.slices == 0 || self.slices as usize > MAX_RATE_SLICES {
            return Err("a pool policy has 1 to 60 rate slices".into());
        }
        for checker in &self.checkers {
            hex64(checker, "checker")?;
        }
        Ok(())
    }
}

/// The lowercase SHA-256 hex digest of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The digest of a set of event IDs: SHA-256 of the sorted, newline-joined
/// lowercase IDs with no trailing newline.
#[must_use]
pub fn id_set_digest<'a>(ids: impl IntoIterator<Item = &'a str>) -> String {
    let sorted: BTreeSet<&str> = ids.into_iter().collect();
    let joined = sorted.into_iter().collect::<Vec<_>>().join("\n");
    sha256_hex(joined.as_bytes())
}

fn canonical<T: Serialize>(body: &T) -> Result<Vec<u8>, String> {
    let value = serde_json::to_value(body).map_err(|e| e.to_string())?;
    jcs(&value).map_err(|e| e.to_string())
}

fn hex64(value: &str, field: &str) -> Result<(), String> {
    if value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(())
    } else {
        Err(format!("{field} is not 64 lowercase hex characters"))
    }
}

fn slug(value: &str, field: &str) -> Result<(), String> {
    if !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    {
        Ok(())
    } else {
        Err(format!("{field} is not a slug of 1 to 64 [a-z0-9_-] bytes"))
    }
}

fn single_tag<'a>(event: &'a Event, name: &'a str) -> Result<&'a str, String> {
    let mut values = event.tag_values(name);
    let first = values
        .next()
        .ok_or_else(|| format!("missing `{name}` tag"))?;
    if values.next().is_some() {
        return Err(format!("more than one `{name}` tag"));
    }
    Ok(first)
}

/// Check what every NIP-PYLON event shares: kind, signature, the single
/// `t` marker, the `x` digest of the exact content, and JCS content.
fn envelope<T: for<'de> Deserialize<'de> + Serialize>(
    event: &Event,
    kind: u16,
    marker: &str,
) -> Result<T, String> {
    if event.kind != kind {
        return Err(format!("expected kind {kind}, got {}", event.kind));
    }
    event
        .validate_crypto()
        .map_err(|e| format!("bad signature: {e}"))?;
    if single_tag(event, "t")? != marker {
        return Err("wrong `t` marker".into());
    }
    if single_tag(event, "x")? != sha256_hex(event.content.as_bytes()) {
        return Err("`x` tag does not match the content".into());
    }
    let body: T = serde_json::from_str(&event.content).map_err(|e| format!("bad body: {e}"))?;
    if canonical(&body)? != event.content.as_bytes() {
        return Err("content is not the canonical JSON of its body".into());
    }
    Ok(body)
}

impl Beacon {
    /// Check the body on its own, as a publisher must before signing.
    ///
    /// # Errors
    ///
    /// Names the first rule the body breaks.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != BEACON_V {
            return Err("unknown beacon version".into());
        }
        if !self.requires.is_empty() {
            return Err("beacon requires an unknown feature".into());
        }
        hex64(&self.provider, "provider")?;
        slug(&self.pylon, "pylon")?;
        if self.label.len() > 64 {
            return Err("label is longer than 64 bytes".into());
        }
        if self.valid_until < self.observed_at
            || self.valid_until > self.observed_at + MAX_VALIDITY_SECS
        {
            return Err("valid_until is not within 300 seconds after observed_at".into());
        }
        if self.since > self.observed_at {
            return Err("since is later than observed_at".into());
        }
        if !MEMORY_GB.contains(&self.class.memory_gb) {
            return Err("memory_gb is not a listed size".into());
        }
        if self.slots.total > MAX_SLOTS || self.slots.free > self.slots.total {
            return Err("slots are out of bounds".into());
        }
        if self.services.is_empty() || self.services.len() > MAX_SERVICES {
            return Err("a beacon has 1 to 16 services".into());
        }
        for service in &self.services {
            if service.capability.is_empty() || service.capability.len() > 256 {
                return Err("service capability is empty or too long".into());
            }
            if service.model.is_empty() || service.model.len() > 128 {
                return Err("service model is empty or longer than 128 bytes".into());
            }
            let lanes: BTreeSet<_> = service.lanes.iter().collect();
            if lanes.is_empty() || lanes.len() != service.lanes.len() {
                return Err("service lanes are empty or repeated".into());
            }
        }
        let settlement: BTreeSet<_> = self.settlement.iter().collect();
        if settlement.is_empty() || settlement.len() != self.settlement.len() {
            return Err("settlement is empty or repeated".into());
        }
        if self.pools.len() > MAX_POOLS {
            return Err("a beacon asks to join at most 8 pools".into());
        }
        for pool in &self.pools {
            slug(pool, "pool")?;
        }
        Ok(())
    }

    /// The `30200` address of this beacon.
    #[must_use]
    pub fn address(&self) -> String {
        format!("{BEACON_KIND}:{}:{}", self.provider, self.pylon)
    }

    /// Whether the beacon offers `lane` for any service.
    #[must_use]
    pub fn serves(&self, lane: Lane) -> Option<&Service> {
        self.services.iter().find(|s| s.lanes.contains(&lane))
    }
}

/// Sign `beacon` as a `30200` event. The signer must be the provider.
///
/// # Errors
///
/// When the body breaks a rule or the signer is not the provider.
pub fn beacon_event(signer: &RelaySigner, beacon: &Beacon) -> Result<Event, String> {
    owned_beacon_event(signer, beacon, None)
}

/// Sign `beacon` as a `30200` event that carries `owner`, the NIP-OA
/// `auth` tag by which the pylon's owner authorized this pylon key. The
/// credential must verify on the signed event, so one minted for another
/// key or with conditions this beacon breaks is refused here.
///
/// # Errors
///
/// When the body breaks a rule, the signer is not the provider, or the
/// credential does not verify.
pub fn owned_beacon_event(
    signer: &RelaySigner,
    beacon: &Beacon,
    owner: Option<&MintedOwnerAttestation>,
) -> Result<Event, String> {
    beacon.validate()?;
    if signer.pubkey() != beacon.provider {
        return Err("the beacon's provider must sign it".into());
    }
    let content = String::from_utf8(canonical(beacon)?).map_err(|e| e.to_string())?;
    let tags = vec![
        Tag::new(vec!["d".into(), beacon.pylon.clone()]),
        Tag::new(vec!["t".into(), BEACON_MARKER.into()]),
        Tag::new(vec!["x".into(), sha256_hex(content.as_bytes())]),
        Tag::new(vec!["expiration".into(), beacon.valid_until.to_string()]),
    ];
    let mut tags = tags;
    if let Some(owner) = owner {
        tags.push(owner.tag());
    }
    let event = signer.sign(beacon.observed_at, BEACON_KIND, tags, content);
    if owner.is_some() {
        beacon_owner(&event)?;
    }
    Ok(event)
}

/// The verified NIP-OA owner of a `30200` event, or `None` when it carries
/// no `auth` tag.
///
/// # Errors
///
/// When an `auth` tag is present and does not verify under NIP-OA: a bad
/// signature, a credential for another key, more than one tag, or a
/// condition the event breaks.
pub fn beacon_owner(event: &Event) -> Result<Option<String>, String> {
    Ok(verify_owner_attestation(event)
        .map_err(|e| format!("NIP-OA owner tag refused: {e}"))?
        .map(|attestation| attestation.owner_pubkey))
}

/// Verify a `30200` event and return its body. This checks the record, not
/// its freshness; see [`freshness`].
///
/// # Errors
///
/// Names the first refusal from NIP-PYLON's Validation section.
pub fn parse_beacon(event: &Event) -> Result<Beacon, String> {
    parse_owned_beacon(event).map(|(beacon, _)| beacon)
}

/// Verify a `30200` event and return its body with its verified NIP-OA
/// owner, when it names one.
///
/// # Errors
///
/// As [`parse_beacon`].
pub fn parse_owned_beacon(event: &Event) -> Result<(Beacon, Option<String>), String> {
    let beacon: Beacon = envelope(event, BEACON_KIND, BEACON_MARKER)?;
    beacon.validate()?;
    if beacon.provider != event.pubkey {
        return Err("beacon provider is not the signer".into());
    }
    if single_tag(event, "d")? != beacon.pylon {
        return Err("`d` tag differs from the pylon slug".into());
    }
    if single_tag(event, "expiration")? != beacon.valid_until.to_string() {
        return Err("expiration differs from valid_until".into());
    }
    let owner = beacon_owner(event)?;
    Ok((beacon, owner))
}

/// How a reader judges a beacon against its own receipt time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Current: draw the pylon by its stated status.
    Fresh,
    /// Past `valid_until` or received too late: draw it as unknown.
    Stale,
    /// Sampled more than 30 seconds after the reader received it: refuse.
    Future,
}

/// Judge `beacon` received at `received_at` (the reader's clock).
#[must_use]
pub fn freshness(beacon: &Beacon, received_at: u64) -> Freshness {
    if beacon.observed_at > received_at + MAX_FUTURE_SKEW_SECS {
        Freshness::Future
    } else if received_at > beacon.valid_until
        || received_at > beacon.observed_at + MAX_VALIDITY_SECS
    {
        Freshness::Stale
    } else {
        Freshness::Fresh
    }
}

/// The newest beacon per pylon address, under the generation rule.
#[derive(Debug, Default, Clone)]
pub struct BeaconBook {
    held: BTreeMap<String, (Event, Beacon)>,
}

impl BeaconBook {
    /// Offer a verified beacon. Returns whether it replaced what was held.
    /// A lower generation, or an older sample within one generation, is
    /// ignored.
    pub fn offer(&mut self, event: Event, beacon: Beacon) -> bool {
        let address = beacon.address();
        if let Some((_, held)) = self.held.get(&address)
            && (beacon.generation < held.generation
                || (beacon.generation == held.generation && beacon.observed_at <= held.observed_at))
        {
            return false;
        }
        self.held.insert(address, (event, beacon));
        true
    }

    /// Every held beacon.
    pub fn iter(&self) -> impl Iterator<Item = &(Event, Beacon)> {
        self.held.values()
    }

    /// The held beacon for one address.
    #[must_use]
    pub fn get(&self, address: &str) -> Option<&(Event, Beacon)> {
        self.held.get(address)
    }

    /// Whether a receipt from `buyer` crediting the pylon at `address` is
    /// self-dealt and never counts: the buyer is the pylon's provider, or
    /// its verified NIP-OA owner per the held beacon.
    #[must_use]
    pub fn self_dealt(&self, buyer: &str, address: &str) -> bool {
        let Some((event, beacon)) = self.held.get(address) else {
            return false;
        };
        beacon.provider == buyer || beacon_owner(event).ok().flatten().as_deref() == Some(buyer)
    }
}

impl Receipt {
    /// Check the body on its own, as a buyer must before signing.
    ///
    /// # Errors
    ///
    /// Names the first rule the body breaks.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != RECEIPT_V {
            return Err("unknown receipt version".into());
        }
        if !self.requires.is_empty() {
            return Err("receipt requires an unknown feature".into());
        }
        hex64(&self.buyer, "buyer")?;
        hex64(&self.provider, "provider")?;
        slug(&self.pylon, "pylon")?;
        hex64(&self.request, "request")?;
        hex64(&self.request_digest, "request_digest")?;
        if let Some(digest) = &self.result_digest {
            hex64(digest, "result_digest")?;
        }
        if self.buyer == self.provider {
            return Err("a provider's receipt for its own pylon never counts".into());
        }
        if self.finished_at < self.started_at {
            return Err("finished_at is earlier than started_at".into());
        }
        if self.capability.is_empty() || self.capability.len() > 256 {
            return Err("capability is empty or too long".into());
        }
        if let Some(payment) = &self.payment {
            if !matches!(payment.profile.as_str(), "lightning-bolt11" | "x402-exact") {
                return Err("unknown payment profile".into());
            }
            if !matches!(
                payment.network.as_str(),
                "bitcoin" | "testnet" | "signet" | "regtest"
            ) {
                return Err("unknown payment network".into());
            }
            hex64(&payment.payment_hash, "payment_hash")?;
            hex64(&payment.preimage, "preimage")?;
            let mut raw = [0_u8; 32];
            for (i, byte) in raw.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&payment.preimage[2 * i..2 * i + 2], 16)
                    .map_err(|e| e.to_string())?;
            }
            if sha256_hex(&raw) != payment.payment_hash {
                return Err("preimage does not hash to the payment hash".into());
            }
        }
        Ok(())
    }

    /// The beacon address this receipt credits.
    #[must_use]
    pub fn address(&self) -> String {
        format!("{BEACON_KIND}:{}:{}", self.provider, self.pylon)
    }
}

/// Sign `receipt` as a `3201` event at `created_at`. The signer must be the
/// buyer.
///
/// # Errors
///
/// When the body breaks a rule or the signer is not the buyer.
pub fn receipt_event(
    signer: &RelaySigner,
    receipt: &Receipt,
    created_at: u64,
) -> Result<Event, String> {
    receipt.validate()?;
    if signer.pubkey() != receipt.buyer {
        return Err("the receipt's buyer must sign it".into());
    }
    let content = String::from_utf8(canonical(receipt)?).map_err(|e| e.to_string())?;
    let tags = vec![
        Tag::new(vec!["p".into(), receipt.provider.clone()]),
        Tag::new(vec!["a".into(), receipt.address()]),
        Tag::new(vec!["t".into(), RECEIPT_MARKER.into()]),
        Tag::new(vec!["x".into(), sha256_hex(content.as_bytes())]),
    ];
    Ok(signer.sign(created_at, RECEIPT_KIND, tags, content))
}

/// Verify a `3201` event and return its body. `owner` is the beacon's
/// verified NIP-OA owner, when there is one; its receipts never count.
///
/// # Errors
///
/// Names the first refusal from NIP-PYLON's Validation section.
pub fn parse_receipt(event: &Event, owner: Option<&str>) -> Result<Receipt, String> {
    let receipt: Receipt = envelope(event, RECEIPT_KIND, RECEIPT_MARKER)?;
    receipt.validate()?;
    if receipt.buyer != event.pubkey {
        return Err("receipt buyer is not the signer".into());
    }
    if owner == Some(receipt.buyer.as_str()) {
        return Err("the pylon owner's receipt never counts".into());
    }
    if single_tag(event, "p")? != receipt.provider {
        return Err("`p` tag differs from the provider".into());
    }
    if single_tag(event, "a")? != receipt.address() {
        return Err("`a` tag differs from the pylon address".into());
    }
    Ok(receipt)
}

impl PoolAggregate {
    fn validate(&self) -> Result<(), String> {
        if self.v != POOL_V {
            return Err("unknown aggregate version".into());
        }
        if !self.requires.is_empty() {
            return Err("aggregate requires an unknown feature".into());
        }
        hex64(&self.aggregator, "aggregator")?;
        slug(&self.pool, "pool")?;
        hex64(&self.policy, "policy")?;
        window(self.window)?;
        if self.rate.is_empty() || self.rate.len() > MAX_RATE_SLICES {
            return Err("rate has 1 to 60 slices".into());
        }
        if self.valid_until < self.generated_at
            || self.valid_until > self.generated_at + MAX_VALIDITY_SECS
        {
            return Err("valid_until is not within 300 seconds after generated_at".into());
        }
        Ok(())
    }
}

fn window(window: Window) -> Result<(), String> {
    if window.from % 60 != 0 || window.to % 60 != 0 {
        return Err("window bounds are not whole minutes".into());
    }
    if window.to <= window.from || window.to - window.from > MAX_WINDOW_SECS {
        return Err("window is empty or longer than an hour".into());
    }
    Ok(())
}

/// What an aggregator counts: verified beacons, receipts, and check
/// labels it fetched.
pub struct AggregateInputs<'a> {
    pub beacons: &'a [Event],
    pub receipts: &'a [Event],
    pub checks: &'a [Event],
}

/// Compute a pool's totals over `window` from raw events, under `policy`.
/// Events that fail verification, fall outside the window, or are not
/// admitted by the policy are left out, and the input digests name only
/// the events counted. `generated_at` and `valid_until` are the caller's.
///
/// # Errors
///
/// When the window, the policy, or the bounds are invalid.
pub fn compute_aggregate(
    aggregator: &str,
    policy: &PoolPolicy,
    window_: Window,
    inputs: &AggregateInputs<'_>,
    generated_at: u64,
) -> Result<PoolAggregate, String> {
    policy.validate()?;
    window(window_)?;
    hex64(aggregator, "aggregator")?;
    if inputs.beacons.len() > MAX_BEACONS
        || inputs.receipts.len() > MAX_RECEIPTS
        || inputs.checks.len() > check::MAX_CHECKS
    {
        return Err("too many inputs for one aggregate; split the pool".into());
    }

    // Verdicts: from a trusted checker, made inside the window, on a
    // receipt among the inputs that it binds to.
    let mut by_id: BTreeMap<String, Receipt> = BTreeMap::new();
    if !policy.checkers.is_empty() {
        for event in inputs.receipts {
            if let Ok(receipt) = parse_receipt(event, None) {
                by_id.insert(event.id.clone(), receipt);
            }
        }
    }
    let parsed: Vec<Check> = inputs
        .checks
        .iter()
        .filter(|e| e.created_at >= window_.from && e.created_at < window_.to)
        .filter_map(|e| parse_check(e).ok())
        .collect();
    let counted_checks = counted(&parsed, &by_id, &policy.checkers);
    let failing: BTreeSet<String> = standings(counted_checks.iter().copied(), &by_id)
        .into_iter()
        .filter(|(_, record)| record.standing == Standing::Failing)
        .map(|(address, _)| address)
        .collect();

    // Beacons: the newest valid one per pylon that names the pool and is
    // fresh (sampled at or before, and valid at) the window's end.
    let mut book = BeaconBook::default();
    let mut owners: BTreeMap<String, String> = BTreeMap::new();
    for event in inputs.beacons {
        let Ok((beacon, owner)) = parse_owned_beacon(event) else {
            continue;
        };
        if !beacon.pools.iter().any(|p| p == &policy.pool)
            || beacon.observed_at > window_.to
            || policy
                .pylons
                .as_ref()
                .is_some_and(|admitted| !admitted.contains(&beacon.address()))
            || (policy.exclude_failed && failing.contains(&beacon.address()))
        {
            continue;
        }
        let address = beacon.address();
        if book.offer(event.clone(), beacon) {
            match owner {
                Some(owner) => owners.insert(address, owner),
                None => owners.remove(&address),
            };
        }
    }
    let mut totals = Totals::default();
    for check in &counted_checks {
        match check.verdict {
            Verdict::Pass => totals.checks.pass += 1,
            Verdict::Fail => totals.checks.fail += 1,
            Verdict::Inconclusive => totals.checks.inconclusive += 1,
        }
    }
    let mut beacon_ids = Vec::new();
    let mut admitted = BTreeSet::new();
    for (event, beacon) in book.iter() {
        admitted.insert(beacon.address());
        if beacon.valid_until < window_.to {
            continue;
        }
        beacon_ids.push(event.id.as_str());
        if beacon.status == Status::Offline {
            continue;
        }
        totals.pylons_online += 1;
        totals.slots_total += u64::from(beacon.slots.total);
        totals.slots_free += u64::from(beacon.slots.free);
        match beacon.class.family {
            Family::UnifiedMemory => totals.by_family.unified_memory += 1,
            Family::Gpu => totals.by_family.gpu += 1,
            Family::Cpu => totals.by_family.cpu += 1,
        }
    }

    // Receipts: the first per (buyer, request), finished inside the window,
    // crediting a pylon the pool knows, from a buyer the policy counts.
    let mut sorted: Vec<&Event> = inputs.receipts.iter().collect();
    sorted.sort_by(|a, b| (a.created_at, &a.id).cmp(&(b.created_at, &b.id)));
    let mut seen = BTreeSet::new();
    let mut receipt_ids = Vec::new();
    let span = window_.to - window_.from;
    let slices = u64::from(policy.slices);
    let mut rate = vec![0_u64; policy.slices as usize];
    for event in sorted {
        let Ok(receipt) = parse_receipt(event, None) else {
            continue;
        };
        // A receipt from the provider or its NIP-OA owner never counts.
        if receipt.buyer == receipt.provider
            || owners.get(&receipt.address()) == Some(&receipt.buyer)
        {
            continue;
        }
        if receipt.finished_at < window_.from
            || receipt.finished_at >= window_.to
            || !admitted.contains(&receipt.address())
            || policy
                .buyers
                .as_ref()
                .is_some_and(|buyers| !buyers.contains(&receipt.buyer))
            || !seen.insert((receipt.buyer.clone(), receipt.request.clone()))
        {
            continue;
        }
        receipt_ids.push(event.id.as_str());
        match receipt.outcome {
            Outcome::Accepted => {
                totals.jobs.accepted += 1;
                let slice = (receipt.finished_at - window_.from) * slices / span;
                rate[slice as usize] += 1;
            }
            Outcome::Rejected => totals.jobs.rejected += 1,
            Outcome::Failed => totals.jobs.failed += 1,
            Outcome::Timeout => totals.jobs.timeout += 1,
        }
        match receipt.units.kind {
            UnitKind::Tokens => totals.units.tokens += receipt.units.count,
            UnitKind::Seconds => totals.units.seconds += receipt.units.count,
            UnitKind::Jobs => totals.units.jobs += receipt.units.count,
        }
        if let Some(payment) = &receipt.payment {
            let slot = match payment.network.as_str() {
                "bitcoin" => &mut totals.paid_msat.bitcoin,
                "testnet" => &mut totals.paid_msat.testnet,
                "signet" => &mut totals.paid_msat.signet,
                _ => &mut totals.paid_msat.regtest,
            };
            *slot += payment.amount_msat;
        }
    }

    Ok(PoolAggregate {
        v: POOL_V.into(),
        requires: Vec::new(),
        meta: None,
        aggregator: aggregator.into(),
        pool: policy.pool.clone(),
        policy: policy.digest()?,
        window: window_,
        inputs: Inputs {
            beacons: InputSet {
                count: beacon_ids.len() as u64,
                digest: id_set_digest(beacon_ids),
            },
            receipts: InputSet {
                count: receipt_ids.len() as u64,
                digest: id_set_digest(receipt_ids),
            },
            checks: InputSet {
                count: counted_checks.len() as u64,
                digest: id_set_digest(counted_checks.iter().map(|c| c.id.as_str())),
            },
        },
        totals,
        rate,
        generated_at,
        valid_until: generated_at + MAX_VALIDITY_SECS,
    })
}

/// Sign `aggregate` as a `30201` event. The signer must be the aggregator.
///
/// # Errors
///
/// When the body breaks a rule or the signer is not the aggregator.
pub fn aggregate_event(signer: &RelaySigner, aggregate: &PoolAggregate) -> Result<Event, String> {
    aggregate.validate()?;
    if signer.pubkey() != aggregate.aggregator {
        return Err("the aggregate's aggregator must sign it".into());
    }
    let content = String::from_utf8(canonical(aggregate)?).map_err(|e| e.to_string())?;
    let tags = vec![
        Tag::new(vec!["d".into(), aggregate.pool.clone()]),
        Tag::new(vec!["t".into(), POOL_MARKER.into()]),
        Tag::new(vec!["x".into(), sha256_hex(content.as_bytes())]),
        Tag::new(vec!["expiration".into(), aggregate.valid_until.to_string()]),
    ];
    Ok(signer.sign(aggregate.generated_at, POOL_KIND, tags, content))
}

/// Verify a `30201` event's signature, envelope, and body, and return the
/// body. This doesn't recompute the totals; see [`verify_aggregate`].
///
/// # Errors
///
/// Names the first refusal from NIP-PYLON's Validation section.
pub fn parse_aggregate(event: &Event) -> Result<PoolAggregate, String> {
    let claimed: PoolAggregate = envelope(event, POOL_KIND, POOL_MARKER)?;
    claimed.validate()?;
    if claimed.aggregator != event.pubkey {
        return Err("aggregate aggregator is not the signer".into());
    }
    if single_tag(event, "d")? != claimed.pool {
        return Err("`d` tag differs from the pool".into());
    }
    if single_tag(event, "expiration")? != claimed.valid_until.to_string() {
        return Err("expiration differs from valid_until".into());
    }
    Ok(claimed)
}

/// Verify a `30201` event and recompute it from `inputs` under `policy`.
/// Any difference in the policy digest, the input digests, the totals, or
/// the rate refuses the aggregate.
///
/// # Errors
///
/// Names the first check that fails.
pub fn verify_aggregate(
    event: &Event,
    policy: &PoolPolicy,
    inputs: &AggregateInputs<'_>,
) -> Result<PoolAggregate, String> {
    let claimed = parse_aggregate(event)?;
    let recomputed = compute_aggregate(
        &claimed.aggregator,
        policy,
        claimed.window,
        inputs,
        claimed.generated_at,
    )?;
    if recomputed.policy != claimed.policy {
        return Err("the aggregate names a different policy".into());
    }
    if recomputed.inputs != claimed.inputs {
        return Err("the input digests do not recompute".into());
    }
    if recomputed.totals != claimed.totals || recomputed.rate != claimed.rate {
        return Err("the totals do not recompute".into());
    }
    Ok(claimed)
}

/// A pylon's world state, as NIP-PYLON's World projection defines it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonState {
    /// The `30200` address.
    pub pylon: String,
    pub label: String,
    /// `online`, `draining`, `offline`, or `unknown` when stale.
    pub status: String,
    pub family: Family,
    pub tier: Tier,
    /// Slots in use: `total − free`.
    pub busy: u32,
    pub total: u32,
    /// Accepted, receipt-backed jobs the caller counted.
    pub jobs: u64,
    pub paid_msat: PaidMsat,
    /// Seconds since the online period began; zero when not fresh.
    pub uptime: u64,
}

/// Project a verified beacon into a pylon's world state at `now`. `jobs`
/// is the count of accepted receipts the caller verified for this pylon.
#[must_use]
pub fn project(beacon: &Beacon, now: u64, jobs: u64) -> PylonState {
    let fresh = freshness(beacon, now) == Freshness::Fresh;
    let status = if fresh {
        match beacon.status {
            Status::Online => "online",
            Status::Draining => "draining",
            Status::Offline => "offline",
        }
    } else {
        "unknown"
    };
    PylonState {
        pylon: beacon.address(),
        label: beacon.label.clone(),
        status: status.into(),
        family: beacon.class.family,
        tier: beacon.class.tier,
        busy: if fresh {
            beacon.slots.total - beacon.slots.free
        } else {
            0
        },
        total: beacon.slots.total,
        jobs,
        paid_msat: PaidMsat::default(),
        uptime: if fresh {
            beacon.observed_at - beacon.since
        } else {
            0
        },
    }
}

#[cfg(test)]
mod tests;
