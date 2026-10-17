//! Limits for accounts that sign in with only a Nostr key (#11994,
//! `docs/audits/2026-10-17-nostr-only-identity.md`, "Abuse controls
//! without GitHub").
//!
//! A key costs nothing, so the free allowances of a key-only account are
//! bounded by something else:
//!
//! - **App Attest.** The phone's first sign-in may carry an App Attest
//!   attestation over the sign-in challenge ([`AttestBody`]); a key that
//!   verifies ([`oa_auth::attest`]) is bound to the account, and one
//!   install makes at most one attested account.
//! - **Sign-up limits for the rest.** A new key account that isn't
//!   attested (the desktop, the CLI, a script) needs the same limits an
//!   agent's self-registration has ([`crate::agent_accounts`]): a proof of
//!   work over the sign-in challenge and per-address, per-network (an IPv4
//!   /24 or an IPv6 /48), and global daily caps.
//! - **A daily breaker.** Every key-only account that hasn't connected an
//!   account or paid shares one daily dollar cap on its free model
//!   allowance ([`KeyAccounts::free_usd_micros_per_day`]), so the worst
//!   day is bounded whatever the number of accounts.
//! - **Free cloud time.** [`Standing::free_cloud`] says which free cloud
//!   allowance the account gets: the full one after it connects an account
//!   (GitHub, Apple) or pays, a smaller one when attested, none otherwise.
//!   `GET /v1/account` carries it as `account.free_cloud`.
//!
//! Accounts with any other sign-in are [`Standing::Proven`] and nothing
//! here changes for them.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use oa_auth::attest::{self, Attested, BookRefusal, Verifier};
use oa_auth::device::Answer;
use oa_auth::nostr::{NewKeyGate, Proof};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::accounts::{refused, unix_now};
use crate::serve::ServeState;

/// The day's key-account sign-up count, beside the registry.
const SIGNUPS: &str = "key-signups.json";
/// The longest proof-of-work solution taken, bytes.
const MAX_SOLUTION: usize = 64;

/// What bounds key-only accounts. Every field has a default, so `{}` (or
/// leaving it out) applies them.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyAccounts {
    /// Leading zero bits the proof of work over the sign-in challenge
    /// needs (SHA-256 of `challenge ":" solution`). At 0 no proof is
    /// needed; the caps still apply.
    #[serde(default = "default_bits")]
    pub difficulty_bits: u8,
    /// New unattested key accounts one address may make a day (UTC).
    #[serde(default = "default_per_ip")]
    pub per_ip_per_day: u32,
    /// New unattested key accounts one network may make a day.
    #[serde(default = "default_per_network")]
    pub per_network_per_day: u32,
    /// New unattested key accounts everyone together may make a day.
    #[serde(default = "default_per_day")]
    pub per_day: u32,
    /// The proxies in front of the gateway that append to
    /// `X-Forwarded-For` (as for agents).
    #[serde(default = "default_hops")]
    pub forwarded_hops: usize,
    /// The App Attest app id, `<team>.<bundle>`.
    #[serde(default = "default_app_id")]
    pub app_id: String,
    /// Whether keys made by a development build count as attested.
    #[serde(default)]
    pub allow_development: bool,
    /// The breaker: the most every unproven key account's free model
    /// allowance together may cost a day, micros of USD.
    #[serde(default = "default_breaker")]
    pub free_usd_micros_per_day: u64,
}

fn default_bits() -> u8 {
    20
}
fn default_per_ip() -> u32 {
    3
}
fn default_per_network() -> u32 {
    10
}
fn default_per_day() -> u32 {
    500
}
fn default_hops() -> usize {
    1
}
fn default_app_id() -> String {
    attest::APP_ID.to_owned()
}
fn default_breaker() -> u64 {
    20_000_000
}

impl Default for KeyAccounts {
    fn default() -> Self {
        Self {
            difficulty_bits: default_bits(),
            per_ip_per_day: default_per_ip(),
            per_network_per_day: default_per_network(),
            per_day: default_per_day(),
            forwarded_hops: default_hops(),
            app_id: default_app_id(),
            allow_development: false,
            free_usd_micros_per_day: default_breaker(),
        }
    }
}

/// What an account has shown beyond holding a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// Any sign-in besides a key (GitHub, Apple, SSO, an operator
    /// account), or a key account that paid: unchanged by this module.
    Proven,
    /// A key-only account made on an attested install.
    Attested,
    /// A key-only account with nothing else.
    KeyOnly,
}

impl Standing {
    /// The free cloud allowance it gets: `full`, `attested`, or `none`.
    #[must_use]
    pub fn free_cloud(self) -> &'static str {
        match self {
            Self::Proven => "full",
            Self::Attested => "attested",
            Self::KeyOnly => "none",
        }
    }

    /// Whether its free model use counts toward the daily breaker.
    #[must_use]
    pub fn under_breaker(self) -> bool {
        self != Self::Proven
    }
}

/// Whether every principal of an account is a Nostr key (and it has one).
#[must_use]
pub fn key_only(principals: &[String]) -> bool {
    !principals.is_empty() && principals.iter().all(|p| p.starts_with("nostr:"))
}

/// The standing of an account with these principals.
#[must_use]
pub fn standing(principals: &[String], attested: bool, paid: bool) -> Standing {
    if !key_only(principals) || paid {
        Standing::Proven
    } else if attested {
        Standing::Attested
    } else {
        Standing::KeyOnly
    }
}

/// Checks a proof of work: `SHA-256(challenge ":" solution)` starts with
/// `bits` zero bits.
///
/// # Errors
///
/// A plain sentence saying what's wrong.
pub fn check_pow(challenge: &str, solution: &str, bits: u8) -> Result<(), &'static str> {
    if solution.is_empty()
        || solution.len() > MAX_SOLUTION
        || !solution.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err("`pow` must be 1 to 64 printable characters.");
    }
    let digest = Sha256::digest(format!("{challenge}:{solution}").as_bytes());
    if crate::agent_accounts::leading_zero_bits(&digest) < u32::from(bits) {
        return Err("That `pow` doesn't solve the challenge.");
    }
    Ok(())
}

/// The day's new unattested key accounts: the total persisted, the
/// per-address and per-network counts in memory.
#[derive(Debug, Default)]
pub struct Signups {
    day: u64,
    total: u32,
    per_ip: HashMap<IpAddr, u32>,
    per_net: HashMap<String, u32>,
}

#[derive(Default, Serialize, Deserialize)]
struct SignupsSaved {
    day: u64,
    total: u32,
}

impl Signups {
    /// Counts one new account from `ip` on `day`, or says which cap stops
    /// it.
    ///
    /// # Errors
    ///
    /// A plain sentence naming the cap.
    pub fn admit(
        &mut self,
        config: &KeyAccounts,
        ip: Option<IpAddr>,
        day: u64,
    ) -> Result<(), &'static str> {
        if self.day != day {
            *self = Self {
                day,
                ..Self::default()
            };
        }
        if self.total >= config.per_day {
            return Err(
                "We've made all the new accounts we make today. Try again after 00:00 UTC, or sign in on the iPhone app.",
            );
        }
        if let Some(ip) = ip {
            if self.per_ip.get(&ip).copied().unwrap_or(0) >= config.per_ip_per_day {
                return Err(
                    "This address has made all the accounts it may make today. Try again after 00:00 UTC.",
                );
            }
            let net = crate::agent_accounts::network(ip);
            if self.per_net.get(&net).copied().unwrap_or(0) >= config.per_network_per_day {
                return Err(
                    "Your network has made all the accounts it may make today. Try again after 00:00 UTC.",
                );
            }
            *self.per_ip.entry(ip).or_default() += 1;
            *self.per_net.entry(net).or_default() += 1;
        }
        self.total += 1;
        Ok(())
    }
}

/// An App Attest attestation sent with a first key sign-in, base64: the
/// key id `DCAppAttestService.generateKey` returned and the attestation
/// `attestKey(_:clientDataHash:)` made over `SHA-256(challenge)`, where
/// `challenge` is the sign-in challenge the request signs.
#[derive(Clone, Debug, Deserialize)]
pub struct AttestBody {
    pub key_id: String,
    pub attestation: String,
}

/// A refusal, as either an account-service answer or a response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
    /// Seconds to wait, for a daily cap.
    pub retry_after: Option<u64>,
}

impl Refusal {
    fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            retry_after: None,
        }
    }

    /// As the account service's answer; a wait rides in
    /// `error.retry_after`, which the sign-in route copies to `Retry-After`.
    #[must_use]
    pub fn answer(&self) -> Answer {
        let mut body = json!({"error": {"code": self.code, "message": self.message}});
        if let Some(wait) = self.retry_after {
            body["error"]["retry_after"] = json!(wait);
        }
        Answer {
            status: self.status,
            body,
        }
    }

    /// As a response.
    #[must_use]
    pub fn response(&self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::SERVICE_UNAVAILABLE);
        let mut response = refused(status, self.code, self.message.clone());
        if let Some(wait) = self.retry_after
            && let Ok(value) = HeaderValue::from_str(&wait.to_string())
        {
            response.headers_mut().insert("retry-after", value);
        }
        response
    }
}

fn busy() -> Refusal {
    Refusal::new(503, "accounts_unavailable", "Try again in a moment.")
}

/// How long a sign-in event id is remembered, seconds: longer than a
/// challenge stands.
const SEEN_TTL: u64 = 600;

/// The key-account state the gateway holds.
pub struct KeyState {
    pub(crate) config: KeyAccounts,
    verifier: Verifier,
    book: Mutex<attest::Book>,
    signups: Mutex<Signups>,
    signups_path: PathBuf,
    /// Sign-in events already counted, with when to forget them, so a
    /// replayed request can't spend the day's caps.
    seen: Mutex<HashMap<String, u64>>,
    /// Recent standings, with when each was read, so a busy account's
    /// model requests don't each read the account store.
    standings: Mutex<HashMap<String, (u64, Standing)>>,
}

impl KeyState {
    /// The state over the registry directory `dir`.
    ///
    /// # Errors
    ///
    /// The attestation book exists and can't be read.
    pub(crate) fn open(dir: &Path, config: KeyAccounts) -> Result<Self, String> {
        let signups_path = dir.join(SIGNUPS);
        let saved: SignupsSaved = std::fs::read(&signups_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Ok(Self {
            verifier: Verifier::apple(&config.app_id, config.allow_development),
            config,
            book: Mutex::new(attest::Book::open(dir)?),
            signups: Mutex::new(Signups {
                day: saved.day,
                total: saved.total,
                ..Signups::default()
            }),
            signups_path,
            seen: Mutex::new(HashMap::new()),
            standings: Mutex::new(HashMap::new()),
        })
    }

    /// The gate a Nostr sign-in passes before it makes an account.
    pub(crate) fn gate<'a>(&'a self, headers: &'a HeaderMap) -> Gate<'a> {
        Gate {
            state: self,
            headers,
            attested: None,
        }
    }

    /// Verifies an attestation over `challenge` (the exact challenge text
    /// the key signed). `Ok(None)` when the install's key already belongs
    /// to an account: the sign-in then goes through the unattested limits.
    ///
    /// # Errors
    ///
    /// A `400 invalid_attestation` refusal.
    pub(crate) fn verify(
        &self,
        body: &AttestBody,
        challenge: &str,
    ) -> Result<Option<Attested>, Refusal> {
        let bad = |why: String| {
            Refusal::new(
                400,
                "invalid_attestation",
                format!("We couldn't check this app install: {why}. Update the app and try again."),
            )
        };
        let key_id = attest::decode_base64(&body.key_id).ok_or_else(|| bad("bad key id".into()))?;
        let object = attest::decode_base64(&body.attestation)
            .ok_or_else(|| bad("bad attestation".into()))?;
        let now = i64::try_from(unix_now()).unwrap_or(i64::MAX);
        let attested = self
            .verifier
            .attestation(
                &object,
                &key_id,
                &attest::client_data_hash(challenge.as_bytes()),
                now,
            )
            .map_err(|e| bad(e.to_string()))?;
        let book = self.book.lock().map_err(|_| busy())?;
        Ok(book.key(&attested.key_id).is_none().then_some(attested))
    }

    /// Records that `account` was made on an attested install.
    ///
    /// # Errors
    ///
    /// The key belongs to another account, or the book can't be written.
    pub(crate) fn bind(&self, attested: &Attested, account: &str) -> Result<(), Refusal> {
        let mut book = self.book.lock().map_err(|_| busy())?;
        book.bind(attested, account, unix_now())
            .map_err(|refusal| match refusal {
                BookRefusal::TakenByOther => Refusal::new(
                    409,
                    "attestation_in_use",
                    "This app install already has an account.",
                ),
                BookRefusal::Unknown | BookRefusal::Io(_) => busy(),
            })
    }

    /// Whether `account` was made on an attested install.
    pub(crate) fn is_attested(&self, account: &str) -> bool {
        self.book
            .lock()
            .map(|book| book.is_attested(account))
            .unwrap_or(false)
    }

    /// Takes a sign-in event once; a second time is refused, so a replayed
    /// request counts toward no cap.
    fn first_seen(&self, event_id: &str) -> Result<(), Refusal> {
        let now = unix_now();
        let mut seen = self.seen.lock().map_err(|_| busy())?;
        seen.retain(|_, until| *until > now);
        if seen.insert(event_id.to_owned(), now + SEEN_TTL).is_some() {
            return Err(Refusal::new(
                409,
                "proof_used",
                "That sign-in was already used. Sign in again.",
            ));
        }
        Ok(())
    }

    /// Admits a new key account that isn't attested: the proof of work
    /// over `challenge`, then the daily caps for the caller's address.
    ///
    /// # Errors
    ///
    /// `400 proof_required` or `invalid_proof` (the answer names the
    /// difficulty), or `429 signup_limit` with a wait.
    pub(crate) fn admit_unattested(
        &self,
        headers: &HeaderMap,
        challenge: &str,
        solution: Option<&str>,
    ) -> Result<(), Refusal> {
        let bits = self.config.difficulty_bits;
        // At zero bits (local use and tests) no proof is needed.
        let solution = solution.or((bits == 0).then_some("0"));
        let Some(solution) = solution else {
            return Err(Refusal::new(
                400,
                "proof_required",
                format!(
                    "A new account needs a proof of work: send `pow`, a solution where SHA-256 of the challenge, a colon, and the solution starts with {bits} zero bits."
                ),
            ));
        };
        check_pow(challenge, solution, bits)
            .map_err(|why| Refusal::new(400, "invalid_proof", why))?;
        let ip = crate::agent_accounts::caller_ip(headers, self.config.forwarded_hops);
        let day = crate::inference_public::today();
        let mut signups = self.signups.lock().map_err(|_| busy())?;
        if let Err(why) = signups.admit(&self.config, ip, day) {
            let mut refusal = Refusal::new(429, "signup_limit", why);
            refusal.retry_after = Some(86_400 - unix_now() % 86_400);
            return Err(refusal);
        }
        let saved = SignupsSaved {
            day,
            total: signups.total,
        };
        if let Ok(bytes) = serde_json::to_vec(&saved) {
            let _ = std::fs::write(&self.signups_path, bytes);
        }
        Ok(())
    }
}

/// What a Nostr sign-in by a key with no account must pass (#11994): an
/// App Attest attestation in `app_attest`, or, without one, a proof of
/// work in `pow` and the daily caps.
pub(crate) struct Gate<'a> {
    state: &'a KeyState,
    headers: &'a HeaderMap,
    attested: Option<Attested>,
}

impl NewKeyGate for Gate<'_> {
    fn admit(&mut self, proof: &Proof, fields: &Value) -> Result<(), Answer> {
        self.state
            .first_seen(&proof.event_id)
            .map_err(|r| r.answer())?;
        if let Some(body) = fields.get("app_attest") {
            let body: AttestBody = serde_json::from_value(body.clone()).map_err(|_| {
                Refusal::new(
                    400,
                    "invalid_attestation",
                    "Send `app_attest` as `key_id` and `attestation`.",
                )
                .answer()
            })?;
            if let Some(attested) = self
                .state
                .verify(&body, &proof.challenge)
                .map_err(|r| r.answer())?
            {
                self.attested = Some(attested);
                return Ok(());
            }
        }
        self.state
            .admit_unattested(
                self.headers,
                &proof.challenge,
                fields.get("pow").and_then(Value::as_str),
            )
            .map_err(|r| r.answer())
    }

    fn made(&mut self, account: &str) {
        if let Some(attested) = &self.attested
            && let Err(refusal) = self.state.bind(attested, account)
        {
            eprintln!(
                "key accounts: attestation not recorded: {}",
                refusal.message
            );
        }
    }
}

/// Whether any workspace `account` belongs to bought credit.
async fn paid(state: &ServeState, store: &tenancy::accounts::Store, account: &str) -> bool {
    let workspaces: Vec<&str> = store
        .workspaces
        .values()
        .filter(|ws| {
            ws.members
                .get(account)
                .is_some_and(|m| m.status == tenancy::MemberStatus::Active)
        })
        .map(|ws| ws.id.as_str())
        .collect();
    let Some(ledger) = state.money_lock().await else {
        return false;
    };
    workspaces.iter().any(|ws| {
        ledger
            .balance(ws)
            .is_ok_and(|balance| balance.purchased_funding > 0)
    })
}

/// The standing of `account` in `store`.
pub(crate) async fn standing_in(
    state: &ServeState,
    store: &tenancy::accounts::Store,
    account: &str,
) -> Standing {
    let Some(record) = store.accounts.get(account) else {
        return Standing::Proven;
    };
    if !key_only(&record.principals) {
        return Standing::Proven;
    }
    let attested = state.key_accounts.is_attested(account);
    standing(
        &record.principals,
        attested,
        paid(state, store, account).await,
    )
}

/// How long a standing read stays fresh, seconds.
const STANDING_TTL: u64 = 60;

/// The standing of `account`, reading the account store at most once a
/// minute. An account the store can't be read for counts as proven, as
/// before this module.
pub(crate) async fn standing_of(state: &ServeState, account: &str) -> Standing {
    let now = unix_now();
    if let Ok(standings) = state.key_accounts.standings.lock()
        && let Some((at, standing)) = standings.get(account)
        && now.saturating_sub(*at) < STANDING_TTL
    {
        return *standing;
    }
    let Some(store) = tenancy::Accounts::open(&state.dir)
        .ok()
        .and_then(|accounts| accounts.store().ok())
    else {
        return Standing::Proven;
    };
    let standing = standing_in(state, &store, account).await;
    if let Ok(mut standings) = state.key_accounts.standings.lock() {
        standings.retain(|_, (at, _)| now.saturating_sub(*at) < STANDING_TTL);
        standings.insert(account.to_owned(), (now, standing));
    }
    standing
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> String {
        format!("nostr:{}", "ab".repeat(32))
    }

    #[test]
    fn only_key_only_accounts_lose_standing() {
        assert_eq!(standing(&[key()], false, false), Standing::KeyOnly);
        assert_eq!(standing(&[key()], true, false), Standing::Attested);
        assert_eq!(standing(&[key()], false, true), Standing::Proven);
        assert_eq!(
            standing(&[key(), "github:42".into()], false, false),
            Standing::Proven
        );
        assert_eq!(
            standing(&["github:42".into()], false, false),
            Standing::Proven
        );
        assert_eq!(standing(&[], false, false), Standing::Proven);
        assert_eq!(Standing::Proven.free_cloud(), "full");
        assert_eq!(Standing::Attested.free_cloud(), "attested");
        assert_eq!(Standing::KeyOnly.free_cloud(), "none");
        assert!(Standing::Attested.under_breaker());
        assert!(Standing::KeyOnly.under_breaker());
        assert!(!Standing::Proven.under_breaker());
    }

    #[test]
    fn the_proof_of_work_needs_its_zero_bits() {
        let challenge = "nostr-challenge";
        let solution = (0_u64..)
            .map(|n| n.to_string())
            .find(|s| check_pow(challenge, s, 8).is_ok())
            .unwrap();
        assert!(check_pow(challenge, &solution, 8).is_ok());
        assert!(check_pow(challenge, "", 0).is_err());
        assert!(check_pow(challenge, &"x".repeat(65), 0).is_err());
        assert!(check_pow(challenge, "has space", 0).is_err());
        assert!(check_pow(challenge, "anything", 0).is_ok());
    }

    #[test]
    fn sign_ups_stop_at_each_cap_and_reset_daily() {
        let config = KeyAccounts {
            per_ip_per_day: 2,
            per_network_per_day: 3,
            per_day: 5,
            ..KeyAccounts::default()
        };
        let mut signups = Signups::default();
        let ip = |last: u8| Some(IpAddr::from([10, 0, 0, last]));
        assert!(signups.admit(&config, ip(1), 1).is_ok());
        assert!(signups.admit(&config, ip(1), 1).is_ok());
        let per_ip = signups.admit(&config, ip(1), 1).unwrap_err();
        assert!(per_ip.contains("address"));
        assert!(signups.admit(&config, ip(2), 1).is_ok());
        let per_net = signups.admit(&config, ip(3), 1).unwrap_err();
        assert!(per_net.contains("network"));
        assert!(
            signups
                .admit(&config, Some(IpAddr::from([10, 0, 1, 1])), 1)
                .is_ok()
        );
        assert!(signups.admit(&config, None, 1).is_ok());
        let global = signups.admit(&config, None, 1).unwrap_err();
        assert!(global.contains("today"));
        // A new day starts every count over.
        assert!(signups.admit(&config, ip(1), 2).is_ok());
    }

    #[test]
    fn defaults_apply_when_left_out() {
        let config: KeyAccounts = serde_json::from_str("{}").unwrap();
        assert_eq!(config.app_id, "HQWSG26L43.com.openagents.app");
        assert_eq!(config.difficulty_bits, 20);
        assert!(!config.allow_development);
        assert_eq!(config.free_usd_micros_per_day, 20_000_000);
    }
}
