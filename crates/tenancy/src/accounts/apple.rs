//! Apple identities linked by Sign in with Apple (#11980).
//!
//! Sign in with Apple was removed (#12005), so nothing links a new Apple
//! identity. The book still holds the ones linked before, under Apple's
//! stable user id (`sub`): the account and the address Apple shared,
//! beside the `apple:<sub>` principal on the account, so those accounts
//! keep their data. Validation requires the two to agree, as it does for
//! GitHub. No Apple token is kept.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Store, Trouble};

/// The most Apple identities the book holds.
const MAX_IDENTITIES: usize = 1_000_000;
/// The longest Apple `sub` accepted.
const MAX_SUB: usize = 128;
/// The longest address or label kept.
const MAX_TEXT: usize = 320;

/// The principal reference for an Apple user id.
#[must_use]
pub fn apple_principal(sub: &str) -> String {
    format!("apple:{sub}")
}

/// Whether `sub` can be an Apple user id: Apple's are dotted runs of
/// letters and digits (`001234.0a1b….1234`).
#[must_use]
pub fn valid_sub(sub: &str) -> bool {
    !sub.is_empty()
        && sub.len() <= MAX_SUB
        && sub
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_')
}

/// What a verified Apple identity token said about the person.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppleProfile {
    /// Apple's stable user id for this app team.
    pub sub: String,
    /// The address Apple shared, when the person shared one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: bool,
    /// Whether the address is Apple's private relay.
    #[serde(default)]
    pub private_email: bool,
}

impl AppleProfile {
    /// The bounds a record must keep before the store accepts it.
    pub fn validate(&self) -> Result<(), String> {
        let email_ok = self.email.as_deref().is_none_or(|email| {
            !email.is_empty() && email.len() <= MAX_TEXT && !email.chars().any(char::is_control)
        });
        if !valid_sub(&self.sub) || !email_ok {
            return Err("invalid Apple identity record".into());
        }
        Ok(())
    }
}

/// A linked Apple identity: the account it signs in and what Apple said.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppleIdentity {
    pub account: String,
    pub profile: AppleProfile,
    /// When the identity was linked, as RFC 3339 in UTC.
    pub linked: String,
    /// When it was last signed in with, as RFC 3339 in UTC.
    pub refreshed: String,
}

pub(super) fn validate(
    book: &BTreeMap<String, AppleIdentity>,
    store: &Store,
) -> Result<(), String> {
    if book.len() > MAX_IDENTITIES {
        return Err("Apple identity book exceeds its bound".into());
    }
    for (sub, identity) in book {
        identity.profile.validate()?;
        if identity.profile.sub != *sub {
            return Err("an Apple identity is filed under the wrong id".into());
        }
        let account = store
            .accounts
            .get(&identity.account)
            .ok_or("an Apple identity names no account")?;
        if !account.principals.contains(&apple_principal(sub)) {
            return Err("an Apple identity's account does not hold its principal".into());
        }
    }
    for account in store.accounts.values() {
        let mut apple = 0;
        for principal in &account.principals {
            if let Some(sub) = principal.strip_prefix("apple:") {
                apple += 1;
                if book.get(sub).is_none_or(|i| i.account != account.id) {
                    return Err("an Apple principal has no identity record".into());
                }
            }
        }
        if apple > 1 {
            return Err("an account holds more than one Apple identity".into());
        }
    }
    Ok(())
}

impl super::Accounts {
    /// The Apple identity linked under an Apple user id, when one is.
    pub fn apple_identity(&self, sub: &str) -> Result<Option<AppleIdentity>, Trouble> {
        Ok(super::load(&self.dir)?.identities.apple.get(sub).cloned())
    }
}
