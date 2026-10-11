//! GitHub access for a Claude Code run (#11226): the run pushes a branch
//! and opens a pull request on its environment's repository as the
//! signed-in person, through their own GitHub connection.
//!
//! [`access`] asks the account service for a credential-broker ticket for
//! that one repository (`oa_auth::repos::broker`; the repository becomes
//! one of the person's projects first when it isn't yet). The run's
//! computer gets only the ticket; Git and `open-pull-request` trade it at
//! [`CREDENTIAL`] on this site for a short-lived installation token scoped
//! to that repository ([`coder_environment_operator::studio::push`]).
//! Without a GitHub App installation on the repository, or on a server
//! that doesn't sign people in, the run gets no access and is told so.

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use coder_environment_operator::studio::Studio;
use coder_environment_operator::studio::push::Push;
use oa_auth::repos::{Access, RepoError};

use crate::App;
use crate::cloud::session::github::RepoCallError;

/// Where a run's Git credential helper redeems its ticket. No session:
/// the ticket is the only authority, and it names one repository.
pub(crate) const CREDENTIAL: &str = "/github/git-credential";

/// How long a run's ticket lasts: the longest run, with room for a usage
/// limit's pause before it continues.
const TICKET_SECONDS: u64 = 12 * 3600;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(CREDENTIAL, post(credential))
        .layer(DefaultBodyLimit::max(4096))
}

async fn credential(State(app): State<App>, form: Bytes) -> Response {
    let (status, text) = match app.config.cloud.as_deref() {
        Some(service) => service.git_credential(&form).await,
        None => (404, "error=unavailable\n".into()),
    };
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::SERVICE_UNAVAILABLE),
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        text,
    )
        .into_response()
}

/// GitHub access for a run on environment `id`, from the signed-in
/// person's own connection; `None` when there's none to give.
pub(crate) async fn access(
    app: &App,
    headers: &HeaderMap,
    studio: &Studio,
    id: &str,
) -> Option<Push> {
    if !crate::account::sign_in_available(app) {
        return None;
    }
    let service = app.config.cloud.as_deref()?;
    let summary = studio.view(id)?.summary;
    let repository = summary.repository;
    let ticket = match service
        .github_broker_ticket(headers, &repository, TICKET_SECONDS)
        .await
    {
        Ok(ticket) => ticket,
        // Not one of the person's projects yet: add it, then ask again.
        Err(RepoCallError::Repo(RepoError::NotFound)) => {
            service.add_project(headers, &repository).await.ok()?;
            service
                .github_broker_ticket(headers, &repository, TICKET_SECONDS)
                .await
                .ok()?
        }
        Err(_) => return None,
    };
    let login = match service.github_status(headers).await.ok()?.access {
        Access::Installed { login, .. }
        | Access::Connected { login, .. }
        | Access::Reconnect { login } => Some(login),
        Access::None => None,
    };
    let credential = format!("{}{CREDENTIAL} {ticket}", service.origin());
    Push::new(&repository, &summary.branch, login.as_deref(), credential).ok()
}
