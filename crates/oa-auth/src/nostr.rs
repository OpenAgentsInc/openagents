//! Sign in with a Nostr key (#11993): a signed NIP-98 event becomes the
//! same 30-day app session a device sign-in issues, so nothing downstream
//! changes. See `docs/audits/2026-10-17-nostr-only-identity.md`,
//! "Authenticating to our servers".
//!
//! | Route | Caller | Function |
//! | --- | --- | --- |
//! | `GET /v1/sessions/nostr/challenge` | anyone | [`NostrAuth::challenge`] with [`Purpose::SignIn`] |
//! | `POST /v1/sessions/nostr` | the app, `Authorization: Nostr <base64 event>` | [`NostrAuth::sign_in`] |
//! | `GET /v1/account/keys/challenge` | a session | [`NostrAuth::challenge`] with [`Purpose::Link`] |
//! | `POST /v1/account/keys` `{proof}` | a session | [`NostrAuth::link`] |
//! | `GET /v1/account/keys` | a session | [`NostrAuth::keys`] |
//! | `DELETE /v1/account/keys/{pubkey}` | a session | [`NostrAuth::unlink`] |
//!
//! The event is kind 27235 over the exact URL the app called (one of
//! [`SIGN_IN_PATHS`] or [`LINK_PATHS`] on a configured origin) and the
//! method, signed within 60 seconds, with a `challenge` tag holding a
//! challenge this service issued. A sign-in also carries a `payload` tag,
//! the SHA-256 (lowercase hex) of the exact body bytes. A challenge is an
//! HMAC over its expiry, a nonce, and what it is for (a sign-in, or adding
//! a key to one account), under a key kept beside the registry; nothing is
//! stored when one is issued. The event id and the challenge nonce are
//! burned in the session book in the same write that issues the session,
//! so each works once.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use ring::hmac;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tenancy::Accounts;
use tenancy::accounts::Refusal as AccountRefusal;
use tenancy::accounts::nostr::{keys_of, nostr_principal, valid_pubkey};
use tenancy::sessions::{self, AppLabel, Refusal as SessionRefusal, Sessions};
use tenancy::workspaces::UserId;

use crate::device::Answer;

/// The paths a sign-in event's `u` tag may name, on a configured origin:
/// the account service's own and the website's forwarder.
pub const SIGN_IN_PATHS: [&str; 2] = ["/v1/sessions/nostr", "/v1/device/nostr"];
/// The paths a key-link proof's `u` tag may name.
pub const LINK_PATHS: [&str; 2] = ["/v1/account/keys", "/v1/device/keys"];
/// The origins events are accepted for when a deployment names none.
pub const DEFAULT_ORIGINS: [&str; 2] = ["https://openagents.com", "https://api.openagents.com"];
/// How long a challenge stands, in seconds.
pub const CHALLENGE_TTL: u64 = 300;

/// The challenge key's file, beside the registry.
const KEY_FILE: &str = "nostr-challenge.key";
/// The longest `Authorization` header or proof accepted.
const HEADER_MAX: usize = 8 * 1024;
/// The longest sign-in body accepted.
pub const BODY_MAX: usize = 4 * 1024;

/// What a challenge is for. A challenge made for one purpose is refused
/// for any other, so a key-link proof made for one account can't add the
/// key to another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose<'a> {
    SignIn,
    /// Adding a key to this account.
    Link(&'a str),
}

impl Purpose<'_> {
    fn tag(self) -> String {
        match self {
            Self::SignIn => "sign-in".into(),
            Self::Link(account) => format!("link:{account}"),
        }
    }
}

/// Why a proof was refused. Never shown to people; the routes answer one
/// plain refusal for all of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejected {
    Malformed,
    /// The `u` tag names no accepted URL.
    Url,
    /// Signature, kind, time, URL, or method didn't check out.
    Event(String),
    /// The `payload` tag doesn't match the body.
    Payload,
    /// The challenge is missing, isn't ours, or is for something else.
    Challenge,
    /// The challenge expired.
    Expired,
}

/// A verified proof: who signed it, and what to burn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proof {
    pub pubkey: String,
    pub event_id: String,
    nonce: String,
    expires: u64,
}

/// The deployment's Nostr sign-in: its stores, the origins it accepts
/// events for, and its challenge key.
pub struct NostrAuth {
    dir: PathBuf,
    origins: Vec<String>,
    key: hmac::Key,
}

impl std::fmt::Debug for NostrAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NostrAuth")
            .field("origins", &self.origins)
            .finish_non_exhaustive()
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn lower_hex(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The challenge key: read, or made once with mode 0600.
fn challenge_key(dir: &Path) -> Result<[u8; 32], String> {
    let path = dir.join(KEY_FILE);
    if let Ok(bytes) = std::fs::read(&path) {
        return <[u8; 32]>::try_from(bytes.as_slice())
            .map_err(|_| format!("{} is not a 32-byte key", path.display()));
    }
    let mut key = [0_u8; 32];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut key)
        .map_err(|_| "no randomness for the challenge key".to_string())?;
    let temporary = dir.join(format!("{KEY_FILE}.{}.tmp", hex(&key[..4])));
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|e| format!("{}: {e}", temporary.display()))?;
        file.write_all(&key).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
    }
    // Another process may have made one first; keep whichever landed.
    let made = std::fs::hard_link(&temporary, &path);
    let _ = std::fs::remove_file(&temporary);
    match made {
        Ok(()) => Ok(key),
        Err(_) => {
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            <[u8; 32]>::try_from(bytes.as_slice())
                .map_err(|_| format!("{} is not a 32-byte key", path.display()))
        }
    }
}

fn ok(mut body: Value) -> Answer {
    body["v"] = json!("openagents.accounts.v1");
    Answer { status: 200, body }
}

fn refused(status: u16, code: &str, message: &str) -> Answer {
    Answer {
        status,
        body: json!({"error": {"code": code, "message": message}}),
    }
}

fn unavailable() -> Answer {
    refused(
        503,
        "sign_in_unavailable",
        "Login isn't available right now. Try again in a minute.",
    )
}

fn denied() -> Answer {
    refused(
        401,
        "sign_in_denied",
        "That sign-in didn't go through. Get a new challenge and try again.",
    )
}

fn used() -> Answer {
    refused(
        409,
        "proof_used",
        "That sign-in was already used. Get a new challenge and try again.",
    )
}

/// The event inside `Authorization: Nostr <base64>`, read only to find
/// its `u` and `challenge` tags; [`nostr::domain::parse_http_authorization_claim`]
/// checks it.
fn peek(header: &str) -> Result<nostr::domain::Event, Rejected> {
    let encoded = header.strip_prefix("Nostr ").ok_or(Rejected::Malformed)?;
    let bytes = [STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD]
        .iter()
        .find_map(|engine| engine.decode(encoded).ok())
        .ok_or(Rejected::Malformed)?;
    serde_json::from_slice(&bytes).map_err(|_| Rejected::Malformed)
}

fn single<'a>(event: &'a nostr::domain::Event, name: &'a str) -> Result<&'a str, Rejected> {
    let mut values = event.tag_values(name);
    match (values.next(), values.next()) {
        (Some(value), None) => Ok(value),
        _ => Err(Rejected::Malformed),
    }
}

/// The body's app label: `{"app", "computer"}`, each optional.
fn label(body: &Value) -> Result<AppLabel, Answer> {
    let text = |name: &str| {
        body.get(name)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };
    AppLabel::new(
        text("app").unwrap_or("OpenAgents"),
        text("computer").unwrap_or("Device"),
    )
    .map_err(|_| {
        refused(
            400,
            "invalid_request",
            "Name the app and device in 1 to 64 characters.",
        )
    })
}

/// A key as the routes show it.
fn key_view(pubkey: &str) -> Value {
    let npub = <[u8; 32]>::try_from(
        (0..32)
            .map(|i| u8::from_str_radix(&pubkey[i * 2..i * 2 + 2], 16).unwrap_or(0))
            .collect::<Vec<_>>()
            .as_slice(),
    )
    .map(|bytes| nostr::nip19::encode_npub(&bytes))
    .unwrap_or_default();
    json!({"pubkey": pubkey, "npub": npub})
}

/// A key named by a route: 64 hex characters (any case) or an `npub`.
#[must_use]
pub fn parse_pubkey(text: &str) -> Option<String> {
    if text.starts_with("npub1") {
        return nostr::nip19::decode_npub(text)
            .ok()
            .map(|bytes| hex(&bytes));
    }
    let lower = text.to_ascii_lowercase();
    valid_pubkey(&lower).then_some(lower)
}

impl NostrAuth {
    /// Nostr sign-in over the stores in `dir`, accepting events for
    /// `origins` (each `scheme://host[:port]`, no trailing slash; empty
    /// means [`DEFAULT_ORIGINS`]). Makes the challenge key on first use.
    pub fn open(dir: &Path, origins: &[String]) -> Result<Self, String> {
        let key = challenge_key(dir)?;
        let origins = if origins.is_empty() {
            DEFAULT_ORIGINS.iter().map(|o| (*o).to_string()).collect()
        } else {
            origins
                .iter()
                .map(|o| o.trim_end_matches('/').to_string())
                .collect()
        };
        Ok(Self {
            dir: dir.to_path_buf(),
            origins,
            key: hmac::Key::new(hmac::HMAC_SHA256, &key),
        })
    }

    fn mac(&self, body: &str, purpose: Purpose<'_>) -> String {
        let signed = hmac::sign(
            &self.key,
            format!("openagents-nostr|{body}|{}", purpose.tag()).as_bytes(),
        );
        hex(&signed.as_ref()[..16])
    }

    /// A fresh challenge for `purpose` at `now`: `v1.<expires>.<nonce>.<mac>`.
    #[must_use]
    pub fn issue(&self, purpose: Purpose<'_>, now: u64) -> (String, u64) {
        let mut nonce = [0_u8; 32];
        let _ = ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut nonce);
        let expires = now + CHALLENGE_TTL;
        let body = format!("v1.{expires}.{}", hex(&nonce));
        let mac = self.mac(&body, purpose);
        (format!("{body}.{mac}"), expires)
    }

    /// `GET .../challenge`: a challenge for `purpose`.
    #[must_use]
    pub fn challenge(&self, purpose: Purpose<'_>, now: u64) -> Answer {
        let (challenge, expires) = self.issue(purpose, now);
        ok(json!({
            "challenge": challenge,
            "expires_at": expires,
            "expires_in": CHALLENGE_TTL,
        }))
    }

    fn check_challenge(
        &self,
        challenge: &str,
        purpose: Purpose<'_>,
        now: u64,
    ) -> Result<(String, u64), Rejected> {
        let parts: Vec<&str> = challenge.split('.').collect();
        let [version, expires, nonce, mac] = parts.as_slice() else {
            return Err(Rejected::Challenge);
        };
        if *version != "v1" || !lower_hex(nonce, 64) || !lower_hex(mac, 32) {
            return Err(Rejected::Challenge);
        }
        let expected = self.mac(&format!("v1.{expires}.{nonce}"), purpose);
        if !bool::from(subtle::ConstantTimeEq::ct_eq(
            expected.as_bytes(),
            mac.as_bytes(),
        )) {
            return Err(Rejected::Challenge);
        }
        let expires: u64 = expires.parse().map_err(|_| Rejected::Challenge)?;
        if now > expires {
            return Err(Rejected::Expired);
        }
        Ok(((*nonce).to_string(), expires))
    }

    /// Verify a NIP-98 `Authorization` value for `method` on one of
    /// `paths`, holding the body to its `payload` tag when `payload` is
    /// given (and refusing a `payload` tag when it isn't), with a
    /// challenge for `purpose`. `host` is the request's own `Host`, whose
    /// origin is accepted beside the configured ones. Burns nothing.
    #[allow(clippy::too_many_arguments)]
    pub fn verify(
        &self,
        host: Option<&str>,
        header: &str,
        method: &str,
        paths: &[&str],
        payload: Option<&[u8]>,
        purpose: Purpose<'_>,
        now: u64,
    ) -> Result<Proof, Rejected> {
        if header.len() > HEADER_MAX {
            return Err(Rejected::Malformed);
        }
        let event = peek(header)?;
        let url = single(&event, "u")?;
        let own = host
            .filter(|h| !h.is_empty() && h.bytes().all(|b| b.is_ascii_graphic() && b != b'/'))
            .map(|h| [format!("http://{h}"), format!("https://{h}")]);
        let accepted = self
            .origins
            .iter()
            .chain(own.iter().flatten())
            .any(|origin| {
                url.strip_prefix(origin.as_str())
                    .is_some_and(|path| paths.contains(&path))
            });
        if !accepted {
            return Err(Rejected::Url);
        }
        let claim = nostr::domain::parse_http_authorization_claim(header, method, url, now)
            .map_err(|error| Rejected::Event(error.to_string()))?;
        if claim.auth.event_id != event.id || claim.auth.pubkey != event.pubkey {
            return Err(Rejected::Malformed);
        }
        let expected = payload.map(|body| hex(&Sha256::digest(body)));
        if claim.payload_hash != expected {
            return Err(Rejected::Payload);
        }
        let (nonce, expires) = self.check_challenge(single(&event, "challenge")?, purpose, now)?;
        if !valid_pubkey(&claim.auth.pubkey) || !lower_hex(&claim.auth.event_id, 64) {
            return Err(Rejected::Malformed);
        }
        Ok(Proof {
            pubkey: claim.auth.pubkey,
            event_id: claim.auth.event_id,
            nonce,
            expires,
        })
    }

    /// `POST /v1/sessions/nostr`: verify `authorization` over `body` (the
    /// exact bytes, `{"app", "computer"}` or empty), find or create the
    /// account holding `nostr:<pubkey>` (a new one gets a personal
    /// workspace on `tenant`), burn the proof, and issue an app session.
    /// Answers the device poll's `{session, token, account}` plus
    /// `created` and `pubkey`.
    pub fn sign_in(
        &self,
        host: Option<&str>,
        tenant: &str,
        authorization: Option<&str>,
        body: &[u8],
        now: u64,
    ) -> Answer {
        if body.len() > BODY_MAX {
            return refused(413, "invalid_request", "That request is too large.");
        }
        let fields: Value = if body.iter().all(u8::is_ascii_whitespace) {
            json!({})
        } else {
            match serde_json::from_slice::<Value>(body) {
                Ok(value) if value.is_object() => value,
                _ => {
                    return refused(
                        400,
                        "invalid_request",
                        "Send a JSON object with `app` and `computer`.",
                    );
                }
            }
        };
        let label = match label(&fields) {
            Ok(label) => label,
            Err(answer) => return answer,
        };
        let Some(authorization) = authorization else {
            return refused(
                401,
                "unauthenticated",
                "Sign the request with your key (`Authorization: Nostr ...`).",
            );
        };
        let proof = match self.verify(
            host,
            authorization,
            "POST",
            &SIGN_IN_PATHS,
            Some(body),
            Purpose::SignIn,
            now,
        ) {
            Ok(proof) => proof,
            Err(_) => return denied(),
        };
        let Ok(accounts) = Accounts::open(&self.dir) else {
            return unavailable();
        };
        let resolved = match accounts.sign_in_nostr(&proof.pubkey, tenant) {
            Ok(resolved) => resolved,
            Err(_) => return unavailable(),
        };
        if accounts
            .store()
            .ok()
            .is_some_and(|store| store.agent(&resolved.account.id).is_some())
        {
            return refused(
                403,
                "agent_account",
                "An agent account uses its API key directly; it can't sign in here.",
            );
        }
        let Ok(sessions) = Sessions::open(&self.dir) else {
            return unavailable();
        };
        let account = resolved.account.id.clone();
        let workspace = resolved.workspace.as_ref().map(|w| w.id.clone());
        let created = resolved.created;
        let issued = sessions.mutate(|book, access, at| {
            book.burn_nostr_proofs(&[&proof.event_id, &proof.nonce], proof.expires, at)?;
            let issued = book.issue_app(UserId::from(account.as_str()), label.clone(), at)?;
            sessions::push_access(
                access,
                sessions::Access {
                    at,
                    actor: account.clone(),
                    action: if created { "sign-up" } else { "sign-in" }.to_string(),
                    workspace: workspace.clone(),
                    session: Some(issued.session.id.as_str().to_string()),
                    detail: Some(nostr_principal(&proof.pubkey)),
                },
            );
            Ok(issued)
        });
        let issued = match issued {
            Ok(issued) => issued,
            Err(SessionRefusal::ProofUsed) => return used(),
            Err(_) => return unavailable(),
        };
        let session = &issued.session;
        let mut body = json!({
            "session": {
                "id": session.id.as_str(),
                "kind": "user",
                "account": session.user.as_str(),
                "created_at": session.created_at,
                "expires_at": session.expires_at,
            },
            "token": issued.once,
            "account": {"id": resolved.account.id, "label": resolved.account.label},
            "created": created,
            "pubkey": proof.pubkey,
        });
        if let Some(label) = &session.app {
            body["session"]["app"] = json!(label.app);
            body["session"]["computer"] = json!(label.computer);
        }
        ok(body)
    }

    /// `POST /v1/account/keys` `{"proof": "<base64 event>"}` as `account`:
    /// add the key that signed `proof` (kind 27235, `POST` on one of
    /// [`LINK_PATHS`], a challenge from `GET /v1/account/keys/challenge`
    /// for this account, no `payload` tag). Answers the key and the list.
    pub fn link(&self, host: Option<&str>, account: &str, body: &[u8], now: u64) -> Answer {
        let proof = serde_json::from_slice::<Value>(body)
            .ok()
            .and_then(|v| v.get("proof").and_then(Value::as_str).map(str::to_string));
        let Some(proof) = proof.filter(|p| !p.is_empty() && p.len() <= HEADER_MAX) else {
            return refused(
                400,
                "invalid_request",
                "Send `proof`, a request signed by the key to add.",
            );
        };
        let proof = match self.verify(
            host,
            &format!("Nostr {proof}"),
            "POST",
            &LINK_PATHS,
            None,
            Purpose::Link(account),
            now,
        ) {
            Ok(proof) => proof,
            Err(_) => {
                return refused(
                    400,
                    "key_proof_denied",
                    "That key's signature didn't go through. Get a new challenge and try again.",
                );
            }
        };
        let Ok(sessions) = Sessions::open(&self.dir) else {
            return unavailable();
        };
        match sessions.mutate(|book, _, at| {
            book.burn_nostr_proofs(&[&proof.event_id, &proof.nonce], proof.expires, at)
        }) {
            Ok(()) => {}
            Err(SessionRefusal::ProofUsed) => return used(),
            Err(_) => return unavailable(),
        }
        let Ok(accounts) = Accounts::open(&self.dir) else {
            return unavailable();
        };
        match accounts.link_nostr(account, &proof.pubkey) {
            Ok(record) => ok(json!({
                "key": key_view(&proof.pubkey),
                "keys": keys_of(&record).iter().map(|k| key_view(k)).collect::<Vec<_>>(),
            })),
            Err(AccountRefusal::PrincipalTaken { .. }) => refused(
                409,
                "identity_taken",
                "That key already signs in to another account.",
            ),
            Err(AccountRefusal::UnknownAccount(_)) => {
                refused(401, "unauthenticated", "Log in again.")
            }
            Err(AccountRefusal::Store(tenancy::accounts::Trouble::Invalid(_))) => refused(
                409,
                "too_many_keys",
                "This account has as many keys as it can hold. Remove one first.",
            ),
            Err(_) => unavailable(),
        }
    }

    /// `GET /v1/account/keys` as `account`: its keys, oldest first.
    #[must_use]
    pub fn keys(&self, account: &str) -> Answer {
        let Ok(store) = Accounts::open(&self.dir).and_then(|a| a.store()) else {
            return unavailable();
        };
        match store.accounts.get(account) {
            Some(record) => ok(json!({
                "keys": keys_of(record).iter().map(|k| key_view(k)).collect::<Vec<_>>(),
            })),
            None => refused(401, "unauthenticated", "Log in again."),
        }
    }

    /// `DELETE /v1/account/keys/{pubkey}` as `account` (`pubkey` as hex or
    /// `npub`). Never removes the account's last key.
    #[must_use]
    pub fn unlink(&self, account: &str, pubkey: &str) -> Answer {
        let not_found = || refused(404, "not_found", "This account has no such key.");
        let Some(pubkey) = parse_pubkey(pubkey) else {
            return not_found();
        };
        let Ok(accounts) = Accounts::open(&self.dir) else {
            return unavailable();
        };
        match accounts.unlink_nostr(account, &pubkey) {
            Ok(record) => ok(json!({
                "removed": key_view(&pubkey),
                "keys": keys_of(&record).iter().map(|k| key_view(k)).collect::<Vec<_>>(),
            })),
            Err(AccountRefusal::UnknownPrincipal(_)) => not_found(),
            Err(AccountRefusal::LastCredential(_)) => refused(
                409,
                "last_key",
                "This is the account's last key. Add another before removing it.",
            ),
            Err(AccountRefusal::UnknownAccount(_)) => {
                refused(401, "unauthenticated", "Log in again.")
            }
            Err(_) => unavailable(),
        }
    }
}

/// The routes over a [`NostrAuth`] for the local account service, with a
/// session bearer resolved through the session book.
#[cfg(feature = "local")]
pub fn router(auth: NostrAuth, tenant: &str) -> axum::Router {
    use axum::body::Bytes;
    use axum::extract::{Path as UrlPath, State};
    use axum::http::{HeaderMap, StatusCode, header};
    use axum::response::{IntoResponse, Response};
    use axum::routing::{delete, get};
    use std::sync::Arc;

    #[derive(Clone)]
    struct Local(Arc<(NostrAuth, String)>);

    fn answer(answer: Answer) -> Response {
        (
            StatusCode::from_u16(answer.status).unwrap_or(StatusCode::SERVICE_UNAVAILABLE),
            axum::Json(answer.body),
        )
            .into_response()
    }
    fn caller(local: &Local, headers: &HeaderMap) -> Result<String, Response> {
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .filter(|t| sessions::is_session_bearer(t));
        let account = token.and_then(|token| {
            let store = Sessions::open(&local.0.0.dir).ok()?.store().ok()?;
            let session = store.book.session_of_token(token)?;
            (session.kind == sessions::SessionKind::User
                && session.standing(now()) == sessions::SessionState::Active)
                .then(|| session.user.as_str().to_string())
        });
        account.ok_or_else(|| answer(refused(401, "unauthenticated", "Log in again.")))
    }
    fn host(headers: &HeaderMap) -> Option<&str> {
        headers.get(header::HOST).and_then(|v| v.to_str().ok())
    }
    let local = Local(Arc::new((auth, tenant.to_string())));
    axum::Router::new()
        .route(
            "/v1/sessions/nostr/challenge",
            get(|State(local): State<Local>| async move {
                answer(local.0.0.challenge(Purpose::SignIn, now()))
            }),
        )
        .route(
            "/v1/sessions/nostr",
            axum::routing::post(
                |State(local): State<Local>, headers: HeaderMap, body: Bytes| async move {
                    let authorization = headers
                        .get(header::AUTHORIZATION)
                        .and_then(|v| v.to_str().ok());
                    answer(local.0.0.sign_in(
                        host(&headers),
                        &local.0.1,
                        authorization,
                        &body,
                        now(),
                    ))
                },
            ),
        )
        .route(
            "/v1/account/keys/challenge",
            get(
                |State(local): State<Local>, headers: HeaderMap| async move {
                    match caller(&local, &headers) {
                        Ok(account) => answer(local.0.0.challenge(Purpose::Link(&account), now())),
                        Err(response) => response,
                    }
                },
            ),
        )
        .route(
            "/v1/account/keys",
            get(
                |State(local): State<Local>, headers: HeaderMap| async move {
                    match caller(&local, &headers) {
                        Ok(account) => answer(local.0.0.keys(&account)),
                        Err(response) => response,
                    }
                },
            )
            .post(
                |State(local): State<Local>, headers: HeaderMap, body: Bytes| async move {
                    match caller(&local, &headers) {
                        Ok(account) => {
                            answer(local.0.0.link(host(&headers), &account, &body, now()))
                        }
                        Err(response) => response,
                    }
                },
            ),
        )
        .route(
            "/v1/account/keys/{pubkey}",
            delete(
                |State(local): State<Local>,
                 UrlPath(pubkey): UrlPath<String>,
                 headers: HeaderMap| async move {
                    match caller(&local, &headers) {
                        Ok(account) => answer(local.0.0.unlink(&account, &pubkey)),
                        Err(response) => response,
                    }
                },
            ),
        )
        .with_state(local)
}

/// The current time as Unix seconds.
#[must_use]
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests;
