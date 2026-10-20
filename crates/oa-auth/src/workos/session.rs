//! The account service's half of a WorkOS sign-in, as a plain function
//! over the stores (#12323): check the access token WorkOS issued, read
//! the User, find or make the account ([`super::link`]), and issue our
//! session, recording the sign-in with the WorkOS User it came from. The
//! gateway's `POST /v1/sessions/workos` and the local fixture's account
//! service both call [`sign_in`].

use std::path::Path;

use tenancy::sessions::{self, Issued, Sessions};
use tenancy::workspaces::UserId;

use super::client::{Refusal, User, Workos};
use super::link::{LinkedBy, ResolveError, Resolved, github_id, resolve};
use crate::invite::{self, InviteOnly};

/// A finished WorkOS sign-in.
pub struct SignedIn {
    pub resolved: Resolved,
    pub user: User,
    /// The WorkOS session ID the token belongs to.
    pub session_id: String,
    /// Our session; its token is in `session.once`, returned exactly once.
    pub session: Issued,
}

impl std::fmt::Debug for SignedIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedIn")
            .field("account", &self.resolved.account.id)
            .field("created", &self.resolved.created)
            .finish_non_exhaustive()
    }
}

/// Why a WorkOS sign-in didn't finish. Nothing here is secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignInError {
    /// The token isn't one WorkOS signed, or it expired.
    Denied,
    /// The deployment is invite-only and this person isn't on the list.
    InviteOnly,
    /// No account matches and this deployment makes no accounts.
    SignupClosed,
    /// The account that matches belongs to another person's WorkOS User.
    Conflict,
    /// An agent account; it uses its API key.
    Agent,
    /// WorkOS or the stores can't be reached.
    Unavailable,
}

impl SignInError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Denied => "sign_in_denied",
            Self::InviteOnly => "invite_only",
            Self::SignupClosed => "signup_disabled",
            Self::Conflict => "identity_taken",
            Self::Agent => "agent_account",
            Self::Unavailable => "sign_in_unavailable",
        }
    }

    #[must_use]
    pub fn message(&self) -> &'static str {
        match self {
            Self::Denied => "That sign-in didn't go through. Try again.",
            Self::InviteOnly => "Sign-in is invite-only for now.",
            Self::SignupClosed => "This service doesn't make new accounts.",
            Self::Conflict => "That sign-in is already used by another OpenAgents account.",
            Self::Agent => {
                "An agent account uses its API key directly; it can't sign in to the website."
            }
            Self::Unavailable => "Sign-in isn't available right now. Try again in a minute.",
        }
    }

    #[must_use]
    pub fn status(&self) -> u16 {
        match self {
            Self::Denied => 401,
            Self::InviteOnly | Self::SignupClosed | Self::Agent => 403,
            Self::Conflict => 409,
            Self::Unavailable => 503,
        }
    }
}

/// Sign in with the access token a WorkOS sign-in issued. `tenant` is the
/// sign-up tenant a new account binds to (`None` makes no accounts); with
/// a closed `invite` list only its GitHub people get new accounts.
pub async fn sign_in(
    dir: &Path,
    workos: &Workos,
    access_token: &str,
    tenant: Option<&str>,
    invite: Option<&InviteOnly>,
) -> Result<SignedIn, SignInError> {
    let claims = match workos.verify(access_token).await {
        Ok(claims) => claims,
        Err(Refusal::Unavailable) => return Err(SignInError::Unavailable),
        Err(_) => return Err(SignInError::Denied),
    };
    let user = match workos.user(&claims.sub).await {
        Ok(Some(user)) => user,
        Ok(None) => return Err(SignInError::Denied),
        Err(_) => return Err(SignInError::Unavailable),
    };
    let closed = invite.is_some_and(InviteOnly::closed);
    let mut tenant = tenant;
    if closed {
        let identities = workos.identities(&user.id).await.unwrap_or_default();
        let invited = github_id(&identities).is_some_and(|id| invite::allowed(invite, id, ""));
        if !invited {
            tenant = None;
        }
    }
    let resolved = match resolve(dir, workos, &user, tenant).await {
        Ok(resolved) => resolved,
        Err(ResolveError::SignupClosed) if closed => return Err(SignInError::InviteOnly),
        Err(ResolveError::SignupClosed) => return Err(SignInError::SignupClosed),
        Err(ResolveError::Conflict) => return Err(SignInError::Conflict),
        Err(ResolveError::Unavailable) => return Err(SignInError::Unavailable),
    };
    if resolved.account.agent.is_some() {
        return Err(SignInError::Agent);
    }
    let sessions = Sessions::open(dir).map_err(|_| SignInError::Unavailable)?;
    let account = resolved.account.id.clone();
    let created = resolved.created;
    let workspace = resolved.workspace.as_ref().map(|w| w.id.clone());
    let detail = format!("workos:{}", user.id);
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
                    detail: Some(detail.clone()),
                },
            );
            Ok(issued)
        })
        .map_err(|_| SignInError::Unavailable)?;
    Ok(SignedIn {
        resolved,
        user,
        session_id: claims.sid,
        session,
    })
}

/// The JSON a WorkOS sign-in answers: the shape `POST /v1/sessions/github`
/// uses, with the WorkOS User and session added.
#[must_use]
pub fn signed_in_body(signed: &SignedIn, admin: bool) -> serde_json::Value {
    serde_json::json!({
        "v": "openagents.accounts.v1",
        "session": {
            "id": signed.session.session.id.as_str(),
            "kind": "user",
            "account": signed.resolved.account.id,
            "created_at": signed.session.session.created_at,
            "expires_at": signed.session.session.expires_at,
        },
        "token": signed.session.once,
        "created": signed.resolved.created,
        "admin": admin,
        "github": signed.resolved.github.map(|id| serde_json::json!({"id": id, "login": ""})),
        "workos": {
            "user_id": signed.user.id,
            "session_id": signed.session_id,
            "linked_by": LinkedBy::as_str(signed.resolved.by),
        },
    })
}
