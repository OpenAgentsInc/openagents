//! Provider credentials: PEM keys and compact JWTs.
//!
//! APNs uses an ES256 provider token signed with the `.p8` key from the
//! Apple Developer account. FCM uses an RS256 assertion signed with the
//! service account key, exchanged for an OAuth access token. Keys are read
//! from files at startup and never logged.

use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use ring::{
    rand::SystemRandom,
    signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, RSA_PKCS1_SHA256, RsaKeyPair},
};

/// Decode the first PEM block labeled `label` to DER.
///
/// # Errors
///
/// Returns a reason when the block is missing or not base64.
pub fn pem_der(pem: &str, label: &str) -> Result<Vec<u8>, String> {
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let start = pem
        .find(&begin)
        .ok_or_else(|| format!("no {label} PEM block"))?
        + begin.len();
    let stop = pem[start..]
        .find(&end)
        .ok_or_else(|| format!("unterminated {label} PEM block"))?
        + start;
    let body: String = pem[start..stop]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    STANDARD
        .decode(body)
        .map_err(|_| format!("the {label} PEM block is not base64"))
}

/// An ES256 signer from a PKCS#8 `.p8` key.
///
/// # Errors
///
/// Returns a reason when the key is not a P-256 PKCS#8 key.
pub fn es256_key(pem: &str) -> Result<EcdsaKeyPair, String> {
    let der = pem_der(pem, "PRIVATE KEY")?;
    EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &der, &SystemRandom::new())
        .map_err(|_| "the APNs key is not a P-256 PKCS#8 key".to_owned())
}

/// An RS256 signer from a PKCS#8 RSA key.
///
/// # Errors
///
/// Returns a reason when the key is not an RSA PKCS#8 key.
pub fn rs256_key(pem: &str) -> Result<RsaKeyPair, String> {
    let der = pem_der(pem, "PRIVATE KEY")?;
    RsaKeyPair::from_pkcs8(&der)
        .map_err(|_| "the service account key is not an RSA PKCS#8 key".to_owned())
}

/// A compact ES256 JWT.
///
/// # Errors
///
/// Returns a reason when signing fails.
pub fn es256(
    key: &EcdsaKeyPair,
    header: &serde_json::Value,
    claims: &serde_json::Value,
) -> Result<String, String> {
    let input = signing_input(header, claims);
    let signature = key
        .sign(&SystemRandom::new(), input.as_bytes())
        .map_err(|_| "ES256 signing failed")?;
    Ok(format!(
        "{input}.{}",
        URL_SAFE_NO_PAD.encode(signature.as_ref())
    ))
}

/// A compact RS256 JWT.
///
/// # Errors
///
/// Returns a reason when signing fails.
pub fn rs256(
    key: &RsaKeyPair,
    header: &serde_json::Value,
    claims: &serde_json::Value,
) -> Result<String, String> {
    let input = signing_input(header, claims);
    let mut signature = vec![0_u8; key.public().modulus_len()];
    key.sign(
        &RSA_PKCS1_SHA256,
        &SystemRandom::new(),
        input.as_bytes(),
        &mut signature,
    )
    .map_err(|_| "RS256 signing failed")?;
    Ok(format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature)))
}

fn signing_input(header: &serde_json::Value, claims: &serde_json::Value) -> String {
    format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(header.to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string())
    )
}

/// Accept `https://` URLs, and `http://` only on a loopback host for local
/// test servers.
///
/// # Errors
///
/// Returns a reason for any other URL.
pub fn provider_url(name: &str, url: &str) -> Result<String, String> {
    let trimmed = url.trim_end_matches('/');
    if url.contains(['?', '#', '@']) || url.chars().any(char::is_whitespace) {
        return Err(format!(
            "{name} must not contain a query, fragment, or credentials"
        ));
    }
    if trimmed.starts_with("https://") && trimmed.len() > "https://".len() {
        return Ok(trimmed.to_owned());
    }
    if let Some(rest) = trimmed.strip_prefix("http://") {
        let host = rest.split('/').next().unwrap_or_default();
        let host = host.rsplit_once(':').map_or(host, |(host, port)| {
            if port.parse::<u16>().is_ok() {
                host
            } else {
                ""
            }
        });
        if matches!(host, "127.0.0.1" | "localhost" | "[::1]") {
            return Ok(trimmed.to_owned());
        }
    }
    Err(format!(
        "{name} must be an https:// URL, or http:// on a loopback host for testing"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{ECDSA_P256_SHA256_FIXED, KeyPair as _, UnparsedPublicKey};

    #[test]
    fn an_es256_token_verifies_with_the_public_key() {
        let document =
            EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &SystemRandom::new())
                .unwrap();
        let pem = format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            STANDARD.encode(document.as_ref())
        );
        let key = es256_key(&pem).unwrap();
        let token = es256(
            &key,
            &serde_json::json!({"alg":"ES256","kid":"KEY"}),
            &serde_json::json!({"iss":"TEAM","iat":1}),
        )
        .unwrap();
        let (input, signature) = token.rsplit_once('.').unwrap();
        UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, key.public_key().as_ref())
            .verify(
                input.as_bytes(),
                &URL_SAFE_NO_PAD.decode(signature).unwrap(),
            )
            .unwrap();
        assert!(es256_key("not a key").is_err());
        assert!(rs256_key(&pem).is_err());
    }

    #[test]
    fn provider_urls_are_https_or_loopback() {
        assert!(provider_url("x", "https://api.push.apple.com").is_ok());
        assert!(provider_url("x", "http://127.0.0.1:9000/").is_ok());
        assert!(provider_url("x", "http://localhost:9000").is_ok());
        assert!(provider_url("x", "http://api.push.apple.com").is_err());
        assert!(provider_url("x", "https://user@host").is_err());
        assert!(provider_url("x", "ftp://host").is_err());
    }
}
