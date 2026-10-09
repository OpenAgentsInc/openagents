//! The account service's GitHub routes, as plain functions over the
//! tenancy stores: `POST /v1/sessions/github` ([`sign_in`]) and
//! `POST /v1/account/identities/github` ([`link`]). The gateway and the
//! local fixture's account service both call these.

use std::path::Path;

use tenancy::accounts::identities::{GithubIdentity, github_principal};
use tenancy::sessions::{self, Issued, Sessions};
use tenancy::workspaces::UserId;
use tenancy::{Account, Accounts};

use crate::AuthError;
use crate::github::Github;
use crate::invite::{self, InviteOnly};

/// The body both routes take.
#[derive(Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeRequest {
    pub code: String,
    pub code_verifier: String,
}

impl std::fmt::Debug for CodeRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CodeRequest([redacted])")
    }
}

/// A completed GitHub sign-in: the account, whether it is new, and the
/// session (its token is in `session.once`, returned exactly once).
pub struct SignedIn {
    pub account: Account,
    pub workspace: Option<tenancy::Workspace>,
    pub created: bool,
    pub session: Issued,
    /// The GitHub user signed in as: id and current login.
    pub github: (u64, String),
    /// A site admin by the deployment's invite list.
    pub admin: bool,
}

impl std::fmt::Debug for SignedIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedIn")
            .field("account", &self.account.id)
            .field("created", &self.created)
            .finish_non_exhaustive()
    }
}

/// Sign in (or sign up) with a GitHub authorization code.
///
/// `dir` holds `accounts.json` and `sessions.json`; `tenant` is the
/// deployment's sign-up tenant a new personal workspace binds to. With
/// `invite` set, a GitHub user not on it is refused
/// ([`AuthError::InviteOnly`]) before the stores are opened: no account,
/// no session.
pub async fn sign_in(
    dir: &Path,
    github: &Github,
    tenant: &str,
    invite: Option<&InviteOnly>,
    request: &CodeRequest,
) -> Result<SignedIn, AuthError> {
    let profile = github
        .profile(&request.code, &request.code_verifier)
        .await?;
    profile.validate().map_err(|_| AuthError::Denied)?;
    if !invite::allowed(invite, profile.id, &profile.login) {
        return Err(AuthError::InviteOnly);
    }
    let admin = invite.is_some_and(|list| list.admin(profile.id, &profile.login));
    let accounts = Accounts::open(dir).map_err(|_| AuthError::Unavailable)?;
    let resolved = accounts
        .sign_in_github(&profile, tenant)
        .map_err(AuthError::from_refusal)?;
    let sessions = Sessions::open(dir).map_err(|_| AuthError::Unavailable)?;
    let account = resolved.account.id.clone();
    let workspace = resolved.workspace.as_ref().map(|w| w.id.clone());
    let created = resolved.created;
    let session = sessions
        .mutate(|book, access, now| {
            let issued = book.issue(UserId::from(account.as_str()), now)?;
            sessions::push_access(
                access,
                sessions::Access {
                    at: now,
                    actor: account.clone(),
                    action: if created { "sign-up" } else { "sign-in" }.to_string(),
                    workspace: workspace.clone(),
                    session: Some(issued.session.id.as_str().to_string()),
                    detail: Some(github_principal(profile.id)),
                },
            );
            Ok(issued)
        })
        .map_err(|_| AuthError::Unavailable)?;
    Ok(SignedIn {
        account: resolved.account,
        workspace: resolved.workspace,
        created,
        session,
        github: (profile.id, profile.login.clone()),
        admin,
    })
}

/// Link the GitHub account behind `code` to the signed-in `account`. With
/// `invite` set, only an invited GitHub account can be linked.
pub async fn link(
    dir: &Path,
    github: &Github,
    invite: Option<&InviteOnly>,
    account: &str,
    request: &CodeRequest,
) -> Result<GithubIdentity, AuthError> {
    let profile = github
        .profile(&request.code, &request.code_verifier)
        .await?;
    profile.validate().map_err(|_| AuthError::Denied)?;
    if !invite::allowed(invite, profile.id, &profile.login) {
        return Err(AuthError::InviteOnly);
    }
    let accounts = Accounts::open(dir).map_err(|_| AuthError::Unavailable)?;
    let identity = accounts
        .link_github(account, &profile)
        .map_err(AuthError::from_refusal)?;
    if let Ok(sessions) = Sessions::open(dir) {
        sessions
            .record(
                account,
                "link",
                None,
                None,
                Some(github_principal(profile.id)),
            )
            .ok();
    }
    Ok(identity)
}

/// Whether the account whose linked GitHub identity is `identity` is a
/// site admin by `invite`. No list, or no GitHub identity, is not admin.
#[must_use]
pub fn site_admin(invite: Option<&InviteOnly>, identity: Option<&GithubIdentity>) -> bool {
    match (invite, identity) {
        (Some(list), Some(identity)) => list.admin(identity.profile.id, &identity.profile.login),
        _ => false,
    }
}

/// The JSON a sign-in answers, the same shape `POST /v1/sessions` uses.
#[must_use]
pub fn signed_in_body(signed: &SignedIn) -> serde_json::Value {
    serde_json::json!({
        "v": "openagents.accounts.v1",
        "session": {
            "id": signed.session.session.id.as_str(),
            "kind": "user",
            "account": signed.account.id,
            "created_at": signed.session.session.created_at,
            "expires_at": signed.session.session.expires_at,
        },
        "token": signed.session.once,
        "created": signed.created,
        "github": {"id": signed.github.0, "login": signed.github.1},
        "admin": signed.admin,
    })
}
