//! The payment router (#11136): one `402` that carries every live
//! challenge, and one settle path that reads whichever credential comes
//! back.
//!
//! The router sits in front of a paid HTTP resource (the inference
//! gateway's keyless path is the first). For an unpaid request it issues
//! **one** Lightning invoice for the quote and asks every configured
//! [`Adapter`] to add its encoding of that invoice to the `402`: x402
//! `exact`/`lnbtc` in `PAYMENT-REQUIRED`, the HTTP `Payment` scheme's
//! Lightning `charge` (MPP) in `WWW-Authenticate`. For a paid retry it
//! finds the adapter whose credential the request carries, and that
//! adapter verifies it and consumes the invoice's replay key
//! (`lnbtc:<network>:<payment_hash>`) once in the shared store, so one
//! preimage pays once whichever encoding carries it.
//!
//! # Adding a method
//!
//! A new method is one [`Adapter`]: say which [`Method`] it is
//! ([`Adapter::method`]), recognise its credential on a request
//! ([`Adapter::credential`]), add its challenge to an unpaid answer
//! ([`Adapter::challenge`]), and settle a credential to a [`Settled`]
//! carrying a replay key ([`Adapter::settle`]). The gateway never names a
//! method: it asks the router for the challenge, the settlement, and the
//! live list ([`Router::methods`]) that every discovery surface prints.
//!
//! - **L402** encodes the same invoice with a macaroon bound to its payment
//!   hash (`WWW-Authenticate: L402 macaroon=…, invoice=…`), and settles
//!   through [`Settle::lightning`] like the two adapters here, so it shares
//!   their replay key. [`Method::L402`] is reserved; no adapter exists yet,
//!   so nothing can configure or advertise it.
//! - **Stripe** (MPP `stripe`, a Shared Payment Token) settles on its own
//!   rail and returns its own replay key (`stripe:<payment_intent>`); it
//!   ignores the shared invoice in [`Offer`]. Taproot Assets stablecoins
//!   (tap-ldk) come later over Lightning. We take no EVM, Solana or Tempo
//!   stablecoins; Cashu is not planned for now (owner, 2026-10-09).
//!
//! Every adapter follows the same rules as the x402 front: settle before
//! execute, one replay key per payment, release only when nothing was
//! answered. Nothing here pays, and no bearer secret (a preimage, a token,
//! a proof) is kept or returned.

use std::sync::Arc;

use nostr::x402::{PaymentRequirements, decode_invoice};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::facilitator::{Admission, Facilitator};
use crate::payment_scheme::{self, Challenge, PAYMENT_RECEIPT, Problem, Terms, WWW_AUTHENTICATE};
use crate::replay::ReplayStore;
use crate::server::Receiver;
use crate::wire::{PaymentRequired, ResourceInfo, decode_payment_payload, encode_header};
use crate::{PAYMENT_REQUIRED, PAYMENT_RESPONSE, PAYMENT_SIGNATURE, PaymentPayload};
use crate::{SettlementResponse, facilitator};

/// A payment method the router can speak. Only the ones with an adapter
/// can be live; the others are typed slots for adapters still to come.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    /// x402 v2 `exact` on Lightning (`lnbtc`).
    X402,
    /// The HTTP `Payment` scheme, Lightning `charge` (as MPP uses it).
    Mpp,
    /// L402 on the same invoice. Reserved: no adapter yet.
    L402,
    /// Cashu NUT-24. Reserved: no adapter yet.
    Cashu,
}

impl Method {
    /// The id discovery and receipts use.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::X402 => "x402",
            Self::Mpp => "mpp",
            Self::L402 => "l402",
            Self::Cashu => "cashu",
        }
    }
}

/// What a live method looks like to a buyer: what every discovery surface
/// prints and the `402` body lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MethodInfo {
    /// `x402`, `mpp`.
    pub id: &'static str,
    /// A plain name.
    pub name: &'static str,
    /// The protocol a receipt names (`x402`, `mpp`).
    pub protocol: &'static str,
    /// The rail the money moves on (`lightning`).
    pub rail: &'static str,
    /// The network the invoice is on, as x402 names it.
    pub network: String,
    /// The asset (`BTC`).
    pub asset: &'static str,
    /// Where the challenge is in the `402`.
    pub challenge: &'static str,
    /// What the paid retry sends.
    pub credential: &'static str,
    /// What a paid answer carries back.
    pub receipt: &'static str,
    /// The specification.
    pub spec: &'static str,
}

/// The request being paid for, exactly as it was received.
pub struct Bound<'a> {
    pub http_method: &'a str,
    /// The absolute URL the payment binds (origin plus path).
    pub url: &'a str,
    pub body: &'a [u8],
    /// The `http:1` binding hash of method, URL, and body, hex.
    pub request_hash: &'a str,
    pub headers: &'a [(String, String)],
}

impl Bound<'_> {
    /// The first header named `name`, case-insensitively.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// The one price every challenge carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quote {
    pub amount_msat: u64,
    pub usd_micros: u64,
}

/// The shared invoice of one `402`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invoice {
    pub bolt11: String,
    pub payment_hash: String,
    /// When the invoice stops being payable, Unix seconds.
    pub expires_at: u64,
}

/// What an adapter needs to write its challenge.
pub struct Offer<'a> {
    pub request: &'a Bound<'a>,
    pub quote: Quote,
    pub invoice: &'a Invoice,
    /// The x402 terms of the shared invoice (also what a Lightning
    /// credential is checked against).
    pub terms: &'a PaymentRequirements,
    /// The invoice's x402 network identifier.
    pub network: &'static str,
    pub resource: &'a ResourceInfo,
    /// The x402 `extensions` (Bazaar) for this resource, if any.
    pub extensions: Option<&'a Map<String, Value>>,
    /// Why a presented credential was refused, when this `402` answers one.
    pub refusal: Option<&'a str>,
    pub now: u64,
}

/// An unpaid answer being assembled: headers to send and fields for the
/// JSON body.
#[derive(Debug, Default, Clone)]
pub struct Challenged {
    pub headers: Vec<(String, String)>,
    pub body: Map<String, Value>,
    /// The shared invoice's payment hash, for the operator's log.
    pub payment_hash: String,
}

/// A credential that settled: admit the request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settled {
    pub method: Method,
    /// Consumed once in the replay store.
    pub replay_key: String,
    pub payment_hash: Option<String>,
    pub network: String,
    pub asset: &'static str,
    /// In the asset's smallest unit (msat for Lightning), decimal.
    pub amount: String,
    pub rail: &'static str,
    /// The protocol's own receipt headers for the paid answer.
    pub headers: Vec<(String, String)>,
}

/// A credential that did not settle. Nothing was consumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub method: Method,
    /// The x402 `errorReason` vocabulary.
    pub reason: String,
    /// The `Payment` scheme problem type URI.
    pub problem: String,
    /// Headers the protocol sends with a refusal.
    pub headers: Vec<(String, String)>,
}

/// The Lightning settlement every Lightning encoding shares: one
/// facilitator, one replay store, one receiver.
pub trait LightningSettle: Send + Sync {
    /// The x402 terms for this request, invoice, and amount: what any
    /// Lightning credential is checked against.
    fn requirements(
        &self,
        request_hash: &str,
        invoice: &str,
        amount_msat: u64,
    ) -> PaymentRequirements;
    /// Verify the preimage against the terms and consume the payment hash.
    fn settle(
        &self,
        requirements: &PaymentRequirements,
        payload: &PaymentPayload,
        purchase: &str,
        now: u64,
    ) -> Result<Admission, SettlementResponse>;
    /// The x402 network identifier.
    fn network(&self) -> &'static str;
}

/// What an adapter settles with.
pub struct Settle<'a> {
    pub request: &'a Bound<'a>,
    pub quote: Quote,
    pub purchase: &'a str,
    pub now: u64,
    pub lightning: &'a dyn LightningSettle,
}

/// One payment method. See the module docs for how a new one plugs in.
pub trait Adapter: Send + Sync {
    fn method(&self) -> Method;
    /// How a buyer uses it.
    fn info(&self, network: &'static str) -> MethodInfo;
    /// This adapter's credential on `request`, if it carries one.
    fn credential<'r>(&self, request: &'r Bound<'_>) -> Option<&'r str>;
    /// Add this method's challenge to an unpaid answer.
    fn challenge(&self, offer: &Offer<'_>, out: &mut Challenged);
    /// Verify `credential` and consume its replay key once.
    fn settle(&self, ctx: &Settle<'_>, credential: &str) -> Result<Settled, Refused>;
}

// ---------------------------------------------------------------- x402

/// x402 v2 `exact` on `lnbtc`: `PAYMENT-REQUIRED`, `PAYMENT-SIGNATURE`,
/// `PAYMENT-RESPONSE`.
pub struct X402Lightning;

fn x402_refused(network: &str, reason: &str) -> Refused {
    let mut headers = Vec::new();
    if let Ok(value) = encode_header(&SettlementResponse::failed(network, reason)) {
        headers.push((PAYMENT_RESPONSE.to_owned(), value));
    }
    Refused {
        method: Method::X402,
        reason: reason.to_owned(),
        problem: Problem::from_reason(reason).type_uri(),
        headers,
    }
}

impl Adapter for X402Lightning {
    fn method(&self) -> Method {
        Method::X402
    }

    fn info(&self, network: &'static str) -> MethodInfo {
        MethodInfo {
            id: "x402",
            name: "x402 on Lightning",
            protocol: "x402",
            rail: "lightning",
            network: network.to_owned(),
            asset: "BTC",
            challenge: "PAYMENT-REQUIRED",
            credential: "PAYMENT-SIGNATURE",
            receipt: "PAYMENT-RESPONSE",
            spec: "https://github.com/x402-foundation/x402",
        }
    }

    fn credential<'r>(&self, request: &'r Bound<'_>) -> Option<&'r str> {
        request.header(PAYMENT_SIGNATURE)
    }

    fn challenge(&self, offer: &Offer<'_>, out: &mut Challenged) {
        let required = PaymentRequired {
            x402_version: 2,
            error: Some(
                offer
                    .refusal
                    .unwrap_or("PAYMENT-SIGNATURE header is required")
                    .to_owned(),
            ),
            resource: offer.resource.clone(),
            accepts: vec![offer.terms.clone()],
            extensions: offer.extensions.cloned(),
        };
        if let Ok(header) = encode_header(&required) {
            out.headers.push((PAYMENT_REQUIRED.to_owned(), header));
        }
    }

    fn settle(&self, ctx: &Settle<'_>, credential: &str) -> Result<Settled, Refused> {
        let network = ctx.lightning.network();
        let payload = decode_payment_payload(credential)
            .map_err(|_| x402_refused(network, "invalid_payment_payload"))?;
        let invoice = payload
            .accepted
            .extra
            .get("invoice")
            .and_then(Value::as_str)
            .filter(|invoice| !invoice.is_empty())
            .ok_or_else(|| x402_refused(network, "invalid_exact_lnbtc_invoice_missing"))?;
        let terms =
            ctx.lightning
                .requirements(ctx.request.request_hash, invoice, ctx.quote.amount_msat);
        let admitted = ctx
            .lightning
            .settle(&terms, &payload, ctx.purchase, ctx.now)
            .map_err(|settlement| {
                x402_refused(
                    network,
                    settlement
                        .error_reason
                        .as_deref()
                        .unwrap_or("settlement_failed"),
                )
            })?;
        let mut headers = Vec::new();
        if let Ok(value) = encode_header(&admitted.response) {
            headers.push((PAYMENT_RESPONSE.to_owned(), value));
        }
        Ok(settled(Method::X402, &admitted, headers))
    }
}

fn settled(method: Method, admitted: &Admission, headers: Vec<(String, String)>) -> Settled {
    Settled {
        method,
        replay_key: admitted.proof.consumption_key.clone(),
        payment_hash: Some(admitted.proof.payment_hash.clone()),
        network: admitted.proof.network.clone(),
        asset: "BTC",
        amount: admitted.proof.invoice_amount_msat.to_string(),
        rail: "lightning",
        headers,
    }
}

// ---------------------------------------------------------------- MPP

/// The HTTP `Payment` scheme with the Lightning `charge` intent
/// ([`crate::payment_scheme`]): `WWW-Authenticate: Payment`,
/// `Authorization: Payment`, `Payment-Receipt`. The challenge id is an
/// HMAC under `key`; every process that settles must share it.
pub struct MppLightning {
    pub realm: String,
    key: Vec<u8>,
}

impl MppLightning {
    /// # Errors
    ///
    /// A sentence when the key is shorter than 32 bytes or the realm is
    /// empty.
    pub fn new(realm: impl Into<String>, key: Vec<u8>) -> Result<Self, String> {
        let realm = realm.into();
        if key.len() < 32 {
            return Err("The Payment challenge key must be at least 32 bytes.".into());
        }
        if realm.is_empty() {
            return Err("The Payment challenge realm is empty.".into());
        }
        Ok(Self { realm, key })
    }

    fn refused(&self, reason: &str, problem: Problem) -> Refused {
        Refused {
            method: Method::Mpp,
            reason: reason.to_owned(),
            problem: problem.type_uri(),
            headers: Vec::new(),
        }
    }
}

impl Adapter for MppLightning {
    fn method(&self) -> Method {
        Method::Mpp
    }

    fn info(&self, network: &'static str) -> MethodInfo {
        MethodInfo {
            id: "mpp",
            name: "The Payment scheme on Lightning (MPP)",
            protocol: "mpp",
            rail: "lightning",
            network: network.to_owned(),
            asset: "BTC",
            challenge: "WWW-Authenticate: Payment",
            credential: "Authorization: Payment",
            receipt: "Payment-Receipt",
            spec: "https://paymentauth.org/draft-httpauth-payment-01.txt",
        }
    }

    fn credential<'r>(&self, request: &'r Bound<'_>) -> Option<&'r str> {
        request
            .header(payment_scheme::AUTHORIZATION)
            .filter(|value| payment_scheme::is_payment_authorization(value))
    }

    fn challenge(&self, offer: &Offer<'_>, out: &mut Challenged) {
        // The charge intent is denominated in whole sats.
        if offer.quote.amount_msat % 1000 != 0 {
            return;
        }
        let challenge = Challenge::issue(
            &self.key,
            &Terms {
                realm: &self.realm,
                amount_sats: offer.quote.amount_msat / 1000,
                invoice: &offer.invoice.bolt11,
                payment_hash: &offer.invoice.payment_hash,
                network: offer.network,
                http_method: offer.request.http_method,
                url: offer.request.url,
                body: offer.request.body,
                expires_at: offer.invoice.expires_at,
                description: offer.resource.description.as_deref(),
            },
        );
        out.body.insert("challengeId".into(), json!(challenge.id));
        out.headers
            .push((WWW_AUTHENTICATE.to_owned(), challenge.header_value()));
    }

    fn settle(&self, ctx: &Settle<'_>, credential: &str) -> Result<Settled, Refused> {
        let credential = payment_scheme::parse_credential(credential)
            .map_err(|problem| self.refused("malformed_credential", problem))?;
        let charge = payment_scheme::verify_binding(
            &self.key,
            &credential,
            ctx.request.http_method,
            ctx.request.url,
            ctx.request.body,
            ctx.now,
        )
        .map_err(|problem| {
            let reason = match problem {
                Problem::DigestMismatch => "digest_mismatch",
                Problem::Expired => "challenge_expired",
                _ => "unknown_challenge",
            };
            self.refused(reason, problem)
        })?;
        if charge.amount_sats.checked_mul(1000) != Some(ctx.quote.amount_msat) {
            return Err(self.refused(
                "invalid_exact_lnbtc_amount_mismatch",
                Problem::VerificationFailed,
            ));
        }
        let terms = ctx.lightning.requirements(
            ctx.request.request_hash,
            &charge.invoice,
            ctx.quote.amount_msat,
        );
        let mut proof = Map::new();
        proof.insert(
            "preimage".into(),
            json!(credential.preimage().unwrap_or_default()),
        );
        let payload = PaymentPayload {
            x402_version: 2,
            resource: None,
            accepted: terms.clone(),
            payload: proof,
            extensions: None,
        };
        let admitted = ctx
            .lightning
            .settle(&terms, &payload, ctx.purchase, ctx.now)
            .map_err(|settlement| {
                let reason = settlement
                    .error_reason
                    .unwrap_or_else(|| "settlement_failed".into());
                let problem = Problem::from_reason(&reason);
                self.refused(&reason, problem)
            })?;
        let receipt = payment_scheme::receipt(
            &credential.challenge.id,
            &admitted.proof.payment_hash,
            ctx.now,
        );
        Ok(settled(
            Method::Mpp,
            &admitted,
            vec![(PAYMENT_RECEIPT.to_owned(), receipt)],
        ))
    }
}

// ---------------------------------------------------------------- the router

/// The Lightning side: the receiver that issues invoices and the
/// facilitator whose replay store every Lightning encoding shares.
pub struct Lightning<S: ReplayStore> {
    receiver: Arc<dyn Receiver>,
    facilitator: Facilitator<S>,
    network: &'static str,
    timeout_secs: u32,
}

impl<S: ReplayStore> Lightning<S> {
    #[must_use]
    pub fn new(
        receiver: Arc<dyn Receiver>,
        facilitator: Facilitator<S>,
        network: &'static str,
        timeout_secs: u32,
    ) -> Self {
        Self {
            receiver,
            facilitator,
            network,
            timeout_secs,
        }
    }

    pub fn store(&self) -> &S {
        self.facilitator.store()
    }
}

impl<S: ReplayStore + Send + Sync> LightningSettle for Lightning<S> {
    fn requirements(
        &self,
        request_hash: &str,
        invoice: &str,
        amount_msat: u64,
    ) -> PaymentRequirements {
        let mut extra = Map::new();
        extra.insert("assetTransferMethod".into(), json!("bolt11"));
        extra.insert("paymentFlow".into(), json!("upfront"));
        extra.insert("requestHash".into(), json!(request_hash));
        extra.insert("requestBindingProfile".into(), json!("http:1"));
        extra.insert("requestBindingParams".into(), json!({"headers": []}));
        extra.insert("invoice".into(), json!(invoice));
        PaymentRequirements {
            scheme: "exact".into(),
            network: self.network.into(),
            amount: amount_msat.to_string(),
            asset: "BTC".into(),
            pay_to: self.receiver.pay_to(),
            max_timeout_seconds: u64::from(self.timeout_secs),
            extra,
        }
    }

    fn settle(
        &self,
        requirements: &PaymentRequirements,
        payload: &PaymentPayload,
        purchase: &str,
        now: u64,
    ) -> Result<Admission, SettlementResponse> {
        self.facilitator
            .settle(requirements, payload, purchase, now)
    }

    fn network(&self) -> &'static str {
        self.network
    }
}

/// Why no challenge could be issued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChallengeError {
    /// The request hash is not 64 hex characters.
    Binding,
    /// The receiver could not issue an invoice.
    Invoice,
}

/// The router: the Lightning settlement and the live adapters, in the
/// order their challenges are sent.
pub struct Router<S: ReplayStore> {
    lightning: Lightning<S>,
    adapters: Vec<Box<dyn Adapter>>,
}

impl<S: ReplayStore + Send + Sync> Router<S> {
    /// A router over `adapters`, each method at most once.
    ///
    /// # Errors
    ///
    /// A sentence when there is no adapter or one method appears twice.
    pub fn new(lightning: Lightning<S>, adapters: Vec<Box<dyn Adapter>>) -> Result<Self, String> {
        if adapters.is_empty() {
            return Err("A payment router needs at least one method.".into());
        }
        for (index, adapter) in adapters.iter().enumerate() {
            if adapters[..index]
                .iter()
                .any(|seen| seen.method() == adapter.method())
            {
                return Err(format!(
                    "Payment method {} is configured twice.",
                    adapter.method().id()
                ));
            }
        }
        Ok(Self {
            lightning,
            adapters,
        })
    }

    /// The live methods, in challenge order: what discovery advertises.
    #[must_use]
    pub fn methods(&self) -> Vec<MethodInfo> {
        self.adapters
            .iter()
            .map(|adapter| adapter.info(self.lightning.network))
            .collect()
    }

    #[must_use]
    pub fn network(&self) -> &'static str {
        self.lightning.network
    }

    pub fn store(&self) -> &S {
        self.lightning.store()
    }

    /// Whether `request` carries a credential for a live method.
    #[must_use]
    pub fn presents(&self, request: &Bound<'_>) -> bool {
        self.adapters
            .iter()
            .any(|adapter| adapter.credential(request).is_some())
    }

    /// The unpaid answer's challenges: one invoice for `quote`, every live
    /// method's encoding of it, and the body fields that list them.
    ///
    /// # Errors
    ///
    /// [`ChallengeError`] when the request can't be bound or no invoice
    /// can be issued.
    pub fn challenge(
        &self,
        request: &Bound<'_>,
        quote: Quote,
        resource: &ResourceInfo,
        extensions: Option<&Map<String, Value>>,
        refusal: Option<&str>,
        now: u64,
    ) -> Result<Challenged, ChallengeError> {
        let mut digest = [0u8; 32];
        hex::decode_to_slice(request.request_hash, &mut digest)
            .map_err(|_| ChallengeError::Binding)?;
        let bolt11 = self
            .lightning
            .receiver
            .invoice(quote.amount_msat, digest, self.lightning.timeout_secs)
            .map_err(|_| ChallengeError::Invoice)?;
        let decoded = decode_invoice(&bolt11).map_err(|_| ChallengeError::Invoice)?;
        let invoice = Invoice {
            payment_hash: hex::encode(decoded.payment_hash()),
            expires_at: decoded
                .created_at()
                .saturating_add(decoded.expiry_seconds())
                .min(now + u64::from(self.lightning.timeout_secs)),
            bolt11,
        };
        let terms =
            self.lightning
                .requirements(request.request_hash, &invoice.bolt11, quote.amount_msat);
        let mut out = Challenged {
            payment_hash: invoice.payment_hash.clone(),
            ..Challenged::default()
        };
        let offer = Offer {
            request,
            quote,
            invoice: &invoice,
            terms: &terms,
            network: self.lightning.network,
            resource,
            extensions,
            refusal,
            now,
        };
        for adapter in &self.adapters {
            adapter.challenge(&offer, &mut out);
        }
        out.body.insert("methods".into(), json!(self.methods()));
        Ok(out)
    }

    /// Settle the credential `request` carries, if any: `None` when it
    /// carries none for a live method. The first adapter whose credential
    /// is present settles it.
    pub fn settle(
        &self,
        request: &Bound<'_>,
        quote: Quote,
        purchase: &str,
        now: u64,
    ) -> Option<Result<Settled, Refused>> {
        let (adapter, credential) = self.adapters.iter().find_map(|adapter| {
            adapter
                .credential(request)
                .map(|credential| (adapter, credential))
        })?;
        let ctx = Settle {
            request,
            quote,
            purchase,
            now,
            lightning: &self.lightning,
        };
        Some(adapter.settle(&ctx, credential))
    }

    /// Give a consumed replay key back: only for a payment whose request
    /// got no answer at all.
    pub fn release(&self, replay_key: &str) {
        let _ = self.lightning.store().release(replay_key);
    }
}

/// The x402 Bazaar extension for one JSON `POST` resource: how a
/// facilitator lists it (`extensions.bazaar` in `PAYMENT-REQUIRED`).
#[must_use]
pub fn bazaar(example_body: &Value, output_example: &Value) -> Map<String, Value> {
    let mut extensions = Map::new();
    extensions.insert(
        "bazaar".into(),
        json!({
            "info": {
                "input": {"type": "http", "method": "POST", "bodyType": "json", "body": example_body},
                "output": {"type": "json", "example": output_example}
            },
            "schema": {
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "properties": {
                    "input": {"type": "object", "required": ["type", "method"]},
                    "output": {"type": "object"}
                },
                "required": ["input"]
            }
        }),
    );
    extensions
}

/// The duplicate-settlement reason, re-exported for callers matching it.
pub const DUPLICATE: &str = facilitator::DUPLICATE_SETTLEMENT;
