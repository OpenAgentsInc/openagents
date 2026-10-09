//! The agent market on pooled compute (P4 of `docs/compute/verse-compute.md`).
//!
//! An agent offers a service as a public NIP-MKT offering ([`offering`]),
//! and another agent hires it through a private NIP-MKT negotiation under
//! the NIP-LAB labor profile: the buyer's `rfq`, the seller's `quote` with
//! exact terms, the buyer's `order`, and the seller's confirming
//! `order_ack` ([`Hire`]). Both sides check every record through the same
//! pure negotiation (`nostr::market_contracts::Negotiation`) at their own
//! [`Desk`]. The order's compute runs on the pool: OpenAgents' broker buys
//! one job from a pylon and signs its `3201` receipt ([`run`]). After the
//! buyer accepts the delivery, it pays the order's fixed price under
//! `lightning-bolt11-fixed-postacceptance-v1` to an invoice from
//! OpenAgents' receiver, bound to the order ([`instruction`]), and the
//! broker settles that payment in the split ledger
//! (`crate::broker::Broker::settle_order`): the seller's fee first, the
//! pylon provider's share of the compute, and OpenAgents the rest, tied to
//! the job's receipt.
//!
//! The order's compute is the seller's admitted input cost: the seller's
//! fee is the price less the broker's compute price for one job. As the
//! payment profile says, the seller and the pool bear credit risk until
//! the buyer pays; nothing here is escrow.
//!
//! Test networks first: a [`Desk`] refuses `bitcoin` terms. Nothing here
//! opens a wallet; the buyer pays with whatever `paid::Payer` its caller
//! hands it, under that payer's own ceilings.

use std::collections::BTreeMap;
use std::time::Duration;

use nostr::contracts::{ArtifactRef, ContractError, DefinitionRef, digest_bytes, jcs};
use nostr::domain::{Event, Tag};
use nostr::market_contracts::labor::{
    self, ACCEPTANCE_POLICY_SCHEMA, AcceptancePolicy, ClosureAdmission, LABOR_TERMS_SCHEMA,
    LaborProfile, LaborTerms, Parties, RIGHTS_SCHEMA, Rights,
};
use nostr::market_contracts::{
    self as mkt, DomainProfile, Ingest, LIGHTNING_PROFILE, Negotiation, OFFERING_KIND,
    OFFERING_SCHEMA, OrderRef, RECORD_SCHEMA, TERMS_SCHEMA, Terms,
};
use nostr::private_artifact;
use secp256k1::XOnlyPublicKey;
use serde::Serialize;
use serde_json::{Value, json};

use crate::client::{self, Answer, Ask};
use crate::identity::Identity;
use crate::paid::{Invoice, Network, Receiver};

/// The schema of an agent service's capability definition.
pub const SERVICE_SCHEMA: &str = "openagents.pylon.agent-service.v1";
/// The schema of a hire's input: the prompt the pool runs.
pub const INPUT_SCHEMA: &str = "openagents.pylon.agent-input.v1";
/// The one deliverable an agent service delivers.
pub const DELIVERABLE: &str = "answer";
/// The acceptance criterion: the pool answered.
pub const CRITERION: &str = "answered";
/// How long both sides keep the negotiation's records.
const RETAIN: u64 = 7 * 24 * 3_600;
/// The bound on one negotiation's retained records.
const MAX_RECORDS: usize = 64;
/// The clock skew a desk allows a peer's records.
const SKEW: u64 = 30;
/// How long an order's invoice stays payable, s.
pub const INVOICE_SECS: u32 = 600;

/// One agent's priced service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    /// The offering's slug, such as `plan-review`.
    pub offer: String,
    /// One line for the Agora's wall.
    pub summary: String,
    /// The fixed price of one order, msat.
    pub price_msat: u64,
    pub network: Network,
}

/// The JSON ArtifactRef of `value` under `schema`.
fn reference(value: &Value, schema: &str) -> Result<Value, String> {
    let bytes = jcs(value).map_err(|e| e.to_string())?;
    Ok(json!({
        "digest": digest_bytes(&bytes),
        "size": bytes.len(),
        "media_type": "application/json",
        "schema": schema,
    }))
}

fn artifact_value(a: &ArtifactRef) -> Value {
    json!({"digest": a.digest, "size": a.size, "media_type": a.media_type, "schema": a.schema})
}

fn err(e: ContractError) -> String {
    e.to_string()
}

/// The capability a seller's offering sells: the qualified ID
/// `SELLER:pylon/agent-OFFER` over a fixed definition that runs on the
/// pool.
///
/// # Errors
///
/// None in practice; JSON canonicalization of a fixed shape.
pub fn capability(seller: &str, offer: &str) -> Result<Value, String> {
    let id = format!("{seller}:pylon/agent-{offer}");
    let definition = json!({
        "v": SERVICE_SCHEMA,
        "id": id,
        "offer": offer,
        "runs_on": "pylon-pool",
    });
    Ok(json!({"id": id, "artifact": reference(&definition, SERVICE_SCHEMA)?}))
}

/// The seller's signed, immutable NIP-MKT offering for `service`, valid
/// until `valid_until`.
///
/// # Errors
///
/// A network NIP-MKT does not name (`signet`), or an offering the market
/// checker refuses (a malformed slug, an expiry not after `now`).
pub fn offering(
    seller: &Identity,
    service: &Service,
    now: u64,
    valid_until: u64,
) -> Result<Event, String> {
    if service.network == Network::Signet {
        return Err("NIP-MKT names bitcoin, testnet, and regtest only".into());
    }
    let body = json!({
        "v": OFFERING_SCHEMA,
        "requires": [],
        "provider": seller.pubkey(),
        "offer": service.offer,
        "capability": capability(seller.pubkey(), &service.offer)?,
        "profiles": [labor::PROFILE],
        "payment_profiles": [LIGHTNING_PROFILE],
        "networks": [service.network.as_str()],
        "summary": service.summary,
        "price_hint_msat": service.price_msat,
        "capacity_hint": 1,
        "valid_until": valid_until,
    });
    let content = String::from_utf8(jcs(&body).map_err(err)?).map_err(|e| e.to_string())?;
    let digest = digest_bytes(content.as_bytes());
    let event = seller.signer().sign(
        now,
        OFFERING_KIND,
        vec![
            Tag::new(vec!["t".into(), "oa:market-offering:v1".into()]),
            Tag::new(vec!["x".into(), digest[7..].into()]),
        ],
        content,
    );
    mkt::parse_offering(&event).map_err(err)?;
    Ok(event)
}

/// An offering as the Agora's services wall shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Listing {
    /// The offering's event ID.
    pub id: String,
    pub seller: String,
    pub offer: String,
    pub summary: String,
    pub price_msat: Option<u64>,
    pub networks: Vec<String>,
    pub created_at: u64,
    pub valid_until: u64,
}

impl Listing {
    /// Whether every network it sells on is a test network.
    #[must_use]
    pub fn test(&self) -> bool {
        self.networks.iter().all(|n| n != "bitcoin")
    }
}

/// The listing of a verified offering for an agent service.
///
/// # Errors
///
/// An offering the market checker refuses, or one that is not for an
/// agent service on the pool.
pub fn listing(event: &Event) -> Result<Listing, String> {
    mkt::parse_offering(event).map_err(err)?;
    let body: Value = serde_json::from_str(&event.content).map_err(|e| e.to_string())?;
    let field = |name: &str| body.get(name).cloned().unwrap_or(Value::Null);
    let offer = field("offer").as_str().unwrap_or_default().to_string();
    if field("capability")["id"].as_str()
        != Some(format!("{}:pylon/agent-{offer}", event.pubkey).as_str())
    {
        return Err("not an agent service on the pool".into());
    }
    Ok(Listing {
        id: event.id.clone(),
        seller: event.pubkey.clone(),
        offer,
        summary: field("summary").as_str().unwrap_or_default().to_string(),
        price_msat: field("price_hint_msat").as_u64(),
        networks: field("networks")
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|n| n.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        created_at: event.created_at,
        valid_until: field("valid_until").as_u64().unwrap_or_default(),
    })
}

/// The bytes every hire's labor terms reference, by digest: the host's
/// closure, which both desks resolve.
#[derive(Debug, Clone, Default)]
pub struct Closure {
    blobs: BTreeMap<String, Vec<u8>>,
    capability: Option<DefinitionRef>,
}

impl Closure {
    fn put(&mut self, value: &Value, schema: &str) -> Result<Value, String> {
        let bytes = jcs(value).map_err(err)?;
        let reference = reference(value, schema)?;
        self.blobs.insert(digest_bytes(&bytes), bytes);
        Ok(reference)
    }
}

impl ClosureAdmission for Closure {
    fn resolve(&self, reference: &ArtifactRef) -> Result<Vec<u8>, ContractError> {
        self.blobs.get(&reference.digest).cloned().ok_or_else(|| {
            ContractError::new(
                nostr::contracts::RefusalCode::ContentUnavailable,
                "agent hire closure",
            )
        })
    }

    fn check(
        &self,
        _: &Parties,
        terms: &LaborTerms,
        checker: &AcceptancePolicy,
        _: &Rights,
        capability: Option<&DefinitionRef>,
    ) -> Result<(), ContractError> {
        let refuse = |detail: &'static str| {
            Err(ContractError::new(
                nostr::contracts::RefusalCode::NotAdmitted,
                detail,
            ))
        };
        if self.capability.as_ref() != Some(&terms.execution.target)
            || capability.is_some_and(|c| c != &terms.execution.target)
        {
            return refuse("the work runs the offered agent service");
        }
        if terms.execution.input.schema.as_deref() != Some(INPUT_SCHEMA)
            || terms.deliverables.len() != 1
            || terms.deliverables[0].id != DELIVERABLE
            || checker.criteria != [CRITERION]
        {
            return refuse("an agent service delivers one checked answer");
        }
        Ok(())
    }
}

/// The NIP-LAB profile with the one payment profile the pool admits:
/// fixed-price Lightning after acceptance, on the desk's test network.
struct Profile<'a> {
    labor: LaborProfile<'a, Closure>,
    network: Network,
}

impl DomainProfile for Profile<'_> {
    fn id(&self) -> &str {
        labor::PROFILE
    }
    fn validate_request(&self, request: &ArtifactRef) -> Result<(), ContractError> {
        self.labor.validate_request(request)
    }
    fn validate_terms(
        &self,
        terms: &Terms,
        capability: &DefinitionRef,
    ) -> Result<(), ContractError> {
        self.labor.validate_terms(terms, capability)
    }
    fn validate_payment_profile(&self, terms: &Terms) -> Result<(), ContractError> {
        if terms.payment_profile == LIGHTNING_PROFILE
            && terms.network.as_deref() == Some(self.network.as_str())
        {
            Ok(())
        } else {
            Err(ContractError::new(
                nostr::contracts::RefusalCode::UnsupportedFeature,
                "the desk's payment network",
            ))
        }
    }
}

/// One hire: the offering, the parties, the prompt the pool runs, and
/// the exact labor and market terms both desks check.
#[derive(Debug, Clone)]
pub struct Hire {
    pub offering: Event,
    pub buyer: String,
    pub seller: String,
    /// The market's resolver: OpenAgents' broker key.
    pub resolver: String,
    /// The negotiation's market ID, 64 hex.
    pub market: String,
    pub prompt: String,
    pub price_msat: u64,
    pub network: Network,
    closure: Closure,
    labor: Value,
    terms: Value,
}

impl Hire {
    /// A hire of the service `offering` sells, by `buyer`, for `prompt`
    /// at `price_msat`, opened at `now`; `resolver` is the market's.
    ///
    /// # Errors
    ///
    /// An offering that does not verify, or parties that coincide.
    pub fn new(
        offering: &Event,
        buyer: &str,
        resolver: &str,
        prompt: &str,
        price_msat: u64,
        network: Network,
        now: u64,
    ) -> Result<Self, String> {
        let listing = listing(offering)?;
        let seller = listing.seller.clone();
        if buyer == seller || resolver == buyer || resolver == seller {
            return Err("a hire needs a distinct buyer, seller, and resolver".into());
        }
        let market = nostr::pylon::sha256_hex(
            format!(
                "openagents.pylon.agent-market.v1:{}:{buyer}:{now}",
                offering.id
            )
            .as_bytes(),
        );
        let mut closure = Closure::default();
        let target = capability(&seller, &listing.offer)?;
        closure.capability = Some(nostr::contracts::parse_definition(&target).map_err(err)?);
        let lock = closure.put(
            &json!({"v": "openagents.lock.v1", "runs_on": "pylon-pool"}),
            "openagents.lock.v1",
        )?;
        let deliverable_schema = {
            let schema = json!({"type": "object", "required": ["text"]});
            let bytes = jcs(&schema).map_err(err)?;
            closure.blobs.insert(digest_bytes(&bytes), bytes);
            let mut r = reference(&schema, "https://json-schema.org/draft/2020-12/schema")?;
            r["media_type"] = json!("application/schema+json");
            r
        };
        let policy = closure.put(
            &json!({
                "v": ACCEPTANCE_POLICY_SCHEMA,
                "requires": [],
                "checker": {
                    "id": format!("{buyer}:pylon/answer-check"),
                    "artifact": reference(&json!({"v": "openagents.pylon.answer-check.v1"}), "openagents.cap.v1")?,
                },
                "lock": lock,
                "criteria": [CRITERION],
                "rule": "all-pass-v1",
            }),
            ACCEPTANCE_POLICY_SCHEMA,
        )?;
        let rights = closure.put(
            &json!({
                "v": RIGHTS_SCHEMA,
                "requires": [],
                "license": reference(&json!({"v": "openagents.pylon.license.v1"}), "openagents.pylon.license.v1")?,
                "input_use": "perform-and-review-order",
                "output_use": "use-under-license",
                "publication": "deny",
                "training": "deny",
                "evaluation_reuse": "deny",
                "redistribution": "deny",
                "recipients": [buyer, seller, resolver],
                "retention": "through-market-retain-until",
            }),
            RIGHTS_SCHEMA,
        )?;
        let deadline = Deadlines::from(now);
        let labor = json!({
            "v": LABOR_TERMS_SCHEMA,
            "requires": [],
            "task_frame": closure.put(&json!({"v": "openagents.task-frame.v1", "service": listing.offer}), "openagents.task-frame.v1")?,
            "execution": {
                "target": target,
                "lock": lock,
                "input": closure.put(&json!({"v": INPUT_SCHEMA, "prompt": prompt}), INPUT_SCHEMA)?,
                "context": closure.put(&json!({"v": "openagents.context.v1"}), "openagents.context.v1")?,
                "requirements": closure.put(&json!({"v": "openagents.pylon.agent-requirements.v1", "pool": "everglade"}), "openagents.pylon.agent-requirements.v1")?,
                "bounds": [],
            },
            "deliverables": [{"id": DELIVERABLE, "schema": deliverable_schema, "max_bytes": 65_536}],
            "reviewer": buyer,
            "acceptance_policy": policy,
            "resolver": resolver,
            "resolver_policy": "labor-evidence-v1",
            "max_reworks": 0,
            "rework_due_at": null,
            "dispute_due_at": deadline.dispute,
            "resolution_due_at": deadline.resolution,
            "cancellation": "evaluate-delivered-work-v1",
            "partial_delivery": "no-partial-payment-v1",
            "buyer_unavailable": "resolver-required-v1",
            "rights": rights,
            "role_relationships": [
                {"pubkey": buyer, "operator": "buyer"},
                {"pubkey": seller, "operator": "seller"},
                {"pubkey": resolver, "operator": "openagents"},
            ],
        });
        let labor_ref = closure.put(&labor, LABOR_TERMS_SCHEMA)?;
        let terms = json!({
            "v": TERMS_SCHEMA,
            "requires": [],
            "profile": labor::PROFILE,
            "profile_terms": labor_ref,
            "buyer": buyer,
            "provider": seller,
            "worker": seller,
            "price_msat": price_msat,
            "fee_limit_msat": 0,
            "payment_profile": LIGHTNING_PROFILE,
            "network": network.as_str(),
            "quote_expires_at": deadline.quote,
            "order_confirm_by": deadline.confirm,
            "delivery_due_at": deadline.delivery,
            "review_due_at": deadline.review,
            "payment_due_at": deadline.payment,
            "retain_until": deadline.retain,
        });
        Ok(Self {
            offering: offering.clone(),
            buyer: buyer.into(),
            seller,
            resolver: resolver.into(),
            market,
            prompt: prompt.into(),
            price_msat,
            network,
            closure,
            labor: labor_ref,
            terms,
        })
    }

    /// The exact market terms the seller quotes.
    ///
    /// # Errors
    ///
    /// Terms the market checker refuses.
    pub fn terms(&self) -> Result<Terms, String> {
        mkt::parse_terms(&jcs(&self.terms).map_err(err)?).map_err(err)
    }

    fn record(
        &self,
        kind: &str,
        issuer: &str,
        seq: u64,
        prev: Option<&mkt::Record>,
        body: Value,
        now: u64,
    ) -> Value {
        json!({
            "v": RECORD_SCHEMA,
            "requires": [],
            "type": kind,
            "market": self.market,
            "buyer": self.buyer,
            "provider": self.seller,
            "issuer": issuer,
            "seq": seq,
            "prev": prev.map(|p| artifact_value(p.artifact())),
            "issued_at": now,
            "body": body,
        })
    }

    fn seal(&self, me: &Identity, value: &Value, schema: &str, now: u64) -> Result<Event, String> {
        let to = if me.pubkey() == self.buyer {
            &self.seller
        } else {
            &self.buyer
        };
        let body = json!({
            "v": "openagents.artifact-envelope.v1",
            "requires": [],
            "artifact": reference(value, schema)?,
            "inline": value,
            "issued_at": now,
            "retain_until": now + RETAIN,
        });
        let recipient = to
            .parse::<XOnlyPublicKey>()
            .map_err(|_| format!("{to} is not an x-only public key"))?;
        private_artifact::seal(
            &body,
            me.secret(),
            &recipient,
            &self.market,
            now,
            secp256k1::rand::random(),
        )
        .map_err(err)
    }

    /// The buyer's request for a quote, sealed to the seller.
    ///
    /// # Errors
    ///
    /// A malformed key or record.
    pub fn rfq(&self, buyer: &Identity, now: u64) -> Result<Event, String> {
        let offering = mkt::parse_offering(&self.offering).map_err(err)?;
        let e = offering.event();
        let body = json!({
            "offering": {"id": e.id, "pubkey": e.pubkey, "kind": e.kind},
            "profile": labor::PROFILE,
            "request": self.labor,
            "price_limit_msat": self.price_msat,
            "response_due_at": now + 600,
            "retain_until": now + RETAIN,
        });
        let value = self.record("rfq", buyer.pubkey(), 0, None, body, now);
        self.seal(buyer, &value, RECORD_SCHEMA, now)
    }

    /// The seller's quote on `rfq` and its sealed terms document.
    ///
    /// # Errors
    ///
    /// An RFQ that does not open, or a malformed record.
    pub fn quote(
        &self,
        seller: &Identity,
        rfq: &Event,
        now: u64,
    ) -> Result<(Event, Event), String> {
        let rfq = open_record(seller, rfq)?;
        let body = json!({
            "rfq": artifact_value(rfq.artifact()),
            "quote_id": nostr::pylon::sha256_hex(format!("quote:{}", self.market).as_bytes()),
            "terms": reference(&self.terms, TERMS_SCHEMA)?,
        });
        let value = self.record("quote", seller.pubkey(), 0, None, body, now);
        Ok((
            self.seal(seller, &value, RECORD_SCHEMA, now)?,
            self.seal(seller, &self.terms, TERMS_SCHEMA, now)?,
        ))
    }

    /// The buyer's order on `quote`, after `rfq`, its own previous record.
    ///
    /// # Errors
    ///
    /// Records that do not open, or a malformed record.
    pub fn order(
        &self,
        buyer: &Identity,
        rfq: &Event,
        quote: &Event,
        now: u64,
    ) -> Result<Event, String> {
        let rfq = open_record(buyer, rfq)?;
        let quote = open_record(buyer, quote)?;
        let body = json!({
            "quote": artifact_value(quote.artifact()),
            "terms_digest": reference(&self.terms, TERMS_SCHEMA)?["digest"],
            "order_id": nostr::pylon::sha256_hex(format!("order:{}", self.market).as_bytes()),
        });
        let value = self.record("order", buyer.pubkey(), 1, Some(&rfq), body, now);
        self.seal(buyer, &value, RECORD_SCHEMA, now)
    }

    /// The seller's confirming `order_ack`, after `quote`, its own
    /// previous record.
    ///
    /// # Errors
    ///
    /// Records that do not open, or a malformed record.
    pub fn ack(
        &self,
        seller: &Identity,
        quote: &Event,
        order: &Event,
        now: u64,
    ) -> Result<Event, String> {
        let quote = open_record(seller, quote)?;
        let order = open_record(seller, order)?;
        let body = json!({
            "order": artifact_value(order.artifact()),
            "decision": "confirmed",
            "code": null,
        });
        let value = self.record("order_ack", seller.pubkey(), 1, Some(&quote), body, now);
        self.seal(seller, &value, RECORD_SCHEMA, now)
    }
}

/// A hire's deadlines from when it opened.
struct Deadlines {
    quote: u64,
    confirm: u64,
    delivery: u64,
    review: u64,
    dispute: u64,
    resolution: u64,
    payment: u64,
    retain: u64,
}

impl Deadlines {
    fn from(now: u64) -> Self {
        Self {
            quote: now + 600,
            confirm: now + 900,
            delivery: now + 3_600,
            review: now + 7_200,
            dispute: now + 7_200,
            resolution: now + 10_800,
            payment: now + 86_400,
            retain: now + RETAIN,
        }
    }
}

/// Open a market record sealed to or by `me`.
///
/// # Errors
///
/// An envelope that does not open or a record the checker refuses.
pub fn open_record(me: &Identity, event: &Event) -> Result<mkt::Record, String> {
    let opened = private_artifact::open(event, me.secret()).map_err(err)?;
    mkt::parse_record(&opened, None).map_err(err)
}

/// One side's view of a hire: every record checked, in order, through the
/// market's pure negotiation under the NIP-LAB profile.
pub struct Desk {
    me: Identity,
    hire: Hire,
    negotiation: Negotiation,
}

impl Desk {
    /// `me`'s desk for `hire`.
    ///
    /// # Errors
    ///
    /// `me` is neither party, the hire is on `bitcoin` (mainnet agent
    /// orders wait for the owner's gate), or the offering does not verify.
    pub fn new(me: Identity, hire: Hire) -> Result<Self, String> {
        if me.pubkey() != hire.buyer && me.pubkey() != hire.seller {
            return Err("this key is neither party to the hire".into());
        }
        if !hire.network.is_test() {
            return Err("agent orders run on test networks until the owner's gate opens".into());
        }
        let offering = mkt::parse_offering(&hire.offering).map_err(err)?;
        let negotiation =
            Negotiation::new(offering, &hire.buyer, &hire.market, MAX_RECORDS).map_err(err)?;
        Ok(Self {
            me,
            hire,
            negotiation,
        })
    }

    /// Check one record (and, for a quote, its sealed terms) at `now`.
    ///
    /// # Errors
    ///
    /// A record that does not open or that the negotiation refuses.
    pub fn ingest(
        &mut self,
        record: &Event,
        terms: Option<&Event>,
        now: u64,
    ) -> Result<Ingest, String> {
        let record = open_record(&self.me, record)?;
        let document = match terms {
            Some(event) => {
                let opened = private_artifact::open(event, self.me.secret()).map_err(err)?;
                Some(mkt::parse_terms_document(&opened, None).map_err(err)?)
            }
            None => None,
        };
        let parties = Parties {
            buyer: self.hire.buyer.clone(),
            provider: self.hire.seller.clone(),
            worker: self.hire.seller.clone(),
        };
        let profile = Profile {
            labor: LaborProfile::new(parties, &self.hire.closure).map_err(err)?,
            network: self.hire.network,
        };
        self.negotiation
            .ingest(record, document.as_ref(), &profile, now, SKEW)
            .map_err(err)
    }

    /// The confirmed order, once the seller's `order_ack` is in.
    #[must_use]
    pub fn confirmed(&self) -> Option<&OrderRef> {
        self.negotiation.confirmed()
    }

    #[must_use]
    pub fn hire(&self) -> &Hire {
        &self.hire
    }
}

/// The digest an order's invoice binds as its description hash: the
/// order's market, ID, and parties.
///
/// # Errors
///
/// None in practice; JSON canonicalization of a fixed shape.
pub fn binding(order: &OrderRef) -> Result<[u8; 32], String> {
    let bytes = jcs(&json!({
        "v": "openagents.pylon.agent-order-binding.v1",
        "market": order.market,
        "order_id": order.order_id,
        "buyer": order.buyer,
        "provider": order.provider,
    }))
    .map_err(err)?;
    let hex = nostr::pylon::sha256_hex(&bytes);
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// The earned-price invoice for a confirmed `order` at `terms`' fixed
/// price, from OpenAgents' `receiver` and bound to the order: what the
/// buyer pays after it accepts the delivery.
///
/// # Errors
///
/// A receiver that cannot issue it, or an invoice that does not decode.
pub fn instruction(
    receiver: &dyn Receiver,
    order: &OrderRef,
    terms: &Terms,
) -> Result<Invoice, String> {
    if terms.payment_profile != LIGHTNING_PROFILE || terms.price_msat == 0 {
        return Err("only fixed-price Lightning terms are payable".into());
    }
    let bolt11 = receiver.invoice(terms.price_msat, binding(order)?, INVOICE_SECS)?;
    let decoded = nostr::x402::decode_invoice(&bolt11).map_err(|e| format!("{e:?}"))?;
    Ok(Invoice {
        bolt11,
        payment_hash: decoded
            .payment_hash()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        amount_msat: decoded.amount_msat(),
    })
}

/// Run a confirmed order's compute on the pool: OpenAgents' `broker` key
/// buys one job for `hire`'s prompt from the best fresh pylon on `relay`
/// (or `pylon`), skipping pylons the `checkers` failed, and signs its
/// receipt. The order pays for it later, so the job carries no payment.
///
/// # Errors
///
/// No order yet, and the client's errors.
pub async fn run(
    broker: &Identity,
    desk: &Desk,
    relay: &str,
    pylon: Option<String>,
    home: &std::path::Path,
    checkers: std::collections::BTreeSet<String>,
) -> Result<Answer, String> {
    if desk.confirmed().is_none() {
        return Err("the order is not confirmed".into());
    }
    client::ask(
        broker,
        &Ask {
            relay: relay.into(),
            pylon,
            prompt: desk.hire().prompt.clone(),
            wait: Duration::from_secs(60),
            publish_receipt: true,
            home: home.to_path_buf(),
            checkers,
            pay: None,
        },
    )
    .await
}

/// The buyer's acceptance check: the pool answered with text within the
/// deliverable's bound. Returns the answer.
///
/// # Errors
///
/// No answer, or an answer over the bound.
pub fn accept(answer: &Answer) -> Result<&str, String> {
    let text = answer
        .text
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .ok_or("the pool did not answer")?;
    if text.len() > 65_536 {
        return Err("the answer is over the deliverable's bound".into());
    }
    if answer.receipt_event.is_none() {
        return Err("the job has no receipt".into());
    }
    Ok(text)
}

/// The brokers whose jobs the Agora counts as sales and draws settlement
/// threads for: the npubs or hex keys in `OPENAGENTS_PYLON_BROKERS`,
/// comma-separated. OpenAgents publishes no broker key yet, so an unset
/// variable trusts none.
#[must_use]
pub fn brokers() -> std::collections::BTreeSet<String> {
    std::env::var("OPENAGENTS_PYLON_BROKERS")
        .map(|list| {
            list.split(',')
                .filter_map(|k| crate::identity::hex_pubkey(k.trim()))
                .collect()
        })
        .unwrap_or_default()
}
