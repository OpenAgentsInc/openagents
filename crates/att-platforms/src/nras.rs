//! NVIDIA GPU evidence verified by NVIDIA's Remote Attestation Service
//! (NRAS): the overall EAT and each GPU's detached EAT, ES384 JWTs whose
//! signing keys come from NRAS's JWKS with an `x5c` chain to a pinned NVIDIA
//! certificate. The workload gathers this with the endpoint binding as its
//! nonce.
use crate::{Result, pki};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const ISSUER: &str = "https://nras.attestation.nvidia.com";
const SKEW: u64 = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gpus {
    /// Each GPU's hardware model, such as `GH100 A01 GSP BROM`.
    pub models: Vec<String>,
}

fn segment(part: &str) -> Result<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(part)
        .map_err(|_| "a JWT segment is not base64url".into())
}

/// Verify one ES384 JWT against the JWKS and the pinned anchors.
fn token(jwt: &str, jwks: &Value, anchors: &[String], now: u64) -> Result<Value> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err("not a JWT".into());
    }
    let header: Value = serde_json::from_slice(&segment(parts[0])?).map_err(|_| "JWT header")?;
    if header["alg"] != "ES384" {
        return Err("NRAS tokens are ES384".into());
    }
    let kid = header["kid"].as_str().ok_or("the token names no key")?;
    let jwk = jwks["keys"]
        .as_array()
        .and_then(|keys| keys.iter().find(|k| k["kid"] == kid))
        .ok_or("the token's key is not in NRAS's key set")?;
    let x5c: Vec<Vec<u8>> = jwk["x5c"]
        .as_array()
        .ok_or("the key has no certificate chain")?
        .iter()
        .map(|c| {
            base64::engine::general_purpose::STANDARD
                .decode(c.as_str().unwrap_or_default())
                .map_err(|_| "x5c".into())
        })
        .collect::<Result<_>>()?;
    // The chain ends at a pinned NVIDIA certificate; the last element is
    // the anchor itself, so it is checked as present rather than self-signed.
    let anchor = x5c.last().ok_or("empty x5c")?;
    if !anchors.contains(&pki::fingerprint(anchor)) {
        return Err("the NRAS key does not chain to a pinned NVIDIA certificate".into());
    }
    let certs: Vec<x509_cert::Certificate> =
        x5c.iter().map(|d| pki::parse(d)).collect::<Result<_>>()?;
    for cert in &certs {
        pki::current(cert, now)?;
    }
    for pair in certs.windows(2) {
        pki::signed_by(&pair[0], &pair[1])?;
    }
    let pki::Key::P384(key) = pki::key(&certs[0])? else {
        return Err("the NRAS signing key is not P-384".into());
    };
    let point = key.to_encoded_point(false);
    if jwk["x"].as_str().map(segment).transpose()?.as_deref() != point.x().map(|x| x.as_slice())
        || jwk["y"].as_str().map(segment).transpose()?.as_deref() != point.y().map(|y| y.as_slice())
    {
        return Err("the JWK differs from its certificate".into());
    }
    let sig = p384::ecdsa::Signature::from_slice(&segment(parts[2])?)
        .map_err(|_| "malformed JWT signature")?;
    use p384::ecdsa::signature::Verifier;
    key.verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &sig)
        .map_err(|_| "the NRAS token signature does not verify")?;
    let claims: Value = serde_json::from_slice(&segment(parts[1])?).map_err(|_| "JWT claims")?;
    if claims["iss"] != ISSUER {
        return Err("the token is not from NRAS".into());
    }
    let exp = claims["exp"].as_u64().ok_or("the token has no expiry")?;
    if exp <= now
        || claims["iat"].as_u64().unwrap_or(u64::MAX) > now + SKEW
        || claims["nbf"].as_u64().is_some_and(|n| n > now + SKEW)
    {
        return Err("the NRAS token is not current".into());
    }
    Ok(claims)
}

/// Verify NRAS evidence: `[["JWT", overall], {"GPU-0": gpu, …}]`, bound to
/// `binding`, every GPU passing with one of `models`.
pub fn verify(
    evidence: &str,
    jwks: &Value,
    anchors: &[String],
    binding: &str,
    models: &[String],
    now: u64,
) -> Result<Gpus> {
    let value: Value =
        serde_json::from_str(evidence).map_err(|_| "NRAS evidence does not parse")?;
    let overall = value[0][1]
        .as_str()
        .ok_or("NRAS evidence has no overall token")?;
    let claims = token(overall, jwks, anchors, now)?;
    if claims["x-nvidia-overall-att-result"] != true {
        return Err("NRAS reports the GPU attestation failed".into());
    }
    if claims["eat_nonce"] != binding {
        return Err("the GPU evidence is not bound to this endpoint".into());
    }
    let submods = claims["submods"]
        .as_object()
        .ok_or("NRAS evidence lists no GPUs")?;
    let gpus = value[1]
        .as_object()
        .ok_or("NRAS evidence has no GPU tokens")?;
    if submods.is_empty() || submods.len() != gpus.len() {
        return Err("the GPU tokens differ from the overall token".into());
    }
    let mut found = Vec::new();
    for (name, digest) in submods {
        let jwt = gpus
            .get(name)
            .and_then(Value::as_str)
            .ok_or("a GPU token is missing")?;
        let expected: String = Sha256::digest(jwt.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if digest[0] != "DIGEST" || digest[1][0] != "SHA256" || digest[1][1] != expected.as_str() {
            return Err("a GPU token does not match its digest".into());
        }
        let gpu = token(jwt, jwks, anchors, now)?;
        if gpu["measres"] != "success"
            || gpu["secboot"] != true
            || gpu["dbgstat"] != "disabled"
            || gpu["eat_nonce"] != binding
        {
            return Err("a GPU failed measurement, secure boot, or debug checks".into());
        }
        let hwmodel = gpu["hwmodel"].as_str().unwrap_or_default().to_string();
        if !models.iter().any(|m| hwmodel.starts_with(&format!("G{m}"))) {
            return Err("a GPU is not a model the release admits".into());
        }
        found.push(hwmodel);
    }
    Ok(Gpus { models: found })
}
