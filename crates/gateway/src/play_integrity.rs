//! Bounded Google decode transport for the existing key-account gate.
//! No app-supplied decoded verdict, implicit credential, or provider URL is trusted.
use inference::upstream::google::TokenSource;
use oa_auth::play_integrity::Policy;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

const SCOPE: &str = "https://www.googleapis.com/auth/playintegrity";
const MAX_TOKEN: usize = 12 * 1024;
const MAX_RESPONSE: usize = 32 * 1024;
const REPLAY_SECONDS: u64 = 600;

/// Explicit opt-in; no credential is selected from the process environment.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub policy: Policy,
    #[serde(default)]
    pub service_account_file: Option<PathBuf>,
    /// An operator-refreshed OAuth token with the Play Integrity scope.
    #[serde(default)]
    pub access_token_file: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Error {
    Rejected,
    Unavailable,
    Replayed,
}

pub(crate) trait Decoder: Send + Sync {
    /// Called only after the key's signature and challenge are checked,
    /// from a blocking task, never from an async executor worker.
    fn decode(&self, token: &str) -> Result<Value, Error>;
}

struct Google {
    http: reqwest::Client,
    tokens: TokenSource,
    url: String,
}
impl Decoder for Google {
    fn decode(&self, token: &str) -> Result<Value, Error> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|_| Error::Unavailable)?;
        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(10), async {
                let access = self.tokens.token().await.map_err(|_| Error::Unavailable)?;
                let mut response = self
                    .http
                    .post(&self.url)
                    .bearer_auth(access.expose())
                    .json(&json!({"integrity_token": token}))
                    .send()
                    .await
                    .map_err(|_| Error::Unavailable)?;
                match response.status().as_u16() {
                    200 => {}
                    400 => return Err(Error::Rejected),
                    // Authorization faults, quotas, and outages are availability failures.
                    _ => return Err(Error::Unavailable),
                }
                if response
                    .content_length()
                    .is_some_and(|n| n > MAX_RESPONSE as u64)
                {
                    return Err(Error::Unavailable);
                }
                let mut bytes = Vec::new();
                while let Some(chunk) = response.chunk().await.map_err(|_| Error::Unavailable)? {
                    if bytes.len() + chunk.len() > MAX_RESPONSE {
                        return Err(Error::Unavailable);
                    }
                    bytes.extend_from_slice(&chunk);
                }
                let mut decoded: Value =
                    serde_json::from_slice(&bytes).map_err(|_| Error::Unavailable)?;
                decoded
                    .get_mut("tokenPayloadExternal")
                    .map(Value::take)
                    .ok_or(Error::Unavailable)
            })
            .await
            .map_err(|_| Error::Unavailable)?
        })
    }
}

pub(crate) struct Verifier {
    policy: Policy,
    decoder: Arc<dyn Decoder>,
    slots: tokio::sync::Semaphore,
    // Only token digests; no tokens or decoded device details are saved.
    seen: Mutex<HashMap<[u8; 32], u64>>,
}
impl Verifier {
    pub(crate) fn new(config: &Config) -> Result<Self, String> {
        config.policy.validate().map_err(str::to_owned)?;
        let tokens = match (&config.service_account_file, &config.access_token_file) {
            (Some(path), None) => TokenSource::service_account(path)?.with_scope(SCOPE),
            (None, Some(path)) => TokenSource::file(path.clone()),
            _ => return Err("Play Integrity needs exactly one explicit credential file".into()),
        };
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(8))
            .build()
            .map_err(|_| "cannot create the Play Integrity client".to_owned())?;
        let decoder = Google {
            http,
            tokens,
            url: format!(
                "https://playintegrity.googleapis.com/v1/{}:decodeIntegrityToken",
                config.policy.package_name
            ),
        };
        Ok(Self::with_decoder(config.policy.clone(), Arc::new(decoder)))
    }

    pub(crate) fn with_decoder(policy: Policy, decoder: Arc<dyn Decoder>) -> Self {
        Self {
            policy,
            decoder,
            slots: tokio::sync::Semaphore::new(4),
            seen: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn verify(
        &self,
        token: &str,
        challenge: &str,
        pubkey: &str,
        now: u64,
    ) -> Result<(), Error> {
        if token.is_empty()
            || token.len() > MAX_TOKEN
            || !token.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(Error::Rejected);
        }
        let _slot = self.slots.try_acquire().map_err(|_| Error::Unavailable)?;
        {
            let mut seen = self.seen.lock().map_err(|_| Error::Unavailable)?;
            seen.retain(|_, until| *until > now);
            let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
            if seen.contains_key(&digest) {
                return Err(Error::Replayed);
            }
            if seen.len() >= 4096 {
                return Err(Error::Unavailable);
            }
            // Even an unsuccessful decode consumes this submission. A retry
            // uses a new server challenge and a new platform token.
            seen.insert(digest, now.saturating_add(REPLAY_SECONDS));
        }
        let decoded = self.decoder.decode(token)?;
        self.policy
            .verify(
                &decoded,
                &oa_auth::play_integrity::request_hash(challenge, pubkey),
                now.saturating_mul(1000),
            )
            .map_err(|_| Error::Rejected)
    }
}

#[cfg(test)]
mod tests;
