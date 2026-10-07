//! The `verse-assets` broker: it grants a five-minute signed URL for one
//! private pack to a reader the pack's manifest names.
//!
//! [`Broker::grant`] is the whole decision, over an injected [`Bucket`] (the
//! manifests) and [`UrlSigner`] (the service account's `signBlob`), so tests
//! run it with fakes. [`gcp`] holds the Cloud Run implementations, which use
//! the metadata server's token for the service account: no key file exists.
//! [`router`] is the HTTP surface.
//!
//! Every refusal of an unauthenticated or unauthorized request is the same
//! `403`, so a stranger learns nothing about which assets exist.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use crate::auth::{Grant, GrantRequest, MAX_REQUEST_BYTES, URL_SECONDS, grant_url};
use crate::manifest::Manifest;
use crate::signed_url::Draft;
use crate::{manifest_object, pack_object};

/// How long a request's event ID is remembered, s: past NIP-98's 60-second
/// window on either side.
const REPLAY_SECONDS: u64 = 150;
/// Most remembered event IDs, which bounds the memory a flood can take.
const MAX_SEEN: usize = 10_000;

/// Reads manifests from the private bucket.
pub trait Bucket: Send + Sync + 'static {
    /// The bytes of `object`, or `None` when it doesn't exist.
    fn read(&self, object: &str) -> impl Future<Output = Result<Option<Vec<u8>>, String>> + Send;
}

/// Signs with the broker's service account.
pub trait UrlSigner: Send + Sync + 'static {
    /// The service account's email, which names its key in a signed URL.
    fn email(&self) -> impl Future<Output = Result<String, String>> + Send;
    /// An RSA-SHA256 signature over `bytes`.
    fn sign(&self, bytes: &[u8]) -> impl Future<Output = Result<Vec<u8>, String>> + Send;
}

/// Why a request got no grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Unauthenticated, replayed, not a reader, or no such asset: `403`.
    Forbidden,
    /// An authenticated request whose body isn't a grant request: `400`.
    BadRequest,
    /// The bucket, the manifest, or the signer failed: `503`.
    Unavailable,
}

impl Refusal {
    /// The HTTP status.
    #[must_use]
    pub fn status(self) -> u16 {
        match self {
            Self::Forbidden => 403,
            Self::BadRequest => 400,
            Self::Unavailable => 503,
        }
    }
}

/// The broker over its bucket and signer.
pub struct Broker<B, S> {
    bucket: String,
    url: String,
    store: B,
    signer: S,
    seen: Mutex<HashMap<String, u64>>,
}

impl<B: Bucket, S: UrlSigner> Broker<B, S> {
    /// A broker for `bucket` whose public origin is `origin`, the URL every
    /// request's NIP-98 event must name.
    pub fn new(bucket: &str, origin: &str, store: B, signer: S) -> Self {
        Self {
            bucket: bucket.to_owned(),
            url: grant_url(origin),
            store,
            signer,
            seen: Mutex::new(HashMap::new()),
        }
    }

    /// The URL requests must sign.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    fn first_use(&self, id: &str, now: u64) -> bool {
        let mut seen = self.seen.lock().unwrap_or_else(|e| e.into_inner());
        seen.retain(|_, at| now.saturating_sub(*at) <= REPLAY_SECONDS);
        if seen.contains_key(id) || seen.len() >= MAX_SEEN {
            return false;
        }
        seen.insert(id.to_owned(), now);
        true
    }

    /// Decides one request: its `Authorization` header and body at `now`.
    /// On a grant, returns it with the reader's public key, for the log.
    ///
    /// # Errors
    ///
    /// Returns the [`Refusal`].
    pub async fn grant(
        &self,
        authorization: Option<&str>,
        body: &[u8],
        now: u64,
    ) -> Result<(Grant, String), Refusal> {
        if body.len() > MAX_REQUEST_BYTES {
            return Err(Refusal::Forbidden);
        }
        let header = authorization.ok_or(Refusal::Forbidden)?;
        let auth = nostr::domain::parse_http_authorization(header, "POST", &self.url, body, now)
            .map_err(|_| Refusal::Forbidden)?;
        if !self.first_use(&auth.event_id, now) {
            return Err(Refusal::Forbidden);
        }
        let request: GrantRequest =
            serde_json::from_slice(body).map_err(|_| Refusal::BadRequest)?;
        let object = manifest_object(&request.name).ok_or(Refusal::Forbidden)?;
        let pack = pack_object(&request.sha256).ok_or(Refusal::Forbidden)?;
        let bytes = self
            .store
            .read(&object)
            .await
            .map_err(|_| Refusal::Unavailable)?
            .ok_or(Refusal::Forbidden)?;
        let manifest = Manifest::parse(&bytes).map_err(|_| Refusal::Unavailable)?;
        if manifest.name != request.name {
            return Err(Refusal::Unavailable);
        }
        if !manifest.admits(&auth.pubkey) || manifest.pack.sha256 != request.sha256 {
            return Err(Refusal::Forbidden);
        }
        let email = self
            .signer
            .email()
            .await
            .map_err(|_| Refusal::Unavailable)?;
        let draft = Draft::new(&self.bucket, &pack, &email, now, URL_SECONDS);
        let signature = self
            .signer
            .sign(draft.string_to_sign())
            .await
            .map_err(|_| Refusal::Unavailable)?;
        Ok((
            Grant {
                url: draft.finish(&signature),
                sha256: manifest.pack.sha256,
                bytes: manifest.pack.bytes,
                expires: now + u64::from(URL_SECONDS),
            },
            auth.pubkey,
        ))
    }
}

/// The HTTP surface: `POST /v1/private/url` and `GET /health` (Cloud Run reserves `/healthz`).
pub fn router<B: Bucket, S: UrlSigner>(broker: Arc<Broker<B, S>>) -> axum::Router {
    use axum::extract::{DefaultBodyLimit, State};
    use axum::http::{HeaderMap, StatusCode, header};
    use axum::response::IntoResponse;
    use axum::routing::{get, post};

    async fn grant<B: Bucket, S: UrlSigner>(
        State(broker): State<Arc<Broker<B, S>>>,
        headers: HeaderMap,
        body: axum::body::Bytes,
    ) -> axum::response::Response {
        let authorization = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let json = [(header::CONTENT_TYPE, "application/json")];
        let no_store = [(header::CACHE_CONTROL, "no-store")];
        match broker.grant(authorization, &body, now).await {
            Ok((grant, reader)) => {
                // The URL is a bearer credential: never logged.
                eprintln!(
                    "verse-assets: granted {} to {}",
                    &grant.sha256[..12],
                    &reader[..12.min(reader.len())]
                );
                let body = serde_json::to_vec(&grant).unwrap_or_default();
                (StatusCode::OK, json, no_store, body).into_response()
            }
            Err(refusal) => {
                eprintln!("verse-assets: refused ({})", refusal.status());
                let status =
                    StatusCode::from_u16(refusal.status()).unwrap_or(StatusCode::FORBIDDEN);
                let body = match refusal {
                    Refusal::Forbidden => &br#"{"error":"forbidden"}"#[..],
                    Refusal::BadRequest => br#"{"error":"bad request"}"#,
                    Refusal::Unavailable => br#"{"error":"unavailable"}"#,
                };
                (status, json, no_store, body.to_vec()).into_response()
            }
        }
    }

    axum::Router::new()
        .route(crate::auth::ROUTE, post(grant::<B, S>))
        .route("/health", get(|| async { "ok" }))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .with_state(broker)
}

/// The Cloud Run implementations, through the metadata server.
pub mod gcp {
    use std::time::{Duration, Instant};

    use base64::Engine as _;
    use tokio::sync::Mutex;

    const METADATA: &str =
        "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default";

    /// The service account's access token and email, from the metadata
    /// server, cached until a minute before the token expires.
    pub struct Metadata {
        http: reqwest::Client,
        token: Mutex<Option<(String, Instant)>>,
        email: Mutex<Option<String>>,
    }

    impl Metadata {
        /// A client for the metadata server.
        ///
        /// # Errors
        ///
        /// Returns a message when the HTTP client can't be built.
        pub fn new() -> Result<Self, String> {
            let http = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())?;
            Ok(Self {
                http,
                token: Mutex::new(None),
                email: Mutex::new(None),
            })
        }

        async fn get(&self, path: &str) -> Result<reqwest::Response, String> {
            let response = self
                .http
                .get(format!("{METADATA}/{path}"))
                .header("Metadata-Flavor", "Google")
                .send()
                .await
                .map_err(|e| format!("metadata: {e}"))?;
            if !response.status().is_success() {
                return Err(format!("metadata: {}", response.status()));
            }
            Ok(response)
        }

        /// A current access token.
        ///
        /// # Errors
        ///
        /// Returns a message when the metadata server doesn't answer.
        pub async fn token(&self) -> Result<String, String> {
            let mut held = self.token.lock().await;
            if let Some((token, until)) = held.as_ref()
                && Instant::now() < *until
            {
                return Ok(token.clone());
            }
            #[derive(serde::Deserialize)]
            struct Token {
                access_token: String,
                expires_in: u64,
            }
            let token: Token = self
                .get("token")
                .await?
                .json()
                .await
                .map_err(|e| format!("metadata token: {e}"))?;
            let until = Instant::now() + Duration::from_secs(token.expires_in.saturating_sub(60));
            *held = Some((token.access_token.clone(), until));
            Ok(token.access_token)
        }

        /// The service account's email.
        ///
        /// # Errors
        ///
        /// Returns a message when the metadata server doesn't answer.
        pub async fn email(&self) -> Result<String, String> {
            let mut held = self.email.lock().await;
            if let Some(email) = held.as_ref() {
                return Ok(email.clone());
            }
            let email = self
                .get("email")
                .await?
                .text()
                .await
                .map_err(|e| format!("metadata email: {e}"))?
                .trim()
                .to_owned();
            *held = Some(email.clone());
            Ok(email)
        }

        fn http(&self) -> &reqwest::Client {
            &self.http
        }
    }

    /// The private bucket, read through the JSON API.
    pub struct Gcs {
        pub metadata: std::sync::Arc<Metadata>,
        pub bucket: String,
    }

    impl super::Bucket for Gcs {
        async fn read(&self, object: &str) -> Result<Option<Vec<u8>>, String> {
            let token = self.metadata.token().await?;
            let encoded: String = object
                .bytes()
                .map(|b| {
                    if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                        (b as char).to_string()
                    } else {
                        format!("%{b:02X}")
                    }
                })
                .collect();
            let response = self
                .metadata
                .http()
                .get(format!(
                    "https://storage.googleapis.com/storage/v1/b/{}/o/{encoded}?alt=media",
                    self.bucket
                ))
                .bearer_auth(token)
                .send()
                .await
                .map_err(|e| format!("storage: {e}"))?;
            match response.status().as_u16() {
                404 => Ok(None),
                200 => {
                    let bytes = response
                        .bytes()
                        .await
                        .map_err(|e| format!("storage: {e}"))?;
                    if bytes.len() > crate::MAX_MANIFEST_BYTES {
                        return Err("storage: manifest is too long".into());
                    }
                    Ok(Some(bytes.to_vec()))
                }
                status => Err(format!("storage: {status}")),
            }
        }
    }

    /// The service account's own `signBlob`, which needs
    /// `roles/iam.serviceAccountTokenCreator` on itself.
    pub struct IamSigner {
        pub metadata: std::sync::Arc<Metadata>,
    }

    impl super::UrlSigner for IamSigner {
        async fn email(&self) -> Result<String, String> {
            self.metadata.email().await
        }

        async fn sign(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
            let email = self.metadata.email().await?;
            let token = self.metadata.token().await?;
            let engine = base64::engine::general_purpose::STANDARD;
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Signed {
                signed_blob: String,
            }
            let response = self
                .metadata
                .http()
                .post(format!(
                    "https://iamcredentials.googleapis.com/v1/projects/-/serviceAccounts/{email}:signBlob"
                ))
                .bearer_auth(token)
                .json(&serde_json::json!({ "payload": engine.encode(bytes) }))
                .send()
                .await
                .map_err(|e| format!("signBlob: {e}"))?;
            if !response.status().is_success() {
                return Err(format!("signBlob: {}", response.status()));
            }
            let signed: Signed = response
                .json()
                .await
                .map_err(|e| format!("signBlob: {e}"))?;
            engine
                .decode(signed.signed_blob)
                .map_err(|e| format!("signBlob: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use nostr::domain::RelaySigner;

    use super::*;
    use crate::auth::authorization;
    use crate::manifest::tests::sample;

    /// A bucket of objects in memory.
    struct FakeBucket(BTreeMap<String, Vec<u8>>);

    impl Bucket for FakeBucket {
        async fn read(&self, object: &str) -> Result<Option<Vec<u8>>, String> {
            Ok(self.0.get(object).cloned())
        }
    }

    /// A signer that "signs" with the SHA-256 of what it is given.
    struct FakeSigner;

    impl UrlSigner for FakeSigner {
        async fn email(&self) -> Result<String, String> {
            Ok("broker@test.iam.gserviceaccount.com".into())
        }
        async fn sign(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
            use sha2::{Digest, Sha256};
            Ok(Sha256::digest(bytes).to_vec())
        }
    }

    const NOW: u64 = 1_791_324_930;

    fn broker(readers: &[&str]) -> Broker<FakeBucket, FakeSigner> {
        let manifest = sample(readers);
        let mut objects = BTreeMap::new();
        objects.insert(
            "manifests/sample-guest.json".to_owned(),
            manifest.to_bytes().unwrap(),
        );
        Broker::new(
            "private-bucket",
            "https://broker.example",
            FakeBucket(objects),
            FakeSigner,
        )
    }

    fn request(signer: &RelaySigner, name: &str, sha256: &str, at: u64) -> (String, Vec<u8>) {
        let body = GrantRequest {
            name: name.into(),
            sha256: sha256.into(),
        }
        .body();
        let header = authorization(signer, "https://broker.example/v1/private/url", &body, at);
        (header, body)
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn a_reader_gets_a_short_lived_url_for_the_pack_only() {
        let reader = RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
        let broker = broker(&[reader.pubkey()]);
        let (header, body) = request(&reader, "sample-guest", &"cd".repeat(32), NOW);
        let (grant, who) = block_on(broker.grant(Some(&header), &body, NOW)).unwrap();
        assert_eq!(who, reader.pubkey());
        assert_eq!(grant.sha256, "cd".repeat(32));
        assert_eq!(grant.bytes, 1234);
        assert_eq!(grant.expires, NOW + 300);
        let pack = format!("/private-bucket/packs/{}.vtp?", "cd".repeat(32));
        assert!(
            grant
                .url
                .starts_with(&format!("https://storage.googleapis.com{pack}"))
        );
        assert!(grant.url.contains("X-Goog-Expires=300"));
        assert!(grant.url.contains("X-Goog-Signature="));
        // The same signed request again is a replay.
        assert_eq!(
            block_on(broker.grant(Some(&header), &body, NOW + 1)),
            Err(Refusal::Forbidden)
        );
    }

    #[test]
    fn strangers_and_bad_requests_get_the_same_refusal() {
        let reader = RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
        let stranger = RelaySigner::from_secret_hex(&"22".repeat(32)).unwrap();
        let broker = broker(&[reader.pubkey()]);
        let refused = |signer: &RelaySigner, name: &str, sha: &str, at: u64| {
            let (header, body) = request(signer, name, sha, at);
            block_on(broker.grant(Some(&header), &body, NOW)).map(|_| ())
        };
        let sha = "cd".repeat(32);
        // Not a reader.
        assert_eq!(
            refused(&stranger, "sample-guest", &sha, NOW),
            Err(Refusal::Forbidden)
        );
        // No such asset, and a name that is no registry name.
        assert_eq!(
            refused(&reader, "missing", &sha, NOW),
            Err(Refusal::Forbidden)
        );
        assert_eq!(
            refused(&reader, "../vendor", &sha, NOW),
            Err(Refusal::Forbidden)
        );
        // Another digest than the manifest's, or no digest at all.
        assert_eq!(
            refused(&reader, "sample-guest", &"ef".repeat(32), NOW),
            Err(Refusal::Forbidden)
        );
        assert_eq!(
            refused(&reader, "sample-guest", "x", NOW),
            Err(Refusal::Forbidden)
        );
        // Signed too long ago.
        assert_eq!(
            refused(&reader, "sample-guest", &sha, NOW - 120),
            Err(Refusal::Forbidden)
        );
        // No header, or one signed for another broker.
        let (_, body) = request(&reader, "sample-guest", &sha, NOW);
        assert_eq!(
            block_on(broker.grant(None, &body, NOW)),
            Err(Refusal::Forbidden)
        );
        let other = authorization(
            &reader,
            "https://elsewhere.example/v1/private/url",
            &body,
            NOW,
        );
        assert_eq!(
            block_on(broker.grant(Some(&other), &body, NOW)),
            Err(Refusal::Forbidden)
        );
        // An authenticated body that isn't a grant request.
        let junk = b"{\"asset\":1}";
        let header = authorization(&reader, broker.url(), junk, NOW);
        assert_eq!(
            block_on(broker.grant(Some(&header), junk, NOW)),
            Err(Refusal::BadRequest)
        );
    }

    #[test]
    fn a_revoked_reader_is_refused_on_the_next_request() {
        let reader = RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
        let broker = broker(&[]);
        let (header, body) = request(&reader, "sample-guest", &"cd".repeat(32), NOW);
        assert_eq!(
            block_on(broker.grant(Some(&header), &body, NOW)).map(|_| ()),
            Err(Refusal::Forbidden)
        );
    }
}
