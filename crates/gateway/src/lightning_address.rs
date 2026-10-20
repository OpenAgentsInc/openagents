//! OpenAgents account authorization in front of the private Breez LNURL service.
use crate::serve::ServeState;
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{MethodRouter, get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};
use tenancy::{
    Accounts,
    accounts::lightning::{self, Address, Problem},
};

const MAX_BODY: usize = 16 * 1024;
const MAX_RESPONSE: usize = 256 * 1024;
static SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(64);

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub backend: String,
    #[serde(default = "api_key_env")]
    pub api_key_env: String,
    #[serde(default = "origin")]
    pub public_origin: String,
    #[serde(default = "minimum")]
    pub min_msat: u64,
    #[serde(default = "maximum")]
    pub max_msat: u64,
}
fn api_key_env() -> String {
    "OPENAGENTS_BREEZ_API_KEY".into()
}
fn origin() -> String {
    "https://openagents.com".into()
}
fn minimum() -> u64 {
    1000
}
fn maximum() -> u64 {
    100_000_000
}
impl Config {
    fn validate(&self) -> Result<(), ()> {
        let url = reqwest::Url::parse(&self.backend).map_err(|_| ())?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
            || self.public_origin != origin()
            || self.min_msat < 1000
            || self.max_msat < self.min_msat
            || self.max_msat > 4_000_000_000
        {
            return Err(());
        }
        Ok(())
    }
    fn load() -> Result<Option<Self>, ()> {
        static CONFIG: OnceLock<Result<Option<Config>, ()>> = OnceLock::new();
        CONFIG
            .get_or_init(|| {
                let Some(path) = std::env::var_os("OPENAGENTS_LIGHTNING_ADDRESS_CONFIG") else {
                    return Ok(None);
                };
                let bytes = std::fs::read(path).map_err(|_| ())?;
                let config: Config = serde_json::from_slice(&bytes).map_err(|_| ())?;
                config.validate()?;
                Ok(Some(config))
            })
            .clone()
    }
}

pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        (
            "/v1/account/lightning-address",
            get(status).put(enroll).delete(disable),
        ),
        ("/v1/account/lightning-address/challenge", post(challenge)),
        ("/.well-known/lnurlp/{name}", get(lookup)),
        ("/lnurlp/{name}", get(lookup)),
        ("/lnurlp/{name}/invoice", get(invoice)),
        ("/lnurlp/verify/{hash}", get(verify)),
        ("/lnurlpay/{key}", post(register)),
        ("/lnurlpay/{key}/recover", post(recover)),
        ("/lnurlpay/{key}/transfer", post(transfer)),
        ("/lnurlpay/{key}/available", post(available_signed)),
        ("/lnurlpay/available/{name}", get(available)),
        ("/lnurlpay/{key}/metadata", get(metadata)),
    ]
    .into_iter()
    .map(|(path, route)| {
        (
            path,
            route.layer(axum::extract::DefaultBodyLimit::max(MAX_BODY)),
        )
    })
    .collect()
}
fn response(status: StatusCode, value: Value) -> Response {
    let mut response = (status, Json(value)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    response
}
fn failure(status: StatusCode, message: &str) -> Response {
    response(status, json!({"status":"ERROR","reason":message}))
}
fn unavailable() -> Response {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "Receiving payments isn't available right now. Try again later.",
    )
}
fn problem(problem: Problem) -> Response {
    let status = match problem {
        Problem::Conflict | Problem::Held => StatusCode::CONFLICT,
        Problem::Expired => StatusCode::GONE,
        Problem::TooMany => StatusCode::TOO_MANY_REQUESTS,
        Problem::NotFound => StatusCode::NOT_FOUND,
        _ => StatusCode::BAD_REQUEST,
    };
    response(
        status,
        json!({"error":{"code":problem,"message":problem.message()}}),
    )
}
fn stored<T: Serialize>(
    result: Result<Result<T, Problem>, tenancy::accounts::Refusal>,
) -> Response {
    match result {
        Ok(Ok(value)) => response(StatusCode::OK, json!(value)),
        Ok(Err(error)) => problem(error),
        Err(_) => unavailable(),
    }
}
fn book(state: &ServeState) -> Result<Accounts, Response> {
    Accounts::open(&state.dir).map_err(|_| unavailable())
}
fn configured() -> Result<Config, Response> {
    let config = Config::load().ok().flatten().ok_or_else(unavailable)?;
    if std::env::var(&config.api_key_env)
        .ok()
        .is_none_or(|key| key.trim().is_empty())
    {
        return Err(unavailable());
    }
    Ok(config)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Proof {
    challenge: String,
    pubkey: String,
    signature: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Disable {
    name: String,
    expected_revision: u64,
}

async fn status(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let account = match crate::nostr_sign_in::caller(&state, &headers) {
        Ok(a) => a,
        Err(e) => return e,
    };
    match book(&state).and_then(|book| book.lightning_address(&account).map_err(|_| unavailable()))
    {
        Ok(records) => response(
            StatusCode::OK,
            json!({"configured":configured().is_ok(),"addresses":records}),
        ),
        Err(e) => e,
    }
}
async fn challenge(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let account = match crate::nostr_sign_in::caller(&state, &headers) {
        Ok(a) => a,
        Err(e) => return e,
    };
    if configured().is_err() {
        return unavailable();
    }
    let Ok(request) = parse::<Revision>(&body) else {
        return problem(Problem::InvalidProof);
    };
    match book(&state) {
        Ok(book) => stored(book.lightning_challenge(&account, request.expected_revision)),
        Err(e) => e,
    }
}
async fn enroll(State(state): State<Arc<ServeState>>, headers: HeaderMap, body: Bytes) -> Response {
    let account = match crate::nostr_sign_in::caller(&state, &headers) {
        Ok(a) => a,
        Err(e) => return e,
    };
    if configured().is_err() {
        return unavailable();
    }
    let Ok(request) = parse::<Proof>(&body) else {
        return problem(Problem::InvalidProof);
    };
    if request.challenge.len() != 64 || request.pubkey.len() != 66 || request.signature.len() > 144
    {
        return problem(Problem::InvalidProof);
    }
    match book(&state) {
        Ok(book) => stored(book.enroll_lightning(
            &account,
            &request.challenge,
            &request.pubkey,
            &request.signature,
        )),
        Err(e) => e,
    }
}
async fn disable(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let account = match crate::nostr_sign_in::caller(&state, &headers) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let Ok(request) = parse::<Disable>(&body) else {
        return problem(Problem::InvalidProof);
    };
    match book(&state) {
        Ok(book) => {
            stored(book.disable_lightning(&account, &request.name, request.expected_revision))
        }
        Err(e) => e,
    }
}
fn parse<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, ()> {
    if body.len() > MAX_BODY {
        return Err(());
    }
    serde_json::from_slice(body).map_err(|_| ())
}
fn enabled(book: &Accounts, name: &str) -> Result<Address, Response> {
    if !tenancy::accounts::names::valid_name(name) {
        return Err(problem(Problem::NotFound));
    }
    lightning::active(&book.store().map_err(|_| unavailable())?, name)
        .ok_or_else(|| problem(Problem::NotFound))
}

/// Bounds the anonymous work that can reach the invoice provider.
fn admit(headers: &HeaderMap, recipient: &str) -> bool {
    type Rates = HashMap<String, (Instant, u32)>;
    static RATES: OnceLock<Mutex<Rates>> = OnceLock::new();
    let Ok(mut rates) = RATES.get_or_init(Default::default).lock() else {
        return false;
    };
    rates.retain(|_, (at, _)| at.elapsed() < Duration::from_secs(60));
    // Forwarded IPs are not trusted here; the ingress applies its own IP limit.
    let _ = headers;
    let key = recipient.to_owned();
    if !rates.contains_key(&key) && rates.len() >= 4096 {
        return false;
    }
    let (_, count) = rates.entry(key).or_insert((Instant::now(), 0));
    *count += 1;
    *count <= 120
}
async fn upstream(
    config: &Config,
    method: Method,
    path: &str,
    query: &[(String, String)],
    body: Option<Value>,
    signed_headers: Option<&HeaderMap>,
) -> Result<Value, Response> {
    let _slot = SLOTS.try_acquire().map_err(|_| {
        failure(
            StatusCode::TOO_MANY_REQUESTS,
            "Receiving is busy. Try again in a minute.",
        )
    })?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| unavailable())?;
    let mut request = client
        .request(
            method,
            format!("{}{path}", config.backend.trim_end_matches('/')),
        )
        .header(header::HOST, lightning::DOMAIN)
        .bearer_auth(std::env::var(&config.api_key_env).map_err(|_| unavailable())?)
        .query(query);
    if let Some(body) = body {
        request = request.json(&body);
    }
    if let Some(headers) = signed_headers {
        for name in ["x-breez-signature", "x-breez-timestamp"] {
            if let Some(value) = headers.get(name) {
                request = request.header(name, value);
            }
        }
    }
    let mut reply = request.send().await.map_err(|_| unavailable())?;
    if !reply.status().is_success() {
        let status = match reply.status().as_u16() {
            400 => StatusCode::BAD_REQUEST,
            404 => StatusCode::NOT_FOUND,
            409 => StatusCode::CONFLICT,
            429 => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::BAD_GATEWAY,
        };
        return Err(failure(
            status,
            "The payment request couldn't be completed. Try again.",
        ));
    }
    if reply
        .content_length()
        .is_some_and(|n| n > MAX_RESPONSE as u64)
    {
        return Err(unavailable());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = reply.chunk().await.map_err(|_| unavailable())? {
        if bytes.len() + chunk.len() > MAX_RESPONSE {
            return Err(unavailable());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| unavailable())
}
fn valid_metadata(metadata: &str, name: &str) -> bool {
    let Ok(entries) = serde_json::from_str::<Vec<Vec<String>>>(metadata) else {
        return false;
    };
    let expected = format!("{name}@{}", lightning::DOMAIN);
    entries.iter().all(|entry| entry.len() == 2)
        && entries
            .iter()
            .filter(|entry| entry.first().is_some_and(|kind| kind == "text/identifier"))
            .count()
            == 1
        && entries
            .iter()
            .any(|entry| entry[0] == "text/identifier" && entry[1] == expected)
        && entries.iter().any(|entry| entry[0] == "text/plain")
}
async fn terms(config: &Config, name: &str) -> Result<Value, Response> {
    let mut value = upstream(
        config,
        Method::GET,
        &format!("/.well-known/lnurlp/{name}"),
        &[],
        None,
        None,
    )
    .await?;
    if value["tag"] != "payRequest" {
        return Err(unavailable());
    }
    let min = value["minSendable"]
        .as_u64()
        .ok_or_else(unavailable)?
        .max(config.min_msat);
    let max = value["maxSendable"]
        .as_u64()
        .ok_or_else(unavailable)?
        .min(config.max_msat);
    if min > max
        || !valid_metadata(value["metadata"].as_str().unwrap_or(""), name)
        || (value["allowsNostr"] == true
            && !value["nostrPubkey"]
                .as_str()
                .is_some_and(|key| key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit())))
    {
        return Err(unavailable());
    }
    value["minSendable"] = json!(min);
    value["maxSendable"] = json!(max);
    value["callback"] = json!(format!("{}/lnurlp/{name}/invoice", config.public_origin));
    Ok(value)
}
async fn lookup(
    State(state): State<Arc<ServeState>>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Response {
    let config = match configured() {
        Ok(c) => c,
        Err(e) => return e,
    };
    let accounts = match book(&state) {
        Ok(a) => a,
        Err(e) => return e,
    };
    if let Err(e) = enabled(&accounts, &name) {
        return e;
    }
    if !admit(&headers, &name) {
        return failure(StatusCode::TOO_MANY_REQUESTS, "Try again in a minute.");
    }
    match terms(&config, &name).await {
        Ok(value) => response(StatusCode::OK, value),
        Err(e) => e,
    }
}
fn query(uri: &Uri) -> Result<Vec<(String, String)>, Response> {
    let raw = uri.query().unwrap_or("");
    if raw.len() > MAX_BODY {
        return Err(problem(Problem::InvalidProof));
    }
    let mut pairs = Vec::new();
    for (key, value) in url::form_urlencoded::parse(raw.as_bytes()) {
        if pairs.iter().any(|(old, _)| old == &key) {
            return Err(problem(Problem::InvalidProof));
        }
        pairs.push((key.into_owned(), value.into_owned()));
    }
    Ok(pairs)
}
fn checked_amount(pairs: &[(String, String)], terms: &Value) -> Option<u64> {
    if pairs
        .iter()
        .any(|(key, _)| !matches!(key.as_str(), "amount" | "comment" | "nostr" | "lnurl"))
    {
        return None;
    }
    if pairs.iter().any(|(key, _)| key == "nostr") && terms["allowsNostr"] != true {
        return None;
    }
    let raw = &pairs.iter().find(|(key, _)| key == "amount")?.1;
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let amount = raw.parse::<u64>().ok()?;
    if amount < terms["minSendable"].as_u64()?
        || amount > terms["maxSendable"].as_u64()?
        || amount % 1000 != 0
    {
        return None;
    }
    if let Some((_, comment)) = pairs.iter().find(|(key, _)| key == "comment") {
        if comment.chars().count() as u64 > terms["commentAllowed"].as_u64().unwrap_or(0) {
            return None;
        }
    }
    Some(amount)
}
fn valid_zap(raw: &str, recipient: &str, amount: u64) -> bool {
    let Ok(event) = serde_json::from_str::<nostr::domain::Event>(raw) else {
        return false;
    };
    event.validate_crypto().is_ok()
        && nostr::domain::open_zap_request(&event).is_ok_and(|request| {
            request.recipient == recipient
                && request.amount_msat.is_none_or(|value| value == amount)
        })
}
async fn invoice(
    State(state): State<Arc<ServeState>>,
    Path(name): Path<String>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    let config = match configured() {
        Ok(c) => c,
        Err(e) => return e,
    };
    let accounts = match book(&state) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let recipient = match enabled(&accounts, &name) {
        Ok(a) => a,
        Err(e) => return e,
    };
    if !admit(&headers, &name) {
        return failure(StatusCode::TOO_MANY_REQUESTS, "Try again in a minute.");
    }
    let pairs = match query(&uri) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let terms = match terms(&config, &name).await {
        Ok(t) => t,
        Err(e) => return e,
    };
    let Some(amount) = checked_amount(&pairs, &terms) else {
        return failure(
            StatusCode::BAD_REQUEST,
            "Choose an amount within the payment limits.",
        );
    };
    if pairs
        .iter()
        .find(|(key, _)| key == "nostr")
        .is_some_and(|(_, raw)| !valid_zap(raw, &recipient.nostr_pubkey, amount))
    {
        return failure(
            StatusCode::BAD_REQUEST,
            "That tip request doesn't match this address.",
        );
    }
    let value = match upstream(
        &config,
        Method::GET,
        &format!("/lnurlp/{name}/invoice"),
        &pairs,
        None,
        None,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return e,
    };
    let description = pairs
        .iter()
        .find(|(key, _)| key == "nostr")
        .map(|(_, v)| v.as_str())
        .unwrap_or_else(|| terms["metadata"].as_str().unwrap_or(""));
    let parsed = value["pr"]
        .as_str()
        .and_then(|invoice| nostr::x402::decode_invoice(invoice).ok());
    let now = crate::accounts::unix_now();
    let expected: [u8; 32] = Sha256::digest(description.as_bytes()).into();
    if !parsed.is_some_and(|i| {
        i.currency() == "bc"
            && i.amount_msat() == amount
            && i.description_hash() == expected
            && i.created_at().saturating_add(i.expiry_seconds()) > now
    }) {
        return unavailable();
    }
    // Recheck the revision after the provider call, so a concurrent disable wins.
    if !enabled(&accounts, &name).is_ok_and(|current| {
        current.revision == recipient.revision && current.spark_pubkey == recipient.spark_pubkey
    }) {
        return problem(Problem::NotFound);
    }
    response(StatusCode::OK, value)
}

fn authorized_key(accounts: &Accounts, key: &str, history: bool) -> Result<Address, Response> {
    if key.len() != 66 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(problem(Problem::InvalidProof));
    }
    let store = accounts.store().map_err(|_| unavailable())?;
    store
        .lightning
        .addresses
        .values()
        .find(|a| {
            (a.spark_pubkey == key
                || (history && a.historical_spark_pubkeys.iter().any(|old| old == key)))
                && store.accounts.get(&a.account).is_some_and(|account| {
                    tenancy::accounts::nostr::keys_of(account).contains(&a.nostr_pubkey)
                })
                && (history
                    || matches!(
                        a.state,
                        lightning::State::Pending | lightning::State::Active
                    ))
        })
        .cloned()
        .ok_or_else(|| problem(Problem::NotFound))
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registration {
    username: String,
    signature: String,
    timestamp: u64,
    description: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedRequest {
    signature: String,
    timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
}
fn valid_signature(signature: &str) -> bool {
    (128..=144).contains(&signature.len()) && signature.bytes().all(|b| b.is_ascii_hexdigit())
}
fn valid_timestamp(timestamp: u64) -> bool {
    timestamp.abs_diff(crate::accounts::unix_now()) <= 300
}
async fn register(
    State(state): State<Arc<ServeState>>,
    Path(key): Path<String>,
    body: Bytes,
) -> Response {
    let config = match configured() {
        Ok(c) => c,
        Err(e) => return e,
    };
    let accounts = match book(&state) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let address = match authorized_key(&accounts, &key, false) {
        Ok(a) => a,
        Err(e) => return e,
    };
    if address.previous_spark_pubkey.is_some() {
        return problem(Problem::Conflict);
    }
    let request = match parse::<Registration>(&body) {
        Ok(v) => v,
        Err(_) => return problem(Problem::InvalidProof),
    };
    if request.username != address.name
        || !valid_signature(&request.signature)
        || !valid_timestamp(request.timestamp)
        || request.description.len() > 1024
    {
        return problem(Problem::InvalidProof);
    }
    let value = json!(request);
    let value = match upstream(
        &config,
        Method::POST,
        &format!("/lnurlpay/{key}"),
        &[],
        Some(value),
        None,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return e,
    };
    if value["lightning_address"] != format!("{}@{}", address.name, lightning::DOMAIN) {
        return unavailable();
    }
    match accounts.activate_lightning(&address.name, &key, address.revision) {
        Ok(Ok(_)) => response(StatusCode::OK, value),
        Ok(Err(p)) => problem(p),
        Err(_) => unavailable(),
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Transfer {
    username: String,
    description: String,
    from_pubkey: String,
    from_signature: String,
    to_signature: String,
    timestamp: u64,
}
async fn transfer(
    State(state): State<Arc<ServeState>>,
    Path(key): Path<String>,
    body: Bytes,
) -> Response {
    let config = match configured() {
        Ok(c) => c,
        Err(e) => return e,
    };
    let accounts = match book(&state) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let address = match authorized_key(&accounts, &key, false) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let request = match parse::<Transfer>(&body) {
        Ok(v) => v,
        Err(_) => return problem(Problem::InvalidProof),
    };
    if address.previous_spark_pubkey.as_deref() != Some(&request.from_pubkey)
        || request.username != address.name
        || !valid_signature(&request.from_signature)
        || !valid_signature(&request.to_signature)
        || !valid_timestamp(request.timestamp)
        || request.description.len() > 1024
    {
        return problem(Problem::InvalidProof);
    }
    let value = match upstream(
        &config,
        Method::POST,
        &format!("/lnurlpay/{key}/transfer"),
        &[],
        Some(json!(request)),
        None,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return e,
    };
    if value["lightning_address"] != format!("{}@{}", address.name, lightning::DOMAIN) {
        return unavailable();
    }
    match accounts.activate_lightning(&address.name, &key, address.revision) {
        Ok(Ok(_)) => response(StatusCode::OK, value),
        Ok(Err(p)) => problem(p),
        Err(_) => unavailable(),
    }
}
async fn recover(
    State(state): State<Arc<ServeState>>,
    Path(key): Path<String>,
    body: Bytes,
) -> Response {
    management(&state, &key, "recover", Method::POST, None, body, None).await
}
async fn available_signed(
    State(state): State<Arc<ServeState>>,
    Path(key): Path<String>,
    body: Bytes,
) -> Response {
    management(&state, &key, "available", Method::POST, None, body, None).await
}
async fn metadata(
    State(state): State<Arc<ServeState>>,
    Path(key): Path<String>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    management(
        &state,
        &key,
        "metadata",
        Method::GET,
        Some(uri),
        Bytes::new(),
        Some(headers),
    )
    .await
}
async fn management(
    state: &ServeState,
    key: &str,
    operation: &str,
    method: Method,
    uri: Option<Uri>,
    body: Bytes,
    headers: Option<HeaderMap>,
) -> Response {
    let config = match configured() {
        Ok(c) => c,
        Err(e) => return e,
    };
    let accounts = match book(state) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let address = match authorized_key(&accounts, key, operation == "metadata") {
        Ok(a) => a,
        Err(e) => return e,
    };
    let value = if method == Method::GET {
        None
    } else {
        let request = match parse::<SignedRequest>(&body) {
            Ok(v) => v,
            Err(_) => return problem(Problem::InvalidProof),
        };
        if !valid_signature(&request.signature) || !valid_timestamp(request.timestamp) {
            return problem(Problem::InvalidProof);
        }
        if operation == "available" && request.username.as_deref() != Some(&address.name) {
            return response(StatusCode::OK, json!({"available":false}));
        }
        if operation == "recover" && request.username.is_some() {
            return problem(Problem::InvalidProof);
        }
        Some(json!(request))
    };
    if operation == "metadata" {
        let signed = headers.as_ref().is_some_and(|headers| {
            headers
                .get("x-breez-signature")
                .and_then(|v| v.to_str().ok())
                .is_some_and(valid_signature)
                && headers
                    .get("x-breez-timestamp")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse().ok())
                    .is_some_and(valid_timestamp)
        });
        if !signed {
            return problem(Problem::InvalidProof);
        }
    }
    let pairs = match uri.as_ref().map(query).transpose() {
        Ok(p) => p.unwrap_or_default(),
        Err(e) => return e,
    };
    if operation == "metadata"
        && pairs.iter().any(|(key, value)| {
            !matches!(key.as_str(), "offset" | "limit")
                || value.is_empty()
                || !value.bytes().all(|b| b.is_ascii_digit())
                || value
                    .parse::<u32>()
                    .ok()
                    .is_none_or(|number| key == "limit" && number > 200)
        })
    {
        return problem(Problem::InvalidProof);
    }
    match upstream(
        &config,
        method,
        &format!("/lnurlpay/{key}/{operation}"),
        &pairs,
        value,
        headers.as_ref(),
    )
    .await
    {
        Ok(v) => {
            if operation == "recover" {
                if v["lightning_address"] != format!("{}@{}", address.name, lightning::DOMAIN) {
                    return unavailable();
                }
                match accounts.activate_lightning(&address.name, key, address.revision) {
                    Ok(Ok(_)) => {}
                    Ok(Err(p)) => return problem(p),
                    Err(_) => return unavailable(),
                }
            }
            response(StatusCode::OK, v)
        }
        Err(e) => e,
    }
}
async fn available(Path(_name): Path<String>) -> Response {
    response(StatusCode::OK, json!({"available":false}))
}
async fn verify(State(state): State<Arc<ServeState>>, Path(hash): Path<String>) -> Response {
    let _ = state;
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return problem(Problem::NotFound);
    }
    let config = match configured() {
        Ok(c) => c,
        Err(e) => return e,
    };
    match upstream(
        &config,
        Method::GET,
        &format!("/lnurlp/verify/{hash}"),
        &[],
        None,
        None,
    )
    .await
    {
        Ok(v) => response(StatusCode::OK, v),
        Err(e) => e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn private_requests_replace_client_credentials_and_refuse_redirects() {
        use axum::{Router, routing::get};
        let captured = Arc::new(Mutex::new(None));
        let capture = captured.clone();
        let router = Router::new()
            .route(
                "/capture",
                get(move |headers: HeaderMap| {
                    let capture = capture.clone();
                    async move {
                        *capture.lock().unwrap() = Some(headers);
                        Json(json!({"ok":true}))
                    }
                }),
            )
            .route(
                "/redirect",
                get(|| async { axum::response::Redirect::temporary("/capture") }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let config = Config {
            backend: format!("http://{}", listener.local_addr().unwrap()),
            api_key_env: "PATH".into(),
            public_origin: origin(),
            min_msat: minimum(),
            max_msat: maximum(),
        };
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer account-session"),
        );
        headers.insert(header::COOKIE, HeaderValue::from_static("session=private"));
        headers.insert(
            "x-breez-signature",
            HeaderValue::from_static("wallet-signature"),
        );
        assert!(
            upstream(&config, Method::GET, "/capture", &[], None, Some(&headers))
                .await
                .is_ok()
        );
        let received = captured.lock().unwrap().take().unwrap();
        assert_eq!(received[header::HOST], "openagents.com");
        assert_eq!(
            received[header::AUTHORIZATION],
            format!("Bearer {}", std::env::var("PATH").unwrap())
        );
        assert!(!received.contains_key(header::COOKIE));
        assert_eq!(received["x-breez-signature"], "wallet-signature");
        assert!(
            upstream(&config, Method::GET, "/redirect", &[], None, None)
                .await
                .is_err()
        );
        assert!(captured.lock().unwrap().is_none());
        server.abort();
    }

    #[test]
    fn payment_metadata_must_identify_the_requested_address() {
        assert!(valid_metadata(
            r#"[["text/plain","Pay Alice"],["text/identifier","alice@openagents.com"]]"#,
            "alice"
        ));
        for metadata in [
            r#"[["text/plain","Pay Alice"]]"#,
            r#"[["text/identifier","bob@openagents.com"],["text/plain","Pay Alice"]]"#,
            r#"[["text/identifier","alice@openagents.com"],["text/identifier","mallory@openagents.com"],["text/plain","Pay Alice"]]"#,
            r#"[[],["text/plain","Pay Alice"]]"#,
            "{}",
        ] {
            assert!(!valid_metadata(metadata, "alice"));
        }
    }

    #[test]
    fn callbacks_reject_ambiguous_and_fractional_amounts() {
        let terms = json!({"minSendable":1000,"maxSendable":100000,"commentAllowed":3});
        for amount in [
            "",
            "-1000",
            "1e4",
            "1000.0",
            "999",
            "1001",
            "101000",
            "18446744073709551616",
        ] {
            assert_eq!(
                checked_amount(&[("amount".into(), amount.into())], &terms),
                None
            );
        }
        assert_eq!(
            checked_amount(
                &[
                    ("amount".into(), "1000".into()),
                    ("comment".into(), "☀☀☀".into())
                ],
                &terms
            ),
            Some(1000)
        );
        assert_eq!(
            checked_amount(
                &[
                    ("amount".into(), "1000".into()),
                    ("comment".into(), "long".into())
                ],
                &terms
            ),
            None
        );
        assert!(query(&"/pay?amount=1000&amount=2000".parse().unwrap()).is_err());
        assert!(query(&"/pay?amount=1000&%61mount=2000".parse().unwrap()).is_err());
    }

    #[test]
    fn tips_must_be_signed_for_the_address_owner_and_requested_amount() {
        use nostr::domain::{RelaySigner, Tag};
        let payer = RelaySigner::from_secret_hex(&"03".repeat(32)).unwrap();
        let recipient = RelaySigner::from_secret_hex(&"04".repeat(32)).unwrap();
        let mut event = payer.sign(
            crate::accounts::unix_now(),
            9734,
            vec![
                Tag::new(vec!["p".into(), recipient.pubkey().into()]),
                Tag::new(vec!["amount".into(), "1000".into()]),
                Tag::new(vec!["relays".into(), "wss://relay.example.com".into()]),
            ],
            String::new(),
        );
        let raw = serde_json::to_string(&event).unwrap();
        assert!(valid_zap(&raw, recipient.pubkey(), 1000));
        assert!(!valid_zap(&raw, payer.pubkey(), 1000));
        assert!(!valid_zap(&raw, recipient.pubkey(), 2000));
        event.content.push('x');
        assert!(!valid_zap(
            &serde_json::to_string(&event).unwrap(),
            recipient.pubkey(),
            1000
        ));
    }

    #[test]
    fn sdk_management_requests_are_bounded_and_typed() {
        assert!(parse::<Registration>(br#"{"username":"alice","signature":"a","timestamp":1,"description":"Pay Alice","pubkey":"other"}"#).is_err());
        assert!(
            parse::<SignedRequest>(br#"{"signature":"a","timestamp":1,"account":"other"}"#)
                .is_err()
        );
        assert!(!valid_signature(""));
        assert!(!valid_signature(&"z".repeat(128)));
        assert!(!valid_timestamp(0));
        assert!(valid_timestamp(crate::accounts::unix_now()));
    }

    #[test]
    fn private_backend_config_rejects_credentials_and_path_injection() {
        let mut config = Config {
            backend: "http://127.0.0.1:8181".into(),
            api_key_env: api_key_env(),
            public_origin: origin(),
            min_msat: minimum(),
            max_msat: maximum(),
        };
        assert!(config.validate().is_ok());
        for backend in [
            "file:///etc/passwd",
            "https://user:password@host/",
            "https://host/admin",
            "https://host/?key=value",
            "https://host/#fragment",
        ] {
            config.backend = backend.into();
            assert!(config.validate().is_err());
        }
    }
}
