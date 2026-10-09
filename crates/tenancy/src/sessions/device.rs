//! Device sign-in: an app on another computer (Coder in a terminal, an
//! SSH session) asks for a session, a signed-in person approves it on the
//! website, and the app picks the session up. The shape is RFC 8628's
//! device authorization grant.
//!
//! - [`SessionBook::start_device`] mints a `device_code` (the app's
//!   secret, polled with) and a short `user_code` (what the person types
//!   on `/device`). Both are kept only as SHA-256 digests; each grant
//!   stands [`DEVICE_CODE_TTL`] seconds.
//! - [`SessionBook::decide_device`] is the person's Approve or Deny,
//!   made under their own signed-in session.
//! - [`SessionBook::poll_device`] is the app's poll. An approved grant
//!   issues one session, exactly once, labeled with the app and the
//!   computer, and the grant is spent. Polling faster than the grant's
//!   interval answers `slow_down` and widens the interval.
//! - A grant can carry a pair code ([`valid_pair`]): the website's
//!   "Connect your terminal" page shows the person `coder login --pair
//!   <code>`, then lists the grants started with it
//!   ([`SessionBook::paired_devices`]) so the person approves right there.
//!   The pair code is kept as a digest and the user code under it only
//!   masked with the pair code, so the store still holds no code.
//! - App sessions are ordinary `user` sessions with an [`AppLabel`], so
//!   every route that takes a `sess_` token takes them; they stand
//!   [`APP_SESSION_TTL`] and are listed and revoked one by one
//!   ([`SessionBook::app_sessions`], [`SessionBook::revoke_session`]).

use serde::{Deserialize, Serialize};

use super::{
    Issued, Refusal, Session, SessionBook, SessionId, SessionKind, SessionState, digest_secret,
    fresh,
};
use crate::workspaces::UserId;

/// How long a device grant can be approved and picked up: ten minutes.
pub const DEVICE_CODE_TTL: u64 = 600;

/// The polling interval a grant starts with, in seconds.
pub const DEVICE_POLL_INTERVAL: u64 = 5;

/// How long an app session stands: thirty days. Settings revokes it
/// sooner; signing out of the app ends it at once.
pub const APP_SESSION_TTL: u64 = 30 * 86_400;

/// How long a finished grant stays in the book before it is pruned.
const DEVICE_KEEP: u64 = 86_400;

/// The most grants waiting at once, so a flood of starts cannot grow
/// the store without bound.
const DEVICE_LIVE_MAX: usize = 2_048;

/// The device code's wire prefix: `dvc_<hex>`.
const DEVICE_PREFIX: &str = "dvc";

/// The user code's letters: consonants only, no vowels (no words), no
/// look-alikes. RFC 8628 section 6.1's suggestion.
const USER_CODE_ALPHABET: &[u8] = b"BCDFGHJKLMNPQRSTVWXZ";

/// User code length, without the dash.
const USER_CODE_LEN: usize = 8;

/// What a device session is: which app, on which computer. Both are the
/// app's own words, bounded and stripped of control characters.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AppLabel {
    /// The app's name, such as `Coder`.
    pub app: String,
    /// The computer's name, as the app reported it.
    pub computer: String,
}

impl AppLabel {
    /// A label from an app's request. Empty or overlong names are
    /// refused; control characters are dropped.
    pub fn new(app: &str, computer: &str) -> Result<Self, Refusal> {
        let clean = |value: &str| -> Option<String> {
            let value: String = value.chars().filter(|c| !c.is_control()).collect();
            let value = value.trim().to_string();
            (!value.is_empty() && value.chars().count() <= 64).then_some(value)
        };
        Ok(Self {
            app: clean(app).ok_or(Refusal::Device(DeviceRefusal::InvalidLabel))?,
            computer: clean(computer).ok_or(Refusal::Device(DeviceRefusal::InvalidLabel))?,
        })
    }
}

/// Where a device grant stands.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceState {
    /// Waiting for the person.
    Pending,
    /// Approved; the next poll picks up the session.
    Approved,
    /// The person said no.
    Denied,
    /// The session was issued; the grant is spent.
    Redeemed,
}

/// One device sign-in in progress (or finished).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeviceGrant {
    /// SHA-256 of the device code, hex — the key the book files it under.
    pub id: String,
    /// SHA-256 of the normalized user code, hex.
    pub user_code: String,
    /// The app and computer asking.
    pub label: AppLabel,
    pub created_at: u64,
    pub expires_at: u64,
    /// Seconds the app must wait between polls.
    pub interval: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_poll: Option<u64>,
    pub state: DeviceState,
    /// The account that approved or denied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<UserId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<u64>,
    /// The session the grant issued, once redeemed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionId>,
    /// The pair code the app started with, for the page that shows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pair: Option<Paired>,
}

/// A grant started with a pair code: the code's digest, and the user
/// code masked with the pair code (hex), so only someone holding the pair
/// code reads it back.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Paired {
    pub id: String,
    pub code: String,
}

/// Whether `pair` can be a pair code: 16 to 64 ASCII letters and digits.
#[must_use]
pub fn valid_pair(pair: &str) -> bool {
    (16..=64).contains(&pair.len()) && pair.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// The pad a pair code masks its grant's user code with.
fn pair_pad(pair: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"openagents.device.pair.v1\0");
    hash.update(pair.as_bytes());
    hash.finalize().into()
}

fn mask(code: &str, pair: &str) -> String {
    code.bytes()
        .zip(pair_pad(pair))
        .map(|(byte, pad)| format!("{:02x}", byte ^ pad))
        .collect()
}

fn unmask(masked: &str, pair: &str) -> Option<String> {
    if masked.len() != USER_CODE_LEN * 2 {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..masked.len())
        .step_by(2)
        .zip(pair_pad(pair))
        .map(|(at, pad)| {
            u8::from_str_radix(masked.get(at..at + 2)?, 16)
                .ok()
                .map(|b| b ^ pad)
        })
        .collect();
    let code = normalize_user_code(&String::from_utf8(bytes?).ok()?)?;
    Some(format!("{}-{}", &code[..4], &code[4..]))
}

impl DeviceGrant {
    fn expired(&self, now: u64) -> bool {
        self.expires_at <= now
    }
}

/// What `start_device` hands back: the record and, once, both codes.
pub struct DeviceIssued {
    pub grant: DeviceGrant,
    /// `dvc_<hex>` — the app's secret for polling.
    pub device_code: String,
    /// `BCDF-GHJK` — what the person types.
    pub user_code: String,
}

impl std::fmt::Debug for DeviceIssued {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceIssued")
            .field("grant", &self.grant)
            .field("device_code", &"[redacted]")
            .field("user_code", &"[redacted]")
            .finish()
    }
}

/// A poll's answer when nothing is refused.
#[derive(Debug)]
pub enum DevicePoll {
    /// Approved: the session, issued once.
    Issued(Issued, UserId),
    /// Not decided yet (`authorization_pending`).
    Pending,
    /// Polled too soon (`slow_down`); the new interval.
    SlowDown(u64),
}

/// Why a device step was refused. The codes are RFC 8628's where it has
/// one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceRefusal {
    /// No grant has that code (`invalid_grant`).
    UnknownCode,
    /// The grant's ten minutes are up (`expired_token`).
    Expired,
    /// The person denied it (`access_denied`).
    Denied,
    /// The grant was already used or decided.
    Closed,
    /// The app or computer name is empty or too long.
    InvalidLabel,
    /// Too many sign-ins are waiting; try again shortly.
    Busy,
}

impl DeviceRefusal {
    /// The stable wire code.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::UnknownCode => "invalid_grant",
            Self::Expired => "expired_token",
            Self::Denied => "access_denied",
            Self::Closed => "invalid_grant",
            Self::InvalidLabel => "invalid_request",
            Self::Busy => "slow_down",
        }
    }
}

impl std::fmt::Display for DeviceRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnknownCode => "That code isn't right. Check it and try again.",
            Self::Expired => "That code expired. Start sign-in again on your computer.",
            Self::Denied => "Sign-in was denied.",
            Self::Closed => "That code was already used.",
            Self::InvalidLabel => "Send the app and computer names.",
            Self::Busy => "Too many sign-ins are waiting. Try again in a minute.",
        })
    }
}

/// A typed user code in canonical form: letters only, upper case. A
/// code that cannot be one answers `None`.
#[must_use]
pub fn normalize_user_code(typed: &str) -> Option<String> {
    let code: String = typed
        .chars()
        .filter(|c| !matches!(c, '-' | ' ' | '\t'))
        .map(|c| c.to_ascii_uppercase())
        .collect();
    (code.len() == USER_CODE_LEN && code.bytes().all(|b| USER_CODE_ALPHABET.contains(&b)))
        .then_some(code)
}

/// A fresh user code, `XXXX-XXXX`.
fn fresh_user_code() -> Result<String, Refusal> {
    let mut out = String::with_capacity(USER_CODE_LEN + 1);
    // 240 is the largest multiple of 20 at most 256: reject above it so
    // every letter is equally likely.
    let mut bytes = [0_u8; 32];
    while out.len() < USER_CODE_LEN + 1 {
        getrandom::fill(&mut bytes).map_err(|_| Refusal::Unavailable)?;
        for byte in bytes {
            if byte >= 240 {
                continue;
            }
            if out.len() == 4 {
                out.push('-');
            }
            out.push(USER_CODE_ALPHABET[usize::from(byte % 20)] as char);
            if out.len() == USER_CODE_LEN + 1 {
                break;
            }
        }
    }
    Ok(out)
}

impl SessionBook {
    /// Start a device sign-in for `label`. Prunes long-finished grants.
    pub fn start_device(&mut self, label: AppLabel, now: u64) -> Result<DeviceIssued, Refusal> {
        self.start_paired_device(label, None, now)
    }

    /// [`Self::start_device`] with an optional pair code ([`valid_pair`];
    /// an invalid one is ignored).
    pub fn start_paired_device(
        &mut self,
        label: AppLabel,
        pair: Option<&str>,
        now: u64,
    ) -> Result<DeviceIssued, Refusal> {
        self.devices
            .retain(|_, grant| grant.expires_at + DEVICE_KEEP > now);
        let live = self
            .devices
            .values()
            .filter(|grant| !grant.expired(now) && grant.state == DeviceState::Pending)
            .count();
        if live >= DEVICE_LIVE_MAX {
            return Err(Refusal::Device(DeviceRefusal::Busy));
        }
        let device_code = format!("{DEVICE_PREFIX}_{}", fresh()?);
        let user_code = loop {
            let code = fresh_user_code()?;
            let digest = digest_secret(&normalize_user_code(&code).expect("fresh codes are valid"));
            if !self
                .devices
                .values()
                .any(|grant| grant.user_code == digest && !grant.expired(now))
            {
                break code;
            }
        };
        let grant = DeviceGrant {
            id: digest_secret(&device_code),
            user_code: digest_secret(&normalize_user_code(&user_code).expect("valid")),
            label,
            created_at: now,
            expires_at: now + DEVICE_CODE_TTL,
            interval: DEVICE_POLL_INTERVAL,
            last_poll: None,
            state: DeviceState::Pending,
            account: None,
            decided_at: None,
            session: None,
            pair: pair.filter(|pair| valid_pair(pair)).map(|pair| Paired {
                id: digest_secret(pair),
                code: mask(&normalize_user_code(&user_code).expect("valid"), pair),
            }),
        };
        self.devices.insert(grant.id.clone(), grant.clone());
        Ok(DeviceIssued {
            grant,
            device_code,
            user_code,
        })
    }

    /// The pending grant a typed user code names, for the approval page.
    pub fn device_of_user_code(&self, typed: &str, now: u64) -> Result<&DeviceGrant, Refusal> {
        let digest = normalize_user_code(typed)
            .map(|code| digest_secret(&code))
            .ok_or(Refusal::Device(DeviceRefusal::UnknownCode))?;
        let grant = self
            .devices
            .values()
            .filter(|grant| grant.user_code == digest)
            .max_by_key(|grant| grant.created_at)
            .ok_or(Refusal::Device(DeviceRefusal::UnknownCode))?;
        if grant.state != DeviceState::Pending {
            return Err(Refusal::Device(DeviceRefusal::Closed));
        }
        if grant.expired(now) {
            return Err(Refusal::Device(DeviceRefusal::Expired));
        }
        Ok(grant)
    }

    /// The grants waiting for approval that were started with `pair`,
    /// newest first, each with its user code (`BCDF-GHJK`).
    #[must_use]
    pub fn paired_devices(&self, pair: &str, now: u64) -> Vec<(&DeviceGrant, String)> {
        if !valid_pair(pair) {
            return Vec::new();
        }
        let id = digest_secret(pair);
        let mut found: Vec<(&DeviceGrant, String)> = self
            .devices
            .values()
            .filter(|grant| grant.state == DeviceState::Pending && !grant.expired(now))
            .filter_map(|grant| {
                let paired = grant.pair.as_ref().filter(|paired| paired.id == id)?;
                Some((grant, unmask(&paired.code, pair)?))
            })
            .collect();
        found.sort_by_key(|(grant, _)| std::cmp::Reverse(grant.created_at));
        found
    }

    /// The signed-in `account` approves (or denies) the grant behind a
    /// typed user code.
    pub fn decide_device(
        &mut self,
        typed: &str,
        account: &UserId,
        approve: bool,
        now: u64,
    ) -> Result<DeviceGrant, Refusal> {
        let id = self.device_of_user_code(typed, now)?.id.clone();
        let grant = self.devices.get_mut(&id).expect("just found");
        grant.state = if approve {
            DeviceState::Approved
        } else {
            DeviceState::Denied
        };
        grant.account = Some(account.clone());
        grant.decided_at = Some(now);
        Ok(grant.clone())
    }

    /// The app's poll with its device code.
    pub fn poll_device(&mut self, device_code: &str, now: u64) -> Result<DevicePoll, Refusal> {
        let id = digest_secret(device_code);
        let grant = self
            .devices
            .get_mut(&id)
            .ok_or(Refusal::Device(DeviceRefusal::UnknownCode))?;
        match grant.state {
            DeviceState::Redeemed => return Err(Refusal::Device(DeviceRefusal::Closed)),
            DeviceState::Denied => return Err(Refusal::Device(DeviceRefusal::Denied)),
            _ if grant.expired(now) => return Err(Refusal::Device(DeviceRefusal::Expired)),
            DeviceState::Approved => {}
            DeviceState::Pending => {
                let early = grant
                    .last_poll
                    .is_some_and(|last| now < last + grant.interval);
                grant.last_poll = Some(now);
                if early {
                    grant.interval = (grant.interval + 5).min(60);
                    return Ok(DevicePoll::SlowDown(grant.interval));
                }
                return Ok(DevicePoll::Pending);
            }
        }
        let account = grant
            .account
            .clone()
            .expect("an approved grant names its account");
        let label = grant.label.clone();
        let token = format!("sess_{}", fresh()?);
        let session = Session {
            id: SessionId(digest_secret(&token)),
            user: account.clone(),
            kind: SessionKind::User,
            created_at: now,
            expires_at: now + APP_SESSION_TTL,
            state: SessionState::Active,
            closed_at: None,
            app: Some(label),
        };
        let grant = self.devices.get_mut(&id).expect("still here");
        grant.state = DeviceState::Redeemed;
        grant.last_poll = Some(now);
        grant.session = Some(session.id.clone());
        self.sessions.insert(session.id.clone(), session.clone());
        Ok(DevicePoll::Issued(
            Issued {
                session,
                once: token,
            },
            account,
        ))
    }

    /// The account's active app sessions, newest first.
    #[must_use]
    pub fn app_sessions(&self, user: &UserId, now: u64) -> Vec<&Session> {
        let mut found: Vec<&Session> = self
            .sessions
            .values()
            .filter(|s| {
                s.user == *user && s.app.is_some() && s.standing(now) == SessionState::Active
            })
            .collect();
        found.sort_by_key(|s| std::cmp::Reverse(s.created_at));
        found
    }

    /// End one of `user`'s own sessions. A session that belongs to
    /// someone else answers as unknown.
    pub fn revoke_session(
        &mut self,
        user: &UserId,
        id: &SessionId,
        now: u64,
    ) -> Result<Session, Refusal> {
        if self.sessions.get(id).is_none_or(|s| s.user != *user) {
            return Err(Refusal::UnknownSession(id.clone()));
        }
        self.logout(id, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label() -> AppLabel {
        AppLabel::new("Coder", "chris-mbp").unwrap()
    }

    #[test]
    fn user_codes_are_short_unambiguous_and_typed_loosely() {
        let code = fresh_user_code().unwrap();
        assert_eq!(code.len(), 9);
        assert_eq!(&code[4..5], "-");
        assert!(normalize_user_code(&code).is_some());
        assert_eq!(
            normalize_user_code("bcdf ghjk").as_deref(),
            Some("BCDFGHJK")
        );
        assert_eq!(normalize_user_code("BCDF-GHJ"), None);
        assert_eq!(
            normalize_user_code("ABCD-EFGH"),
            None,
            "vowels never appear"
        );
    }

    #[test]
    fn stores_written_before_devices_keep_their_shape() {
        // Older `sessions.json` documents have no `devices` and no `app`;
        // re-serializing them must not add either, or their digest moves.
        let mut book = SessionBook::new(100, 100);
        book.issue(UserId::from("a"), 1).unwrap();
        let text = serde_json::to_string(&book).unwrap();
        assert!(!text.contains("devices"));
        assert!(!text.contains("\"app\""));
    }

    #[test]
    fn labels_are_bounded_and_cleaned() {
        assert_eq!(
            AppLabel::new("Coder", "my\u{7}box").unwrap().computer,
            "mybox"
        );
        assert!(AppLabel::new("", "x").is_err());
        assert!(AppLabel::new("Coder", &"x".repeat(65)).is_err());
    }

    #[test]
    fn approve_then_poll_issues_one_labeled_session_once() {
        let mut book = SessionBook::new(100, 100);
        let issued = book.start_device(label(), 1_000).unwrap();
        assert!(issued.device_code.starts_with("dvc_"));
        assert!(!format!("{issued:?}").contains(&issued.device_code));
        // Nothing stored is a code.
        let stored = serde_json::to_string(&book).unwrap();
        assert!(!stored.contains(&issued.device_code));
        assert!(!stored.contains(&issued.user_code));

        assert!(matches!(
            book.poll_device(&issued.device_code, 1_000),
            Ok(DevicePoll::Pending)
        ));
        // Too soon: slow down, and the interval widens.
        assert!(matches!(
            book.poll_device(&issued.device_code, 1_002),
            Ok(DevicePoll::SlowDown(10))
        ));

        let account = UserId::from("acct_1");
        let shown = book
            .device_of_user_code(&issued.user_code.to_lowercase(), 1_010)
            .unwrap();
        assert_eq!(shown.label.computer, "chris-mbp");
        book.decide_device(&issued.user_code, &account, true, 1_010)
            .unwrap();
        // A decided code can't be decided again.
        assert_eq!(
            book.decide_device(&issued.user_code, &account, false, 1_011)
                .unwrap_err(),
            Refusal::Device(DeviceRefusal::Closed)
        );

        let DevicePoll::Issued(session, who) =
            book.poll_device(&issued.device_code, 1_020).unwrap()
        else {
            panic!("approved grant issues");
        };
        assert_eq!(who, account);
        assert!(session.once.starts_with("sess_"));
        assert_eq!(session.session.expires_at, 1_020 + APP_SESSION_TTL);
        assert_eq!(session.session.app, Some(label()));
        assert_eq!(book.session_of_token(&session.once).unwrap().user, account);
        // Exactly once.
        assert_eq!(
            book.poll_device(&issued.device_code, 1_030).unwrap_err(),
            Refusal::Device(DeviceRefusal::Closed)
        );
        assert_eq!(book.app_sessions(&account, 1_030).len(), 1);
        assert!(book.app_sessions(&UserId::from("acct_2"), 1_030).is_empty());

        // Someone else can't revoke it; the owner can.
        let id = session.session.id.clone();
        assert!(
            book.revoke_session(&UserId::from("acct_2"), &id, 1_040)
                .is_err()
        );
        book.revoke_session(&account, &id, 1_040).unwrap();
        assert!(book.app_sessions(&account, 1_040).is_empty());
    }

    #[test]
    fn a_paired_grant_lists_under_its_pair_code_only_and_stores_no_code() {
        let mut book = SessionBook::new(100, 100);
        let pair = "k7qxm2tz9w4v8r3n";
        let issued = book
            .start_paired_device(label(), Some(pair), 1_000)
            .unwrap();
        book.start_device(label(), 1_001).unwrap();
        let stored = serde_json::to_string(&book).unwrap();
        assert!(!stored.contains(&issued.user_code));
        assert!(!stored.contains(&normalize_user_code(&issued.user_code).unwrap()));
        assert!(!stored.contains(pair));
        let found = book.paired_devices(pair, 1_010);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1, issued.user_code);
        assert_eq!(found[0].0.label.computer, "chris-mbp");
        assert!(book.paired_devices("someoneelsespair1", 1_010).is_empty());
        assert!(book.paired_devices("short", 1_010).is_empty());
        // Approved, it leaves the list; expired ones never show.
        book.decide_device(&issued.user_code, &UserId::from("a"), true, 1_011)
            .unwrap();
        assert!(book.paired_devices(pair, 1_012).is_empty());
        let late = book
            .start_paired_device(label(), Some(pair), 2_000)
            .unwrap();
        assert_eq!(book.paired_devices(pair, 2_001)[0].1, late.user_code);
        assert!(
            book.paired_devices(pair, 2_000 + DEVICE_CODE_TTL)
                .is_empty()
        );
        // An invalid pair code is ignored.
        let plain = book
            .start_paired_device(label(), Some("x y"), 3_000)
            .unwrap();
        assert!(plain.grant.pair.is_none());
    }

    #[test]
    fn denied_expired_and_unknown_codes_answer_their_rfc_errors() {
        let mut book = SessionBook::new(100, 100);
        let denied = book.start_device(label(), 1_000).unwrap();
        book.decide_device(&denied.user_code, &UserId::from("a"), false, 1_001)
            .unwrap();
        let refusal = book.poll_device(&denied.device_code, 1_010).unwrap_err();
        assert_eq!(refusal, Refusal::Device(DeviceRefusal::Denied));
        assert_eq!(
            match refusal {
                Refusal::Device(d) => d.code(),
                _ => "",
            },
            "access_denied"
        );

        let late = book.start_device(label(), 1_000).unwrap();
        assert_eq!(
            book.poll_device(&late.device_code, 1_000 + DEVICE_CODE_TTL)
                .unwrap_err(),
            Refusal::Device(DeviceRefusal::Expired)
        );
        assert_eq!(
            book.decide_device(
                &late.user_code,
                &UserId::from("a"),
                true,
                1_000 + DEVICE_CODE_TTL
            )
            .unwrap_err(),
            Refusal::Device(DeviceRefusal::Expired)
        );
        assert_eq!(
            book.poll_device("dvc_nope", 1_000).unwrap_err(),
            Refusal::Device(DeviceRefusal::UnknownCode)
        );
        // Old grants are pruned on the next start.
        book.start_device(label(), 1_000 + DEVICE_CODE_TTL + DEVICE_KEEP)
            .unwrap();
        assert_eq!(book.devices.len(), 1);
    }
}
