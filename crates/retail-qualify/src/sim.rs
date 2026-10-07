//! Simulated backends for the live bindings (#10748): a Lightning network
//! of two simulated wallets behind the real resident wallet socket, and a
//! fake Boat API server on loopback that speaks the HTTP protocol the
//! `boat` SDK sends and simulates the owner program's verbs.
//!
//! The live adapters run unchanged against them: the receiver is the real
//! [`openagents_wallet::resident::RemoteWallet`] over a Unix socket, and the
//! Boat binding is the real [`retail_cloud::boat::BoatAdapter`] over HTTP.
//! Nothing here moves money, signs a real invoice, starts a machine, or
//! reads a credential. Every invoice is `lnsim`, and every receipt built on
//! these backends is labeled a simulation.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use openagents_wallet::resident::{Served, Server};
use openagents_wallet::{
    Balance, Channel, IssuedInvoice, LightningWallet, PaymentDirection, PaymentRecord,
    PaymentStatus, Proof, WalletError,
};
use retail_cloud::boat::{OWNER_PATH, OWNER_SCRIPT};
use retail_cloud::sha256_hex;
use serde_json::{Value, json};

use crate::bound::Payer;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn hex32(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}

// ---------------------------------------------------------------------------
// Lightning

#[derive(Clone)]
struct SimInvoice {
    amount_msat: u64,
    bolt11: String,
    preimage: String,
    paid: bool,
    expires_at: u64,
}

#[derive(Default)]
struct Network {
    issued: u64,
    /// By payment hash.
    invoices: BTreeMap<String, SimInvoice>,
}

/// A simulated Lightning network: one receiver and one payer share it.
/// Invoices carry a real preimage whose SHA-256 is the payment hash, so the
/// preimage checks the adapters run are real.
#[derive(Clone, Default)]
pub struct SimNetwork {
    state: Arc<Mutex<Network>>,
}

/// The receiver wallet the resident socket serves.
pub struct SimReceiver {
    network: SimNetwork,
}

/// The customer's simulated wallet: it pays `lnsim` invoices only.
pub struct SimPayer {
    network: SimNetwork,
    /// Proofs the payer received, by payment hash.
    pub proofs: Mutex<Vec<Proof>>,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl SimNetwork {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn receiver(&self) -> SimReceiver {
        SimReceiver {
            network: self.clone(),
        }
    }

    #[must_use]
    pub fn payer(&self) -> SimPayer {
        SimPayer {
            network: self.clone(),
            proofs: Mutex::new(Vec::new()),
        }
    }

    /// How many invoices the receiver issued.
    #[must_use]
    pub fn issued(&self) -> u64 {
        lock(&self.state).issued
    }
}

impl LightningWallet for SimReceiver {
    fn node_id(&self) -> String {
        format!("02{}", "51".repeat(32))
    }

    fn receive_exact(
        &self,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<IssuedInvoice, WalletError> {
        if amount_msat == 0 {
            return Err(WalletError::Invalid("amount".into()));
        }
        let mut state = lock(&self.network.state);
        state.issued += 1;
        let preimage = hex32(
            format!(
                "sim-preimage:{}:{}:{}",
                state.issued,
                hex::encode(request_hash),
                unix_now()
            )
            .as_bytes(),
        );
        let payment_hash = hex32(&hex::decode(&preimage).unwrap_or_default());
        let bolt11 = format!("lnsim{amount_msat}n1{}", &payment_hash[..24]);
        state.invoices.insert(
            payment_hash.clone(),
            SimInvoice {
                amount_msat,
                bolt11: bolt11.clone(),
                preimage,
                paid: false,
                expires_at: unix_now() + u64::from(expiry_secs),
            },
        );
        Ok(IssuedInvoice {
            bolt11,
            payment_hash,
            amount_msat,
            description_hash: hex::encode(request_hash),
            expiry_secs,
            pay_to: self.node_id(),
        })
    }

    fn pay(&self, _: &str, _: u64, _: Duration) -> Result<Proof, WalletError> {
        Err(WalletError::Node("the retail receiver never pays".into()))
    }

    fn lookup(&self, payment_hash: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        let hash = hex::encode(payment_hash);
        let state = lock(&self.network.state);
        Ok(state.invoices.get(&hash).map(|invoice| {
            let status = if invoice.paid {
                PaymentStatus::Succeeded
            } else if unix_now() > invoice.expires_at {
                PaymentStatus::Failed
            } else {
                PaymentStatus::Pending
            };
            PaymentRecord {
                payment_hash: hash.clone(),
                direction: PaymentDirection::Inbound,
                status,
                amount_msat: invoice.paid.then_some(invoice.amount_msat),
                fee_msat: None,
                // Like the ldk wallet: an unpaid invoice's preimage stays secret.
                preimage: invoice.paid.then(|| invoice.preimage.clone()),
                bolt11: Some(invoice.bolt11.clone()),
                updated_at: unix_now(),
            }
        }))
    }

    fn balance(&self) -> Result<Balance, WalletError> {
        let state = lock(&self.network.state);
        let msat: u64 = state
            .invoices
            .values()
            .filter(|i| i.paid)
            .map(|i| i.amount_msat)
            .sum();
        Ok(Balance {
            onchain_total_sats: 0,
            onchain_spendable_sats: 0,
            lightning_total_sats: msat / 1000,
            anchor_reserve_sats: 0,
        })
    }

    fn channels(&self) -> Result<Vec<Channel>, WalletError> {
        Ok(Vec::new())
    }

    fn funding_address(&self) -> Result<String, WalletError> {
        Err(WalletError::Node(
            "the simulated receiver has no chain".into(),
        ))
    }

    fn open_channel(&self, _: &str, _: &str, _: u64, _: bool) -> Result<String, WalletError> {
        Err(WalletError::Node(
            "the simulated receiver opens no channels".into(),
        ))
    }

    fn close_channel(&self, _: &str, _: &str, _: bool) -> Result<(), WalletError> {
        Err(WalletError::Node(
            "the simulated receiver has no channels".into(),
        ))
    }
}

impl Served for SimReceiver {
    fn status(&self) -> Value {
        json!({ "running": true, "network": "simulated", "simulation": true })
    }

    fn buy_channel(&self, _: u64, _: u64, _: u32, _: bool) -> Result<Value, WalletError> {
        Err(WalletError::Node("simulated".into()))
    }

    fn channel_order(&self, _: &str) -> Result<Value, WalletError> {
        Err(WalletError::Node("simulated".into()))
    }

    fn send_onchain(&self, _: &str, _: u64) -> Result<String, WalletError> {
        Err(WalletError::Node("simulated".into()))
    }
}

impl SimPayer {
    /// Pay an `lnsim` invoice in full and return the preimage proof.
    ///
    /// # Errors
    ///
    /// An unknown or expired invoice.
    pub fn pay_invoice(&self, bolt11: &str) -> Result<Proof, WalletError> {
        let mut state = lock(&self.network.state);
        let Some((hash, invoice)) = state
            .invoices
            .iter_mut()
            .find(|(_, invoice)| invoice.bolt11 == bolt11)
        else {
            return Err(WalletError::Invalid("unknown simulated invoice".into()));
        };
        if unix_now() > invoice.expires_at {
            return Err(WalletError::Failed {
                payment_hash: hash.clone(),
                reason: "expired".into(),
            });
        }
        invoice.paid = true;
        let proof = Proof {
            payment_hash: hash.clone(),
            preimage: invoice.preimage.clone(),
            amount_msat: invoice.amount_msat,
            fee_msat: 0,
            bolt11: bolt11.to_owned(),
        };
        drop(state);
        lock(&self.proofs).push(proof.clone());
        Ok(proof)
    }
}

impl Payer for SimPayer {
    fn pay(&self, invoice: &IssuedInvoice) -> Result<Option<Proof>, String> {
        self.pay_invoice(&invoice.bolt11)
            .map(Some)
            .map_err(|e| e.to_string())
    }
}

/// The resident receiver wallet on a private socket, served from its own
/// thread until dropped.
pub struct Resident {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Resident {
    /// Serve `receiver` at `home/control.sock`.
    ///
    /// # Errors
    ///
    /// The socket cannot be bound.
    pub fn serve(home: &Path, receiver: SimReceiver) -> Result<Self, WalletError> {
        let server = Server::bind(home)?;
        let stop = server.stop_flag();
        let thread = std::thread::spawn(move || server.run(Arc::new(receiver)));
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Resident {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

// ---------------------------------------------------------------------------
// Boat

const ID_ALPHABET: &[u8] = b"23456789abcdefghjkmnpqrstuvwxyz";

#[derive(Clone)]
struct SimSandbox {
    id: String,
    state: &'static str,
    gets: u32,
    deleted: bool,
    deletion: Option<String>,
    files: BTreeMap<String, Vec<u8>>,
    started: BTreeSet<String>,
}

#[derive(Default)]
struct BoatState {
    next: u64,
    sandboxes: BTreeMap<String, SimSandbox>,
    /// Idempotency key to (body, sandbox).
    keys: BTreeMap<String, (Value, String)>,
    create_requests: u64,
    executors_started: u64,
    commands: Vec<String>,
    lose_next_create_reply: bool,
    usage_seconds: u64,
    unauthorized: u64,
}

/// A fake Boat API on loopback.
pub struct FakeBoat {
    state: Arc<Mutex<BoatState>>,
    base: String,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl FakeBoat {
    /// Listen on `127.0.0.1` and accept only `key`.
    ///
    /// # Errors
    ///
    /// The listener cannot bind.
    pub fn start(key: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let base = format!("http://{}/api/v1", listener.local_addr()?);
        let state = Arc::new(Mutex::new(BoatState {
            usage_seconds: 90,
            ..BoatState::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let state = Arc::clone(&state);
            let stop = Arc::clone(&stop);
            let key = key.to_owned();
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let state = Arc::clone(&state);
                            let key = key.clone();
                            std::thread::spawn(move || serve(stream, &state, &key));
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(5)),
                    }
                }
            })
        };
        Ok(Self {
            state,
            base,
            stop,
            thread: Some(thread),
        })
    }

    /// The API base the adapter is configured with.
    #[must_use]
    pub fn base(&self) -> &str {
        &self.base
    }

    /// The next create creates the sandbox and drops the connection
    /// without a reply.
    pub fn lose_next_create_reply(&self) {
        lock(&self.state).lose_next_create_reply = true;
    }

    /// The billed seconds every sandbox reports.
    pub fn set_usage_seconds(&self, seconds: u64) {
        lock(&self.state).usage_seconds = seconds;
    }

    /// Distinct sandboxes created.
    #[must_use]
    pub fn sandboxes_created(&self) -> usize {
        lock(&self.state).sandboxes.len()
    }

    /// Create requests received, including retries.
    #[must_use]
    pub fn create_requests(&self) -> u64 {
        lock(&self.state).create_requests
    }

    /// Sandboxes not deleted.
    #[must_use]
    pub fn active(&self) -> usize {
        lock(&self.state)
            .sandboxes
            .values()
            .filter(|s| !s.deleted)
            .count()
    }

    /// Executors the owner program started.
    #[must_use]
    pub fn executors_started(&self) -> u64 {
        lock(&self.state).executors_started
    }

    /// Every command line the adapter sent.
    #[must_use]
    pub fn commands(&self) -> Vec<String> {
        lock(&self.state).commands.clone()
    }

    /// Requests refused for a wrong or missing key.
    #[must_use]
    pub fn unauthorized(&self) -> u64 {
        lock(&self.state).unauthorized
    }

    /// The bytes of one sandbox file, for assertions.
    #[must_use]
    pub fn file(&self, sandbox: &str, path: &str) -> Option<Vec<u8>> {
        lock(&self.state)
            .sandboxes
            .get(sandbox)
            .and_then(|s| s.files.get(path).cloned())
    }

    /// Every sandbox file path that ever held `needle`'s bytes.
    #[must_use]
    pub fn files_containing(&self, needle: &str) -> Vec<String> {
        lock(&self.state)
            .sandboxes
            .values()
            .flat_map(|s| {
                s.files
                    .iter()
                    .filter(|(_, b)| String::from_utf8_lossy(b).contains(needle))
                    .map(|(p, _)| p.clone())
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

impl Drop for FakeBoat {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Request {
    method: String,
    path: String,
    query: BTreeMap<String, String>,
    headers: BTreeMap<String, String>,
    body: Option<Value>,
}

fn decode_query(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                if let Ok(v) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 2;
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn read_request(stream: &TcpStream) -> Option<Request> {
    stream.set_nonblocking(false).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_owned();
    let target = parts.next()?.to_owned();
    let mut headers = BTreeMap::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((k, v)) = header.split_once(':') {
            headers.insert(k.trim().to_lowercase(), v.trim().to_owned());
        }
    }
    let length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let query = query
        .split('&')
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (decode_query(k), decode_query(v)))
        .collect();
    Some(Request {
        method,
        path: path.to_owned(),
        query,
        headers,
        body: serde_json::from_slice(&body).ok(),
    })
}

fn respond(mut stream: &TcpStream, status: u16, body: &Value) {
    let text = body.to_string();
    let head = format!(
        "HTTP/1.1 {status} Sim\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        text.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(text.as_bytes());
}

fn error(status: u16, code: &str) -> (u16, Value) {
    (
        status,
        json!({
            "ok": false, "type": "sandbox.error", "status": status, "code": code,
            "message": code, "requestId": "req_sim",
            "error": { "code": code, "message": code, "status": status },
        }),
    )
}

fn sandbox_json(sandbox: &SimSandbox) -> Value {
    json!({
        "id": sandbox.id, "name": format!("sim {}", sandbox.id), "state": sandbox.state,
        "type": "large", "vcpu": 8, "memoryGB": 16, "billingMultiplier": 1,
        "url": format!("https://{}.sim.invalid", sandbox.id),
        "desktopAvailable": false, "snapshotAvailable": false,
    })
}

fn serve(stream: TcpStream, state: &Mutex<BoatState>, key: &str) {
    let Some(request) = read_request(&stream) else {
        return;
    };
    let authorized =
        request.headers.get("authorization").map(String::as_str) == Some(&format!("Bearer {key}"));
    let (status, body) = if authorized {
        let mut state = lock(state);
        match route(&mut state, &request) {
            Some(reply) => reply,
            // A lost reply: the effect happened, the connection closes.
            None => return,
        }
    } else {
        lock(state).unauthorized += 1;
        error(401, "unauthorized")
    };
    respond(&stream, status, &body);
}

fn route(state: &mut BoatState, request: &Request) -> Option<(u16, Value)> {
    let segments: Vec<&str> = request
        .path
        .trim_start_matches("/api/v1/")
        .split('/')
        .collect();
    Some(match (request.method.as_str(), segments.as_slice()) {
        ("POST", ["sandboxes"]) => return create(state, request),
        ("GET", ["sandboxes", id]) => match state.sandboxes.get_mut(*id) {
            Some(s) if !s.deleted => {
                s.gets += 1;
                if s.state == "provisioning" && s.gets >= 2 {
                    s.state = "ready";
                }
                (
                    200,
                    json!({ "ok": true, "type": "sandbox.info", "sandbox": sandbox_json(s) }),
                )
            }
            _ => error(404, "not_found"),
        },
        ("DELETE", ["sandboxes", id]) => {
            if request
                .headers
                .get("x-ascii-confirm-delete")
                .map(String::as_str)
                != Some(*id)
            {
                return Some(error(409, "delete_confirmation_required"));
            }
            match state.sandboxes.get_mut(*id) {
                Some(s) if !s.deleted => {
                    s.state = "archiving";
                    let op = format!("del_{}", s.id);
                    s.deletion = Some(op.clone());
                    (202, deletion(&op, id, "pending"))
                }
                _ => error(404, "not_found"),
            }
        }
        ("GET", ["deletion-operations", op]) => {
            match state
                .sandboxes
                .values_mut()
                .find(|s| s.deletion.as_deref() == Some(*op))
            {
                Some(s) => {
                    s.deleted = true;
                    s.state = "archived";
                    let id = s.id.clone();
                    (200, deletion(op, &id, "completed"))
                }
                None => error(404, "not_found"),
            }
        }
        ("GET", ["sandboxes", id, "usage"]) => match state.sandboxes.get(*id) {
            Some(s) => (
                200,
                json!({
                    "ok": true, "type": "sandbox.usage", "sandboxId": s.id, "sandboxType": "large",
                    "billingMultiplier": 1.0, "since": "2026-10-06T00:00:00Z",
                    "until": "2026-10-06T00:01:30Z", "seconds": state.usage_seconds,
                    "dollars": 0.0, "secondsPerDollar": 3600, "running": !s.deleted,
                }),
            ),
            None => error(404, "not_found"),
        },
        ("PUT", ["sandboxes", id, "files"]) => {
            let body = request.body.as_ref()?;
            let path = body["path"].as_str().unwrap_or_default().to_owned();
            let content = body["content"].as_str().unwrap_or_default();
            let bytes = if body["encoding"] == "base64" {
                base64::engine::general_purpose::STANDARD
                    .decode(content)
                    .unwrap_or_default()
            } else {
                content.as_bytes().to_vec()
            };
            match state.sandboxes.get_mut(*id) {
                Some(s) if !s.deleted && s.state == "ready" => {
                    let size = bytes.len();
                    s.files.insert(path.clone(), bytes);
                    (
                        200,
                        json!({ "ok": true, "type": "file.write", "success": true, "path": path, "encoding": "utf8", "size": size }),
                    )
                }
                _ => error(404, "not_found"),
            }
        }
        ("GET", ["sandboxes", id, "files"]) => {
            let path = request.query.get("path").cloned().unwrap_or_default();
            let base64 = request.query.get("encoding").map(String::as_str) == Some("base64");
            match state
                .sandboxes
                .get(*id)
                .filter(|s| !s.deleted)
                .and_then(|s| s.files.get(&path))
            {
                Some(bytes) => {
                    let content = if base64 {
                        base64::engine::general_purpose::STANDARD.encode(bytes)
                    } else {
                        String::from_utf8_lossy(bytes).into_owned()
                    };
                    (
                        200,
                        json!({
                            "ok": true, "type": "file.read", "success": true, "path": path,
                            "encoding": if base64 { "base64" } else { "utf8" },
                            "size": bytes.len(), "content": content,
                        }),
                    )
                }
                None => error(404, "not_found"),
            }
        }
        ("POST", ["sandboxes", id, "commands"]) => {
            let command = request.body.as_ref()?["command"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            state.commands.push(command.clone());
            let Some(sandbox) = state.sandboxes.get_mut(*id).filter(|s| !s.deleted) else {
                return Some(error(404, "not_found"));
            };
            let (exit, stdout, started) = owner(sandbox, &command);
            if started {
                state.executors_started += 1;
            }
            (
                200,
                json!({
                    "ok": true, "type": "command.result", "success": exit == 0, "exitCode": exit,
                    "stdout": stdout, "stderr": "", "timedOut": false,
                }),
            )
        }
        _ => error(404, "not_found"),
    })
}

fn deletion(op: &str, target: &str, status: &str) -> Value {
    json!({
        "ok": true, "type": "deletion.operation",
        "operation": {
            "id": op, "kind": "sandbox", "targetId": target, "reason": "explicit",
            "status": status, "attemptCount": 1, "requestedAt": "2026-10-06T00:00:00Z",
            "completedAt": if status == "completed" { json!("2026-10-06T00:00:01Z") } else { Value::Null },
        },
    })
}

fn create(state: &mut BoatState, request: &Request) -> Option<(u16, Value)> {
    state.create_requests += 1;
    let body = request.body.clone().unwrap_or(Value::Null);
    let key = request.headers.get("idempotency-key").cloned();
    if let Some(key) = &key
        && let Some((first, id)) = state.keys.get(key)
    {
        if *first != body {
            return Some(error(409, "idempotency_key_reused"));
        }
        let sandbox = &state.sandboxes[id];
        return Some((200, created(sandbox)));
    }
    state.next += 1;
    let mut n = state.next * 7_919;
    let mut id = String::from("bx_");
    for _ in 0..8 {
        id.push(char::from(
            ID_ALPHABET[(n % ID_ALPHABET.len() as u64) as usize],
        ));
        n /= ID_ALPHABET.len() as u64;
        n += 3;
    }
    let sandbox = SimSandbox {
        id: id.clone(),
        state: "provisioning",
        gets: 0,
        deleted: false,
        deletion: None,
        files: BTreeMap::new(),
        started: BTreeSet::new(),
    };
    let reply = created(&sandbox);
    state.sandboxes.insert(id.clone(), sandbox);
    if let Some(key) = key {
        state.keys.insert(key, (body, id));
    }
    if std::mem::take(&mut state.lose_next_create_reply) {
        return None;
    }
    Some((201, reply))
}

fn created(sandbox: &SimSandbox) -> Value {
    json!({
        "ok": true, "type": "sandbox.created", "status": "provisioning", "ttlSeconds": 4800,
        "sandbox": sandbox_json(sandbox),
    })
}

/// Split a command line the adapter built with single-quote shell quoting.
fn words(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut any = false;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                quoted = !quoted;
                any = true;
            }
            '\\' if !quoted => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            ' ' if !quoted => {
                if any || !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
                any = false;
            }
            c => current.push(c),
        }
    }
    if any || !current.is_empty() {
        out.push(current);
    }
    out
}

fn text(files: &BTreeMap<String, Vec<u8>>, path: &str) -> Option<String> {
    files
        .get(path)
        .map(|b| String::from_utf8_lossy(b).into_owned())
}

/// Simulate one owner verb. Returns the exit status, standard output, and
/// whether an executor started.
fn owner(sandbox: &mut SimSandbox, command: &str) -> (i32, String, bool) {
    let args = words(command);
    if args.first().map(String::as_str) != Some("sh")
        || args.get(1).map(String::as_str) != Some(OWNER_PATH)
    {
        return (127, String::new(), false);
    }
    // The program must be the exact one the adapter ships.
    if sandbox.files.get(OWNER_PATH).map(Vec::as_slice) != Some(OWNER_SCRIPT.as_bytes()) {
        return (127, String::new(), false);
    }
    let arg = |n: usize| args.get(n).cloned().unwrap_or_default();
    let files = &mut sandbox.files;
    match arg(2).as_str() {
        "clone" => {
            let (repo, commit, ws) = (arg(3), arg(4), arg(5));
            let parent = ws.rsplit_once('/').map_or("", |(p, _)| p).to_owned();
            files.insert(
                format!("{parent}/source"),
                format!("{repo} {commit}\n").into_bytes(),
            );
            files.insert(format!("{ws}/.git/HEAD"), commit.clone().into_bytes());
            (0, format!("{commit} clean\n"), false)
        }
        "prepare" => {
            files.insert(arg(3), Vec::new());
            (0, "prepared\n".into(), false)
        }
        "private" => (0, "600\n".into(), false),
        "remove" => {
            files.remove(&arg(3));
            (0, "removed\n".into(), false)
        }
        "exists" => {
            let found = files.contains_key(&arg(3));
            (0, if found { "yes\n" } else { "no\n" }.into(), false)
        }
        "submit" => {
            let dir = arg(3);
            if !sandbox.started.insert(dir.clone()) {
                return (0, "existing\n".into(), false);
            }
            simulate_run(&mut sandbox.files, &dir);
            (0, "new\n".into(), true)
        }
        "stop" => {
            let (dir, request) = (arg(3), arg(4));
            let receipt = format!("{dir}/stop/{request}");
            if files.contains_key(&receipt) {
                return (0, "existing\n".into(), false);
            }
            let status = text(files, &format!("{dir}/status")).unwrap_or_default();
            let status = if status.starts_with("ended") {
                status
            } else {
                files.insert(format!("{dir}/status"), b"cancelled\n".to_vec());
                "cancelled\n".into()
            };
            let mut body = format!(
                "at {}\nstarted {}\n",
                unix_now(),
                u8::from(sandbox.started.contains(&dir))
            );
            if let Some(patch) = files.get(&format!("{dir}/artifacts/patch")) {
                body.push_str(&format!("effect {}\n", sha256_hex(patch)));
            }
            body.push_str("status\n");
            body.push_str(&status);
            sandbox.files.insert(receipt, body.into_bytes());
            (0, "stopped\n".into(), false)
        }
        _ => (64, String::new(), false),
    }
}

/// What the owner program's runner leaves behind, with a simulated
/// executor: a patch, a passing run of each frozen check, a log, a
/// manifest, events, and the ended status.
fn simulate_run(files: &mut BTreeMap<String, Vec<u8>>, dir: &str) {
    let prompt = text(files, &format!("{dir}/prompt")).unwrap_or_default();
    let count: usize = text(files, &format!("{dir}/checks/count"))
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(0);
    let patch = format!(
        "diff --git a/src/parse.rs b/src/parse.rs\n+// simulated executor: {}\n",
        prompt.lines().next().unwrap_or_default()
    )
    .into_bytes();
    let patch_digest = sha256_hex(&patch);
    let mut checks_out = String::new();
    let mut status = format!("ended completed {patch_digest}\n");
    for n in 0..count {
        let check = text(files, &format!("{dir}/checks/{n}")).unwrap_or_default();
        checks_out.push_str(&format!("$ {check}\nsimulated: passed\n"));
        status.push_str(&format!("check {n} 0\n"));
    }
    let log = b"simulated executor: completed\n".to_vec();
    let artifacts = [
        ("patch", "patch", patch),
        ("checks", "checks", checks_out.into_bytes()),
        ("log", "log", log),
    ];
    let mut manifest = String::new();
    for (name, kind, bytes) in artifacts {
        manifest.push_str(&format!(
            "{name} {kind} {} {}\n",
            sha256_hex(&bytes),
            bytes.len()
        ));
        files.insert(format!("{dir}/artifacts/{name}"), bytes);
    }
    files.insert(format!("{dir}/manifest"), manifest.into_bytes());
    files.insert(
        format!("{dir}/events"),
        format!("the executor started\nthe executor ended: completed\n{count} checks ran\n")
            .into_bytes(),
    );
    files.insert(format!("{dir}/status"), status.into_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_undo_the_adapter_quoting() {
        assert_eq!(
            words("sh /tmp/x.sh 'clone' 'it'\\''s' ''"),
            vec!["sh", "/tmp/x.sh", "clone", "it's", ""]
        );
        assert_eq!(words(&boat::shell_quote("a b")), vec!["a b"]);
    }

    #[test]
    fn simulated_preimages_hash_to_their_payment_hash() {
        let network = SimNetwork::new();
        let invoice = network
            .receiver()
            .receive_exact(1_000, [1; 32], 60)
            .unwrap();
        assert!(invoice.bolt11.starts_with("lnsim"));
        let record = network
            .receiver()
            .lookup(openagents_wallet::parse_hash32(&invoice.payment_hash).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(record.preimage, None);
        let proof = network.payer().pay_invoice(&invoice.bolt11).unwrap();
        assert_eq!(
            sha256_hex(&hex::decode(&proof.preimage).unwrap()),
            invoice.payment_hash
        );
    }
}
