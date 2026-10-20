//! The public profile at `/u/{login}`: what a person made public by signing
//! in with GitHub, which is their login, display name, a link to their
//! GitHub profile, and the month they joined. When that account has a
//! `name@openagents.com` name, the page moves to `/@{name}` (#12254). Nothing private (email, runs,
//! balance, invite codes) is ever drawn. The private service also served a
//! stored avatar; this site's policy loads images from its own origin only,
//! and it stores none yet.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
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
    if let Some(service) = app.config.cloud.as_ref()
        && let Ok(answer) = service
            .nostr_call(
                reqwest::Method::GET,
                &format!("/v1/names?github={login}"),
                None,
                None,
            )
            .await
    {
        if answer.status == 404 {
            return missing(&headers);
        }
        if answer.status == 200 {
            if let Some(name) = answer.body["name"].as_str() {
                return axum::response::Redirect::permanent(&format!("/@{name}")).into_response();
            }
            let github = &answer.body["github"];
            let profile = crate::backend::Profile {
                login: github["login"].as_str().unwrap_or(&login).to_string(),
                name: github["name"].as_str().map(str::to_string),
                joined: joined(github["joined"].as_str().unwrap_or_default()),
            };
            return profile_page(&headers, path, &profile);
        }
    }
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
    profile_page(&headers, path, &profile)
}

/// A person's profile page.
pub fn profile_page(
    headers: &HeaderMap,
    path: String,
    profile: &crate::backend::Profile,
) -> Response {
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
        .respond(headers)
}

/// "September 2026" from an RFC 3339 time.
fn joined(stamp: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let year = stamp
        .get(..4)
        .filter(|y| y.bytes().all(|b| b.is_ascii_digit()));
    let month = stamp
        .get(5..7)
        .and_then(|m| m.parse::<usize>().ok())
        .and_then(|m| MONTHS.get(m.wrapping_sub(1)));
    match (year, month) {
        (Some(year), Some(month)) => format!("{month} {year}"),
        _ => "a while ago".to_string(),
    }
}

pub fn missing(headers: &HeaderMap) -> Response {
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
    fn joined_reads_the_month() {
        assert_eq!(joined("2026-09-03T10:00:00Z"), "September 2026");
        assert_eq!(joined("nonsense"), "a while ago");
    }

    #[test]
    fn only_github_logins_pass() {
        assert!(is_login("AtlantisPleb"));
        assert!(is_login("a-b"));
        for bad in ["", "-a", "a-", "a--b", "a b", "<x>", &"a".repeat(40)] {
            assert!(!is_login(bad), "{bad}");
        }
    }
}
