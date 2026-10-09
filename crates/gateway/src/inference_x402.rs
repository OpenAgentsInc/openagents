//! Pay per request with x402 over Lightning (#11078; OpenAgents API D3,
//! D12): `POST /v1/responses` and `POST /v1/chat/completions` with no key.
//!
//! - **The price.** The request's worst case from the rate card: for every
//!   model its name can reach (the model itself, a task class's models, or
//!   every class's for `openagents/auto`, plus `openagents.fallbacks`),
//!   the dearest provider row plus margin, at the most input tokens its
//!   bytes can hold and its `max_output_tokens` (or the model's own
//!   ceiling). Dollars become sats at `inference.sats_rate`, rounded up to
//!   a whole sat. The same request bytes always get the same price, so the
//!   paid retry matches its challenge.
//! - **The challenge.** No `PAYMENT-SIGNATURE`: `402 payment_required`
//!   with x402 v2 `exact`/`lnbtc` terms in `PAYMENT-REQUIRED`. The invoice
//!   comes from the resident wallet and its description hash binds the
//!   method, URL, and body bytes (`http:1`, no headers), so a proof buys
//!   only this request.
//! - **The paid call.** The same request with `PAYMENT-SIGNATURE` (the
//!   accepted terms and the preimage): `crates/x402`'s facilitator checks
//!   the proof and consumes the payment hash once in the replay store,
//!   then the request runs like any other. The answer carries
//!   `PAYMENT-RESPONSE`.
//! - **Settling.** The payment is the quoted worst case, paid up front.
//!   The answer's actual cost is in `x-openagents-cost-usd` (and its
//!   `openagents:cost` event); the unspent part is not returned per call.
//!   A request that gets no answer at all (every provider failed before
//!   its first token, or no route) gives the payment back to the replay
//!   store, so the same `PAYMENT-SIGNATURE` can be sent again. A key with
//!   a balance pays the actual cost instead.
//!
//! One payment buys one model turn: hosted tools, `store`, and
//! `previous_response_id` need a key.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use inference::error::{ApiError, ErrorType};
use inference::meter::Api;
use inference::request::{CreateResponse, Tool};
use inference::router::{PriceLimit, TaskClass, micros_usd};
use inference::run::{Admission, Admit, Admitted, Caller, Events, Gateway, Prepared};
use inference::upstream::BoxFuture;
use nostr::x402::{PaymentRequirements, binding_hash, http_binding};
use openagents_x402::server::Receiver;
use openagents_x402::wire::{decode_payment_payload, encode_header};
use openagents_x402::{
    Facilitator, FileReplayStore, PAYMENT_REQUIRED, PAYMENT_RESPONSE, PAYMENT_SIGNATURE,
    PaymentRequired, ReplayStore, ResourceInfo, SettlementResponse,
};
use serde_json::{Map, Value, json};

use crate::config::InferenceX402;
use crate::serve::ServeState;

/// The tenant name x402 calls are metered under.
pub const TENANT: &str = "x402";

/// The pay-per-request toll: the receiver that issues invoices, the
/// facilitator and its replay store, and the network.
pub struct Toll {
    receiver: Arc<dyn Receiver>,
    facilitator: Facilitator<FileReplayStore>,
    network: &'static str,
    origin: Option<String>,
    timeout_secs: u32,
}

impl Toll {
    /// The toll for `config`, its invoices from `receiver` (the resident
    /// wallet in production, a test signer in tests).
    ///
    /// # Errors
    ///
    /// A sentence when the network is not `bitcoin` or `testnet` or the
    /// replay store cannot be opened.
    pub fn open(
        config: &InferenceX402,
        registry: &std::path::Path,
        public_origin: Option<&str>,
        receiver: Arc<dyn Receiver>,
    ) -> Result<Self, String> {
        let network = openagents_x402::network_id(&config.network)
            .ok_or("inference.x402.network must be bitcoin or testnet.")?;
        let dir = config
            .replay_dir
            .clone()
            .unwrap_or_else(|| registry.join("inference").join("x402-replay"));
        let store = FileReplayStore::open(&dir).map_err(|error| error.to_string())?;
        Ok(Self {
            receiver,
            facilitator: Facilitator::new(store, nostr::x402::DEFAULT_CLOCK_SKEW),
            network,
            origin: config
                .origin
                .clone()
                .or_else(|| public_origin.map(str::to_owned))
                .map(|origin| origin.trim_end_matches('/').to_owned()),
            timeout_secs: config.timeout_secs,
        })
    }
}

/// The resident wallet as the receiver, reached on every call the way
/// Lightning funding reaches it, so a restarted wallet is picked up.
pub struct WalletReceiver {
    pub wallet_home: PathBuf,
    pub receiver_node: String,
}

impl WalletReceiver {
    fn wallet(&self) -> Result<openagents_wallet::resident::RemoteWallet, String> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = std::fs::symlink_metadata(&self.wallet_home)
            .map_err(|_| "The wallet is not reachable.".to_owned())?;
        // SAFETY: geteuid has no preconditions.
        let uid = unsafe { libc::geteuid() };
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 || metadata.uid() != uid
        {
            return Err("The wallet's home is not private to this user.".into());
        }
        let wallet = openagents_wallet::resident::RemoteWallet::probe(&self.wallet_home)
            .ok_or("The wallet is not running.")?;
        Ok(wallet)
    }
}

impl Receiver for WalletReceiver {
    fn pay_to(&self) -> String {
        self.receiver_node.clone()
    }

    fn invoice(
        &self,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<String, String> {
        use openagents_wallet::LightningWallet;
        self.wallet()?
            .receive_exact_from_node(&self.receiver_node, amount_msat, request_hash, expiry_secs)
            .map(|issued| issued.bolt11)
            .map_err(|error| error.to_string())
    }
}

/// Should this request go the x402 way: no key, and a toll configured.
pub(crate) fn applies(state: &ServeState, headers: &HeaderMap) -> bool {
    state.inference_x402.is_some() && !headers.contains_key("authorization")
}

/// The models a request's name can reach: what the price covers.
pub(crate) fn reachable(gateway: &Gateway, request: &CreateResponse) -> Vec<String> {
    let mut names = Vec::new();
    let mut add = |name: &str| {
        if !names.iter().any(|seen: &String| seen == name) {
            names.push(name.to_owned());
        }
    };
    let mut named = vec![request.model.clone().unwrap_or_default()];
    if let Some(options) = &request.openagents {
        named.extend(options.fallbacks.iter().cloned());
    }
    for name in named {
        let classes: Vec<TaskClass> = if name == "openagents/auto" {
            TaskClass::ALL.to_vec()
        } else {
            TaskClass::from_model_id(&name).into_iter().collect()
        };
        if classes.is_empty() {
            add(&name);
            continue;
        }
        for class in classes {
            if let Some(entry) = gateway.classes().classes.get(&class) {
                for model in &entry.models {
                    add(&model.model);
                }
            }
        }
    }
    names
}

/// A request's worst case in micros of a dollar: the dearest reachable
/// model, `None` when none of them has a price.
pub(crate) fn worst_micros(gateway: &Gateway, request: &CreateResponse) -> Option<u64> {
    let offerings = gateway.offerings();
    reachable(gateway, request)
        .iter()
        .filter_map(|model| {
            let upstreams: Vec<&str> = offerings
                .iter()
                .filter(|offering| offering.model == *model)
                .map(|offering| offering.upstream.as_str())
                .collect();
            let priced = crate::inference_public::priced(gateway, request, model, &upstreams)?;
            priced.price.quote(&priced.maximum_usage).ok()
        })
        .max()
}

/// Micros of a dollar in millisatoshis, rounded up to a whole sat, at
/// least one sat.
#[must_use]
pub fn msat_of(micros: u64, usd_per_btc: u64) -> u64 {
    // 1 USD = 1e11 / usd_per_btc msat, so micros * 1e5 / usd_per_btc.
    let per_btc = u128::from(usd_per_btc.max(1));
    let msat = (u128::from(micros) * 100_000).div_ceil(per_btc);
    let sats = msat.div_ceil(1_000).max(1);
    u64::try_from(sats * 1_000).unwrap_or(u64::MAX)
}

fn api_error(error: &ApiError, request_id: &str) -> Response {
    crate::inference_routes::error(error, request_id)
}

fn requirements(
    toll: &Toll,
    amount_msat: u64,
    request_hash: &str,
    invoice: &str,
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
        network: toll.network.into(),
        amount: amount_msat.to_string(),
        asset: "BTC".into(),
        pay_to: toll.receiver.pay_to(),
        max_timeout_seconds: u64::from(toll.timeout_secs),
        extra,
    }
}

fn payment_required(message: impl Into<String>, extra: Value, request_id: &str) -> Response {
    let mut body = json!({
        "error": {"type": "payment_required", "code": "payment_required",
                  "message": message.into(), "param": null},
        "x402Version": 2,
    });
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        for (name, value) in extra {
            body.insert(name.clone(), value.clone());
        }
    }
    let mut response = (StatusCode::PAYMENT_REQUIRED, axum::Json(body)).into_response();
    let headers = response.headers_mut();
    headers.insert("cache-control", HeaderValue::from_static("no-store"));
    if let Ok(value) = HeaderValue::from_str(request_id) {
        headers.insert("x-request-id", value);
    }
    response
}

/// A paid request, ready to run: its caller (with the admission that
/// gives the payment back if nothing is answered) and the settlement
/// header its answer carries.
pub(crate) struct Paid {
    pub caller: Caller,
    pub settlement: String,
    run: Arc<Run>,
}

impl Paid {
    /// The request failed before any answer: give the payment back so the
    /// same proof can be sent again.
    pub(crate) fn refund(&self) {
        self.run.give_back();
    }

    /// Put `PAYMENT-RESPONSE` on the answer.
    pub(crate) fn stamp(&self, response: &mut Response) {
        if let Ok(value) = HeaderValue::from_str(&self.settlement) {
            response.headers_mut().insert(PAYMENT_RESPONSE, value);
        }
    }
}

/// The x402 side of a keyless request to `path` with these exact `body`
/// bytes (`request` is what they parse to, translated to Open Responses
/// for Chat Completions): a `402` challenge, a refusal, or the paid
/// caller.
pub(crate) fn admit(
    state: &Arc<ServeState>,
    headers: &HeaderMap,
    path: &str,
    body: &[u8],
    request: &CreateResponse,
    api: Api,
    request_id: &str,
) -> Result<Paid, Response> {
    let refuse = |error: ApiError| api_error(&error, request_id);
    let (Some(toll), Some(gateway)) = (&state.inference_x402, &state.inference) else {
        return Err(refuse(ApiError::new(
            ErrorType::Unauthorized,
            "Send your API key in the `Authorization: Bearer` header.",
        )));
    };
    let Some(usd_per_btc) = state
        .config
        .inference
        .as_ref()
        .and_then(|config| config.sats_rate.as_ref())
        .map(|rate| rate.usd_per_btc)
    else {
        return Err(refuse(ApiError::new(
            ErrorType::ServerError,
            "Paying per request isn't priced here yet. Use an API key.",
        )));
    };
    if request.store == Some(true) || request.previous_response_id.is_some() {
        return Err(refuse(ApiError::invalid_request(
            if request.store == Some(true) {
                "store"
            } else {
                "previous_response_id"
            },
            "Stored responses need an API key; a paid request without one keeps nothing.",
        )));
    }
    if request
        .tools
        .as_ref()
        .is_some_and(|tools| tools.iter().any(|tool| !matches!(tool, Tool::Function(_))))
    {
        return Err(refuse(ApiError::invalid_request(
            "tools",
            "Hosted tools need an API key; a paid request covers one model turn.",
        )));
    }
    let Some(micros) = worst_micros(gateway, request) else {
        return Err(refuse(ApiError::new(
            ErrorType::NoRoute,
            format!(
                "{} has no price yet, so it can't be paid for per request.",
                request.model.as_deref().unwrap_or("This model")
            ),
        )));
    };
    let amount_msat = msat_of(micros, usd_per_btc);
    let origin = toll.origin.clone().or_else(|| {
        headers
            .get("host")
            .and_then(|host| host.to_str().ok())
            .map(|host| format!("http://{host}"))
    });
    let Some(origin) = origin else {
        return Err(refuse(ApiError::invalid_request(
            "host",
            "Send a Host header.",
        )));
    };
    let url = format!("{origin}{path}");
    let Ok(request_hash) =
        http_binding("POST", &url, body, &[]).and_then(|binding| binding_hash(&binding))
    else {
        return Err(refuse(ApiError::invalid_request(
            "body",
            "This request can't be bound to a payment.",
        )));
    };
    let stream = request.stream == Some(true);
    let resource = ResourceInfo {
        url: url.clone(),
        description: Some(format!(
            "One {} request to {}",
            if api == Api::Chat {
                "Chat Completions"
            } else {
                "Open Responses"
            },
            request.model.as_deref().unwrap_or("a model")
        )),
        mime_type: Some(if stream {
            "text/event-stream".into()
        } else {
            "application/json".into()
        }),
        rest: Map::new(),
    };
    let sats = amount_msat / 1_000;
    let price = json!({"price_sats": sats, "price_msat": amount_msat.to_string(),
                       "price_usd": micros_usd(micros)});

    let Some(signature) = headers
        .get(PAYMENT_SIGNATURE)
        .and_then(|value| value.to_str().ok())
    else {
        let mut digest = [0u8; 32];
        if hex_into(&request_hash, &mut digest).is_err() {
            return Err(refuse(ApiError::new(
                ErrorType::ServerError,
                "The request couldn't be bound.",
            )));
        }
        let invoice = match toll
            .receiver
            .invoice(amount_msat, digest, toll.timeout_secs)
        {
            Ok(invoice) => invoice,
            Err(_) => {
                return Err(refuse(ApiError::new(
                    ErrorType::ServerError,
                    "We can't make an invoice right now. Try again in a minute, or use an API key.",
                )));
            }
        };
        let required = PaymentRequired {
            x402_version: 2,
            error: Some("PAYMENT-SIGNATURE header is required".into()),
            resource,
            accepts: vec![requirements(toll, amount_msat, &request_hash, &invoice)],
            extensions: None,
        };
        let Ok(header) = encode_header(&required) else {
            return Err(refuse(ApiError::new(
                ErrorType::ServerError,
                "The payment terms couldn't be written.",
            )));
        };
        let mut response = payment_required(
            format!(
                "This request costs up to {sats} sats (${}). Pay the invoice in the PAYMENT-REQUIRED header and send the same request again with PAYMENT-SIGNATURE, or use an API key.",
                micros_usd(micros)
            ),
            price,
            request_id,
        );
        if let Ok(value) = HeaderValue::from_str(&header) {
            response.headers_mut().insert(PAYMENT_REQUIRED, value);
        }
        return Err(response);
    };

    let refused = |reason: &str, settlement: &SettlementResponse| {
        let mut response = payment_required(
            format!(
                "The payment wasn't accepted ({reason}). Send the request without PAYMENT-SIGNATURE for fresh terms."
            ),
            json!({"reason": reason}),
            request_id,
        );
        if let Ok(header) = encode_header(settlement)
            && let Ok(value) = HeaderValue::from_str(&header)
        {
            response.headers_mut().insert(PAYMENT_RESPONSE, value);
        }
        response
    };
    let Ok(payload) = decode_payment_payload(signature) else {
        let reason = "invalid_payment_payload";
        return Err(refused(
            reason,
            &SettlementResponse::failed(toll.network, reason),
        ));
    };
    let Some(invoice) = payload
        .accepted
        .extra
        .get("invoice")
        .and_then(Value::as_str)
        .filter(|invoice| !invoice.is_empty())
    else {
        let reason = "invalid_exact_lnbtc_invoice_missing";
        return Err(refused(
            reason,
            &SettlementResponse::failed(toll.network, reason),
        ));
    };
    let terms = requirements(toll, amount_msat, &request_hash, invoice);
    let admitted = match toll.facilitator.settle(
        &terms,
        &payload,
        &format!("inference:{request_id}"),
        openagents_x402::unix_now(),
    ) {
        Ok(admitted) => admitted,
        Err(settlement) => {
            let reason = settlement
                .error_reason
                .clone()
                .unwrap_or_else(|| "settlement_failed".into());
            return Err(refused(&reason, &settlement));
        }
    };
    let settlement = encode_header(&admitted.response).unwrap_or_default();
    let run = Arc::new(Run {
        state: state.clone(),
        key: admitted.proof.consumption_key.clone(),
        payment_hash: admitted.proof.payment_hash.clone(),
        paid_micros: micros,
        models: reachable(gateway, request),
        request_id: request_id.to_owned(),
        admitted: AtomicBool::new(false),
        given_back: Arc::new(AtomicBool::new(false)),
    });
    Ok(Paid {
        caller: Caller {
            request_id: request_id.to_owned(),
            tenant: Some(TENANT.to_owned()),
            key_id: None,
            api,
            limits: PriceLimit::default(),
            admission: Some(Admission(run.clone())),
            own: inference::run::OwnUpstreams::default(),
        },
        settlement,
        run,
    })
}

fn hex_into(text: &str, out: &mut [u8; 32]) -> Result<(), ()> {
    if text.len() != 64 {
        return Err(());
    }
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).map_err(|_| ())?;
    }
    Ok(())
}

/// One paid request: admitted once, and its payment given back if it is
/// never answered.
struct Run {
    state: Arc<ServeState>,
    key: String,
    payment_hash: String,
    paid_micros: u64,
    models: Vec<String>,
    request_id: String,
    admitted: AtomicBool,
    /// Set once the payment is given back, by whichever side does it
    /// first, so a later release can't free a claim a retry made since.
    given_back: Arc<AtomicBool>,
}

/// Give the payment back once.
fn release_once(state: &ServeState, key: &str, given_back: &AtomicBool) {
    if given_back.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(toll) = &state.inference_x402 {
        let _ = toll.facilitator.store().release(key);
    }
}

impl Run {
    fn give_back(&self) {
        release_once(&self.state, &self.key, &self.given_back);
    }

    async fn record(&self, micros: Option<u64>, settlement: &'static str) {
        if let Some(book) = &self.state.inference_book {
            book.lock().await.record_charge(
                &self.request_id,
                crate::inference_public::Charge {
                    tenant: TENANT.to_owned(),
                    free: false,
                    micros,
                    settlement,
                },
            );
        }
    }
}

impl Admit for Run {
    fn check<'a>(
        &'a self,
        _request: &'a CreateResponse,
        _caller: &'a Caller,
    ) -> BoxFuture<'a, Result<PriceLimit, ApiError>> {
        Box::pin(async { Ok(PriceLimit::default()) })
    }

    fn admit<'a>(
        &'a self,
        _request: &'a CreateResponse,
        prepared: &'a Prepared,
        _caller: &'a Caller,
    ) -> BoxFuture<'a, Result<Box<dyn Admitted>, ApiError>> {
        Box::pin(async move {
            if self.admitted.swap(true, Ordering::SeqCst) {
                return Err(ApiError::new(
                    ErrorType::LimitReached,
                    "A paid request covers one model turn. Use an API key for more.",
                ));
            }
            if let Some(outside) = prepared
                .attempts()
                .iter()
                .find(|attempt| !self.models.contains(&attempt.model))
            {
                return Err(ApiError::new(
                    ErrorType::ServerError,
                    format!("{} wasn't in this request's price.", outside.model),
                ));
            }
            self.record(None, "x402_paid").await;
            Ok(Box::new(Answering {
                run: Arc::new(RunRef {
                    state: self.state.clone(),
                    key: self.key.clone(),
                    request_id: self.request_id.clone(),
                    paid_micros: self.paid_micros,
                    payment_hash: self.payment_hash.clone(),
                    given_back: self.given_back.clone(),
                }),
            }) as Box<dyn Admitted>)
        })
    }
}

/// What an admitted paid run needs after the admission is gone.
struct RunRef {
    state: Arc<ServeState>,
    key: String,
    request_id: String,
    paid_micros: u64,
    payment_hash: String,
    given_back: Arc<AtomicBool>,
}

struct Answering {
    run: Arc<RunRef>,
}

impl Admitted for Answering {
    fn abandon(self: Box<Self>) -> BoxFuture<'static, ()> {
        Box::pin(async move {
            release_once(&self.run.state, &self.run.key, &self.run.given_back);
            if let Some(book) = &self.run.state.inference_book {
                book.lock().await.record_charge(
                    &self.run.request_id,
                    crate::inference_public::Charge {
                        tenant: TENANT.to_owned(),
                        free: false,
                        micros: Some(0),
                        settlement: "x402_returned",
                    },
                );
            }
        })
    }

    fn settle_on_end(self: Box<Self>, events: Events) -> Events {
        use futures_util::StreamExt;
        let run = self.run;
        Box::pin(events.then(move |event| {
            let run = run.clone();
            async move {
                if event.body.is_terminal()
                    && let Some(response) = event.body.response()
                {
                    let cost = response
                        .openagents
                        .as_ref()
                        .and_then(|info| info.cost.as_ref())
                        .and_then(|cost| inference::router::usd_micros(&cost.price_usd));
                    tracing_paid(&run, cost);
                    if let Some(book) = &run.state.inference_book {
                        book.lock().await.record_charge(
                            &run.request_id,
                            crate::inference_public::Charge {
                                tenant: TENANT.to_owned(),
                                free: false,
                                micros: Some(run.paid_micros),
                                settlement: "x402_settled",
                            },
                        );
                    }
                }
                event
            }
        }))
    }
}

/// One line for the operator: what was paid, what the answer cost.
fn tracing_paid(run: &RunRef, cost: Option<u64>) {
    eprintln!(
        "{}",
        json!({"event": "inference_x402_settled", "request_id": run.request_id,
               "payment_hash": run.payment_hash, "paid_usd": micros_usd(run.paid_micros),
               "cost_usd": cost.map(micros_usd)})
    );
}

/// Run a paid request's outcome: stamp the settlement on an answer, give
/// the payment back on a refusal that came before any answer.
pub(crate) fn finish(paid: &Paid, outcome: Result<Response, Response>) -> Response {
    match outcome {
        Ok(mut response) => {
            paid.stamp(&mut response);
            response
        }
        Err(refusal) => {
            paid.refund();
            refusal
        }
    }
}
