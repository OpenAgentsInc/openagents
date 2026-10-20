//! Readable names for Nostr keys: `name@openagents.com` (NIP-05, #12254).
//!
//! An account claims one name and points it at one of its own Nostr keys.
//! Names are first come, first served. A name on [`RESERVED`] (brand
//! names, staff, and the words people expect to be official) can't be
//! claimed. An account changes its name at most [`CHANGES_PER_DAY`] times
//! in a rolling day, so names can't be churned through to squat them. A
//! released name is free at once.
//!
//! The book holds only the name, the account, the key, and when. A lookup
//! answers a claim only while its account still holds the key, so removing
//! a key or deleting the account takes the name down with it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::nostr::{keys_of, valid_pubkey};
use super::{Refusal, Store, Trouble};

/// The most name changes one account makes in [`DAY`] seconds.
pub const CHANGES_PER_DAY: usize = 3;

/// The window [`CHANGES_PER_DAY`] counts over.
pub const DAY: u64 = 24 * 60 * 60;

/// The shortest and longest names.
pub const MIN_LEN: usize = 2;
pub const MAX_LEN: usize = 30;

/// Names nobody claims: our brand and products, staff, and words that
/// read as official.
pub const RESERVED: &[&str] = &[
    // The brand and its products.
    "openagents",
    "open-agents",
    "open_agents",
    "oa",
    "coder",
    "microcoder",
    "foreman",
    "verse",
    "pylon",
    "pylons",
    "gym",
    "forum",
    "explore",
    "jev",
    "kev",
    "lev",
    "laya",
    "typesafe",
    "systemone",
    "system-one",
    "sarah",
    "alice",
    "everglade",
    "lagrange",
    // Staff.
    "chris",
    "christopher",
    "christopherdavid",
    "atlantispleb",
    // Words that read as official.
    "admin",
    "administrator",
    "root",
    "sysadmin",
    "system",
    "staff",
    "team",
    "official",
    "support",
    "help",
    "helpdesk",
    "security",
    "abuse",
    "moderator",
    "mod",
    "mods",
    "trust",
    "safety",
    "legal",
    "privacy",
    "terms",
    "billing",
    "payments",
    "pay",
    "wallet",
    "status",
    "info",
    "news",
    "press",
    "contact",
    "mail",
    "email",
    "postmaster",
    "hostmaster",
    "webmaster",
    "noreply",
    "no-reply",
    "api",
    "www",
    "app",
    "apps",
    "web",
    "docs",
    "blog",
    "download",
    "settings",
    "login",
    "logout",
    "signin",
    "signup",
    "account",
    "accounts",
    "agent",
    "agents",
    "bot",
    "bots",
    "nostr",
    "bitcoin",
    "lightning",
    "spark",
    "relay",
    "test",
    "null",
    "undefined",
    "everyone",
    "anonymous",
    "me",
];

/// One claimed name.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    /// The account that claimed it.
    pub account: String,
    /// The key it names, 64 lowercase hex characters, one of the account's.
    pub pubkey: String,
    /// When it was claimed or last pointed at a key, as RFC 3339 in UTC.
    pub claimed: String,
}

/// The name book. Empty books are omitted from the store, so stores
/// written before it keep their digests.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    /// Name to claim.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub names: BTreeMap<String, Claim>,
    /// Account to the Unix times of its recent changes, inside [`DAY`].
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub changes: BTreeMap<String, Vec<u64>>,
}

/// Why a name can't be claimed, in words a person can act on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameProblem {
    /// Not 2 to 30 lowercase letters, digits, `-`, or `_`, starting and
    /// ending with a letter or digit.
    Shape,
    /// On [`RESERVED`].
    Reserved,
    /// Another account holds it.
    Taken,
    /// The account changed its name too often today.
    TooOften,
    /// The key isn't one of the account's.
    NotYourKey,
}

impl NameProblem {
    /// A stable code for the wire.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Shape => "name_invalid",
            Self::Reserved => "name_reserved",
            Self::Taken => "name_taken",
            Self::TooOften => "name_too_often",
            Self::NotYourKey => "key_not_yours",
        }
    }

    /// What to tell the person.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            Self::Shape => {
                "Use 2 to 30 lowercase letters, numbers, dashes, or underscores, starting and ending with a letter or number."
            }
            Self::Reserved => "That name is reserved. Try another.",
            Self::Taken => "Someone already has that name. Try another.",
            Self::TooOften => "You've changed your name a few times today. Try again tomorrow.",
            Self::NotYourKey => "That key isn't on your account.",
        }
    }
}

/// The name as a book key: trimmed and lowercased.
#[must_use]
pub fn normalize(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

/// Whether `name` (already normalized) has a name's shape.
#[must_use]
pub fn valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (MIN_LEN..=MAX_LEN).contains(&bytes.len())
        && bytes
            .iter()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_'))
        && bytes[0].is_ascii_alphanumeric()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
}

/// Whether `name` (already normalized) is reserved.
#[must_use]
pub fn reserved(name: &str) -> bool {
    RESERVED.contains(&name)
}

/// Whether `name` could be claimed by someone, ignoring who holds it.
///
/// # Errors
///
/// [`NameProblem::Shape`] or [`NameProblem::Reserved`].
pub fn check(name: &str) -> Result<String, NameProblem> {
    let name = normalize(name);
    if !valid_name(&name) {
        return Err(NameProblem::Shape);
    }
    if reserved(&name) {
        return Err(NameProblem::Reserved);
    }
    Ok(name)
}

impl Book {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty() && self.changes.is_empty()
    }

    /// The name `account` holds, with its claim.
    #[must_use]
    pub fn of_account(&self, account: &str) -> Option<(&str, &Claim)> {
        self.names
            .iter()
            .find(|(_, claim)| claim.account == account)
            .map(|(name, claim)| (name.as_str(), claim))
    }

    /// Shape checks for the store's own validation.
    ///
    /// # Errors
    ///
    /// A message naming the bad entry.
    pub fn validate(&self) -> Result<(), String> {
        let mut accounts = std::collections::BTreeSet::new();
        for (name, claim) in &self.names {
            if !valid_name(name) {
                return Err(format!("names: `{name}` is not a name"));
            }
            if !valid_pubkey(&claim.pubkey) {
                return Err(format!("names: `{name}` names a malformed key"));
            }
            if !accounts.insert(claim.account.as_str()) {
                return Err(format!(
                    "names: account `{}` holds two names",
                    claim.account
                ));
            }
        }
        Ok(())
    }
}

/// A claim that still stands: its account exists and holds its key.
fn standing<'a>(store: &'a Store, name: &str) -> Option<&'a Claim> {
    let claim = store.names.names.get(name)?;
    let account = store.accounts.get(&claim.account)?;
    keys_of(account)
        .iter()
        .any(|key| *key == claim.pubkey)
        .then_some(claim)
}

/// The claim behind `name`, when it stands.
#[must_use]
pub fn resolve(store: &Store, name: &str) -> Option<Claim> {
    standing(store, &normalize(name)).cloned()
}

/// The standing name `account` holds, with its claim.
#[must_use]
pub fn name_of(store: &Store, account: &str) -> Option<(String, Claim)> {
    let (name, _) = store.names.of_account(account)?;
    let name = name.to_string();
    standing(store, &name).cloned().map(|claim| (name, claim))
}

/// The standing name that points at `pubkey`.
#[must_use]
pub fn name_for_key(store: &Store, pubkey: &str) -> Option<String> {
    store
        .names
        .names
        .iter()
        .find(|(_, claim)| claim.pubkey == pubkey)
        .map(|(name, _)| name.clone())
        .filter(|name| standing(store, name).is_some())
}

/// What a claim attempt answered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Claimed {
    /// The name now names the key.
    Done { name: String, claim: Claim },
    /// It can't be claimed.
    Refused(NameProblem),
}

impl super::Accounts {
    /// Claim `name` for `account`, pointing at `pubkey` (one of its keys;
    /// `None` takes its first key). Replaces the account's earlier name.
    /// Pointing the name it already holds at another of its keys is not a
    /// change and isn't counted.
    ///
    /// # Errors
    ///
    /// A store trouble; a refusal the person can act on is
    /// [`Claimed::Refused`].
    pub fn claim_name(
        &self,
        account: &str,
        name: &str,
        pubkey: Option<&str>,
    ) -> Result<Claimed, Refusal> {
        let name = match check(name) {
            Ok(name) => name,
            Err(problem) => return Ok(Claimed::Refused(problem)),
        };
        self.mutate(|store, now| {
            let record = store
                .accounts
                .get(account)
                .ok_or_else(|| Refusal::UnknownAccount(account.to_string()))?;
            let keys = keys_of(record);
            let key = match pubkey {
                Some(key) => key.to_ascii_lowercase(),
                None => match keys.first() {
                    Some(key) => key.clone(),
                    None => return Ok(Claimed::Refused(NameProblem::NotYourKey)),
                },
            };
            if !keys.contains(&key) {
                return Ok(Claimed::Refused(NameProblem::NotYourKey));
            }
            if let Some(holder) = store.names.names.get(&name)
                && holder.account != account
                && standing(store, &name).is_some()
            {
                return Ok(Claimed::Refused(NameProblem::Taken));
            }
            let same = store
                .names
                .of_account(account)
                .is_some_and(|(held, _)| held == name);
            let recent = store.names.changes.entry(account.to_string()).or_default();
            recent.retain(|at| now.saturating_sub(*at) < DAY);
            if !same {
                if recent.len() >= CHANGES_PER_DAY {
                    return Ok(Claimed::Refused(NameProblem::TooOften));
                }
                recent.push(now);
            }
            if recent.is_empty() {
                store.names.changes.remove(account);
            }
            store
                .names
                .names
                .retain(|_, claim| claim.account != account);
            let claim = Claim {
                account: account.to_string(),
                pubkey: key,
                claimed: crate::registry::now_utc(),
            };
            store.names.names.insert(name.clone(), claim.clone());
            store
                .names
                .validate()
                .map_err(|e| Refusal::Store(Trouble::Invalid(e)))?;
            Ok(Claimed::Done { name, claim })
        })
    }

    /// Release `account`'s name. Answers the name it held, if any.
    ///
    /// # Errors
    ///
    /// A store trouble.
    pub fn release_name(&self, account: &str) -> Result<Option<String>, Refusal> {
        self.mutate(|store, _| {
            let held = store
                .names
                .of_account(account)
                .map(|(name, _)| name.to_string());
            store
                .names
                .names
                .retain(|_, claim| claim.account != account);
            Ok(held)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Accounts;

    const A: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
    const B: &str = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
    const C: &str = "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3";

    fn two() -> (tempfile::TempDir, Accounts, String, String) {
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::install(dir.path()).unwrap();
        let a = accounts.sign_in_nostr(A, "signup").unwrap().account.id;
        let b = accounts.sign_in_nostr(B, "signup").unwrap().account.id;
        (dir, accounts, a, b)
    }

    fn done(claimed: Claimed) -> (String, Claim) {
        match claimed {
            Claimed::Done { name, claim } => (name, claim),
            Claimed::Refused(problem) => panic!("refused: {problem:?}"),
        }
    }

    #[test]
    fn shapes() {
        for good in ["ab", "satoshi", "a-b_c", "x1", &"a".repeat(30)] {
            assert!(valid_name(good), "{good}");
        }
        for bad in [
            "a",
            "-ab",
            "ab-",
            "_ab",
            "a b",
            "a.b",
            "Ab",
            "ab@",
            &"a".repeat(31),
        ] {
            assert!(!valid_name(bad), "{bad}");
        }
        assert_eq!(check(" Satoshi "), Ok("satoshi".into()));
        assert_eq!(check("admin"), Err(NameProblem::Reserved));
        assert_eq!(check("OpenAgents"), Err(NameProblem::Reserved));
        assert_eq!(check("a"), Err(NameProblem::Shape));
        for problem in [
            NameProblem::Shape,
            NameProblem::Reserved,
            NameProblem::Taken,
            NameProblem::TooOften,
            NameProblem::NotYourKey,
        ] {
            assert!(!problem.message().is_empty());
        }
    }

    #[test]
    fn first_come_first_served() {
        let (_dir, accounts, a, b) = two();
        let (name, claim) = done(accounts.claim_name(&a, "Satoshi", None).unwrap());
        assert_eq!(name, "satoshi");
        assert_eq!(claim.pubkey, A);
        assert_eq!(
            accounts.claim_name(&b, "satoshi", None).unwrap(),
            Claimed::Refused(NameProblem::Taken)
        );
        let store = accounts.store().unwrap();
        assert_eq!(resolve(&store, "SATOSHI").unwrap().account, a);
        assert_eq!(name_of(&store, &a).unwrap().0, "satoshi");
        assert_eq!(name_for_key(&store, A).as_deref(), Some("satoshi"));
        assert!(name_of(&store, &b).is_none());
    }

    #[test]
    fn one_name_per_account_and_releasing_frees_it() {
        let (_dir, accounts, a, b) = two();
        done(accounts.claim_name(&a, "first", None).unwrap());
        done(accounts.claim_name(&a, "second", None).unwrap());
        let store = accounts.store().unwrap();
        assert!(resolve(&store, "first").is_none());
        assert_eq!(resolve(&store, "second").unwrap().account, a);
        assert_eq!(
            accounts.release_name(&a).unwrap().as_deref(),
            Some("second")
        );
        done(accounts.claim_name(&b, "second", None).unwrap());
    }

    #[test]
    fn only_the_accounts_own_keys() {
        let (_dir, accounts, a, _b) = two();
        assert_eq!(
            accounts.claim_name(&a, "mine", Some(B)).unwrap(),
            Claimed::Refused(NameProblem::NotYourKey)
        );
        accounts.link_nostr(&a, C).unwrap();
        let (_, claim) = done(accounts.claim_name(&a, "mine", Some(C)).unwrap());
        assert_eq!(claim.pubkey, C);
        // Pointing the same name at another key isn't a change.
        for _ in 0..5 {
            done(accounts.claim_name(&a, "mine", Some(A)).unwrap());
        }
    }

    #[test]
    fn a_removed_key_takes_the_name_down() {
        let (_dir, accounts, a, b) = two();
        accounts.link_nostr(&a, C).unwrap();
        done(accounts.claim_name(&a, "gone", Some(C)).unwrap());
        accounts.unlink_nostr(&a, C).unwrap();
        let store = accounts.store().unwrap();
        assert!(resolve(&store, "gone").is_none());
        // And someone else may take it.
        done(accounts.claim_name(&b, "gone", None).unwrap());
    }

    #[test]
    fn changes_are_limited_per_day() {
        let (_dir, accounts, a, _b) = two();
        for i in 0..CHANGES_PER_DAY {
            done(accounts.claim_name(&a, &format!("name{i}"), None).unwrap());
        }
        assert_eq!(
            accounts.claim_name(&a, "onemore", None).unwrap(),
            Claimed::Refused(NameProblem::TooOften)
        );
        assert_eq!(
            accounts.claim_name(&a, "admin", None).unwrap(),
            Claimed::Refused(NameProblem::Reserved)
        );
    }

    #[test]
    fn the_book_survives_a_reload() {
        let (dir, accounts, a, _b) = two();
        done(accounts.claim_name(&a, "kept", None).unwrap());
        let store = Accounts::open(dir.path()).unwrap().store().unwrap();
        assert_eq!(resolve(&store, "kept").unwrap().pubkey, A);
    }
}
