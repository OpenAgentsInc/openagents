//! The public profile at `/u/{login}`: what a person made public by signing
//! in with GitHub, which is their login, display name, a link to their
//! GitHub profile, and the month they joined. Nothing private (email, runs,
//! balance, invite codes) is ever drawn. The private service also served a
//! stored avatar; this site's policy loads images from its own origin only,
//! and it stores none yet.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;

use crate::App;
use crate::backend::NOT_CONNECTED;
use crate::layout::{escape, page, problem, segment};

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

async fn profile(State(app): State<App>, Path(login): Path<String>) -> Response {
    if !is_login(&login) {
        return missing();
    }
    let backend = &app.config.backend;
    if !backend.connected() {
        return page(
            &format!("@{login}"),
            None,
            &format!(
                "<h1>@{}</h1><p class=\"notice\">{}</p>",
                escape(&login),
                escape(NOT_CONNECTED)
            ),
        );
    }
    let Some(profile) = backend.profile(&login).await else {
        return missing();
    };
    let name = profile.name.as_deref().unwrap_or(&profile.login);
    page(
        &format!("@{}", profile.login),
        None,
        &format!(
            "<section class=\"box\"><h1 class=\"box-title\">{}</h1><p class=\"loud\">@{}</p>\
<p class=\"meta\">Joined {}</p><p><a href=\"https://github.com/{}\">[ GitHub profile ]</a></p></section>",
            escape(name),
            escape(&profile.login),
            escape(&profile.joined),
            segment(&profile.login)
        ),
    )
}

fn missing() -> Response {
    problem(
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
