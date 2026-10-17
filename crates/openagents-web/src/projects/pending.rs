//! Invitations sent to a person: an owner invites by GitHub username or by
//! Nostr key (`npub1…`, #12003), and the invitation waits on that person's
//! Projects page with Join and Decline, no link to pass along. The
//! invitation itself is the usual single-use code (#11589); this keeps it,
//! for the person it names, in the chat store under their GitHub user id
//! (ids never change; a login can be renamed and taken) or their Nostr key,
//! until they join, decline, or it expires.

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

/// Who an invitation waits for: a GitHub user id, or a Nostr key as
/// lowercase hex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Invitee {
    Github(u64),
    Nostr(String),
}

impl Invitee {
    fn folder(&self) -> String {
        match self {
            Self::Github(id) => format!("project-invitations/github-{id}"),
            Self::Nostr(hex) => format!("project-invitations/nostr-{hex}"),
        }
    }

    /// Everyone the signed-in `viewer` is: their GitHub identity and their
    /// Nostr key, whichever they have.
    pub(crate) fn of_viewer(viewer: &crate::cloud::session::Viewer) -> Vec<Self> {
        let mut found = Vec::new();
        if let Some((github, _)) = &viewer.github {
            found.push(Self::Github(*github));
        }
        if let Some(hex) = &viewer.nostr {
            found.push(Self::Nostr(hex.clone()));
        }
        found
    }
}

/// The Nostr key `npub` names, as lowercase hex.
pub(crate) fn npub_hex(npub: &str) -> Option<String> {
    let key = nostr::nip19::decode_npub(npub.trim()).ok()?;
    Some(key.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// An npub shortened for a sentence: `npub1abcdefgh…uvwxyz`.
fn short_npub(npub: &str) -> String {
    let npub = npub.trim();
    if npub.len() <= 20 {
        return npub.to_owned();
    }
    format!("{}…{}", &npub[..12], &npub[npub.len() - 6..])
}

/// The person `text` names, a GitHub username or an `npub1…` key, and
/// how to name them to the owner; or what to tell the owner.
pub(crate) async fn person(
    app: &App,
    headers: &HeaderMap,
    text: &str,
) -> Result<(Invitee, String), String> {
    let text = text.trim();
    if text.starts_with("npub1") {
        return npub_hex(text)
            .map(|hex| (Invitee::Nostr(hex), short_npub(text)))
            .ok_or_else(|| "That npub isn't a valid Nostr key.".to_owned());
    }
    let (id, login) = github_person(app, headers, text).await?;
    Ok((Invitee::Github(id), login))
}

/// One invitation waiting for a person.
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

/// The invitation's name in the folder: part of its code's digest, so the
/// code itself never shows in a URL.
fn name_of(code: &str) -> String {
    let digest = Sha256::digest(code.as_bytes());
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn key(invitee: &Invitee, name: &str) -> String {
    format!("{}/{name}.json", invitee.folder())
}

/// Keep `pending` for `invitee`.
pub(crate) async fn record(
    store: &Store,
    invitee: &Invitee,
    pending: &Pending,
) -> Result<(), Error> {
    let bytes = serde_json::to_vec(pending)
        .map_err(|_| Error::Invalid("The invitation could not be saved."))?;
    let key = key(invitee, &name_of(&pending.code));
    let current = store
        .read_key(&key)
        .await?
        .map(|(_, generation)| generation);
    store.write_key(&key, bytes, current.as_deref()).await?;
    Ok(())
}

/// The invitations waiting for any of `invitees` (one person's GitHub
/// identity and Nostr key), soonest to expire first, with each one's name
/// for Decline. Drop expired invitations and invitations to projects the
/// person already belongs to, including through another invitation.
pub(crate) async fn waiting(
    store: &Store,
    invitees: &[Invitee],
    projects: &[oa_auth::repos::Project],
) -> Vec<(String, Pending)> {
    let mut keys = Vec::new();
    for invitee in invitees {
        if let Ok(found) = store.folder_keys_limit(&invitee.folder(), MOST * 4).await {
            keys.extend(found);
        }
    }
    let now = now_unix();
    let mut found: Vec<(String, Pending)> = Vec::new();
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
        let name = name_of(&pending.code);
        if found.iter().all(|(seen, _)| *seen != name) {
            found.push((name, pending));
        }
    }
    found.sort_by_key(|(_, pending)| pending.expires_unix);
    found.truncate(MOST);
    found
}

/// Stop listing the invitation named `name` for any of `invitees`.
async fn forget_named(store: &Store, invitees: &[Invitee], name: &str) {
    for invitee in invitees {
        let key = key(invitee, name);
        if let Ok(Some((_, generation))) = store.read_key(&key).await {
            store.delete_key(&key, &generation).await.ok();
        }
    }
}

/// Stop listing the invitation with `code` for any of `invitees` (it was
/// used).
pub(crate) async fn forget(store: &Store, invitees: &[Invitee], code: &str) {
    forget_named(store, invitees, &name_of(code)).await;
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
    forget_named(&app.config.chat_store, &Invitee::of_viewer(&viewer), &name).await;
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
    fn an_npub_names_a_nostr_invitee_apart_from_github() {
        let npub = nostr::nip19::encode_npub(&[0xab; 32]);
        assert_eq!(npub_hex(&npub), Some("ab".repeat(32)));
        assert_eq!(npub_hex("npub1nope"), None);
        let nostr = Invitee::Nostr("ab".repeat(32));
        assert_eq!(
            nostr.folder(),
            format!("project-invitations/nostr-{}", "ab".repeat(32))
        );
        assert_eq!(Invitee::Github(7).folder(), "project-invitations/github-7");
        let short = short_npub(&npub);
        assert!(short.starts_with("npub1") && short.contains('…') && short.len() < npub.len());
    }

    #[test]
    fn an_invitation_is_named_by_its_code_digest_not_the_code() {
        let name = name_of("prjinv_secret");
        assert!(valid_name(&name));
        assert!(!name.contains("secret"));
        assert_eq!(name, name_of("prjinv_secret"));
    }
}
