//! Link a computer to the phone's account with the scan that paired it
//! (#12000, NIP-HOST "Account key link by QR").
//!
//! The connect code carries no link field. After the phone pairs, it gets a
//! challenge for its own account from our server, asks the computer over
//! the new device connection to sign a NIP-98 request with that challenge
//! ([`request`]), checks the proof, and sends it to `POST /v1/device/keys`
//! with its own session. That request is the phone's approval: the server
//! adds the computer's account key to the phone's account. When the server
//! accepts it, the phone tells the computer ([`confirm`],
//! [`METHOD_LINKED`]), and the computer then signs in with its key as the
//! same person.
//!
//! The computer half is [`Signer`]. It signs only for the device whose
//! redemption of a connect code made its current grant, only shortly after
//! that redemption, only once per grant, only for our server's key-link
//! addresses, and only the exact event the server accepts.

use crate::device_channel::{Code, DeviceChannel, Failure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use nostr::domain::{Event, RelaySigner, Tag};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// The NIP-98 HTTP authorization kind.
pub const HTTP_AUTH_KIND: u16 = 27_235;
/// The app method the phone calls (`coder_access::app_rpc::Method::LinkAccount`).
pub const METHOD: &str = "LinkAccount";
/// The app method that tells the computer the link landed
/// (`coder_access::app_rpc::Method::AccountLinked`).
pub const METHOD_LINKED: &str = "AccountLinked";
/// The key-link addresses our server accepts a proof for.
pub const LINK_URLS: [&str; 4] = [
    "https://api.openagents.com/v1/device/keys",
    "https://api.openagents.com/v1/account/keys",
    "https://openagents.com/v1/device/keys",
    "https://openagents.com/v1/account/keys",
];
/// A connect code's grant runs 30 days from the code's issue
/// (`coder_access::protocol::MAX_GRANT_LIFETIME`).
pub const GRANT_LIFETIME: u64 = 30 * 24 * 60 * 60;
/// How long after a connect code's issue its grant may ask for a link
/// proof: the code's 300-second life plus the 300 seconds after redemption.
pub const LINK_WINDOW: u64 = 600;
/// The longest challenge the computer signs.
const MAX_CHALLENGE: usize = 256;
/// The most grants the computer remembers signing for.
const MAX_USED: usize = 256;

/// Why linking didn't finish, in words for the screen.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LinkError {
    /// The computer doesn't link by scan, or this pairing can't ask now.
    #[error(
        "This computer is connected, but it hasn't joined your account. Choose Connect your phone on it and scan the new code."
    )]
    NotOffered,
    /// The computer's answer wasn't a valid proof.
    #[error("This computer's answer didn't check out. Show a new code on it and scan again.")]
    Mismatch,
    /// The computer couldn't be reached.
    #[error("Couldn't reach your computer. Check that OpenAgents is open on it, then scan again.")]
    Unavailable,
}

/// The key-link address under an account API base such as
/// `https://api.openagents.com/v1/` (`openagents::Deployment::api_base`).
#[must_use]
pub fn link_url(api_base: &str) -> String {
    format!("{}/device/keys", api_base.trim_end_matches('/'))
}

/// The challenge address beside [`link_url`].
#[must_use]
pub fn challenge_url(api_base: &str) -> String {
    format!("{}/challenge", link_url(api_base))
}

/// A checked link proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proof {
    /// What `POST /v1/device/keys` takes as `proof`: base64 of the event.
    pub proof: String,
    /// The computer's account key, 64 lowercase hex characters.
    pub key: String,
}

/// Ask the paired computer to sign a key-link proof for `url` with
/// `challenge`, and check it: a valid NIP-98 `POST` on `url` within 60
/// seconds of `now`, with exactly this challenge and no payload, signed by
/// a key that isn't `own` (the phone's own account key, when it has one).
///
/// # Errors
/// [`LinkError`] when the computer refuses, can't be reached, or answers
/// with anything else.
pub async fn request(
    channel: &DeviceChannel,
    url: &str,
    challenge: &str,
    own: Option<&str>,
    now: u64,
) -> Result<Proof, LinkError> {
    let answer = channel
        .call(METHOD, json!({"url": url, "challenge": challenge}))
        .await
        .map_err(|error| match error {
            Failure::Refused(_) => LinkError::NotOffered,
            _ => LinkError::Unavailable,
        })?
        .ok_or(LinkError::NotOffered)?;
    check(&answer, url, challenge, own, now)
}

/// Tell the computer the server added its account key `key` to this
/// account, so it signs in now.
///
/// # Errors
/// [`LinkError`] when the computer refuses or can't be reached; the link
/// itself stands either way.
pub async fn confirm(channel: &DeviceChannel, key: &str) -> Result<(), LinkError> {
    channel
        .call(METHOD_LINKED, json!({ "pubkey": key }))
        .await
        .map_err(|error| match error {
            Failure::Refused(_) => LinkError::NotOffered,
            _ => LinkError::Unavailable,
        })?
        .map(|_| ())
        .ok_or(LinkError::NotOffered)
}

/// Check a computer's [`METHOD`] answer; see [`request`].
///
/// # Errors
/// [`LinkError::Mismatch`] for anything but the expected proof.
pub fn check(
    answer: &Value,
    url: &str,
    challenge: &str,
    own: Option<&str>,
    now: u64,
) -> Result<Proof, LinkError> {
    let object = answer.as_object().ok_or(LinkError::Mismatch)?;
    if object.len() != 1 {
        return Err(LinkError::Mismatch);
    }
    let event: Event =
        serde_json::from_value(answer["event"].clone()).map_err(|_| LinkError::Mismatch)?;
    let bytes = serde_json::to_vec(&event).map_err(|_| LinkError::Mismatch)?;
    let proof = STANDARD.encode(bytes);
    let claim =
        nostr::domain::parse_http_authorization_claim(&format!("Nostr {proof}"), "POST", url, now)
            .map_err(|_| LinkError::Mismatch)?;
    let challenges: Vec<&str> = event.tag_values("challenge").collect();
    if claim.payload_hash.is_some()
        || challenges != [challenge]
        || !event.content.is_empty()
        || own.is_some_and(|own| own.eq_ignore_ascii_case(&claim.auth.pubkey))
    {
        return Err(LinkError::Mismatch);
    }
    Ok(Proof {
        proof,
        key: claim.auth.pubkey,
    })
}

/// The computer's half: signs key-link proofs with its account key.
pub struct Signer {
    secret: secp256k1::SecretKey,
    urls: Vec<String>,
    ready: AtomicBool,
    /// Grants a proof was signed for.
    used: Mutex<HashSet<String>>,
    /// Grants that said the link landed.
    confirmed: Mutex<HashSet<String>>,
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The secret key stays out of debug output.
        f.debug_struct("Signer")
            .field("key", &self.key())
            .finish_non_exhaustive()
    }
}

impl Signer {
    /// A signer for the account key `secret`, for [`LINK_URLS`] and any
    /// `extra` key-link addresses (another deployment's [`link_url`]). It
    /// starts ready.
    #[must_use]
    pub fn new(secret: secp256k1::SecretKey, extra: &[String]) -> Self {
        let mut urls: Vec<String> = LINK_URLS.iter().map(|u| (*u).to_string()).collect();
        urls.extend(extra.iter().cloned());
        Self {
            secret,
            urls,
            ready: AtomicBool::new(true),
            used: Mutex::new(HashSet::new()),
            confirmed: Mutex::new(HashSet::new()),
        }
    }

    /// A signer for the account key `nsec` (`nsec1…`), or `None` when it
    /// isn't one.
    #[must_use]
    pub fn from_nsec(nsec: &str, extra: &[String]) -> Option<Self> {
        let bytes = nostr::nip19::decode_nsec(nsec).ok()?;
        let secret = secp256k1::SecretKey::from_byte_array(bytes).ok()?;
        Some(Self::new(secret, extra))
    }

    /// Answer a [`METHOD_LINKED`] call's params `{pubkey}` from `grant`:
    /// the phone that got a proof signed says the server accepted it.
    /// Answers the key, for the computer to sign in with.
    ///
    /// # Errors
    /// [`Code::Forbidden`] unless `grant` got a proof from this signer and
    /// hasn't confirmed yet, and the key is this signer's;
    /// [`Code::Malformed`] for bad params.
    pub fn confirm(&self, grant: &str, params: &Value) -> Result<String, Code> {
        let object = params.as_object().ok_or(Code::Malformed)?;
        let key = object
            .get("pubkey")
            .and_then(Value::as_str)
            .filter(|_| object.len() == 1)
            .ok_or(Code::Malformed)?;
        let mine = self.key();
        if key != mine || grant.is_empty() {
            return Err(Code::Forbidden);
        }
        let signed = self
            .used
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(grant);
        let mut confirmed = self
            .confirmed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !signed || confirmed.contains(grant) {
            return Err(Code::Forbidden);
        }
        if confirmed.len() >= MAX_USED {
            confirmed.clear();
        }
        confirmed.insert(grant.to_owned());
        Ok(mine)
    }

    /// The account key, 64 lowercase hex characters.
    #[must_use]
    pub fn key(&self) -> String {
        coder_reach::pubkey(&self.secret)
    }

    /// Whether the computer signs at all. A computer already signed in to a
    /// different account sets this off until the person confirms on its
    /// own screen.
    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::Release);
    }

    /// Answer a [`METHOD`] call's params `{url, challenge}` at `now` with
    /// `{event}`, for the grant `grant` that expires at `grant_expires_at`.
    ///
    /// # Errors
    /// [`Code::Forbidden`] when the computer isn't ready, the grant isn't a
    /// fresh connect-code grant or already asked, or the address isn't a
    /// key-link address; [`Code::Malformed`] for bad params.
    pub fn sign(
        &self,
        grant: &str,
        grant_expires_at: u64,
        params: &Value,
        now: u64,
    ) -> Result<Value, Code> {
        if !self.ready.load(Ordering::Acquire) {
            return Err(Code::Forbidden);
        }
        let object = params.as_object().ok_or(Code::Malformed)?;
        if object.len() != 2 {
            return Err(Code::Malformed);
        }
        let url = object
            .get("url")
            .and_then(Value::as_str)
            .ok_or(Code::Malformed)?;
        let challenge = object
            .get("challenge")
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty() && c.len() <= MAX_CHALLENGE)
            .filter(|c| c.bytes().all(|b| b.is_ascii_graphic()))
            .ok_or(Code::Malformed)?;
        if !self.urls.iter().any(|allowed| allowed == url) {
            return Err(Code::Forbidden);
        }
        // A connect code's grant expires exactly 30 days after the code was
        // issued; an older or longer-lived grant can't relink the computer.
        let issued = grant_expires_at
            .checked_sub(GRANT_LIFETIME)
            .ok_or(Code::Forbidden)?;
        if now < issued.saturating_sub(60) || now > issued.saturating_add(LINK_WINDOW) {
            return Err(Code::Forbidden);
        }
        {
            let mut used = self
                .used
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if used.contains(grant) || grant.is_empty() {
                return Err(Code::Forbidden);
            }
            if used.len() >= MAX_USED {
                used.clear();
            }
            used.insert(grant.to_owned());
        }
        let signer = RelaySigner::from_secret_hex(&self.secret.display_secret().to_string())
            .map_err(|_| Code::Unavailable)?;
        let event = signer.sign(
            now,
            HTTP_AUTH_KIND,
            vec![
                Tag::new(vec!["u".into(), url.into()]),
                Tag::new(vec!["method".into(), "POST".into()]),
                Tag::new(vec!["challenge".into(), challenge.into()]),
            ],
            String::new(),
        );
        Ok(json!({ "event": event }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_000_000;

    fn signer() -> Signer {
        Signer::new(secp256k1::SecretKey::new(&mut secp256k1::rand::rng()), &[])
    }

    /// A connect-code grant whose code was issued `ago` seconds before NOW.
    fn fresh(ago: u64) -> u64 {
        NOW - ago + GRANT_LIFETIME
    }

    #[test]
    fn the_api_base_names_the_link_addresses() {
        assert_eq!(
            link_url("https://api.openagents.com/v1/"),
            "https://api.openagents.com/v1/device/keys"
        );
        assert_eq!(
            challenge_url("https://api.openagents.com/v1"),
            "https://api.openagents.com/v1/device/keys/challenge"
        );
        assert!(LINK_URLS.contains(&link_url("https://api.openagents.com/v1/").as_str()));
    }

    #[test]
    fn the_computer_signs_once_per_fresh_connect_code_grant() {
        let signer = signer();
        let url = LINK_URLS[0];
        let params = json!({"url": url, "challenge": "v1.1.abc.def"});
        let answer = signer.sign("grant-a", fresh(120), &params, NOW).unwrap();
        let proof = check(&answer, url, "v1.1.abc.def", None, NOW).unwrap();
        assert_eq!(proof.key, signer.key());
        // Once per grant.
        assert_eq!(
            signer.sign("grant-a", fresh(120), &params, NOW),
            Err(Code::Forbidden)
        );
        // An old grant, or one that isn't a 30-day connect-code grant.
        assert_eq!(
            signer.sign("grant-b", fresh(LINK_WINDOW + 1), &params, NOW),
            Err(Code::Forbidden)
        );
        assert_eq!(
            signer.sign("grant-c", NOW + 3600, &params, NOW),
            Err(Code::Forbidden)
        );
        // Not while the person hasn't confirmed on the computer.
        signer.set_ready(false);
        assert_eq!(
            signer.sign("grant-d", fresh(10), &params, NOW),
            Err(Code::Forbidden)
        );
        signer.set_ready(true);
        assert!(signer.sign("grant-d", fresh(10), &params, NOW).is_ok());
    }

    #[test]
    fn only_the_grant_that_got_a_proof_confirms_the_link_once() {
        let signer = signer();
        let key = signer.key();
        let linked = json!({ "pubkey": key });
        assert_eq!(signer.confirm("g", &linked), Err(Code::Forbidden));
        signer
            .sign(
                "g",
                fresh(10),
                &json!({"url": LINK_URLS[0], "challenge": "c"}),
                NOW,
            )
            .unwrap();
        assert_eq!(
            signer.confirm("g", &json!({ "pubkey": "ab".repeat(32) })),
            Err(Code::Forbidden)
        );
        assert_eq!(signer.confirm("other", &linked), Err(Code::Forbidden));
        assert_eq!(signer.confirm("g", &json!({})), Err(Code::Malformed));
        assert_eq!(signer.confirm("g", &linked), Ok(key));
        assert_eq!(signer.confirm("g", &linked), Err(Code::Forbidden));
    }

    #[test]
    fn a_signer_reads_its_key_from_an_nsec() {
        let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
        let nsec = nostr::nip19::encode_nsec(&secret.secret_bytes());
        let signer = Signer::from_nsec(&nsec, &[]).unwrap();
        assert_eq!(signer.key(), coder_reach::pubkey(&secret));
        assert!(Signer::from_nsec("nsec1nope", &[]).is_none());
    }

    #[test]
    fn the_computer_signs_only_our_link_addresses_and_shape() {
        let signer = signer();
        let refused = |n: u32, params: Value| {
            signer
                .sign(&format!("grant-{n}"), fresh(10), &params, NOW)
                .unwrap_err()
        };
        assert_eq!(
            refused(
                1,
                json!({"url": "https://evil.example/v1/device/keys", "challenge": "c"})
            ),
            Code::Forbidden
        );
        assert_eq!(
            refused(
                2,
                json!({"url": "https://openagents.com/v1/sessions/nostr", "challenge": "c"})
            ),
            Code::Forbidden
        );
        assert_eq!(refused(3, json!({"url": LINK_URLS[0]})), Code::Malformed);
        assert_eq!(
            refused(4, json!({"url": LINK_URLS[0], "challenge": ""})),
            Code::Malformed
        );
        assert_eq!(
            refused(
                5,
                json!({"url": LINK_URLS[0], "challenge": "c", "payload": "x"})
            ),
            Code::Malformed
        );
        assert_eq!(
            refused(6, json!({"url": LINK_URLS[0], "challenge": "a b"})),
            Code::Malformed
        );
        let staging = "https://api.staging.example/v1/device/keys".to_string();
        let extra = Signer::new(
            secp256k1::SecretKey::new(&mut secp256k1::rand::rng()),
            std::slice::from_ref(&staging),
        );
        assert!(
            extra
                .sign(
                    "g",
                    fresh(10),
                    &json!({"url": staging, "challenge": "c"}),
                    NOW
                )
                .is_ok()
        );
    }

    #[test]
    fn the_phone_keeps_only_a_valid_proof() {
        let signer = signer();
        let url = LINK_URLS[0];
        let answer = signer
            .sign(
                "g",
                fresh(10),
                &json!({"url": url, "challenge": "chal"}),
                NOW,
            )
            .unwrap();
        check(&answer, url, "chal", None, NOW + 30).unwrap();
        // The phone's own key, another challenge, address, or time is refused.
        let key = signer.key();
        assert_eq!(
            check(&answer, url, "chal", Some(&key), NOW),
            Err(LinkError::Mismatch)
        );
        assert_eq!(
            check(&answer, url, "other", None, NOW),
            Err(LinkError::Mismatch)
        );
        assert_eq!(
            check(&answer, LINK_URLS[1], "chal", None, NOW),
            Err(LinkError::Mismatch)
        );
        assert_eq!(
            check(&answer, url, "chal", None, NOW + 200),
            Err(LinkError::Mismatch)
        );
        // A tampered event fails its signature.
        let mut tampered = answer.clone();
        tampered["event"]["created_at"] = json!(NOW + 1);
        assert_eq!(
            check(&tampered, url, "chal", None, NOW),
            Err(LinkError::Mismatch)
        );
        assert_eq!(
            check(&json!({}), url, "chal", None, NOW),
            Err(LinkError::Mismatch)
        );
    }
}
