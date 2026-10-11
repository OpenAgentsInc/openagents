//! The sign-in and entry screens: home, sign-in, the device code page,
//! the connect pages, and the app shell itself, over fixture data.

use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use maud::html;
use openagents_ui::content::PageColumn;

use super::{Group, Screen, state};
use crate::account::Account;
use crate::ui_page::UiPage;

/// The sample person every signed-in state shows.
pub(crate) const NAME: &str = "Christopher D.";
/// The sample computer.
pub(crate) const COMPUTER: &str = "Christopher's MacBook Pro";
const CSRF: &str = "sample-ticket";

/// A signed-in account with a profile picture.
pub(crate) fn signed_in() -> Account {
    Account::SignedIn {
        name: NAME.to_owned(),
        sign_out: Some(CSRF.to_owned()),
        picture: true,
        admin: false,
    }
}

/// A signed-in site admin (the account menu adds Analytics).
pub(crate) fn admin() -> Account {
    Account::SignedIn {
        name: NAME.to_owned(),
        sign_out: Some(CSRF.to_owned()),
        picture: false,
        admin: true,
    }
}

pub(crate) fn screens() -> Vec<Screen> {
    vec![
        Screen {
            id: "app-shell",
            title: "App shell",
            group: Group::Organism,
            routes: &["every page"],
            states: vec![
                state("signed-out", "Signed out", shell_signed_out),
                state("signed-in", "Signed in", shell_signed_in),
                state("admin", "Signed in as an admin", shell_admin),
                state("unknown", "No sign-in on this server", shell_unknown),
            ],
        },
        Screen {
            id: "home",
            title: "Home (new chat)",
            group: Group::Page,
            routes: &["/"],
            states: vec![
                state("signed-out", "Signed out", home_signed_out),
                state("signed-in", "Signed in", home_signed_in),
            ],
        },
        Screen {
            id: "sign-in",
            title: "Sign in",
            group: Group::Page,
            routes: &["/login", "/signup", "/auth/github/callback"],
            states: vec![
                state("login", "Log in", login),
                state("signup", "Sign up", signup),
                state("invite-only", "Invite-only", invite_only),
                state("try-again", "GitHub sign-in failed", try_again),
                state(
                    "unavailable",
                    "No sign-in on this server",
                    sign_in_unavailable,
                ),
                state("signed-in", "Signed in", signed_in_notice),
            ],
        },
        Screen {
            id: "device",
            title: "Sign in on another computer",
            group: Group::Page,
            routes: &["/device"],
            states: vec![
                state("code", "Enter a code", device_code),
                state("typed", "Code typed", device_typed),
                state("wrong", "Wrong code", device_wrong),
                state("expired", "Expired code", device_expired),
                state("approve", "Approve a sign-in", device_approve),
                state("approved", "Approved", device_approved),
                state("denied", "Denied", device_denied),
            ],
        },
        Screen {
            id: "connect",
            title: "Connect a phone or terminal",
            group: Group::Page,
            routes: &["/connect", "/settings/terminal"],
            states: vec![
                state("phone", "Phone without the app", connect_phone),
                state("terminal", "Connect your terminal", connect_terminal),
            ],
        },
        Screen {
            id: "app-consent",
            title: "Let an app use your account",
            group: Group::Page,
            routes: &["/oauth/authorize"],
            states: vec![
                state("on-this-computer", "An app on this computer", consent_local),
                state("on-a-site", "A website", consent_site),
                state("bad-link", "Sign-in link not valid", consent_bad_link),
                state("too-slow", "Sign-in took too long", consent_too_slow),
            ],
        },
    ]
}

fn consent(headers: &HeaderMap, name: &str, redirect_uri: &str) -> Response {
    let request = crate::oauth::Authorization {
        client_id: "sample-app-id".to_owned(),
        client: crate::oauth::Client {
            name: name.to_owned(),
            redirect_uris: vec![redirect_uri.to_owned()],
            issued_at: 1_791_547_200,
        },
        redirect_uri: redirect_uri.to_owned(),
        redirect_given: true,
        challenge: "sample-challenge".to_owned(),
        state: Some("sample-state".to_owned()),
    };
    let body = crate::oauth::consent_form(&request, "sample-request", CSRF);
    crate::oauth::consent_page_as(headers, signed_in(), &request, &body)
}

fn consent_local(headers: &HeaderMap) -> Response {
    consent(headers, "Sample Editor", "http://127.0.0.1:43123/callback")
}

fn consent_site(headers: &HeaderMap) -> Response {
    consent(
        headers,
        "Sample Notes",
        "https://notes.example.com/callback",
    )
}

/// What `crate::cloud::failure` answers, in the request's theme.
fn failure(headers: &HeaderMap, title: &str, text: &str) -> Response {
    crate::ui_page::problem(
        headers,
        StatusCode::BAD_REQUEST,
        title,
        text,
        ("/", "Back to chat"),
    )
}

fn consent_bad_link(headers: &HeaderMap) -> Response {
    failure(headers, "Sign-in link not valid", crate::oauth::BAD_LINK)
}

fn consent_too_slow(headers: &HeaderMap) -> Response {
    failure(headers, "Sign-in took too long", "Start again in the app.")
}

fn shell(headers: &HeaderMap, account: Account) -> Response {
    UiPage::new("Sample page")
        .path("/ui/screens/app-shell")
        .account(account)
        .content(PageColumn::new(html! {
            h1 { "A page in the shell" }
            p { "The left panel, the header with its actions, and the content column." }
        }))
        .respond(headers)
}

fn shell_signed_out(headers: &HeaderMap) -> Response {
    shell(headers, Account::SignedOut)
}

fn shell_signed_in(headers: &HeaderMap) -> Response {
    shell(headers, signed_in())
}

fn shell_admin(headers: &HeaderMap) -> Response {
    shell(headers, admin())
}

fn shell_unknown(headers: &HeaderMap) -> Response {
    shell(headers, Account::Unknown)
}

fn home(headers: &HeaderMap, account: Account) -> Response {
    let content = html! {
        div.oa-thread-view {
            div.oa-thread {
                div.oa-thread-column.oa-home-stage { (crate::pages::home::learn_cards()) }
            }
        }
    };
    let dock = html! {
        (crate::pages::chat::composer_with("/chat", "Start a chat", None, html! {}, None))
        (crate::ui_page::legal_links())
    };
    crate::chat_html::protect(
        UiPage::new("OpenAgents")
            .section("/")
            .path("/")
            .app()
            .head(crate::chat_html::head())
            .account(account)
            .content(content)
            .composer(dock)
            .respond(headers),
    )
}

fn home_signed_out(headers: &HeaderMap) -> Response {
    home(headers, Account::SignedOut)
}

fn home_signed_in(headers: &HeaderMap) -> Response {
    home(headers, signed_in())
}

fn login(headers: &HeaderMap) -> Response {
    crate::auth::sign_in_page(headers, false, false, "/")
}

fn signup(headers: &HeaderMap) -> Response {
    crate::auth::sign_in_page(headers, true, false, "/")
}

fn invite_only(headers: &HeaderMap) -> Response {
    crate::auth::sign_in_page(headers, false, true, "/")
}

fn try_again(headers: &HeaderMap) -> Response {
    crate::auth::try_again(
        headers,
        StatusCode::BAD_GATEWAY,
        "GitHub didn't sign you in",
        "GitHub didn't answer. Try again in a moment.",
    )
}

fn sign_in_unavailable(headers: &HeaderMap) -> Response {
    crate::auth::notice(
        headers,
        StatusCode::SERVICE_UNAVAILABLE,
        "Sign-in isn't available",
        "This server doesn't sign people in.",
    )
}

fn signed_in_notice(headers: &HeaderMap) -> Response {
    crate::auth::notice(
        headers,
        StatusCode::OK,
        "Signed in",
        "You're signed in to OpenAgents. Close this tab and go back to your computer.",
    )
}

fn device(headers: &HeaderMap, status: StatusCode, title: &str, body: maud::Markup) -> Response {
    UiPage::new(title)
        .path("/device")
        .status(status)
        .account(signed_in())
        .content(PageColumn::new(body))
        .respond(headers)
}

const DEVICE_TITLE: &str = "Sign in on another computer";

fn device_code(headers: &HeaderMap) -> Response {
    device(
        headers,
        StatusCode::OK,
        DEVICE_TITLE,
        crate::device::code_form("", None),
    )
}

fn device_typed(headers: &HeaderMap) -> Response {
    device(
        headers,
        StatusCode::OK,
        DEVICE_TITLE,
        crate::device::code_form("KPJT-HW", None),
    )
}

fn device_wrong(headers: &HeaderMap) -> Response {
    device(
        headers,
        StatusCode::BAD_REQUEST,
        DEVICE_TITLE,
        crate::device::code_form(
            "KPJT-HWLL",
            Some(crate::device::problem_text("invalid_grant")),
        ),
    )
}

fn device_expired(headers: &HeaderMap) -> Response {
    device(
        headers,
        StatusCode::BAD_REQUEST,
        DEVICE_TITLE,
        crate::device::code_form(
            "KPJT-HWLL",
            Some(crate::device::problem_text("expired_token")),
        ),
    )
}

fn device_approve(headers: &HeaderMap) -> Response {
    device(
        headers,
        StatusCode::OK,
        &format!("Sign in to Coder on {COMPUTER}?"),
        crate::device::approve_form("KPJT-HWLL", "Coder", COMPUTER, CSRF),
    )
}

fn device_approved(headers: &HeaderMap) -> Response {
    device(
        headers,
        StatusCode::OK,
        "Signed in",
        html! {
            h1 { "Signed in" }
            p { "Coder on " (COMPUTER) " is signed in to your account. You can go back to your computer." }
        },
    )
}

fn device_denied(headers: &HeaderMap) -> Response {
    device(
        headers,
        StatusCode::OK,
        "Sign-in denied",
        html! {
            h1 { "Sign-in denied" }
            p { "Coder on " (COMPUTER) " was not signed in. Nothing changed on your account." }
        },
    )
}

fn connect_phone(headers: &HeaderMap) -> Response {
    crate::pages::connect::connect_page(headers)
}

fn connect_terminal(headers: &HeaderMap) -> Response {
    let body = crate::terminal_connect::page_body(headers, "sample-pair", html! {});
    UiPage::new("Connect your terminal")
        .path(crate::terminal_connect::PAGE)
        .account(signed_in())
        .content(PageColumn::new(body))
        .respond(headers)
}
