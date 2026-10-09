//! Pay per request over Lightning (#11078, #11136; OpenAgents API D3,
//! D12): `POST /v1/responses` and `POST /v1/chat/completions` with no key,
//! through the payment router (`openagents_x402::router`).
//!
//! - **The methods.** x402 `exact`/`lnbtc` always (the `inference.x402`
//!   block turns the toll on), and the HTTP `Payment` scheme's Lightning
//!   `charge` (MPP) when `inference.x402.mpp` is set. Both are encodings of
//!   one invoice per `402` and consume the same replay key, so one payment
//!   settles once. Discovery (`/v1/openapi.json`, and through it the
//!   website's catalogs, `llms.txt`, `auth.md`, and the For agents page)
//!   prints [`Toll::methods`], never a static list.
//!
//! - **The price.** The request's worst case from the rate card: for every
//!   model its name can reach (the model itself, a task class's models, or
//!   every class's for `openagents/auto`, plus `openagents.fallbacks`),
//!   the dearest provider row plus margin, at the most input tokens its
//!   bytes can hold and its `max_output_tokens` (or the model's own
//!   ceiling). Dollars become sats at `inference.sats_rate`, rounded up to
//!   a whole sat. The same request bytes always get the same price, so the
//!   paid retry matches its challenge.
//! - **The challenge.** No credential: `402 payment_required` with x402 v2
//!   `exact`/`lnbtc` terms in `PAYMENT-REQUIRED` (with the Bazaar
//!   extension) and, with MPP on, `WWW-Authenticate: Payment` on the same
//!   invoice; the body lists the live methods. The invoice
//!   comes from the resident wallet and its description hash binds the
//!   method, URL, and body bytes (`http:1`, no headers), so a proof buys
//!   only this request.
//! - **The paid call.** The same request with `PAYMENT-SIGNATURE` (the
//!   accepted terms and the preimage) or `Authorization: Payment` (the
//!   echoed challenge and the preimage): the router's adapter checks the
//!   proof and `crates/x402`'s facilitator consumes the payment hash once
//!   in the replay store, then the request runs like any other. The answer
//!   carries `PAYMENT-RESPONSE` or `Payment-Receipt`, and
//!   `x-openagents-receipt` names the one `openagents.payment-receipt.v1`
//!   record written when it is served (no preimage in it).
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

use std::path::{Path, PathBuf};
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
use nostr::x402::{binding_hash, http_binding};
use openagents_x402::payment_scheme;
use openagents_x402::receipt::{self, FileReceiptStore, Payer, PaymentReceipt};
use openagents_x402::router::{
    Adapter, Bound, ChallengeError, Lightning, Method, MethodInfo, MppLightning, Quote, Refused,
    Router, Settled, X402Lightning,
};
use openagents_x402::server::Receiver;
use openagents_x402::{Facilitator, FileReplayStore, PAYMENT_SIGNATURE, ResourceInfo};
use serde_json::{Map, Value, json};

use crate::config::InferenceX402;
use crate::serve::ServeState;

/// The tenant name paid-per-request calls are metered under.
pub const TENANT: &str = "x402";

/// Where agents read how to pay.
pub const DOCS: &str = "https://openagents.com/docs/api/for-agents";

/// The pay-per-request toll: the payment router (its receiver, replay
/// store, and live methods), the receipt store, and the bound origin.
pub struct Toll {
    router: Router<FileReplayStore>,
    receipts: FileReceiptStore,
    origin: Option<String>,
}

impl Toll {
    /// The toll for `config`, its invoices from `receiver` (the resident
    /// wallet in production, a test signer in tests).
    ///
    /// # Errors
    ///
    /// A sentence when the network is not `bitcoin` or `testnet`, a store
    /// cannot be opened, or the `Payment` challenge key cannot be read or
    /// made.
    pub fn open(
        config: &InferenceX402,
        registry: &Path,
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
        let receipts_dir = config
            .receipts_dir
            .clone()
            .unwrap_or_else(|| registry.join("inference").join("payment-receipts"));
        let receipts = FileReceiptStore::open(&receipts_dir).map_err(|error| error.to_string())?;
        let origin = config
            .origin
            .clone()
            .or_else(|| public_origin.map(str::to_owned))
            .map(|origin| origin.trim_end_matches('/').to_owned());
        let mut adapters: Vec<Box<dyn Adapter>> = vec![Box::new(X402Lightning)];
        if let Some(mpp) = &config.mpp {
            let realm = mpp
                .realm
                .clone()
                .or_else(|| {
                    origin
                        .as_deref()
                        .and_then(|origin| origin.split("://").nth(1))
                        .map(|host| host.split('/').next().unwrap_or(host).to_owned())
                })
                .unwrap_or_else(|| "api.openagents.com".to_owned());
            let key_file = mpp
                .challenge_key_file
                .clone()
                .unwrap_or_else(|| registry.join("inference").join("payment-challenge.key"));
            adapters.push(Box::new(MppLightning::new(
                realm,
                challenge_key(&key_file)?,
            )?));
        }
        let router = Router::new(
            Lightning::new(
                receiver,
                Facilitator::new(store, nostr::x402::DEFAULT_CLOCK_SKEW),
                network,
                config.timeout_secs,
            ),
            adapters,
        )?;
        Ok(Self {
            router,
            receipts,
            origin,
        })
    }

    /// The live payment methods, in the order the `402` sends them.
    #[must_use]
    pub fn methods(&self) -> Vec<MethodInfo> {
        self.router.methods()
    }

    /// The receipts written so far.
    #[must_use]
    pub fn receipts(&self) -> &FileReceiptStore {
        &self.receipts
    }

    fn has(&self, method: Method) -> bool {
        self.router
            .methods()
            .iter()
            .any(|info| info.id == method.id())
    }
}

/// The `Payment` challenge HMAC key: 32 random bytes, hex, in a file
/// only this user can read. Made on first use.
fn challenge_key(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Write;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            // SAFETY: geteuid has no preconditions.
            let uid = unsafe { libc::geteuid() };
            if !metadata.is_file()
                || metadata.permissions().mode() & 0o077 != 0
                || metadata.uid() != uid
            {
                return Err(format!(
                    "The Payment challenge key {} must be a file only this user can read.",
                    path.display()
                ));
            }
            let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
            let key = hex::decode(text.trim())
                .map_err(|_| format!("The Payment challenge key {} isn't hex.", path.display()))?;
            if key.len() < 32 {
                return Err("The Payment challenge key must be at least 32 bytes.".into());
            }
            Ok(key)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            use ring::rand::SecureRandom;
            let mut key = [0u8; 32];
            ring::rand::SystemRandom::new()
                .fill(&mut key)
                .map_err(|_| "No randomness for the Payment challenge key.".to_owned())?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .map_err(|error| error.to_string())?;
            file.write_all(hex::encode(key).as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|error| error.to_string())?;
            Ok(key.to_vec())
        }
        Err(error) => Err(error.to_string()),
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

/// Should this request go the pay-per-request way: a toll configured, and
/// either no `Authorization` at all or a `Payment` credential while that
/// method is live. A bearer key always takes the keyed path.
pub(crate) fn applies(state: &ServeState, headers: &HeaderMap) -> bool {
    let Some(toll) = &state.inference_x402 else {
        return false;
    };
    match headers.get("authorization") {
        None => true,
        Some(value) => {
            value
                .to_str()
                .is_ok_and(payment_scheme::is_payment_authorization)
                && toll.has(Method::Mpp)
        }
    }
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

fn header_pair(name: &str, value: &str) -> Option<(axum::http::HeaderName, HeaderValue)> {
    Some((
        axum::http::HeaderName::from_bytes(name.as_bytes()).ok()?,
        HeaderValue::from_str(value).ok()?,
    ))
}

/// A `402` with `fields` merged into the JSON body and `headers` added.
fn payment_required(
    message: impl Into<String>,
    fields: &Map<String, Value>,
    headers: &[(String, String)],
    request_id: &str,
) -> Response {
    let mut body = json!({
        "error": {"type": "payment_required", "code": "payment_required",
                  "message": message.into(), "param": null},
        "x402Version": 2,
        "docs": DOCS,
    });
    if let Some(body) = body.as_object_mut() {
        for (name, value) in fields {
            body.insert(name.clone(), value.clone());
        }
    }
    let mut response = (StatusCode::PAYMENT_REQUIRED, axum::Json(body)).into_response();
    let out = response.headers_mut();
    out.insert("cache-control", HeaderValue::from_static("no-store"));
    if let Ok(value) = HeaderValue::from_str(request_id) {
        out.insert("x-request-id", value);
    }
    for (name, value) in headers {
        if let Some((name, value)) = header_pair(name, value) {
            // Several challenges may share a name (`WWW-Authenticate`).
            out.append(name, value);
        }
    }
    response
}

/// A paid request, ready to run: its caller (with the admission that
/// gives the payment back if nothing is answered), the protocol's receipt
/// headers, and the receipt id its answer carries.
pub(crate) struct Paid {
    pub caller: Caller,
    headers: Vec<(String, String)>,
    receipt_id: String,
    run: Arc<Run>,
}

impl Paid {
    /// The request failed before any answer: give the payment back so the
    /// same proof can be sent again.
    pub(crate) fn refund(&self) {
        self.run.give_back();
    }

    /// Put the protocol's receipt (`PAYMENT-RESPONSE` or
    /// `Payment-Receipt`) and `x-openagents-receipt` on the answer.
    pub(crate) fn stamp(&self, response: &mut Response) {
        let out = response.headers_mut();
        for (name, value) in &self.headers {
            if let Some((name, value)) = header_pair(name, value) {
                out.insert(name, value);
            }
        }
        if let Ok(value) = HeaderValue::from_str(&self.receipt_id) {
            out.insert(receipt::HEADER, value);
        }
    }
}

/// The Bazaar listing for `api`: an example request and answer.
fn bazaar(api: Api, model: Option<&str>) -> Map<String, Value> {
    let model = model.unwrap_or("openagents/chat");
    let (input, output) = if api == Api::Chat {
        (
            json!({"model": model, "messages": [{"role": "user", "content": "Say hello."}], "max_tokens": 200}),
            json!({"object": "chat.completion", "choices": [{"message": {"role": "assistant", "content": "Hello."}}]}),
        )
    } else {
        (
            json!({"model": model, "input": "Say hello.", "max_output_tokens": 200}),
            json!({"object": "response", "status": "completed", "output_text": "Hello."}),
        )
    };
    openagents_x402::router::bazaar(&input, &output)
}

/// The pay-per-request side of a keyless request to `path` with these
/// exact `body` bytes (`request` is what they parse to, translated to Open
/// Responses for Chat Completions): a `402` challenge, a refusal, or the
/// paid caller.
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
    let quote = Quote {
        amount_msat,
        usd_micros: micros,
    };
    // Only the credential headers reach the router.
    let credentials: Vec<(String, String)> = [PAYMENT_SIGNATURE, payment_scheme::AUTHORIZATION]
        .into_iter()
        .filter_map(|name| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(|value| (name.to_owned(), value.to_owned()))
        })
        .collect();
    let bound = Bound {
        http_method: "POST",
        url: &url,
        body,
        request_hash: &request_hash,
        headers: &credentials,
    };
    let now = openagents_x402::unix_now();
    let extensions = bazaar(api, request.model.as_deref());
    let methods = toll.methods();
    let mpp = toll.has(Method::Mpp);
    let send_with = methods
        .iter()
        .map(|method| method.credential)
        .collect::<Vec<_>>()
        .join(" or ");

    // A fresh 402: every live method's challenge on one new invoice.
    let challenge = |refused: Option<&Refused>| -> Response {
        let challenged = match toll.router.challenge(
            &bound,
            quote,
            &resource,
            Some(&extensions),
            refused.map(|refused| refused.reason.as_str()),
            now,
        ) {
            Ok(challenged) => challenged,
            Err(ChallengeError::Binding) => {
                return refuse(ApiError::new(
                    ErrorType::ServerError,
                    "The request couldn't be bound.",
                ));
            }
            Err(ChallengeError::Invoice) => {
                return refuse(ApiError::new(
                    ErrorType::ServerError,
                    "We can't make an invoice right now. Try again in a minute, or use an API key.",
                ));
            }
        };
        let message = match refused {
            None => format!(
                "This request costs up to {sats} sats (${}). Pay the invoice in this answer and send the same request again with {send_with}, or use an API key. How: {DOCS}",
                micros_usd(micros)
            ),
            Some(refused) => format!(
                "The payment wasn't accepted ({}). This request costs up to {sats} sats (${}); pay the fresh invoice in this answer and send the same request again with {send_with}.",
                refused.reason,
                micros_usd(micros)
            ),
        };
        let mut fields = challenged.body;
        fields.insert("price_sats".into(), json!(sats));
        fields.insert("price_msat".into(), json!(amount_msat.to_string()));
        fields.insert("price_usd".into(), json!(micros_usd(micros)));
        if mpp {
            // The `Payment` scheme's problem details (RFC 9457).
            let problem = refused.map_or_else(
                || payment_scheme::Problem::PaymentRequired.type_uri(),
                |refused| refused.problem.clone(),
            );
            fields.insert("type".into(), json!(problem));
            fields.insert("title".into(), json!("Payment Required"));
            fields.insert("status".into(), json!(402));
            fields.insert("detail".into(), json!(message));
        }
        let mut headers = challenged.headers;
        if let Some(refused) = refused {
            fields.insert("reason".into(), json!(refused.reason));
            headers.extend(refused.headers.iter().cloned());
        }
        payment_required(message, &fields, &headers, request_id)
    };

    let purchase = format!("inference:{request_id}");
    let settled = match toll.router.settle(&bound, quote, &purchase, now) {
        None => return Err(challenge(None)),
        Some(Err(refused)) => return Err(challenge(Some(&refused))),
        Some(Ok(settled)) => settled,
    };
    let receipt_id = receipt::receipt_id(&settled.replay_key);
    let receipt = PaymentReceipt {
        v: receipt::SCHEMA.to_owned(),
        id: receipt_id.clone(),
        protocol: settled.method.id().to_owned(),
        rail: settled.rail.to_owned(),
        network: settled.network.clone(),
        asset: settled.asset.to_owned(),
        amount: settled.amount.clone(),
        usd_micros: micros,
        replay_key: settled.replay_key.clone(),
        request_hash: request_hash.clone(),
        resource: format!("POST {path}"),
        payer: Payer::default(),
        settled_at: now,
        outcome: "served".to_owned(),
    };
    let Settled {
        method,
        replay_key,
        payment_hash,
        headers: receipt_headers,
        ..
    } = settled;
    let run = Arc::new(Run {
        state: state.clone(),
        key: replay_key,
        method,
        payment_hash: payment_hash.unwrap_or_default(),
        paid_micros: micros,
        models: reachable(gateway, request),
        request_id: request_id.to_owned(),
        receipt: Arc::new(receipt),
        admitted: AtomicBool::new(false),
        given_back: Arc::new(AtomicBool::new(false)),
    });
    Ok(Paid {
        caller: Caller {
            request_id: request_id.to_owned(),
            tenant: Some(TENANT.to_owned()),
            key_id: None,
            api,
            traffic: inference::meter::Traffic {
                audience: inference::meter::Audience::Outside,
                payment: inference::meter::Payment::Paid,
                synthetic: false,
            },
            limits: PriceLimit::default(),
            admission: Some(Admission(run.clone())),
            own: inference::run::OwnUpstreams::default(),
        },
        headers: receipt_headers,
        receipt_id,
        run,
    })
}

#[derive(Clone, Copy)]
enum Stage {
    Paid,
    Returned,
    Settled,
}

/// What a charge is recorded as, per method and stage.
fn settlement(method: Method, stage: Stage) -> &'static str {
    match (method, stage) {
        (Method::Mpp, Stage::Paid) => "mpp_paid",
        (Method::Mpp, Stage::Returned) => "mpp_returned",
        (Method::Mpp, Stage::Settled) => "mpp_settled",
        (_, Stage::Paid) => "x402_paid",
        (_, Stage::Returned) => "x402_returned",
        (_, Stage::Settled) => "x402_settled",
    }
}

/// One paid request: admitted once, and its payment given back if it is
/// never answered.
struct Run {
    state: Arc<ServeState>,
    key: String,
    method: Method,
    payment_hash: String,
    paid_micros: u64,
    models: Vec<String>,
    request_id: String,
    /// Written once, when the answer is served.
    receipt: Arc<PaymentReceipt>,
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
        toll.router.release(key);
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
            self.record(None, settlement(self.method, Stage::Paid))
                .await;
            Ok(Box::new(Answering {
                run: Arc::new(RunRef {
                    state: self.state.clone(),
                    key: self.key.clone(),
                    method: self.method,
                    request_id: self.request_id.clone(),
                    paid_micros: self.paid_micros,
                    payment_hash: self.payment_hash.clone(),
                    receipt: self.receipt.clone(),
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
    method: Method,
    request_id: String,
    paid_micros: u64,
    payment_hash: String,
    receipt: Arc<PaymentReceipt>,
    given_back: Arc<AtomicBool>,
}

struct Answering {
    run: Arc<RunRef>,
}

impl Admitted for Answering {
    fn payment(&self) -> inference::meter::Payment {
        inference::meter::Payment::Paid
    }

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
                        settlement: settlement(self.run.method, Stage::Returned),
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
                    write_receipt(&run);
                    if let Some(book) = &run.state.inference_book {
                        book.lock().await.record_charge(
                            &run.request_id,
                            crate::inference_public::Charge {
                                tenant: TENANT.to_owned(),
                                free: false,
                                micros: Some(run.paid_micros),
                                settlement: settlement(run.method, Stage::Settled),
                            },
                        );
                    }
                }
                event
            }
        }))
    }
}

/// The one receipt for this payment, once it served an answer.
fn write_receipt(run: &RunRef) {
    let Some(toll) = &run.state.inference_x402 else {
        return;
    };
    if let Err(error) = toll.receipts.write(&run.receipt) {
        eprintln!(
            "{}",
            json!({"event": "payment_receipt_unwritten", "request_id": run.request_id,
                   "receipt": run.receipt.id, "error": error.to_string()})
        );
    }
}

/// One line for the operator: what was paid, what the answer cost. The
/// payment hash is public; the preimage never appears.
fn tracing_paid(run: &RunRef, cost: Option<u64>) {
    eprintln!(
        "{}",
        json!({"event": "inference_paid_settled", "method": run.method.id(),
               "request_id": run.request_id, "receipt": run.receipt.id,
               "payment_hash": run.payment_hash, "paid_usd": micros_usd(run.paid_micros),
               "cost_usd": cost.map(micros_usd)})
    );
}

/// Run a paid request's outcome: stamp the receipts on an answer, give
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
