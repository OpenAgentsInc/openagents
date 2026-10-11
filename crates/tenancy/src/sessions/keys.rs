//! Sign-in keys (#11305): a session the person makes in Settings, with a
//! name they choose, for an agent to act as their account without the
//! device code.
//!
//! A sign-in key is an ordinary user session in this book. Its bearer
//! token has its own prefix, `oa_sk_<64 hex>`, so secret scanners and
//! redaction can tell it from a `sess_` token; the record carries a
//! [`KeyLabel`] (name and last use); its deadline is the one the person
//! picked, or [`KEY_NEVER`]. Everything else is the session's: the id is
//! the SHA-256 of the token (the token is never stored), it resolves
//! through [`SessionBook::session_of_token`] like any session, and
//! revoking it, a credential reset, or `revoke_all` ends it.

use serde::{Deserialize, Serialize};

use super::{
    Issued, Refusal, Session, SessionBook, SessionId, SessionKind, SessionState, digest_secret,
    fresh,
};
use crate::workspaces::UserId;

/// The sign-in key's wire prefix: `oa_sk_<hex>`.
pub const SIGN_IN_KEY_PREFIX: &str = "oa_sk_";

/// The deadline of a key made to never expire: the last second of the
/// year 9999, so every reader's "is it past its deadline" stays a plain
/// comparison.
pub const KEY_NEVER: u64 = 253_402_300_799;

/// The longest name a key takes, in characters.
pub const KEY_NAME_MAX: usize = 80;

/// How many working keys one account may hold.
pub const KEYS_PER_ACCOUNT: usize = 50;

/// How stale a key's recorded last use may get before a use writes it
/// again: an hour, so a busy agent costs at most one write an hour.
pub const KEY_LAST_USED_EVERY: u64 = 3_600;

/// Whether `token` is presented as a session bearer: a `sess_` session
/// or an `oa_sk_` sign-in key. Both resolve through the same lookup.
#[must_use]
pub fn is_session_bearer(token: &str) -> bool {
    token.starts_with("sess_") || token.starts_with(SIGN_IN_KEY_PREFIX)
}

/// Whether `token` has exactly a session token's shape: `sess_` or
/// `oa_sk_` and 64 hex characters.
#[must_use]
pub fn is_session_token(token: &str) -> bool {
    token
        .strip_prefix("sess_")
        .or_else(|| token.strip_prefix(SIGN_IN_KEY_PREFIX))
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// What a sign-in key's session carries beyond any session's fields.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KeyLabel {
    /// The name the person gave it.
    pub name: String,
    /// When it was last used, to [`KEY_LAST_USED_EVERY`]; `None` until
    /// its first use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used: Option<u64>,
}

impl KeyLabel {
    /// A key's name: trimmed, 1 to [`KEY_NAME_MAX`] characters, no
    /// control characters.
    pub fn new(name: &str) -> Result<Self, Refusal> {
        let name = name.trim();
        if name.is_empty()
            || name.chars().count() > KEY_NAME_MAX
            || name.chars().any(char::is_control)
        {
            return Err(Refusal::KeyName);
        }
        Ok(Self {
            name: name.to_string(),
            last_used: None,
        })
    }
}

/// Compare two digests without stopping at the first difference.
pub(super) fn same_digest(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl SessionBook {
    /// Make a sign-in key for `user`: a user session named `label`,
    /// standing `ttl` seconds, or until revoked when `ttl` is `None`.
    /// The token is in `once`; the book keeps only its digest.
    pub fn issue_key(
        &mut self,
        user: UserId,
        label: KeyLabel,
        ttl: Option<u64>,
        now: u64,
    ) -> Result<Issued, Refusal> {
        if self.sign_in_keys(&user, now).len() >= KEYS_PER_ACCOUNT {
            return Err(Refusal::KeyLimit);
        }
        let token = format!("{SIGN_IN_KEY_PREFIX}{}", fresh()?);
        let session = Session {
            id: SessionId(digest_secret(&token)),
            user,
            kind: SessionKind::User,
            created_at: now,
            expires_at: ttl.map_or(KEY_NEVER, |ttl| now.saturating_add(ttl).min(KEY_NEVER)),
            state: SessionState::Active,
            closed_at: None,
            app: None,
            key: Some(label),
        };
        self.sessions.insert(session.id.clone(), session.clone());
        Ok(Issued {
            session,
            once: token,
        })
    }

    /// `user`'s working sign-in keys, newest first.
    #[must_use]
    pub fn sign_in_keys(&self, user: &UserId, now: u64) -> Vec<&Session> {
        let mut found: Vec<&Session> = self
            .sessions
            .values()
            .filter(|s| {
                s.user == *user && s.key.is_some() && s.standing(now) == SessionState::Active
            })
            .collect();
        found.sort_by_key(|s| std::cmp::Reverse((s.created_at, s.id.clone())));
        found
    }

    /// Revoke one of `user`'s own sign-in keys. Someone else's key, or a
    /// session that isn't a key, answers as unknown.
    pub fn revoke_key(
        &mut self,
        user: &UserId,
        id: &SessionId,
        now: u64,
    ) -> Result<Session, Refusal> {
        if self
            .sessions
            .get(id)
            .is_none_or(|s| s.user != *user || s.key.is_none())
        {
            return Err(Refusal::UnknownSession(id.clone()));
        }
        self.logout(id, now)
    }

    /// Whether a use of key `id` at `now` should be written: the key is
    /// working and its recorded last use is older than
    /// [`KEY_LAST_USED_EVERY`] (or missing).
    #[must_use]
    pub fn key_use_stale(&self, id: &SessionId, now: u64) -> bool {
        self.sessions.get(id).is_some_and(|s| {
            s.standing(now) == SessionState::Active
                && s.key.as_ref().is_some_and(|key| {
                    key.last_used
                        .is_none_or(|at| now.saturating_sub(at) >= KEY_LAST_USED_EVERY)
                })
        })
    }

    /// Record a use of key `id` at `now`, when [`Self::key_use_stale`].
    /// Answers whether anything changed.
    pub fn note_key_use(&mut self, id: &SessionId, now: u64) -> bool {
        if !self.key_use_stale(id, now) {
            return false;
        }
        let Some(key) = self.sessions.get_mut(id).and_then(|s| s.key.as_mut()) else {
            return false;
        };
        key.last_used = Some(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> SessionBook {
        SessionBook::new(1_000, 500)
    }

    fn me() -> UserId {
        UserId::from("acct_me")
    }

    #[test]
    fn a_key_is_a_user_session_with_its_own_prefix_and_no_stored_token() {
        let mut book = book();
        let issued = book
            .issue_key(me(), KeyLabel::new("ci agent").unwrap(), None, 100)
            .unwrap();
        assert!(issued.once.starts_with(SIGN_IN_KEY_PREFIX));
        assert!(is_session_token(&issued.once));
        assert!(is_session_bearer(&issued.once));
        let found = book.session_of_token(&issued.once).unwrap();
        assert_eq!(found.kind, SessionKind::User);
        assert_eq!(found.user, me());
        assert_eq!(found.expires_at, KEY_NEVER);
        assert_eq!(found.key.as_ref().unwrap().name, "ci agent");
        let stored = serde_json::to_string(&book).unwrap();
        assert!(!stored.contains(&issued.once), "only the digest is kept");
        assert!(!stored.contains(&issued.once[SIGN_IN_KEY_PREFIX.len()..]));
    }

    #[test]
    fn keys_expire_on_the_day_picked() {
        let mut book = book();
        let issued = book
            .issue_key(me(), KeyLabel::new("a").unwrap(), Some(30 * 86_400), 100)
            .unwrap();
        let id = issued.session.id.clone();
        assert_eq!(issued.session.expires_at, 100 + 30 * 86_400);
        assert_eq!(book.sign_in_keys(&me(), 200).len(), 1);
        let later = 100 + 30 * 86_400;
        assert_eq!(
            book.session(&id).unwrap().standing(later),
            SessionState::Expired
        );
        assert!(book.sign_in_keys(&me(), later).is_empty());
    }

    #[test]
    fn revoking_ends_only_your_own_key() {
        let mut book = book();
        let mine = book
            .issue_key(me(), KeyLabel::new("a").unwrap(), None, 100)
            .unwrap();
        let theirs = book
            .issue_key(
                UserId::from("acct_other"),
                KeyLabel::new("b").unwrap(),
                None,
                100,
            )
            .unwrap();
        let browser = book.issue(me(), 100).unwrap();
        assert!(matches!(
            book.revoke_key(&me(), &theirs.session.id, 150),
            Err(Refusal::UnknownSession(_))
        ));
        assert!(matches!(
            book.revoke_key(&me(), &browser.session.id, 150),
            Err(Refusal::UnknownSession(_))
        ));
        let ended = book.revoke_key(&me(), &mine.session.id, 150).unwrap();
        assert_eq!(ended.state, SessionState::Revoked);
        assert_eq!(
            book.session_of_token(&mine.once).unwrap().standing(151),
            SessionState::Revoked
        );
        assert!(book.sign_in_keys(&me(), 151).is_empty());
    }

    #[test]
    fn last_use_is_written_at_most_hourly() {
        let mut book = book();
        let issued = book
            .issue_key(me(), KeyLabel::new("a").unwrap(), None, 100)
            .unwrap();
        let id = issued.session.id;
        assert!(book.note_key_use(&id, 200));
        assert!(!book.note_key_use(&id, 300));
        assert!(book.note_key_use(&id, 200 + KEY_LAST_USED_EVERY));
        assert_eq!(
            book.session(&id).unwrap().key.as_ref().unwrap().last_used,
            Some(200 + KEY_LAST_USED_EVERY)
        );
        let browser = book.issue(me(), 100).unwrap();
        assert!(!book.note_key_use(&browser.session.id, 200));
    }

    #[test]
    fn names_are_checked_and_keys_are_bounded() {
        assert!(KeyLabel::new("  ").is_err());
        assert!(KeyLabel::new(&"x".repeat(KEY_NAME_MAX + 1)).is_err());
        assert!(KeyLabel::new("a\nb").is_err());
        let mut book = book();
        for _ in 0..KEYS_PER_ACCOUNT {
            book.issue_key(me(), KeyLabel::new("a").unwrap(), None, 100)
                .unwrap();
        }
        assert_eq!(
            book.issue_key(me(), KeyLabel::new("a").unwrap(), None, 100)
                .unwrap_err(),
            Refusal::KeyLimit
        );
    }

    #[test]
    fn token_shapes() {
        assert!(is_session_token(&format!("sess_{}", "a".repeat(64))));
        assert!(is_session_token(&format!("oa_sk_{}", "0".repeat(64))));
        assert!(!is_session_token(&format!("oa_sk_{}", "0".repeat(63))));
        assert!(!is_session_token("oak_abc.def"));
        assert!(!is_session_bearer("oak_abc.def"));
    }
}
