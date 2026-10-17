//! `/settings/link`: a signed-in person gets a link they can share.
//!
//! The page uses the account service's create and link handlers, then, when
//! they ask for one, a short name beside the long link. It does not mention
//! the public terms page.

use axum::extract::{Form, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::get;
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonType};
use openagents_ui::forms::{Field, Input};
use serde::Deserialize;
use serde_json::json;

use super::{page, viewer};
use crate::App;
use crate::cloud::refused;
use crate::cloud::session::SessionError;
use crate::ui_page::action_link;

pub(crate) const PAGE: &str = "/settings/link";
const SCOPE: &str = "referral-link";

pub(crate) fn route() -> axum::routing::MethodRouter<App> {
    get(show).post(create)
}

/// The Settings row.
pub(crate) fn row() -> Markup {
    html! {
        div class="oa-settings-row" {
            div class="oa-settings-text" {
                span class="oa-settings-label" { "Your link" }
                span class="oa-settings-hint" { "Get a link you can share." }
            }
            div class="oa-settings-control" { (action_link("Get your link", PAGE)) }
        }
    }
}

#[derive(Deserialize, Default)]
struct LinkForm {
    #[serde(default)]
    label: String,
    #[serde(default)]
    handle: String,
    #[serde(default)]
    csrf: String,
}

async fn show(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Ok(csrf) = service.csrf(&headers, &viewer, SCOPE, PAGE) else {
        return refused(SessionError::Unavailable);
    };
    page(
        &headers,
        service,
        &viewer,
        "Your link",
        PAGE,
        form(&csrf, None),
    )
}

async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form_input): Form<LinkForm>,
) -> Response {
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if service
        .verify_csrf(&headers, Some(&viewer), SCOPE, PAGE, &form_input.csrf)
        .is_err()
    {
        return again(
            &headers,
            service,
            &viewer,
            StatusCode::BAD_REQUEST,
            "That page expired. Reload it and try again.",
        );
    }
    let handle = form_input.handle.trim();
    if !handle.is_empty() && !tenancy::accounts::referrals::handle_word(handle) {
        return again(
            &headers,
            service,
            &viewer,
            StatusCode::BAD_REQUEST,
            "Use letters, numbers, and hyphens for the short name.",
        );
    }
    let label = chosen_label(&form_input.label, &viewer.account_label);
    let Some(label) = label else {
        return again(
            &headers,
            service,
            &viewer,
            StatusCode::BAD_REQUEST,
            "Use a shorter name.",
        );
    };
    let created = match service
        .account_json(
            reqwest::Method::POST,
            "/v1/account/referrers",
            &headers,
            Some(json!({"kind":"person","label":label})),
        )
        .await
    {
        Ok(value) => value,
        Err(error) => return again(&headers, service, &viewer, status_of(error), words(error)),
    };
    let Some(id) = created["referral"]["id"].as_str().filter(|id| {
        id.starts_with("ref_")
            && id.len() <= 64
            && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    }) else {
        return again(
            &headers,
            service,
            &viewer,
            StatusCode::SERVICE_UNAVAILABLE,
            "We couldn't make a link just now. Try again later.",
        );
    };
    let issued = match service
        .account_json(
            reqwest::Method::POST,
            &format!("/v1/account/referrers/{id}/link"),
            &headers,
            None,
        )
        .await
    {
        Ok(value) => value,
        Err(error) => return again(&headers, service, &viewer, status_of(error), words(error)),
    };
    let Some(token) = issued["referral"]["token"]
        .as_str()
        .filter(|token| crate::attribution::affiliate_token(token))
    else {
        return again(
            &headers,
            service,
            &viewer,
            StatusCode::SERVICE_UNAVAILABLE,
            "We couldn't make a link just now. Try again later.",
        );
    };
    let mut short = None;
    let mut short_problem = None;
    if !handle.is_empty() {
        match service
            .account_json(
                reqwest::Method::POST,
                &format!("/v1/account/referrers/{id}/handle"),
                &headers,
                Some(json!({"handle": handle})),
            )
            .await
        {
            Ok(value) => {
                if value["referral"]["path"].as_str() == Some(&format!("/r/{handle}")) {
                    short = Some(handle.to_owned());
                }
            }
            Err(SessionError::Conflict) => short_problem = Some("That short name is taken."),
            Err(_) => {
                short_problem =
                    Some("The short name couldn't be saved. Your long link still works.")
            }
        }
    }
    page(
        &headers,
        service,
        &viewer,
        "Your link",
        PAGE,
        ready(token, short.as_deref(), short_problem),
    )
}

fn chosen_label(typed: &str, account: &str) -> Option<String> {
    let typed = typed.trim();
    let label = if typed.is_empty() {
        account.trim()
    } else {
        typed
    };
    let label = if label.is_empty() { "My link" } else { label };
    (label.len() <= 128 && !label.chars().any(char::is_control)).then(|| label.to_owned())
}

fn status_of(error: SessionError) -> StatusCode {
    match error {
        SessionError::Unauthenticated => StatusCode::UNAUTHORIZED,
        SessionError::Forbidden => StatusCode::FORBIDDEN,
        SessionError::InvalidRequest => StatusCode::BAD_REQUEST,
        SessionError::Conflict => StatusCode::CONFLICT,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

fn words(error: SessionError) -> &'static str {
    match error {
        SessionError::Unauthenticated => "Sign in to get a link.",
        _ => "We couldn't make a link just now. Try again later.",
    }
}

fn again(
    headers: &HeaderMap,
    service: &crate::cloud::session::CloudSession,
    viewer: &crate::cloud::session::Viewer,
    status: StatusCode,
    problem: &str,
) -> Response {
    let csrf = service
        .csrf(headers, viewer, SCOPE, PAGE)
        .unwrap_or_default();
    let mut response = page(
        headers,
        service,
        viewer,
        "Your link",
        PAGE,
        form(&csrf, Some(problem)),
    );
    *response.status_mut() = status;
    response
}

fn form(csrf: &str, problem: Option<&str>) -> Markup {
    html! {
        h1 { "Your link" }
        p { "Get a link you can share. Save the long one when it appears. We show it only this once." }
        @if let Some(problem) = problem {
            p { (problem) }
        }
        form method="post" action=(PAGE) {
            input type="hidden" name="csrf" value=(csrf);
            (Field::new("link-label", "Name").optional(true).control(
                Input::new("label").id("link-label").maxlength(128).autocomplete("off")
            ))
            (Field::new("link-handle", "Short name").optional(true).control(
                Input::new("handle").id("link-handle").maxlength(48).autocomplete("off")
                    .placeholder("letters, numbers, and hyphens")
            ))
            (Button::new("Get your link").kind(ButtonType::Submit))
        }
    }
}

fn ready(token: &str, short: Option<&str>, short_problem: Option<&str>) -> Markup {
    html! {
        h1 { "Your link" }
        p { "Save this link. We show the long one only this once." }
        p { "openagents.com/r/" (token) }
        @if let Some(short) = short {
            p { "Your short link is openagents.com/r/" (short) "." }
        }
        @if let Some(problem) = short_problem {
            p { (problem) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_link_page_is_plain_and_does_not_point_at_the_terms() {
        let html = html! { (row()) (form("csrf-value", None)) (ready("rfr_00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff", Some("ada"), None)) }.into_string();
        crate::copy_guard::assert_plain(PAGE, &html);
        assert!(html.contains("Get a link you can share."));
        assert!(html.contains("Save this link."));
        assert!(html.contains("openagents.com/r/ada"));
        assert!(!html.contains("/affiliates"));
        assert!(!html.contains("commission"));
    }
}
