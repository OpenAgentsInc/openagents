//! Sign in with a Nostr key (#11993): a NIP-98 event (kind 27235) the
//! caller's signer makes over `POST /v1/device/nostr` becomes the same
//! app session a device sign-in issues. The SDK never holds the key: the
//! caller passes a [`NostrSigner`] (a key in the platform's keychain, a
//! NIP-46 bunker, a browser extension) that signs one event at a time.

use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use url::Url;

use super::{CredentialRecord, Session, TokenAnswer};

mod web;
pub use web::{SCAN_PATH, ScannedLogin, ScannedLoginAbout, ScannedLoginApproved};
use crate::{Client, Error};

/// The largest sign-in body the server takes (`oa_auth::nostr::BODY_MAX`).
pub const KEY_BODY_MAX: usize = 16 * 1024;

/// The proof-of-work difficulty a new key's first sign-in needs by default
/// (#11994): leading zero bits of `SHA-256(challenge ":" solution)`.
pub const POW_BITS: u8 = 20;

/// Solve the sign-up proof of work over `challenge`: the first counter, in
/// decimal, whose `SHA-256(challenge ":" counter)` starts with `bits` zero
/// bits. About a million hashes at 20 bits: run it off the UI thread.
pub fn solve_pow(challenge: &str, bits: u8) -> String {
    let mut counter: u64 = 0;
    loop {
        let solution = counter.to_string();
        let digest = Sha256::digest(format!("{challenge}:{solution}").as_bytes());
        if leading_zero_bits(&digest) >= u32::from(bits) {
            return solution;
        }
        counter += 1;
    }
}

fn leading_zero_bits(bytes: &[u8]) -> u32 {
    let mut bits = 0;
    for byte in bytes {
        if *byte == 0 {
            bits += 8;
        } else {
            return bits + byte.leading_zeros();
        }
    }
    bits
}

/// The NIP-98 HTTP authorization kind.
pub const HTTP_AUTH_KIND: u16 = 27_235;

/// An unsigned NIP-01 event for the signer to complete with `id`,
/// `pubkey`, and `sig`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EventTemplate {
    pub created_at: u64,
    pub kind: u16,
    pub tags: Vec<Vec<String>>,
    pub content: String,
}

/// Why a signer didn't sign.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignerError;

impl fmt::Display for SignerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Your key couldn't sign. Try again.")
    }
}
impl std::error::Error for SignerError {}

/// Signs events with the caller's key. The SDK only asks for signatures.
pub trait NostrSigner: Send + Sync {
    /// Sign `template`, answering the complete event as NIP-01 JSON
    /// (`{"id", "pubkey", "created_at", "kind", "tags", "content", "sig"}`).
    fn sign_event(&self, template: &EventTemplate) -> Result<String, SignerError>;
}

/// A completed key sign-in.
#[derive(Debug)]
pub struct KeySignIn {
    pub session: Session,
    /// Whether this sign-in made the account.
    pub created: bool,
    /// The signing key, 64 lowercase hex characters.
    pub pubkey: String,
}

/// One of the account's keys.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct LinkedKey {
    pub pubkey: String,
    pub npub: String,
}

#[derive(Deserialize)]
struct Challenge {
    challenge: String,
}

#[derive(Deserialize)]
struct Keys {
    keys: Vec<LinkedKey>,
}

#[derive(Deserialize)]
struct SignedIn {
    #[serde(flatten)]
    token: TokenAnswer,
    #[serde(default)]
    created: bool,
    #[serde(default)]
    pubkey: String,
}

impl Client {
    fn api_url(&self, segments: &[&str]) -> Result<Url, Error> {
        let mut url = self.deployment.api_base().clone();
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|_| Error::InvalidDeployment)?;
            path.pop_if_empty();
            for segment in segments {
                path.push(segment);
            }
        }
        Ok(url)
    }

    fn http_auth(
        signer: &dyn NostrSigner,
        url: &Url,
        challenge: &str,
        payload: Option<&[u8]>,
    ) -> Result<String, Error> {
        let mut tags = vec![
            vec!["u".to_string(), url.to_string()],
            vec!["method".to_string(), "POST".to_string()],
            vec!["challenge".to_string(), challenge.to_string()],
        ];
        if let Some(body) = payload {
            let hash: String = Sha256::digest(body)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            tags.push(vec!["payload".to_string(), hash]);
        }
        let event = signer
            .sign_event(&EventTemplate {
                created_at: crate::now(),
                kind: HTTP_AUTH_KIND,
                tags,
                content: String::new(),
            })
            .map_err(|_| Error::InvalidCredential)?;
        if event.len() > 6 * 1024 || serde_json::from_str::<Value>(&event).is_err() {
            return Err(Error::InvalidCredential);
        }
        Ok(STANDARD.encode(event))
    }

    async fn challenge(&self, segments: &[&str], token: Option<&str>) -> Result<String, Error> {
        let answer: Challenge = self
            .request(Method::GET, segments, &[], None, token)
            .await?;
        if answer.challenge.is_empty() || answer.challenge.len() > 256 {
            return Err(Error::InvalidResponse);
        }
        Ok(answer.challenge)
    }

    /// Sign in with the key behind `signer`: fetch a challenge, sign one
    /// request, and receive an app session. A key that already has an
    /// account signs in this way; a new key may be asked for a sign-up proof
    /// first (`Refused { code: Some("proof_required"), .. }`, #11994), which
    /// [`Client::sign_in_with_key_on`] sends. Sends once; the caller stores
    /// the session as it stores a device sign-in's, and signs in again
    /// before it expires.
    pub async fn sign_in_with_key(
        &self,
        signer: &dyn NostrSigner,
        app: &str,
        computer: &str,
    ) -> Result<KeySignIn, Error> {
        let challenge = self.key_challenge().await?;
        self.sign_in_with_key_on(signer, &challenge, app, computer, serde_json::Map::new())
            .await
    }

    /// A fresh sign-in challenge, for a sign-in that needs a proof over it.
    pub async fn key_challenge(&self) -> Result<String, Error> {
        self.challenge(&["device", "nostr", "challenge"], None)
            .await
    }

    /// Sign in over `challenge` (from [`Client::key_challenge`]), adding
    /// `proof` to the body: `{"pow": solve_pow(challenge, POW_BITS)}`, or
    /// `{"app_attest": {"key_id", "attestation"}}` from App Attest over
    /// `SHA-256(challenge)`. The body stays under [`KEY_BODY_MAX`].
    pub async fn sign_in_with_key_on(
        &self,
        signer: &dyn NostrSigner,
        challenge: &str,
        app: &str,
        computer: &str,
        proof: serde_json::Map<String, Value>,
    ) -> Result<KeySignIn, Error> {
        let mut fields = proof;
        fields.insert("app".into(), json!(app));
        fields.insert("computer".into(), json!(computer));
        let url = self.api_url(&["device", "nostr"])?;
        let body =
            serde_json::to_vec(&Value::Object(fields)).map_err(|_| Error::InvalidCredential)?;
        if body.len() > KEY_BODY_MAX {
            return Err(Error::InvalidCredential);
        }
        let authorization = Self::http_auth(signer, &url, challenge, Some(&body))?;
        let response = self
            .http
            .post(url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Nostr {authorization}"),
            )
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|error| Error::Network {
                timeout: error.is_timeout(),
            })?;
        let answer: SignedIn = match Self::check_response(response).await {
            Ok(response) => Self::decode(response, &["device", "nostr"], 64 * 1024).await?,
            Err(Error::Unauthorized) => return Err(Error::KeyDenied),
            Err(error) => return Err(error),
        };
        answer.token.checked()?;
        Ok(KeySignIn {
            session: Session {
                record: CredentialRecord {
                    deployment: self.deployment.clone(),
                    account: answer.token.account,
                    expires_at: Some(crate::now().saturating_add(answer.token.expires_in)),
                    credential: answer.token.access_token,
                },
            },
            created: answer.created,
            pubkey: answer.pubkey,
        })
    }

    /// Add the key behind `new_key` to the signed-in account; `new_key`
    /// signs a proof bound to this account. Answers the account's keys.
    /// A key that already signs in to another account is refused
    /// (`Refused { status: 409, code: Some("identity_taken") }`).
    pub async fn link_key(
        &self,
        session: &Session,
        new_key: &dyn NostrSigner,
    ) -> Result<Vec<LinkedKey>, Error> {
        self.check_session(session)?;
        let challenge = self
            .challenge(&["device", "keys", "challenge"], Some(session.token()))
            .await?;
        let url = self.api_url(&["device", "keys"])?;
        let proof = Self::http_auth(new_key, &url, &challenge, None)?;
        let answer: Keys = match self
            .request(
                Method::POST,
                &["device", "keys"],
                &[],
                Some(json!({"proof": proof})),
                Some(session.token()),
            )
            .await
        {
            Err(Error::Refused {
                code: Some("key_proof_denied"),
                ..
            }) => return Err(Error::KeyDenied),
            other => other?,
        };
        Ok(answer.keys)
    }

    /// The signed-in account's keys, oldest first.
    pub async fn account_keys(&self, session: &Session) -> Result<Vec<LinkedKey>, Error> {
        self.check_session(session)?;
        let answer: Keys = self
            .request(
                Method::GET,
                &["device", "keys"],
                &[],
                None,
                Some(session.token()),
            )
            .await?;
        Ok(answer.keys)
    }

    /// Remove one of the account's keys (hex or `npub`). The last key is
    /// never removed (`Refused { status: 409, code: Some("last_key") }`).
    pub async fn remove_key(
        &self,
        session: &Session,
        pubkey: &str,
    ) -> Result<Vec<LinkedKey>, Error> {
        self.check_session(session)?;
        if pubkey.is_empty()
            || pubkey.len() > 128
            || !pubkey.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            return Err(Error::InvalidReference);
        }
        let answer: Keys = self
            .request(
                Method::DELETE,
                &["device", "keys", pubkey],
                &[],
                None,
                Some(session.token()),
            )
            .await?;
        Ok(answer.keys)
    }
}

#[cfg(test)]
mod pow_tests {
    use super::*;

    #[test]
    fn a_solved_pow_has_the_leading_zero_bits() {
        let solution = solve_pow("c0ffee", 12);
        let digest = Sha256::digest(format!("c0ffee:{solution}").as_bytes());
        assert!(leading_zero_bits(&digest) >= 12);
        assert!(solution.len() <= 64);
        assert_eq!(leading_zero_bits(&[0, 0x10]), 11);
    }
}
