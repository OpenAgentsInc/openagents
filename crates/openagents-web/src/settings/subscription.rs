//! Connect your Claude subscription: Settings, Claude's three-step flow
//! for a Claude Pro or Max plan token (`claude setup-token`), for the
//! accounts on the subscription-token allowlist only ([`byo::TokenAllow`]).
//!
//! The same steps render three ways from [`form`]: a dialog on Settings,
//! Claude (opened by [`link`]), the same dialog loaded on demand from the
//! composer's connect card ([`crate::composer_row::connect_card`],
//! `?part=dialog`), and, without script, a plain page at [`PATH`]. Posting
//! it checks the token's shape, then checks it with Anthropic
//! ([`byo::Computers::check`]), then keeps it sealed; a refused or
//! malformed token comes back to the page with a plain error and is never
//! repeated.

use axum::extract::rejection::FormRejection;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use maud::{Markup, html};
use openagents_ui::actions::{Alert, Button, ButtonLink, ButtonType, ButtonVariant, Color};
use openagents_ui::content::{CodeBlock, MarkdownRoot};
use openagents_ui::forms::{Field, Input, InputType};
use openagents_ui::overlays::Dialog;
use serde::Deserialize;

use super::{CLAUDE, Context, context, custody_failure, page, problem};
use crate::App;
use crate::cloud::byo;
use crate::cloud::custody::{Key, Material};
use crate::cloud::session::{SessionError, now};
use crate::cloud::{protect, refused};
use crate::ui_page::action_link;

/// The flow's page (no script), its dialog (`?part=dialog`), and its form.
pub(crate) const PATH: &str = "/settings/claude/subscription";
/// The dialog's id, which [`link`] opens.
pub(crate) const DIALOG: &str = "connect-subscription";
/// The command step 1 runs.
pub(crate) const COMMAND: &str = "claude setup-token";
/// Anthropic's Claude Code install page.
pub(crate) const INSTALL_PAGE: &str = "https://code.claude.com/docs/en/setup";
/// Anthropic's page on Claude Code sign-in and long-lived tokens.
pub(crate) const ABOUT_TOKEN: &str = "https://code.claude.com/docs/en/authentication";
const TITLE: &str = "Connect your Claude subscription";
/// The CSRF scope of the form.
const SCOPE: &str = "claude-subscription";
/// A well-formed token, as the browser checks it before sending: the
/// same shape `OwnCredential::SubscriptionToken.canonical` accepts.
pub(crate) const TOKEN_PATTERN: &str = r"\s*sk-ant-oat[0-9]+-[A-Za-z0-9_\-]{16,}\s*";
const MALFORMED: &str = "That isn't a Claude subscription token. It starts with sk-ant-oat. Copy the whole token that claude setup-token printed and paste it again. Nothing was saved.";
const REFUSED: &str = "Anthropic didn't accept that token. Run claude setup-token again and paste the new token. Nothing was saved.";
const UNREACHABLE: &str = "Anthropic couldn't be reached to check the token, so nothing was saved. Try again in a minute.";

/// Where a successful connect returns: Settings, Claude, the home page, or
/// a chat (from the composer's card). Anything else is Settings, Claude.
pub(crate) fn back(raw: Option<&str>) -> &str {
    let Some(raw) = raw else { return CLAUDE };
    let chat = raw.strip_prefix("/chat/").is_some_and(|id| {
        !id.is_empty()
            && id.len() <= 128
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    });
    if raw == "/" || raw == CLAUDE || chat {
        raw
    } else {
        CLAUDE
    }
}

/// The steps as one form posting to [`PATH`]. In the dialog, Cancel closes
/// it (`formmethod="dialog"`); on the page it goes back.
pub(crate) fn form(ticket: (&str, &str), back: &str, error: Option<&str>, dialog: bool) -> Markup {
    let token = Field::new(
        "subscription-token",
        "Paste the token that the command prints",
    )
    .required(true)
    .error_opt(error);
    html! {
        form.oa-setup-form method="post" action=(PATH) autocomplete="off"
            x-data="oaSubmitWhenValid" {
            input type="hidden" name="csrf" value=(ticket.0);
            input type="hidden" name="request" value=(ticket.1);
            input type="hidden" name="back" value=(back);
            p {
                "Use your Claude Pro or Max plan for Claude Code runs on OpenAgents. OpenAgents checks the token with Anthropic before saving it."
            }
            ol.oa-setup-steps {
                li {
                    p { "In a terminal on your own computer, run:" }
                    (CodeBlock::new(COMMAND))
                    p.oa-setup-hint {
                        "This needs Claude Code. "
                        a href=(INSTALL_PAGE) target="_blank" rel="noopener noreferrer" {
                            "Install Claude Code ↗"
                        }
                    }
                }
                li {
                    p { "A browser window opens. Sign in with your Claude Pro or Max account and approve access." }
                }
                li {
                    (token.clone().control(
                        Input::new("token")
                            .input_type(InputType::Text)
                            .placeholder("sk-ant-oat…")
                            .pattern(TOKEN_PATTERN)
                            .maxlength(1024)
                            .required(true)
                            .autocomplete("off")
                            .spellcheck(false)
                            .invalid(error.is_some())
                            .aria(token.aria()),
                    ))
                    p.oa-setup-hint {
                        "The token is valid for one year. "
                        a href=(ABOUT_TOKEN) target="_blank" rel="noopener noreferrer" {
                            "About this token ↗"
                        }
                    }
                }
            }
            p.oa-setup-hint {
                "It's kept sealed for your account, used only for your own Claude Code runs, and bills your Claude plan, one task at a time. Remove it any time."
            }
            div.oa-dialog-footer {
                @if dialog {
                    (Button::new("Cancel")
                        .kind(ButtonType::Submit)
                        .variant(ButtonVariant::Soft)
                        .color(Color::Secondary)
                        .attr("formmethod", "dialog")
                        .attr("formnovalidate", ""))
                } @else {
                    (ButtonLink::new("Cancel", back)
                        .variant(ButtonVariant::Soft)
                        .color(Color::Secondary))
                }
                (Button::new("Connect subscription").kind(ButtonType::Submit))
            }
        }
    }
}

/// The flow as a dialog; `open` for one loaded on demand.
pub(crate) fn dialog(ticket: (&str, &str), back: &str, open: bool) -> Markup {
    html! {
        (Dialog::new(DIALOG, TITLE, form(ticket, back, None, true)).open_on_load(open))
    }
}

/// The button that opens the dialog on this page, or the flow's own page
/// without script.
pub(crate) fn link() -> Markup {
    html! {
        (ButtonLink::new("Connect subscription", PATH)
            .attr("data-oa-dialog", DIALOG)
            .attr("aria-haspopup", "dialog")
            .attr("aria-controls", DIALOG))
    }
}

/// A fresh ticket for the form.
pub(super) fn ticket(
    context: &Context<'_>,
    headers: &HeaderMap,
) -> Result<(String, String), Response> {
    let request = byo::fresh_request();
    context
        .service
        .csrf(
            headers,
            &context.viewer,
            SCOPE,
            &byo::target(&context.owner, &request),
        )
        .map(|csrf| (csrf, request))
        .map_err(refused)
}

/// Whether the signed-in viewer may connect a subscription here: a key
/// store and the allowlist (#11235). The composer's card asks.
pub(crate) async fn offered(app: &App, headers: &HeaderMap) -> bool {
    if app.config.cloud_byo.is_none() {
        return false;
    }
    let Some(service) = app.config.cloud.as_deref() else {
        return false;
    };
    match service.authenticate(headers).await {
        Ok(viewer) => viewer.workspace.is_some() && app.config.claude_tokens.admits_viewer(&viewer),
        Err(_) => false,
    }
}

#[derive(Deserialize)]
pub(super) struct Shown {
    part: Option<String>,
    back: Option<String>,
}

/// The flow's page, or (`?part=dialog`) its dialog alone, open, for the
/// composer. Off the allowlist: Settings, Claude, at the key.
pub(super) async fn show(
    State(app): State<App>,
    headers: HeaderMap,
    Query(shown): Query<Shown>,
) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let back = back(shown.back.as_deref());
    if !app.config.claude_tokens.admits_viewer(&context.viewer) {
        if shown.part.is_some() {
            return protect(StatusCode::NOT_FOUND.into_response());
        }
        return protect(Redirect::to(&format!("{CLAUDE}#key")).into_response());
    }
    let ticket = match ticket(&context, &headers) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if shown.part.as_deref() == Some("dialog") {
        return protect(
            Html(dialog((&ticket.0, &ticket.1), back, true).into_string()).into_response(),
        );
    }
    respond(&headers, &context, (&ticket.0, &ticket.1), back, None)
}

fn respond(
    headers: &HeaderMap,
    context: &Context<'_>,
    ticket: (&str, &str),
    back: &str,
    error: Option<(StatusCode, &str)>,
) -> Response {
    let body = html! {
        p { (action_link("Claude", CLAUDE)) }
        (MarkdownRoot::new(html! { h1 { (TITLE) } }))
        @if let Some((_, text)) = error {
            (Alert::new().color(Color::Danger).description(text))
        }
        (form(ticket, back, error.map(|(_, text)| text), false))
    };
    let mut response = page(headers, context.service, &context.viewer, TITLE, PATH, body);
    if let Some((status, _)) = error {
        *response.status_mut() = status;
    }
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConnectForm {
    csrf: String,
    request: String,
    token: String,
    back: Option<String>,
}

/// Check the token's shape, then with Anthropic, then keep it sealed.
pub(super) async fn connect(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<ConnectForm>, FormRejection>,
) -> Response {
    let Ok(Form(mut form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = context.service.verify_csrf(
        &headers,
        Some(&context.viewer),
        SCOPE,
        &byo::target(&context.owner, &form.request),
        &form.csrf,
    ) {
        return refused(error);
    }
    // Off the allowlist the value is dropped unread (#11235).
    if !app.config.claude_tokens.admits_viewer(&context.viewer) {
        return problem(StatusCode::BAD_REQUEST, super::TOKEN_REFUSED, CLAUDE);
    }
    let back = back(form.back.as_deref()).to_owned();
    let value = std::mem::take(&mut form.token);
    let failed = |status, text| match ticket(&context, &headers) {
        Ok(ticket) => respond(
            &headers,
            &context,
            (&ticket.0, &ticket.1),
            &back,
            Some((status, text)),
        ),
        Err(response) => response,
    };
    // Malformed: refused before anything is sent to Anthropic.
    let shaped = byo::detect(Material::ClaudeSubscriptionToken, &value)
        == Material::ClaudeSubscriptionToken
        && value.trim().starts_with("sk-ant-oat");
    let key = match shaped
        .then(|| Key::for_material(Material::ClaudeSubscriptionToken, value))
        .and_then(Result::ok)
    {
        Some(key) => key,
        None => return failed(StatusCode::BAD_REQUEST, MALFORMED),
    };
    if let Err(error) = context
        .computers
        .check(Material::ClaudeSubscriptionToken, &key)
        .await
    {
        return match error {
            byo::CheckError::Refused => failed(StatusCode::BAD_REQUEST, REFUSED),
            byo::CheckError::Unreachable => failed(StatusCode::BAD_GATEWAY, UNREACHABLE),
        };
    }
    match context.computers.store(
        &context.owner,
        Material::ClaudeSubscriptionToken,
        key,
        true,
        now(),
    ) {
        Ok(_) => protect(Redirect::to(&back).into_response()),
        Err(error) => custody_failure(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(dialog: bool) -> String {
        form(("t", "r"), CLAUDE, None, dialog).into_string()
    }

    #[test]
    fn the_flow_has_three_steps_in_order() {
        let html = rendered(true);
        let one = html
            .find("In a terminal on your own computer, run:")
            .unwrap();
        let two = html
            .find("A browser window opens. Sign in with your Claude Pro or Max account and approve access.")
            .unwrap();
        let three = html
            .find("Paste the token that the command prints")
            .unwrap();
        assert!(one < two && two < three, "{html}");
        assert_eq!(html.matches("<li>").count(), 3, "{html}");
        for needle in [
            "Use your Claude Pro or Max plan for Claude Code runs on OpenAgents. OpenAgents checks the token with Anthropic before saving it.",
            "This needs Claude Code.",
            "Install Claude Code ↗",
            INSTALL_PAGE,
            "placeholder=\"sk-ant-oat…\"",
            "The token is valid for one year.",
            "About this token ↗",
            ABOUT_TOKEN,
            ">Cancel<",
            ">Connect subscription<",
            "action=\"/settings/claude/subscription\"",
            "x-data=\"oaSubmitWhenValid\"",
        ] {
            assert!(html.contains(needle), "{needle}: {html}");
        }
        // The secret never reaches autofill or the spellchecker.
        assert!(
            html.contains("autocomplete=\"off\" spellcheck=\"false\""),
            "{html}"
        );
        crate::copy_guard::assert_plain(PATH, &html);
    }

    #[test]
    fn the_command_has_a_copy_button() {
        let html = rendered(false);
        assert!(
            html.contains("<code class=\"oa-code-block__code\">claude setup-token</code>"),
            "{html}"
        );
        // The CopyButton hook copies data-oa-copy in one click, no inline
        // script; the hook ships in the page's component script.
        assert!(
            html.contains("data-oa-copy=\"claude setup-token\""),
            "{html}"
        );
        assert!(openagents_ui::script().contains("[data-oa-copy]"));
        assert!(!html.contains("<script"), "{html}");
    }

    #[test]
    fn connect_waits_for_a_well_formed_token() {
        let html = rendered(true);
        let input = html
            .split("<input")
            .find(|tag| tag.contains("name=\"token\""))
            .unwrap();
        assert!(input.contains("required"), "{input}");
        assert!(input.contains("pattern="), "{input}");
        // The browser's pattern matches what the server accepts.
        let pattern = regex_lite(TOKEN_PATTERN);
        let good = format!("sk-ant-oat01-{}", "a1".repeat(20));
        assert!(pattern(&good));
        assert!(pattern(&format!(" {good} ")));
        for bad in [
            "",
            "sk-ant-api03-abcdefghijklmnopqrst",
            "sk-ant-oat01-short",
            "hello",
        ] {
            assert!(!pattern(bad), "{bad}");
        }
        // Script disables it until then; without script the stylesheet
        // shows it disabled and the browser refuses the form.
        assert!(openagents_ui::script().contains("oaSubmitWhenValid"));
        assert!(openagents_ui::stylesheet().contains(".oa-setup-form:invalid"));
        // In the dialog, Cancel closes it without checking the field.
        assert!(html.contains("formmethod=\"dialog\""), "{html}");
        assert!(html.contains("formnovalidate"), "{html}");
        // On its own page, Cancel goes back.
        let page = rendered(false);
        assert!(!page.contains("formmethod"), "{page}");
        assert!(page.contains("href=\"/settings/claude\""), "{page}");
    }

    /// The token pattern as the browser applies it (anchored), for the few
    /// constructs it uses.
    fn regex_lite(pattern: &str) -> impl Fn(&str) -> bool {
        assert_eq!(pattern, r"\s*sk-ant-oat[0-9]+-[A-Za-z0-9_\-]{16,}\s*");
        |value: &str| {
            let value = value.trim();
            let Some(rest) = value.strip_prefix("sk-ant-oat") else {
                return false;
            };
            let Some((version, body)) = rest.split_once('-') else {
                return false;
            };
            !version.is_empty()
                && version.chars().all(|c| c.is_ascii_digit())
                && body.len() >= 16
                && body
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        }
    }

    #[test]
    fn the_dialog_opens_from_its_link_and_falls_back_to_the_page() {
        let dialog = dialog(("t", "r"), CLAUDE, false).into_string();
        assert!(
            dialog.contains("<dialog id=\"connect-subscription\""),
            "{dialog}"
        );
        assert!(
            dialog.contains(">Connect your Claude subscription<"),
            "{dialog}"
        );
        assert!(!dialog.contains("data-oa-open"), "{dialog}");
        let loaded = super::dialog(("t", "r"), "/", true).into_string();
        assert!(loaded.contains("data-oa-open"), "{loaded}");
        let link = link().into_string();
        assert!(
            link.contains("href=\"/settings/claude/subscription\""),
            "{link}"
        );
        assert!(
            link.contains("data-oa-dialog=\"connect-subscription\""),
            "{link}"
        );
    }

    #[test]
    fn back_goes_only_to_known_places() {
        assert_eq!(back(None), CLAUDE);
        assert_eq!(back(Some("/")), "/");
        assert_eq!(back(Some("/chat/abc-123_x")), "/chat/abc-123_x");
        for bad in [
            "//evil.example",
            "https://evil.example",
            "/chat/",
            "/chat/a/b",
            "/chat/a?x=1",
            "/settings",
        ] {
            assert_eq!(back(Some(bad)), CLAUDE, "{bad}");
        }
    }
}
