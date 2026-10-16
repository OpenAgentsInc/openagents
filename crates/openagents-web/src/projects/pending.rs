//! Invitations sent to a GitHub person: an owner invites by GitHub
//! username, and the invitation waits on that person's Projects page with
//! Join and Decline, no link to pass along. The invitation itself is the
//! usual single-use code (#11589); this keeps it, for the person it names,
//! in the chat store under their GitHub user id (ids never change; a login
//! can be renamed and taken), until they join, decline, or it expires.

use axum::Router;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::post;
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonLink, ButtonType, ButtonVariant, Color, ControlSize};
use openagents_ui::content::MarkdownRoot;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{PAGE, checked};
use crate::App;
use crate::chat_store::{Error, Store, now_unix};
use crate::cloud::protect;

/// The most invitations one person's page lists.
const MOST: usize = 50;

pub(super) fn routes() -> Router<App> {
    Router::new().route("/projects/invitations/{key}/decline", post(decline))
}

/// One invitation waiting for a GitHub person.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct Pending {
    /// The single-use code Join spends.
    pub(crate) code: String,
    pub(crate) project: String,
    pub(crate) project_name: String,
    /// Who invited, as their account is named.
    pub(crate) from: String,
    pub(crate) role: String,
    pub(crate) expires_unix: u64,
}

fn folder(github: u64) -> String {
    format!("project-invitations/github-{github}")
}

/// The invitation's name in the folder: part of its code's digest, so the
/// code itself never shows in a URL.
fn name_of(code: &str) -> String {
    let digest = Sha256::digest(code.as_bytes());
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn key(github: u64, name: &str) -> String {
    format!("{}/{name}.json", folder(github))
}

/// Keep `pending` for the GitHub person `github`.
pub(crate) async fn record(store: &Store, github: u64, pending: &Pending) -> Result<(), Error> {
    let bytes = serde_json::to_vec(pending)
        .map_err(|_| Error::Invalid("The invitation could not be saved."))?;
    let key = key(github, &name_of(&pending.code));
    let current = store
        .read_key(&key)
        .await?
        .map(|(_, generation)| generation);
    store.write_key(&key, bytes, current.as_deref()).await?;
    Ok(())
}

/// The invitations waiting for `github`, soonest to expire first, with
/// each one's name for Decline. Drop expired invitations and invitations to
/// projects the person already belongs to, including through another invitation.
pub(crate) async fn waiting(
    store: &Store,
    github: u64,
    projects: &[oa_auth::repos::Project],
) -> Vec<(String, Pending)> {
    let Ok(keys) = store.folder_keys_limit(&folder(github), MOST * 4).await else {
        return Vec::new();
    };
    let now = now_unix();
    let mut found = Vec::new();
    for key in keys {
        let Ok(Some((bytes, generation))) = store.read_key(&key).await else {
            continue;
        };
        let Ok(pending) = serde_json::from_slice::<Pending>(&bytes) else {
            continue;
        };
        if pending.expires_unix <= now
            || projects.iter().any(|project| project.id == pending.project)
        {
            store.delete_key(&key, &generation).await.ok();
            continue;
        }
        found.push((name_of(&pending.code), pending));
    }
    found.sort_by_key(|(_, pending)| pending.expires_unix);
    found.truncate(MOST);
    found
}

/// Stop listing the invitation named `name` for `github`.
async fn forget_named(store: &Store, github: u64, name: &str) {
    let key = key(github, name);
    if let Ok(Some((_, generation))) = store.read_key(&key).await {
        store.delete_key(&key, &generation).await.ok();
    }
}

/// Stop listing the invitation with `code` for `github` (it was used).
pub(crate) async fn forget(store: &Store, github: u64, code: &str) {
    forget_named(store, github, &name_of(code)).await;
}

fn valid_name(name: &str) -> bool {
    name.len() == 32 && name.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// A GitHub login as GitHub allows them: letters, digits and single
/// hyphens, at most 39, not starting or ending with a hyphen.
pub(crate) fn valid_login(login: &str) -> bool {
    !login.is_empty()
        && login.len() <= 39
        && !login.starts_with('-')
        && !login.ends_with('-')
        && !login.contains("--")
        && login
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// The GitHub person named `login` (`@` and spaces trimmed): their id and
/// login as GitHub spells it, or what to tell the owner.
pub(crate) async fn github_person(
    app: &App,
    headers: &HeaderMap,
    login: &str,
) -> Result<(u64, String), String> {
    let login = login.trim().trim_start_matches('@');
    if !valid_login(login) {
        return Err(format!("“{login}” isn't a GitHub username."));
    }
    let base = app.config.github.as_ref().map_or_else(
        || "https://api.github.com".to_string(),
        |github| github.endpoints.api_url.trim_end_matches('/').to_string(),
    );
    let token = match app.config.cloud.as_deref() {
        Some(service) => service.github_token(headers).await.ok(),
        None => None,
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| "We couldn't reach GitHub. Try again.".to_owned())?;
    let mut request = client
        .get(format!("{base}/users/{login}"))
        .header("User-Agent", "openagents-web")
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .await
        .map_err(|_| "We couldn't reach GitHub. Try again.".to_owned())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(format!("There's no GitHub account named {login}."));
    }
    #[derive(Deserialize)]
    struct User {
        id: u64,
        login: String,
    }
    let user: User = match response.error_for_status() {
        Ok(response) => response
            .json()
            .await
            .map_err(|_| "GitHub didn't answer clearly. Try again.".to_owned())?,
        Err(_) => return Err("GitHub didn't answer. Try again in a minute.".to_owned()),
    };
    Ok((user.id, user.login))
}

fn a_role(role: &str) -> &'static str {
    if role == "viewer" {
        "a viewer"
    } else {
        "an editor"
    }
}

/// "Invitations" on the Projects page: each project someone invited you
/// to, with Join (the invitation's own page) and Decline.
pub(crate) fn section(waiting: &[(String, Pending)], csrf: &str) -> Markup {
    html! {
        @if !waiting.is_empty() {
            section aria-labelledby="projects-invited" {
                (MarkdownRoot::new(html! { h2 #projects-invited { "Invitations" } }))
                ul.oa-chat-archive-list role="list" {
                    @for (name, pending) in waiting {
                        li.oa-chat-archive-row {
                            span {
                                strong { (pending.project_name) }
                                " · " (pending.from) " invited you as " (a_role(&pending.role))
                            }
                            span.oa-page-actions {
                                (ButtonLink::new("Join", format!("{}/{}", super::members::JOIN, pending.code))
                                    .size(ControlSize::Sm))
                                form method="post" action=(format!("/projects/invitations/{name}/decline")) {
                                    input type="hidden" name="csrf" value=(csrf);
                                    (Button::new("Decline")
                                        .kind(ButtonType::Submit)
                                        .size(ControlSize::Sm)
                                        .variant(ButtonVariant::Ghost)
                                        .color(Color::Secondary))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct CsrfForm {
    csrf: String,
}

async fn decline(
    State(app): State<App>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Response {
    let (_, viewer) = match checked(&app, &headers, &form.csrf).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if !valid_name(&name) {
        return protect(StatusCode::NOT_FOUND.into_response());
    }
    if let Some((github, _)) = viewer.github {
        forget_named(&app.config.chat_store, github, &name).await;
    }
    protect(Redirect::to(PAGE).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_logins_are_checked_like_github_does() {
        for good in ["kikidesigns", "octo-local", "a", "A1-b2"] {
            assert!(valid_login(good), "{good}");
        }
        for bad in ["", "-a", "a-", "a--b", "a b", "a/b", "a.b", &"x".repeat(40)] {
            assert!(!valid_login(bad), "{bad}");
        }
    }

    #[test]
    fn an_invitation_is_named_by_its_code_digest_not_the_code() {
        let name = name_of("prjinv_secret");
        assert!(valid_name(&name));
        assert!(!name.contains("secret"));
        assert_eq!(name, name_of("prjinv_secret"));
    }
}
