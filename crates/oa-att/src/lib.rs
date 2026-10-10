//! NIP-ATT client verification (`nips/openagents/NIP-ATT.md`, "Client
//! verification"), split into the steps a person can watch:
//!
//! 1. [`parse`]: the release, head and endpoint events verify, are signed
//!    by the expected publisher and endpoint key, point at each other, and
//!    the endpoint is current.
//! 2. [`chain`]: the endpoint's Google Confidential Space token verifies up
//!    to Google's pinned root ([`token`]).
//! 3. [`measure`]: the head admits the release, and the token's image
//!    digest, hardware model and support level match it.
//! 4. [`bind`]: the binding of the endpoint key and the release is in the
//!    token's nonces, so the key was made inside that machine.
//! 5. [`check_answer`], after the sealed job: the result is signed by the
//!    endpoint key, its receipt seals, and its attestation block names the
//!    request ciphertext, the release, the measurement and the model.
//!
//! Every step is pure (the caller passes the clock) and builds for
//! `wasm32-unknown-unknown`, so the web page runs the same code the gateway
//! does. [`Tamper`] changes one input on purpose for the live negative test.

#[cfg(feature = "net")]
pub mod net;
pub mod token;

use nostr::att::{
    self, ENDPOINT_KIND, EndpointRecord, Head, Level, Release, TOKEN_AUDIENCE,
};
use nostr::domain::Event;
use nostr::pylon::Beacon;
use serde::Serialize;
use serde_json::Value;

pub use nostr;
pub use token::{Claims, Refused};

/// The workload slug of the sealed Clef decision service.
pub const WORKLOAD: &str = "clef-decisions";
/// The platform kind this verifier admits.
pub const PLATFORM: &str = "gcp-confidential-space";

/// Whose releases the client trusts, and for which workload.
#[derive(Debug, Clone)]
pub struct Policy {
    /// The publisher key (hex) that signs releases and heads.
    pub publisher: String,
    /// The workload slug.
    pub workload: String,
    /// The lowest level the data may go to.
    pub required: Level,
    /// The highest head generation already seen, for the rollback check.
    pub seen_generation: Option<u64>,
}

/// The records fetched from the relay.
#[derive(Debug, Clone)]
pub struct Records {
    pub release: Event,
    pub head: Event,
    pub endpoint: Event,
    /// The provider's NIP-PYLON beacon, when it has one (a claim only).
    pub beacon: Option<Event>,
}

/// The records after [`parse`].
#[derive(Debug, Clone)]
pub struct Parsed {
    pub release_id: String,
    pub release: Release,
    pub head: Head,
    pub endpoint: EndpointRecord,
    pub beacon: Option<Beacon>,
}

/// A deliberate change for the live negative test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tamper {
    None,
    /// Change one character of the logged measurement before comparing.
    Measurement,
    /// Swap in a key that the hardware never bound.
    UnboundKey,
}

/// What [`measure`] compared.
#[derive(Debug, Clone, Serialize)]
pub struct Measured {
    /// The image digest the release logs (changed when tampered).
    pub expected: String,
    /// The image digest the hardware reported.
    pub reported: String,
    pub hwmodel: String,
    pub support: Vec<String>,
    /// Why the head admits the release.
    pub admitted: String,
}

/// What [`bind`] found.
#[derive(Debug, Clone, Serialize)]
pub struct Bound {
    /// The key checked (the swapped one when tampered).
    pub key: String,
    /// The binding recomputed from that key and the release.
    pub binding: String,
    /// The token's nonces.
    pub nonces: Vec<String>,
    /// The level computed from the evidence.
    pub level: Level,
}

/// What [`check_answer`] found.
#[derive(Debug, Clone, Serialize)]
pub struct Checked {
    pub receipt_digest: String,
    pub request_ciphertext_digest: String,
    pub measurement: String,
    pub model: String,
    pub model_digest: String,
    pub level: String,
}

fn refuse<T>(reason: impl Into<String>) -> Result<T, Refused> {
    Err(Refused(reason.into()))
}

/// Step 1: verify the three records and how they point at each other.
///
/// # Errors
///
/// The first record that does not verify, in plain words.
pub fn parse(records: &Records, policy: &Policy, now: u64) -> Result<Parsed, Refused> {
    let release = att::parse_release(&records.release)
        .map_err(|e| Refused(format!("the release record is invalid: {e}")))?;
    if records.release.pubkey != policy.publisher {
        return refuse("the release is signed by another publisher than the one this page trusts");
    }
    let head = att::parse_head(&records.head)
        .map_err(|e| Refused(format!("the release head is invalid: {e}")))?;
    if records.head.pubkey != policy.publisher {
        return refuse("the release head is signed by another publisher");
    }
    if head.workload != policy.workload || release.workload != policy.workload {
        return refuse("the records are for another workload");
    }
    att::check_generation(policy.seen_generation, &head).map_err(Refused)?;
    let endpoint = att::parse_endpoint(&records.endpoint)
        .map_err(|e| Refused(format!("the endpoint record is invalid: {e}")))?;
    if endpoint.head != head.address(&policy.publisher) {
        return refuse("the endpoint names another release head");
    }
    if endpoint.body.release != records.release.id {
        return refuse("the endpoint runs another release than the one fetched");
    }
    endpoint.body.current(now).map_err(Refused)?;
    let beacon = records
        .beacon
        .as_ref()
        .and_then(|event| nostr::pylon::parse_beacon(event).ok())
        .filter(|beacon| beacon.provider == endpoint.body.endpoint);
    Ok(Parsed {
        release_id: records.release.id.clone(),
        release,
        head,
        endpoint,
        beacon,
    })
}

/// Step 2: verify the endpoint's Confidential Space token up to Google's
/// root, and that the endpoint does not outlive it.
///
/// # Errors
///
/// When the endpoint has no such token or it does not verify.
pub fn chain(parsed: &Parsed, now: u64) -> Result<Claims, Refused> {
    let evidence = parsed
        .endpoint
        .body
        .evidence
        .iter()
        .find(|e| e.kind == "gcp-confidential-space-token" && e.format == "pki")
        .ok_or_else(|| Refused("the endpoint carries no Confidential Space token".into()))?;
    let claims = token::verify(&evidence.token, TOKEN_AUDIENCE, now)?;
    if parsed.endpoint.body.valid_until > claims.exp {
        return refuse("the endpoint claims to be valid after its evidence expires");
    }
    Ok(claims)
}

/// Step 3: the head admits the release, and the hardware ran exactly that
/// image on the release's platform.
///
/// # Errors
///
/// When the release is not admitted or the measurement differs.
pub fn measure(parsed: &Parsed, claims: &Claims, now: u64, tamper: Tamper) -> Result<Measured, Refused> {
    parsed
        .head
        .admits(&parsed.release_id, &parsed.release, now)
        .map_err(|e| Refused(format!("the release head does not admit this release: {e}")))?;
    let platform = parsed
        .release
        .platform(PLATFORM)
        .ok_or_else(|| Refused("the release is not admitted on Confidential Space".into()))?;
    let mut expected = parsed.release.image.digest.clone();
    if tamper == Tamper::Measurement {
        expected = flip_last_hex(&expected);
    }
    let measured = Measured {
        expected: expected.clone(),
        reported: claims.image_digest.clone(),
        hwmodel: claims.hwmodel.clone(),
        support: claims.support.clone(),
        admitted: format!(
            "listed in head generation {}, notice {} s, published {}",
            parsed.head.generation, parsed.head.notice_seconds, parsed.release.published_at
        ),
    };
    if claims.hwmodel != platform.hwmodel {
        return refuse(format!(
            "the hardware is {}, but the release needs {}",
            claims.hwmodel, platform.hwmodel
        ));
    }
    if !claims.support.iter().any(|s| *s == platform.support) {
        return refuse(format!(
            "the launcher image is not {} (it is {:?})",
            platform.support, claims.support
        ));
    }
    if claims.image_digest != expected {
        return Err(Refused(format!(
            "the program's fingerprint {} is not the logged release {}",
            claims.image_digest, expected
        )));
    }
    Ok(measured)
}

/// Step 4: the binding of the endpoint key and the release is one of the
/// token's nonces, so the key was made inside the attested machine.
///
/// # Errors
///
/// When the binding is absent, or the level is below the policy's.
pub fn bind(parsed: &Parsed, claims: &Claims, policy: &Policy, swapped: Option<&str>) -> Result<Bound, Refused> {
    let key = swapped.unwrap_or(&parsed.endpoint.body.endpoint).to_string();
    let hpke = parsed
        .endpoint
        .body
        .hpke
        .as_ref()
        .map(|_| ())
        .map_or(Ok(None), |()| refuse("an HPKE key is not used by this client"))?;
    let binding = att::binding(&key, hpke, &parsed.release_id).map_err(Refused)?;
    let level = level_of(claims);
    let bound = Bound {
        key: key.clone(),
        binding: binding.clone(),
        nonces: claims.nonces.clone(),
        level,
    };
    if binding != parsed.endpoint.body.binding || !claims.nonces.iter().any(|n| *n == binding) {
        return refuse(format!(
            "the key {} is not bound to the attested machine: its binding {} is not in the hardware's nonces",
            short(&key),
            short(&binding)
        ));
    }
    if level < policy.required {
        return refuse(format!(
            "the endpoint's level is {}, below the {} this data needs",
            level.as_str(),
            policy.required.as_str()
        ));
    }
    Ok(bound)
}

/// The level a verified Confidential Space token supports: `tee-cloud` on
/// TDX or SEV-SNP in Google's data center.
#[must_use]
pub fn level_of(claims: &Claims) -> Level {
    match claims.hwmodel.as_str() {
        "GCP_INTEL_TDX" | "GCP_AMD_SEV_SNP" if claims.swname == "CONFIDENTIAL_SPACE" => Level::TeeCloud,
        _ => Level::Open,
    }
}

pub use nostr::att::{attested_block, response_digest};

/// Step 5: check a decrypted sealed result. `result` is the `26910` event
/// (its signature and signer are checked here), `payload` its decrypted
/// content, `request` the `25910` event this client sent, and
/// `request_digest` the request body's digest.
///
/// # Errors
///
/// When any binding of the answer to the request or the release fails.
pub fn check_answer(
    parsed: &Parsed,
    measurement: &str,
    result: &Event,
    payload: &Value,
    request: &Event,
    request_digest: &str,
) -> Result<Checked, Refused> {
    result
        .validate_crypto()
        .map_err(|_| Refused("the answer's signature does not verify".into()))?;
    if result.pubkey != parsed.endpoint.body.endpoint {
        return refuse("the answer is not signed by the attested endpoint key");
    }
    if !result.tag_values("e").any(|id| id == request.id) {
        return refuse("the answer names another request");
    }
    let receipt: receipts::ExecutionReceipt = serde_json::from_value(payload["receipt"].clone())
        .map_err(|e| Refused(format!("the receipt does not parse: {e}")))?;
    receipt
        .verify()
        .map_err(|e| Refused(format!("the receipt's seal is broken: {e}")))?;
    if receipt.attempt_id != request.id || receipt.request_digest != request_digest {
        return refuse("the receipt is for another request");
    }
    let response = &payload["response"];
    if receipt.result_digest.as_deref() != Some(response_digest(response).as_str()) {
        return refuse("the receipt does not cover this answer");
    }
    let block = &response["attested"];
    let field = |key: &str| block[key].as_str().unwrap_or_default().to_string();
    if field("endpoint") != parsed.endpoint.address() {
        return refuse("the answer names another endpoint");
    }
    if field("release") != parsed.release_id {
        return refuse("the answer names another release");
    }
    if field("measurement") != measurement {
        return refuse("the answer names another measurement");
    }
    if field("request_ciphertext_digest") != att::sha256_hex(request.content.as_bytes()) {
        return refuse("the answer names another request ciphertext");
    }
    let model = field("model");
    let model_digest = field("model_digest");
    if !parsed
        .release
        .models
        .iter()
        .any(|m| m.id == model && m.digest == model_digest)
    {
        return refuse("the answer names a model the release does not pin");
    }
    if receipt.served.artifact_signature != model_digest {
        return refuse("the receipt names another model artifact");
    }
    Ok(Checked {
        receipt_digest: receipt.digest.clone(),
        request_ciphertext_digest: field("request_ciphertext_digest"),
        measurement: field("measurement"),
        model,
        model_digest,
        level: field("level"),
    })
}

/// The endpoint's address, `30203:<key>:<instance>`.
#[must_use]
pub fn endpoint_address(record: &EndpointRecord) -> String {
    format!("{ENDPOINT_KIND}:{}:{}", record.body.endpoint, record.instance)
}

fn flip_last_hex(digest: &str) -> String {
    let mut chars: Vec<char> = digest.chars().collect();
    if let Some(last) = chars.last_mut() {
        *last = if *last == '0' { '1' } else { '0' };
    }
    chars.into_iter().collect()
}

fn short(hex: &str) -> String {
    if hex.len() > 16 {
        format!("{}…{}", &hex[..8], &hex[hex.len() - 8..])
    } else {
        hex.to_string()
    }
}

/// A sealed decision request to the verified endpoint: one yes-or-no
/// question about `state`, NIP-44 encrypted to the endpoint key, with the
/// `openagents.attested.v1` fields naming the endpoint, release and level
/// the client verified. `nonce` is a fresh 32 random bytes and `request`
/// a fresh logical ID. Returns the event and the body (its digest is what
/// the receipt must name).
///
/// # Errors
///
/// When the body is out of bounds.
#[allow(clippy::too_many_arguments)]
pub fn sealed_request(
    client: &nostr::domain::RelaySigner,
    client_secret: &secp256k1::SecretKey,
    parsed: &Parsed,
    level: Level,
    state: &str,
    question: &str,
    request: &str,
    nonce: [u8; 32],
    now: u64,
) -> Result<(Event, nostr::decision::RequestBody), Refused> {
    use nostr::decision::{REQUEST_KIND, RequestBody, Seal};
    use nostr::domain::Tag;
    let model = parsed
        .release
        .models
        .first()
        .map(|m| m.id.clone())
        .ok_or_else(|| Refused("the release pins no model".into()))?;
    let mut questions = serde_json::Map::new();
    questions.insert(
        "answer".into(),
        serde_json::json!({"type": "noul", "instructions": question}),
    );
    let body = RequestBody::new(request, 1, model, Value::String(state.to_string()), questions)
        .deadline(now + 180);
    body.validate()
        .map_err(|e| Refused(format!("the request is out of bounds: {e}")))?;
    let mut payload = body.payload();
    let (requires, attested) =
        att::sealed_fields(&parsed.endpoint.address(), &parsed.release_id, level);
    payload["requires"] = requires;
    payload["attested"] = attested;
    let peer = endpoint_key(&parsed.endpoint.body.endpoint)?;
    let seal = Seal {
        signer: client,
        conversation: nostr::nip44::conversation_key(client_secret, &peer),
        nonce,
        created_at: now,
    };
    let tags = vec![
        Tag::new(vec!["p".into(), parsed.endpoint.body.endpoint.clone()]),
        Tag::new(vec!["expiration".into(), (now + 180).to_string()]),
    ];
    let event = seal
        .event(REQUEST_KIND, tags, &payload)
        .map_err(|e| Refused(format!("the request does not seal: {e}")))?;
    Ok((event, body))
}

fn endpoint_key(hex: &str) -> Result<secp256k1::XOnlyPublicKey, Refused> {
    hex.parse::<secp256k1::XOnlyPublicKey>()
        .map_err(|_| Refused("the endpoint key is not a valid key".into()))
}

/// What one decrypted answer event says.
#[derive(Debug, Clone)]
pub enum Opened {
    /// A `27010` status: `processing`, or a terminal refusal with its code
    /// and message.
    Status { word: String, refusal: Option<(String, String)> },
    /// The `26910` result: the payload to pass to [`check_answer`].
    Result(Value),
}

/// Decrypt and bind one answer event to the request this client sent.
///
/// # Errors
///
/// When it is not an answer to that request from that endpoint.
pub fn open_answer(
    event: &Event,
    request: &Event,
    body: &nostr::decision::RequestBody,
    client_secret: &secp256k1::SecretKey,
    client_pubkey: &str,
) -> Result<Opened, Refused> {
    use nostr::decision::{Answer, Pending, bind_answer};
    let worker = request.tag_values("p").next().unwrap_or_default().to_string();
    let pending = Pending {
        attempt_id: &request.id,
        worker: &worker,
        customer: client_pubkey,
        request: &body.request,
        attempt: body.attempt,
        request_digest: body.digest(),
    };
    match bind_answer(event, &pending, client_secret)
        .map_err(|e| Refused(format!("the answer does not bind to this request: {e}")))?
    {
        Answer::Status(status) => Ok(Opened::Status {
            word: status.status.as_str().to_string(),
            refusal: status.refusal.map(|r| (r.code, r.message.unwrap_or_default())),
        }),
        Answer::Result(result) => Ok(Opened::Result(serde_json::json!({
            "outcome": result.outcome.as_str(),
            "response": result.response,
            "receipt": result.receipt,
            "error": result.refusal.map(|r| serde_json::json!({"code": r.code, "message": r.message})),
        }))),
    }
}
