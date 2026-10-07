//! Asks a broker for a grant, blocking: HTTPS only, no redirects, bounded.

use std::io::Read;
use std::time::Duration;

use nostr::domain::RelaySigner;

use crate::auth::{Grant, GrantRequest, authorization, grant_url};

const MAX_GRANT_BYTES: u64 = 8 * 1024;

/// Why a grant request failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrantError {
    /// The broker refused this key: not a reader, or no such asset.
    Refused,
    /// The broker, the network, or the reply failed.
    Unavailable(String),
}

impl std::fmt::Display for GrantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused => f.write_str("the broker refused this key"),
            Self::Unavailable(why) => write!(f, "the broker is unavailable: {why}"),
        }
    }
}

/// Asks `broker` (an `https://` origin) for `name`'s pack `sha256`, signed
/// by `signer` at `now`.
///
/// # Errors
///
/// Returns [`GrantError::Refused`] on a `403`, and
/// [`GrantError::Unavailable`] otherwise.
pub fn request_grant(
    broker: &str,
    signer: &RelaySigner,
    name: &str,
    sha256: &str,
    now: u64,
) -> Result<Grant, GrantError> {
    let unavailable = |why: &str| GrantError::Unavailable(why.to_owned());
    if !broker.starts_with("https://") {
        return Err(unavailable("the broker must use HTTPS"));
    }
    let url = grant_url(broker);
    let body = GrantRequest {
        name: name.into(),
        sha256: sha256.into(),
    }
    .body();
    let header = authorization(signer, &url, &body, now);
    let client = reqwest::blocking::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| unavailable("the HTTP client could not start"))?;
    let response = client
        .post(&url)
        .header("Authorization", header)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .map_err(|_| unavailable("could not connect"))?;
    match response.status().as_u16() {
        200 => {}
        403 => return Err(GrantError::Refused),
        status => return Err(GrantError::Unavailable(format!("status {status}"))),
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_GRANT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| unavailable("the reply was interrupted"))?;
    if bytes.len() as u64 > MAX_GRANT_BYTES {
        return Err(unavailable("the reply is too long"));
    }
    let grant: Grant =
        serde_json::from_slice(&bytes).map_err(|_| unavailable("the reply is not a grant"))?;
    if grant.sha256 != sha256 || !grant.url.starts_with("https://") {
        return Err(unavailable("the grant names another pack"));
    }
    Ok(grant)
}
