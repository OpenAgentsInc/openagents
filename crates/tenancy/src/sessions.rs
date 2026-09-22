//! Session and credential lifecycle records: who signed in, for how
//! long, and what ends the claim.
//!
//! A session is a claim that someone signed in. The claim is bounded
//! in time — every record carries the deadline it was issued under,
//! and an expired session answers `expired` whether or not anything
//! marked it — and revocable as a set: a logout ends one session, a
//! credential reset or a membership removal ends every session the
//! user holds, because a session must never outlive the standing it
//! was minted against.
//!
//! The store keeps digests because a stolen store must not become a
//! list of live tokens. A session's id is the SHA-256 of its bearer
//! token, a credential is the digest of the secret a sign-in proves,
//! and a recovery token stores only its digest — the token leaves the
//! book once, in the response that issued it, the same discipline
//! `tenancy::keys` keeps for API keys.
//!
//! # The records
//!
//! - [`Credential`] binds a user to the digest their sign-in proof
//!   verifies against. Replacing it ends the sessions the old one
//!   signed in.
//! - [`Session`] is the claim itself: a digest id, a user, an issued
//!   time, a deadline, and a state — `active`, `expired`, or
//!   `revoked`, the last two being answers a caller can tell apart.
//! - [`Recovery`] is the single-use way back in: a bearer token bound
//!   to a user that resets the credential and dies doing it.
//! - [`Onboarding`] is the anonymous first call, funded: an operator's
//!   stated bound and the abuse counters that keep one session from
//!   draining it — never an implicit free-for-all.
//!
//! # Persistence
//!
//! The book itself is pure records: every timestamp arrives as an
//! argument in Unix seconds, and the only entropy the module touches is
//! minting bearer tokens. [`Sessions`] is the store that persists it —
//! `sessions.json` beside `accounts.json`, under the same discipline:
//! schema tag, sequence, `supersedes` chain, self-recomputing digest,
//! an archive of every sealed revision, and a writer lock. The store
//! also carries the [`Access`] history: who signed in, who acted, and
//! where — references only, never a secret.
//!
//! # What this is not
//!
//! There is no transport here, and membership itself is
//! `tenancy::workspaces`'s record — this module only reads it, in
//! [`Session::active_for`], so a removed member's session cannot keep
//! answering.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::workspaces::{Membership, UserId, Workspace, WorkspaceId};

/// The session token's wire prefix, so a pasted token announces its
/// shape: `sess_<hex>`.
const SESSION_PREFIX: &str = "sess";

/// The recovery token's wire prefix: `rcv_<hex>`.
const RECOVERY_PREFIX: &str = "rcv";

/// A session's id — the SHA-256 digest of its bearer token, hex.
///
/// The digest is the only handle a record keeps: presenting the token
/// resolves to this id by hashing, so the token itself is never stored
/// and a stolen store is not a list of live sessions.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct SessionId(pub String);

impl SessionId {
    /// The id as a string — for display and persistence, never
    /// a way to compare across types.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for SessionId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl From<&str> for SessionId {
    fn from(id: &str) -> Self {
        Self(id.to_string())
    }
}

/// Where a session stands. `expired` and `revoked` are distinct
/// answers — a session that ran out of time and a session that was
/// ended are not both "invalid".
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionState {
    /// Answers `active_for`.
    Active,
    /// The deadline passed — set by [`SessionBook::expire`], or derived
    /// from `expires_at` for a record nobody swept.
    Expired,
    /// Ended — by logout, by credential reset, or by revocation of the
    /// user's whole set.
    Revoked,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Active => "active",
            Self::Expired => "expired",
            Self::Revoked => "revoked",
        })
    }
}

/// What a session may act as.
///
/// A `user` session claims an account signed in and answers only what
/// its memberships permit. An `anonymous` session is the funded public
/// lane — it holds no account, no membership, and may only draw down an
/// operator-funded [`Onboarding`] budget.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionKind {
    /// An account signed in — the default every session was before
    /// the anonymous lane existed.
    #[default]
    User,
    /// The funded public caller — bounded by an [`Onboarding`] record,
    /// never by a membership.
    Anonymous,
}

/// A session: the claim that `user` signed in, bounded in time.
///
/// The id is the bearer token's digest — the token itself is never
/// here — so the record is safe to log and keep, and presenting the
/// token resolves to this record by hashing.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Session {
    /// The session's id — SHA-256 of the bearer token, hex.
    pub id: SessionId,
    /// The user the session claims signed in.
    pub user: UserId,
    /// `user` or `anonymous` — what the session may act as.
    #[serde(default)]
    pub kind: SessionKind,
    /// When the session was issued, as Unix seconds.
    pub created_at: u64,
    /// Unix seconds at which the session stops answering — the
    /// comparison [`Session::standing`] makes against the caller's
    /// `now`.
    pub expires_at: u64,
    /// `active`, `expired`, or `revoked`.
    pub state: SessionState,
    /// When the session last left `active`, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<u64>,
}

impl Session {
    /// Where the session stands at `now`. An `active` record past its
    /// deadline answers `expired` whether or not `expire` swept it —
    /// expiry is a comparison, not a mark somebody had to make.
    #[must_use]
    pub fn standing(&self, now: u64) -> SessionState {
        match self.state {
            SessionState::Active if self.expires_at <= now => SessionState::Expired,
            state => state,
        }
    }

    /// Whether the session may act for `workspace` at `now` — the
    /// session must stand active and the user must hold an active
    /// membership there.
    ///
    /// This is the check a removed member fails: the membership read
    /// happens inside, so a session minted while the user belonged
    /// stops answering the moment the membership does — it never
    /// outlives the standing it was minted against.
    pub fn active_for<'w>(
        &self,
        workspace: &'w Workspace,
        now: u64,
    ) -> Result<&'w Membership, Refusal> {
        if self.kind == SessionKind::Anonymous {
            return Err(Refusal::AnonymousSession);
        }
        match self.standing(now) {
            SessionState::Active => {}
            state => return Err(Refusal::SessionClosed { state }),
        }
        Ok(workspace.active_membership(&self.user)?)
    }
}

/// A user's sign-in credential — the digest a presented proof verifies
/// against. The secret itself is never here.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Credential {
    /// The user the credential signs in.
    pub user: UserId,
    /// SHA-256 of the secret, hex.
    pub digest: String,
    /// When the credential was last set, as Unix seconds.
    pub set_at: u64,
}

/// Where a recovery token stands.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecoveryState {
    /// Issued, not yet presented.
    Pending,
    /// Presented once and consumed — a second use is a replay, not a
    /// recovery.
    Consumed,
    /// A newer token for the same user retired it unconsumed.
    Superseded,
}

impl std::fmt::Display for RecoveryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Pending => "pending",
            Self::Consumed => "consumed",
            Self::Superseded => "superseded",
        })
    }
}

/// An account-recovery token's record: single-use, expiring, bound to
/// the user, and — like every secret here — kept as a digest only.
///
/// At most one token per user is live: issuing a new one supersedes
/// the unconsumed predecessor, so the older token cannot be replayed
/// after the newer exists.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Recovery {
    /// SHA-256 of the bearer token, hex — the record's lookup key, so
    /// an unknown id and a wrong token are the same answer.
    pub token_digest: String,
    /// The user the token recovers.
    pub user: UserId,
    /// `pending`, `consumed`, or `superseded`.
    pub state: RecoveryState,
    /// When the token was issued, as Unix seconds.
    pub issued_at: u64,
    /// Unix seconds after which the token refuses use.
    pub expires_at: u64,
    /// When the token was answered, when it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answered_at: Option<u64>,
}

/// An operator-funded anonymous budget: the onboarding first call,
/// bounded and counted.
///
/// Anonymous reach is a stated allowance, never an implicit
/// free-for-all: `bound` caps what the funding answers in total and
/// `session_cap` caps what one session may draw, so the record is the
/// abuse counter as well as the budget.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Onboarding {
    /// The budget's id — `onb_<hex>` by convention.
    pub id: String,
    /// The workspace the funding is attached to — a reference, never
    /// a credential.
    pub workspace: WorkspaceId,
    /// The operator that funded the budget — a reference, never a
    /// credential.
    pub funded_by: String,
    /// The stated bound: how many anonymous requests the funding
    /// answers in total.
    pub bound: u64,
    /// How many requests the funding has answered.
    pub spent: u64,
    /// The most one session may draw — the cap the abuse counters
    /// count against.
    pub session_cap: u64,
    /// Session id to requests drawn — the abuse counters.
    #[serde(default)]
    pub sessions: BTreeMap<String, u64>,
    /// When the funding was recorded, as Unix seconds.
    pub funded_at: u64,
    /// Unix seconds after which the funding stops answering.
    pub expires_at: u64,
}

impl Onboarding {
    /// How many requests the funding still answers.
    #[must_use]
    pub fn remaining(&self) -> u64 {
        self.bound.saturating_sub(self.spent)
    }

    /// Draw one anonymous request against the budget on `session`'s
    /// counter.
    ///
    /// The funding answers while it is live, unspent, and the session
    /// under its own cap; each refusal names what ran out.
    pub fn spend(&mut self, session: &str, now: u64) -> Result<(), Refusal> {
        if self.expires_at <= now {
            return Err(Refusal::AnonymousBudgetExpired {
                budget: self.id.clone(),
            });
        }
        if self.spent >= self.bound {
            return Err(Refusal::AnonymousBudgetSpent {
                budget: self.id.clone(),
                bound: self.bound,
            });
        }
        let drawn = self.sessions.entry(session.to_string()).or_insert(0);
        if *drawn >= self.session_cap {
            return Err(Refusal::AnonymousSessionCapped {
                budget: self.id.clone(),
                cap: self.session_cap,
            });
        }
        *drawn += 1;
        self.spent += 1;
        Ok(())
    }
}

/// What `sign_in` hands back: the record and, once, the token.
pub struct Issued {
    /// The stored record — safe to log and keep.
    pub session: Session,
    /// The bearer token, `sess_<hex>`. This is the only place the token
    /// exists outside the holder's hands: the record keeps its digest
    /// as the session id, and the book cannot reproduce it.
    pub once: String,
}

impl std::fmt::Debug for Issued {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Issued")
            .field("session", &self.session)
            .field("once", &"[redacted]")
            .finish()
    }
}

/// What `issue_recovery` hands back: the record and, once, the token.
pub struct RecoveryIssued {
    /// The stored record — safe to log and keep.
    pub recovery: Recovery,
    /// The bearer token, `rcv_<hex>`. This is the only place the token
    /// exists outside the holder's hands; the book cannot reproduce it.
    pub once: String,
}

impl std::fmt::Debug for RecoveryIssued {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecoveryIssued")
            .field("recovery", &self.recovery)
            .field("once", &"[redacted]")
            .finish()
    }
}

/// Why a session, recovery, or anonymous-budget operation was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// The sign-in was denied: the user holds no credential, or the
    /// proof did not verify against it. Deliberately one answer — the
    /// refusal does not say which half failed.
    SignInDenied,
    /// Fresh randomness was not available to mint a token — the caller
    /// may retry.
    Unavailable,
    /// A digest field is not 64 hex characters — a SHA-256, never the
    /// secret; the field name says which.
    MalformedDigest(&'static str),
    /// The user holds no credential — there is nothing to recover.
    UnknownUser(UserId),
    /// The id names no session the book holds.
    UnknownSession(SessionId),
    /// The session already ended — `expired` and `revoked` are
    /// distinct answers, not both "invalid".
    SessionClosed { state: SessionState },
    /// An anonymous session holds no membership — `active_for` is the
    /// wrong question for it.
    AnonymousSession,
    /// The store itself failed — not a session answer. The message is
    /// the store's own; the variant says the failure was storage, not
    /// the session.
    Store(String),
    /// The digest names no recovery the book holds — an unknown id and
    /// a wrong token are the same answer.
    UnknownRecovery,
    /// The recovery token's deadline has passed.
    RecoveryExpired,
    /// The recovery token was already answered — a second use is a
    /// replay, not a recovery.
    RecoveryClosed { state: RecoveryState },
    /// The id already names an anonymous budget the book holds.
    DuplicateBudget { budget: String },
    /// The id names no anonymous budget the book holds.
    UnknownBudget { budget: String },
    /// The anonymous budget's funding window has closed.
    AnonymousBudgetExpired { budget: String },
    /// The anonymous budget is spent — the refusal names the bound.
    AnonymousBudgetSpent { budget: String, bound: u64 },
    /// One session drew its own cap against the anonymous budget.
    AnonymousSessionCapped { budget: String, cap: u64 },
    /// The membership read underneath `active_for` refused — a removed
    /// member is a different answer from a missing one.
    Membership(crate::workspaces::Refusal),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SignInDenied => write!(f, "sign-in denied"),
            Self::Unavailable => {
                write!(f, "no randomness was available to mint the token")
            }
            Self::MalformedDigest(field) => write!(
                f,
                "{field} must be 64 hex characters — a SHA-256 digest, never the secret"
            ),
            Self::UnknownUser(user) => write!(
                f,
                "user `{user}` holds no credential — there is nothing to recover"
            ),
            Self::UnknownSession(id) => {
                write!(f, "session `{id}` is not one this book holds")
            }
            Self::SessionClosed { state } => {
                write!(f, "the session is {state} — it answers nothing")
            }
            Self::AnonymousSession => {
                write!(f, "an anonymous session holds no membership")
            }
            Self::Store(trouble) => write!(f, "{trouble}"),
            Self::UnknownRecovery => {
                write!(f, "the digest names no recovery this book holds")
            }
            Self::RecoveryExpired => write!(f, "the recovery token has expired"),
            Self::RecoveryClosed { state } => write!(
                f,
                "the recovery token is already {state} — a recovery token is single-use"
            ),
            Self::DuplicateBudget { budget } => {
                write!(f, "an anonymous budget named `{budget}` already exists")
            }
            Self::UnknownBudget { budget } => {
                write!(f, "`{budget}` names no anonymous budget this book holds")
            }
            Self::AnonymousBudgetExpired { budget } => {
                write!(f, "the anonymous budget `{budget}` has expired")
            }
            Self::AnonymousBudgetSpent { budget, bound } => write!(
                f,
                "the anonymous budget `{budget}` is spent — its {bound}-request \
                 bound is exhausted"
            ),
            Self::AnonymousSessionCapped { budget, cap } => write!(
                f,
                "the session has drawn its {cap}-request cap against the \
                 anonymous budget `{budget}`"
            ),
            Self::Membership(refusal) => write!(f, "{refusal}"),
        }
    }
}

impl std::error::Error for Refusal {}

impl From<crate::workspaces::Refusal> for Refusal {
    fn from(refusal: crate::workspaces::Refusal) -> Self {
        Self::Membership(refusal)
    }
}

/// The lifecycle book: credentials, the sessions they minted, recovery
/// tokens, and the funded anonymous budgets — one record the caller
/// persists.
///
/// The collections are the whole record: ended sessions, answered
/// recoveries, and spent budgets stay in their maps, because the maps
/// are the history as well as the state.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SessionBook {
    /// How many seconds a session stands from issue — the book's
    /// stated bound, applied to every `sign_in`.
    #[serde(default)]
    pub session_ttl: u64,
    /// How many seconds a recovery token stands from issue.
    #[serde(default)]
    pub recovery_ttl: u64,
    /// User id to credential — digests only.
    #[serde(default)]
    pub credentials: BTreeMap<UserId, Credential>,
    /// Session id to session — ended ones included.
    #[serde(default)]
    pub sessions: BTreeMap<SessionId, Session>,
    /// Token digest to recovery — answered ones included.
    #[serde(default)]
    pub recoveries: BTreeMap<String, Recovery>,
    /// Budget id to anonymous funding — spent ones included.
    #[serde(default)]
    pub onboarding: BTreeMap<String, Onboarding>,
}

impl SessionBook {
    /// A book with the stated lifetimes. Every `sign_in` issues a
    /// session bounded by `session_ttl` seconds, every `issue_recovery`
    /// a token bounded by `recovery_ttl`.
    #[must_use]
    pub fn new(session_ttl: u64, recovery_ttl: u64) -> Self {
        Self {
            session_ttl,
            recovery_ttl,
            ..Self::default()
        }
    }

    /// Set — or replace — the credential a user's sign-in proof
    /// verifies against.
    ///
    /// Replacing a credential ends the sessions the old one signed in:
    /// a session claims someone presented what was then the secret,
    /// and a new secret makes the old claims stale.
    pub fn set_credential(
        &mut self,
        user: UserId,
        digest: String,
        now: u64,
    ) -> Result<Credential, Refusal> {
        if !is_digest(&digest) {
            return Err(Refusal::MalformedDigest("credential-digest"));
        }
        let credential = Credential {
            user: user.clone(),
            digest,
            set_at: now,
        };
        if self
            .credentials
            .insert(user.clone(), credential.clone())
            .is_some()
        {
            self.revoke_all(&user, now);
        }
        Ok(credential)
    }

    /// Sign a user in: verify `secret_proof` against the stored
    /// credential digest and issue a bounded session.
    ///
    /// A user with no credential and a proof that does not verify get
    /// the same answer — the refusal never says which half failed. A
    /// successful issue returns the record and, separately in `once`,
    /// the bearer token; the book keeps only its digest.
    pub fn sign_in(
        &mut self,
        user: &UserId,
        secret_proof: &str,
        now: u64,
    ) -> Result<Issued, Refusal> {
        let credential = self.credentials.get(user).ok_or(Refusal::SignInDenied)?;
        if credential.digest != digest_secret(secret_proof) {
            return Err(Refusal::SignInDenied);
        }
        self.issue_kind(user.clone(), SessionKind::User, now)
    }

    /// Issue a session for a user the caller already authenticated.
    ///
    /// `sign_in` is the book's own proof check; an adapter that
    /// authenticated the user another way — an `oak_` key resolved to
    /// its account, a consumed recovery token — mints through this
    /// call so the proof stays the adapter's business and the claim
    /// stays the book's.
    pub fn issue(&mut self, user: UserId, now: u64) -> Result<Issued, Refusal> {
        self.issue_kind(user, SessionKind::User, now)
    }

    /// Issue an anonymous session — the funded public lane's claim.
    ///
    /// The session names no account and answers no membership; its only
    /// authority is drawing against an [`Onboarding`] budget under its
    /// own session cap.
    pub fn issue_anonymous(&mut self, now: u64) -> Result<Issued, Refusal> {
        self.issue_kind(UserId::from("anonymous"), SessionKind::Anonymous, now)
    }

    /// The mint every issue call shares: a fresh bearer token whose
    /// digest is the record's id, bounded by `session_ttl`.
    fn issue_kind(&mut self, user: UserId, kind: SessionKind, now: u64) -> Result<Issued, Refusal> {
        let token = format!("{SESSION_PREFIX}_{}", fresh()?);
        let session = Session {
            id: SessionId(digest_secret(&token)),
            user,
            kind,
            created_at: now,
            expires_at: now + self.session_ttl,
            state: SessionState::Active,
            closed_at: None,
        };
        self.sessions.insert(session.id.clone(), session.clone());
        Ok(Issued {
            session,
            once: token,
        })
    }

    /// The session record an id names, in whatever state it stands.
    #[must_use]
    pub fn session(&self, id: &SessionId) -> Option<&Session> {
        self.sessions.get(id)
    }

    /// Resolve a presented session token to its record — the digest is
    /// the lookup, so the token itself is never stored.
    #[must_use]
    pub fn session_of_token(&self, token: &str) -> Option<&Session> {
        self.sessions.get(&SessionId(digest_secret(token)))
    }

    /// Resolve a presented recovery token to its record — the digest
    /// is the lookup, the same rule the session records keep.
    #[must_use]
    pub fn recovery_of_token(&self, token: &str) -> Option<&Recovery> {
        self.recoveries.get(&digest_secret(token))
    }

    /// The whole `active_for` check for a presented token: resolve the
    /// session, then require it to stand active and the user to hold
    /// an active membership in `workspace`.
    pub fn active_for<'w>(
        &self,
        token: &str,
        workspace: &'w Workspace,
        now: u64,
    ) -> Result<&'w Membership, Refusal> {
        self.session_of_token(token)
            .ok_or_else(|| Refusal::UnknownSession(SessionId(digest_secret(token))))?
            .active_for(workspace, now)
    }

    /// Mark every session whose deadline passed `expired` — the sweep
    /// that writes the state a `standing` read already derives.
    /// Returns the records it closed.
    pub fn expire(&mut self, now: u64) -> Vec<Session> {
        let mut expired = Vec::new();
        for session in self.sessions.values_mut() {
            if session.state == SessionState::Active && session.expires_at <= now {
                session.state = SessionState::Expired;
                session.closed_at = Some(now);
                expired.push(session.clone());
            }
        }
        expired
    }

    /// End one session — a logout is a revocation of one. A session
    /// that already ended answers its state: `expired` and `revoked`
    /// are distinct, never both "invalid".
    pub fn logout(&mut self, id: &SessionId, now: u64) -> Result<Session, Refusal> {
        let session = self
            .sessions
            .get_mut(id)
            .ok_or_else(|| Refusal::UnknownSession(id.clone()))?;
        match session.standing(now) {
            SessionState::Active => {
                session.state = SessionState::Revoked;
                session.closed_at = Some(now);
                Ok(session.clone())
            }
            state => Err(Refusal::SessionClosed { state }),
        }
    }

    /// End every session a user holds — the set removal a membership
    /// removal, a credential reset, or an operator's revocation calls
    /// for. Returns the records it closed; sessions already ended keep
    /// the answer they had.
    pub fn revoke_all(&mut self, user: &UserId, now: u64) -> Vec<Session> {
        let mut ended = Vec::new();
        for session in self.sessions.values_mut() {
            if session.user == *user && session.standing(now) == SessionState::Active {
                session.state = SessionState::Revoked;
                session.closed_at = Some(now);
                ended.push(session.clone());
            }
        }
        ended
    }

    /// Issue a recovery token for a user who holds a credential.
    ///
    /// Issuing retires the user's unconsumed predecessor — at most one
    /// recovery token per user is live, so the older one cannot be
    /// replayed after the newer exists. Returns the record and, in
    /// `once`, the token; the book keeps only its digest.
    pub fn issue_recovery(&mut self, user: &UserId, now: u64) -> Result<RecoveryIssued, Refusal> {
        if !self.credentials.contains_key(user) {
            return Err(Refusal::UnknownUser(user.clone()));
        }
        let token = format!("{RECOVERY_PREFIX}_{}", fresh()?);
        let token_digest = digest_secret(&token);
        for recovery in self.recoveries.values_mut() {
            if recovery.user == *user && recovery.state == RecoveryState::Pending {
                recovery.state = RecoveryState::Superseded;
                recovery.answered_at = Some(now);
            }
        }
        let recovery = Recovery {
            token_digest: token_digest.clone(),
            user: user.clone(),
            state: RecoveryState::Pending,
            issued_at: now,
            expires_at: now + self.recovery_ttl,
            answered_at: None,
        };
        self.recoveries.insert(token_digest, recovery.clone());
        Ok(RecoveryIssued {
            recovery,
            once: token,
        })
    }

    /// Use a recovery token: consume it and set the user's credential
    /// to `new_digest`.
    ///
    /// A consumed token refuses replay, a superseded one was retired by
    /// a newer issue, and an expired one answers `RecoveryExpired` —
    /// three distinct refusals. A successful recovery ends every
    /// session the user holds, because a credential reset must not
    /// leave sessions minted under the old secret running.
    pub fn use_recovery(
        &mut self,
        token: &str,
        new_digest: String,
        now: u64,
    ) -> Result<Credential, Refusal> {
        if !is_digest(&new_digest) {
            return Err(Refusal::MalformedDigest("credential-digest"));
        }
        let user = {
            let recovery = self
                .recoveries
                .get_mut(&digest_secret(token))
                .ok_or(Refusal::UnknownRecovery)?;
            match recovery.state {
                RecoveryState::Pending => {}
                state => return Err(Refusal::RecoveryClosed { state }),
            }
            if recovery.expires_at <= now {
                return Err(Refusal::RecoveryExpired);
            }
            recovery.state = RecoveryState::Consumed;
            recovery.answered_at = Some(now);
            recovery.user.clone()
        };
        self.set_credential(user, new_digest, now)
    }

    /// Consume a recovery token and answer the user it was issued for —
    /// the redemption half an adapter runs when the replacement
    /// credential is its own operation: consume the token, replace the
    /// credential in its own store, then call `set_credential` so the
    /// sessions the old proof minted end with it.
    ///
    /// The same refusals `use_recovery` answers apply: a consumed or
    /// superseded token is a replay, an expired one names its deadline.
    pub fn redeem_recovery(&mut self, token: &str, now: u64) -> Result<UserId, Refusal> {
        let recovery = self
            .recoveries
            .get_mut(&digest_secret(token))
            .ok_or(Refusal::UnknownRecovery)?;
        match recovery.state {
            RecoveryState::Pending => {}
            state => return Err(Refusal::RecoveryClosed { state }),
        }
        if recovery.expires_at <= now {
            return Err(Refusal::RecoveryExpired);
        }
        recovery.state = RecoveryState::Consumed;
        recovery.answered_at = Some(now);
        Ok(recovery.user.clone())
    }

    /// Record an operator-funded anonymous budget. The budget is its
    /// own typed record — a stated bound and abuse counters, never an
    /// implicit free-for-all.
    pub fn fund_anonymous(&mut self, budget: Onboarding) -> Result<Onboarding, Refusal> {
        if self.onboarding.contains_key(&budget.id) {
            return Err(Refusal::DuplicateBudget { budget: budget.id });
        }
        self.onboarding.insert(budget.id.clone(), budget.clone());
        Ok(budget)
    }

    /// Draw one anonymous request against a funded budget on
    /// `session`'s counter — the spend that spends down the stated
    /// bound and refuses by name when it is gone.
    pub fn spend_anonymous(
        &mut self,
        budget: &str,
        session: &str,
        now: u64,
    ) -> Result<(), Refusal> {
        let budget = self
            .onboarding
            .get_mut(budget)
            .ok_or_else(|| Refusal::UnknownBudget {
                budget: budget.to_string(),
            })?;
        budget.spend(session, now)
    }
}

/// An access event: who acted, what they did, and where — the history a
/// workspace keeps of the calls its members made.
///
/// The record carries references only: the actor's account id, the
/// session's digest id, the workspace id. No bearer token, key secret,
/// or invitation secret ever appears here — the detail field is the
/// adapter's and holds only what the adapter chose to name.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Access {
    /// When the call ran, as Unix seconds.
    pub at: u64,
    /// The acting account id, or `anonymous` for the funded public
    /// lane.
    pub actor: String,
    /// What the call did — `sign-in`, `invite`, `key-rotate`, and the
    /// rest of the adapter's vocabulary.
    pub action: String,
    /// The workspace the call acted on, when it acted on one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// The session the call ran under, as its digest id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// What the call touched, in the caller's own words — an invitation
    /// id, a key id, a role. Never a secret.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// The most access events the live store keeps. Older events stay
/// readable in the archived revisions — the bound keeps the working
/// document small, not the history short.
const ACCESS_MAX: usize = 8192;

/// The file the session store lives in, beside `accounts.json`.
const SESSIONS: &str = "sessions.json";

/// The directory past revisions are archived in, one file per digest.
const HISTORY_DIR: &str = "sessions-history";

/// The lock serializing writers, held only for a mutation's duration.
const SESSIONS_LOCK: &str = "sessions.lock";

/// The schema tag the store carries.
pub const SESSIONS_SCHEMA: &str = "openagents.tenancy.sessions.v1";

/// How many times a writer retries the lock before reporting it held.
const SESSIONS_LOCK_RETRIES: u32 = 200;

/// The largest store document the loader accepts.
const SESSIONS_STORE_BYTES: u64 = 16 * 1024 * 1024;

/// The persisted session store: the book plus the access history, under
/// the same discipline `accounts.json` keeps.
///
/// `sessions.json` sits beside `accounts.json` and follows the same
/// rules: a schema tag, a sequence, a `supersedes` chain, and a SHA-256
/// digest over every field but itself. Every sealed revision is
/// archived under `sessions-history/<digest>.json`, and writers
/// serialize on `sessions.lock`, re-reading inside it so the second of
/// two competing writers decides against committed state.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Store {
    /// The schema tag.
    pub v: String,
    /// The revision number. Genesis is 0; each committed mutation adds
    /// one.
    pub sequence: u64,
    /// The digest of the revision this one replaced, when there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    /// The lifecycle book — credentials, sessions, recoveries, and the
    /// funded anonymous budgets.
    pub book: SessionBook,
    /// The access history, oldest first, bounded at [`ACCESS_MAX`].
    #[serde(default)]
    pub access: Vec<Access>,
    /// The digest over every field above.
    pub digest: String,
}

impl Store {
    /// Fill in `digest` over the store's other fields.
    fn seal(&mut self) {
        self.digest = self.compute_digest();
    }

    /// The digest over every field but `digest`.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a store serializes");
        value
            .as_object_mut()
            .expect("a store is an object")
            .remove("digest");
        let mut hasher = Sha256::new();
        hasher.update(canonicalize(&value).as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }

    /// Read and validate a store's text.
    fn parse(text: &str, name: &str) -> Result<Self, String> {
        let store: Self = serde_json::from_str(text).map_err(|error| format!("{name}: {error}"))?;
        store.validate(name)?;
        Ok(store)
    }

    /// The checks a store must pass before anything reads it: a known
    /// schema, a digest that recomputes, session and recovery ids
    /// shaped like the digests they claim to be, and a bounded access
    /// log.
    pub fn validate(&self, name: &str) -> Result<(), String> {
        if self.v != SESSIONS_SCHEMA {
            return Err(format!(
                "{name}: schema `{}` is not `{SESSIONS_SCHEMA}`",
                self.v
            ));
        }
        if self.digest != self.compute_digest() {
            return Err(format!(
                "{name}: the store's digest does not recompute over its contents"
            ));
        }
        for (id, session) in &self.book.sessions {
            if *id != session.id {
                return Err(format!(
                    "{name}: session `{id}` is filed under the wrong id"
                ));
            }
            if !is_digest(session.id.as_str()) {
                return Err(format!(
                    "{name}: session `{id}` carries an id that is not 64 hex characters"
                ));
            }
        }
        for digest in self.book.recoveries.keys() {
            if !is_digest(digest) {
                return Err(format!(
                    "{name}: a recovery is filed under a digest that is not 64 hex characters"
                ));
            }
        }
        if self.access.len() > ACCESS_MAX {
            return Err(format!(
                "{name}: the access log exceeds {ACCESS_MAX} events"
            ));
        }
        Ok(())
    }
}

/// The session store handle: a directory the calls read and write.
///
/// Like [`Accounts`], the handle holds no cache — every query re-reads
/// and re-validates `sessions.json`, so a revocation committed by any
/// writer is visible to the very next read on any handle. Every
/// mutation takes the lock, re-reads inside it, applies, re-seals, and
/// writes.
#[derive(Clone, Debug)]
pub struct Sessions {
    dir: PathBuf,
}

impl Sessions {
    /// Create the store's genesis revision in a directory.
    ///
    /// `book` sets the lifetimes the store opens with — the config's
    /// session and recovery TTLs — and the anonymous funding is a
    /// mutation like any other, recorded after genesis. The directory
    /// must not already hold a session store.
    pub fn install(dir: &Path, book: SessionBook) -> Result<Self, crate::accounts::Trouble> {
        std::fs::create_dir_all(dir)?;
        let _lock = SessionLock::acquire(dir)?;
        if dir.join(SESSIONS).exists()
            || (dir.join(HISTORY_DIR).exists()
                && std::fs::read_dir(dir.join(HISTORY_DIR))?.next().is_some())
        {
            return Err(crate::accounts::Trouble::Invalid(format!(
                "{} already holds a session store; open it rather than reinstalling",
                dir.display()
            )));
        }
        let mut store = Store {
            v: SESSIONS_SCHEMA.to_string(),
            sequence: 0,
            supersedes: None,
            book,
            access: Vec::new(),
            digest: String::new(),
        };
        store.seal();
        store
            .validate(&dir.join(SESSIONS).display().to_string())
            .map_err(crate::accounts::Trouble::Invalid)?;
        save_sessions(dir, &store)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// Open the store in a directory, validating it end to end. A
    /// missing or corrupt store is refused, not guessed at.
    pub fn open(dir: &Path) -> Result<Self, crate::accounts::Trouble> {
        load_sessions(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// Read the current store. This is the fresh read every query makes.
    pub fn store(&self) -> Result<Store, crate::accounts::Trouble> {
        load_sessions(&self.dir)
    }

    /// Read an archived revision by digest — the lookup that explains
    /// which sessions stood when an earlier call ran.
    pub fn revision(dir: &Path, digest: &str) -> Result<Store, crate::accounts::Trouble> {
        if !digest
            .strip_prefix("sha256:")
            .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(crate::accounts::Trouble::Invalid(
                "revision must be a SHA-256 identity".into(),
            ));
        }
        let path = dir.join(HISTORY_DIR).join(format!("{digest}.json"));
        if !path.exists() {
            return Err(crate::accounts::Trouble::UnknownRevision(
                digest.to_string(),
            ));
        }
        let text = read_sessions(&path)?;
        let store = Store::parse(&text, &path.display().to_string())
            .map_err(crate::accounts::Trouble::Invalid)?;
        if store.digest != digest {
            return Err(crate::accounts::Trouble::Invalid(
                "archived revision identity mismatch".into(),
            ));
        }
        Ok(store)
    }

    /// One mutation: take the lock, re-read inside it, run `f` against
    /// the book, seal the new revision, and write it in one rename.
    ///
    /// The sweep comes first — an expired session is marked `expired`
    /// in the same revision the mutation commits, so the written state
    /// matches what `standing` already derives.
    pub fn mutate<T>(
        &self,
        f: impl FnOnce(&mut SessionBook, &mut Vec<Access>, u64) -> Result<T, Refusal>,
    ) -> Result<T, Refusal> {
        let _lock = SessionLock::acquire(&self.dir).map_err(|t| Refusal::Store(t.to_string()))?;
        let mut store = load_sessions(&self.dir).map_err(|t| Refusal::Store(t.to_string()))?;
        let now = unix_now();
        store.book.expire(now);
        let supersedes = store.digest.clone();
        let out = f(&mut store.book, &mut store.access, now)?;
        store.sequence += 1;
        store.supersedes = Some(supersedes);
        store.seal();
        store
            .validate(&self.dir.join(SESSIONS).display().to_string())
            .map_err(Refusal::Store)?;
        save_sessions(&self.dir, &store).map_err(|t| Refusal::Store(t.to_string()))?;
        Ok(out)
    }

    /// Append an access event inside a mutation that does nothing else —
    /// the audit trail's own write.
    pub fn record(
        &self,
        actor: &str,
        action: &str,
        workspace: Option<&str>,
        session: Option<&str>,
        detail: Option<String>,
    ) -> Result<(), Refusal> {
        self.mutate(|_, access, now| {
            push_access(
                access,
                Access {
                    at: now,
                    actor: actor.to_string(),
                    action: action.to_string(),
                    workspace: workspace.map(str::to_string),
                    session: session.map(str::to_string),
                    detail,
                },
            );
            Ok(())
        })
    }
}

/// Append an event, pruning the oldest when the log is at its bound —
/// the one way events join the log, so a writer inside `mutate` cannot
/// grow it past [`ACCESS_MAX`].
pub fn push_access(access: &mut Vec<Access>, event: Access) {
    access.push(event);
    if access.len() > ACCESS_MAX {
        let overflow = access.len() - ACCESS_MAX;
        access.drain(..overflow);
    }
}

/// The exclusive lock one session mutation holds — the same shape as
/// `accounts.lock`: `create_new` makes it atomic, absence is the
/// release, a dropped guard removes it.
struct SessionLock {
    path: PathBuf,
}

impl SessionLock {
    fn acquire(dir: &Path) -> Result<Self, crate::accounts::Trouble> {
        let path = dir.join(SESSIONS_LOCK);
        for _ in 0..SESSIONS_LOCK_RETRIES {
            match std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
            {
                Ok(mut file) => {
                    writeln!(file, "pid {}", std::process::id()).ok();
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => return Err(crate::accounts::Trouble::Io(error)),
            }
        }
        Err(crate::accounts::Trouble::Locked(path.display().to_string()))
    }
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
    }
}

/// The current time as Unix seconds — the clock sessions expire and
/// access events record against.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default()
}

fn read_sessions(path: &Path) -> Result<String, crate::accounts::Trouble> {
    let mut text = String::new();
    std::fs::File::open(path)?
        .take(SESSIONS_STORE_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > SESSIONS_STORE_BYTES {
        return Err(crate::accounts::Trouble::Invalid(
            "session store exceeds 16 MiB".into(),
        ));
    }
    Ok(text)
}

fn write_sessions_synced(path: &Path, text: &str) -> Result<(), crate::accounts::Trouble> {
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

/// Load the store from a directory, validating it end to end.
fn load_sessions(dir: &Path) -> Result<Store, crate::accounts::Trouble> {
    let path = dir.join(SESSIONS);
    let text = read_sessions(&path)?;
    Store::parse(&text, &path.display().to_string()).map_err(crate::accounts::Trouble::Invalid)
}

/// Write a sealed store: archive it by digest, then replace
/// `sessions.json` in one rename.
fn save_sessions(dir: &Path, store: &Store) -> Result<(), crate::accounts::Trouble> {
    let history = dir.join(HISTORY_DIR);
    std::fs::create_dir_all(&history)?;
    let text = serde_json::to_string_pretty(store)
        .map_err(|error| crate::accounts::Trouble::Invalid(error.to_string()))?;
    let archived = history.join(format!("{}.json", store.digest));
    if text.len() as u64 + 1 > SESSIONS_STORE_BYTES {
        return Err(crate::accounts::Trouble::Invalid(
            "session store exceeds 16 MiB".into(),
        ));
    }
    if !archived.exists() {
        write_sessions_synced(&archived, &format!("{text}\n"))?;
    } else if read_sessions(&archived)? != format!("{text}\n") {
        return Err(crate::accounts::Trouble::Invalid(
            "archived revision content mismatch".into(),
        ));
    }
    std::fs::File::open(&history)?.sync_all()?;
    let staged = dir.join(format!(".{SESSIONS}.{}.tmp", fresh_id()?));
    write_sessions_synced(&staged, &format!("{text}\n"))?;
    std::fs::rename(&staged, dir.join(SESSIONS))?;
    std::fs::File::open(dir)?.sync_all()?;
    Ok(())
}

/// Fresh random material for staging file names — the store's own
/// `fresh` shaped for the I/O path's error type.
fn fresh_id() -> Result<String, crate::accounts::Trouble> {
    fresh().map_err(|error| crate::accounts::Trouble::Invalid(error.to_string()))
}

/// Canonical JSON: keys sorted, whitespace gone — the same
/// canonicalization `accounts.json` digests under.
fn canonicalize(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = String::from("{");
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).expect("a key serializes"));
                out.push(':');
                out.push_str(&canonicalize(&map[*key]));
            }
            out.push('}');
            out
        }
        serde_json::Value::Array(items) => {
            let mut out = String::from("[");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&canonicalize(item));
            }
            out.push(']');
            out
        }
        other => serde_json::to_string(other).expect("a value serializes"),
    }
}

/// Whether a value is shaped like a SHA-256 digest: 64 hex characters.
fn is_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The digest the book keeps of a secret.
fn digest_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Fresh random material for bearer tokens — 32 bytes of hex.
fn fresh() -> Result<String, Refusal> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| Refusal::Unavailable)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspaces::{BillingAccountId, MemberState, Role};

    /// A 64-hex stand-in for a SHA-256 digest.
    fn digest(byte: u8) -> String {
        format!("{byte:064x}")
    }

    fn user(name: &str) -> UserId {
        UserId::from(name)
    }

    /// A book whose sessions stand 1,000 seconds and whose recovery
    /// tokens stand 500.
    fn book() -> SessionBook {
        SessionBook::new(1_000, 500)
    }

    /// An organization workspace with `member` joined and active.
    fn workspace_with(member: &UserId) -> Workspace {
        let owner = user("u_owner");
        let mut workspace = Workspace::organization(
            WorkspaceId::from("ws_eng"),
            "engineering".to_string(),
            BillingAccountId::from("bill_acme"),
            owner.clone(),
            None,
            1_000,
        );
        workspace
            .invite(&owner, member, Role::Member, digest(0x11), 9_999, 2_000)
            .unwrap();
        workspace.accept(&digest(0x11), 2_100).unwrap();
        workspace
    }

    #[test]
    fn a_wrong_proof_refuses_without_saying_which_half_failed() {
        let mut book = book();
        let member = user("u_member");
        book.set_credential(member.clone(), digest_secret("hunter2"), 1_000)
            .unwrap();
        // No credential at all and a proof that does not verify are the
        // same answer — the refusal never says which half failed.
        assert_eq!(
            book.sign_in(&user("u_nobody"), "hunter2", 2_000)
                .unwrap_err(),
            Refusal::SignInDenied
        );
        assert_eq!(
            book.sign_in(&member, "not-the-secret", 2_000).unwrap_err(),
            Refusal::SignInDenied
        );
        // The right proof issues — and the record is digest-only.
        let issued = book.sign_in(&member, "hunter2", 2_000).unwrap();
        assert_eq!(issued.session.user, member);
        assert_eq!(issued.session.id.as_str(), digest_secret(&issued.once));
    }

    #[test]
    fn expiry_is_a_comparison_against_the_supplied_now() {
        let mut book = book();
        let member = user("u_member");
        book.set_credential(member.clone(), digest_secret("pw"), 1_000)
            .unwrap();
        let session = book.sign_in(&member, "pw", 2_000).unwrap().session;
        assert_eq!(session.expires_at, 3_000);
        assert_eq!(session.standing(2_999), SessionState::Active);
        // The deadline is the comparison — at it, the claim is over.
        assert_eq!(session.standing(3_000), SessionState::Expired);
        // The sweep writes the state the standing read already derived.
        let expired = book.expire(3_000);
        assert_eq!(expired.len(), 1);
        assert_eq!(
            book.session(&session.id).unwrap().state,
            SessionState::Expired
        );
        // A second sweep finds nothing left to mark.
        assert!(book.expire(3_001).is_empty());
    }

    #[test]
    fn logout_ends_one_session_revocation_ends_the_set() {
        let mut book = book();
        let member = user("u_member");
        let other = user("u_other");
        for who in [&member, &other] {
            book.set_credential(who.clone(), digest_secret("pw"), 1_000)
                .unwrap();
        }
        let first = book.sign_in(&member, "pw", 2_000).unwrap().session;
        let second = book.sign_in(&member, "pw", 2_000).unwrap().session;
        let foreign = book.sign_in(&other, "pw", 2_000).unwrap().session;
        // A logout is a revocation of one.
        book.logout(&first.id, 2_100).unwrap();
        assert_eq!(
            book.session(&first.id).unwrap().state,
            SessionState::Revoked
        );
        // Revocation ends the user's whole set — and only theirs.
        let ended = book.revoke_all(&member, 2_200);
        assert_eq!(ended.len(), 1);
        assert_eq!(ended[0].id, second.id);
        assert_eq!(
            book.session(&second.id).unwrap().state,
            SessionState::Revoked
        );
        assert_eq!(
            book.session(&foreign.id).unwrap().state,
            SessionState::Active
        );
        // Ended is ended — a second logout names the state it found.
        assert_eq!(
            book.logout(&first.id, 2_300).unwrap_err(),
            Refusal::SessionClosed {
                state: SessionState::Revoked
            }
        );
        // An expired session answers `expired`, not `revoked` —
        // distinguishable, never both "invalid".
        book.expire(3_000);
        assert_eq!(
            book.logout(&foreign.id, 3_100).unwrap_err(),
            Refusal::SessionClosed {
                state: SessionState::Expired
            }
        );
        assert!(book.revoke_all(&other, 3_100).is_empty());
    }

    #[test]
    fn recovery_is_single_use_and_a_new_issue_retires_the_old() {
        let mut book = book();
        let member = user("u_member");
        book.set_credential(member.clone(), digest_secret("old"), 1_000)
            .unwrap();
        let session = book.sign_in(&member, "old", 2_000).unwrap().session;
        let first = book.issue_recovery(&member, 2_100).unwrap();
        // A new issue retires the unconsumed predecessor.
        let second = book.issue_recovery(&member, 2_200).unwrap();
        assert_eq!(
            book.recoveries[&first.recovery.token_digest].state,
            RecoveryState::Superseded
        );
        assert_eq!(
            book.use_recovery(&first.once, digest(0x22), 2_300)
                .unwrap_err(),
            Refusal::RecoveryClosed {
                state: RecoveryState::Superseded
            }
        );
        // Use consumes: the credential resets and the session minted
        // under the old secret dies with it.
        book.use_recovery(&second.once, digest_secret("new"), 2_400)
            .unwrap();
        assert_eq!(book.credentials[&member].digest, digest_secret("new"));
        assert_eq!(
            book.session(&session.id).unwrap().state,
            SessionState::Revoked
        );
        // A second use is a replay, not a recovery.
        assert_eq!(
            book.use_recovery(&second.once, digest(0x44), 2_500)
                .unwrap_err(),
            Refusal::RecoveryClosed {
                state: RecoveryState::Consumed
            }
        );
        // A token past its deadline refuses as expired.
        let late = book.issue_recovery(&member, 2_500).unwrap();
        assert_eq!(
            book.use_recovery(&late.once, digest(0x55), 3_100)
                .unwrap_err(),
            Refusal::RecoveryExpired
        );
        // The old proof denies now; the new credential signs in.
        assert_eq!(
            book.sign_in(&member, "old", 3_200).unwrap_err(),
            Refusal::SignInDenied
        );
        book.sign_in(&member, "new", 3_200).unwrap();
    }

    #[test]
    fn the_anonymous_budget_spends_down_and_refuses_by_name() {
        let mut book = book();
        book.fund_anonymous(Onboarding {
            id: "onb_trial".to_string(),
            workspace: WorkspaceId::from("ws_eng"),
            funded_by: "op_alice".to_string(),
            bound: 3,
            spent: 0,
            session_cap: 2,
            sessions: BTreeMap::new(),
            funded_at: 1_000,
            expires_at: 9_999,
        })
        .unwrap();
        // One session may draw up to its cap — no further.
        book.spend_anonymous("onb_trial", "sess_a", 2_000).unwrap();
        book.spend_anonymous("onb_trial", "sess_a", 2_001).unwrap();
        assert_eq!(
            book.spend_anonymous("onb_trial", "sess_a", 2_002)
                .unwrap_err(),
            Refusal::AnonymousSessionCapped {
                budget: "onb_trial".to_string(),
                cap: 2
            }
        );
        // Another session draws what remains — then the bound itself
        // refuses, naming the budget.
        book.spend_anonymous("onb_trial", "sess_b", 2_003).unwrap();
        assert_eq!(
            book.spend_anonymous("onb_trial", "sess_c", 2_004)
                .unwrap_err(),
            Refusal::AnonymousBudgetSpent {
                budget: "onb_trial".to_string(),
                bound: 3
            }
        );
        assert_eq!(book.onboarding["onb_trial"].remaining(), 0);
        // Funding past its window answers `expired`, not `spent`.
        book.fund_anonymous(Onboarding {
            id: "onb_late".to_string(),
            workspace: WorkspaceId::from("ws_eng"),
            funded_by: "op_alice".to_string(),
            bound: 5,
            spent: 0,
            session_cap: 5,
            sessions: BTreeMap::new(),
            funded_at: 1_000,
            expires_at: 2_000,
        })
        .unwrap();
        assert_eq!(
            book.spend_anonymous("onb_late", "sess_a", 2_500)
                .unwrap_err(),
            Refusal::AnonymousBudgetExpired {
                budget: "onb_late".to_string()
            }
        );
    }

    #[test]
    fn a_removed_members_session_fails_active_for_even_unexpired() {
        let mut book = book();
        let member = user("u_member");
        let mut workspace = workspace_with(&member);
        book.set_credential(member.clone(), digest_secret("pw"), 1_000)
            .unwrap();
        let issued = book.sign_in(&member, "pw", 2_000).unwrap();
        // While the member belongs, the session answers.
        assert!(issued.session.active_for(&workspace, 2_500).is_ok());
        assert!(book.active_for(&issued.once, &workspace, 2_500).is_ok());
        // Removing the member ends the session's reach — the record
        // still stands active, and still fails the check.
        workspace.revoke(&member, 2_600).unwrap();
        assert_eq!(issued.session.standing(2_700), SessionState::Active);
        assert!(matches!(
            issued.session.active_for(&workspace, 2_700),
            Err(Refusal::Membership(crate::workspaces::Refusal::NotActive {
                state: MemberState::Removed,
                ..
            }))
        ));
        assert!(matches!(
            book.active_for(&issued.once, &workspace, 2_700),
            Err(Refusal::Membership(crate::workspaces::Refusal::NotActive {
                state: MemberState::Removed,
                ..
            }))
        ));
    }

    #[test]
    fn records_hold_digests_never_tokens() {
        let mut book = book();
        let member = user("u_member");
        book.set_credential(member.clone(), digest_secret("hunter2"), 1_000)
            .unwrap();
        let issued = book.sign_in(&member, "hunter2", 2_000).unwrap();
        let recovery = book.issue_recovery(&member, 2_100).unwrap();
        let text = serde_json::to_string(&book).unwrap();
        // Neither the session token, the recovery token, nor the proof
        // itself appears anywhere the store serializes.
        for secret in [&issued.once, &recovery.once, "hunter2"] {
            assert!(!text.contains(secret), "{text}");
        }
        // What the store holds instead is each token's digest.
        assert_eq!(issued.session.id.as_str(), digest_secret(&issued.once));
        assert_eq!(
            recovery.recovery.token_digest,
            digest_secret(&recovery.once)
        );
        assert!(text.contains(issued.session.id.as_str()));
        // The presentation is redacted out of band too — a debug dump
        // never leaks the token.
        assert!(!format!("{issued:?}").contains(&issued.once));
    }

    #[test]
    fn replacing_a_credential_ends_the_sessions_it_signed_in() {
        let mut book = book();
        let member = user("u_member");
        book.set_credential(member.clone(), digest_secret("first"), 1_000)
            .unwrap();
        let session = book.sign_in(&member, "first", 2_000).unwrap().session;
        book.set_credential(member.clone(), digest_secret("second"), 2_500)
            .unwrap();
        assert_eq!(
            book.session(&session.id).unwrap().state,
            SessionState::Revoked
        );
        // The old proof denies; the new one issues.
        assert_eq!(
            book.sign_in(&member, "first", 2_600).unwrap_err(),
            Refusal::SignInDenied
        );
        book.sign_in(&member, "second", 2_600).unwrap();
    }
}
