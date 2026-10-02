//! Author-hosted paid resources on the pay front (#10194): `/x/{resource}`
//! sold through our receiver, so an author never runs a wallet.
//!
//! An author posts a signed registration (`openagents x402 publish`) to
//! `POST /v1/resources`: a JSON [`Registration`] under a NIP-98
//! `Authorization` event signed by their key, the body bound by its
//! `payload` tag. The front checks the signature, the upstream address
//! (an allowed scheme, no credentials, and every resolved address public
//! unless the operator allows private ones), and the payout address, then
//! appends the signed pair to the registry file and records the payout as
//! the owner's payee (`source` = `registration`).
//!
//! `GET` or `POST /x/{resource}` is then a priced route: our `402`, our
//! invoice, our replay store. The settlement names the owner and role
//! `hosted_resource`, so the ledger splits by the rule's `[hosted_resource]`
//! section before the upstream is called. The forward goes to the
//! registered upstream (plus the buyer's query) at the address that was
//! checked, with no redirects and no proxy, under a connect and total
//! timeout, with the request and response bodies capped, and carries an
//! `OpenAgents-Paid` header signed by the pay host key
//! (`openagents_x402::hosted::paid_header`), published at `GET
//! /v1/paid-key`. An upstream that fails or times out after payment leaves
//! the settlement in place and the call recorded as `execution_failed`
//! (NIP-X402's `failed`); refunds are out of scope.

use std::io::Read;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use nostr::domain::{Event, RelaySigner};
use nostr::x402::{binding_hash, http_binding};
use openagents_x402::front::{
    Call, Output as Served, Price, Quote, Route, RouteExecutor, Unpriced,
};
use openagents_x402::hosted::{
    self, Entry, KEY_PATH, PAID_HEADER, REGISTER_PATH, RESOURCE_PATH, ROLE, Registry, Signed,
};
use openagents_x402::server::{Request, Response};
use serde::Deserialize;
use serde_json::json;

use crate::pay_plugin::LedgerSink;

fn to_hex(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// The largest registration body the front reads.
const MAX_REGISTRATION: usize = 16 * 1024;

/// The route file's `[hosted]` section.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HostedSpec {
    /// The signed registrations (default `~/.openagents/x402/hosted.ndjson`).
    pub registry: Option<PathBuf>,
    /// The pay host key the paid header is signed with (default
    /// `~/.openagents/x402/paid-header.key`, made on first use).
    pub key_file: Option<PathBuf>,
    /// Upstream URL schemes allowed (default `["https"]`).
    pub schemes: Option<Vec<String>>,
    /// Allow upstreams at loopback and private addresses. Tests only.
    #[serde(default)]
    pub allow_private: bool,
    /// Total upstream time per call (default 30).
    pub timeout_secs: Option<u64>,
    /// The largest request body sold (default 1 MiB), checked before the 402.
    pub max_request_bytes: Option<usize>,
    /// The largest upstream answer passed back (default 4 MiB).
    pub max_response_bytes: Option<usize>,
}

impl HostedSpec {
    pub(crate) fn anchor(&mut self, base: &Path) {
        for path in [&mut self.registry, &mut self.key_file]
            .into_iter()
            .flatten()
        {
            if path.is_relative() {
                *path = base.join(&*path);
            }
        }
    }
}

/// What an upstream must be, and how long and how much a forward may take.
#[derive(Debug, Clone)]
pub(crate) struct Policy {
    pub schemes: Vec<String>,
    pub allow_private: bool,
    pub timeout: Duration,
    pub max_request: usize,
    pub max_response: usize,
}

impl Policy {
    pub(crate) fn from_spec(spec: &HostedSpec) -> Result<Self, String> {
        let schemes = spec.schemes.clone().unwrap_or_else(|| vec!["https".into()]);
        if schemes.is_empty() || schemes.iter().any(|s| s != "https" && s != "http") {
            return Err("hosted.schemes takes https and http only".into());
        }
        let timeout = spec.timeout_secs.unwrap_or(30);
        if timeout == 0 || timeout > 300 {
            return Err("hosted.timeout_secs must be 1 to 300".into());
        }
        Ok(Self {
            schemes,
            allow_private: spec.allow_private,
            timeout: Duration::from_secs(timeout),
            max_request: spec.max_request_bytes.unwrap_or(1024 * 1024),
            max_response: spec.max_response_bytes.unwrap_or(4 * 1024 * 1024),
        })
    }

    /// Parse `url`, hold it to the scheme list, refuse credentials, and
    /// resolve its host: every address must be public (unless private ones
    /// are allowed). The forward connects only to the addresses returned.
    pub(crate) fn check(&self, url: &str) -> Result<(reqwest::Url, Vec<SocketAddr>), String> {
        let parsed = reqwest::Url::parse(url).map_err(|e| format!("upstream {url}: {e}"))?;
        if !self.schemes.iter().any(|s| s == parsed.scheme()) {
            return Err(format!(
                "upstream scheme {} is not allowed (allowed: {})",
                parsed.scheme(),
                self.schemes.join(", ")
            ));
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err("an upstream URL carries no credentials".into());
        }
        if parsed.fragment().is_some() {
            return Err("an upstream URL has no fragment".into());
        }
        let port = parsed
            .port_or_known_default()
            .ok_or("the upstream has no port")?;
        let host = parsed.host_str().ok_or("the upstream has no host")?;
        let literal = host.trim_start_matches('[').trim_end_matches(']');
        let addrs: Vec<SocketAddr> = match literal.parse::<std::net::IpAddr>() {
            Ok(ip) => vec![SocketAddr::new(ip, port)],
            Err(_) => (host, port)
                .to_socket_addrs()
                .map_err(|e| format!("upstream host {host}: {e}"))?
                .collect(),
        };
        if addrs.is_empty() {
            return Err("the upstream host resolves to no address".into());
        }
        if !self.allow_private
            && let Some(addr) = addrs.iter().find(|a| !hosted::public_ip(a.ip()))
        {
            return Err(format!(
                "the upstream resolves to {}, which is not a public address",
                addr.ip()
            ));
        }
        Ok((parsed, addrs))
    }
}

/// The hosted resources a front sells, their registry, and the key the
/// paid header is signed with.
pub(crate) struct Hosted {
    registry: Registry,
    host: RelaySigner,
    policy: Policy,
    base_url: String,
    ledger: Option<Arc<LedgerSink>>,
}

fn unpriced(status: u16, kind: &str, message: impl Into<String>) -> Unpriced {
    Unpriced {
        status,
        kind: kind.into(),
        message: message.into(),
    }
}

/// Read the pay host key at `path` (64 hex digits), or make one there (0600).
pub(crate) fn host_key(path: &Path) -> Result<RelaySigner, String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    match std::fs::read_to_string(path) {
        Ok(text) => {
            return RelaySigner::from_secret_hex(text.trim())
                .map_err(|e| format!("{}: {e}", path.display()));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("{}: {error}", path.display())),
    }
    let signer = loop {
        let mut secret = [0u8; 32];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut random| random.read_exact(&mut secret))
            .map_err(|e| format!("/dev/urandom: {e}"))?;
        if let Ok(signer) = RelaySigner::from_secret_hex(&to_hex(secret)) {
            break (signer, to_hex(secret));
        }
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut file| {
            file.write_all(signer.1.as_bytes())
                .and_then(|()| file.sync_all())
        });
    match written {
        Ok(()) => Ok(signer.0),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => host_key(path),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

impl Hosted {
    pub(crate) fn new(
        registry: Registry,
        host: RelaySigner,
        policy: Policy,
        base_url: &str,
        ledger: Option<Arc<LedgerSink>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            registry,
            host,
            policy,
            base_url: base_url.trim_end_matches('/').to_owned(),
            ledger,
        })
    }

    /// The `[hosted]` section's front, with the default file locations.
    pub(crate) fn open(
        spec: &HostedSpec,
        base_url: &str,
        ledger: Option<Arc<LedgerSink>>,
    ) -> Result<Arc<Self>, String> {
        let home = crate::x402::x402_home();
        let registry = Registry::open(
            &spec
                .registry
                .clone()
                .unwrap_or_else(|| home.join("hosted.ndjson")),
        )?;
        let host = host_key(
            &spec
                .key_file
                .clone()
                .unwrap_or_else(|| home.join("paid-header.key")),
        )?;
        Ok(Self::new(
            registry,
            host,
            Policy::from_spec(spec)?,
            base_url,
            ledger,
        ))
    }

    pub(crate) fn host_pubkey(&self) -> &str {
        self.host.pubkey()
    }

    /// `GET` and `POST /x/{resource}`.
    pub(crate) fn routes(self: &Arc<Self>) -> Vec<Route> {
        ["GET", "POST"]
            .into_iter()
            .map(|method| {
                let hosted = Arc::clone(self);
                Route {
                    id: format!("hosted-{}", method.to_ascii_lowercase()),
                    method: method.into(),
                    path: RESOURCE_PATH.into(),
                    price: Price::Quote(Arc::new(move |call| hosted.quote(call))),
                    executor: Arc::clone(self) as Arc<dyn RouteExecutor>,
                    role: ROLE.into(),
                    resource: "x".into(),
                    plugin: None,
                    description: "an author-hosted resource sold through OpenAgents".into(),
                    mime_type: "application/octet-stream".into(),
                    model_cost_only: false,
                }
            })
            .collect()
    }

    fn entry(&self, call: &Call<'_>) -> Result<Entry, Unpriced> {
        let name = call.param("resource").unwrap_or_default();
        self.registry
            .get(name)
            .ok_or_else(|| unpriced(404, "resource_not_found", format!("no resource {name}")))
    }

    fn quote(&self, call: &Call<'_>) -> Result<Quote, Unpriced> {
        let entry = self.entry(call)?;
        let registration = &entry.registration;
        if registration.method != call.request.method {
            return Err(unpriced(
                405,
                "method_not_allowed",
                format!(
                    "{} is sold for {}",
                    registration.resource, registration.method
                ),
            ));
        }
        if call.request.body.len() > self.policy.max_request {
            return Err(unpriced(
                413,
                "request_too_large",
                format!(
                    "a request body is at most {} bytes",
                    self.policy.max_request
                ),
            ));
        }
        Ok(Quote {
            price_msat: registration.price_msat,
            author: Some(entry.owner.clone()),
            resource: Some(format!("x:{}", registration.resource)),
            ..Quote::default()
        })
    }

    /// The registration and key endpoints; `None` for any other request.
    pub(crate) fn answer(
        &self,
        request: &Request,
        pay_to: &str,
        network: &str,
        now: u64,
    ) -> Option<(Response, &'static str)> {
        let path = request.target.split('?').next().unwrap_or_default();
        if path == KEY_PATH {
            if request.method != "GET" {
                return Some((
                    Response::json(405, &json!({"error": {"type": "method_not_allowed"}})),
                    "method_not_allowed",
                ));
            }
            return Some((
                Response::json(
                    200,
                    &json!({
                        "pubkey": self.host.pubkey(),
                        "header": "OpenAgents-Paid",
                        "kind": hosted::HTTP_AUTH_KIND,
                        "window_secs": hosted::PAID_WINDOW_SECS,
                    }),
                ),
                "paid_key",
            ));
        }
        if path == REGISTER_PATH {
            if request.method != "POST" {
                return Some((
                    Response::json(405, &json!({"error": {"type": "method_not_allowed"}})),
                    "method_not_allowed",
                ));
            }
            return Some(self.register(request, pay_to, network, now));
        }
        let name = path.strip_prefix(REGISTER_PATH)?.strip_prefix('/')?;
        if request.method != "GET" {
            return Some((
                Response::json(405, &json!({"error": {"type": "method_not_allowed"}})),
                "method_not_allowed",
            ));
        }
        Some(match self.registry.get(name) {
            Some(entry) => (
                Response::json(200, &self.public(&entry, pay_to, network)),
                "registration",
            ),
            None => (
                Response::json(404, &json!({"error": {"type": "resource_not_found"}})),
                "not_found",
            ),
        })
    }

    fn public(&self, entry: &Entry, pay_to: &str, network: &str) -> serde_json::Value {
        let r = &entry.registration;
        json!({
            "resource": r.resource,
            "url": format!("{}/x/{}", self.base_url, r.resource),
            "method": r.method,
            "price_msat": r.price_msat,
            "owner": entry.owner,
            "summary": r.summary,
            "pay_to": pay_to,
            "network": network,
            "registration": entry.id,
            "registered_at": entry.registered_at,
        })
    }

    fn register(
        &self,
        request: &Request,
        pay_to: &str,
        network: &str,
        now: u64,
    ) -> (Response, &'static str) {
        let refuse = |status: u16, kind: &str, message: String| {
            (
                Response::json(
                    status,
                    &json!({"error": {"type": kind, "message": message}}),
                ),
                "registration_refused",
            )
        };
        if request.body.len() > MAX_REGISTRATION {
            return refuse(
                413,
                "request_too_large",
                "a registration is at most 16 KiB".into(),
            );
        }
        let url = format!("{}{REGISTER_PATH}", self.base_url);
        let Some(header) = request.header("authorization") else {
            return refuse(
                401,
                "unauthorized",
                "sign the registration with a NIP-98 Authorization header".into(),
            );
        };
        if let Err(error) =
            nostr::domain::parse_http_authorization(header, "POST", &url, &request.body, now)
        {
            return refuse(401, "unauthorized", error.to_string());
        }
        let event: Option<Event> = header
            .strip_prefix("Nostr ")
            .and_then(|encoded| {
                base64::engine::general_purpose::STANDARD
                    .decode(encoded.trim())
                    .ok()
            })
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let (Some(event), Ok(body)) = (event, String::from_utf8(request.body.clone())) else {
            return refuse(
                400,
                "invalid_registration",
                "the body is not UTF-8 JSON".into(),
            );
        };
        let signed = Signed { event, body };
        let (owner, registration) = match signed.verify() {
            Ok(verified) => verified,
            Err(message) => return refuse(400, "invalid_registration", message),
        };
        if let Err(message) = self.policy.check(&registration.upstream) {
            return refuse(400, "upstream_refused", message);
        }
        if pay_ledger::payee::classify(&registration.payout).is_none() {
            return refuse(
                400,
                "invalid_payout",
                "payout must be a mainnet Spark address, Lightning address, or node key".into(),
            );
        }
        let entry = match self.registry.register(&signed) {
            Ok(entry) => entry,
            Err(refused) => return refuse(refused.status, refused.kind, refused.message),
        };
        if let Some(ledger) = &self.ledger
            && let Err(message) = ledger.register_payout(&owner, &registration.payout, now)
        {
            // The registration stands; the payout is resolved again later.
            eprintln!(
                "hosted {}: payout not recorded: {message}",
                registration.resource
            );
        }
        (
            Response::json(201, &self.public(&entry, pay_to, network)),
            "registered",
        )
    }
}

impl RouteExecutor for Hosted {
    fn execute(&self, call: &Call<'_>) -> Result<Served, String> {
        let entry = self.entry(call).map_err(|u| u.message)?;
        if call.quote.and_then(|q| q.author.as_deref()) != Some(entry.owner.as_str()) {
            return Err("the resource changed owner after it was priced".into());
        }
        let registration = &entry.registration;
        let mut target = registration.upstream.clone();
        if let Some((_, query)) = call.request.target.split_once('?') {
            target.push(if target.contains('?') { '&' } else { '?' });
            target.push_str(query);
        }
        let (url, addrs) = self.policy.check(&target)?;
        let now = openagents_x402::unix_now();
        let request_hash = http_binding(
            &call.request.method,
            &format!("{}{}", self.base_url, call.request.target),
            &call.request.body,
            &[],
        )
        .and_then(|binding| binding_hash(&binding))
        .map_err(|e| format!("request binding: {e:?}"))?;
        let paid = hosted::Paid {
            resource: registration.resource.clone(),
            request_hash,
            payment_hash: call.payment_hash.unwrap_or_default().to_owned(),
            settled_at: now,
            id: String::new(),
        };
        let header = hosted::paid_header(
            &self.host,
            &paid,
            &call.request.method,
            url.as_str(),
            &call.request.body,
            now,
        );
        let mut client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(self.policy.timeout.min(Duration::from_secs(10)))
            .timeout(self.policy.timeout);
        if let Some(domain) = url.domain() {
            // Connect only to the addresses that were checked.
            client = client.resolve_to_addrs(domain, &addrs);
        }
        let client = client.build().map_err(|e| e.to_string())?;
        let method = reqwest::Method::from_bytes(call.request.method.as_bytes())
            .map_err(|e| e.to_string())?;
        let mut forward = client
            .request(method, url.clone())
            .header(PAID_HEADER, header)
            .body(call.request.body.clone());
        for name in ["content-type", "accept"] {
            if let Some(value) = call.request.header(name) {
                forward = forward.header(name, value);
            }
        }
        let response = forward
            .send()
            .map_err(|e| format!("upstream {}: {e}", registration.resource))?;
        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let mut body = Vec::new();
        response
            .take(self.policy.max_response as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|e| format!("upstream {}: {e}", registration.resource))?;
        if body.len() > self.policy.max_response {
            return Err(format!(
                "upstream answer exceeds {} bytes",
                self.policy.max_response
            ));
        }
        if !status.is_success() {
            return Err(format!("upstream answered {status}"));
        }
        Ok(Served { body, content_type })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::x402::decode_invoice;
    use nostr::x402::test_invoice::{number, payee_of, signed_by, tag, words};
    use openagents_x402::facilitator::HTTP_ONLY;
    use openagents_x402::{FileReplayStore, PaymentPayload};
    use serde_json::{Map, Value};
    use sha2::{Digest, Sha256};
    use std::collections::{HashMap, HashSet};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    const NODE: [u8; 32] = [9; 32];

    /// A receiver that signs real invoices with a test key at the real
    /// time and keeps each preimage, so the buyer "pays" by reading it.
    struct FakeReceiver {
        counter: AtomicU64,
        preimages: Mutex<HashMap<String, [u8; 32]>>,
    }

    impl FakeReceiver {
        fn pay(&self, invoice: &str) -> (String, String) {
            let hash = to_hex(decode_invoice(invoice).unwrap().payment_hash());
            let preimage = to_hex(self.preimages.lock().unwrap()[&hash]);
            (hash, preimage)
        }
    }

    impl openagents_x402::server::Receiver for FakeReceiver {
        fn pay_to(&self) -> String {
            to_hex(payee_of(NODE))
        }
        fn invoice(
            &self,
            amount: u64,
            request_hash: [u8; 32],
            expiry: u32,
        ) -> Result<String, String> {
            let n = self.counter.fetch_add(1, Ordering::SeqCst);
            let mut seed = request_hash.to_vec();
            seed.extend(n.to_be_bytes());
            let preimage: [u8; 32] = Sha256::digest(&seed).into();
            let payment_hash: [u8; 32] = Sha256::digest(preimage).into();
            let mut fields = tag(1, &words(&payment_hash));
            fields.extend(tag(16, &words(&[2; 32])));
            fields.extend(tag(23, &words(&request_hash)));
            fields.extend(tag(6, &number(u64::from(expiry))));
            let hrp = format!("lnbc{}n", amount / 100);
            let invoice = signed_by(
                NODE,
                &hrp,
                fields,
                false,
                false,
                openagents_x402::unix_now(),
            );
            self.preimages
                .lock()
                .unwrap()
                .insert(to_hex(payment_hash), preimage);
            Ok(invoice)
        }
        fn received_msat(&self, _: [u8; 32]) -> Result<Option<u64>, String> {
            Ok(None)
        }
    }

    /// What the fake upstream saw: verified paid headers, refused ones,
    /// and the payment hashes it was paid with.
    #[derive(Default)]
    struct Seen {
        verified: Vec<hosted::Paid>,
        refused: Vec<String>,
        payments: HashSet<String>,
    }

    fn listener() -> (std::net::TcpListener, u16) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        (listener, port)
    }

    /// The author's service: answers only a request whose paid header
    /// verifies against the published key, once per payment.
    fn upstream(stop: Arc<AtomicBool>, key: String, seen: Arc<Mutex<Seen>>) -> u16 {
        let (listener, port) = listener();
        std::thread::spawn(move || {
            openagents_x402::server::serve_with(listener, stop, move |request| {
                if request.target.starts_with("/slow") {
                    std::thread::sleep(Duration::from_secs(4));
                }
                let url = format!("http://127.0.0.1:{port}{}", request.target);
                let checked = request
                    .header(PAID_HEADER)
                    .ok_or_else(|| "no paid header".to_owned())
                    .and_then(|header| {
                        hosted::verify_paid(
                            header,
                            &key,
                            &request.method,
                            &url,
                            &request.body,
                            openagents_x402::unix_now(),
                        )
                    });
                let mut seen = seen.lock().unwrap();
                match checked {
                    Ok(paid) if seen.payments.insert(paid.payment_hash.clone()) => {
                        seen.verified.push(paid);
                        let query = request.target.split_once('?').map_or("", |(_, q)| q);
                        Response {
                            status: 200,
                            headers: vec![("content-type".into(), "text/plain".into())],
                            body: format!("sunny ({query})").into_bytes(),
                        }
                    }
                    Ok(paid) => {
                        seen.refused.push(format!("replayed {}", paid.payment_hash));
                        Response::json(409, &json!({"error": "replayed"}))
                    }
                    Err(why) => {
                        seen.refused.push(why);
                        Response::json(401, &json!({"error": "unpaid"}))
                    }
                }
            })
        });
        port
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        stop: Arc<AtomicBool>,
        front_url: String,
        upstream_port: u16,
        receiver: Arc<FakeReceiver>,
        ledger: Arc<LedgerSink>,
        seen: Arc<Mutex<Seen>>,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let (front_listener, front_port) = listener();
        let front_url = format!("http://127.0.0.1:{front_port}");
        let text = format!(
            "public_url = \"{front_url}\"\n[hosted]\nregistry = \"hosted.ndjson\"\nkey_file = \"paid.key\"\nschemes = [\"http\", \"https\"]\nallow_private = true\ntimeout_secs = 2\n"
        );
        let file = crate::pay::RouteFile::parse(&text, dir.path()).unwrap();
        let ledger = Arc::new(LedgerSink::in_memory());
        let hosted = Hosted::open(
            file.hosted.as_ref().unwrap(),
            &file.public_url,
            Some(ledger.clone()),
        )
        .unwrap();
        let receiver = Arc::new(FakeReceiver {
            counter: AtomicU64::new(0),
            preimages: Mutex::new(HashMap::new()),
        });
        let front = crate::pay::front(
            &file,
            nostr::x402::MAINNET,
            vec![7; 32],
            receiver.clone(),
            FileReplayStore::open(&dir.path().join("replay")).unwrap(),
            ledger.clone(),
            Some(&hosted),
        )
        .unwrap();
        let seen = Arc::new(Mutex::new(Seen::default()));
        let upstream_port = upstream(stop.clone(), hosted.host_pubkey().to_owned(), seen.clone());
        let serving = stop.clone();
        std::thread::spawn(move || {
            crate::pay::serve_front(
                front_listener,
                Arc::new(front),
                Some(hosted),
                serving,
                |_| {},
            )
        });
        Fixture {
            _dir: dir,
            stop,
            front_url,
            upstream_port,
            receiver,
            ledger,
            seen,
        }
    }

    fn author(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    fn register(f: &Fixture, by: &RelaySigner, name: &str, path: &str) -> Result<Value, String> {
        let body = json!({
            "v": 1,
            "resource": name,
            "upstream": format!("http://127.0.0.1:{}{path}", f.upstream_port),
            "method": "GET",
            "price_msat": 3_000,
            "payout": "alice@getalby.com",
        })
        .to_string();
        crate::x402::post_registration(&f.front_url, by, &body)
    }

    /// `openagents x402 fetch`'s exchange, paying from the fake receiver.
    fn buy(f: &Fixture, url: &str) -> crate::x402::Fetched {
        let request_hash = binding_hash(&http_binding("GET", url, b"", &[]).unwrap()).unwrap();
        crate::x402::fetch_paid("GET", url, b"", |required| {
            let (terms, invoice) = crate::x402::offered(required, &request_hash, HTTP_ONLY)?;
            let bolt11 = terms.extra["invoice"].as_str().unwrap().to_owned();
            let (payment_hash, preimage) = f.receiver.pay(&bolt11);
            let mut proof = Map::new();
            proof.insert("preimage".into(), Value::String(preimage.clone()));
            Ok((
                PaymentPayload {
                    x402_version: 2,
                    resource: Some(required.resource.clone()),
                    accepted: terms.clone(),
                    payload: proof,
                    extensions: None,
                },
                openagents_wallet::Proof {
                    payment_hash,
                    preimage,
                    amount_msat: invoice.amount_msat(),
                    fee_msat: 0,
                    bolt11,
                },
                invoice.amount_msat(),
            ))
        })
        .unwrap()
    }

    #[test]
    fn a_hosted_resource_is_bought_once_and_its_upstream_gets_one_verified_paid_header() {
        let f = fixture();
        let alice = author("11");

        // The author registers; nobody else can take the name.
        let registered = register(&f, &alice, "weather", "/weather").unwrap();
        let url = format!("{}/x/weather", f.front_url);
        assert_eq!(registered["url"], url);
        assert_eq!(registered["owner"], alice.pubkey());
        assert_eq!(registered["pay_to"], to_hex(payee_of(NODE)));
        let taken = register(&f, &author("22"), "weather", "/weather").unwrap_err();
        assert!(taken.contains("409"), "{taken}");

        // Bought once through the fetch exchange.
        let bought = buy(&f, &format!("{url}?city=oslo"));
        assert_eq!(
            bought.reply.status,
            200,
            "{}",
            String::from_utf8_lossy(&bought.reply.body)
        );
        assert_eq!(bought.reply.body, b"sunny (city=oslo)");
        let (proof, amount) = bought.paid.unwrap();
        assert_eq!(amount, 3_000);

        // Exactly one verified paid header, for this resource and payment.
        {
            let seen = f.seen.lock().unwrap();
            assert_eq!(seen.verified.len(), 1, "refused: {:?}", seen.refused);
            assert!(seen.refused.is_empty(), "{:?}", seen.refused);
            assert_eq!(seen.verified[0].resource, "weather");
            assert_eq!(seen.verified[0].payment_hash, proof.payment_hash);
        }

        // The ledger: one settlement for x:weather, the owner's share by
        // the rule (90%), and the payout from the signed registration.
        f.ledger.with(|ledger| {
            let settled = ledger.since(0).unwrap();
            assert_eq!(settled.len(), 1);
            assert_eq!(settled[0].resource, "x:weather");
            assert_eq!(settled[0].key, proof.payment_hash);
            let share: Vec<_> = settled[0]
                .shares
                .iter()
                .filter(|share| share.role == "resource")
                .collect();
            assert_eq!(share.len(), 1);
            assert_eq!(share[0].party, alice.pubkey());
            assert_eq!(share[0].amount_msat, 2_700);
            let payee = ledger.payee(alice.pubkey()).unwrap().unwrap();
            assert_eq!(payee.source, "registration");
            assert_eq!(payee.destination_value, "alice@getalby.com");
            let calls = ledger.calls_since(0).unwrap();
            let outcomes: Vec<_> = calls.iter().map(|(_, c)| c.outcome.as_str()).collect();
            assert_eq!(outcomes, ["challenged", "executed"]);
            assert!(calls.iter().all(|(_, c)| c.resource == "x:weather"));
        });

        // The published key is the one the header verified against.
        let key: Value = reqwest::blocking::get(format!("{}/v1/paid-key", f.front_url))
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(key["header"], "OpenAgents-Paid");

        // An unpaid request straight to the upstream is refused there.
        let direct =
            reqwest::blocking::get(format!("http://127.0.0.1:{}/weather", f.upstream_port))
                .unwrap();
        assert_eq!(direct.status().as_u16(), 401);
    }

    #[test]
    fn an_upstream_timeout_after_payment_keeps_the_settlement_and_records_a_failed_call() {
        let f = fixture();
        register(&f, &author("11"), "slow", "/slow").unwrap();
        let bought = buy(&f, &format!("{}/x/slow", f.front_url));
        assert_eq!(bought.reply.status, 500);
        let body: Value = serde_json::from_slice(&bought.reply.body).unwrap();
        assert_eq!(body["error"]["type"], "execution_failed");
        f.ledger.with(|ledger| {
            assert_eq!(ledger.since(0).unwrap().len(), 1);
            let calls = ledger.calls_since(0).unwrap();
            let last = &calls.last().unwrap().1;
            assert_eq!(last.outcome, "execution_failed");
            assert!(last.paid);
        });
        // An unknown resource is a 404, never a 402.
        let missing = reqwest::blocking::get(format!("{}/x/nothing", f.front_url)).unwrap();
        assert_eq!(missing.status().as_u16(), 404);
    }

    #[test]
    fn the_default_policy_refuses_internal_and_non_https_upstreams() {
        let policy = Policy::from_spec(&HostedSpec::default()).unwrap();
        for url in [
            "http://1.1.1.1/x",
            "file:///etc/passwd",
            "https://127.0.0.1/x",
            "https://169.254.169.254/latest/meta-data",
            "https://10.0.0.5:8443/x",
            "https://[::1]/x",
            "https://[::ffff:127.0.0.1]/x",
            "https://localhost/x",
            "https://user:pass@1.1.1.1/x",
        ] {
            assert!(policy.check(url).is_err(), "{url}");
        }
        let (url, addrs) = policy.check("https://1.1.1.1:8443/q?a=1").unwrap();
        assert_eq!(url.as_str(), "https://1.1.1.1:8443/q?a=1");
        assert_eq!(addrs, vec!["1.1.1.1:8443".parse().unwrap()]);
        assert!(
            Policy::from_spec(&HostedSpec {
                schemes: Some(vec!["ftp".into()]),
                ..HostedSpec::default()
            })
            .is_err()
        );
    }

    #[test]
    fn a_registration_to_an_internal_upstream_is_refused_at_the_front() {
        let hosted = Hosted::new(
            Registry::in_memory(),
            author("33"),
            Policy::from_spec(&HostedSpec::default()).unwrap(),
            "https://api.example.com",
            None,
        );
        let alice = author("11");
        let post = |upstream: &str, payout: &str| {
            let body = json!({"v": 1, "resource": "meta", "upstream": upstream,
                              "method": "GET", "price_msat": 1_000, "payout": payout})
            .to_string();
            let now = openagents_x402::unix_now();
            let header = hosted::http_auth(
                &alice,
                "POST",
                "https://api.example.com/v1/resources",
                body.as_bytes(),
                now,
                vec![],
            );
            let request = Request {
                method: "POST".into(),
                target: REGISTER_PATH.into(),
                headers: vec![("authorization".into(), header)],
                body: body.into_bytes(),
            };
            let (response, _) = hosted.answer(&request, "pay", "net", now).unwrap();
            let value: Value = serde_json::from_slice(&response.body).unwrap();
            (
                response.status,
                value["error"]["type"].as_str().unwrap_or("").to_owned(),
            )
        };
        assert_eq!(
            post("https://169.254.169.254/latest", "alice@getalby.com"),
            (400, "upstream_refused".into())
        );
        assert_eq!(
            post("http://1.1.1.1/x", "alice@getalby.com"),
            (400, "upstream_refused".into())
        );
        assert_eq!(
            post("https://1.1.1.1/x", "not a payout"),
            (400, "invalid_payout".into())
        );
        assert_eq!(post("https://1.1.1.1/x", "alice@getalby.com").0, 201);
        // An unsigned registration is refused.
        let request = Request {
            method: "POST".into(),
            target: REGISTER_PATH.into(),
            headers: vec![],
            body: b"{}".to_vec(),
        };
        let (response, _) = hosted.answer(&request, "pay", "net", 0).unwrap();
        assert_eq!(response.status, 401);
    }

    #[test]
    fn the_host_key_is_made_once_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("k/paid.key");
        let key = host_key(&path).unwrap();
        assert_eq!(host_key(&path).unwrap().pubkey(), key.pubkey());
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
