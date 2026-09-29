//! Suite bytes on a Blossom server.
//!
//! A published suite is a NIP-EXT release whose manifest lists its files
//! by digest. The files themselves travel as content-addressed blobs on
//! the relay's Blossom server (`PUT /upload` with a NIP-98 authorization,
//! `GET /<sha256>`), so a trainer checking a result fetches exactly the
//! bytes the manifest names and verifies each one before using it.

use std::time::Duration;

use base64::Engine as _;
use nostr::domain::{RelaySigner, Tag};

/// The NIP-98 HTTP authorization kind the relay's Blossom server takes.
pub const HTTP_AUTH_KIND: u16 = 27_235;
/// The largest blob fetched.
pub const MAX_BLOB: u64 = 16 * 1024 * 1024;
/// How long an upload waits out a server's rate limit (`429`) in all
/// before it gives up. A relay limits uploads per key per minute, and a
/// suite of a few dozen files meets that limit.
pub const RATE_PATIENCE: Duration = Duration::from_secs(180);

/// A Blossom server.
pub struct Blossom {
    base: String,
    client: reqwest::blocking::Client,
}

fn hex(digest: &str) -> &str {
    digest.strip_prefix("sha256:").unwrap_or(digest)
}

impl Blossom {
    /// The server at `base`, such as `https://relay.openagents.com`.
    ///
    /// # Errors
    ///
    /// Returns why the HTTP client can't be built.
    pub fn new(base: &str) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            client,
        })
    }

    /// The Blossom server a relay serves on its own HTTP listener:
    /// `ws://` becomes `http://` and `wss://` becomes `https://`.
    ///
    /// # Errors
    ///
    /// Returns why the relay URL has no HTTP form.
    pub fn for_relay(relay: &str) -> Result<Self, String> {
        let base = if let Some(rest) = relay.strip_prefix("wss://") {
            format!("https://{rest}")
        } else if let Some(rest) = relay.strip_prefix("ws://") {
            format!("http://{rest}")
        } else if relay.starts_with("http://") || relay.starts_with("https://") {
            relay.to_string()
        } else {
            return Err(format!("{relay} is not a relay URL"));
        };
        let base = match base.find("://").map(|at| at + 3) {
            Some(start) => match base[start..].find('/') {
                Some(end) => base[..start + end].to_string(),
                None => base,
            },
            None => base,
        };
        Self::new(&base)
    }

    /// The server's base URL.
    #[must_use]
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Whether the server holds the blob.
    #[must_use]
    pub fn has(&self, digest: &str) -> bool {
        self.client
            .head(format!("{}/{}", self.base, hex(digest)))
            .send()
            .is_ok_and(|response| response.status().is_success())
    }

    /// Uploads `bytes` unless the server already holds them.
    ///
    /// # Errors
    ///
    /// Returns the server's refusal.
    pub fn upload(
        &self,
        signer: &RelaySigner,
        bytes: &[u8],
        media_type: &str,
        now: u64,
    ) -> Result<(), String> {
        let digest = nostr::contracts::digest_bytes(bytes);
        if self.has(&digest) {
            return Ok(());
        }
        let url = format!("{}/upload", self.base);
        let started = std::time::Instant::now();
        let mut wait = Duration::from_secs(5);
        loop {
            // A fresh authorization each attempt: the server takes each one
            // once, and a waited-out attempt may be past its time window.
            let at = now + started.elapsed().as_secs();
            let auth = signer.sign(
                at,
                HTTP_AUTH_KIND,
                vec![
                    Tag::new(vec!["u".into(), url.clone()]),
                    Tag::new(vec!["method".into(), "PUT".into()]),
                    Tag::new(vec!["payload".into(), hex(&digest).to_string()]),
                ],
                String::new(),
            );
            let header = format!(
                "Nostr {}",
                base64::engine::general_purpose::STANDARD
                    .encode(serde_json::to_vec(&auth).map_err(|error| error.to_string())?)
            );
            let response = self
                .client
                .put(url.clone())
                .header("authorization", header)
                .header("x-sha-256", hex(&digest))
                .header("content-type", media_type)
                .body(bytes.to_vec())
                .send()
                .map_err(|error| format!("{}: {}", self.base, error.without_url()))?;
            let status = response.status();
            if status.is_success() {
                return Ok(());
            }
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS
                && started.elapsed() + wait <= RATE_PATIENCE
            {
                std::thread::sleep(wait);
                wait = (wait * 2).min(Duration::from_secs(30));
                continue;
            }
            let text = response.text().unwrap_or_default();
            return Err(format!(
                "{} refused the upload ({status}): {}",
                self.base,
                text.trim()
            ));
        }
    }

    /// Fetches the blob `digest` names and checks its bytes.
    ///
    /// # Errors
    ///
    /// Returns why the blob can't be fetched or doesn't match.
    pub fn fetch(&self, digest: &str) -> Result<Vec<u8>, String> {
        let response = self
            .client
            .get(format!("{}/{}", self.base, hex(digest)))
            .send()
            .map_err(|error| format!("{}: {}", self.base, error.without_url()))?;
        if !response.status().is_success() {
            return Err(format!(
                "{} has no blob {digest} ({})",
                self.base,
                response.status()
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_BLOB)
        {
            return Err(format!("blob {digest} is over {MAX_BLOB} bytes"));
        }
        let bytes = response
            .bytes()
            .map_err(|error| error.without_url().to_string())?;
        if nostr::contracts::digest_bytes(&bytes) != format!("sha256:{}", hex(digest)) {
            return Err(format!("blob {digest} does not match its digest"));
        }
        Ok(bytes.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relay_url_maps_to_its_http_listener() {
        assert_eq!(
            Blossom::for_relay("wss://relay.openagents.com")
                .unwrap()
                .base(),
            "https://relay.openagents.com"
        );
        assert_eq!(
            Blossom::for_relay("ws://127.0.0.1:7777/path")
                .unwrap()
                .base(),
            "http://127.0.0.1:7777"
        );
        assert!(Blossom::for_relay("relay").is_err());
    }
}
