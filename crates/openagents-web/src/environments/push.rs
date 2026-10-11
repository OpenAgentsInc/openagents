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
//!
//! **Staging stand-in** ([`StandIn`]): until staging has a GitHub App
//! (#11226, an owner step), a server started with
//! `OPENAGENTS_WEB_STAGING_PUSH_TOKEN` and `OPENAGENTS_WEB_STAGING_PUSH_REPOS`
//! gives a run on one of those named repositories a ticket of its own
//! (`ogs_…`, kept here as a digest, one repository, the run's lifetime),
//! which [`CREDENTIAL`] trades for that token. It is used only when the
//! person's own connection gave nothing, and only staging sets it
//! (`deploy/staging/render.py`); production never does.

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
    let stand_in = app
        .config
        .staging_push
        .as_deref()
        .and_then(|stand_in| stand_in.answer(&form));
    let (status, text) = match (stand_in, app.config.cloud.as_deref()) {
        (Some(answer), _) => answer,
        (None, Some(service)) => service.git_credential(&form).await,
        (None, None) => (404, "error=unavailable\n".into()),
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
    let own = own_access(service, headers, &summary).await;
    if own.is_some() {
        return own;
    }
    let stand_in = app.config.staging_push.as_deref()?;
    let ticket = stand_in.ticket(&summary.repository)?;
    let credential = format!("{}{CREDENTIAL} {ticket}", service.origin());
    Push::new(
        &summary.repository,
        &summary.branch,
        Some(STAND_IN_LOGIN),
        credential,
    )
    .ok()
}

/// The person's own GitHub connection's access, through the broker.
async fn own_access(
    service: &crate::cloud::session::CloudSession,
    headers: &HeaderMap,
    summary: &coder_environment_operator::studio::Summary,
) -> Option<Push> {
    let repository = summary.repository.clone();
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

/// What starts every stand-in ticket.
const STAND_IN_PREFIX: &str = "ogs_";
/// The commit name a stand-in push uses when Git has none.
const STAND_IN_LOGIN: &str = "openagents-staging-test";
/// The most stand-in tickets kept at once (the oldest go).
const STAND_IN_TICKETS: usize = 64;

/// Staging's stand-in for a GitHub App (see the module docs): a token
/// for the named repositories only, handed out through one-repository
/// tickets. The token never prints.
pub struct StandIn {
    token: String,
    repositories: Vec<String>,
    /// SHA-256 of each ticket, hex, with its repository and expiry.
    tickets: std::sync::Mutex<Vec<(String, String, u64)>>,
}

impl std::fmt::Debug for StandIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StandIn({:?}, redacted)", self.repositories)
    }
}

impl StandIn {
    /// From the token and a comma-separated `owner/name` list; `None`
    /// when either is empty.
    #[must_use]
    pub fn new(token: &str, repositories: &str) -> Option<Self> {
        let token = token.trim();
        let repositories: Vec<String> = repositories
            .split(',')
            .map(str::trim)
            .filter(|repo| repo.matches('/').count() == 1 && !repo.starts_with('/'))
            .map(str::to_ascii_lowercase)
            .collect();
        (!token.is_empty() && !token.contains(char::is_whitespace) && !repositories.is_empty())
            .then(|| Self {
                token: token.to_owned(),
                repositories,
                tickets: std::sync::Mutex::new(Vec::new()),
            })
    }

    /// A ticket for `repository`, when it is one of the named ones.
    fn ticket(&self, repository: &str) -> Option<String> {
        let repository = repository.to_ascii_lowercase();
        if !self.repositories.contains(&repository) {
            return None;
        }
        let ticket = format!("{STAND_IN_PREFIX}{}", crate::cloud::byo::fresh_request());
        let expires = now() + TICKET_SECONDS;
        let mut tickets = self.tickets.lock().ok()?;
        let at = now();
        tickets.retain(|(_, _, until)| *until > at);
        if tickets.len() >= STAND_IN_TICKETS {
            tickets.remove(0);
        }
        tickets.push((digest(&ticket), repository, expires));
        Some(ticket)
    }

    /// The answer to a helper's form when it holds a stand-in ticket:
    /// the token for that ticket's repository, or a refusal. `None` for
    /// any other ticket, which goes to the broker.
    fn answer(&self, form: &[u8]) -> Option<(u16, String)> {
        let fields: Vec<(String, String)> =
            url::form_urlencoded::parse(form).into_owned().collect();
        let field = |name: &str| {
            fields
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        let ticket = field("ticket")?;
        if !ticket.starts_with(STAND_IN_PREFIX) {
            return None;
        }
        let refused = Some((403, "error=refused\n".to_owned()));
        let path = field("path").unwrap_or_default();
        let path = path
            .strip_suffix(".git")
            .unwrap_or(path)
            .to_ascii_lowercase();
        if field("protocol") != Some("https") || field("host") != Some("github.com") {
            return refused;
        }
        let digest = digest(ticket);
        let at = now();
        let known = self.tickets.lock().ok().is_some_and(|tickets| {
            tickets
                .iter()
                .any(|(kept, repo, until)| *kept == digest && *repo == path && *until > at)
        });
        if !known {
            return refused;
        }
        Some((
            200,
            format!("username=x-access-token\npassword={}\n", self.token),
        ))
    }
}

fn digest(ticket: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(ticket.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod stand_in_tests {
    use super::*;

    fn form(ticket: &str, path: &str) -> Vec<u8> {
        format!("ticket={ticket}&protocol=https&host=github.com&path={path}").into_bytes()
    }

    #[test]
    fn a_ticket_answers_only_for_its_own_repository() {
        let stand_in = StandIn::new("tok123", "Owner/Repo, other/thing").unwrap();
        assert!(stand_in.ticket("someone/else").is_none());
        let ticket = stand_in.ticket("owner/repo").unwrap();
        assert!(ticket.starts_with("ogs_"));
        let (status, text) = stand_in.answer(&form(&ticket, "Owner/Repo.git")).unwrap();
        assert_eq!(status, 200);
        assert_eq!(text, "username=x-access-token\npassword=tok123\n");
        assert_eq!(
            stand_in
                .answer(&form(&ticket, "other/thing.git"))
                .unwrap()
                .0,
            403
        );
        assert_eq!(
            stand_in
                .answer(&form("ogs_unknown", "owner/repo.git"))
                .unwrap()
                .0,
            403
        );
        let http = format!("ticket={ticket}&protocol=http&host=github.com&path=owner/repo.git");
        assert_eq!(stand_in.answer(http.as_bytes()).unwrap().0, 403);
    }

    #[test]
    fn other_tickets_go_to_the_broker_and_nothing_prints_the_token() {
        let stand_in = StandIn::new("tok123", "owner/repo").unwrap();
        assert!(
            stand_in
                .answer(&form("ogb_abc_def_ghi", "owner/repo.git"))
                .is_none()
        );
        assert!(!format!("{stand_in:?}").contains("tok123"));
        assert!(StandIn::new("", "owner/repo").is_none());
        assert!(StandIn::new("tok", " , nope").is_none());
    }
}
