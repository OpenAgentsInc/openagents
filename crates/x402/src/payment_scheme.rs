//! The HTTP `Payment` authentication scheme
//! ([draft-httpauth-payment-01](https://paymentauth.org/draft-httpauth-payment-01.txt))
//! with the Lightning `charge` intent
//! ([draft-lightning-charge-00](https://paymentauth.org/draft-lightning-charge-00.txt)),
//! the scheme `lnget` and `mppx` speak.
//!
//! It is a second encoding of the x402 invoice, never a second invoice: the
//! challenge's `request.methodDetails.invoice` is the invoice in the x402
//! terms, whose description hash is the x402 request hash. The challenge id
//! is the draft's recommended stateless HMAC-SHA256 over
//! `realm|method|intent|request|expires|digest|opaque`; `digest` (the body's
//! SHA-256, RFC 9530) is always sent and `opaque` carries the method and URL,
//! so a credential bought for one call cannot pay for another. Consumption
//! goes through the same facilitator and replay store as x402, keyed by the
//! payment hash, so a preimage spent one way cannot be spent the other.

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

pub const WWW_AUTHENTICATE: &str = "www-authenticate";
pub const AUTHORIZATION: &str = "authorization";
pub const PAYMENT_RECEIPT: &str = "payment-receipt";
pub const SCHEME: &str = "Payment";
pub const METHOD: &str = "lightning";
pub const INTENT: &str = "charge";
const PROBLEMS: &str = "https://paymentauth.org/problems/";
const MAX_CREDENTIAL_BYTES: usize = 64 * 1024;

/// HMAC-SHA256 (RFC 2104) over `message` with `key`.
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut block = [0u8; BLOCK];
    if key.len() > BLOCK {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(block.map(|b| b ^ 0x36));
    inner.update(message);
    let inner = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(block.map(|b| b ^ 0x5c));
    outer.update(inner);
    outer.finalize().into()
}

/// JSON Canonicalization Scheme (RFC 8785) for the values this module
/// writes: objects with sorted keys, strings, and arrays, no whitespace.
pub fn jcs(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            let fields: Vec<String> = keys
                .into_iter()
                .map(|key| format!("{}:{}", Value::String(key.clone()), jcs(&map[key])))
                .collect();
            format!("{{{}}}", fields.join(","))
        }
        Value::Array(items) => format!("[{}]", items.iter().map(jcs).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
}

fn b64url_json(value: &Value) -> String {
    URL_SAFE_NO_PAD.encode(jcs(value))
}

/// The RFC 9530 `Content-Digest` value of `body`: `sha-256=:<base64>:`.
pub fn content_digest(body: &[u8]) -> String {
    format!("sha-256=:{}:", STANDARD.encode(Sha256::digest(body)))
}

/// Unix seconds as an RFC 3339 UTC timestamp, `YYYY-MM-DDTHH:MM:SSZ`.
pub fn rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Parse an RFC 3339 timestamp to Unix seconds. Fractional seconds are
/// truncated; a numeric offset is applied.
pub fn parse_rfc3339(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    if bytes.len() < 20 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[13] != b':' {
        return None;
    }
    if !matches!(bytes[10], b'T' | b't') || bytes[16] != b':' {
        return None;
    }
    let num = |range: std::ops::Range<usize>| -> Option<i64> {
        let part = text.get(range)?;
        part.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    let (year, month, day) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hour, minute, second) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }
    let mut rest = &text[19..];
    if let Some(fraction) = rest.strip_prefix('.') {
        let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        rest = &fraction[digits..];
    }
    let offset = match rest {
        "Z" | "z" => 0,
        _ if rest.len() == 6 && (rest.starts_with('+') || rest.starts_with('-')) => {
            let sign = if rest.starts_with('-') { -1 } else { 1 };
            let h: i64 = rest.get(1..3)?.parse().ok()?;
            let m: i64 = rest.get(4..6)?.parse().ok()?;
            sign * (h * 3600 + m * 60)
        }
        _ => return None,
    };
    // Howard Hinnant's days_from_civil.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + hour * 3600 + minute * 60 + second - offset;
    u64::try_from(secs).ok()
}

/// The Lightning network name the charge draft uses for an x402 network.
pub fn network_name(x402_network: &str) -> &'static str {
    if x402_network == nostr::x402::TESTNET {
        "testnet"
    } else {
        "mainnet"
    }
}

/// One `WWW-Authenticate: Payment` challenge, exactly as sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Challenge {
    pub id: String,
    pub realm: String,
    pub method: String,
    pub intent: String,
    pub request: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opaque: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

/// What the server fixes in a challenge it issues.
pub struct Terms<'a> {
    pub realm: &'a str,
    pub amount_sats: u64,
    pub invoice: &'a str,
    pub payment_hash: &'a str,
    /// The x402 network identifier of the invoice.
    pub network: &'a str,
    pub http_method: &'a str,
    pub url: &'a str,
    pub body: &'a [u8],
    pub expires_at: u64,
    pub description: Option<&'a str>,
}

fn binding_input(
    realm: &str,
    method: &str,
    intent: &str,
    request: &str,
    expires: Option<&str>,
    digest: Option<&str>,
    opaque: Option<&str>,
) -> String {
    [
        realm,
        method,
        intent,
        request,
        expires.unwrap_or(""),
        digest.unwrap_or(""),
        opaque.unwrap_or(""),
    ]
    .join("|")
}

/// The challenge id: base64url(HMAC-SHA256(key, binding input)).
pub fn challenge_id(key: &[u8], challenge: &Challenge) -> String {
    let input = binding_input(
        &challenge.realm,
        &challenge.method,
        &challenge.intent,
        &challenge.request,
        challenge.expires.as_deref(),
        challenge.digest.as_deref(),
        challenge.opaque.as_deref(),
    );
    URL_SAFE_NO_PAD.encode(hmac_sha256(key, input.as_bytes()))
}

fn opaque_for(http_method: &str, url: &str) -> String {
    b64url_json(&json!({"method": http_method, "url": url}))
}

impl Challenge {
    /// Issue a challenge for `terms`, bound with `key`.
    pub fn issue(key: &[u8], terms: &Terms<'_>) -> Self {
        let request = json!({
            "amount": terms.amount_sats.to_string(),
            "currency": "sat",
            "methodDetails": {
                "invoice": terms.invoice,
                "network": network_name(terms.network),
                "paymentHash": terms.payment_hash,
            },
        });
        let mut challenge = Self {
            id: String::new(),
            realm: terms.realm.to_string(),
            method: METHOD.into(),
            intent: INTENT.into(),
            request: b64url_json(&request),
            expires: Some(rfc3339(terms.expires_at)),
            description: terms.description.map(str::to_string),
            opaque: Some(opaque_for(terms.http_method, terms.url)),
            digest: Some(content_digest(terms.body)),
        };
        challenge.id = challenge_id(key, &challenge);
        challenge
    }

    /// The `WWW-Authenticate` value.
    pub fn header_value(&self) -> String {
        let quote = |value: &str| value.replace('\\', "\\\\").replace('"', "\\\"");
        let mut out = format!(
            "{SCHEME} id=\"{}\", realm=\"{}\", method=\"{}\", intent=\"{}\", request=\"{}\"",
            quote(&self.id),
            quote(&self.realm),
            quote(&self.method),
            quote(&self.intent),
            quote(&self.request)
        );
        for (name, value) in [
            ("expires", &self.expires),
            ("digest", &self.digest),
            ("opaque", &self.opaque),
            ("description", &self.description),
        ] {
            if let Some(value) = value {
                out.push_str(&format!(", {name}=\"{}\"", quote(value)));
            }
        }
        out
    }
}

/// The decoded `Authorization: Payment` credential.
#[derive(Debug, Clone, Deserialize)]
pub struct Credential {
    pub challenge: Challenge,
    pub payload: Map<String, Value>,
}

impl Credential {
    pub fn preimage(&self) -> Option<&str> {
        self.payload.get("preimage").and_then(Value::as_str)
    }
}

/// Whether an `Authorization` value uses the `Payment` scheme.
pub fn is_payment_authorization(value: &str) -> bool {
    value
        .trim_start()
        .get(..SCHEME.len() + 1)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("payment "))
}

/// Decode an `Authorization: Payment <base64url JSON>` value.
pub fn parse_credential(value: &str) -> Result<Credential, Problem> {
    if value.len() > MAX_CREDENTIAL_BYTES || !is_payment_authorization(value) {
        return Err(Problem::MalformedCredential);
    }
    let token = value.trim_start()[SCHEME.len()..].trim();
    let bytes = URL_SAFE_NO_PAD
        .decode(token.trim_end_matches('='))
        .map_err(|_| Problem::MalformedCredential)?;
    let credential: Credential =
        serde_json::from_slice(&bytes).map_err(|_| Problem::MalformedCredential)?;
    if credential.preimage().is_none() {
        return Err(Problem::MalformedCredential);
    }
    Ok(credential)
}

/// The decoded charge request a verified credential paid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Charge {
    pub amount_sats: u64,
    pub invoice: String,
    pub payment_hash: String,
}

/// Check that `credential` echoes a challenge this server issued with `key`
/// for this method, URL, and body, unexpired at `now`. The preimage and the
/// invoice are checked by the facilitator afterwards.
pub fn verify_binding(
    key: &[u8],
    credential: &Credential,
    http_method: &str,
    url: &str,
    body: &[u8],
    now: u64,
) -> Result<Charge, Problem> {
    let echo = &credential.challenge;
    if echo.method != METHOD || echo.intent != INTENT {
        return Err(Problem::UnknownChallenge);
    }
    let expected = challenge_id(key, echo);
    if !constant_time_eq(expected.as_bytes(), echo.id.as_bytes()) {
        return Err(Problem::UnknownChallenge);
    }
    // The id is authentic, so every bound field is ours; now check it
    // was issued for this request.
    if echo.opaque.as_deref() != Some(opaque_for(http_method, url).as_str()) {
        return Err(Problem::UnknownChallenge);
    }
    if echo.digest.as_deref() != Some(content_digest(body).as_str()) {
        return Err(Problem::DigestMismatch);
    }
    let expires = echo
        .expires
        .as_deref()
        .and_then(parse_rfc3339)
        .ok_or(Problem::UnknownChallenge)?;
    if now >= expires {
        return Err(Problem::Expired);
    }
    let request: Value = URL_SAFE_NO_PAD
        .decode(&echo.request)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or(Problem::MalformedCredential)?;
    let amount_sats = request["amount"]
        .as_str()
        .and_then(|a| a.parse().ok())
        .ok_or(Problem::MalformedCredential)?;
    let invoice = request["methodDetails"]["invoice"]
        .as_str()
        .ok_or(Problem::MalformedCredential)?
        .to_string();
    let payment_hash = request["methodDetails"]["paymentHash"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    Ok(Charge {
        amount_sats,
        invoice,
        payment_hash,
    })
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The `Payment-Receipt` value for a settled charge.
pub fn receipt(challenge_id: &str, payment_hash: &str, now: u64) -> String {
    b64url_json(&json!({
        "challengeId": challenge_id,
        "method": METHOD,
        "reference": payment_hash,
        "status": "success",
        "timestamp": rfc3339(now),
    }))
}

/// Why a `Payment` credential was refused, as the drafts' problem types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    PaymentRequired,
    MalformedCredential,
    UnknownChallenge,
    InvalidPreimage,
    Expired,
    DigestMismatch,
    VerificationFailed,
}

impl Problem {
    pub fn type_uri(self) -> String {
        let code = match self {
            Self::PaymentRequired => "payment-required",
            Self::MalformedCredential => "lightning/malformed-credential",
            Self::UnknownChallenge => "lightning/unknown-challenge",
            Self::InvalidPreimage => "lightning/invalid-preimage",
            Self::Expired => "lightning/expired-invoice",
            Self::DigestMismatch | Self::VerificationFailed => "verification-failed",
        };
        format!("{PROBLEMS}{code}")
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::PaymentRequired => "Payment Required",
            Self::MalformedCredential => "Malformed Credential",
            Self::UnknownChallenge => "Unknown Challenge",
            Self::InvalidPreimage => "Invalid Preimage",
            Self::Expired => "Expired Invoice",
            Self::DigestMismatch | Self::VerificationFailed => "Verification Failed",
        }
    }

    /// The problem for an x402 facilitator `errorReason`.
    pub fn from_reason(reason: &str) -> Self {
        match reason {
            crate::facilitator::DUPLICATE_SETTLEMENT => Self::UnknownChallenge,
            "invalid_exact_lnbtc_preimage_hash_mismatch"
            | "invalid_exact_lnbtc_preimage_malformed"
            | "invalid_exact_lnbtc_preimage_length"
            | "invalid_exact_lnbtc_preimage_missing" => Self::InvalidPreimage,
            "invalid_exact_lnbtc_invoice_expired" => Self::Expired,
            _ => Self::VerificationFailed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector_id(request: &str, header: Option<&str>, opaque: &str) -> String {
        // The draft's vectors include the optional header slot; this server
        // never sends `header`, so it is assembled by hand here.
        let mut slots = vec!["api.example.com", "tempo", "charge", request, "", ""];
        if let Some(header) = header {
            slots.push(header);
        }
        slots.push(opaque);
        URL_SAFE_NO_PAD.encode(hmac_sha256(
            b"test-vector-secret",
            slots.join("|").as_bytes(),
        ))
    }

    #[test]
    fn hmac_binding_matches_the_draft_vectors() {
        let request = b64url_json(&json!({"amount": "1000000"}));
        assert_eq!(request, "eyJhbW91bnQiOiIxMDAwMDAwIn0");
        assert_eq!(
            vector_id(&request, None, ""),
            "X6v1eo7fJ76gAxqY0xN9Jd__4lUyDDYmriryOM-5FO4"
        );
        assert_eq!(
            vector_id(&request, Some("Payment-Authorization"), ""),
            "S91xi-OFGZPMs-j7GsX0FDpIkmCcZT1P9XyV58WNy_U"
        );
        let opaque = b64url_json(&json!({"pi": "pi_123"}));
        assert_eq!(opaque, "eyJwaSI6InBpXzEyMyJ9");
        assert_eq!(
            vector_id(&request, Some("Payment-Authorization"), &opaque),
            "CJ4X1O4aTDmS59hfdhnhBtxIQjWDOf0bcrhsswwMOW8"
        );
        // The same function the server uses, on the legacy layout.
        let challenge = Challenge {
            id: String::new(),
            realm: "api.example.com".into(),
            method: "tempo".into(),
            intent: "charge".into(),
            request,
            expires: None,
            description: None,
            opaque: None,
            digest: None,
        };
        assert_eq!(
            challenge_id(b"test-vector-secret", &challenge),
            "X6v1eo7fJ76gAxqY0xN9Jd__4lUyDDYmriryOM-5FO4"
        );
    }

    #[test]
    fn hmac_matches_rfc4231_case_2() {
        assert_eq!(
            hex::encode(hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn digest_and_times_round_trip() {
        // The draft's example digest is of `{"amount":"1000"}`.
        assert_eq!(
            content_digest(b""),
            "sha-256=:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=:"
        );
        for secs in [0, 1_700_000_000, 1_791_043_200, 951_782_400, 4_102_444_800] {
            assert_eq!(parse_rfc3339(&rfc3339(secs)), Some(secs), "{secs}");
        }
        assert_eq!(rfc3339(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(
            parse_rfc3339("2026-10-02T16:49:01.399Z"),
            parse_rfc3339("2026-10-02T16:49:01Z")
        );
        assert_eq!(
            parse_rfc3339("2026-10-02T18:49:01+02:00"),
            parse_rfc3339("2026-10-02T16:49:01Z")
        );
        assert_eq!(parse_rfc3339("2026-10-02 16:49:01Z"), None);
    }

    fn issued() -> (Challenge, Vec<u8>) {
        let key = b"k".to_vec();
        let challenge = Challenge::issue(
            &key,
            &Terms {
                realm: "api.openagents.com",
                amount_sats: 21,
                invoice: "lnbc210n1x",
                payment_hash: &"ab".repeat(32),
                network: nostr::x402::MAINNET,
                http_method: "POST",
                url: "https://api.openagents.com/v1/messages",
                body: b"{}",
                expires_at: 2_000,
                description: Some("a \"quoted\" call"),
            },
        );
        (challenge, key)
    }

    fn credential(challenge: &Challenge) -> Credential {
        let value = json!({"challenge": challenge, "payload": {"preimage": "00".repeat(32)}});
        let header = format!("Payment {}", URL_SAFE_NO_PAD.encode(value.to_string()));
        parse_credential(&header).unwrap()
    }

    #[test]
    fn verifies_only_the_issued_call_unexpired() {
        let (challenge, key) = issued();
        let header = challenge.header_value();
        assert!(header.starts_with("Payment id=\""));
        assert!(header.contains("description=\"a \\\"quoted\\\" call\""));
        let url = "https://api.openagents.com/v1/messages";
        let charge =
            verify_binding(&key, &credential(&challenge), "POST", url, b"{}", 1_999).unwrap();
        assert_eq!(charge.amount_sats, 21);
        assert_eq!(charge.invoice, "lnbc210n1x");

        let check = |c: &Challenge, method: &str, url: &str, body: &[u8], now: u64| {
            verify_binding(&key, &credential(c), method, url, body, now).unwrap_err()
        };
        assert_eq!(
            check(&challenge, "POST", url, b"{}", 2_000),
            Problem::Expired
        );
        assert_eq!(
            check(&challenge, "POST", url, b"{ }", 1),
            Problem::DigestMismatch
        );
        assert_eq!(
            check(&challenge, "GET", url, b"{}", 1),
            Problem::UnknownChallenge
        );
        assert_eq!(
            check(
                &challenge,
                "POST",
                "https://api.openagents.com/v1/other",
                b"{}",
                1
            ),
            Problem::UnknownChallenge
        );
        let mut tampered = challenge.clone();
        tampered.expires = Some(rfc3339(9_999));
        assert_eq!(
            check(&tampered, "POST", url, b"{}", 1),
            Problem::UnknownChallenge
        );
        assert!(verify_binding(b"other", &credential(&challenge), "POST", url, b"{}", 1).is_err());
        assert_eq!(
            parse_credential("Payment abc").unwrap_err(),
            Problem::MalformedCredential
        );
        assert!(parse_credential("Bearer x").is_err());
    }
}
