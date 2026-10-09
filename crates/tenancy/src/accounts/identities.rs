//! Linked sign-in identities: what a provider told us about an account's
//! credential, beside the principal that resolves to it.
//!
//! An account's `principals` say which credentials sign it in. For a
//! provider credential such as GitHub, the book here keeps the profile the
//! provider returned, keyed by the provider's stable id, and refreshed on
//! every sign-in. Validation requires the two to agree: every `github:`
//! principal has exactly one record here, naming the account that holds it.
//!
//! The book never holds a provider token. See `docs/auth/README.md`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{
    Account, MemberStatus, Membership, Refusal, Role, Store, Trouble, Workspace, WorkspaceKind,
    fresh,
};

/// The most emails one GitHub profile record keeps.
pub const MAX_EMAILS: usize = 32;
/// The longest text field a profile record keeps, in bytes.
pub const MAX_FIELD: usize = 2048;
/// The most GitHub identities the book holds.
const MAX_IDENTITIES: usize = 1_000_000;

/// The principal reference for a GitHub user id.
#[must_use]
pub fn github_principal(id: u64) -> String {
    format!("github:{id}")
}

/// One address from GitHub's `/user/emails`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GithubEmail {
    pub email: String,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub primary: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
}

/// Everything GitHub's `/user` and `/user/emails` told us, under the
/// `read:user user:email` scopes. Optional fields are absent when GitHub
/// sent null or nothing.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GithubProfile {
    /// GitHub's numeric user id: the stable key. Logins can change.
    pub id: u64,
    pub login: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html_url: Option<String>,
    /// The public profile email, when the person set one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Every address `/user/emails` listed, with verified and primary flags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emails: Vec<GithubEmail>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blog: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub twitter_username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hireable: Option<bool>,
    /// `User` or `Organization` (GitHub's `type`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_repos: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_gists: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub followers: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub following: Option<u64>,
    /// When the GitHub account was created, as GitHub wrote it (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

impl GithubProfile {
    /// The account label a new account takes: the name, else the login.
    #[must_use]
    pub fn label(&self) -> String {
        self.name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(&self.login)
            .chars()
            .filter(|c| !c.is_control())
            .take(256)
            .collect()
    }

    /// The primary verified address, else any verified one.
    #[must_use]
    pub fn verified_email(&self) -> Option<&str> {
        self.emails
            .iter()
            .find(|e| e.verified && e.primary)
            .or_else(|| self.emails.iter().find(|e| e.verified))
            .map(|e| e.email.as_str())
    }

    /// The account's picture on GitHub, when it is GitHub's own avatar host.
    #[must_use]
    pub fn avatar(&self) -> Option<&str> {
        self.avatar_url
            .as_deref()
            .filter(|url| url.starts_with("https://avatars.githubusercontent.com/"))
    }

    /// The bounds a record must keep before the store accepts it.
    pub fn validate(&self) -> Result<(), String> {
        let text = |value: &str| value.len() <= MAX_FIELD && !value.chars().any(char::is_control);
        let login_ok = !self.login.is_empty()
            && self.login.len() <= 64
            && self.login.bytes().all(|b| {
                b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'[' || b == b']'
            });
        let optional = [
            &self.node_id,
            &self.name,
            &self.avatar_url,
            &self.html_url,
            &self.email,
            &self.company,
            &self.blog,
            &self.location,
            &self.twitter_username,
            &self.kind,
            &self.created_at,
            &self.updated_at,
        ];
        // A bio may hold line breaks; every other field is one line.
        let bio_ok = self.bio.as_deref().is_none_or(|bio| {
            bio.len() <= MAX_FIELD
                && !bio
                    .chars()
                    .any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t')
        });
        if self.id == 0
            || !login_ok
            || !optional
                .iter()
                .all(|value| value.as_deref().is_none_or(text))
            || !bio_ok
            || self.emails.len() > MAX_EMAILS
            || self
                .emails
                .iter()
                .any(|e| e.email.is_empty() || e.email.len() > 320 || !text(&e.email))
        {
            return Err("invalid GitHub profile record".into());
        }
        Ok(())
    }
}

/// A linked GitHub identity: the account it signs in and the profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GithubIdentity {
    /// The account the identity signs in.
    pub account: String,
    pub profile: GithubProfile,
    /// When the identity was linked, as RFC 3339 in UTC.
    pub linked: String,
    /// When the profile was last refreshed, as RFC 3339 in UTC.
    pub refreshed: String,
}

/// The linked-identity book. Empty books are omitted from the store, so
/// stores written before it keep their digests.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    /// GitHub user id (decimal) to identity.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub github: BTreeMap<String, GithubIdentity>,
}

impl Book {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.github.is_empty()
    }

    /// The GitHub identity linked to `account`, if any.
    #[must_use]
    pub fn github_of(&self, account: &str) -> Option<&GithubIdentity> {
        self.github.values().find(|identity| identity.account == account)
    }

    pub(super) fn validate(&self, store: &Store) -> Result<(), String> {
        if self.github.len() > MAX_IDENTITIES {
            return Err("identity book exceeds its bound".into());
        }
        for (id, identity) in &self.github {
            identity.profile.validate()?;
            if identity.profile.id.to_string() != *id {
                return Err("a GitHub identity is filed under the wrong id".into());
            }
            let account = store
                .accounts
                .get(&identity.account)
                .ok_or("a GitHub identity names no account")?;
            if !account
                .principals
                .contains(&github_principal(identity.profile.id))
            {
                return Err("a GitHub identity's account does not hold its principal".into());
            }
        }
        for account in store.accounts.values() {
            let mut github = 0;
            for principal in &account.principals {
                if let Some(id) = principal.strip_prefix("github:") {
                    github += 1;
                    if self.github.get(id).is_none_or(|i| i.account != account.id) {
                        return Err("a GitHub principal has no identity record".into());
                    }
                }
            }
            if github > 1 {
                return Err("an account holds more than one GitHub identity".into());
            }
        }
        Ok(())
    }
}

/// What a GitHub sign-in resolved to.
#[derive(Clone, Debug)]
pub struct GithubSignIn {
    pub account: Account,
    /// The personal workspace, when this sign-in created the account.
    pub workspace: Option<Workspace>,
    /// Whether this sign-in created the account (a sign-up).
    pub created: bool,
}

impl super::Accounts {
    /// The GitHub identity linked under a GitHub user id, when one is.
    pub fn github_identity(&self, id: u64) -> Result<Option<GithubIdentity>, Trouble> {
        Ok(super::load(&self.dir)?
            .identities
            .github
            .get(&id.to_string())
            .cloned())
    }

    /// Sign in with a verified GitHub profile: find the account that holds
    /// `github:<id>` and refresh its record, or create the account, its
    /// personal workspace bound to `tenant`, and the link, in one write.
    pub fn sign_in_github(
        &self,
        profile: &GithubProfile,
        tenant: &str,
    ) -> Result<GithubSignIn, Refusal> {
        profile
            .validate()
            .map_err(|e| Refusal::Store(Trouble::Invalid(e)))?;
        if tenant.is_empty() {
            return Err(Refusal::EmptyField("tenant"));
        }
        let key = profile.id.to_string();
        self.mutate(|store, _| {
            let stamp = crate::registry::now_utc();
            if let Some(identity) = store.identities.github.get_mut(&key) {
                identity.profile = profile.clone();
                identity.refreshed = stamp;
                let account = store
                    .accounts
                    .get(&identity.account)
                    .cloned()
                    .ok_or_else(|| Refusal::UnknownAccount(identity.account.clone()))?;
                return Ok(GithubSignIn {
                    account,
                    workspace: None,
                    created: false,
                });
            }
            let account = Account {
                id: format!("acct_{}", &fresh().map_err(Refusal::Store)?[..16]),
                label: profile.label(),
                principals: vec![github_principal(profile.id)],
                created: stamp.clone(),
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
            store.identities.github.insert(
                key,
                GithubIdentity {
                    account: account.id.clone(),
                    profile: profile.clone(),
                    linked: stamp.clone(),
                    refreshed: stamp,
                },
            );
            Ok(GithubSignIn {
                account,
                workspace: Some(workspace),
                created: true,
            })
        })
    }

    /// Link a verified GitHub profile to an existing account. Refused when
    /// the GitHub identity already belongs to another account, or when the
    /// account already holds a different GitHub identity. Linking the same
    /// identity again refreshes its record.
    pub fn link_github(
        &self,
        account: &str,
        profile: &GithubProfile,
    ) -> Result<GithubIdentity, Refusal> {
        profile
            .validate()
            .map_err(|e| Refusal::Store(Trouble::Invalid(e)))?;
        let principal = github_principal(profile.id);
        let key = profile.id.to_string();
        self.mutate(|store, _| {
            if let Some(other) = store
                .accounts
                .values()
                .find(|a| a.id != account && a.principals.contains(&principal))
            {
                return Err(Refusal::PrincipalTaken {
                    principal: principal.clone(),
                    account: other.id.clone(),
                });
            }
            let record = store
                .accounts
                .get_mut(account)
                .ok_or_else(|| Refusal::UnknownAccount(account.to_string()))?;
            if record
                .principals
                .iter()
                .any(|p| p.starts_with("github:") && *p != principal)
            {
                return Err(Refusal::ProviderLinked {
                    account: account.to_string(),
                    provider: "github",
                });
            }
            let stamp = crate::registry::now_utc();
            if !record.principals.contains(&principal) {
                record.principals.push(principal.clone());
            }
            let identity = store
                .identities
                .github
                .entry(key)
                .and_modify(|identity| {
                    identity.profile = profile.clone();
                    identity.refreshed = stamp.clone();
                })
                .or_insert_with(|| GithubIdentity {
                    account: account.to_string(),
                    profile: profile.clone(),
                    linked: stamp.clone(),
                    refreshed: stamp.clone(),
                });
            Ok(identity.clone())
        })
    }

    /// Remove an account's GitHub identity. Refused when it is the
    /// account's only way to sign in.
    pub fn unlink_github(&self, account: &str) -> Result<GithubIdentity, Refusal> {
        self.mutate(|store, _| {
            let record = store
                .accounts
                .get_mut(account)
                .ok_or_else(|| Refusal::UnknownAccount(account.to_string()))?;
            let principal = record
                .principals
                .iter()
                .find(|p| p.starts_with("github:"))
                .cloned()
                .ok_or_else(|| Refusal::UnknownPrincipal(format!("github for {account}")))?;
            if record.principals.len() == 1 {
                return Err(Refusal::LastCredential(account.to_string()));
            }
            record.principals.retain(|p| *p != principal);
            let key = principal.trim_start_matches("github:").to_string();
            store
                .identities
                .github
                .remove(&key)
                .ok_or_else(|| Refusal::UnknownPrincipal(principal.clone()))
        })
    }
}

/// A new personal workspace owned by `account`, named after it.
fn personal_workspace(account: &Account, tenant: &str, stamp: &str) -> Result<Workspace, Refusal> {
    let mut members = BTreeMap::new();
    members.insert(
        account.id.clone(),
        Membership {
            account: account.id.clone(),
            role: Role::Owner,
            status: MemberStatus::Active,
            epoch: 1,
            granted: stamp.to_string(),
            revoked: None,
        },
    );
    Ok(Workspace {
        id: format!("ws_{}", &fresh().map_err(Refusal::Store)?[..16]),
        kind: WorkspaceKind::Personal,
        name: account.label.clone(),
        tenant: tenant.to_string(),
        seats: None,
        members_epoch: 1,
        members,
        created: stamp.to_string(),
    })
}
