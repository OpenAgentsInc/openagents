//! Google Confidential Space attestation tokens (`PKI` type): an RS256 JWT
//! whose header carries the signing certificate chain (`x5c`), checked up
//! to Google's Confidential Space root, which is pinned here by its bytes
//! and SHA-256 fingerprint.
//!
//! Google documents the token at
//! <https://docs.cloud.google.com/confidential-computing/confidential-space/docs/reference/token-claims>
//! and the root at
//! <https://confidentialcomputing.googleapis.com/.well-known/confidential_space_root.crt>.

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use rsa::RsaPublicKey;
use rsa::pkcs1v15::{Signature, VerifyingKey};
use rsa::pkcs8::DecodePublicKey;
use rsa::signature::Verifier;
use serde_json::Value;
use sha2::{Digest, Sha256};
use x509_cert::Certificate;
use x509_cert::der::{Decode, Encode};

/// Google's Confidential Space root CA, as published (PEM).
pub const ROOT_PEM: &str = include_str!("../roots/confidential_space_root.pem");
/// SHA-256 of the root's DER bytes.
pub const ROOT_SHA256: &str = "148b293821bb0c6a317f413c8ba475814091cb22d49b9e3c94198db8e8f86c39";
/// The issuer every token names.
pub const ISSUER: &str = "https://confidentialcomputing.googleapis.com";
/// sha256WithRSAEncryption.
const SHA256_WITH_RSA: &str = "1.2.840.113549.1.1.11";

/// One certificate of the chain, as shown to people.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ChainLink {
    /// The subject's common name and organization.
    pub subject: String,
    pub issuer: String,
    /// SHA-256 of the certificate's DER bytes.
    pub sha256: String,
    pub not_before: u64,
    pub not_after: u64,
}

/// The claims a verified token carries that NIP-ATT reads, plus the chain
/// that signed it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Claims {
    pub iss: String,
    pub aud: Vec<String>,
    pub iat: u64,
    pub exp: u64,
    pub hwmodel: String,
    pub swname: String,
    pub swversion: Vec<String>,
    pub dbgstat: String,
    /// `submods.confidential_space.support_attributes`.
    pub support: Vec<String>,
    /// `submods.container.image_digest`.
    pub image_digest: String,
    /// `submods.container.image_reference`.
    pub image_reference: String,
    /// `eat_nonce`, one string or several.
    pub nonces: Vec<String>,
    /// `submods.gce.zone`.
    pub zone: String,
    /// `submods.gce.instance_name`.
    pub instance_name: String,
    /// `submods.gce.project_id`.
    pub project_id: String,
    /// The JWT header's `alg`.
    pub alg: String,
    /// `submods.nvidia_gpu`, when the machine has a GPU attached and the
    /// launcher attested it (Google checks the GPU's NVIDIA evidence and
    /// signs these claims in the same token).
    pub gpu: Option<GpuClaims>,
    /// Leaf first, root last.
    pub chain: Vec<ChainLink>,
    /// Every claim, as decoded.
    pub raw: Value,
}

/// `submods.nvidia_gpu` of a Confidential Space token.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GpuClaims {
    /// `OFF`, `ON` or `DEVTOOLS`. Only `ON` has every NVIDIA confidential
    /// computing protection active.
    pub cc_mode: String,
    /// `SPT` (single GPU passthrough), the only mode Confidential Space runs.
    pub cc_feature: String,
    pub driver_version: String,
    pub gpus: Vec<GpuDevice>,
}

/// One attested GPU.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GpuDevice {
    /// `GCP_NVIDIA_H100`.
    pub hwmodel: String,
    /// The device's universal entity ID (RFC 9711).
    pub ueid: String,
    pub vbios_version: String,
}

fn gpu_claims(raw: &Value) -> Option<GpuClaims> {
    let gpu = raw["submods"].get("nvidia_gpu")?;
    if !gpu.is_object() {
        return None;
    }
    let text = |v: &Value, key: &str| v[key].as_str().unwrap_or_default().to_string();
    Some(GpuClaims {
        cc_mode: text(gpu, "cc_mode"),
        cc_feature: text(gpu, "cc_feature"),
        // Per GPU in tokens seen so far; the top level is the documented spot.
        driver_version: Some(text(gpu, "driver_version"))
            .filter(|v| !v.is_empty())
            .or_else(|| {
                gpu["gpus"][0]["driver_version"]
                    .as_str()
                    .map(str::to_string)
            })
            .unwrap_or_default(),
        gpus: gpu["gpus"]
            .as_array()
            .map(|list| {
                list.iter()
                    .map(|d| GpuDevice {
                        hwmodel: text(d, "hwmodel"),
                        ueid: text(d, "ueid"),
                        vbios_version: text(d, "vbios_version"),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// Why a token was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub String);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn refuse<T>(reason: impl Into<String>) -> Result<T, Refused> {
    Err(Refused(reason.into()))
}

/// The pinned root's DER bytes.
///
/// # Panics
///
/// Never: the PEM is compiled in and checked by a test.
#[must_use]
pub fn root_der() -> Vec<u8> {
    let body: String = ROOT_PEM
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    STANDARD.decode(body).expect("the pinned root is base64")
}

/// Verify a Confidential Space PKI token at `now` (Unix seconds) and
/// return its claims. Checks, in order: the JWT shape and `alg: RS256`;
/// the `x5c` chain ends in the pinned root, each certificate is signed by
/// the next and valid at `now`; the JWT signature under the leaf; then the
/// issuer, the audience `audience`, `iat`/`exp` against `now`, and that
/// the image is not a debug image (`dbgstat` `disabled-since-boot`,
/// `swname` `CONFIDENTIAL_SPACE`).
///
/// # Errors
///
/// The first check that fails, in plain words.
pub fn verify(token: &str, audience: &str, now: u64) -> Result<Claims, Refused> {
    let parts: Vec<&str> = token.split('.').collect();
    let [header_b64, payload_b64, signature_b64] = parts.as_slice() else {
        return refuse("the token is not a three-part JWT");
    };
    let header: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(header_b64)
            .map_err(|_| Refused("the token header is not base64url".into()))?,
    )
    .map_err(|_| Refused("the token header is not JSON".into()))?;
    let alg = header["alg"].as_str().unwrap_or_default().to_string();
    if alg != "RS256" {
        return refuse(format!("the token is signed with {alg}, not RS256"));
    }
    let x5c: Vec<Vec<u8>> = header["x5c"]
        .as_array()
        .ok_or_else(|| {
            Refused("the token carries no certificate chain (x5c); it is not a PKI token".into())
        })?
        .iter()
        .map(|c| {
            c.as_str()
                .and_then(|s| STANDARD.decode(s).ok())
                .ok_or_else(|| Refused("a chain certificate is not base64".into()))
        })
        .collect::<Result<_, _>>()?;
    if x5c.len() < 2 {
        return refuse("the chain is shorter than a leaf and a root");
    }
    let chain = check_chain(&x5c, now)?;
    let leaf = Certificate::from_der(&x5c[0])
        .map_err(|_| Refused("the leaf certificate does not parse".into()))?;
    let key = public_key(&leaf)?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature_b64)
        .map_err(|_| Refused("the token signature is not base64url".into()))?;
    let signed = format!("{header_b64}.{payload_b64}");
    verify_rsa(&key, signed.as_bytes(), &signature).map_err(|_| {
        Refused("the token's signature does not verify under the leaf certificate".into())
    })?;

    let raw: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(payload_b64)
            .map_err(|_| Refused("the token claims are not base64url".into()))?,
    )
    .map_err(|_| Refused("the token claims are not JSON".into()))?;
    let claims = claims(raw, alg, chain)?;
    if claims.iss != ISSUER {
        return refuse(format!(
            "the token's issuer is {}, not Google's attestation service",
            claims.iss
        ));
    }
    if !claims.aud.iter().any(|a| a == audience) {
        return refuse(format!(
            "the token was issued for another audience, not {audience}"
        ));
    }
    if claims.exp <= now {
        return refuse("the token has expired");
    }
    if claims.iat > now + 300 {
        return refuse("the token was issued in the future");
    }
    if claims.swname != "CONFIDENTIAL_SPACE" {
        return refuse(format!(
            "the software is {}, not Confidential Space",
            claims.swname
        ));
    }
    if claims.dbgstat != "disabled-since-boot" {
        return refuse(format!(
            "the machine allows debugging ({}); a debug image is never trusted",
            claims.dbgstat
        ));
    }
    Ok(claims)
}

/// Check the `x5c` chain: the last certificate is the pinned root, each
/// one is signed by the next, names it as issuer, and is valid at `now`.
fn check_chain(x5c: &[Vec<u8>], now: u64) -> Result<Vec<ChainLink>, Refused> {
    let root = root_der();
    let last = x5c
        .last()
        .ok_or_else(|| Refused("the chain is empty".into()))?;
    if hex(&Sha256::digest(last)) != ROOT_SHA256 || *last != root {
        return refuse("the chain does not end in Google's Confidential Space root");
    }
    let certs: Vec<Certificate> = x5c
        .iter()
        .map(|der| {
            Certificate::from_der(der)
                .map_err(|_| Refused("a chain certificate does not parse".into()))
        })
        .collect::<Result<_, _>>()?;
    let mut links = Vec::with_capacity(certs.len());
    for (i, cert) in certs.iter().enumerate() {
        let tbs = &cert.tbs_certificate;
        let not_before = tbs.validity.not_before.to_unix_duration().as_secs();
        let not_after = tbs.validity.not_after.to_unix_duration().as_secs();
        let subject = tbs.subject.to_string();
        if now < not_before || now > not_after {
            return refuse(format!("the certificate {subject} is not valid now"));
        }
        let issuer_cert = certs.get(i + 1).unwrap_or(cert);
        if tbs.issuer != issuer_cert.tbs_certificate.subject {
            return refuse(format!("the certificate {subject} names another issuer"));
        }
        if cert.signature_algorithm.oid.to_string() != SHA256_WITH_RSA {
            return refuse(format!(
                "the certificate {subject} is not signed with RSA and SHA-256"
            ));
        }
        let key = public_key(issuer_cert)?;
        let tbs_der = tbs
            .to_der()
            .map_err(|_| Refused("a certificate does not re-encode".into()))?;
        let signature = cert
            .signature
            .as_bytes()
            .ok_or_else(|| Refused("a certificate signature is not whole bytes".into()))?;
        verify_rsa(&key, &tbs_der, signature).map_err(|_| {
            Refused(format!(
                "the certificate {subject} is not signed by its issuer"
            ))
        })?;
        links.push(ChainLink {
            subject,
            issuer: tbs.issuer.to_string(),
            sha256: hex(&Sha256::digest(&x5c[i])),
            not_before,
            not_after,
        });
    }
    Ok(links)
}

fn public_key(cert: &Certificate) -> Result<RsaPublicKey, Refused> {
    let spki = cert
        .tbs_certificate
        .subject_public_key_info
        .to_der()
        .map_err(|_| Refused("a certificate key does not encode".into()))?;
    RsaPublicKey::from_public_key_der(&spki)
        .map_err(|_| Refused("a certificate key is not RSA".into()))
}

fn verify_rsa(key: &RsaPublicKey, message: &[u8], signature: &[u8]) -> Result<(), ()> {
    let verifier = VerifyingKey::<Sha256>::new(key.clone());
    let signature = Signature::try_from(signature).map_err(|_| ())?;
    verifier.verify(message, &signature).map_err(|_| ())
}

fn strings(value: &Value) -> Vec<String> {
    match value {
        Value::String(s) => vec![s.clone()],
        Value::Array(items) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

fn claims(raw: Value, alg: String, chain: Vec<ChainLink>) -> Result<Claims, Refused> {
    let text = |path: &[&str]| -> String {
        let mut at = &raw;
        for key in path {
            at = &at[*key];
        }
        at.as_str().unwrap_or_default().to_string()
    };
    let number = |key: &str| {
        raw[key]
            .as_u64()
            .ok_or_else(|| Refused(format!("the token has no `{key}`")))
    };
    Ok(Claims {
        iss: text(&["iss"]),
        aud: strings(&raw["aud"]),
        iat: number("iat")?,
        exp: number("exp")?,
        hwmodel: text(&["hwmodel"]),
        swname: text(&["swname"]),
        swversion: strings(&raw["swversion"]),
        dbgstat: text(&["dbgstat"]),
        support: strings(&raw["submods"]["confidential_space"]["support_attributes"]),
        image_digest: text(&["submods", "container", "image_digest"]),
        image_reference: text(&["submods", "container", "image_reference"]),
        nonces: strings(&raw["eat_nonce"]),
        zone: text(&["submods", "gce", "zone"]),
        instance_name: text(&["submods", "gce", "instance_name"]),
        project_id: text(&["submods", "gce", "project_id"]),
        gpu: gpu_claims(&raw),
        alg,
        chain,
        raw,
    })
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pinned_root_matches_its_fingerprint() {
        let der = root_der();
        assert_eq!(hex(&Sha256::digest(&der)), ROOT_SHA256);
        let cert = Certificate::from_der(&der).unwrap();
        assert!(
            cert.tbs_certificate
                .subject
                .to_string()
                .contains("Confidential Space Root CA")
        );
        // The root signs itself.
        check_chain(&[der.clone(), der], 1_791_400_000).unwrap();
    }

    #[test]
    fn a_chain_not_ending_in_the_root_is_refused() {
        let der = root_der();
        let mut other = der.clone();
        let n = other.len();
        other[n - 5] ^= 1;
        assert!(check_chain(&[der, other], 1_791_400_000).is_err());
    }

    #[test]
    fn gpu_claims_are_read_from_submods() {
        let raw = serde_json::json!({"submods": {"nvidia_gpu": {
            "cc_mode": "ON", "cc_feature": "SPT", "driver_version": "590.48.01",
            "gpus": [{"hwmodel": "GCP_NVIDIA_H100", "ueid": "ab", "vbios_version": "96.00.CF.00.01"}]
        }}});
        let gpu = gpu_claims(&raw).unwrap();
        assert_eq!(gpu.cc_mode, "ON");
        assert_eq!(gpu.gpus[0].hwmodel, "GCP_NVIDIA_H100");
        assert!(gpu_claims(&serde_json::json!({"submods": {}})).is_none());
    }

    #[test]
    fn shapes_that_are_not_pki_tokens_are_refused() {
        assert!(verify("abc", "x", 0).is_err());
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        assert!(
            verify(&format!("{header}.e30.x"), "x", 0)
                .unwrap_err()
                .0
                .contains("RS256")
        );
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256"}"#);
        assert!(
            verify(&format!("{header}.e30.x"), "x", 0)
                .unwrap_err()
                .0
                .contains("x5c")
        );
    }
}
