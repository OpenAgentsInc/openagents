//! The consent page: shown before the cookie is set to a visitor the law
//! asks us to ask (`Attribution::needs_consent`). Two buttons post the
//! choice back to the link's own address; either way the visitor goes on
//! to the page the link points to. No script.

use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use maud::html;
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color, ControlSize};

use super::Target;
use crate::account::Account;
use crate::cloud::protect;
use crate::ui_page::{UiPage, auth_card};

/// Where the visitor came from and why we'd remember it, in plain words.
fn question(target: &Target) -> String {
    let channel = target
        .code
        .as_deref()
        .and_then(super::codes::find)
        .map(super::codes::Code::channel);
    let from = match channel {
        Some("x") => "our post on X",
        Some("yt") => "our YouTube video",
        Some("tt") => "our TikTok video",
        Some("ig") => "our Instagram post",
        Some("li") => "our LinkedIn post",
        Some(_) => "one of our posts",
        None => {
            return "You came here through a link from someone who recommends us. \
                    May we remember that for 30 days, so we can credit them if you sign up?"
                .into();
        }
    };
    format!(
        "You came here from {from}. May we remember that for 30 days, so we know which posts bring people who sign up?"
    )
}

pub(super) fn page(headers: &HeaderMap, target: &Target) -> Response {
    let action = target.path();
    let question = question(target);
    let card = auth_card(
        "Before you go on",
        Some(html! { (question) }),
        html! {
            form.oa-auth-login-form method="post" action=(action) {
                (Button::new("Yes, remember it")
                    .kind(ButtonType::Submit)
                    .name("choice")
                    .value("allow")
                    .size(ControlSize::Lg)
                    .block(true))
                (Button::new("No thanks")
                    .kind(ButtonType::Submit)
                    .name("choice")
                    .value("decline")
                    .variant(ButtonVariant::Soft)
                    .color(Color::Secondary)
                    .size(ControlSize::Lg)
                    .block(true))
            }
            p.oa-auth-switch {
                "We keep one small cookie with the link and the time, nothing about you. "
                a href="/privacy" { "How we use it" }
            }
        },
    );
    let mut response = protect(
        UiPage::new("Before you go on")
            .account(Account::Unknown)
            .scriptless()
            .bare()
            .status(StatusCode::OK)
            .content(card)
            .respond(headers),
    );
    response.headers_mut().insert(
        "x-robots-tag",
        axum::http::HeaderValue::from_static("noindex, nofollow"),
    );
    response
}
