//! `/settings`: the signed-in account's profile and theme, and
//! `/settings/claude`, where the customer adds or removes their own Claude
//! credential ([`crate::cloud::byo`]). Both open from the account menu.

use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use openagents_ui::actions::{Avatar, AvatarSize, Button, ButtonType, ButtonVariant, Color};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::forms::{Checkbox, Field, Select, Textarea};
use openagents_ui::shell::ThemeToggle;
use serde::Deserialize;

use crate::App;
use crate::account::Account;
use crate::cloud::byo::{self, Computers, Owner};
use crate::cloud::custody::{CustodyError, Key, Material};
use crate::cloud::session::{CloudSession, SessionError, Viewer, now};
use crate::cloud::{SIGN_IN, protect, refused, service};
use crate::ui_page::{UiPage, action_link};

pub(crate) const PAGE: &str = "/settings";
pub(crate) const CLAUDE: &str = "/settings/claude";
const CLAUDE_REMOVE: &str = "/settings/claude/remove";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(settings))
        .route(CLAUDE, get(claude).post(add))
        .route(CLAUDE_REMOVE, post(remove))
}

/// The signed-in viewer, or the answer to give instead (sign in first).
async fn viewer<'a>(
    app: &'a App,
    headers: &HeaderMap,
) -> Result<(&'a CloudSession, Viewer), Response> {
    let service = service(app)?;
    match service.authenticate(headers).await {
        Ok(viewer) => Ok((service, viewer)),
        Err(SessionError::Unauthenticated) => Err(protect(Redirect::to(SIGN_IN).into_response())),
        Err(error) => Err(refused(error)),
    }
}

/// The page, shown with the viewer's account menu.
fn page(
    headers: &HeaderMap,
    service: &CloudSession,
    viewer: &Viewer,
    title: &str,
    path: &str,
    body: Markup,
) -> Response {
    let account = Account::SignedIn {
        name: viewer.account_label.clone(),
        sign_out: service.logout_csrf(headers, viewer).ok(),
        picture: viewer.avatar_url.is_some(),
    };
    protect(
        UiPage::new(title)
            .path(path)
            .section(PAGE)
            .account(account)
            .content(PageColumn::new(body))
            .respond(headers),
    )
}

async fn settings(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, viewer) = match viewer(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let claude = match app.config.cloud_byo.as_deref() {
        Some(computers) => match Owner::from_viewer(&viewer)
            .ok()
            .map(|owner| computers.status(&owner, now()))
        {
            Some(Ok(Some(status))) => Some(format!("Saved: {}", material_label(status.material))),
            Some(Ok(None)) => Some("Not added".to_owned()),
            Some(Err(_)) | None => Some("Unavailable right now".to_owned()),
        },
        None => None,
    };
    let body = html! {
        (MarkdownRoot::new(html! { h1 { "Settings" } }))
        section aria-labelledby="settings-profile" {
            (MarkdownRoot::new(html! { h2 #settings-profile { "Profile" } }))
            p {
                (Avatar::new().name(viewer.account_label.clone()).size(AvatarSize::Px40))
                " " strong { (viewer.account_label) }
            }
        }
        section aria-labelledby="settings-theme" {
            (MarkdownRoot::new(html! { h2 #settings-theme { "Theme" } }))
            (MarkdownRoot::new(html! { p { "Switch between light and dark." } }))
            div { (ThemeToggle::new().fallback_action(crate::theme::TOGGLE_PATH).return_to(PAGE)) }
        }
        @if let Some(claude) = claude {
            section aria-labelledby="settings-claude" {
                (MarkdownRoot::new(html! {
                    h2 #settings-claude { "Claude" }
                    p { "Your own Anthropic API key or cloud credential for Claude Code. " (claude) }
                }))
                p { (action_link("Manage", CLAUDE)) }
            }
        }
    };
    page(&headers, service, &viewer, "Settings", PAGE, body)
}

fn material_label(material: Material) -> &'static str {
    match material {
        Material::AnthropicApiKey => "Anthropic API key",
        Material::BedrockCredential => "Amazon Bedrock",
        Material::VertexCredential => "Google Vertex AI",
        Material::FoundryCredential => "Microsoft Foundry",
        Material::OpenAiApiKey => "OpenAI API key",
    }
}

/// The Claude credential page's viewer, owner, and storage.
struct Context<'a> {
    service: &'a CloudSession,
    viewer: Viewer,
    owner: Owner,
    computers: &'a Computers,
}

async fn context<'a>(app: &'a App, headers: &HeaderMap) -> Result<Context<'a>, Response> {
    let (service, viewer) = viewer(app, headers).await?;
    let Some(computers) = app.config.cloud_byo.as_deref() else {
        return Err(problem(
            StatusCode::NOT_FOUND,
            "This server doesn't store Claude credentials.",
        ));
    };
    let owner = Owner::from_viewer(&viewer).map_err(refused)?;
    Ok(Context {
        service,
        viewer,
        owner,
        computers,
    })
}

fn problem(status: StatusCode, text: &str) -> Response {
    protect(crate::layout::problem(
        status,
        "Claude credential",
        text,
        (CLAUDE, "Back"),
    ))
}

fn custody_failure(error: CustodyError) -> Response {
    let (status, text) = match error {
        CustodyError::Invalid => (
            StatusCode::BAD_REQUEST,
            "That isn't a valid credential for this provider. Claude.ai logins and setup tokens aren't accepted.",
        ),
        CustodyError::Consent => (
            StatusCode::BAD_REQUEST,
            "Check the box to save the credential.",
        ),
        CustodyError::Absent | CustodyError::Changed => (
            StatusCode::CONFLICT,
            "Your credential changed. Reload the page.",
        ),
        CustodyError::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Saving credentials isn't working right now. Try again later.",
        ),
    };
    problem(status, text)
}

async fn claude(State(app): State<App>, headers: HeaderMap) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let status = match context.computers.status(&context.owner, now()) {
        Ok(value) => value,
        Err(error) => return custody_failure(error),
    };
    let mut tickets = Vec::new();
    for scope in ["claude-credential", "claude-credential-remove"] {
        let request = byo::fresh_request();
        match context.service.csrf(
            &headers,
            &context.viewer,
            scope,
            &byo::target(&context.owner, &request),
        ) {
            Ok(csrf) => tickets.push((csrf, request)),
            Err(error) => return refused(error),
        }
    }
    let body = claude_content(
        status.map(|status| status.material),
        (&tickets[0].0, &tickets[0].1),
        (&tickets[1].0, &tickets[1].1),
    );
    page(
        &headers,
        context.service,
        &context.viewer,
        "Claude credential",
        CLAUDE,
        body,
    )
}

/// The Claude credential page: what is saved, a remove button, and the
/// form that adds or replaces it.
fn claude_content(saved: Option<Material>, add: (&str, &str), remove: (&str, &str)) -> Markup {
    let material = Field::new("claude-credential-material", "Provider");
    let value = Field::new("claude-credential-value", "Credential")
        .required(true)
        .description("Anthropic: the API key. Bedrock, Vertex, or Foundry: the JSON credential.");
    html! {
        p { (action_link("Settings", PAGE)) }
        (MarkdownRoot::new(html! {
            h1 { "Claude credential" }
            p { "Add your own Anthropic API key, or an Amazon Bedrock, Google Vertex AI, or Microsoft Foundry credential, to run Claude Code tasks in parallel. Usage bills to your own account." }
            @match saved {
                Some(material) => p { "Saved: " (material_label(material)) },
                None => p { "Nothing saved. Without a credential, Claude Code runs one task at a time on your Claude plan." },
            }
        }))
        @if saved.is_some() {
            form method="post" action=(CLAUDE_REMOVE) {
                input type="hidden" name="csrf" value=(remove.0);
                input type="hidden" name="request" value=(remove.1);
                p {
                    (Button::new("Remove")
                        .kind(ButtonType::Submit)
                        .variant(ButtonVariant::Soft)
                        .color(Color::Secondary))
                }
            }
        }
        form method="post" action=(CLAUDE) autocomplete="off" {
            input type="hidden" name="csrf" value=(add.0);
            input type="hidden" name="request" value=(add.1);
            (material.clone().control(
                Select::new("material")
                    .aria(material.aria())
                    .option("anthropic_api_key", "Anthropic API key")
                    .option("bedrock_credential", "Amazon Bedrock")
                    .option("vertex_credential", "Google Vertex AI")
                    .option("foundry_credential", "Microsoft Foundry"),
            ))
            (value.clone().control(
                Textarea::new("value")
                    .aria(value.aria())
                    .required(true)
                    .autocomplete("off")
                    .spellcheck(false),
            ))
            p {
                (Checkbox::new("consent", "Save this credential for my account")
                    .value("custody")
                    .description("It's used only for your own tasks and never shows up in saved environments, exports, or logs. Remove it any time.")
                    .required(true))
            }
            p {
                (Button::new(if saved.is_some() { "Replace" } else { "Save" })
                    .kind(ButtonType::Submit))
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddForm {
    csrf: String,
    request: String,
    material: Material,
    value: String,
    consent: Option<String>,
}

async fn add(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<AddForm>, FormRejection>,
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
        "claude-credential",
        &byo::target(&context.owner, &form.request),
        &form.csrf,
    ) {
        return refused(error);
    }
    let key = match Key::for_material(form.material, std::mem::take(&mut form.value)) {
        Ok(value) => value,
        Err(error) => return custody_failure(error),
    };
    let consent = form.consent.as_deref() == Some("custody");
    match context
        .computers
        .store(&context.owner, form.material, key, consent, now())
    {
        Ok(_) => protect(Redirect::to(CLAUDE).into_response()),
        Err(error) => custody_failure(error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoveForm {
    csrf: String,
    request: String,
}

async fn remove(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<RemoveForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = context.service.verify_csrf(
        &headers,
        Some(&context.viewer),
        "claude-credential-remove",
        &byo::target(&context.owner, &form.request),
        &form.csrf,
    ) {
        return refused(error);
    }
    match context.computers.revoke(&context.owner) {
        Ok(_) => protect(Redirect::to(CLAUDE).into_response()),
        Err(error) => custody_failure(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_claude_page_says_usage_bills_to_the_users_own_account() {
        let html = claude_content(None, ("t", "r"), ("t", "r")).into_string();
        assert!(html.contains("Usage bills to your own account."));
        assert!(!html.contains(">Remove<"));
        // The secret never reaches autofill or the spellchecker.
        assert!(
            html.contains(
                "<form method=\"post\" action=\"/settings/claude\" autocomplete=\"off\">"
            )
        );
        let area = html
            .split("<textarea")
            .nth(1)
            .unwrap()
            .split('>')
            .next()
            .unwrap();
        assert!(
            area.contains(" autocomplete=\"off\" spellcheck=\"false\""),
            "{area}"
        );
        assert!(html.contains("name=\"consent\" value=\"custody\""));
        let saved =
            claude_content(Some(Material::BedrockCredential), ("t", "r"), ("t", "r")).into_string();
        assert!(saved.contains("Saved: Amazon Bedrock"));
        assert!(saved.contains("action=\"/settings/claude/remove\""));
        assert!(byo::TERMS.contains(
            "never put in a checkpoint, saved environment image, export, log, or evidence"
        ));
    }
}
