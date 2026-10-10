//! The small piece of JOSE that agent checkout needs: base64url, JSON
//! canonicalization (RFC 8785), ES256 keys as JWKs, and compact and
//! detached JWS (RFC 7515, Appendix F).
//!
//! Only ES256 (ECDSA P-256 with SHA-256) is accepted, both for what we
//! sign and for what we verify: AP2 v0.2 names ES256/384/512, UCP's
//! examples use ES256, and a single algorithm leaves no room for
//! algorithm substitution. Signatures are ring's randomized ECDSA, as AP2
//! asks (no deterministic schemes for a checkout JWT).

use ring::rand::SystemRandom;
use ring::signature::{
    ECDSA_P256_SHA256_FIXED, ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair,
    UnparsedPublicKey,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Base64url without padding.
#[must_use]
pub fn b64url(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(URL[(n >> 18) as usize & 63] as char);
        out.push(URL[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(URL[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(URL[n as usize & 63] as char);
        }
    }
    out
}

fn sextet(byte: u8, url: bool) -> Option<u32> {
    Some(match byte {
        b'A'..=b'Z' => u32::from(byte - b'A'),
        b'a'..=b'z' => u32::from(byte - b'a') + 26,
        b'0'..=b'9' => u32::from(byte - b'0') + 52,
        b'-' if url => 62,
        b'_' if url => 63,
        b'+' if !url => 62,
        b'/' if !url => 63,
        _ => return None,
    })
}

fn decode(text: &str, url: bool) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if bytes.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, &byte) in chunk.iter().enumerate() {
            n |= sextet(byte, url)? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
        // Non-canonical trailing bits are refused, so one value has one
        // encoding (a tampered last character can't verify the same).
        let unused = match chunk.len() {
            2 => n & 0xFFFF,
            3 => n & 0xFF,
            _ => 0,
        };
        if unused != 0 {
            return None;
        }
    }
    Some(out)
}

/// Decode base64url without padding; anything else is `None`.
#[must_use]
pub fn unb64url(text: &str) -> Option<Vec<u8>> {
    decode(text, true)
}

/// Decode key material from configuration: base64url or standard
/// base64, padding optional, surrounding whitespace ignored.
#[must_use]
pub fn unbase64_any(text: &str) -> Option<Vec<u8>> {
    let trimmed: String = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .trim_end_matches('=')
        .to_owned();
    decode(&trimmed, true).or_else(|| decode(&trimmed, false))
}

/// `base64url(sha256(bytes))`, the digest SD-JWT, AP2 and our receipts use.
#[must_use]
pub fn sha256_b64url(bytes: &[u8]) -> String {
    b64url(&Sha256::digest(bytes))
}

/// RFC 8785 (JCS) serialization: object members sorted by their UTF-16
/// code units, no whitespace, ECMAScript string escaping (serde_json's),
/// and integral numbers printed without a fraction.
#[must_use]
pub fn canonical(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Null | Value::Bool(_) | Value::String(_) => {
            out.push_str(&serde_json::to_string(value).unwrap_or_default());
        }
        Value::Number(number) => {
            if let Some(f) = number.as_f64().filter(|_| number.is_f64())
                && f.is_finite()
                && f.fract() == 0.0
                && f.abs() < 1e21
            {
                out.push_str(&format!("{f:.0}"));
            } else {
                out.push_str(&number.to_string());
            }
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).unwrap_or_default());
                out.push(':');
                write_canonical(&map[key], out);
            }
            out.push('}');
        }
    }
}

/// A P-256 public key as JWK coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicKey {
    /// The key id, when the JWK names one.
    pub kid: Option<String>,
    /// `0x04 || x || y`.
    point: Vec<u8>,
}

impl PublicKey {
    /// Read an EC P-256 JWK (`kty: EC`, `crv: P-256`, `x`, `y`). Keys
    /// for other algorithms are `None`; they are never used.
    #[must_use]
    pub fn from_jwk(jwk: &Value) -> Option<Self> {
        if jwk["kty"] != "EC" || jwk["crv"] != "P-256" {
            return None;
        }
        if let Some(alg) = jwk.get("alg").and_then(Value::as_str)
            && alg != "ES256"
        {
            return None;
        }
        if let Some(usage) = jwk.get("use").and_then(Value::as_str)
            && usage != "sig"
        {
            return None;
        }
        let x = unb64url(jwk["x"].as_str()?)?;
        let y = unb64url(jwk["y"].as_str()?)?;
        if x.len() != 32 || y.len() != 32 {
            return None;
        }
        let mut point = Vec::with_capacity(65);
        point.push(4);
        point.extend_from_slice(&x);
        point.extend_from_slice(&y);
        Some(Self {
            kid: jwk.get("kid").and_then(Value::as_str).map(str::to_owned),
            point,
        })
    }

    /// The JWK, with `kid` when the key has one.
    #[must_use]
    pub fn jwk(&self) -> Value {
        let mut jwk = json!({
            "kty": "EC",
            "crv": "P-256",
            "x": b64url(&self.point[1..33]),
            "y": b64url(&self.point[33..65]),
            "use": "sig",
            "alg": "ES256",
        });
        if let Some(kid) = &self.kid {
            jwk["kid"] = json!(kid);
        }
        jwk
    }

    /// Check an ES256 signature (`r || s`, 64 bytes) over `message`.
    #[must_use]
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        signature.len() == 64
            && UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, &self.point)
                .verify(message, signature)
                .is_ok()
    }
}

/// An ES256 signing key, from an unencrypted PKCS#8 document.
pub struct Signer {
    pair: EcdsaKeyPair,
    rng: SystemRandom,
    kid: String,
}

impl Signer {
    /// Load the key. `pkcs8` is the DER document; `kid` names it in the
    /// published key set and in every JWS header.
    ///
    /// # Errors
    ///
    /// The document isn't a P-256 PKCS#8 private key.
    pub fn from_pkcs8(pkcs8: &[u8], kid: &str) -> Result<Self, String> {
        let rng = SystemRandom::new();
        let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8, &rng)
            .map_err(|_| "the signing key isn't a P-256 PKCS#8 private key".to_owned())?;
        Ok(Self {
            pair,
            rng,
            kid: kid.to_owned(),
        })
    }

    /// A fresh key, for tests and for minting sample mandates.
    ///
    /// # Errors
    ///
    /// The system's random source failed.
    pub fn generate(kid: &str) -> Result<(Self, Vec<u8>), String> {
        let rng = SystemRandom::new();
        let document = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng)
            .map_err(|_| "no randomness for a new key".to_owned())?;
        let pkcs8 = document.as_ref().to_vec();
        Ok((Self::from_pkcs8(&pkcs8, kid)?, pkcs8))
    }

    /// The key id.
    #[must_use]
    pub fn kid(&self) -> &str {
        &self.kid
    }

    /// The public half.
    #[must_use]
    pub fn public(&self) -> PublicKey {
        PublicKey {
            kid: Some(self.kid.clone()),
            point: self.pair.public_key().as_ref().to_vec(),
        }
    }

    /// Sign `message` (`r || s`).
    ///
    /// # Errors
    ///
    /// The system's random source failed.
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>, String> {
        self.pair
            .sign(&self.rng, message)
            .map(|signature| signature.as_ref().to_vec())
            .map_err(|_| "signing failed".to_owned())
    }

    /// A compact JWS over `payload` with `extra` header fields (`typ`,
    /// `cnf`…) beside `alg` and `kid`.
    ///
    /// # Errors
    ///
    /// Signing failed.
    pub fn compact(&self, extra: &Map<String, Value>, payload: &[u8]) -> Result<String, String> {
        let input = format!("{}.{}", self.header(extra), b64url(payload));
        let signature = self.sign(input.as_bytes())?;
        Ok(format!("{input}.{}", b64url(&signature)))
    }

    /// A detached JWS (`<header>..<signature>`) over the JCS form of
    /// `content`, as UCP's `merchant_authorization` is.
    ///
    /// # Errors
    ///
    /// Signing failed.
    pub fn detached(&self, content: &Value) -> Result<String, String> {
        let header = self.header(&Map::new());
        let input = format!("{header}.{}", b64url(canonical(content).as_bytes()));
        let signature = self.sign(input.as_bytes())?;
        Ok(format!("{header}..{}", b64url(&signature)))
    }

    fn header(&self, extra: &Map<String, Value>) -> String {
        let mut header = extra.clone();
        header.insert("alg".into(), json!("ES256"));
        header.insert("kid".into(), json!(self.kid));
        b64url(canonical(&Value::Object(header)).as_bytes())
    }
}

/// A compact JWS split and decoded, before its signature is checked.
#[derive(Debug, Clone)]
pub struct Jws {
    pub header: Map<String, Value>,
    pub payload: Vec<u8>,
    /// `<header>.<payload>` exactly as received: what was signed.
    pub input: String,
    pub signature: Vec<u8>,
}

impl Jws {
    /// Split a compact JWS. Only `alg: ES256` is read; anything else is
    /// `None` before any key is consulted.
    #[must_use]
    pub fn parse(token: &str) -> Option<Self> {
        let mut parts = token.split('.');
        let (header, payload, signature) = (parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() || header.is_empty() || signature.is_empty() {
            return None;
        }
        let header: Value = serde_json::from_slice(&unb64url(header)?).ok()?;
        let header = header.as_object()?.clone();
        if header.get("alg").and_then(Value::as_str) != Some("ES256")
            || header.contains_key("crit")
        {
            return None;
        }
        let input_len = token.len() - parts_len(signature);
        Some(Self {
            header,
            payload: unb64url(payload)?,
            input: token[..input_len].to_owned(),
            signature: unb64url(signature)?,
        })
    }

    /// The header's `kid`.
    #[must_use]
    pub fn kid(&self) -> Option<&str> {
        self.header.get("kid").and_then(Value::as_str)
    }

    /// The header's `typ`.
    #[must_use]
    pub fn typ(&self) -> Option<&str> {
        self.header.get("typ").and_then(Value::as_str)
    }

    /// The payload as a JSON object.
    #[must_use]
    pub fn claims(&self) -> Option<Map<String, Value>> {
        serde_json::from_slice::<Value>(&self.payload)
            .ok()?
            .as_object()
            .cloned()
    }

    /// Whether `key` signed it.
    #[must_use]
    pub fn verified_by(&self, key: &PublicKey) -> bool {
        key.verify(self.input.as_bytes(), &self.signature)
    }
}

/// `".<signature>"`: what follows the signing input in a compact JWS.
fn parts_len(signature: &str) -> usize {
    signature.len() + 1
}

/// Check a detached JWS (`<header>..<signature>`) over the JCS form of
/// `content` against `keys`, chosen by the header's `kid`.
#[must_use]
pub fn verify_detached(token: &str, content: &Value, keys: &[PublicKey]) -> bool {
    let Some((header, signature)) = token.split_once("..") else {
        return false;
    };
    if signature.contains('.') {
        return false;
    }
    let attached = format!(
        "{header}.{}.{signature}",
        b64url(canonical(content).as_bytes())
    );
    let Some(jws) = Jws::parse(&attached) else {
        return false;
    };
    pick(keys, jws.kid()).is_some_and(|key| jws.verified_by(key))
}

/// The key a `kid` names; with no `kid`, the only key when there is one.
#[must_use]
pub fn pick<'a>(keys: &'a [PublicKey], kid: Option<&str>) -> Option<&'a PublicKey> {
    match kid {
        Some(kid) => keys.iter().find(|key| key.kid.as_deref() == Some(kid)),
        None if keys.len() == 1 => keys.first(),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_round_trips_and_refuses_other_alphabets() {
        for bytes in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar"] {
            assert_eq!(unb64url(&b64url(bytes)).as_deref(), Some(bytes));
        }
        assert_eq!(b64url(b"foobar"), "Zm9vYmFy");
        assert_eq!(b64url(&[0xfb, 0xff]), "-_8");
        assert!(unb64url("+/8").is_none());
        assert!(unb64url("Zm9vYg==").is_none());
        // A non-canonical last character is refused.
        assert!(unb64url("Zh").is_none());
        assert_eq!(unbase64_any("+/8=").unwrap(), vec![0xfb, 0xff]);
    }

    #[test]
    fn canonical_json_sorts_members_and_drops_whitespace() {
        let value: Value =
            serde_json::from_str(r#"{"b": [1, 2.0, "x"], "a": {"d": null, "c": true}}"#).unwrap();
        assert_eq!(
            canonical(&value),
            r#"{"a":{"c":true,"d":null},"b":[1,2,"x"]}"#
        );
    }

    #[test]
    fn compact_and_detached_signatures_verify_and_tampering_fails() {
        let (signer, _) = Signer::generate("k1").unwrap();
        let keys = vec![signer.public()];
        let token = signer.compact(&Map::new(), br#"{"a":1}"#).unwrap();
        let jws = Jws::parse(&token).unwrap();
        assert!(jws.verified_by(&keys[0]));
        let tampered = token.replacen(&b64url(br#"{"a":1}"#), &b64url(br#"{"a":2}"#), 1);
        assert!(!Jws::parse(&tampered).unwrap().verified_by(&keys[0]));

        let content = json!({"id": "chk_1", "totals": [{"type": "total", "amount": 2000}]});
        let detached = signer.detached(&content).unwrap();
        assert!(detached.contains(".."));
        assert!(verify_detached(&detached, &content, &keys));
        let changed = json!({"id": "chk_1", "totals": [{"type": "total", "amount": 1}]});
        assert!(!verify_detached(&detached, &changed, &keys));
        let (other, _) = Signer::generate("k1").unwrap();
        assert!(!verify_detached(&detached, &content, &[other.public()]));
    }

    #[test]
    fn jwks_round_trip_and_other_curves_are_ignored() {
        let (signer, _) = Signer::generate("k1").unwrap();
        let jwk = signer.public().jwk();
        assert_eq!(PublicKey::from_jwk(&jwk), Some(signer.public()));
        let mut ed = jwk.clone();
        ed["kty"] = json!("OKP");
        assert!(PublicKey::from_jwk(&ed).is_none());
    }

    #[test]
    fn only_es256_is_read() {
        let header = b64url(br#"{"alg":"none"}"#);
        assert!(Jws::parse(&format!("{header}.e30.AA")).is_none());
        let header = b64url(br#"{"alg":"HS256"}"#);
        assert!(Jws::parse(&format!("{header}.e30.AA")).is_none());
    }
}
