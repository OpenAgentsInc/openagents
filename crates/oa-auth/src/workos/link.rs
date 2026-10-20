//! Which account a WorkOS User signs in to (#12321, #12323).
//!
//! One WorkOS User per person, and never a second account for someone we
//! already know. In order:
//!
//! 1. The User is already linked to an account.
//! 2. The User's `external_id` is one of our accounts.
//! 3. The User signed in with GitHub, and an account holds that GitHub
//!    identity (`github:<id>`).
//! 4. The User's email is verified and matches the verified GitHub email
//!    of exactly one account.
//! 5. Otherwise a new account and its personal workspace are made.
//!
//! An account already linked to another User whose email is one of our
//! synthetic addresses (a key-only account the dual-write made a User
//! for) moves to the User that just signed in, and the synthetic User is
//! deleted. An account linked to another real User is never taken over.

use std::path::Path;

use tenancy::workos_links::{LinkError, Links};
use tenancy::{Account, Accounts, Workspace};

use super::client::{Identity, Refusal, User, UserChange, Workos};

/// How the account was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkedBy {
    Existing,
    ExternalId,
    Github,
    Email,
    New,
}

impl LinkedBy {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Existing => "existing",
            Self::ExternalId => "external_id",
            Self::Github => "github",
            Self::Email => "email",
            Self::New => "new",
        }
    }
}

/// The account a User signs in to.
#[derive(Debug)]
pub struct Resolved {
    pub account: Account,
    /// The personal workspace, when the account is new.
    pub workspace: Option<Workspace>,
    pub created: bool,
    pub by: LinkedBy,
    /// The GitHub user ID, when the User signed in with GitHub.
    pub github: Option<u64>,
}

/// Why no account could be resolved.
#[derive(Debug, PartialEq, Eq)]
pub enum ResolveError {
    /// The account that matches is linked to another person's User.
    Conflict,
    /// No account matches and this deployment makes no accounts.
    SignupClosed,
    /// A store or WorkOS can't be reached.
    Unavailable,
}

/// One resolution at a time: two first sign-ins by the same person must
/// not make two accounts.
fn gate() -> &'static tokio::sync::Mutex<()> {
    static GATE: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    GATE.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// The GitHub user ID among a User's identities.
#[must_use]
pub fn github_id(identities: &[Identity]) -> Option<u64> {
    identities
        .iter()
        .find(|identity| identity.provider.eq_ignore_ascii_case("GithubOAuth"))
        .and_then(|identity| identity.idp_id.parse().ok())
}

/// Find or make the account for `user`, link it, and set the User's
/// external ID. `tenant` is the sign-up tenant a new account's personal
/// workspace binds to; `None` makes no new accounts.
pub async fn resolve(
    dir: &Path,
    workos: &Workos,
    user: &User,
    tenant: Option<&str>,
) -> Result<Resolved, ResolveError> {
    let _held = gate().lock().await;
    let links = Links::open(dir);
    let accounts = Accounts::open(dir).map_err(|_| ResolveError::Unavailable)?;
    let store = accounts.store().map_err(|_| ResolveError::Unavailable)?;
    let existing = |id: &str| store.accounts.get(id).cloned();

    let identities = workos.identities(&user.id).await.unwrap_or_default();
    let github = github_id(&identities);

    // 1. Already linked.
    if let Some(account) = links
        .account_of(&user.id)
        .map_err(|_| ResolveError::Unavailable)?
        .and_then(|id| existing(&id))
    {
        stamp(workos, user, &account.id).await;
        return Ok(Resolved {
            account,
            workspace: None,
            created: false,
            by: LinkedBy::Existing,
            github,
        });
    }

    // 2 to 4: an account we already hold.
    let mut candidate = None;
    if let Some(account) = user.external_id.as_deref().and_then(existing) {
        candidate = Some((account, LinkedBy::ExternalId));
    }
    if candidate.is_none()
        && let Some(id) = github
        && let Some(identity) = store.identities.github.get(&id.to_string())
        && let Some(account) = existing(&identity.account)
    {
        candidate = Some((account, LinkedBy::Github));
    }
    if candidate.is_none() && user.email_verified && !super::is_synthetic(&user.email) {
        let mut matches = store
            .accounts
            .values()
            .filter(|account| account.agent.is_none())
            .filter(|account| {
                store
                    .identities
                    .email_of(&account.id)
                    .is_some_and(|email| email.eq_ignore_ascii_case(user.email.trim()))
            });
        if let (Some(account), None) = (matches.next(), matches.next()) {
            candidate = Some((account.clone(), LinkedBy::Email));
        }
    }
    if let Some((account, by)) = candidate {
        claim(workos, &links, &account.id, &user.id).await?;
        stamp(workos, user, &account.id).await;
        return Ok(Resolved {
            account,
            workspace: None,
            created: false,
            by,
            github,
        });
    }

    // 5. Someone new.
    let tenant = tenant.ok_or(ResolveError::SignupClosed)?;
    let made = accounts
        .sign_up_workos(&user.label(), tenant)
        .map_err(|_| ResolveError::Unavailable)?;
    links
        .link_user(&made.account.id, &user.id)
        .map_err(|_| ResolveError::Unavailable)?;
    stamp(workos, user, &made.account.id).await;
    Ok(Resolved {
        account: made.account,
        workspace: made.workspace,
        created: true,
        by: LinkedBy::New,
        github,
    })
}

/// Link `account` to `user`, moving it off a synthetic User of ours.
async fn claim(
    workos: &Workos,
    links: &Links,
    account: &str,
    user: &str,
) -> Result<(), ResolveError> {
    match links.link_user(account, user) {
        Ok(()) => Ok(()),
        Err(LinkError::Taken) => {
            let Some(previous) = links
                .user_of(account)
                .map_err(|_| ResolveError::Unavailable)?
            else {
                // The User is linked to another account.
                return Err(ResolveError::Conflict);
            };
            let old = workos
                .user(&previous)
                .await
                .map_err(|_| ResolveError::Unavailable)?;
            if old
                .as_ref()
                .is_some_and(|old| !super::is_synthetic(&old.email))
            {
                return Err(ResolveError::Conflict);
            }
            links
                .unlink_account(account)
                .map_err(|_| ResolveError::Unavailable)?;
            links
                .link_user(account, user)
                .map_err(|_| ResolveError::Conflict)?;
            if old.is_some() {
                workos.delete_user(&previous).await.ok();
            }
            Ok(())
        }
        Err(_) => Err(ResolveError::Unavailable),
    }
}

/// Set the User's external ID to the account, unless WorkOS holds a
/// different one we didn't set. Best effort: the link is what we read.
async fn stamp(workos: &Workos, user: &User, account: &str) {
    match user.external_id.as_deref() {
        Some(id) if id == account => {}
        Some(id) if !id.starts_with("acct_") => {}
        _ => {
            workos
                .update_user(
                    &user.id,
                    UserChange {
                        external_id: Some(account.to_string()),
                        ..UserChange::default()
                    },
                )
                .await
                .ok();
        }
    }
}

impl From<Refusal> for ResolveError {
    fn from(_: Refusal) -> Self {
        Self::Unavailable
    }
}
