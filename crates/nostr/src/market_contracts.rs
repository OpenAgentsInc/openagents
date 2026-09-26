//! NIP-MKT offering and no-spend negotiation validation.
//!
//! These pure checks retain per-party history and exact commercial identities.
//! They perform no storage, network I/O, capacity reservation, execution, or
//! payment. Hosts must persist the history and admit domain-profile semantics
//! separately before confirming or executing work. Payment records fail closed.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::contracts::{
    ArtifactRef, ContractError, DefinitionRef, EventRef, RefusalCode, SAFE_INTEGER, digest_bytes,
    jcs, parse_artifact, parse_definition, parse_strict,
};
use crate::domain::Event;
use crate::private_artifact::OpenEnvelope;

pub mod labor;

pub const OFFERING_KIND: u16 = 3192;
pub const HEAD_KIND: u16 = 30192;
pub const OFFERING_SCHEMA: &str = "openagents.market-offering.v1";
pub const HEAD_SCHEMA: &str = "openagents.market-head.v1";
pub const TERMS_SCHEMA: &str = "openagents.market-terms.v1";
pub const RECORD_SCHEMA: &str = "openagents.market-record.v1";
pub const FREE_PROFILE: &str = "free-v1";
pub const LIGHTNING_PROFILE: &str = "lightning-bolt11-fixed-postacceptance-v1";

/// Parsed public offering. Construction verifies the event signature and tags.
#[derive(Debug, Clone)]
pub struct Offering {
    event: EventRef,
    provider: String,
    offer: String,
    capability: DefinitionRef,
    profiles: Vec<String>,
    payment_profiles: Vec<String>,
    networks: Vec<String>,
    valid_until: u64,
}

impl Offering {
    #[must_use]
    pub fn event(&self) -> &EventRef {
        &self.event
    }
    #[must_use]
    pub fn capability(&self) -> &DefinitionRef {
        &self.capability
    }
}

/// The discovery head is an observation, never an order amendment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Active,
    Paused,
    Withdrawn,
}

/// Verify an immutable offering's closed content, signature, and digest tag.
pub fn parse_offering(event: &Event) -> Result<Offering, ContractError> {
    let value = public_body(event, OFFERING_KIND, "oa:market-offering:v1")?;
    let m = versioned(
        &value,
        OFFERING_SCHEMA,
        &[
            "provider",
            "offer",
            "capability",
            "profiles",
            "payment_profiles",
            "networks",
            "summary",
            "price_hint_msat",
            "capacity_hint",
            "valid_until",
        ],
    )?;
    let provider = key(field(m, "provider")?, "provider")?;
    equal(&provider, &event.pubkey, "offering signer")?;
    let offer = slug(field(m, "offer")?, "offer")?;
    let profiles = strings(field(m, "profiles")?, true, "profiles")?;
    let payment_profiles = strings(field(m, "payment_profiles")?, true, "payment_profiles")?;
    if payment_profiles
        .iter()
        .any(|p| p != FREE_PROFILE && p != LIGHTNING_PROFILE)
    {
        return Err(unsupported("payment profile"));
    }
    let networks = strings(field(m, "networks")?, false, "networks")?;
    if networks
        .iter()
        .any(|n| !["bitcoin", "testnet", "regtest"].contains(&n.as_str()))
    {
        return Err(unsupported("network"));
    }
    if payment_profiles.iter().any(|p| p == LIGHTNING_PROFILE) == networks.is_empty() {
        return Err(malformed("offering networks"));
    }
    display(field(m, "summary")?, "summary")?;
    optional_uint(field(m, "price_hint_msat")?, "price_hint_msat")?;
    optional_uint(field(m, "capacity_hint")?, "capacity_hint")?;
    let valid_until = uint(field(m, "valid_until")?, "valid_until")?;
    if valid_until <= event.created_at {
        return Err(malformed("offering expiry"));
    }
    let digest = digest_bytes(event.content.as_bytes());
    one_tag(event, "x", &digest[7..])?;
    Ok(Offering {
        event: EventRef {
            id: event.id.clone(),
            pubkey: event.pubkey.clone(),
            kind: OFFERING_KIND,
            coordinate: None,
        },
        provider,
        offer,
        capability: parse_definition(field(m, "capability")?)?,
        profiles,
        payment_profiles,
        networks,
        valid_until,
    })
}

/// A signed discovery hint whose referenced offering may still be unavailable.
#[derive(Debug, Clone)]
pub struct Head {
    provider: String,
    offer: String,
    offering: EventRef,
    availability: Availability,
    valid_until: u64,
}

/// Validate a public head without claiming that its referenced offering exists.
/// Relays can apply this shape/signature check without fetching another event.
pub fn parse_head(event: &Event) -> Result<Head, ContractError> {
    let value = public_body(event, HEAD_KIND, "oa:market-head:v1")?;
    let m = versioned(
        &value,
        HEAD_SCHEMA,
        &["provider", "offer", "offering", "status", "valid_until"],
    )?;
    let provider = key(field(m, "provider")?, "provider")?;
    equal(&event.pubkey, &provider, "head signer")?;
    let offer = slug(field(m, "offer")?, "offer")?;
    one_tag(event, "d", &offer)?;
    let offering = event_reference(field(m, "offering")?)?;
    if offering.kind != OFFERING_KIND || offering.pubkey != provider {
        return Err(identity("head offering author/kind"));
    }
    let valid_until = uint(field(m, "valid_until")?, "valid_until")?;
    if valid_until <= event.created_at {
        return Err(malformed("head expiry"));
    }
    let availability = match text(field(m, "status")?, "status")? {
        "active" => Availability::Active,
        "paused" => Availability::Paused,
        "withdrawn" => Availability::Withdrawn,
        _ => return Err(unsupported("head status")),
    };
    Ok(Head {
        provider,
        offer,
        offering,
        availability,
        valid_until,
    })
}

/// Check an exact offering head. `now` is the observing host's receipt time.
/// Expired heads are unavailable for new discovery; old agreements are unaffected.
pub fn check_head(
    event: &Event,
    offering: &Offering,
    now: u64,
) -> Result<Availability, ContractError> {
    let head = parse_head(event)?;
    equal(&head.provider, &offering.provider, "head provider")?;
    equal(&head.offer, &offering.offer, "head offer")?;
    if head.offering != offering.event {
        return Err(identity("head offering"));
    }
    if now >= head.valid_until || now >= offering.valid_until {
        return Err(stale("offering head"));
    }
    Ok(head.availability)
}

/// Closed market terms. Parsing does not validate the domain terms they name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terms {
    pub profile: String,
    pub profile_terms: ArtifactRef,
    pub buyer: String,
    pub provider: String,
    pub worker: String,
    pub price_msat: u64,
    pub fee_limit_msat: u64,
    pub payment_profile: String,
    pub network: Option<String>,
    pub quote_expires_at: u64,
    pub order_confirm_by: u64,
    pub delivery_due_at: u64,
    pub review_due_at: u64,
    pub payment_due_at: u64,
    pub retain_until: u64,
}

/// Parse exact JCS bytes with the MKT deadline and payment-profile invariants.
pub fn parse_terms(bytes: &[u8]) -> Result<Terms, ContractError> {
    let value = canonical(bytes)?;
    let m = versioned(
        &value,
        TERMS_SCHEMA,
        &[
            "profile",
            "profile_terms",
            "buyer",
            "provider",
            "worker",
            "price_msat",
            "fee_limit_msat",
            "payment_profile",
            "network",
            "quote_expires_at",
            "order_confirm_by",
            "delivery_due_at",
            "review_due_at",
            "payment_due_at",
            "retain_until",
        ],
    )?;
    let terms = Terms {
        profile: identifier(field(m, "profile")?, "profile")?,
        profile_terms: structured(field(m, "profile_terms")?, None)?,
        buyer: key(field(m, "buyer")?, "buyer")?,
        provider: key(field(m, "provider")?, "provider")?,
        worker: key(field(m, "worker")?, "worker")?,
        price_msat: uint(field(m, "price_msat")?, "price_msat")?,
        fee_limit_msat: uint(field(m, "fee_limit_msat")?, "fee_limit_msat")?,
        payment_profile: identifier(field(m, "payment_profile")?, "payment_profile")?,
        network: match field(m, "network")? {
            Value::Null => None,
            v => Some(identifier(v, "network")?),
        },
        quote_expires_at: uint(field(m, "quote_expires_at")?, "quote_expires_at")?,
        order_confirm_by: uint(field(m, "order_confirm_by")?, "order_confirm_by")?,
        delivery_due_at: uint(field(m, "delivery_due_at")?, "delivery_due_at")?,
        review_due_at: uint(field(m, "review_due_at")?, "review_due_at")?,
        payment_due_at: uint(field(m, "payment_due_at")?, "payment_due_at")?,
        retain_until: uint(field(m, "retain_until")?, "retain_until")?,
    };
    if terms.buyer == terms.provider {
        return Err(malformed("distinct commercial parties"));
    }
    if !(terms.quote_expires_at <= terms.order_confirm_by
        && terms.order_confirm_by < terms.delivery_due_at
        && terms.delivery_due_at < terms.review_due_at
        && terms.review_due_at < terms.payment_due_at
        && terms.payment_due_at < terms.retain_until)
    {
        return Err(malformed("market deadline ordering"));
    }
    match terms.payment_profile.as_str() {
        FREE_PROFILE
            if terms.price_msat == 0 && terms.fee_limit_msat == 0 && terms.network.is_none() => {}
        FREE_PROFILE => return Err(malformed("free terms cannot spend")),
        LIGHTNING_PROFILE
            if terms.price_msat > 0
                && terms
                    .network
                    .as_deref()
                    .is_some_and(|n| ["bitcoin", "testnet", "regtest"].contains(&n)) => {}
        LIGHTNING_PROFILE => return Err(malformed("Lightning terms")),
        _ => return Err(unsupported("payment profile")),
    }
    Ok(terms)
}

/// Only the negotiation records supported by this no-spend implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    Rfq {
        offering: EventRef,
        profile: String,
        request: ArtifactRef,
        price_limit_msat: u64,
        response_due_at: u64,
        retain_until: u64,
    },
    Quote {
        rfq: ArtifactRef,
        quote_id: String,
        terms: ArtifactRef,
    },
    Order {
        quote: ArtifactRef,
        terms_digest: String,
        order_id: String,
    },
    OrderAck {
        order: ArtifactRef,
        confirmed: bool,
        code: Option<String>,
    },
}

/// A closed record with authenticated origin. Fields cannot be forged by callers.
#[derive(Debug, Clone)]
pub struct Record {
    artifact: ArtifactRef,
    issuer: String,
    recipient: String,
    declarations: Vec<EventRef>,
    market: String,
    buyer: String,
    provider: String,
    seq: u64,
    prev: Option<ArtifactRef>,
    issued_at: u64,
    body: Body,
    canonical: Vec<u8>,
}

impl Record {
    #[must_use]
    pub fn artifact(&self) -> &ArtifactRef {
        &self.artifact
    }
    #[must_use]
    pub fn body(&self) -> &Body {
        &self.body
    }
    #[must_use]
    pub fn issuer(&self) -> &str {
        &self.issuer
    }
}

/// Parse a market record only from its authenticated encrypted declaration.
/// External bytes are accepted only when they match the envelope's exact pin.
pub fn parse_record(
    envelope: &OpenEnvelope,
    external: Option<&[u8]>,
) -> Result<Record, ContractError> {
    let bytes = envelope_bytes(envelope, external, RECORD_SCHEMA)?;
    let record = record_body(
        bytes,
        envelope.signer(),
        envelope.recipient(),
        declaration(envelope),
    )?;
    if !same_artifact(&record.artifact, envelope.artifact()) {
        return Err(identity("market record artifact"));
    }
    Ok(record)
}

/// Exact market terms authenticated by their provider's private envelope.
#[derive(Debug, Clone)]
pub struct TermsDocument {
    artifact: ArtifactRef,
    declaration: EventRef,
    signer: String,
    terms: Terms,
}

/// Authenticate a closed market-terms artifact. Domain semantics remain separate.
pub fn parse_terms_document(
    envelope: &OpenEnvelope,
    external: Option<&[u8]>,
) -> Result<TermsDocument, ContractError> {
    let bytes = envelope_bytes(envelope, external, TERMS_SCHEMA)?;
    let terms = parse_terms(bytes)?;
    equal(envelope.signer(), &terms.provider, "terms signer")?;
    equal(envelope.recipient(), &terms.buyer, "terms recipient")?;
    Ok(TermsDocument {
        artifact: envelope.artifact().clone(),
        declaration: declaration(envelope),
        signer: envelope.signer().into(),
        terms,
    })
}

fn envelope_bytes<'a>(
    envelope: &'a OpenEnvelope,
    external: Option<&'a [u8]>,
    schema: &str,
) -> Result<&'a [u8], ContractError> {
    if envelope.artifact().schema.as_deref() != Some(schema)
        || envelope.artifact().media_type != "application/json"
    {
        return Err(identity("market envelope schema"));
    }
    let bytes = envelope.inline_bytes().or(external).ok_or_else(|| {
        ContractError::new(RefusalCode::ContentUnavailable, "market artifact bytes")
    })?;
    envelope.check_external(bytes)?;
    Ok(bytes)
}
fn declaration(envelope: &OpenEnvelope) -> EventRef {
    EventRef {
        id: envelope.event_id().into(),
        pubkey: envelope.signer().into(),
        kind: 3188,
        coordinate: None,
    }
}

fn record_body(
    bytes: &[u8],
    issuer: &str,
    recipient: &str,
    declaration: EventRef,
) -> Result<Record, ContractError> {
    let value = canonical(bytes)?;
    let m = versioned(
        &value,
        RECORD_SCHEMA,
        &[
            "type",
            "market",
            "buyer",
            "provider",
            "issuer",
            "seq",
            "prev",
            "issued_at",
            "body",
        ],
    )?;
    let buyer = key(field(m, "buyer")?, "buyer")?;
    let provider = key(field(m, "provider")?, "provider")?;
    if buyer == provider {
        return Err(malformed("distinct commercial parties"));
    }
    let declared = key(field(m, "issuer")?, "issuer")?;
    equal(&declared, issuer, "record signer")?;
    if issuer != buyer && issuer != provider {
        return Err(not_admitted("record issuer"));
    }
    if recipient != buyer && recipient != provider {
        return Err(not_admitted("negotiation recipient"));
    }
    let seq = uint(field(m, "seq")?, "seq")?;
    let prev = match field(m, "prev")? {
        Value::Null => None,
        v => Some(structured(v, Some(RECORD_SCHEMA))?),
    };
    if (seq == 0) != prev.is_none() {
        return Err(malformed("record predecessor"));
    }
    let issued_at = uint(field(m, "issued_at")?, "issued_at")?;
    let b = object(field(m, "body")?, "body")?;
    let body = match text(field(m, "type")?, "type")? {
        "rfq" => {
            role(issuer, &buyer)?;
            closed(
                b,
                &[
                    "offering",
                    "profile",
                    "request",
                    "price_limit_msat",
                    "response_due_at",
                    "retain_until",
                ],
            )?;
            let response_due_at = uint(field(b, "response_due_at")?, "response_due_at")?;
            let retain_until = uint(field(b, "retain_until")?, "retain_until")?;
            if issued_at >= response_due_at || response_due_at >= retain_until {
                return Err(malformed("RFQ deadlines"));
            }
            let offering = event_reference(field(b, "offering")?)?;
            if offering.kind != OFFERING_KIND || offering.pubkey != provider {
                return Err(identity("RFQ offering"));
            }
            Body::Rfq {
                offering,
                profile: identifier(field(b, "profile")?, "profile")?,
                request: structured(field(b, "request")?, None)?,
                price_limit_msat: uint(field(b, "price_limit_msat")?, "price_limit_msat")?,
                response_due_at,
                retain_until,
            }
        }
        "quote" => {
            role(issuer, &provider)?;
            closed(b, &["rfq", "quote_id", "terms"])?;
            Body::Quote {
                rfq: structured(field(b, "rfq")?, Some(RECORD_SCHEMA))?,
                quote_id: hex(field(b, "quote_id")?, "quote_id")?,
                terms: structured(field(b, "terms")?, Some(TERMS_SCHEMA))?,
            }
        }
        "order" => {
            role(issuer, &buyer)?;
            closed(b, &["quote", "terms_digest", "order_id"])?;
            Body::Order {
                quote: structured(field(b, "quote")?, Some(RECORD_SCHEMA))?,
                terms_digest: digest(field(b, "terms_digest")?, "terms_digest")?,
                order_id: hex(field(b, "order_id")?, "order_id")?,
            }
        }
        "order_ack" => {
            role(issuer, &provider)?;
            closed(b, &["order", "decision", "code"])?;
            let confirmed = match text(field(b, "decision")?, "decision")? {
                "confirmed" => true,
                "refused" => false,
                _ => return Err(unsupported("order decision")),
            };
            let code = match field(b, "code")? {
                Value::Null => None,
                v => Some(display(v, "code")?),
            };
            if confirmed != code.is_none() || code.as_ref().is_some_and(String::is_empty) {
                return Err(malformed("order refusal code"));
            }
            Body::OrderAck {
                order: structured(field(b, "order")?, Some(RECORD_SCHEMA))?,
                confirmed,
                code,
            }
        }
        _ => {
            return Err(unsupported(
                "market record type; payment and fulfillment are unsupported",
            ));
        }
    };
    Ok(Record {
        artifact: ArtifactRef {
            digest: digest_bytes(bytes),
            size: bytes.len() as u64,
            media_type: "application/json".into(),
            schema: Some(RECORD_SCHEMA.into()),
            event: None,
            sources: vec![],
        },
        issuer: issuer.into(),
        recipient: recipient.into(),
        declarations: vec![declaration],
        market: hex(field(m, "market")?, "market")?,
        buyer,
        provider,
        seq,
        prev,
        issued_at,
        body,
        canonical: bytes.to_vec(),
    })
}

/// Exact bilateral order identity, available only after a confirmed ack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderRef {
    pub market: String,
    pub order_id: String,
    pub buyer: String,
    pub provider: String,
    pub order: ArtifactRef,
    pub confirmation: ArtifactRef,
}

/// Parse a claimed OrderRef. Use [`Negotiation::check_order_ref`] to resolve it.
pub fn parse_order_ref(value: &Value) -> Result<OrderRef, ContractError> {
    let m = object(value, "order reference")?;
    closed(
        m,
        &[
            "market",
            "order_id",
            "buyer",
            "provider",
            "order",
            "confirmation",
        ],
    )?;
    let reference = OrderRef {
        market: hex(field(m, "market")?, "market")?,
        order_id: hex(field(m, "order_id")?, "order_id")?,
        buyer: key(field(m, "buyer")?, "buyer")?,
        provider: key(field(m, "provider")?, "provider")?,
        order: structured(field(m, "order")?, Some(RECORD_SCHEMA))?,
        confirmation: structured(field(m, "confirmation")?, Some(RECORD_SCHEMA))?,
    };
    if reference.buyer == reference.provider {
        return Err(malformed("distinct commercial parties"));
    }
    Ok(reference)
}

/// A caller implements the domain's exact request/terms semantics and complete
/// reference closure. Returning success attests only to that local validation.
/// This module contains no default that accepts arbitrary domain JSON.
pub trait DomainProfile {
    fn id(&self) -> &str;
    fn validate_request(&self, request: &ArtifactRef) -> Result<(), ContractError>;
    fn validate_terms(
        &self,
        terms: &Terms,
        capability: &DefinitionRef,
    ) -> Result<(), ContractError>;
}

/// Progress in the retained pure negotiation. None of these values dispatches work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ingest {
    Applied,
    Duplicate,
    Gap,
    Conflict,
}

/// A bounded, in-memory reconstruction for one provider and buyer negotiation.
/// Persist accepted records and conflict evidence before returning an external
/// acknowledgment. A fresh instance does not provide durable replay protection.
#[derive(Debug, Clone)]
pub struct Negotiation {
    offering: Offering,
    buyer: String,
    market: String,
    max_records: usize,
    records: BTreeMap<String, Record>,
    slots: BTreeMap<(String, u64), BTreeSet<String>>,
    tips: BTreeMap<String, String>,
    applied: BTreeSet<String>,
    terms: BTreeMap<String, Terms>,
    quote_ids: BTreeMap<String, String>,
    order_ids: BTreeMap<String, String>,
    consumed_quotes: BTreeMap<String, String>,
    decisions: BTreeMap<String, String>,
    confirmed: Option<OrderRef>,
    conflict: bool,
}

impl Negotiation {
    /// Start an exact negotiation after the caller checks fresh active discovery
    /// and resolves and admits the offered capability. This checker grants neither.
    pub fn new(
        offering: Offering,
        buyer: &str,
        market: &str,
        max_records: usize,
    ) -> Result<Self, ContractError> {
        key(&Value::String(buyer.into()), "buyer")?;
        hex(&Value::String(market.into()), "market")?;
        if buyer == offering.provider || max_records == 0 || max_records > 4096 {
            return Err(malformed("negotiation bounds or parties"));
        }
        Ok(Self {
            offering,
            buyer: buyer.into(),
            market: market.into(),
            max_records,
            records: BTreeMap::new(),
            slots: BTreeMap::new(),
            tips: BTreeMap::new(),
            applied: BTreeSet::new(),
            terms: BTreeMap::new(),
            quote_ids: BTreeMap::new(),
            order_ids: BTreeMap::new(),
            consumed_quotes: BTreeMap::new(),
            decisions: BTreeMap::new(),
            confirmed: None,
            conflict: false,
        })
    }

    #[must_use]
    pub fn conflicted(&self) -> bool {
        self.conflict
    }

    /// Every retained record, including gaps and equivocations. Hosts also retain
    /// original signed envelopes, resolved terms, and observed receipt times;
    /// these plaintext records alone cannot reconstruct authenticated admission.
    pub fn retained(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.records
            .iter()
            .map(|(digest, record)| (digest.as_str(), record.canonical.as_slice()))
    }

    /// A confirmed commercial reference is unavailable during a conflict.
    #[must_use]
    pub fn confirmed(&self) -> Option<&OrderRef> {
        if self.conflict {
            None
        } else {
            self.confirmed.as_ref()
        }
    }

    pub fn check_order_ref(&self, reference: &OrderRef) -> Result<(), ContractError> {
        let confirmed = self
            .confirmed()
            .ok_or_else(|| not_admitted("no unconflicted confirmed order"))?;
        if reference.market != confirmed.market
            || reference.order_id != confirmed.order_id
            || reference.buyer != confirmed.buyer
            || reference.provider != confirmed.provider
        {
            return Err(identity("order identity"));
        }
        self.resolve(&reference.order)?;
        self.resolve(&reference.confirmation)?;
        if !same_artifact(&reference.order, &confirmed.order)
            || !same_artifact(&reference.confirmation, &confirmed.confirmation)
        {
            return Err(identity("order evidence"));
        }
        Ok(())
    }

    /// Apply one authenticated record with the host's receipt time. `terms`
    /// is required only for quotes and checked against the signed quote's exact
    /// ArtifactRef. `skew` is a host-declared allowance, not a sender field.
    /// Only free-v1 terms can be confirmed by this implementation.
    pub fn ingest(
        &mut self,
        record: Record,
        terms: Option<&TermsDocument>,
        profile: &impl DomainProfile,
        now: u64,
        skew: u64,
    ) -> Result<Ingest, ContractError> {
        if record.market != self.market
            || record.buyer != self.buyer
            || record.provider != self.offering.provider
        {
            return Err(identity("negotiation identity"));
        }
        if record.recipient == record.issuer {
            return Err(not_admitted("negotiation peer recipient"));
        }
        let digest = record.artifact.digest.clone();
        if let Some(retained) = self.records.get_mut(&digest) {
            for proof in &record.declarations {
                if !retained.declarations.contains(proof) {
                    if retained.declarations.len() >= 64 {
                        return Err(ContractError::new(
                            RefusalCode::LimitExceeded,
                            "market declaration copies",
                        ));
                    }
                    retained.declarations.push(proof.clone());
                }
            }
        }
        if self.applied.contains(&digest) {
            return Ok(Ingest::Duplicate);
        }
        if !self.records.contains_key(&digest) && self.records.len() >= self.max_records {
            return Err(ContractError::new(
                RefusalCode::LimitExceeded,
                "market retained records",
            ));
        }
        let slot = (record.issuer.clone(), record.seq);
        self.records
            .entry(digest.clone())
            .or_insert_with(|| record.clone());
        self.slots
            .entry(slot.clone())
            .or_default()
            .insert(digest.clone());
        if self.slots[&slot].len() > 1 {
            self.conflict = true;
        }
        if self.conflict {
            return Ok(Ingest::Conflict);
        }
        if let Some(prev) = &record.prev {
            let Some(previous) = self.records.get(&prev.digest) else {
                return Ok(Ingest::Gap);
            };
            if !self.applied.contains(&prev.digest) {
                return Ok(Ingest::Gap);
            }
            reference_matches(prev, previous)?;
            if previous.issuer != record.issuer
                || previous.seq.checked_add(1) != Some(record.seq)
                || self.tips.get(&record.issuer) != Some(&prev.digest)
            {
                self.conflict = true;
                return Ok(Ingest::Conflict);
            }
        } else if self.tips.contains_key(&record.issuer) {
            self.conflict = true;
            return Ok(Ingest::Conflict);
        }
        if record.issued_at > now.saturating_add(skew) {
            return Err(stale("future record"));
        }
        self.transition(&record, terms, profile, now, skew)?;
        self.tips.insert(record.issuer, digest.clone());
        self.applied.insert(digest);
        Ok(Ingest::Applied)
    }

    fn resolve(&self, reference: &ArtifactRef) -> Result<&Record, ContractError> {
        let record = self
            .records
            .get(&reference.digest)
            .filter(|_| self.applied.contains(&reference.digest))
            .ok_or_else(|| {
                ContractError::new(
                    RefusalCode::ContentUnavailable,
                    "market predecessor or cross-reference",
                )
            })?;
        reference_matches(reference, record)?;
        Ok(record)
    }

    fn transition(
        &mut self,
        record: &Record,
        terms_document: Option<&TermsDocument>,
        profile: &impl DomainProfile,
        now: u64,
        skew: u64,
    ) -> Result<(), ContractError> {
        match &record.body {
            Body::Rfq {
                offering,
                profile: selected,
                request,
                response_due_at,
                ..
            } => {
                if offering != &self.offering.event {
                    return Err(identity("RFQ exact offering"));
                }
                if selected != profile.id() || !self.offering.profiles.contains(selected) {
                    return Err(unsupported("domain profile"));
                }
                fresh(now, *response_due_at, skew, "RFQ response deadline")?;
                fresh(now, self.offering.valid_until, skew, "offering expiry")?;
                profile.validate_request(request)?;
            }
            Body::Quote {
                rfq,
                quote_id,
                terms,
            } => {
                let original = self.resolve(rfq)?;
                let Body::Rfq {
                    profile: selected,
                    price_limit_msat,
                    response_due_at,
                    ..
                } = &original.body
                else {
                    return Err(identity("quote RFQ type"));
                };
                fresh(now, *response_due_at, skew, "quote response deadline")?;
                if record.issued_at >= *response_due_at {
                    return Err(stale("quote issued after RFQ deadline"));
                }
                let document = terms_document.ok_or_else(|| {
                    ContractError::new(RefusalCode::ContentUnavailable, "quote terms")
                })?;
                if !same_artifact(terms, &document.artifact)
                    || terms
                        .event
                        .as_ref()
                        .is_some_and(|e| e != &document.declaration)
                    || document.signer != record.issuer
                {
                    return Err(identity("quote terms declaration"));
                }
                let parsed = document.terms.clone();
                if parsed.buyer != self.buyer
                    || parsed.provider != self.offering.provider
                    || &parsed.profile != selected
                {
                    return Err(identity("quote parties or profile"));
                }
                if profile.id() != selected {
                    return Err(unsupported("domain profile"));
                }
                if parsed.price_msat > *price_limit_msat {
                    return Err(ContractError::new(
                        RefusalCode::LimitExceeded,
                        "price_exceeded",
                    ));
                }
                if !self
                    .offering
                    .payment_profiles
                    .contains(&parsed.payment_profile)
                    || parsed
                        .network
                        .as_ref()
                        .is_some_and(|n| !self.offering.networks.contains(n))
                {
                    return Err(unsupported("offered payment profile/network"));
                }
                if parsed.payment_profile != FREE_PROFILE {
                    return Err(unsupported("paid negotiation not implemented"));
                }
                if parsed.quote_expires_at <= record.issued_at {
                    return Err(stale("quote issuance expiry"));
                }
                fresh(now, parsed.quote_expires_at, skew, "quote expiry")?;
                profile.validate_terms(&parsed, &self.offering.capability)?;
                unique(
                    &self.quote_ids,
                    quote_id,
                    &record.artifact.digest,
                    "quote identity",
                )?;
                self.quote_ids
                    .insert(quote_id.clone(), record.artifact.digest.clone());
                self.terms.insert(record.artifact.digest.clone(), parsed);
            }
            Body::Order {
                quote,
                terms_digest,
                order_id,
            } => {
                let quoted = self.resolve(quote)?;
                let Body::Quote { terms, .. } = &quoted.body else {
                    return Err(identity("order quote type"));
                };
                equal(terms_digest, &terms.digest, "accepted terms digest")?;
                let parsed = self
                    .terms
                    .get(&quote.digest)
                    .ok_or_else(|| identity("missing validated terms"))?;
                fresh(now, parsed.quote_expires_at, skew, "quote_expired")?;
                if record.issued_at >= parsed.quote_expires_at {
                    return Err(stale("order signed after quote expiry"));
                }
                unique(
                    &self.order_ids,
                    order_id,
                    &record.artifact.digest,
                    "order identity",
                )?;
                unique(
                    &self.consumed_quotes,
                    &quote.digest,
                    &record.artifact.digest,
                    "quote_consumed",
                )?;
                if self.confirmed.is_some() {
                    return Err(not_admitted("negotiation already confirmed"));
                }
                self.order_ids
                    .insert(order_id.clone(), record.artifact.digest.clone());
                self.consumed_quotes
                    .insert(quote.digest.clone(), record.artifact.digest.clone());
            }
            Body::OrderAck {
                order, confirmed, ..
            } => {
                let ordered = self.resolve(order)?;
                let Body::Order {
                    quote, order_id, ..
                } = &ordered.body
                else {
                    return Err(identity("ack order type"));
                };
                let parsed = self
                    .terms
                    .get(&quote.digest)
                    .ok_or_else(|| identity("missing validated terms"))?;
                unique(
                    &self.decisions,
                    &order.digest,
                    &record.artifact.digest,
                    "order decision",
                )?;
                if *confirmed {
                    fresh(now, parsed.order_confirm_by, skew, "confirmation_expired")?;
                    if record.issued_at > parsed.order_confirm_by {
                        return Err(stale("confirmation issuance"));
                    }
                    if self.confirmed.is_some() {
                        return Err(not_admitted("negotiation already confirmed"));
                    }
                    self.confirmed = Some(OrderRef {
                        market: self.market.clone(),
                        order_id: order_id.clone(),
                        buyer: self.buyer.clone(),
                        provider: self.offering.provider.clone(),
                        order: order.clone(),
                        confirmation: record.artifact.clone(),
                    });
                }
                self.decisions
                    .insert(order.digest.clone(), record.artifact.digest.clone());
            }
        }
        Ok(())
    }
}

fn unique(
    index: &BTreeMap<String, String>,
    key: &str,
    digest: &str,
    detail: &str,
) -> Result<(), ContractError> {
    if index.get(key).is_some_and(|old| old != digest) {
        return Err(ContractError::new(RefusalCode::IdempotencyConflict, detail));
    }
    Ok(())
}
fn fresh(now: u64, deadline: u64, skew: u64, detail: &str) -> Result<(), ContractError> {
    if now.saturating_sub(skew) >= deadline {
        Err(stale(detail))
    } else {
        Ok(())
    }
}
fn reference_matches(reference: &ArtifactRef, record: &Record) -> Result<(), ContractError> {
    if !same_artifact(reference, &record.artifact)
        || reference
            .event
            .as_ref()
            .is_some_and(|e| !record.declarations.contains(e))
    {
        return Err(identity("record reference"));
    }
    Ok(())
}
fn same_artifact(a: &ArtifactRef, b: &ArtifactRef) -> bool {
    a.digest == b.digest && a.size == b.size && a.media_type == b.media_type && a.schema == b.schema
}
fn public_body(event: &Event, kind: u16, marker: &str) -> Result<Value, ContractError> {
    if event.kind != kind {
        return Err(unsupported("market event kind"));
    }
    event
        .validate_nip01_structure()
        .map_err(|_| malformed("market event structure"))?;
    event
        .validate_crypto()
        .map_err(|_| identity("market event signature"))?;
    one_tag(event, "t", marker)?;
    canonical(event.content.as_bytes())
}
fn one_tag(event: &Event, name: &str, value: &str) -> Result<(), ContractError> {
    let tags: Vec<_> = event
        .tags
        .iter()
        .filter(|t| t.name() == Some(name) && (name != "t" || t.value() == Some(value)))
        .collect();
    if tags.len() != 1 || tags[0].0.len() != 2 || tags[0].value() != Some(value) {
        return Err(identity("market semantic tag"));
    }
    Ok(())
}
fn canonical(bytes: &[u8]) -> Result<Value, ContractError> {
    let value = parse_strict(bytes)?;
    if jcs(&value)? != bytes {
        return Err(malformed("market artifact must use JCS"));
    }
    Ok(value)
}
fn object<'a>(value: &'a Value, detail: &str) -> Result<&'a Map<String, Value>, ContractError> {
    value.as_object().ok_or_else(|| malformed(detail))
}
fn field<'a>(m: &'a Map<String, Value>, name: &str) -> Result<&'a Value, ContractError> {
    m.get(name).ok_or_else(|| malformed(name))
}
fn closed(m: &Map<String, Value>, fields: &[&str]) -> Result<(), ContractError> {
    if m.keys().any(|k| !fields.contains(&k.as_str())) {
        return Err(unsupported("market semantic field"));
    }
    for name in fields {
        field(m, name)?;
    }
    Ok(())
}
fn versioned<'a>(
    v: &'a Value,
    version: &str,
    fields: &[&str],
) -> Result<&'a Map<String, Value>, ContractError> {
    let m = object(v, "market object")?;
    if text(field(m, "v")?, "v")? != version {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "market schema",
        ));
    }
    if !field(m, "requires")?
        .as_array()
        .ok_or_else(|| malformed("requires"))?
        .is_empty()
    {
        return Err(unsupported("required market feature"));
    }
    if m.get("meta").is_some_and(|v| !v.is_object()) {
        return Err(malformed("meta"));
    }
    if m.keys()
        .any(|k| !["v", "requires", "meta"].contains(&k.as_str()) && !fields.contains(&k.as_str()))
    {
        return Err(unsupported("market semantic field"));
    }
    for name in fields {
        field(m, name)?;
    }
    Ok(m)
}
fn text<'a>(v: &'a Value, detail: &str) -> Result<&'a str, ContractError> {
    v.as_str().ok_or_else(|| malformed(detail))
}
fn uint(v: &Value, detail: &str) -> Result<u64, ContractError> {
    v.as_u64()
        .filter(|n| i128::from(*n) <= SAFE_INTEGER)
        .ok_or_else(|| malformed(detail))
}
fn optional_uint(v: &Value, detail: &str) -> Result<Option<u64>, ContractError> {
    if v.is_null() {
        Ok(None)
    } else {
        uint(v, detail).map(Some)
    }
}
fn hex(v: &Value, detail: &str) -> Result<String, ContractError> {
    let s = text(v, detail)?;
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(malformed(detail));
    }
    Ok(s.into())
}
fn key(v: &Value, detail: &str) -> Result<String, ContractError> {
    let s = hex(v, detail)?;
    s.parse::<secp256k1::XOnlyPublicKey>()
        .map_err(|_| malformed(detail))?;
    Ok(s)
}
fn identifier(v: &Value, detail: &str) -> Result<String, ContractError> {
    let s = text(v, detail)?;
    if s.is_empty() || s.len() > 128 || !s.is_ascii() {
        return Err(malformed(detail));
    }
    Ok(s.into())
}
fn display(v: &Value, detail: &str) -> Result<String, ContractError> {
    let s = text(v, detail)?;
    if s.len() > 2048 {
        return Err(ContractError::new(RefusalCode::LimitExceeded, detail));
    }
    Ok(s.into())
}
fn slug(v: &Value, detail: &str) -> Result<String, ContractError> {
    let s = text(v, detail)?;
    if s.is_empty()
        || s.len() > 64
        || !s
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
        || !s.as_bytes()[0].is_ascii_alphanumeric()
    {
        return Err(malformed(detail));
    }
    Ok(s.into())
}
fn digest(v: &Value, detail: &str) -> Result<String, ContractError> {
    let s = text(v, detail)?;
    let suffix = s.strip_prefix("sha256:").ok_or_else(|| malformed(detail))?;
    hex(&Value::String(suffix.into()), detail)?;
    Ok(s.into())
}
fn strings(v: &Value, nonempty: bool, detail: &str) -> Result<Vec<String>, ContractError> {
    let values = v.as_array().ok_or_else(|| malformed(detail))?;
    if values.len() > 64 || nonempty && values.is_empty() {
        return Err(ContractError::new(RefusalCode::LimitExceeded, detail));
    }
    let strings = values
        .iter()
        .map(|v| identifier(v, detail))
        .collect::<Result<Vec<_>, _>>()?;
    if strings.iter().collect::<BTreeSet<_>>().len() != strings.len() {
        return Err(malformed("duplicate list entry"));
    }
    Ok(strings)
}
fn structured(v: &Value, schema: Option<&str>) -> Result<ArtifactRef, ContractError> {
    let a = parse_artifact(v)?;
    if a.media_type != "application/json"
        || a.schema.is_none()
        || schema.is_some_and(|s| a.schema.as_deref() != Some(s))
    {
        return Err(identity("structured artifact schema/media type"));
    }
    Ok(a)
}
fn event_reference(v: &Value) -> Result<EventRef, ContractError> {
    let m = object(v, "event reference")?;
    closed(m, &["id", "pubkey", "kind"])?;
    let kind = u16::try_from(uint(field(m, "kind")?, "kind")?).map_err(|_| malformed("kind"))?;
    Ok(EventRef {
        id: hex(field(m, "id")?, "event id")?,
        pubkey: key(field(m, "pubkey")?, "event pubkey")?,
        kind,
        coordinate: None,
    })
}
fn role(actual: &str, expected: &str) -> Result<(), ContractError> {
    if actual == expected {
        Ok(())
    } else {
        Err(not_admitted("market record role"))
    }
}
fn equal(a: &str, b: &str, detail: &str) -> Result<(), ContractError> {
    if a == b {
        Ok(())
    } else {
        Err(identity(detail))
    }
}
fn malformed(d: &str) -> ContractError {
    ContractError::new(RefusalCode::Malformed, d)
}
fn unsupported(d: &str) -> ContractError {
    ContractError::new(RefusalCode::UnsupportedFeature, d)
}
fn identity(d: &str) -> ContractError {
    ContractError::new(RefusalCode::IdentityMismatch, d)
}
fn not_admitted(d: &str) -> ContractError {
    ContractError::new(RefusalCode::NotAdmitted, d)
}
fn stale(d: &str) -> ContractError {
    ContractError::new(RefusalCode::Stale, d)
}

#[cfg(test)]
mod tests;
