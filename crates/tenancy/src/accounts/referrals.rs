//! Stable introduction identities and inert, consented acquisition sources.

use super::*;

pub mod attribution;
pub mod commission;

pub const SCHEMA: &str = "openagents.referral.source.v1";
pub const CONSENT: &str = "openagents.referral.consent.v1";
const LIMIT: usize = 4096;
const LINKS: usize = 32;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Person,
    Agent,
    Author,
    Partner,
}

/// A commercial source identity, independent of credentials and payees.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Referrer {
    pub schema: String,
    pub id: String,
    pub version: u64,
    pub owner: String,
    pub kind: Kind,
    pub label: String,
    pub source_only: bool,
    pub pending_owner: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub id: String,
    pub version: u64,
    pub kind: Kind,
    pub source_only: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Captured,
    Missing,
    Declined,
    Unknown,
    Disabled,
    Malformed,
}

/// No share token, contact text, payout destination, or commission right.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub schema: String,
    pub account: String,
    pub request: String,
    pub outcome: Outcome,
    pub referrer: Option<Identity>,
    pub consent_version: Option<String>,
    pub captured_at: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    pub request: String,
    pub token: Option<String>,
    pub consent: bool,
    pub consent_version: Option<String>,
}

impl Capture {
    pub fn missing(request: String) -> Self {
        Self {
            request,
            token: None,
            consent: false,
            consent_version: None,
        }
    }
    fn validate(&self) -> Result<(), Error> {
        bounded(&self.request, 128)?;
        if self.token.as_ref().is_some_and(|t| t.len() > 256)
            || self.consent_version.as_ref().is_some_and(|t| t.len() > 64)
            || (self.consent && self.consent_version.as_deref() != Some(CONSENT))
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}

/// The only data in a public share path is random source lookup material.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Issued {
    pub referrer: String,
    pub token: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Link {
    referrer: String,
    version: u64,
    active: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Recorded {
    input: String,
    source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    at_signup: Option<bool>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    #[serde(default)]
    referrers: BTreeMap<String, Referrer>,
    #[serde(default)]
    links: BTreeMap<String, Link>,
    #[serde(default)]
    sources: BTreeMap<String, Recorded>,
    #[serde(default, skip_serializing_if = "attribution::State::is_empty")]
    attribution: attribution::State,
    #[serde(default, skip_serializing_if = "commission::State::is_empty")]
    commissions: commission::State,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Unauthorized,
    Invalid,
    Conflict,
    Bound,
    Unavailable,
    Store(String),
}
impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "referral_forbidden",
            Self::Invalid => "invalid_referral",
            Self::Conflict => "referral_conflict",
            Self::Bound => "referral_bound",
            Self::Unavailable => "referral_unavailable",
            Self::Store(_) => "referral_store_unavailable",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for Error {}
fn storage(error: Trouble) -> Error {
    Error::Store(error.to_string())
}
fn bounded(value: &str, max: usize) -> Result<(), Error> {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        Err(Error::Invalid)
    } else {
        Ok(())
    }
}
fn token(value: &str) -> bool {
    value.strip_prefix("rfr_").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn hash(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

impl Book {
    pub fn is_empty(&self) -> bool {
        self.referrers.is_empty()
            && self.links.is_empty()
            && self.sources.is_empty()
            && self.attribution.is_empty()
            && self.commissions.is_empty()
    }
    pub(super) fn validate(
        &self,
        accounts: &BTreeMap<String, Account>,
        workspaces: &BTreeMap<String, Workspace>,
    ) -> Result<(), String> {
        if self.referrers.len() > LIMIT
            || self.sources.len() > LIMIT
            || self.links.len() > LIMIT * LINKS
        {
            return Err("referral book exceeds its bounds".into());
        }
        for (id, record) in &self.referrers {
            if record.id != *id
                || record.schema != SCHEMA
                || record.version == 0
                || !accounts.contains_key(&record.owner)
                || record
                    .pending_owner
                    .as_ref()
                    .is_some_and(|owner| !accounts.contains_key(owner))
                || bounded(&record.label, 128).is_err()
                || (record.source_only && record.kind != Kind::Agent)
            {
                return Err("invalid referrer identity".into());
            }
        }
        for (digest, link) in &self.links {
            if digest.len() != 64
                || !digest.bytes().all(|b| b.is_ascii_hexdigit())
                || !self
                    .referrers
                    .get(&link.referrer)
                    .is_some_and(|r| link.version > 0 && link.version <= r.version)
            {
                return Err("invalid referral link".into());
            }
        }
        for (account, recorded) in &self.sources {
            let source = &recorded.source;
            if source.schema != SCHEMA
                || source.account != *account
                || !accounts.contains_key(account)
                || bounded(&source.request, 128).is_err()
                || recorded.input.len() != 64
                || (source.outcome == Outcome::Captured) != source.referrer.is_some()
                || (source.outcome == Outcome::Captured
                    && source.consent_version.as_deref() != Some(CONSENT))
            {
                return Err("invalid acquisition source".into());
            }
            if let Some(identity) = &source.referrer {
                if !self.referrers.get(&identity.id).is_some_and(|r| {
                    identity.version > 0
                        && identity.version <= r.version
                        && identity.kind == r.kind
                        && identity.source_only == r.source_only
                }) {
                    return Err("acquisition referrer does not match its stable identity".into());
                }
            }
        }
        self.attribution.validate(self, accounts, workspaces)?;
        self.commissions.validate(self, accounts)
    }
    fn capture(
        &mut self,
        account: &str,
        input: &Capture,
        now: u64,
    ) -> Result<(Source, bool), Error> {
        input.validate()?;
        let input_digest = hash(&serde_json::to_vec(input).map_err(|_| Error::Invalid)?);
        if let Some(prior) = self.sources.get(account) {
            return if prior.input == input_digest {
                Ok((prior.source.clone(), false))
            } else {
                Err(Error::Conflict)
            };
        }
        if self.sources.len() >= LIMIT {
            return Err(Error::Bound);
        }
        let mut identity = None;
        let outcome = match input.token.as_deref() {
            None => Outcome::Missing,
            Some(_) if !input.consent => Outcome::Declined,
            Some(value) if !token(value) => Outcome::Malformed,
            Some(value) => match self.links.get(&hash(value.as_bytes())) {
                None => Outcome::Unknown,
                Some(link) if !link.active => Outcome::Disabled,
                Some(link) => {
                    let r = self
                        .referrers
                        .get(&link.referrer)
                        .ok_or(Error::Unavailable)?;
                    identity = Some(Identity {
                        id: r.id.clone(),
                        version: link.version,
                        kind: r.kind,
                        source_only: r.source_only,
                    });
                    Outcome::Captured
                }
            },
        };
        let source = Source {
            schema: SCHEMA.into(),
            account: account.into(),
            request: input.request.clone(),
            outcome,
            referrer: identity,
            consent_version: input.consent.then(|| CONSENT.into()),
            captured_at: now,
        };
        self.sources.insert(
            account.into(),
            Recorded {
                input: input_digest,
                source: source.clone(),
                at_signup: Some(false),
            },
        );
        Ok((source, true))
    }
}

impl Accounts {
    pub fn create_account_unattributed(&self, label: &str) -> Result<(Account, Source), Error> {
        self.create_account_acquired(
            label,
            &Capture::missing(format!("signup_{}", fresh().map_err(storage)?)),
        )
    }
    fn referral_write<T>(
        &self,
        apply: impl FnOnce(&mut Store, u64) -> Result<(T, bool), Error>,
    ) -> Result<T, Error> {
        let _lock = Lock::acquire(&self.dir).map_err(storage)?;
        let mut store = load(&self.dir).map_err(storage)?;
        let prior = store.digest.clone();
        let (result, changed) = apply(&mut store, unix_now())?;
        if changed {
            store.sequence = store.sequence.checked_add(1).ok_or(Error::Bound)?;
            store.supersedes = Some(prior);
            store.seal();
            store.validate(ACCOUNTS).map_err(Error::Store)?;
            save(&self.dir, &store).map_err(storage)?;
        }
        Ok(result)
    }
    pub fn create_referrer(&self, actor: &str, kind: Kind, label: &str) -> Result<Referrer, Error> {
        self.referrer_create(actor, kind, label, false)
    }
    /// Operator-only provisioning for OpenAgents sales agents. No HTTP route
    /// can convert one of these immutable source-only identities to a payee.
    pub fn create_sales_referrer(&self, owner: &str, label: &str) -> Result<Referrer, Error> {
        self.referrer_create(owner, Kind::Agent, label, true)
    }
    fn referrer_create(
        &self,
        actor: &str,
        kind: Kind,
        label: &str,
        source_only: bool,
    ) -> Result<Referrer, Error> {
        bounded(label, 128)?;
        self.referral_write(|store, _| {
            if !store.accounts.contains_key(actor) {
                return Err(Error::Unauthorized);
            }
            if store.referrals.referrers.len() >= LIMIT {
                return Err(Error::Bound);
            }
            let record = Referrer {
                schema: SCHEMA.into(),
                id: format!("ref_{}", &fresh().map_err(storage)?[..32]),
                version: 1,
                owner: actor.into(),
                kind,
                label: label.into(),
                source_only,
                pending_owner: None,
            };
            store
                .referrals
                .referrers
                .insert(record.id.clone(), record.clone());
            Ok((record, true))
        })
    }
    pub fn referrer(&self, actor: &str, id: &str) -> Result<Referrer, Error> {
        let store = load(&self.dir).map_err(storage)?;
        let record = store
            .referrals
            .referrers
            .get(id)
            .ok_or(Error::Unavailable)?;
        if record.owner != actor || !store.accounts.contains_key(actor) {
            return Err(Error::Unauthorized);
        }
        Ok(record.clone())
    }
    /// Rotate all earlier links; captured sources retain their historical ID.
    pub fn issue_referral_link(&self, actor: &str, id: &str) -> Result<Issued, Error> {
        self.referral_write(|store, _| {
            let record = store
                .referrals
                .referrers
                .get_mut(id)
                .ok_or(Error::Unavailable)?;
            if record.owner != actor {
                return Err(Error::Unauthorized);
            }
            if store
                .referrals
                .links
                .values()
                .filter(|l| l.referrer == id)
                .count()
                >= LINKS
            {
                return Err(Error::Bound);
            }
            record.version = record.version.checked_add(1).ok_or(Error::Bound)?;
            for link in store
                .referrals
                .links
                .values_mut()
                .filter(|l| l.referrer == id)
            {
                link.active = false;
            }
            let token = format!("rfr_{}", fresh().map_err(storage)?);
            store.referrals.links.insert(
                hash(token.as_bytes()),
                Link {
                    referrer: id.into(),
                    version: record.version,
                    active: true,
                },
            );
            Ok((
                Issued {
                    referrer: id.into(),
                    path: format!("/join?ref={token}"),
                    token,
                },
                true,
            ))
        })
    }
    pub fn disable_referral_links(&self, actor: &str, id: &str) -> Result<(), Error> {
        self.referral_write(|store, _| {
            if store
                .referrals
                .referrers
                .get(id)
                .ok_or(Error::Unavailable)?
                .owner
                != actor
            {
                return Err(Error::Unauthorized);
            }
            let mut changed = false;
            for link in store
                .referrals
                .links
                .values_mut()
                .filter(|l| l.referrer == id)
            {
                changed |= link.active;
                link.active = false;
            }
            Ok(((), changed))
        })
    }
    pub fn offer_referrer_migration(
        &self,
        actor: &str,
        id: &str,
        new_owner: &str,
    ) -> Result<Referrer, Error> {
        self.offer_referrer_migration_guarded(actor, id, new_owner, || true)
    }
    pub fn offer_referrer_migration_guarded(
        &self,
        actor: &str,
        id: &str,
        new_owner: &str,
        current: impl FnOnce() -> bool,
    ) -> Result<Referrer, Error> {
        self.referral_write(|store, _| {
            if !current() {
                return Err(Error::Unauthorized);
            }
            if !store.accounts.contains_key(new_owner) || actor == new_owner {
                return Err(Error::Invalid);
            }
            let record = store
                .referrals
                .referrers
                .get_mut(id)
                .ok_or(Error::Unavailable)?;
            if record.owner != actor {
                return Err(Error::Unauthorized);
            }
            if record.pending_owner.as_deref() == Some(new_owner) {
                return Ok((record.clone(), false));
            }
            record.pending_owner = Some(new_owner.into());
            Ok((record.clone(), true))
        })
    }
    pub fn accept_referrer_migration(&self, actor: &str, id: &str) -> Result<Referrer, Error> {
        self.accept_referrer_migration_guarded(actor, id, || true)
    }
    pub fn accept_referrer_migration_guarded(
        &self,
        actor: &str,
        id: &str,
        current: impl FnOnce() -> bool,
    ) -> Result<Referrer, Error> {
        self.referral_write(|store, _| {
            if !current() {
                return Err(Error::Unauthorized);
            }
            let record = store
                .referrals
                .referrers
                .get_mut(id)
                .ok_or(Error::Unavailable)?;
            if record.pending_owner.as_deref() != Some(actor) {
                if record.pending_owner.is_none()
                    && record.owner == actor
                    && store.referrals.attribution.confirms_manager(record, actor)
                {
                    return Ok((record.clone(), false));
                }
                return Err(Error::Unauthorized);
            }
            let predecessor = record.owner.clone();
            record.owner = actor.into();
            record.pending_owner = None;
            record.version = record.version.checked_add(1).ok_or(Error::Bound)?;
            for link in store
                .referrals
                .links
                .values_mut()
                .filter(|link| link.referrer == id)
            {
                link.active = false;
            }
            let successor = record.clone();
            store
                .referrals
                .attribution
                .record_successor(&successor, &predecessor)?;
            Ok((successor, true))
        })
    }
    pub fn capture_acquisition(&self, actor: &str, input: &Capture) -> Result<Source, Error> {
        self.referral_write(|store, now| {
            if !store.accounts.contains_key(actor) {
                return Err(Error::Unauthorized);
            }
            store.referrals.capture(actor, input, now)
        })
    }
    pub fn acquisition(&self, actor: &str) -> Result<Option<Source>, Error> {
        let store = load(&self.dir).map_err(storage)?;
        if !store.accounts.contains_key(actor) {
            return Err(Error::Unauthorized);
        }
        Ok(store.referrals.sources.get(actor).map(|r| r.source.clone()))
    }
    /// Signup provisions the account and its consented source in one revision.
    pub fn create_account_acquired(
        &self,
        label: &str,
        input: &Capture,
    ) -> Result<(Account, Source), Error> {
        bounded(label, 256)?;
        input.validate()?;
        self.referral_write(|store, now| {
            if store
                .referrals
                .sources
                .values()
                .any(|record| record.source.request == input.request)
            {
                return Err(Error::Conflict);
            }
            let account = Account {
                id: format!("acct_{}", &fresh().map_err(storage)?[..16]),
                label: label.into(),
                principals: vec![],
                created: crate::registry::now_utc(),
            };
            store.accounts.insert(account.id.clone(), account.clone());
            let (source, _) = store.referrals.capture(&account.id, input, now)?;
            store
                .referrals
                .sources
                .get_mut(&account.id)
                .ok_or(Error::Unavailable)?
                .at_signup = Some(true);
            Ok(((account, source), true))
        })
    }
}

#[cfg(test)]
mod tests;
