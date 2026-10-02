//! One paid `http:1` resource: challenge, reconstruct, settle, then execute.
//!
//! `Resource::handle` is transport-free so it can be tested without a socket;
//! `serve` is the smallest HTTP/1.1 loop that carries it. Every request is
//! bound from the actual method, the configured public URL, and the body
//! bytes; no header is bound, so the resource must not read any header that
//! changes what it does.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use nostr::x402::{PaymentRequirements, binding_hash, http_binding};
use serde_json::{Map, Value, json};

use crate::facilitator::Facilitator;
use crate::replay::ReplayStore;
use crate::wire::{
    PAYMENT_REQUIRED, PAYMENT_RESPONSE, PAYMENT_SIGNATURE, PaymentRequired, ResourceInfo,
    SettlementResponse, decode_payment_payload, encode_header,
};

/// Issues exact invoices on the key that is `payTo`.
pub trait Receiver: Send + Sync {
    fn pay_to(&self) -> String;
    fn invoice(
        &self,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<String, String>;
    /// What the node actually received for `payment_hash` (an LSP's
    /// just-in-time fee makes it less than the invoice), when the wallet has
    /// the inbound payment as succeeded; `None` when it cannot tell.
    fn received_msat(&self, payment_hash: [u8; 32]) -> Result<Option<u64>, String> {
        let _ = payment_hash;
        Ok(None)
    }
}

/// Runs the purchased operation. `Ok` bytes become the 200 body.
pub trait Executor: Send + Sync {
    fn execute(&self, body: &[u8]) -> Result<Vec<u8>, String>;
}

impl<F: Fn(&[u8]) -> Result<Vec<u8>, String> + Send + Sync> Executor for F {
    fn execute(&self, body: &[u8]) -> Result<Vec<u8>, String> {
        self(body)
    }
}

pub struct Request {
    pub method: String,
    /// Path and query as received, for example `/run?x=1`.
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn json(status: u16, value: &Value) -> Self {
        Self {
            status,
            headers: vec![("content-type".into(), "application/json".into())],
            body: value.to_string().into_bytes(),
        }
    }
}

/// What one request did, for the operator's log. Never carries a preimage
/// or an invoice.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Event {
    pub method: String,
    pub target: String,
    pub request_hash: Option<String>,
    pub status: u16,
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_reason: Option<String>,
}

pub struct Resource<S: ReplayStore> {
    pub url: String,
    pub network: &'static str,
    pub amount_msat: u64,
    pub timeout_secs: u32,
    pub description: String,
    pub mime_type: String,
    pub receiver: Arc<dyn Receiver>,
    pub executor: Arc<dyn Executor>,
    pub facilitator: Facilitator<S>,
}

impl<S: ReplayStore> Resource<S> {
    fn path_and_query(&self) -> &str {
        let rest = self
            .url
            .split_once("://")
            .map(|(_, rest)| rest)
            .unwrap_or(&self.url);
        match rest.find('/') {
            Some(index) => &rest[index..],
            None => "/",
        }
    }

    fn requirements(&self, request_hash: &str, invoice: &str) -> PaymentRequirements {
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
            amount: self.amount_msat.to_string(),
            asset: "BTC".into(),
            pay_to: self.receiver.pay_to(),
            max_timeout_seconds: u64::from(self.timeout_secs),
            extra,
        }
    }

    fn resource_info(&self) -> ResourceInfo {
        ResourceInfo {
            url: self.url.clone(),
            description: Some(self.description.clone()),
            mime_type: Some(self.mime_type.clone()),
            rest: Map::new(),
        }
    }

    fn refuse(&self, request: &Request, hash: Option<String>, reason: &str) -> (Response, Event) {
        let settlement = SettlementResponse::failed(self.network, reason);
        let mut response = Response::json(402, &json!({"error": reason, "x402Version": 2}));
        if let Ok(header) = encode_header(&settlement) {
            response.headers.push((PAYMENT_RESPONSE.into(), header));
        }
        let event = Event {
            method: request.method.clone(),
            target: request.target.clone(),
            request_hash: hash,
            status: 402,
            outcome: "refused".into(),
            payment_hash: None,
            error_reason: Some(reason.to_string()),
        };
        (response, event)
    }

    /// Answer one request at time `now`.
    pub fn handle(&self, request: &Request, now: u64) -> (Response, Event) {
        let event = |status: u16, outcome: &str, hash: Option<String>| Event {
            method: request.method.clone(),
            target: request.target.clone(),
            request_hash: hash,
            status,
            outcome: outcome.into(),
            payment_hash: None,
            error_reason: None,
        };
        if request.target != self.path_and_query() {
            return (
                Response::json(404, &json!({"error": "not the paid resource"})),
                event(404, "not_found", None),
            );
        }
        let binding = match http_binding(&request.method, &self.url, &request.body, &[]) {
            Ok(binding) => binding,
            Err(_) => {
                return (
                    Response::json(400, &json!({"error": "request cannot be bound"})),
                    event(400, "unbound", None),
                );
            }
        };
        let request_hash = match binding_hash(&binding) {
            Ok(hash) => hash,
            Err(_) => {
                return (
                    Response::json(400, &json!({"error": "request cannot be bound"})),
                    event(400, "unbound", None),
                );
            }
        };

        let Some(signature) = request.header(PAYMENT_SIGNATURE) else {
            let mut digest = [0u8; 32];
            if hex::decode_to_slice(&request_hash, &mut digest).is_err() {
                return (
                    Response::json(500, &json!({"error": "binding digest"})),
                    event(500, "binding_digest", Some(request_hash)),
                );
            }
            let invoice = match self
                .receiver
                .invoice(self.amount_msat, digest, self.timeout_secs)
            {
                Ok(invoice) => invoice,
                Err(_) => {
                    return (
                        Response::json(
                            503,
                            &json!({"error": "exact_lnbtc_invoice_issuance_denied"}),
                        ),
                        event(503, "issuance_denied", Some(request_hash)),
                    );
                }
            };
            let required = PaymentRequired {
                x402_version: 2,
                error: Some("PAYMENT-SIGNATURE header is required".into()),
                resource: self.resource_info(),
                accepts: vec![self.requirements(&request_hash, &invoice)],
                extensions: None,
            };
            let Ok(header) = encode_header(&required) else {
                return (
                    Response::json(500, &json!({"error": "challenge encoding"})),
                    event(500, "challenge_encoding", Some(request_hash)),
                );
            };
            let mut response =
                Response::json(402, &json!({"error": "payment required", "x402Version": 2}));
            response.headers.push((PAYMENT_REQUIRED.into(), header));
            return (response, event(402, "challenged", Some(request_hash)));
        };

        let payload = match decode_payment_payload(signature) {
            Ok(payload) => payload,
            Err(_) => return self.refuse(request, Some(request_hash), "invalid_payment_payload"),
        };
        let Some(accepted_invoice) = payload
            .accepted
            .extra
            .get("invoice")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            return self.refuse(
                request,
                Some(request_hash),
                "invalid_exact_lnbtc_invoice_missing",
            );
        };
        let requirements = self.requirements(&request_hash, accepted_invoice);
        let admitted = match self
            .facilitator
            .settle(&requirements, &payload, &request_hash, now)
        {
            Ok(admitted) => admitted,
            Err(settlement) => {
                let reason = settlement
                    .error_reason
                    .clone()
                    .unwrap_or_else(|| "settlement_failed".into());
                return self.refuse(request, Some(request_hash), &reason);
            }
        };
        let payment_hash = admitted.proof.payment_hash.clone();
        let settlement_header = encode_header(&admitted.response).unwrap_or_default();
        match self.executor.execute(&request.body) {
            Ok(body) => {
                let mut response = Response {
                    status: 200,
                    headers: vec![("content-type".into(), self.mime_type.clone())],
                    body,
                };
                response
                    .headers
                    .push((PAYMENT_RESPONSE.into(), settlement_header));
                let mut event = event(200, "executed", Some(request_hash));
                event.payment_hash = Some(payment_hash);
                (response, event)
            }
            Err(message) => {
                // The claim is consumed; the buyer holds a settlement that
                // bought a failed execution. Say so, do not pretend otherwise.
                let mut response = Response::json(
                    500,
                    &json!({"error": "execution failed after settlement", "detail": message}),
                );
                response
                    .headers
                    .push((PAYMENT_RESPONSE.into(), settlement_header));
                let mut event = event(500, "execution_failed", Some(request_hash));
                event.payment_hash = Some(payment_hash);
                event.error_reason = Some(message);
                (response, event)
            }
        }
    }
}

const MAX_BODY: usize = 8 * 1024 * 1024;

fn read_request(stream: &mut TcpStream) -> Result<Request, String> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    let mut parts = line.trim_end().split(' ');
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("").to_string();
    if method.is_empty() || !target.starts_with('/') {
        return Err("bad request line".into());
    }
    let mut headers = Vec::new();
    let mut length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').ok_or("bad header")?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            length = value.parse().map_err(|_| "bad content-length")?;
            if length > MAX_BODY {
                return Err("body too large".into());
            }
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err("chunked bodies are not accepted".into());
        }
        headers.push((name.to_ascii_lowercase(), value.to_string()));
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).map_err(|e| e.to_string())?;
    Ok(Request {
        method,
        target,
        headers,
        body,
    })
}

fn write_response(stream: &mut TcpStream, response: &Response) {
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        402 => "Payment Required",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "",
    };
    let mut head = format!("HTTP/1.1 {} {reason}\r\n", response.status);
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!(
        "content-length: {}\r\nconnection: close\r\n\r\n",
        response.body.len()
    ));
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&response.body);
    let _ = stream.flush();
}

/// Serve `resource` on `listener` until `stop` is set, calling `log` for
/// each request. Connections are handled one at a time per thread; the
/// replay store keeps concurrent settlements honest.
pub fn serve<S: ReplayStore + Send + Sync + 'static>(
    listener: TcpListener,
    resource: Arc<Resource<S>>,
    stop: Arc<AtomicBool>,
    log: impl Fn(&Event) + Send + Sync + 'static,
) -> std::io::Result<()> {
    serve_with(listener, stop, move |request| {
        let (response, event) = resource.handle(request, crate::unix_now());
        log(&event);
        response
    })
}

/// The HTTP/1.1 loop under [`serve`] and the multi-route front: one thread
/// per connection, one request per connection, `handle` answers it.
pub fn serve_with(
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    handle: impl Fn(&Request) -> Response + Send + Sync + 'static,
) -> std::io::Result<()> {
    listener.set_nonblocking(true)?;
    let handle = Arc::new(handle);
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let handle = handle.clone();
                std::thread::spawn(move || {
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
                    match read_request(&mut stream) {
                        Ok(request) => write_response(&mut stream, &handle(&request)),
                        Err(message) => write_response(
                            &mut stream,
                            &Response::json(400, &json!({"error": message})),
                        ),
                    }
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::FileReplayStore;
    use crate::wire::{PaymentPayload, decode_payment_required};

    const INVOICE: &str = "lnbc250n1pj48ugqpp54y3u9s8ylemsv8l3ewyzzu0klhujvuvmkl6llchq23vy8rzjsf0qsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygshp5p4nz8am4uqj4q8a87z3sk4x6yk4dv2mvel34epw68qqkwy0xcqvqxqzfvcqpjr4rx6ls6j5rpwknuea64evlk7yfx56wmqcer5eerekdsn9tlv6v4ex9mlz5dtm9qapl3svwlqcf7837dmjkru9z9w4h2rvm0md52w2sqxrwu5f";
    const PREIMAGE: &str = "0001020304050607080900010203040506070809000102030405060708090102";
    const PAYEE: &str = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    struct Pinned;
    impl Receiver for Pinned {
        fn pay_to(&self) -> String {
            PAYEE.into()
        }
        fn invoice(&self, _: u64, _: [u8; 32], _: u32) -> Result<String, String> {
            Ok(INVOICE.into())
        }
    }

    fn resource() -> (Resource<FileReplayStore>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "x402-server-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let resource = Resource {
            url: "https://example.com/run?x=1".into(),
            network: nostr::x402::MAINNET,
            amount_msat: 25000,
            timeout_secs: 300,
            description: "echo".into(),
            mime_type: "application/octet-stream".into(),
            receiver: Arc::new(Pinned),
            executor: Arc::new(|body: &[u8]| Ok(body.to_vec())),
            facilitator: Facilitator::new(FileReplayStore::open(&dir).unwrap(), 60),
        };
        (resource, dir)
    }

    fn request(target: &str, headers: Vec<(String, String)>) -> Request {
        Request {
            method: "POST".into(),
            target: target.into(),
            headers,
            body: b"hello".to_vec(),
        }
    }

    #[test]
    fn challenges_with_bound_terms_and_refuses_a_foreign_proof() {
        let (resource, dir) = resource();
        let (other, _) = resource.handle(&request("/other", vec![]), 1_700_000_000);
        assert_eq!(other.status, 404);

        let (response, event) = resource.handle(&request("/run?x=1", vec![]), 1_700_000_000);
        assert_eq!(response.status, 402);
        assert_eq!(event.outcome, "challenged");
        let header = response
            .headers
            .iter()
            .find(|(n, _)| n == PAYMENT_REQUIRED)
            .map(|(_, v)| v.clone())
            .unwrap();
        let required = decode_payment_required(&header).unwrap();
        let terms = &required.accepts[0];
        assert_eq!(terms.pay_to, PAYEE);
        assert_eq!(terms.amount, "25000");
        assert_eq!(
            terms.extra["requestHash"],
            json!(event.request_hash.unwrap())
        );
        assert_eq!(terms.extra["requestBindingProfile"], json!("http:1"));

        // The pinned invoice is signed over a different request hash, so the
        // reconstruction catches the substitution even though the payload is
        // internally consistent.
        let mut payload = Map::new();
        payload.insert("preimage".into(), json!(PREIMAGE));
        let signature = encode_header(&PaymentPayload {
            x402_version: 2,
            resource: None,
            accepted: terms.clone(),
            payload,
            extensions: None,
        })
        .unwrap();
        let (paid, event) = resource.handle(
            &request("/run?x=1", vec![(PAYMENT_SIGNATURE.into(), signature)]),
            1_700_000_000,
        );
        assert_eq!(paid.status, 402);
        assert_eq!(
            event.error_reason.as_deref(),
            Some("invalid_exact_lnbtc_invoice_request_mismatch")
        );
        assert!(paid.headers.iter().any(|(n, _)| n == PAYMENT_RESPONSE));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
