//! Policy for a Standard Play Integrity verdict decoded by Google's server.
//!
//! This module does not authenticate an arbitrary JSON document. Its caller
//! must obtain the document from Google's authenticated decode endpoint.
//! It supplies no stable install identity and grants no per-install credit.
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Bind the platform proof to this sign-in challenge and identity key.
/// The SDK uses the same versioned byte format.
#[must_use]
pub fn request_hash(challenge: &str, pubkey: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            format!("openagents.play-integrity.sign-in.v1\n{challenge}\n{pubkey}").as_bytes()
        )
    )
}

/// The operator's allowed Android release identities.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub package_name: String,
    /// Play app-signing certificate SHA-256 digests, base64url without padding.
    pub certificate_sha256: Vec<String>,
    pub minimum_version_code: u64,
}

/// Why a decoded verdict is not sufficient for sign-up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Malformed,
    Request,
    Expired,
    App,
    Certificate,
    Version,
    Device,
    License,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Verdict {
    request_details: Request,
    app_integrity: App,
    device_integrity: Device,
    account_details: Account,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    request_package_name: String,
    request_hash: String,
    timestamp_millis: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct App {
    app_recognition_verdict: String,
    #[serde(default)]
    package_name: String,
    #[serde(default)]
    certificate_sha256_digest: Vec<String>,
    #[serde(default)]
    version_code: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Device {
    #[serde(default)]
    device_recognition_verdict: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Account {
    app_licensing_verdict: String,
}

impl Policy {
    /// Validate configuration without accepting missing release identities.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.package_name.len() > 255
            || !self.package_name.contains('.')
            || self.package_name.split('.').any(|part| {
                part.is_empty() || !part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            })
            || self.minimum_version_code == 0
            || self.certificate_sha256.is_empty()
            || self.certificate_sha256.iter().any(|digest| {
                digest.len() != 43
                    || !digest
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                    || crate::attest::decode_base64(digest).is_none_or(|bytes| bytes.len() != 32)
            })
        {
            return Err(
                "Play Integrity needs a package, signing certificate digests, and minimum version code",
            );
        }
        Ok(())
    }

    /// Check only a verdict returned by the trusted Google decoder.
    /// `expected_hash` binds the verified Nostr proof's challenge and key.
    pub fn verify(
        &self,
        decoded: &serde_json::Value,
        expected_hash: &str,
        now_ms: u64,
    ) -> Result<(), Refusal> {
        let verdict: Verdict =
            serde_json::from_value(decoded.clone()).map_err(|_| Refusal::Malformed)?;
        let request = verdict.request_details;
        if request.request_package_name != self.package_name
            || request.request_hash != expected_hash
        {
            return Err(Refusal::Request);
        }
        let at: u64 = request
            .timestamp_millis
            .parse()
            .map_err(|_| Refusal::Malformed)?;
        if at > now_ms.saturating_add(30_000) || now_ms.saturating_sub(at) > 120_000 {
            return Err(Refusal::Expired);
        }
        let app = verdict.app_integrity;
        if app.app_recognition_verdict != "PLAY_RECOGNIZED" || app.package_name != self.package_name
        {
            return Err(Refusal::App);
        }
        // Every signer must be explicitly allowed, including after a rotation.
        if app.certificate_sha256_digest.is_empty()
            || app
                .certificate_sha256_digest
                .iter()
                .any(|digest| !self.certificate_sha256.contains(digest))
        {
            return Err(Refusal::Certificate);
        }
        let version: u64 = app.version_code.parse().map_err(|_| Refusal::Version)?;
        if version < self.minimum_version_code {
            return Err(Refusal::Version);
        }
        if !verdict
            .device_integrity
            .device_recognition_verdict
            .iter()
            .any(|v| v == "MEETS_DEVICE_INTEGRITY")
        {
            return Err(Refusal::Device);
        }
        if verdict.account_details.app_licensing_verdict != "LICENSED" {
            return Err(Refusal::License);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
