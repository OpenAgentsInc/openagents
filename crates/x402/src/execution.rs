//! One fixed-charge HTTP request funds one existing task identity.
//!
//! This opt-in adapter reuses the dual-scheme front and central settlement
//! sink. The caller supplies an authenticated scope and an admitted inbox;
//! payment confers no execution, disclosure, provider-key, or spending grant.
//! The inbox must reconcile its stable create key before starting anything.
use crate::front::{
    Call, Config, Front, Output, Price, Quote, Route, Settlement, SettlementSink, Usage,
};
use crate::server::{Receiver, Request, Response};
use crate::{Facilitator, ReplayStore};
use crate::{payment_scheme, wire};
use nostr::x402::{binding_hash, decode_invoice, http_binding};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

const SCHEMA: &str = "openagents.route.funded-http-request.v1";
const MAX_RECORD: u64 = 256 * 1024;

/// A selected, versioned resource. Changes to its meaning require a new
/// revision. Quotes and both payment challenges freeze on the first request.
pub struct Resource {
    pub id: String,
    pub revision: String,
    pub method: String,
    pub path: String,
    pub price: Price,
    pub role: String,
    pub resource: String,
    pub plugin: Option<String>,
    pub description: String,
    /// The explicit provider recovery commitment; never a metered session.
    pub recovery_seconds: u64,
}

impl Resource {
    fn route(&self, price: Price, executor: Arc<dyn crate::front::RouteExecutor>) -> Route {
        Route {
            id: self.id.clone(),
            method: self.method.clone(),
            path: self.path.clone(),
            price,
            executor,
            role: self.role.clone(),
            resource: self.resource.clone(),
            plugin: self.plugin.clone(),
            description: self.description.clone(),
            mime_type: "application/json".into(),
            model_cost_only: false,
        }
    }
}

/// The existing task inbox, under independently admitted authority. `create`
/// is idempotent for `execution`; it submits a task, never runs a second loop.
/// Its call contains only bound request bytes and the frozen quote. Transport
/// headers and payment preimages are removed; inbox credentials are separate.
pub trait TaskInbox: Send + Sync {
    fn created(&self, execution: &str) -> Result<Option<String>, String>;
    fn create(&self, execution: &str, call: &Call<'_>) -> Result<String, String>;
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    method: String,
    url: String,
    request_hash: String,
    route: String,
    revision: String,
    resource: String,
    role: String,
    plugin: Option<String>,
    network: String,
    pay_to: String,
    realm: String,
    challenge_key_digest: String,
    timeout_seconds: u32,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    execution: String,
    binding: Binding,
    quote: Quote,
    expires_at: u64,
    recover_until: u64,
    challenge: Option<Response>,
    settlement: Option<Settlement>,
    task: Option<String>,
}

fn digest(value: &Value) -> String {
    hex::encode(Sha256::digest(payment_scheme::jcs(value).as_bytes()))
}
/// The identity a caller approves before submitting a funded request. Put
/// this digest in the body so the invoice binds the exact server quote.
pub fn quote_digest(quote: &Quote) -> String {
    digest(&json!({"schema":"openagents.route.funded-http-quote.v1","quote":quote}))
}

impl Record {
    fn purchase(&self) -> String {
        // No invoice or preimage enters the shared replay record.
        digest(&json!({"schema":SCHEMA,"execution":self.execution,
            "binding":self.binding,"quote":self.quote,"expires_at":self.expires_at,
            "recover_until":self.recover_until}))
    }
    fn invoice(&self) -> Result<String, String> {
        let challenge = self
            .challenge
            .as_ref()
            .ok_or("funded request has no challenge")?;
        let header = challenge
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(wire::PAYMENT_REQUIRED))
            .map(|(_, v)| v)
            .ok_or("funded request has no x402 terms")?;
        let terms = wire::decode_payment_required(header).map_err(|e| e.to_string())?;
        terms
            .accepts
            .first()
            .and_then(|r| r.extra.get("invoice"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "funded request has no invoice".into())
    }
}

/// The journal is private and locked across processes for the whole request.
struct Purchase {
    record: Mutex<Record>,
    path: PathBuf,
    _lock: fs::File,
}
impl Purchase {
    fn save(&self, record: &Record) -> Result<(), String> {
        let bytes = serde_json::to_vec(record).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_RECORD {
            return Err("funded request exceeds its bound".into());
        }
        let pending = self.path.with_extension("pending");
        match fs::remove_file(&pending) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&pending).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(pending, &self.path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        fs::File::open(self.path.parent().ok_or("journal has no parent")?)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Rejections reuse the retained challenge rather than issuing another
/// invoice for the same execution identity.
struct PinnedReceiver {
    receiver: Arc<dyn Receiver>,
    invoice: String,
    amount: u64,
    request_hash: String,
    timeout: u32,
}
impl Receiver for PinnedReceiver {
    fn pay_to(&self) -> String {
        self.receiver.pay_to()
    }
    fn invoice(&self, amount: u64, hash: [u8; 32], timeout: u32) -> Result<String, String> {
        if amount != self.amount
            || hex::encode(hash) != self.request_hash
            || timeout != self.timeout
        {
            return Err("invoice request changed its frozen funding terms".into());
        }
        Ok(self.invoice.clone())
    }
    fn received_msat(&self, hash: [u8; 32]) -> Result<Option<u64>, String> {
        self.receiver.received_msat(hash)
    }
}

struct FundedSink {
    purchase: Arc<Purchase>,
    sink: Arc<dyn SettlementSink>,
}
impl SettlementSink for FundedSink {
    fn on_settled(&self, settlement: &Settlement) -> Result<(), String> {
        let mut record = self
            .purchase
            .record
            .lock()
            .map_err(|_| "funding journal is poisoned")?;
        if let Some(original) = &record.settlement {
            if original.payment_hash != settlement.payment_hash
                || original.request_hash != settlement.request_hash
                || original.network != settlement.network
                || original.price_msat != settlement.price_msat
            {
                return Err("payment conflicts with the original funding".into());
            }
        } else {
            record.settlement = Some(settlement.clone());
            self.purchase.save(&record)?;
        }
        // Time, received amount, scheme, release, and author obligation keep
        // their original values even if a later observation uses another rail.
        self.sink.on_settled(
            record
                .settlement
                .as_ref()
                .ok_or("funding was not retained")?,
        )
    }
    fn on_call(&self, usage: &Usage) {
        self.sink.on_call(usage);
    }
}

struct FundedTask {
    purchase: Arc<Purchase>,
    inbox: Arc<dyn TaskInbox>,
}
impl crate::front::RouteExecutor for FundedTask {
    fn execute(&self, call: &Call<'_>) -> Result<Output, String> {
        let mut record = self
            .purchase
            .record
            .lock()
            .map_err(|_| "funding journal is poisoned")?;
        if record.settlement.is_none()
            || call.payment_hash != record.settlement.as_ref().map(|s| s.payment_hash.as_str())
        {
            return Err("task has no retained funding".into());
        }
        if record.task.is_none() {
            let task = match self.inbox.created(&record.execution)? {
                Some(task) => task,
                None => {
                    let request = Request {
                        method: call.request.method.clone(),
                        target: call.request.target.clone(),
                        headers: Vec::new(),
                        body: call.request.body.clone(),
                    };
                    let bound = Call {
                        route: call.route,
                        params: call.params,
                        request: &request,
                        payment_hash: call.payment_hash,
                        provider_keys: None,
                        quote: call.quote,
                    };
                    self.inbox.create(&record.execution, &bound)?
                }
            };
            if task.is_empty()
                || task.len() > 128
                || !task
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            {
                return Err("inbox returned an invalid task identity".into());
            }
            record.task = Some(task);
            self.purchase.save(&record)?;
        }
        Ok(Output {
            body: serde_json::to_vec(&json!({"schema":SCHEMA,"execution":record.execution,
            "task":record.task,"funded":true}))
            .map_err(|e| e.to_string())?,
            content_type: Some("application/json".into()),
        })
    }
}

/// An explicitly selected fixed-charge resource. The body must contain an
/// `idempotency_key`, the approved `quote_digest`, and a `funding_scope`
/// matching the separately authenticated scope. Both fields are bound body bytes; an unbound header cannot change
/// which account or execution the proof buys.
pub struct ExecutionFront<S: ReplayStore + Send + Sync> {
    config: Config,
    receiver: Arc<dyn Receiver>,
    replay: Arc<S>,
    sink: Arc<dyn SettlementSink>,
    inbox: Arc<dyn TaskInbox>,
    resource: Resource,
    scope: String,
    directory: PathBuf,
    skew: u64,
}
impl<S: ReplayStore + Send + Sync + 'static> ExecutionFront<S> {
    pub fn new(
        config: Config,
        receiver: Arc<dyn Receiver>,
        replay: Arc<S>,
        sink: Arc<dyn SettlementSink>,
        inbox: Arc<dyn TaskInbox>,
        resource: Resource,
        scope: String,
        directory: PathBuf,
        skew: u64,
    ) -> Result<Self, String> {
        if scope.is_empty()
            || scope.len() > 256
            || resource.revision.is_empty()
            || resource.revision.len() > 128
            || resource.path.contains(['{', '}'])
            || u64::from(config.timeout_secs)
                .checked_add(skew)
                .is_none_or(|minimum| resource.recovery_seconds <= minimum)
        {
            return Err(
                "funded resource needs a scope, revision, literal path, and recovery commitment"
                    .into(),
            );
        }
        Front::new(
            config.clone(),
            receiver.clone(),
            Facilitator::new(replay.clone(), skew),
            sink.clone(),
            vec![resource.route(
                resource.price.clone(),
                Arc::new(|_: &Call<'_>| -> Result<Output, String> {
                    Err("funded execution requires admission".into())
                }),
            )],
        )?;
        if !directory.exists() {
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(&directory).map_err(|e| e.to_string())?;
        }
        let meta = fs::symlink_metadata(&directory).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err("funding journal must be a private directory".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err("funding journal must have mode 0700".into());
            }
        }
        Ok(Self {
            config,
            receiver,
            replay,
            sink,
            inbox,
            resource,
            scope,
            directory,
            skew,
        })
    }

    fn open(&self, request: &Request, now: u64) -> Result<Arc<Purchase>, Response> {
        let error = |status, kind: &str, message: String| {
            Response::json(status, &json!({"error":{"type":kind,"message":message}}))
        };
        if request.body.len() > 64 * 1024 {
            return Err(error(
                413,
                "request_bounds",
                "funded request exceeds its bound".into(),
            ));
        }
        let body: Value = serde_json::from_slice(&request.body).map_err(|_| {
            error(
                400,
                "malformed_request",
                "expected JSON with an idempotency key".into(),
            )
        })?;
        if body.get("funding_scope").and_then(Value::as_str) != Some(self.scope.as_str()) {
            return Err(error(
                403,
                "funding_scope_mismatch",
                "the bound body does not name the authenticated funding scope".into(),
            ));
        }
        let key = body
            .get("idempotency_key")
            .and_then(Value::as_str)
            .filter(|key| {
                !key.is_empty()
                    && key.len() <= 128
                    && key
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            })
            .ok_or_else(|| {
                error(
                    400,
                    "malformed_request",
                    "expected a bounded idempotency key in the body".into(),
                )
            })?;
        let execution = digest(&json!({"schema":SCHEMA,"scope":self.scope,"key":key}));
        let lock_path = self.directory.join(format!("{execution}.lock"));
        if fs::symlink_metadata(&lock_path)
            .is_ok_and(|m| !m.is_file() || m.file_type().is_symlink())
        {
            return Err(error(
                503,
                "journal_unavailable",
                "invalid funding lock".into(),
            ));
        }
        let mut options = fs::OpenOptions::new();
        options.create(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options
            .open(lock_path)
            .map_err(|e| error(503, "journal_unavailable", e.to_string()))?;
        lock.try_lock().map_err(|_| {
            error(
                503,
                "funded_request_busy",
                "another request is reconciling this execution".into(),
            )
        })?;
        let url = format!(
            "{}{}",
            self.config.base_url.trim_end_matches('/'),
            request.target
        );
        let hash = binding_hash(
            &http_binding(&request.method, &url, &request.body, &[])
                .map_err(|e| error(400, "malformed_request", format!("{e:?}")))?,
        )
        .map_err(|e| error(400, "malformed_request", format!("{e:?}")))?;
        let binding = Binding {
            method: request.method.clone(),
            url,
            request_hash: hash,
            route: self.resource.id.clone(),
            revision: self.resource.revision.clone(),
            resource: self.resource.resource.clone(),
            role: self.resource.role.clone(),
            plugin: self.resource.plugin.clone(),
            network: self.config.network.into(),
            pay_to: self.receiver.pay_to(),
            realm: self.config.realm.clone(),
            challenge_key_digest: hex::encode(Sha256::digest(&self.config.challenge_key)),
            timeout_seconds: self.config.timeout_secs,
        };
        let path = self.directory.join(format!("{execution}.json"));
        let record = match fs::symlink_metadata(&path) {
            Ok(meta) => {
                if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_RECORD {
                    return Err(error(
                        503,
                        "journal_unavailable",
                        "invalid funding record".into(),
                    ));
                }
                let mut bytes = Vec::new();
                fs::File::open(&path)
                    .and_then(|f| f.take(MAX_RECORD + 1).read_to_end(&mut bytes))
                    .map_err(|e| error(503, "journal_unavailable", e.to_string()))?;
                let record: Record = serde_json::from_slice(&bytes)
                    .map_err(|e| error(503, "journal_unavailable", e.to_string()))?;
                if record.schema != SCHEMA
                    || record.execution != execution
                    || record.binding != binding
                {
                    return Err(error(
                        409,
                        "idempotency_conflict",
                        "this key already names another funded request".into(),
                    ));
                }
                record
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let call = Call {
                    route: &self.resource.id,
                    params: &[],
                    request,
                    payment_hash: None,
                    provider_keys: None,
                    quote: None,
                };
                let quote = self
                    .resource
                    .price
                    .quote(&call)
                    .map_err(|e| error(e.status, &e.kind, e.message))?;
                if quote.price_msat == 0 || quote.price_msat > i64::MAX as u64 {
                    return Err(error(
                        400,
                        "unpriced_request",
                        "funding needs a positive exact price".into(),
                    ));
                }
                Record {
                    schema: SCHEMA.into(),
                    execution,
                    binding,
                    quote,
                    expires_at: now
                        .checked_add(u64::from(self.config.timeout_secs))
                        .ok_or_else(|| {
                            error(400, "time_bounds", "quote expiry overflows".into())
                        })?,
                    recover_until: now.checked_add(self.resource.recovery_seconds).ok_or_else(
                        || error(400, "time_bounds", "recovery deadline overflows".into()),
                    )?,
                    challenge: None,
                    settlement: None,
                    task: None,
                }
            }
            Err(e) => return Err(error(503, "journal_unavailable", e.to_string())),
        };
        if body.get("quote_digest").and_then(Value::as_str)
            != Some(quote_digest(&record.quote).as_str())
        {
            return Err(error(
                409,
                "quote_conflict",
                "the request does not name the exact server quote".into(),
            ));
        }
        if now > record.recover_until {
            return Err(error(
                410,
                "recovery_expired",
                "the funded resource's recovery commitment expired".into(),
            ));
        }
        let purchase = Arc::new(Purchase {
            record: Mutex::new(record),
            path,
            _lock: lock,
        });
        {
            let record = purchase.record.lock().map_err(|_| {
                error(
                    503,
                    "journal_unavailable",
                    "funding journal is poisoned".into(),
                )
            })?;
            purchase
                .save(&record)
                .map_err(|e| error(503, "journal_unavailable", e))?;
        }
        Ok(purchase)
    }

    pub fn handle(&self, request: &Request, now: u64) -> Response {
        if request.method != self.resource.method
            || request.target.split('?').next() != Some(self.resource.path.as_str())
        {
            return Response::json(404, &json!({"error":{"type":"not_found"}}));
        }
        let purchase = match self.open(request, now) {
            Ok(p) => p,
            Err(e) => return e,
        };
        let record = match purchase.record.lock() {
            Ok(r) => r.clone(),
            Err(_) => return Response::json(503, &json!({"error":{"type":"journal_unavailable"}})),
        };
        let signature = request.header(wire::PAYMENT_SIGNATURE);
        let credential = request
            .header(payment_scheme::AUTHORIZATION)
            .filter(|v| payment_scheme::is_payment_authorization(v));
        if signature.is_none() && credential.is_none() {
            if let Some(challenge) = record.challenge {
                if now >= record.expires_at {
                    return Response::json(
                        410,
                        &json!({"error":{"type":"quote_expired","message":"New funding needs a new idempotency key; observe funded work with its original proof."}}),
                    );
                }
                return challenge;
            }
        }
        let invoice = record.invoice().ok();
        // Proof must pay the exact invoice retained before it was disclosed.
        let offered = if let Some(signature) = signature {
            wire::decode_payment_payload(signature).ok().and_then(|p| {
                p.accepted
                    .extra
                    .get("invoice")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
        } else if let Some(value) = credential {
            payment_scheme::parse_credential(value).ok().and_then(|c| {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(c.challenge.request)
                    .ok()?;
                let value: Value = serde_json::from_slice(&bytes).ok()?;
                value
                    .get("methodDetails")?
                    .get("invoice")?
                    .as_str()
                    .map(str::to_owned)
            })
        } else {
            None
        };
        if (signature.is_some() || credential.is_some())
            && (invoice.is_none() || offered != invoice)
        {
            if let Some(mut challenge) = record.challenge.clone() {
                if let Ok(mut body) = serde_json::from_slice::<Value>(&challenge.body) {
                    body["refusal"] = json!("funding_invoice_mismatch");
                    challenge.body = body.to_string().into_bytes();
                }
                return challenge;
            }
            return Response::json(
                400,
                &json!({"error":{"type":"funding_invoice_mismatch","message":"Read this request's challenge before paying."}}),
            );
        }
        let mut effective_now = now;
        if let Some(invoice) = &invoice {
            if let Ok(decoded) = decode_invoice(invoice) {
                let key = format!(
                    "{}:{}",
                    self.config.network,
                    hex::encode(decoded.payment_hash())
                );
                match self.replay.get(&key) {
                    Ok(Some(entry))
                        if entry.purchase == record.purchase()
                            && entry.consumed_at <= now
                            && now <= entry.retain_until =>
                    {
                        effective_now = entry.consumed_at
                    }
                    Ok(Some(_)) => {
                        return Response::json(409, &json!({"error":{"type":"funding_conflict"}}));
                    }
                    Ok(None) => {}
                    Err(_) => {
                        return Response::json(
                            503,
                            &json!({"error":{"type":"replay_store_unavailable"}}),
                        );
                    }
                }
            }
        }
        let receiver: Arc<dyn Receiver> = if let Some(invoice) = invoice {
            Arc::new(PinnedReceiver {
                receiver: self.receiver.clone(),
                invoice,
                amount: record.quote.price_msat,
                request_hash: record.binding.request_hash.clone(),
                timeout: self.config.timeout_secs,
            })
        } else {
            self.receiver.clone()
        };
        let quote = record.quote.clone();
        let front = match Front::new(
            Config {
                base_url: self.config.base_url.clone(),
                network: self.config.network,
                realm: self.config.realm.clone(),
                challenge_key: self.config.challenge_key.clone(),
                timeout_secs: self.config.timeout_secs,
            },
            receiver,
            Facilitator::for_funded_resource(self.replay.clone(), self.skew, record.recover_until),
            Arc::new(FundedSink {
                purchase: purchase.clone(),
                sink: self.sink.clone(),
            }),
            vec![self.resource.route(
                Price::Quote(Arc::new(move |_| Ok(quote.clone()))),
                Arc::new(FundedTask {
                    purchase: purchase.clone(),
                    inbox: self.inbox.clone(),
                }),
            )],
        ) {
            Ok(front) => front.with_funded_purchase(record.purchase()),
            Err(e) => {
                return Response::json(
                    503,
                    &json!({"error":{"type":"resource_unavailable","message":e}}),
                );
            }
        };
        let (response, _) = front.handle(request, effective_now);
        if response.status == 402 {
            if let Some(challenge) = record.challenge {
                return challenge;
            }
            let mut record = match purchase.record.lock() {
                Ok(r) => r,
                Err(_) => {
                    return Response::json(503, &json!({"error":{"type":"journal_unavailable"}}));
                }
            };
            let offered = response
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(wire::PAYMENT_REQUIRED))
                .and_then(|(_, value)| wire::decode_payment_required(value).ok());
            let invoice = offered
                .as_ref()
                .filter(|terms| terms.accepts.len() == 1)
                .and_then(|terms| {
                    nostr::x402::validate_challenge(
                        &terms.accepts[0],
                        &record.binding.request_hash,
                        now,
                        self.skew,
                        crate::facilitator::HTTP_ONLY,
                    )
                    .ok()
                });
            let Some(invoice) = invoice else {
                return Response::json(
                    503,
                    &json!({"error":{"type":"invalid_funding_invoice","message":"The receiver did not issue valid terms for this request."}}),
                );
            };
            record.expires_at = record.expires_at.min(
                invoice
                    .created_at()
                    .saturating_add(invoice.expiry_seconds()),
            );
            record.challenge = Some(response.clone());
            if let Err(e) = purchase.save(&record) {
                return Response::json(
                    503,
                    &json!({"error":{"type":"journal_unavailable","message":e}}),
                );
            }
        }
        response
    }
}
