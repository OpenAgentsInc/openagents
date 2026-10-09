//! The public profile at `/u/{login}`: what a person made public by signing
//! in with GitHub, which is their login, display name, a link to their
//! GitHub profile, and the month they joined. Nothing private (email, runs,
//! balance, invite codes) is ever drawn. The private service also served a
//! stored avatar; this site's policy loads images from its own origin only,
//! and it stores none yet.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::get;
use maud::html;
use openagents_ui::actions::Alert;
use openagents_ui::content::{MarkdownRoot, PageColumn};

use crate::App;
use crate::backend::NOT_CONNECTED;
use crate::layout::segment;
use crate::ui_page::{UiPage, action_link, problem};

/// A GitHub login: 1 to 39 letters, digits, and single inner hyphens.
fn is_login(login: &str) -> bool {
    (1..=39).contains(&login.len())
        && login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !login.starts_with('-')
        && !login.ends_with('-')
        && !login.contains("--")
}

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/u/{login}", get(profile))
}

async fn profile(
    State(app): State<App>,
    Path(login): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !is_login(&login) {
        return missing(&headers);
    }
    let path = format!("/u/{}", segment(&login));
    let backend = &app.config.backend;
    if !backend.connected() {
        return UiPage::new(format!("@{login}"))
            .path(path)
            .scriptless()
            .content(PageColumn::new(html! {
                (MarkdownRoot::new(html! { h1 { "@" (login) } }))
                (Alert::new().description(NOT_CONNECTED))
            }))
            .respond(&headers);
    }
    let Some(profile) = backend.profile(&login).await else {
        return missing(&headers);
    };
    let name = profile.name.as_deref().unwrap_or(&profile.login);
    let github = format!("https://github.com/{}", segment(&profile.login));
    UiPage::new(format!("@{}", profile.login))
        .path(path)
        .scriptless()
        .content(PageColumn::new(html! {
            section.oa-card aria-labelledby="profile-name" {
                (MarkdownRoot::new(html! {
                    h1 id="profile-name" { (name) }
                    p { "@" (profile.login) }
                    p.oa-page-meta { "Joined " (profile.joined) }
                }))
                div.oa-page-actions { (action_link("GitHub profile", &github)) }
            }
        }))
        .respond(&headers)
}

fn missing(headers: &HeaderMap) -> Response {
    problem(
        headers,
        StatusCode::NOT_FOUND,
        "Profile not found",
        "No account has that login.",
        ("/", "Home"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_github_logins_pass() {
        assert!(is_login("AtlantisPleb"));
        assert!(is_login("a-b"));
        for bad in ["", "-a", "a-", "a--b", "a b", "<x>", &"a".repeat(40)] {
            assert!(!is_login(bad), "{bad}");
        }
    }
}
