//! Sign in with Apple identities (#11980).
//!
//! The account service verifies Apple's identity token (in `oa-auth`) and
//! hands the verified claims here. The book keeps, under Apple's stable
//! user id (`sub`), the account it signs in and the address Apple shared,
//! beside the `apple:<sub>` principal on the account. Validation requires
//! the two to agree, as it does for GitHub. No Apple token is kept.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::identities::personal_workspace;
use super::{Account, Refusal, Store, Trouble, Workspace, fresh};

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

/// What an Apple sign-in resolved to.
#[derive(Clone, Debug)]
pub struct AppleSignIn {
    pub account: Account,
    /// The personal workspace, when this sign-in created the account.
    pub workspace: Option<Workspace>,
    /// Whether this sign-in created the account.
    pub created: bool,
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

/// A new account's label: the name the person gave Apple's sheet, else
/// the shared address's name part (not a private relay's), else a plain
/// default.
fn label(profile: &AppleProfile, name: Option<&str>) -> String {
    let clean = |value: &str| -> String {
        value
            .chars()
            .filter(|c| !c.is_control())
            .take(256)
            .collect::<String>()
            .trim()
            .to_string()
    };
    if let Some(name) = name.map(clean).filter(|n| !n.is_empty()) {
        return name;
    }
    if !profile.private_email {
        if let Some(local) = profile
            .email
            .as_deref()
            .and_then(|e| e.split('@').next())
            .map(clean)
            .filter(|l| !l.is_empty())
        {
            return local;
        }
    }
    "OpenAgents user".to_string()
}

impl super::Accounts {
    /// The Apple identity linked under an Apple user id, when one is.
    pub fn apple_identity(&self, sub: &str) -> Result<Option<AppleIdentity>, Trouble> {
        Ok(super::load(&self.dir)?.identities.apple.get(sub).cloned())
    }

    /// Sign in with a verified Apple identity: find the account that holds
    /// `apple:<sub>` and refresh its record, or create the account (labeled
    /// with `name` when the app sent the one Apple's sheet gave it), its
    /// personal workspace bound to `tenant`, and the link, in one write.
    pub fn sign_in_apple(
        &self,
        profile: &AppleProfile,
        name: Option<&str>,
        tenant: &str,
    ) -> Result<AppleSignIn, Refusal> {
        profile
            .validate()
            .map_err(|e| Refusal::Store(Trouble::Invalid(e)))?;
        if tenant.is_empty() {
            return Err(Refusal::EmptyField("tenant"));
        }
        let key = profile.sub.clone();
        self.mutate(|store, _| {
            let stamp = crate::registry::now_utc();
            if let Some(identity) = store.identities.apple.get_mut(&key) {
                // Apple sends the address only while the person shares it.
                if profile.email.is_some() {
                    identity.profile = profile.clone();
                }
                identity.refreshed = stamp;
                let account = store
                    .accounts
                    .get(&identity.account)
                    .cloned()
                    .ok_or_else(|| Refusal::UnknownAccount(identity.account.clone()))?;
                return Ok(AppleSignIn {
                    account,
                    workspace: None,
                    created: false,
                });
            }
            let account = Account {
                id: format!("acct_{}", &fresh().map_err(Refusal::Store)?[..16]),
                label: label(profile, name),
                principals: vec![apple_principal(&key)],
                created: stamp.clone(),
                agent: None,
            };
            let workspace = personal_workspace(&account, tenant, &stamp)?;
            store
                .referrals
                .inherit_workspace(&workspace)
                .map_err(|error| Refusal::Store(Trouble::Invalid(error.to_string())))?;
            store.accounts.insert(account.id.clone(), account.clone());
            store
                .workspaces
                .insert(workspace.id.clone(), workspace.clone());
            store.identities.apple.insert(
                key,
                AppleIdentity {
                    account: account.id.clone(),
                    profile: profile.clone(),
                    linked: stamp.clone(),
                    refreshed: stamp,
                },
            );
            Ok(AppleSignIn {
                account,
                workspace: Some(workspace),
                created: true,
            })
        })
    }
}
