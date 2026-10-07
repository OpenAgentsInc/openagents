//! The multi-route pay front: one listener, one [`Receiver`] (the wallet
//! that is `payTo`), one replay store, many priced routes, and a settlement
//! hook.
//!
//! Every `402` carries one invoice in two encodings: x402 v2 `exact`/`lnbtc`
//! terms in `PAYMENT-REQUIRED`, and, when the price is whole sats, an HTTP
//! `Payment` challenge (`method="lightning"`, `intent="charge"`) in
//! `WWW-Authenticate` ([`crate::payment_scheme`]). Either proof settles
//! through the same facilitator, so one payment hash is consumed once across
//! both schemes and every route.
//!
//! The order after a valid proof is fixed: consume the replay key, ask the
//! receiver what actually arrived, call [`SettlementSink::on_settled`], and
//! only then run the route's executor. A sink that refuses gets the key
//! released and a `503`, and the executor does not run, so the same proof can
//! be presented again once the ledger is back; nothing was sold twice and
//! nothing was sold unrecorded. The funded execution adapter retains the
//! claim for exact recovery instead of releasing it.

use std::sync::Arc;

use nostr::x402::{PaymentRequirements, binding_hash, decode_invoice, http_binding};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::facilitator::{Admission, Facilitator};
use crate::payment_scheme::{
    self, AUTHORIZATION, Challenge, PAYMENT_RECEIPT, Problem, Terms, WWW_AUTHENTICATE,
};
use crate::replay::ReplayStore;
use crate::server::{Receiver, Request, Response};
use crate::wire::{
    PAYMENT_REQUIRED, PAYMENT_RESPONSE, PAYMENT_SIGNATURE, PaymentPayload, PaymentRequired,
    ResourceInfo, SettlementResponse, decode_payment_payload, encode_header,
};

/// What a route's executor is handed once the call is paid and recorded.
pub struct Call<'a> {
    pub route: &'a str,
    /// `{name}` segments of the route's path, in order.
    pub params: &'a [(String, String)],
    pub request: &'a Request,
    /// The settled payment hash; `None` only while pricing the call, or
    /// for a call the caller's own provider keys pay for.
    pub payment_hash: Option<&'a str>,
    /// The caller's own provider keys (`OpenAgents-Provider-Key`, BYOK),
    /// for a route whose price is its model cost alone: the executor runs
    /// every model call on them and never on ours.
    pub provider_keys: Option<&'a model_access::Keys>,
    /// The quote a [`Price::Quote`] route priced this call at: the plugin
    /// release the payment bought, which the executor must run.
    pub quote: Option<&'a Quote>,
}

impl Call<'_> {
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// The 200 body and, when the executor knows it, its media type.
pub struct Output {
    pub body: Vec<u8>,
    pub content_type: Option<String>,
}

/// Runs a paid route: a command, a plugin invocation, an upstream service.
pub trait RouteExecutor: Send + Sync {
    fn execute(&self, call: &Call<'_>) -> Result<Output, String>;
}

impl<F: Fn(&Call<'_>) -> Result<Output, String> + Send + Sync> RouteExecutor for F {
    fn execute(&self, call: &Call<'_>) -> Result<Output, String> {
        self(call)
    }
}

/// A route's price in msat: fixed, computed from the request before the
/// challenge (it must give the same answer when the paid retry arrives),
/// or a [`Quote`] that also names what the payment buys.
#[derive(Clone)]
pub enum Price {
    Fixed(u64),
    Of(Arc<dyn Fn(&Call<'_>) -> Result<u64, String> + Send + Sync>),
    Quote(Arc<dyn Fn(&Call<'_>) -> Result<Quote, Unpriced> + Send + Sync>),
}

/// One named part of a quoted price, such as `endpoint` and `author_fee`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricePart {
    pub name: String,
    pub msat: u64,
}

/// A per-call price and what it buys: for a plugin invocation, the pinned
/// release and the author whose fee is part of the price. The settlement
/// carries the plugin, release, author, and fee, so the ledger can split
/// the fee to the author.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    pub price_msat: u64,
    /// The parts that sum to `price_msat`; the `402` names each one.
    pub parts: Vec<PricePart>,
    pub plugin: Option<String>,
    /// An immutable release. Calls must carry the approved quote digest in
    /// their JSON body before the front offers an invoice for this release.
    pub release: Option<String>,
    /// The author party the fee is owed to.
    pub author: Option<String>,
    pub fee_msat: Option<u64>,
    /// The resource the ledger and the flow view name for this call, when
    /// it is narrower than the route's (`x:{name}` for a hosted resource).
    pub resource: Option<String>,
}

/// Why a [`Price::Quote`] route could not price a call: the status and
/// typed error the caller gets instead of a `402`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unpriced {
    pub status: u16,
    pub kind: String,
    pub message: String,
}

impl Price {
    pub(crate) fn quote(&self, call: &Call<'_>) -> Result<Quote, Unpriced> {
        let unpriced = |message: String| Unpriced {
            status: 400,
            kind: "unpriced_request".into(),
            message,
        };
        match self {
            Self::Fixed(msat) => Ok(Quote {
                price_msat: *msat,
                ..Quote::default()
            }),
            Self::Of(price) => price(call).map_err(unpriced).map(|msat| Quote {
                price_msat: msat,
                ..Quote::default()
            }),
            Self::Quote(quote) => {
                let quote = quote(call)?;
                let parts: Option<u64> = quote
                    .parts
                    .iter()
                    .try_fold(0u64, |sum, part| sum.checked_add(part.msat));
                if !quote.parts.is_empty() && parts != Some(quote.price_msat) {
                    return Err(unpriced("the price parts do not sum to the price".into()));
                }
                Ok(quote)
            }
        }
    }
}

/// One priced route.
#[derive(Clone)]
pub struct Route {
    /// Unique within the front; it names the route in settlements and logs.
    pub id: String,
    pub method: String,
    /// `/v1/plugins/{id}/invoke`: literal segments, and `{name}` segments
    /// that match any one non-empty segment. The query is not matched.
    pub path: String,
    pub price: Price,
    pub executor: Arc<dyn RouteExecutor>,
    /// The split role the ledger applies (`endpoint`, `plugin_call`,
    /// `hosted_resource`, ...).
    pub role: String,
    /// The resource id the ledger and the flow view name.
    pub resource: String,
    /// The plugin a `plugin_call` route sells, when there is one.
    pub plugin: Option<String>,
    pub description: String,
    pub mime_type: String,
    /// The price is the call's model cost alone, so a caller that brings
    /// its own provider keys (`OpenAgents-Provider-Key`) gets no `402`:
    /// the executor runs on their keys (BYOK, #10176).
    pub model_cost_only: bool,
}

fn match_path(pattern: &str, path: &str) -> Option<Vec<(String, String)>> {
    let mut want = pattern.split('/');
    let mut got = path.split('/');
    let mut params = Vec::new();
    loop {
        match (want.next(), got.next()) {
            (None, None) => return Some(params),
            (Some(w), Some(g)) => {
                if let Some(name) = w.strip_prefix('{').and_then(|w| w.strip_suffix('}')) {
                    if g.is_empty() {
                        return None;
                    }
                    params.push((name.to_string(), g.to_string()));
                } else if w != g {
                    return None;
                }
            }
            _ => return None,
        }
    }
}

/// Which encoding the proof came in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scheme {
    X402,
    Payment,
}

/// One settled payment, handed to the ledger before the purchase runs.
/// Never carries a preimage or an invoice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settlement {
    pub payment_hash: String,
    pub request_hash: String,
    pub route: String,
    pub resource: String,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    /// The plugin release the payment bought, for a quoted plugin call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
    /// The author party owed `fee_msat` of this payment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_msat: Option<u64>,
    pub price_msat: u64,
    /// What the wallet received, which an LSP fee can make less than the
    /// price; the invoice amount when the wallet could not say.
    pub received_msat: u64,
    /// Whether `received_msat` came from the wallet's own record.
    pub received_from_wallet: bool,
    pub scheme: Scheme,
    /// The x402 network identifier.
    pub network: String,
    pub settled_at: u64,
}

/// Where settlements go. The ledger (`crates/pay-ledger`) implements it;
/// it must be idempotent per `payment_hash`. An `Err` refuses the call
/// before it runs.
pub trait SettlementSink: Send + Sync {
    fn on_settled(&self, settlement: &Settlement) -> Result<(), String>;

    /// One request reached a route, paid or not (the flow stream's `call`
    /// record). Best effort: it never refuses a call.
    fn on_call(&self, _usage: &Usage) {}
}

/// A `call` usage record: one request that reached a route and method.
/// Carries no payer, request hash, payment hash, or body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Usage {
    pub route: String,
    pub resource: String,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
    /// `executed`, `challenged`, `refused`, `caller_paid`, ...
    pub outcome: String,
    pub status: u16,
    /// Whether a settlement was written for this call.
    pub paid: bool,
    /// The quoted price; absent when the call was never priced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_msat: Option<u64>,
    pub at: u64,
}

/// What one request did, for the operator's log. Never carries a preimage
/// or an invoice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Event {
    pub method: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<Scheme>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_msat: Option<u64>,
    pub status: u16,
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_reason: Option<String>,
    /// `theirs` when the caller's own provider keys paid; absent is a sale.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payer: Option<String>,
    /// The caller's first key's provider and fingerprint, never the key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payer_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payer_fingerprint: Option<String>,
}

/// The front's fixed configuration.
#[derive(Clone)]
pub struct Config {
    /// The public origin the routes are served under, such as
    /// `https://api.openagents.com`; a route's URL is this plus the request
    /// target, and that URL is what both schemes bind.
    pub base_url: String,
    /// The x402 network identifier of the receiver's invoices.
    pub network: &'static str,
    /// The `Payment` challenge realm.
    pub realm: String,
    /// The `Payment` challenge-id HMAC key. Every process that settles for
    /// this receiver must share it, as it shares the replay store.
    pub challenge_key: Vec<u8>,
    /// Invoice expiry and x402 `maxTimeoutSeconds`.
    pub timeout_secs: u32,
}

pub struct Front<S: ReplayStore> {
    config: Config,
    receiver: Arc<dyn Receiver>,
    facilitator: Facilitator<S>,
    sink: Arc<dyn SettlementSink>,
    routes: Vec<Route>,
    funded_purchase: Option<String>,
}

fn field_token(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

impl<S: ReplayStore> Front<S> {
    /// A front over `routes`. Refuses duplicate ids, two routes with the
    /// same method and path, a zero fixed price, and malformed paths.
    pub fn new(
        mut config: Config,
        receiver: Arc<dyn Receiver>,
        facilitator: Facilitator<S>,
        sink: Arc<dyn SettlementSink>,
        routes: Vec<Route>,
    ) -> Result<Self, String> {
        config.base_url = config.base_url.trim_end_matches('/').to_string();
        if http_binding("GET", &format!("{}/", config.base_url), b"", &[]).is_err() {
            return Err(format!(
                "base URL {} must be an absolute http(s) URL without a fragment",
                config.base_url
            ));
        }
        if config.challenge_key.len() < 32 {
            return Err("the Payment challenge key must be at least 32 bytes".into());
        }
        if config.timeout_secs == 0 {
            return Err("the invoice expiry must be positive".into());
        }
        if routes.is_empty() {
            return Err("at least one route is required".into());
        }
        for (index, route) in routes.iter().enumerate() {
            if route.id.is_empty() {
                return Err(format!("route {} has no id", index + 1));
            }
            if !field_token(&route.method) || route.method != route.method.to_ascii_uppercase() {
                return Err(format!(
                    "route {}: method must be an upper-case HTTP method",
                    route.id
                ));
            }
            if !route.path.starts_with('/') || route.path.contains(['?', '#']) {
                return Err(format!(
                    "route {}: path must start with / and have no query",
                    route.id
                ));
            }
            if matches!(route.price, Price::Fixed(0)) {
                return Err(format!("route {}: price must be positive", route.id));
            }
            for other in &routes[..index] {
                if other.id == route.id {
                    return Err(format!("route id {} is used twice", route.id));
                }
                if other.method == route.method && other.path == route.path {
                    return Err(format!(
                        "routes {} and {} serve the same method and path",
                        other.id, route.id
                    ));
                }
            }
        }
        Ok(Self {
            config,
            receiver,
            facilitator,
            sink,
            routes,
            funded_purchase: None,
        })
    }

    pub(crate) fn with_funded_purchase(mut self, purchase: String) -> Self {
        self.funded_purchase = Some(purchase);
        self
    }

    pub fn routes(&self) -> &[Route] {
        &self.routes
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn pay_to(&self) -> String {
        self.receiver.pay_to()
    }

    fn requirements(&self, request_hash: &str, invoice: &str, price: u64) -> PaymentRequirements {
        let mut extra = Map::new();
        extra.insert("assetTransferMethod".into(), json!("bolt11"));
        extra.insert("paymentFlow".into(), json!("upfront"));
        extra.insert("requestHash".into(), json!(request_hash));
        extra.insert("requestBindingProfile".into(), json!("http:1"));
        extra.insert("requestBindingParams".into(), json!({"headers": []}));
        extra.insert("invoice".into(), json!(invoice));
        PaymentRequirements {
            scheme: "exact".into(),
            network: self.config.network.into(),
            amount: price.to_string(),
            asset: "BTC".into(),
            pay_to: self.receiver.pay_to(),
            max_timeout_seconds: u64::from(self.config.timeout_secs),
            extra,
        }
    }

    /// Answer one request at time `now`.
    pub fn handle(&self, request: &Request, now: u64) -> (Response, Event) {
        let mut usage: Option<Usage> = None;
        let (response, event) = self.answer(request, now, &mut usage);
        if let Some(mut usage) = usage {
            usage.outcome = event.outcome.clone();
            usage.status = event.status;
            usage.paid = event.payment_hash.is_some() && event.outcome != "unrecorded";
            usage.price_msat = event.price_msat;
            self.sink.on_call(&usage);
        }
        (response, event)
    }

    fn answer(&self, request: &Request, now: u64, usage: &mut Option<Usage>) -> (Response, Event) {
        let mut event = Event {
            method: request.method.clone(),
            target: request.target.clone(),
            route: None,
            scheme: None,
            request_hash: None,
            price_msat: None,
            status: 0,
            outcome: String::new(),
            payment_hash: None,
            error_reason: None,
            payer: None,
            payer_provider: None,
            payer_fingerprint: None,
        };
        let path = request.target.split('?').next().unwrap_or_default();
        let matched: Vec<(&Route, Vec<(String, String)>)> = self
            .routes
            .iter()
            .filter_map(|route| match_path(&route.path, path).map(|params| (route, params)))
            .collect();
        if matched.is_empty() {
            return done(
                event,
                Response::json(404, &json!({"error": {"type": "not_found"}})),
                "not_found",
            );
        }
        let Some((route, params)) = matched
            .iter()
            .find(|(route, _)| route.method == request.method)
        else {
            let allow: Vec<&str> = matched.iter().map(|(r, _)| r.method.as_str()).collect();
            let mut response =
                Response::json(405, &json!({"error": {"type": "method_not_allowed"}}));
            response.headers.push(("allow".into(), allow.join(", ")));
            return done(event, response, "method_not_allowed");
        };
        event.route = Some(route.id.clone());
        *usage = Some(Usage {
            route: route.id.clone(),
            resource: route.resource.clone(),
            role: route.role.clone(),
            plugin: route.plugin.clone(),
            release: None,
            outcome: String::new(),
            status: 0,
            paid: false,
            price_msat: None,
            at: now,
        });
        let url = format!("{}{}", self.config.base_url, request.target);
        let Some(request_hash) = http_binding(&request.method, &url, &request.body, &[])
            .ok()
            .and_then(|binding| binding_hash(&binding).ok())
        else {
            return done(
                event,
                Response::json(400, &json!({"error": {"type": "unbound_request"}})),
                "unbound",
            );
        };
        event.request_hash = Some(request_hash.clone());
        // BYOK: the caller's own provider keys pay a model-cost-only route,
        // with no 402. The header is read here and never logged or echoed.
        let provided: Vec<&str> = request
            .headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case(model_access::PROVIDER_KEY_HEADER))
            .map(|(_, value)| value.as_str())
            .collect();
        if !provided.is_empty() && route.model_cost_only {
            let keys = match model_access::Keys::from_header_values(provided) {
                Ok(keys) => keys,
                Err(why) => {
                    return done(
                        event,
                        Response::json(
                            400,
                            &json!({"error": {"type": "provider_key_malformed", "message": why}}),
                        ),
                        "provider_key_malformed",
                    );
                }
            };
            if let Some((provider, key)) = keys.iter().next() {
                event.payer = Some("theirs".into());
                event.payer_provider = Some(provider.word().into());
                event.payer_fingerprint = Some(key.fingerprint());
            }
            let call = Call {
                route: &route.id,
                params,
                request,
                payment_hash: None,
                provider_keys: Some(&keys),
                quote: None,
            };
            return match route.executor.execute(&call) {
                Ok(output) => {
                    let content_type = output
                        .content_type
                        .unwrap_or_else(|| route.mime_type.clone());
                    let response = Response {
                        status: 200,
                        headers: vec![
                            ("content-type".into(), content_type),
                            ("openagents-payer".into(), "theirs".into()),
                        ],
                        body: output.body,
                    };
                    done(event, response, "caller_paid")
                }
                Err(message) => {
                    // The caller's own keys could not make the call; it is
                    // never moved to ours and never billed.
                    event.error_reason = Some(message.clone());
                    done(
                        event,
                        Response::json(
                            502,
                            &json!({"error": {"type": "model_call_failed", "message": message}}),
                        ),
                        "caller_paid_failed",
                    )
                }
            };
        }
        let call = Call {
            route: &route.id,
            params,
            request,
            payment_hash: None,
            provider_keys: None,
            quote: None,
        };
        let quote = match route.price.quote(&call) {
            Ok(quote) if quote.price_msat > 0 => quote,
            Ok(_) => {
                return done(
                    event,
                    Response::json(400, &json!({"error": {"type": "unpriced_request"}})),
                    "unpriced",
                );
            }
            Err(unpriced) => {
                event.error_reason = Some(unpriced.kind.clone());
                return done(
                    event,
                    Response::json(
                        unpriced.status,
                        &json!({"error": {"type": unpriced.kind, "message": unpriced.message}}),
                    ),
                    "unpriced",
                );
            }
        };
        if quote.release.is_some() {
            let approved = serde_json::from_slice::<Value>(&request.body)
                .ok()
                .and_then(|body| body["quote_digest"].as_str().map(str::to_owned));
            let digest = crate::execution::quote_digest(&quote);
            if approved.as_deref() != Some(digest.as_str()) {
                return done(
                    event,
                    Response::json(
                        409,
                        &json!({
                            "error":{"type":"quote_conflict","message":"Approve this exact quote and include its quote_digest in the request body before payment."},
                            "quote":quote,"quote_digest":digest,
                        }),
                    ),
                    "unpriced",
                );
            }
        }
        if let Some(usage) = usage.as_mut() {
            if quote.plugin.is_some() {
                usage.plugin.clone_from(&quote.plugin);
            }
            usage.release.clone_from(&quote.release);
            if let Some(resource) = &quote.resource {
                usage.resource.clone_from(resource);
            }
        }
        let price = quote.price_msat;
        event.price_msat = Some(price);
        let paid = Paid {
            route,
            params,
            request,
            url: &url,
            request_hash: &request_hash,
            price,
            quote: &quote,
            funded_purchase: self.funded_purchase.as_deref(),
            now,
        };

        if let Some(signature) = request.header(PAYMENT_SIGNATURE) {
            event.scheme = Some(Scheme::X402);
            return self.x402(&paid, signature, event);
        }
        if let Some(credential) = request
            .header(AUTHORIZATION)
            .filter(|value| payment_scheme::is_payment_authorization(value))
        {
            event.scheme = Some(Scheme::Payment);
            return self.payment(&paid, credential, event);
        }
        let response = self.challenge(&paid, None);
        let outcome = if response.status == 402 {
            "challenged"
        } else {
            "issuance_denied"
        };
        done(event, response, outcome)
    }

    /// A fresh `402` with both encodings of one new invoice. `refusal` says
    /// why a presented proof was refused, in both vocabularies.
    fn challenge(&self, paid: &Paid<'_>, refusal: Option<(&str, Problem)>) -> Response {
        let mut digest = [0u8; 32];
        if hex::decode_to_slice(paid.request_hash, &mut digest).is_err() {
            return Response::json(500, &json!({"error": {"type": "binding_digest"}}));
        }
        let invoice = match self
            .receiver
            .invoice(paid.price, digest, self.config.timeout_secs)
        {
            Ok(invoice) => invoice,
            Err(_) => {
                return Response::json(
                    503,
                    &json!({"error": {"type": "exact_lnbtc_invoice_issuance_denied"}}),
                );
            }
        };
        let required = PaymentRequired {
            x402_version: 2,
            error: Some(
                refusal
                    .map(|(reason, _)| reason.to_string())
                    .unwrap_or_else(|| "PAYMENT-SIGNATURE header is required".into()),
            ),
            resource: ResourceInfo {
                url: paid.url.to_string(),
                description: Some(paid.route.description.clone()),
                mime_type: Some(paid.route.mime_type.clone()),
                rest: Map::new(),
            },
            accepts: vec![self.requirements(paid.request_hash, &invoice, paid.price)],
            extensions: None,
        };
        let Ok(required) = encode_header(&required) else {
            return Response::json(500, &json!({"error": {"type": "challenge_encoding"}}));
        };

        let price_sats = paid.price / 1000;
        let whole_sats = paid.price % 1000 == 0;
        let challenge = whole_sats
            .then(|| decode_invoice(&invoice).ok())
            .flatten()
            .map(|decoded| {
                let invoice_end = decoded
                    .created_at()
                    .saturating_add(decoded.expiry_seconds());
                Challenge::issue(
                    &self.config.challenge_key,
                    &Terms {
                        realm: &self.config.realm,
                        amount_sats: price_sats,
                        invoice: &invoice,
                        payment_hash: &hex::encode(decoded.payment_hash()),
                        network: self.config.network,
                        http_method: &paid.request.method,
                        url: paid.url,
                        body: &paid.request.body,
                        expires_at: invoice_end.min(paid.now + u64::from(self.config.timeout_secs)),
                        description: Some(&paid.route.description),
                    },
                )
            });

        let problem = refusal.map_or(Problem::PaymentRequired, |(_, problem)| problem);
        let amount = |msat: u64| {
            if msat % 1000 == 0 {
                let sats = msat / 1000;
                format!("{sats} {}", if sats == 1 { "sat" } else { "sats" })
            } else {
                format!("{msat} msat")
            }
        };
        let mut price_text = amount(paid.price);
        if !paid.quote.parts.is_empty() {
            let parts: Vec<String> = paid
                .quote
                .parts
                .iter()
                .map(|part| format!("{} {}", part.name.replace('_', " "), amount(part.msat)))
                .collect();
            price_text = format!("{price_text} ({})", parts.join(" + "));
        }
        let detail = match refusal {
            None => format!(
                "This call costs {price_text}. Pay the invoice and retry with PAYMENT-SIGNATURE or Authorization: Payment."
            ),
            Some((reason, _)) => format!(
                "The payment proof was refused ({reason}). This call costs {price_text}; pay the fresh invoice and retry."
            ),
        };
        let mut body = json!({
            "error": {
                "type": if refusal.is_some() { "payment_refused" } else { "payment_required" },
                "code": refusal.map_or("payment_required", |(reason, _)| reason),
                "message": detail,
            },
            "type": problem.type_uri(),
            "title": problem.title(),
            "status": 402,
            "detail": detail,
            "x402Version": 2,
            "price_msat": paid.price,
        });
        if whole_sats {
            body["price_sats"] = json!(price_sats);
        }
        if !paid.quote.parts.is_empty() {
            body["price_parts"] = json!(paid.quote.parts);
        }
        if let Some(plugin) = &paid.quote.plugin {
            body["plugin"] = json!(plugin);
        }
        if let Some(release) = &paid.quote.release {
            body["release"] = json!(release);
        }
        if let Some(challenge) = &challenge {
            body["challengeId"] = json!(challenge.id);
        }
        let mut response = Response::json(402, &body);
        response
            .headers
            .push(("cache-control".into(), "no-store".into()));
        response.headers.push((PAYMENT_REQUIRED.into(), required));
        if let Some(challenge) = challenge {
            response
                .headers
                .push((WWW_AUTHENTICATE.into(), challenge.header_value()));
        }
        if let Some((reason, _)) = refusal {
            let failed = SettlementResponse::failed(self.config.network, reason);
            if let Ok(header) = encode_header(&failed) {
                response.headers.push((PAYMENT_RESPONSE.into(), header));
            }
        }
        response
    }

    fn refuse(
        &self,
        paid: &Paid<'_>,
        reason: &str,
        problem: Problem,
        event: Event,
    ) -> (Response, Event) {
        let mut event = event;
        event.error_reason = Some(reason.to_string());
        done(
            event,
            self.challenge(paid, Some((reason, problem))),
            "refused",
        )
    }

    fn x402(&self, paid: &Paid<'_>, signature: &str, event: Event) -> (Response, Event) {
        let Ok(payload) = decode_payment_payload(signature) else {
            return self.refuse(
                paid,
                "invalid_payment_payload",
                Problem::MalformedCredential,
                event,
            );
        };
        let Some(invoice) = payload
            .accepted
            .extra
            .get("invoice")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            return self.refuse(
                paid,
                "invalid_exact_lnbtc_invoice_missing",
                Problem::VerificationFailed,
                event,
            );
        };
        let requirements = self.requirements(paid.request_hash, invoice, paid.price);
        match self
            .facilitator
            .settle(&requirements, &payload, &paid.purchase(), paid.now)
        {
            Ok(admitted) => self.admitted(paid, admitted, Scheme::X402, None, event),
            Err(settlement) => {
                let reason = settlement
                    .error_reason
                    .unwrap_or_else(|| "settlement_failed".into());
                let problem = Problem::from_reason(&reason);
                self.refuse(paid, &reason, problem, event)
            }
        }
    }

    fn payment(&self, paid: &Paid<'_>, value: &str, event: Event) -> (Response, Event) {
        let credential = match payment_scheme::parse_credential(value) {
            Ok(credential) => credential,
            Err(problem) => return self.refuse(paid, "malformed_credential", problem, event),
        };
        let charge = match payment_scheme::verify_binding(
            &self.config.challenge_key,
            &credential,
            &paid.request.method,
            paid.url,
            &paid.request.body,
            paid.now,
        ) {
            Ok(charge) => charge,
            Err(problem) => {
                let reason = match problem {
                    Problem::DigestMismatch => "digest_mismatch",
                    Problem::Expired => "challenge_expired",
                    _ => "unknown_challenge",
                };
                return self.refuse(paid, reason, problem, event);
            }
        };
        if charge.amount_sats.checked_mul(1000) != Some(paid.price) {
            return self.refuse(
                paid,
                "invalid_exact_lnbtc_amount_mismatch",
                Problem::VerificationFailed,
                event,
            );
        }
        let requirements = self.requirements(paid.request_hash, &charge.invoice, paid.price);
        let mut proof = Map::new();
        proof.insert(
            "preimage".into(),
            json!(credential.preimage().unwrap_or_default()),
        );
        let payload = PaymentPayload {
            x402_version: 2,
            resource: None,
            accepted: requirements.clone(),
            payload: proof,
            extensions: None,
        };
        match self
            .facilitator
            .settle(&requirements, &payload, &paid.purchase(), paid.now)
        {
            Ok(admitted) => self.admitted(
                paid,
                admitted,
                Scheme::Payment,
                Some(&credential.challenge.id),
                event,
            ),
            Err(settlement) => {
                let reason = settlement
                    .error_reason
                    .unwrap_or_else(|| "settlement_failed".into());
                let problem = Problem::from_reason(&reason);
                self.refuse(paid, &reason, problem, event)
            }
        }
    }

    fn admitted(
        &self,
        paid: &Paid<'_>,
        admitted: Admission,
        scheme: Scheme,
        challenge_id: Option<&str>,
        mut event: Event,
    ) -> (Response, Event) {
        let payment_hash = admitted.proof.payment_hash.clone();
        event.payment_hash = Some(payment_hash.clone());
        let mut hash = [0u8; 32];
        let looked_up = hex::decode_to_slice(&payment_hash, &mut hash)
            .ok()
            .and_then(|()| self.receiver.received_msat(hash).ok().flatten());
        let settlement = Settlement {
            payment_hash: payment_hash.clone(),
            request_hash: paid.request_hash.to_string(),
            route: paid.route.id.clone(),
            resource: paid
                .quote
                .resource
                .clone()
                .unwrap_or_else(|| paid.route.resource.clone()),
            role: paid.route.role.clone(),
            plugin: paid
                .quote
                .plugin
                .clone()
                .or_else(|| paid.route.plugin.clone()),
            release: paid.quote.release.clone(),
            author: paid.quote.author.clone(),
            fee_msat: paid.quote.fee_msat,
            price_msat: paid.price,
            received_msat: looked_up.unwrap_or(admitted.proof.invoice_amount_msat),
            received_from_wallet: looked_up.is_some(),
            scheme,
            network: admitted.proof.network.clone(),
            settled_at: paid.now,
        };
        if let Err(message) = self.sink.on_settled(&settlement) {
            // Ordinary resources release an unexecuted claim. Funded resources
            // retain it for recovery of the original settlement and task.
            let released = self
                .facilitator
                .release_unexecuted(&admitted.proof.consumption_key);
            event.error_reason = Some(match released {
                Ok(()) => message,
                Err(error) => format!("{message}; release: {error}"),
            });
            let mut response = Response::json(
                503,
                &json!({
                    "error": {
                        "type": "settlement_unrecorded",
                        "message": "The payment is valid but could not be recorded, so nothing ran. Retry the same request with the same proof.",
                    },
                    "type": "https://paymentauth.org/problems/internal-payment-error",
                    "title": "Settlement Not Recorded",
                    "status": 503,
                }),
            );
            response.headers.push(("retry-after".into(), "5".into()));
            return done(event, response, "unrecorded");
        }

        let settlement_header = encode_header(&admitted.response).unwrap_or_default();
        let call = Call {
            route: &paid.route.id,
            params: paid.params,
            request: paid.request,
            payment_hash: Some(&payment_hash),
            provider_keys: None,
            quote: Some(paid.quote),
        };
        let mut response = match paid.route.executor.execute(&call) {
            Ok(output) => {
                let content_type = output
                    .content_type
                    .unwrap_or_else(|| paid.route.mime_type.clone());
                Response {
                    status: 200,
                    headers: vec![("content-type".into(), content_type)],
                    body: output.body,
                }
            }
            Err(message) => {
                // The payment is consumed and recorded; the buyer holds a
                // settlement that bought a failed execution. Say so.
                event.error_reason = Some(message.clone());
                Response::json(
                    500,
                    &json!({"error": {"type": "execution_failed", "message": "execution failed after settlement", "detail": message}}),
                )
            }
        };
        response
            .headers
            .push((PAYMENT_RESPONSE.into(), settlement_header));
        if let Some(id) = challenge_id {
            response.headers.push((
                PAYMENT_RECEIPT.into(),
                payment_scheme::receipt(id, &payment_hash, paid.now),
            ));
        }
        let outcome = if response.status == 200 {
            "executed"
        } else {
            "execution_failed"
        };
        done(event, response, outcome)
    }
}

struct Paid<'a> {
    route: &'a Route,
    params: &'a [(String, String)],
    request: &'a Request,
    url: &'a str,
    request_hash: &'a str,
    price: u64,
    quote: &'a Quote,
    funded_purchase: Option<&'a str>,
    now: u64,
}

impl Paid<'_> {
    fn purchase(&self) -> String {
        self.funded_purchase
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{}:{}", self.route.id, self.request_hash))
    }
}

fn done(mut event: Event, response: Response, outcome: &str) -> (Response, Event) {
    event.status = response.status;
    event.outcome = outcome.to_string();
    (response, event)
}

/// Serve `front` on `listener` until `stop` is set, calling `log` for each
/// request.
pub fn serve<S: ReplayStore + Send + Sync + 'static>(
    listener: std::net::TcpListener,
    front: Arc<Front<S>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    log: impl Fn(&Event) + Send + Sync + 'static,
) -> std::io::Result<()> {
    crate::server::serve_with(listener, stop, move |request| {
        let (response, event) = front.handle(request, crate::unix_now());
        log(&event);
        response
    })
}

/// A [`SettlementSink`] that appends one JSON line per settlement to a file
/// and syncs it before the purchase runs. It is the record the ledger
/// imports until `crates/pay-ledger` is wired in as the sink.
pub struct NdjsonSettlements {
    path: std::path::PathBuf,
    lock: std::sync::Mutex<()>,
}

impl NdjsonSettlements {
    pub fn open(path: &std::path::Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        Ok(Self {
            path: path.to_path_buf(),
            lock: std::sync::Mutex::new(()),
        })
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl SettlementSink for NdjsonSettlements {
    fn on_settled(&self, settlement: &Settlement) -> Result<(), String> {
        use std::io::Write;
        let mut line = serde_json::to_vec(settlement).map_err(|e| e.to_string())?;
        line.push(b'\n');
        let _guard = self.lock.lock().map_err(|_| "settlement log lock")?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("{}: {e}", self.path.display()))?;
        file.write_all(&line)
            .and_then(|()| file.sync_data())
            .map_err(|e| format!("{}: {e}", self.path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_match_literals_and_named_segments() {
        assert_eq!(match_path("/v1/messages", "/v1/messages"), Some(vec![]));
        assert_eq!(match_path("/v1/messages", "/v1/messages/"), None);
        assert_eq!(
            match_path(
                "/v1/plugins/{id}/invoke",
                "/v1/plugins/explain-error/invoke"
            ),
            Some(vec![("id".into(), "explain-error".into())])
        );
        assert_eq!(
            match_path("/v1/plugins/{id}/invoke", "/v1/plugins//invoke"),
            None
        );
        assert_eq!(match_path("/x/{r}", "/x/a/b"), None);
    }
}
