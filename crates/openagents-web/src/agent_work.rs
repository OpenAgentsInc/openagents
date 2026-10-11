//! Agent work on the website (#11162): Environments (`/environments`),
//! Claude Code runs from a chat (`/chat/{id}/claude`), and continuing a
//! Coder chat on a Cloud computer (`/chat/{id}/continue`).
//!
//! They drive machines and models on this server's accounts, so they are
//! only for people the server trusts with them:
//!
//! - on the local address, as before (anyone the local site lets in);
//! - on a public host, a signed-in site admin (`admin` on the deployment's
//!   invite list, `oa_auth::invite`, as the account service reports it),
//!   or an account named in `OPENAGENTS_WEB_AGENT_ACCOUNTS` (staging's
//!   smoke test account, which has no GitHub identity to invite).
//!
//! Everyone else sees no link to them, and their addresses answer the
//! site's ordinary not-found page. A signed-out visitor is sent to log in
//! first ([`gate`]), so the person who may use them gets there.
//!
//! Each environment belongs to the account that made it ([`Scope`]):
//! lists, pages, posts, and runs reach only the person's own. Environments
//! made before they had owners show only on the local address.

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use coder_environment_operator::studio::{Studio, Summary};

use crate::App;
use crate::cloud::session::{SessionError, Viewer};

/// Whether `path` is agent work (the addresses [`gate`] keeps).
pub(crate) fn path(path: &str) -> bool {
    path == "/environments"
        || path.starts_with("/environments/")
        || path == crate::work_runs::PAGE
        || path.starts_with("/work/")
        || (path.starts_with("/chat/")
            && (path.ends_with("/claude") || path.ends_with("/continue")))
}

/// Whether `viewer` may do agent work on a public host.
pub(crate) fn permitted(app: &App, viewer: &Viewer) -> bool {
    viewer.admin
        || app
            .config
            .agent_accounts
            .iter()
            .any(|account| *account == viewer.account_id)
}

/// Whose environments a request reaches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Scope {
    /// The signed-in account; `None` on a server without sign-in.
    pub account: Option<String>,
    /// Environments made before they had owners (and on a server without
    /// sign-in): only on the local address.
    pub legacy: bool,
}

impl Scope {
    /// Whether an environment made for `account` is this request's.
    pub(crate) fn owns(&self, account: Option<&str>) -> bool {
        match account {
            Some(account) => self.account.as_deref() == Some(account),
            None => self.legacy,
        }
    }

    /// The request's environments, most recently changed first.
    pub(crate) fn rows(&self, studio: &Studio) -> Vec<Summary> {
        studio
            .list()
            .into_iter()
            .filter(|row| self.owns(row.account.as_deref()))
            .collect()
    }

    /// Whether the environment `id` is the request's (one that doesn't
    /// exist is answered by the page that reads it).
    pub(crate) fn has(&self, studio: &Studio, id: &str) -> bool {
        self.owns(studio.account(id).as_deref())
    }
}

/// Whose agent work this request may do, or `None` when it may do none:
/// the local address on a server without sign-in (everything, as before);
/// else a signed-in person who is [`permitted`], or anyone signed in on
/// the local address.
pub(crate) async fn scope(app: &App, headers: &axum::http::HeaderMap) -> Option<Scope> {
    let local = crate::local_request(headers);
    let Some(service) = app
        .config
        .cloud
        .as_deref()
        .filter(|_| crate::account::sign_in_available(app))
    else {
        return local.then_some(Scope {
            account: None,
            legacy: true,
        });
    };
    let viewer = service.authenticate(headers).await.ok()?;
    (local || permitted(app, &viewer)).then(|| Scope {
        account: Some(viewer.account_id),
        legacy: local,
    })
}

/// Keeps agent work for the people allowed it on a public host: a
/// signed-out visitor logs in first; anyone else signed in, and every
/// visitor of a server without environments or sign-in, gets the
/// not-found page. The local address passes as before.
pub(crate) async fn gate(State(app): State<App>, request: Request, next: Next) -> Response {
    if !path(request.uri().path()) || crate::local_request(request.headers()) {
        return next.run(request).await;
    }
    let service =
        app.config.cloud.as_deref().filter(|_| {
            crate::account::sign_in_available(&app) && app.config.environments.is_some()
        });
    let Some(service) = service else {
        return crate::not_found_page();
    };
    match service.authenticate(request.headers()).await {
        Ok(viewer) if permitted(&app, &viewer) => next.run(request).await,
        Ok(_) => crate::not_found_page(),
        Err(SessionError::Unavailable) => crate::cloud::refused(SessionError::Unavailable),
        Err(_) => crate::environments::sign_in_first(&request),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agent_work_addresses() {
        for yes in [
            "/environments",
            "/environments/new",
            "/environments/env-1/runs/claude-env-1-1/events",
            "/chat/abc/claude",
            "/chat/abc/continue",
        ] {
            assert!(path(yes), "{yes}");
        }
        for no in ["/environment", "/chat/abc", "/chat", "/app", "/claude", "/"] {
            assert!(!path(no), "{no}");
        }
    }

    #[test]
    fn a_scope_owns_its_accounts_environments_and_old_ones_only_locally() {
        let alice = Scope {
            account: Some("acct_alice".into()),
            legacy: false,
        };
        assert!(alice.owns(Some("acct_alice")));
        assert!(!alice.owns(Some("acct_bob")));
        assert!(!alice.owns(None), "an unowned environment stays local");
        let local = Scope {
            account: Some("acct_alice".into()),
            legacy: true,
        };
        assert!(local.owns(None));
        assert!(!local.owns(Some("acct_bob")));
        let unsigned = Scope {
            account: None,
            legacy: true,
        };
        assert!(unsigned.owns(None));
        assert!(!unsigned.owns(Some("acct_alice")));
    }
}
