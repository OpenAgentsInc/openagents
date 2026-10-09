//! The payment router (#11136) against a receiver that signs real BOLT11
//! invoices with a test key; the buyer "pays" by reading the preimage back,
//! as a wallet returns it. No wallet, no money.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use nostr::x402::test_invoice::{number, payee_of, signed_by, tag, words};
use nostr::x402::{binding_hash, decode_invoice, http_binding};
use openagents_x402::payment_scheme::{PAYMENT_RECEIPT, WWW_AUTHENTICATE};
use openagents_x402::router::{
    Adapter, Bound, Challenged, Lightning, Method, MppLightning, Quote, Router, X402Lightning,
};
use openagents_x402::server::Receiver;
use openagents_x402::wire::{decode_payment_required, encode_header};
use openagents_x402::{
    Facilitator, FileReplayStore, PAYMENT_REQUIRED, PAYMENT_RESPONSE, PAYMENT_SIGNATURE,
    PaymentPayload, ResourceInfo,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const NODE: [u8; 32] = [9; 32];
const BASE: &str = "https://api.example.com";
const NOW: u64 = 1_800_000_000;

struct TestNode {
    created_at: u64,
    counter: AtomicU64,
    preimages: Mutex<HashMap<String, [u8; 32]>>,
}

impl TestNode {
    fn new(created_at: u64) -> Self {
        Self {
            created_at,
            counter: AtomicU64::new(0),
            preimages: Mutex::new(HashMap::new()),
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
        let invoice = signed_by(NODE, &hrp(amount), fields, false, false, self.created_at);
        self.preimages
            .lock()
            .unwrap()
            .insert(hex::encode(payment_hash), preimage);
        Ok(invoice)
    }
}

struct Fixture {
    router: Router<FileReplayStore>,
    node: Arc<TestNode>,
    _dir: tempfile::TempDir,
}

fn fixture(created_at: u64, mpp: bool) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let node = Arc::new(TestNode::new(created_at));
    let mut adapters: Vec<Box<dyn Adapter>> = vec![Box::new(X402Lightning)];
    if mpp {
        adapters.push(Box::new(
            MppLightning::new("api.example.com", vec![7; 32]).unwrap(),
        ));
    }
    let router = Router::new(
        Lightning::new(
            node.clone(),
            Facilitator::new(FileReplayStore::open(dir.path()).unwrap(), 60),
            nostr::x402::MAINNET,
            300,
        ),
        adapters,
    )
    .unwrap();
    Fixture {
        router,
        node,
        _dir: dir,
    }
}

struct Call {
    url: String,
    body: Vec<u8>,
    hash: String,
    headers: Vec<(String, String)>,
}

impl Call {
    fn new(path: &str, body: &[u8]) -> Self {
        let url = format!("{BASE}{path}");
        let hash = binding_hash(&http_binding("POST", &url, body, &[]).unwrap()).unwrap();
        Self {
            url,
            body: body.to_vec(),
            hash,
            headers: Vec::new(),
        }
    }
    fn with(mut self, name: &str, value: String) -> Self {
        self.headers.push((name.to_owned(), value));
        self
    }
    fn bound(&self) -> Bound<'_> {
        Bound {
            http_method: "POST",
            url: &self.url,
            body: &self.body,
            request_hash: &self.hash,
            headers: &self.headers,
        }
    }
}

const QUOTE: Quote = Quote {
    amount_msat: 21_000,
    usd_micros: 21_000,
};

fn resource(url: &str) -> ResourceInfo {
    ResourceInfo {
        url: url.to_owned(),
        description: Some("One call".into()),
        mime_type: Some("application/json".into()),
        rest: Map::new(),
    }
}

fn challenge(f: &Fixture, call: &Call, now: u64) -> Challenged {
    f.router
        .challenge(&call.bound(), QUOTE, &resource(&call.url), None, None, now)
        .unwrap()
}

fn header<'a>(challenged: &'a Challenged, name: &str) -> Option<&'a str> {
    challenged
        .headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn params(value: &str) -> Map<String, Value> {
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

fn mpp_invoice(challenged: &Challenged) -> String {
    let p = params(header(challenged, WWW_AUTHENTICATE).unwrap());
    let request: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(p["request"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    request["methodDetails"]["invoice"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn signature(f: &Fixture, challenged: &Challenged) -> String {
    let required = decode_payment_required(header(challenged, PAYMENT_REQUIRED).unwrap()).unwrap();
    let accepted = required.accepts[0].clone();
    let invoice = accepted.extra["invoice"].as_str().unwrap().to_owned();
    let mut payload = Map::new();
    payload.insert("preimage".into(), json!(f.node.pay(&invoice)));
    encode_header(&PaymentPayload {
        x402_version: 2,
        resource: None,
        accepted,
        payload,
        extensions: None,
    })
    .unwrap()
}

fn authorization(f: &Fixture, challenged: &Challenged) -> String {
    let p = params(header(challenged, WWW_AUTHENTICATE).unwrap());
    let preimage = f.node.pay(&mpp_invoice(challenged));
    let credential = json!({"challenge": p, "payload": {"preimage": preimage}});
    format!("Payment {}", URL_SAFE_NO_PAD.encode(credential.to_string()))
}

#[test]
fn one_402_carries_both_encodings_of_one_invoice() {
    let f = fixture(NOW, true);
    let call = Call::new("/v1/responses", b"{}");
    let challenged = challenge(&f, &call, NOW);
    let required = decode_payment_required(header(&challenged, PAYMENT_REQUIRED).unwrap()).unwrap();
    let x402_invoice = required.accepts[0].extra["invoice"].as_str().unwrap();
    assert_eq!(x402_invoice, mpp_invoice(&challenged), "the same BOLT11");
    assert_eq!(required.accepts[0].amount, "21000");
    let p = params(header(&challenged, WWW_AUTHENTICATE).unwrap());
    assert_eq!(p["method"], "lightning");
    assert_eq!(p["intent"], "charge");
    assert_eq!(challenged.body["challengeId"], p["id"]);
    let methods: Vec<&str> = challenged.body["methods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(methods, ["x402", "mpp"]);
    assert_eq!(
        f.router.methods().iter().map(|m| m.id).collect::<Vec<_>>(),
        methods
    );
    // An x402-only router sends no Payment challenge and lists one method.
    let only = fixture(NOW, false);
    let challenged = challenge(&only, &call, NOW);
    assert!(header(&challenged, WWW_AUTHENTICATE).is_none());
    assert_eq!(challenged.body["methods"].as_array().unwrap().len(), 1);
}

#[test]
fn one_preimage_pays_once_whichever_encoding_carries_it() {
    // x402 first, then MPP with the same invoice's preimage.
    let f = fixture(NOW, true);
    let call = Call::new("/v1/responses", b"{\"a\":1}");
    let challenged = challenge(&f, &call, NOW);
    let x402 = Call::new("/v1/responses", b"{\"a\":1}")
        .with(PAYMENT_SIGNATURE, signature(&f, &challenged));
    let settled = f
        .router
        .settle(&x402.bound(), QUOTE, "test", NOW)
        .unwrap()
        .unwrap();
    assert_eq!(settled.method, Method::X402);
    assert!(settled.headers.iter().any(|(n, _)| n == PAYMENT_RESPONSE));
    assert_eq!(settled.amount, "21000");
    let mpp = Call::new("/v1/responses", b"{\"a\":1}")
        .with("Authorization", authorization(&f, &challenged));
    let refused = f
        .router
        .settle(&mpp.bound(), QUOTE, "test", NOW)
        .unwrap()
        .unwrap_err();
    assert_eq!(refused.method, Method::Mpp);
    assert_eq!(refused.reason, "duplicate_settlement");

    // And the other way round.
    let f = fixture(NOW, true);
    let challenged = challenge(&f, &call, NOW);
    let mpp = Call::new("/v1/responses", b"{\"a\":1}")
        .with("Authorization", authorization(&f, &challenged));
    let settled = f
        .router
        .settle(&mpp.bound(), QUOTE, "test", NOW)
        .unwrap()
        .unwrap();
    assert_eq!(settled.method, Method::Mpp);
    assert!(settled.headers.iter().any(|(n, _)| n == PAYMENT_RECEIPT));
    let x402 = Call::new("/v1/responses", b"{\"a\":1}")
        .with(PAYMENT_SIGNATURE, signature(&f, &challenged));
    let refused = f
        .router
        .settle(&x402.bound(), QUOTE, "test", NOW)
        .unwrap()
        .unwrap_err();
    assert_eq!(refused.reason, "duplicate_settlement");
    assert!(refused.headers.iter().any(|(n, _)| n == PAYMENT_RESPONSE));
    // A replay of the same MPP credential is refused too.
    let refused = f
        .router
        .settle(&mpp.bound(), QUOTE, "test", NOW)
        .unwrap()
        .unwrap_err();
    assert_eq!(refused.reason, "duplicate_settlement");
}

#[test]
fn a_credential_pays_only_its_own_request() {
    let f = fixture(NOW, true);
    let call = Call::new("/v1/responses", b"{\"n\":1}");
    let challenged = challenge(&f, &call, NOW);
    let other = Call::new("/v1/responses", b"{\"n\":2}")
        .with("Authorization", authorization(&f, &challenged));
    let refused = f
        .router
        .settle(&other.bound(), QUOTE, "test", NOW)
        .unwrap()
        .unwrap_err();
    assert_eq!(refused.reason, "digest_mismatch");
    let other = Call::new("/v1/chat/completions", b"{\"n\":1}")
        .with(PAYMENT_SIGNATURE, signature(&f, &challenged));
    let refused = f
        .router
        .settle(&other.bound(), QUOTE, "test", NOW)
        .unwrap()
        .unwrap_err();
    assert_ne!(refused.reason, "duplicate_settlement");
    // A different price is refused.
    let dearer = Quote {
        amount_msat: 22_000,
        usd_micros: 22_000,
    };
    let same = Call::new("/v1/responses", b"{\"n\":1}")
        .with("Authorization", authorization(&f, &challenged));
    let refused = f
        .router
        .settle(&same.bound(), dearer, "test", NOW)
        .unwrap()
        .unwrap_err();
    assert_eq!(refused.reason, "invalid_exact_lnbtc_amount_mismatch");
    // None of that consumed the payment.
    assert!(
        f.router
            .settle(&same.bound(), QUOTE, "test", NOW)
            .unwrap()
            .is_ok()
    );
    // No credential, nothing to settle.
    assert!(f.router.settle(&call.bound(), QUOTE, "t", NOW).is_none());
    // A Payment credential is not read when MPP is off.
    let only = fixture(NOW, false);
    assert!(only.router.settle(&same.bound(), QUOTE, "t", NOW).is_none());
}

#[test]
fn reserved_methods_have_no_adapter_and_a_method_is_live_once() {
    let dir = tempfile::tempdir().unwrap();
    let lightning = || {
        Lightning::new(
            Arc::new(TestNode::new(NOW)),
            Facilitator::new(FileReplayStore::open(dir.path()).unwrap(), 60),
            nostr::x402::MAINNET,
            300,
        )
    };
    assert!(Router::new(lightning(), vec![]).is_err());
    assert!(
        Router::new(
            lightning(),
            vec![Box::new(X402Lightning), Box::new(X402Lightning)]
        )
        .is_err()
    );
    assert_eq!(Method::L402.id(), "l402");
    assert!(MppLightning::new("r", vec![1; 31]).is_err());
}

/// lnget's `mpp` package built `authorization.txt` from the challenge the
/// x402 front issued for one fixed call (see `tests/front.rs`,
/// `lnget_built_credential_is_accepted`). The router the gateway uses
/// accepts the same credential and answers the same `Payment-Receipt`.
#[test]
fn lnget_built_credential_is_accepted_by_the_router() {
    const AT: u64 = 4_000_000_000;
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lnget");
    let read = |name: &str| {
        std::fs::read_to_string(dir.join(name))
            .unwrap()
            .trim()
            .to_string()
    };
    let f = fixture(AT, true);
    let body = br#"{"message":"What is new in the Gym?"}"#;
    let call = Call::new("/v1/messages", body).with("Authorization", read("authorization.txt"));
    let settled = f
        .router
        .settle(&call.bound(), QUOTE, "lnget", AT)
        .unwrap()
        .unwrap();
    assert_eq!(settled.method, Method::Mpp);
    let receipt = settled
        .headers
        .iter()
        .find(|(n, _)| n == PAYMENT_RECEIPT)
        .map(|(_, v)| v.as_str());
    assert_eq!(receipt, Some(read("receipt.txt").as_str()));
    // The challenge the router issues for that call is the front's, byte for byte.
    let issued = challenge(&f, &Call::new("/v1/messages", body), AT);
    let ours = params(header(&issued, WWW_AUTHENTICATE).unwrap());
    let theirs = params(&read("challenge.txt"));
    for name in ["realm", "method", "intent", "digest", "opaque"] {
        assert_eq!(ours[name], theirs[name], "{name}");
    }
}
