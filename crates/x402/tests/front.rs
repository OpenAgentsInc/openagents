//! The multi-route front against a receiver that signs real BOLT11
//! invoices with a test key and a buyer that "pays" by reading the
//! preimage back from it: two routes on one replay store, both schemes on
//! one invoice, the settlement hook's ordering, and the same flows over a
//! socket.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use nostr::x402::decode_invoice;
use nostr::x402::test_invoice::{number, payee_of, signed_by, tag, words};
use openagents_x402::front::{
    Call, Config, Front, Output, Price, Route, RouteExecutor, Scheme, Settlement, SettlementSink,
};
use openagents_x402::payment_scheme::{self, PAYMENT_RECEIPT, WWW_AUTHENTICATE};
use openagents_x402::server::{Receiver, Request, Response};
use openagents_x402::wire::{decode_header, decode_payment_required};
use openagents_x402::{
    Facilitator, FileReplayStore, PAYMENT_REQUIRED, PAYMENT_RESPONSE, PAYMENT_SIGNATURE,
    PaymentPayload, SettlementResponse,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const NODE: [u8; 32] = [9; 32];
const BASE: &str = "https://api.example.com";

/// Signs invoices as `NODE` and keeps each preimage so the test buyer can
/// "pay" by looking it up, as a wallet returns it after a payment.
struct TestNode {
    created_at: Option<u64>,
    counter: AtomicU64,
    preimages: Mutex<HashMap<String, [u8; 32]>>,
    received: Option<u64>,
}

impl TestNode {
    fn new(created_at: Option<u64>) -> Self {
        Self {
            created_at,
            counter: AtomicU64::new(0),
            preimages: Mutex::new(HashMap::new()),
            received: None,
        }
    }

    fn pay(&self, invoice: &str) -> String {
        let hash = hex::encode(decode_invoice(invoice).unwrap().payment_hash());
        hex::encode(self.preimages.lock().unwrap()[&hash])
    }
}

fn hrp(amount_msat: u64) -> String {
    match amount_msat {
        a if a % 100_000_000 == 0 => format!("lnbc{}m", a / 100_000_000),
        a if a % 100_000 == 0 => format!("lnbc{}u", a / 100_000),
        a if a % 100 == 0 => format!("lnbc{}n", a / 100),
        a => format!("lnbc{}p", a * 10),
    }
}

impl Receiver for TestNode {
    fn pay_to(&self) -> String {
        hex::encode(payee_of(NODE))
    }
    fn invoice(&self, amount: u64, request_hash: [u8; 32], expiry: u32) -> Result<String, String> {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let mut seed = request_hash.to_vec();
        seed.extend(n.to_be_bytes());
        let preimage: [u8; 32] = Sha256::digest(&seed).into();
        let payment_hash: [u8; 32] = Sha256::digest(preimage).into();
        let mut fields = tag(1, &words(&payment_hash));
        fields.extend(tag(16, &words(&[2; 32])));
        fields.extend(tag(23, &words(&request_hash)));
        fields.extend(tag(6, &number(u64::from(expiry))));
        let created_at = self.created_at.unwrap_or_else(openagents_x402::unix_now);
        let invoice = signed_by(NODE, &hrp(amount), fields, false, false, created_at);
        self.preimages
            .lock()
            .unwrap()
            .insert(hex::encode(payment_hash), preimage);
        Ok(invoice)
    }
    fn received_msat(&self, _: [u8; 32]) -> Result<Option<u64>, String> {
        Ok(self.received)
    }
}

#[derive(Default)]
struct Ledger {
    rows: Mutex<Vec<Settlement>>,
    fail: AtomicBool,
}

impl SettlementSink for Ledger {
    fn on_settled(&self, settlement: &Settlement) -> Result<(), String> {
        if self.fail.load(Ordering::SeqCst) {
            return Err("ledger unavailable".into());
        }
        self.rows.lock().unwrap().push(settlement.clone());
        Ok(())
    }
}

#[derive(Default)]
struct Counted {
    runs: AtomicUsize,
    seen: Mutex<Vec<(String, Vec<(String, String)>, String)>>,
}

impl RouteExecutor for Counted {
    fn execute(&self, call: &Call<'_>) -> Result<Output, String> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        self.seen.lock().unwrap().push((
            call.route.to_string(),
            call.params.to_vec(),
            call.payment_hash.unwrap_or_default().to_string(),
        ));
        let mut body = format!("{}:", call.route).into_bytes();
        body.extend(&call.request.body);
        Ok(Output {
            body,
            content_type: None,
        })
    }
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "x402-front-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

struct Fixture {
    front: Front<FileReplayStore>,
    node: Arc<TestNode>,
    ledger: Arc<Ledger>,
    a: Arc<Counted>,
    b: Arc<Counted>,
    dir: PathBuf,
}

fn route(id: &str, path: &str, sats: u64, executor: Arc<Counted>) -> Route {
    Route {
        id: id.into(),
        method: "POST".into(),
        path: path.into(),
        price: Price::Fixed(sats * 1000),
        executor,
        role: "endpoint".into(),
        resource: format!("api:{id}"),
        plugin: None,
        description: format!("{id} call"),
        model_cost_only: false,
        mime_type: "text/plain".into(),
    }
}

fn fixture(name: &str, node: TestNode) -> Fixture {
    let dir = temp(name);
    let node = Arc::new(node);
    let ledger = Arc::new(Ledger::default());
    let (a, b) = (Arc::new(Counted::default()), Arc::new(Counted::default()));
    let front = Front::new(
        Config {
            base_url: BASE.into(),
            network: nostr::x402::MAINNET,
            realm: "api.example.com".into(),
            challenge_key: vec![7; 32],
            timeout_secs: 300,
        },
        node.clone(),
        Facilitator::new(FileReplayStore::open(&dir).unwrap(), 60),
        ledger.clone(),
        vec![
            route("messages", "/v1/messages", 21, a.clone()),
            route("invoke", "/v1/plugins/{id}/invoke", 5, b.clone()),
        ],
    )
    .unwrap();
    Fixture {
        front,
        node,
        ledger,
        a,
        b,
        dir,
    }
}

fn post(target: &str, body: &[u8], headers: Vec<(&str, String)>) -> Request {
    Request {
        method: "POST".into(),
        target: target.into(),
        headers: headers
            .into_iter()
            .map(|(n, v)| (n.to_string(), v))
            .collect(),
        body: body.to_vec(),
    }
}

fn header<'a>(response: &'a Response, name: &str) -> Option<&'a str> {
    response
        .headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// The `PAYMENT-SIGNATURE` for the 402's x402 terms, paid by `node`.
fn x402_signature(node: &TestNode, challenge: &Response) -> String {
    let required = decode_payment_required(header(challenge, PAYMENT_REQUIRED).unwrap()).unwrap();
    let accepted = required.accepts[0].clone();
    let invoice = accepted.extra["invoice"].as_str().unwrap().to_string();
    let mut payload = Map::new();
    payload.insert("preimage".into(), json!(node.pay(&invoice)));
    openagents_x402::wire::encode_header(&PaymentPayload {
        x402_version: 2,
        resource: None,
        accepted,
        payload,
        extensions: None,
    })
    .unwrap()
}

/// Parse a `WWW-Authenticate: Payment` value the way lnget does: quoted
/// auth-params, unescaped.
fn parse_challenge(value: &str) -> Map<String, Value> {
    let rest = value.strip_prefix("Payment ").unwrap();
    let mut params = Map::new();
    let mut chars = rest.chars().peekable();
    loop {
        while matches!(chars.peek(), Some(' ' | ',')) {
            chars.next();
        }
        let name: String = chars.by_ref().take_while(|c| *c != '=').collect();
        if name.is_empty() {
            return params;
        }
        assert_eq!(chars.next(), Some('"'));
        let mut text = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => text.push(chars.next().unwrap()),
                '"' => break,
                c => text.push(c),
            }
        }
        params.insert(name, json!(text));
    }
}

/// The `Authorization: Payment` credential for the 402's challenge, paid
/// by `node`, echoing the challenge as lnget's `BuildChargeCredential`.
fn payment_credential(node: &TestNode, challenge: &Response) -> String {
    let params = parse_challenge(header(challenge, WWW_AUTHENTICATE).unwrap());
    let request: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(params["request"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    let preimage = node.pay(request["methodDetails"]["invoice"].as_str().unwrap());
    let credential = json!({"challenge": params, "payload": {"preimage": preimage}});
    format!("Payment {}", URL_SAFE_NO_PAD.encode(credential.to_string()))
}

fn problem(response: &Response) -> String {
    let body: Value = serde_json::from_slice(&response.body).unwrap();
    body["type"].as_str().unwrap_or_default().to_string()
}

const NOW: u64 = 1_800_000_000;

#[test]
fn one_invoice_in_both_encodings() {
    let f = fixture("encodings", TestNode::new(Some(NOW)));
    let (challenge, event) = f.front.handle(&post("/v1/messages", b"{}", vec![]), NOW);
    assert_eq!(challenge.status, 402);
    assert_eq!(event.outcome, "challenged");
    assert_eq!(header(&challenge, "cache-control"), Some("no-store"));
    let required = decode_payment_required(header(&challenge, PAYMENT_REQUIRED).unwrap()).unwrap();
    let terms = &required.accepts[0];
    assert_eq!(terms.amount, "21000");
    assert_eq!(terms.pay_to, hex::encode(payee_of(NODE)));
    assert_eq!(required.resource.url, format!("{BASE}/v1/messages"));

    let params = parse_challenge(header(&challenge, WWW_AUTHENTICATE).unwrap());
    assert_eq!(params["method"], "lightning");
    assert_eq!(params["intent"], "charge");
    assert_eq!(params["realm"], "api.example.com");
    assert_eq!(
        params["digest"],
        json!(payment_scheme::content_digest(b"{}"))
    );
    let request: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(params["request"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(request["amount"], "21");
    assert_eq!(request["currency"], "sat");
    assert_eq!(request["methodDetails"]["invoice"], terms.extra["invoice"]);
    let decoded = decode_invoice(terms.extra["invoice"].as_str().unwrap()).unwrap();
    assert_eq!(
        request["methodDetails"]["paymentHash"],
        json!(hex::encode(decoded.payment_hash()))
    );
    assert_eq!(
        hex::encode(decoded.description_hash()),
        event.request_hash.unwrap()
    );

    let body: Value = serde_json::from_slice(&challenge.body).unwrap();
    assert_eq!(body["price_sats"], 21);
    assert_eq!(body["challengeId"], params["id"]);
    assert_eq!(
        body["type"],
        "https://paymentauth.org/problems/payment-required"
    );

    let (missing, _) = f.front.handle(&post("/v1/nothing", b"", vec![]), NOW);
    assert_eq!(missing.status, 404);
    let mut get = post("/v1/messages", b"", vec![]);
    get.method = "GET".into();
    let (wrong, _) = f.front.handle(&get, NOW);
    assert_eq!(wrong.status, 405);
    assert_eq!(header(&wrong, "allow"), Some("POST"));
    std::fs::remove_dir_all(f.dir).unwrap();
}

#[test]
fn two_routes_share_one_replay_store_across_both_schemes() {
    let f = fixture("routes", TestNode::new(Some(NOW)));

    // Route A over x402.
    let (challenge, _) = f.front.handle(&post("/v1/messages", b"hi", vec![]), NOW);
    let signature = x402_signature(&f.node, &challenge);
    let paid = post(
        "/v1/messages",
        b"hi",
        vec![(PAYMENT_SIGNATURE, signature.clone())],
    );
    let (ok, event) = f.front.handle(&paid, NOW);
    assert_eq!(ok.status, 200, "{}", String::from_utf8_lossy(&ok.body));
    assert_eq!(ok.body, b"messages:hi");
    assert_eq!(event.scheme, Some(Scheme::X402));
    let settled: SettlementResponse =
        decode_header(header(&ok, PAYMENT_RESPONSE).unwrap()).unwrap();
    assert!(settled.success);
    assert_eq!(header(&ok, PAYMENT_RECEIPT), None);

    // The same proof again, by x402 and by the Payment scheme.
    let (again, event) = f.front.handle(&paid, NOW);
    assert_eq!(again.status, 402);
    assert_eq!(event.error_reason.as_deref(), Some("duplicate_settlement"));
    assert!(
        header(&again, PAYMENT_REQUIRED).is_some(),
        "a fresh challenge"
    );
    let credential = payment_credential(&f.node, &challenge);
    let (crossed, _) = f.front.handle(
        &post("/v1/messages", b"hi", vec![("authorization", credential)]),
        NOW,
    );
    assert_eq!(crossed.status, 402);
    assert_eq!(
        problem(&crossed),
        "https://paymentauth.org/problems/lightning/unknown-challenge"
    );

    // A fresh, paid proof for route A is refused on route B, and B never runs.
    let (challenge, _) = f.front.handle(&post("/v1/messages", b"hi", vec![]), NOW);
    let signature = x402_signature(&f.node, &challenge);
    let (foreign, event) = f.front.handle(
        &post(
            "/v1/plugins/p1/invoke",
            b"hi",
            vec![(PAYMENT_SIGNATURE, signature)],
        ),
        NOW,
    );
    assert_eq!(foreign.status, 402);
    assert_eq!(event.route.as_deref(), Some("invoke"));
    assert_eq!(f.b.runs.load(Ordering::SeqCst), 0);
    let credential = payment_credential(&f.node, &challenge);
    let (foreign, _) = f.front.handle(
        &post(
            "/v1/plugins/p1/invoke",
            b"hi",
            vec![("authorization", credential)],
        ),
        NOW,
    );
    assert_eq!(foreign.status, 402);
    assert_eq!(f.b.runs.load(Ordering::SeqCst), 0);

    // Route B over the Payment scheme, with its path parameter.
    let (challenge, _) = f
        .front
        .handle(&post("/v1/plugins/explain-error/invoke", b"x", vec![]), NOW);
    let credential = payment_credential(&f.node, &challenge);
    // A different body cannot use it.
    let (other_body, _) = f.front.handle(
        &post(
            "/v1/plugins/explain-error/invoke",
            b"y",
            vec![("authorization", credential.clone())],
        ),
        NOW,
    );
    assert_eq!(other_body.status, 402);
    assert_eq!(
        problem(&other_body),
        "https://paymentauth.org/problems/verification-failed"
    );
    let (ok, event) = f.front.handle(
        &post(
            "/v1/plugins/explain-error/invoke",
            b"x",
            vec![("authorization", credential.clone())],
        ),
        NOW,
    );
    assert_eq!(ok.status, 200, "{}", String::from_utf8_lossy(&ok.body));
    assert_eq!(event.scheme, Some(Scheme::Payment));
    let receipt: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(header(&ok, PAYMENT_RECEIPT).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(receipt["status"], "success");
    assert_eq!(receipt["method"], "lightning");
    assert_eq!(
        receipt["reference"],
        json!(event.payment_hash.clone().unwrap())
    );
    assert!(header(&ok, PAYMENT_RESPONSE).is_some());
    let seen = f.b.seen.lock().unwrap().clone();
    assert_eq!(
        seen,
        vec![(
            "invoke".to_string(),
            vec![("id".to_string(), "explain-error".to_string())],
            event.payment_hash.clone().unwrap()
        )]
    );
    // And the Payment proof cannot be replayed by x402 either.
    let (twice, _) = f.front.handle(
        &post(
            "/v1/plugins/explain-error/invoke",
            b"x",
            vec![("authorization", credential)],
        ),
        NOW,
    );
    assert_eq!(twice.status, 402);

    // One settlement per payment, each before its run.
    let rows = f.ledger.rows.lock().unwrap().clone();
    assert_eq!(rows.len(), 2);
    assert_eq!(f.a.runs.load(Ordering::SeqCst), 1);
    assert_eq!(f.b.runs.load(Ordering::SeqCst), 1);
    assert_eq!(rows[0].route, "messages");
    assert_eq!(rows[0].price_msat, 21_000);
    assert_eq!(rows[0].scheme, Scheme::X402);
    assert_eq!(rows[1].route, "invoke");
    assert_eq!(rows[1].resource, "api:invoke");
    assert_eq!(rows[1].scheme, Scheme::Payment);
    assert_ne!(rows[0].payment_hash, rows[1].payment_hash);
    std::fs::remove_dir_all(f.dir).unwrap();
}

#[test]
fn a_refusing_hook_never_executes_and_the_proof_survives() {
    let mut node = TestNode::new(Some(NOW));
    node.received = Some(20_580);
    let f = fixture("hook", node);
    f.ledger.fail.store(true, Ordering::SeqCst);
    let (challenge, _) = f.front.handle(&post("/v1/messages", b"hi", vec![]), NOW);
    let paid = post(
        "/v1/messages",
        b"hi",
        vec![(PAYMENT_SIGNATURE, x402_signature(&f.node, &challenge))],
    );
    for _ in 0..2 {
        let (refused, event) = f.front.handle(&paid, NOW);
        assert_eq!(refused.status, 503);
        assert_eq!(event.outcome, "unrecorded");
        assert_eq!(header(&refused, "retry-after"), Some("5"));
    }
    assert_eq!(f.a.runs.load(Ordering::SeqCst), 0);
    assert!(f.ledger.rows.lock().unwrap().is_empty());

    f.ledger.fail.store(false, Ordering::SeqCst);
    let (ok, _) = f.front.handle(&paid, NOW);
    assert_eq!(ok.status, 200);
    let (again, _) = f.front.handle(&paid, NOW);
    assert_eq!(again.status, 402);
    assert_eq!(f.a.runs.load(Ordering::SeqCst), 1);
    let rows = f.ledger.rows.lock().unwrap().clone();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].received_msat, 20_580);
    assert!(rows[0].received_from_wallet);
    std::fs::remove_dir_all(f.dir).unwrap();
}

#[test]
fn routes_are_validated() {
    let dir = temp("validate");
    let make = |routes: Vec<Route>, key: Vec<u8>| {
        Front::new(
            Config {
                base_url: BASE.into(),
                network: nostr::x402::MAINNET,
                realm: "r".into(),
                challenge_key: key,
                timeout_secs: 300,
            },
            Arc::new(TestNode::new(None)),
            Facilitator::new(FileReplayStore::open(&dir).unwrap(), 60),
            Arc::new(Ledger::default()),
            routes,
        )
        .map(|_| ())
    };
    let ex = Arc::new(Counted::default());
    assert!(make(vec![route("a", "/a", 1, ex.clone())], vec![1; 32]).is_ok());
    assert!(make(vec![route("a", "/a", 1, ex.clone())], vec![1; 8]).is_err());
    assert!(make(vec![route("a", "/a", 0, ex.clone())], vec![1; 32]).is_err());
    assert!(make(vec![route("a", "a", 1, ex.clone())], vec![1; 32]).is_err());
    assert!(
        make(
            vec![
                route("a", "/a", 1, ex.clone()),
                route("a", "/b", 1, ex.clone())
            ],
            vec![1; 32]
        )
        .is_err()
    );
    assert!(
        make(
            vec![
                route("a", "/a", 1, ex.clone()),
                route("b", "/a", 1, ex.clone())
            ],
            vec![1; 32]
        )
        .is_err()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// A non-whole-sat price has no `Payment` encoding; x402 still sells it.
#[test]
fn a_sub_sat_price_is_x402_only() {
    let dir = temp("subsat");
    let node = Arc::new(TestNode::new(Some(NOW)));
    let ex = Arc::new(Counted::default());
    let mut r = route("m", "/m", 1, ex.clone());
    r.price = Price::Fixed(1_500);
    let front = Front::new(
        Config {
            base_url: BASE.into(),
            network: nostr::x402::MAINNET,
            realm: "r".into(),
            challenge_key: vec![1; 32],
            timeout_secs: 300,
        },
        node.clone(),
        Facilitator::new(FileReplayStore::open(&dir).unwrap(), 60),
        Arc::new(Ledger::default()),
        vec![r],
    )
    .unwrap();
    let (challenge, _) = front.handle(&post("/m", b"", vec![]), NOW);
    assert!(header(&challenge, WWW_AUTHENTICATE).is_none());
    let paid = post(
        "/m",
        b"",
        vec![(PAYMENT_SIGNATURE, x402_signature(&node, &challenge))],
    );
    assert_eq!(front.handle(&paid, NOW).0.status, 200);
    std::fs::remove_dir_all(dir).unwrap();
}

fn http(addr: &str, request: &Request) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let mut stream = TcpStream::connect(addr).unwrap();
    let mut head = format!(
        "{} {} HTTP/1.1\r\nhost: {addr}\r\ncontent-length: {}\r\n",
        request.method,
        request.target,
        request.body.len()
    );
    for (name, value) in &request.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(&request.body).unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).unwrap();
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let text = String::from_utf8(raw[..split].to_vec()).unwrap();
    let mut lines = text.split("\r\n");
    let status = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .map(|line| {
            let (n, v) = line.split_once(':').unwrap();
            (n.to_ascii_lowercase(), v.trim().to_string())
        })
        .collect();
    (status, headers, raw[split + 4..].to_vec())
}

fn as_response(parts: (u16, Vec<(String, String)>, Vec<u8>)) -> Response {
    Response {
        status: parts.0,
        headers: parts.1,
        body: parts.2,
    }
}

/// The served front over a real socket, at the wall clock: both schemes,
/// two routes, one settlement per payment.
#[test]
fn serves_two_routes_over_a_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let f = fixture("socket", TestNode::new(None));
    let Fixture {
        front,
        node,
        ledger,
        a,
        b,
        dir,
    } = f;
    let front = Arc::new(front);
    let stop = Arc::new(AtomicBool::new(false));
    let events = Arc::new(Mutex::new(Vec::new()));
    let server = {
        let (front, stop, events) = (front.clone(), stop.clone(), events.clone());
        std::thread::spawn(move || {
            openagents_x402::front::serve(listener, front, stop, move |event| {
                events.lock().unwrap().push(event.clone());
            })
        })
    };

    let challenge = as_response(http(&addr, &post("/v1/messages", b"one", vec![])));
    assert_eq!(challenge.status, 402);
    let signature = x402_signature(&node, &challenge);
    let ok = http(
        &addr,
        &post("/v1/messages", b"one", vec![(PAYMENT_SIGNATURE, signature)]),
    );
    assert_eq!(ok.0, 200);
    assert_eq!(ok.2, b"messages:one");

    let challenge = as_response(http(
        &addr,
        &post("/v1/plugins/repo-map/invoke", b"two", vec![]),
    ));
    let credential = payment_credential(&node, &challenge);
    let ok = as_response(http(
        &addr,
        &post(
            "/v1/plugins/repo-map/invoke",
            b"two",
            vec![("Authorization", credential.clone())],
        ),
    ));
    assert_eq!(ok.status, 200);
    assert!(header(&ok, PAYMENT_RECEIPT).is_some());
    let replay = http(
        &addr,
        &post(
            "/v1/plugins/repo-map/invoke",
            b"two",
            vec![("Authorization", credential)],
        ),
    );
    assert_eq!(replay.0, 402);

    stop.store(true, Ordering::SeqCst);
    server.join().unwrap().unwrap();
    assert_eq!(ledger.rows.lock().unwrap().len(), 2);
    assert_eq!(a.runs.load(Ordering::SeqCst), 1);
    assert_eq!(b.runs.load(Ordering::SeqCst), 1);
    let outcomes: Vec<String> = events
        .lock()
        .unwrap()
        .iter()
        .map(|e| e.outcome.clone())
        .collect();
    assert_eq!(
        outcomes,
        [
            "challenged",
            "executed",
            "challenged",
            "executed",
            "refused"
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// Compatibility with lnget's `mpp` package, from a fixture.
///
/// `tests/fixtures/lnget/` holds the `WWW-Authenticate` value this front
/// issues for one fixed call, the preimage of its invoice, the
/// `Authorization` value lnget's `Handler.HandleChallenge` built from that
/// challenge (its parser, its checks, `BuildChargeCredential`), and the
/// `Payment-Receipt` this front answers, which lnget's `ParseReceipt` read.
/// `lnget_compat_test.go` there is the Go side: copy it into lnget's `mpp`
/// directory and run `go test -run TestOpenAgentsFront`. Regenerate the Rust
/// side with `OPENAGENTS_WRITE_LNGET_FIXTURE=1`.
#[test]
fn lnget_built_credential_is_accepted() {
    const AT: u64 = 4_000_000_000; // 2096, so lnget's wall-clock expiry check passes.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lnget");
    let f = fixture("lnget", TestNode::new(Some(AT)));
    let body = br#"{"message":"What is new in the Gym?"}"#;
    let (challenge, _) = f.front.handle(&post("/v1/messages", body, vec![]), AT);
    let www = header(&challenge, WWW_AUTHENTICATE).unwrap().to_string();
    let params = parse_challenge(&www);
    let request: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(params["request"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    let invoice = request["methodDetails"]["invoice"].as_str().unwrap();
    let preimage = f.node.pay(invoice);
    let payment_hash = request["methodDetails"]["paymentHash"].as_str().unwrap();
    let receipt = payment_scheme::receipt(params["id"].as_str().unwrap(), payment_hash, AT);
    if std::env::var_os("OPENAGENTS_WRITE_LNGET_FIXTURE").is_some() {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("challenge.txt"), &www).unwrap();
        std::fs::write(dir.join("preimage.txt"), &preimage).unwrap();
        std::fs::write(dir.join("receipt.txt"), &receipt).unwrap();
    }
    let read = |name: &str| {
        std::fs::read_to_string(dir.join(name))
            .unwrap()
            .trim()
            .to_string()
    };
    assert_eq!(read("challenge.txt"), www, "the challenge changed");
    assert_eq!(read("preimage.txt"), preimage);
    let authorization = read("authorization.txt");
    let (ok, event) = f.front.handle(
        &post("/v1/messages", body, vec![("Authorization", authorization)]),
        AT,
    );
    assert_eq!(ok.status, 200, "{}", String::from_utf8_lossy(&ok.body));
    assert_eq!(event.scheme, Some(Scheme::Payment));
    assert_eq!(
        header(&ok, PAYMENT_RECEIPT),
        Some(read("receipt.txt").as_str())
    );
    std::fs::remove_dir_all(f.dir).unwrap();
}

/// BYOK (#10176): a caller that brings its own provider key to a route
/// whose price is its model cost gets no `402`, the executor runs with the
/// caller's keys, the event names the payer and never the key, and a
/// malformed header is refused without echoing it. A route whose price is
/// more than model cost still challenges.
#[test]
fn a_callers_own_provider_key_pays_a_model_cost_route_with_no_402() {
    let dir = temp("byok");
    let node = Arc::new(TestNode::new(Some(NOW)));
    let ledger = Arc::new(Ledger::default());
    let (model, priced) = (Arc::new(Counted::default()), Arc::new(Counted::default()));
    let mut byok = route("messages", "/v1/messages", 21, model.clone());
    byok.model_cost_only = true;
    let front = Front::new(
        Config {
            base_url: BASE.into(),
            network: nostr::x402::MAINNET,
            realm: "api.example.com".into(),
            challenge_key: vec![7; 32],
            timeout_secs: 300,
        },
        node,
        Facilitator::new(FileReplayStore::open(&dir).unwrap(), 60),
        ledger.clone(),
        vec![
            byok,
            route("invoke", "/v1/plugins/{id}/invoke", 5, priced.clone()),
        ],
    )
    .unwrap();
    let key = "sk-or-v1-callers-own-key";
    let header = ("OpenAgents-Provider-Key", format!("openrouter {key}"));
    let (answered, event) = front.handle(&post("/v1/messages", b"{}", vec![header.clone()]), NOW);
    assert_eq!(answered.status, 200);
    assert_eq!(model.runs.load(Ordering::SeqCst), 1);
    assert_eq!(event.outcome, "caller_paid");
    assert_eq!(event.payer.as_deref(), Some("theirs"));
    assert_eq!(event.payer_provider.as_deref(), Some("openrouter"));
    let logged = serde_json::to_string(&event).unwrap();
    assert!(!logged.contains(key), "{logged}");
    assert!(ledger.rows.lock().unwrap().is_empty(), "nothing was sold");

    let (refused, event) = front.handle(
        &post(
            "/v1/messages",
            b"{}",
            vec![("OpenAgents-Provider-Key", key.to_string())],
        ),
        NOW,
    );
    assert_eq!(refused.status, 400);
    assert!(!String::from_utf8_lossy(&refused.body).contains(key));
    assert!(!serde_json::to_string(&event).unwrap().contains(key));

    let (challenged, _) = front.handle(&post("/v1/plugins/p1/invoke", b"{}", vec![header]), NOW);
    assert_eq!(
        challenged.status, 402,
        "a key never waives more than model cost"
    );
    assert_eq!(priced.runs.load(Ordering::SeqCst), 0);
}

#[path = "front/funded.rs"]
mod funded;
