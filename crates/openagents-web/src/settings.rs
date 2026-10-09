//! `/settings`: the signed-in account's profile, theme, and plan
//! ([`crate::plan`]), and `/settings/claude`, where the customer adds or
//! removes their own Claude credential ([`crate::cloud::byo`]). Both open
//! from the account menu.

use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, State};
use axum::http::{HeaderMap, StatusCode, header};
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
use crate::cloud::{
    SIGN_IN, default_workspace, default_workspace_cookies, protect, refused, service,
};
use crate::ui_page::{UiPage, action_link};

pub(crate) const PAGE: &str = "/settings";
pub(crate) const CLAUDE: &str = "/settings/claude";
const CLAUDE_REMOVE: &str = "/settings/claude/remove";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(settings))
        .route(CLAUDE, get(claude).post(add))
        .route(CLAUDE_REMOVE, post(remove))
        .route(crate::plan::EXTRA, post(extra_hours))
}

/// The signed-in viewer, or the answer to give instead (sign in first).
///
/// A session that opened without a workspace (GitHub sign-ins before they
/// picked one) gets the account's own workspace selected here and comes
/// back to `back`; the next request checks that selection as usual.
async fn viewer<'a>(
    app: &'a App,
    headers: &HeaderMap,
    back: &str,
) -> Result<(&'a CloudSession, Viewer), Response> {
    let service = service(app)?;
    let viewer = match service.authenticate(headers).await {
        Ok(viewer) => viewer,
        Err(SessionError::Unauthenticated) => {
            return Err(protect(Redirect::to(SIGN_IN).into_response()));
        }
        Err(error) => return Err(refused(error)),
    };
    if viewer.workspace.is_none() && default_workspace(&viewer).is_some() {
        let cookies = default_workspace_cookies(service, &viewer).map_err(refused)?;
        let mut response = protect(Redirect::to(back).into_response());
        for cookie in cookies {
            response.headers_mut().append(header::SET_COOKIE, cookie);
        }
        return Err(response);
    }
    Ok((service, viewer))
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

/// Where the viewer's own Claude credential stands, for the Settings row.
enum Standing {
    Saved(Material),
    Empty,
    /// The account has no workspace to keep a key for.
    NoWorkspace,
    /// The server's key store can't be read.
    Broken,
}

impl Standing {
    fn of(computers: &Computers, viewer: &Viewer) -> Self {
        let Ok(owner) = Owner::from_viewer(viewer) else {
            return Self::NoWorkspace;
        };
        match computers.status(&owner, now()) {
            Ok(Some(status)) => Self::Saved(status.material),
            Ok(None) => Self::Empty,
            Err(_) => Self::Broken,
        }
    }

    /// The row's hint, and whether its Manage button can work.
    fn hint(&self) -> (String, bool) {
        match self {
            Self::Saved(material) => (format!("Saved: {}", material_label(*material)), true),
            Self::Empty => ("Not added".to_owned(), true),
            Self::NoWorkspace => (
                "Your account has no workspace yet, so there's nowhere to keep a key.".to_owned(),
                false,
            ),
            Self::Broken => (
                "This server can't save keys at the moment. Try again later.".to_owned(),
                false,
            ),
        }
    }
}

async fn settings(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let claude = app
        .config
        .cloud_byo
        .as_deref()
        .map(|computers| Standing::of(computers, &viewer).hint());
    let chats = crate::pages::chat::delete_all::saved(
        &app,
        &crate::chat_store::account_owner(&viewer.account_id),
    )
    .await;
    let computers = crate::device::computers_section(service, &headers, &viewer).await;
    let plan = plan_section(&app, service, &headers, &viewer);
    let body = html! {
        (settings_content(&viewer.account_label, claude, chats, plan))
        (computers)
    };
    page(&headers, service, &viewer, "Settings", PAGE, body)
}

/// The plan section for the viewer: the server's plan (or the checked-in
/// one when none is set up here) and, with a meter, their month.
fn plan_section(app: &App, service: &CloudSession, headers: &HeaderMap, viewer: &Viewer) -> Markup {
    let fallback;
    let plans = match app.config.plan.as_deref() {
        Some(plans) => plans,
        None => {
            fallback = crate::plan::Plans::with_meter(None, None);
            &fallback
        }
    };
    let view = plans.view(&viewer.account_id, now() as i64);
    let request = byo::fresh_request();
    let ticket = plans
        .has_meter()
        .then(|| {
            service
                .csrf(
                    headers,
                    viewer,
                    crate::plan::CSRF_SCOPE,
                    &plan_target(viewer, &request),
                )
                .ok()
        })
        .flatten();
    crate::plan::section(&view, ticket.as_deref().map(|t| (t, request.as_str())))
}

fn plan_target(viewer: &Viewer, request: &str) -> String {
    format!("{}:{request}", viewer.account_id)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtraForm {
    csrf: String,
    request: String,
    enabled: Option<String>,
    cap: String,
}

fn plan_problem(status: StatusCode, text: &str) -> Response {
    protect(crate::layout::problem(
        status,
        "Extra hours",
        text,
        (PAGE, "Settings"),
    ))
}

async fn extra_hours(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<ExtraForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        crate::plan::CSRF_SCOPE,
        &plan_target(&viewer, &form.request),
        &form.csrf,
    ) {
        return refused(error);
    }
    let Some(plans) = app.config.plan.as_deref().filter(|p| p.has_meter()) else {
        return plan_problem(
            StatusCode::NOT_FOUND,
            "This server doesn't track hours yet.",
        );
    };
    let enabled = form.enabled.as_deref() == Some("on");
    let cap = match (crate::plan::parse_cap(&form.cap), enabled) {
        (Some(cap), _) => cap,
        (None, false) if form.cap.trim().is_empty() => 0,
        (None, _) => {
            return plan_problem(
                StatusCode::BAD_REQUEST,
                "Enter a dollar amount up to 10000, like 10.",
            );
        }
    };
    if enabled && cap == 0 {
        return plan_problem(
            StatusCode::BAD_REQUEST,
            "Set how much extra hours may cost each month.",
        );
    }
    let choice = retail_cloud::environment::ExtraHours {
        enabled,
        cap_usd_micros: cap,
    };
    match plans.set_extra(&viewer.account_id, choice) {
        Ok(()) => protect(Redirect::to("/settings#settings-plan").into_response()),
        Err(()) => plan_problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "That couldn't be saved. Try again later.",
        ),
    }
}

/// The Settings page: profile, theme, plan, and (when this server keeps
/// keys) the Claude credential row with its hint and whether Manage can
/// work.
fn settings_content(
    name: &str,
    claude: Option<(String, bool)>,
    chats: Option<usize>,
    plan: Markup,
) -> Markup {
    html! {
        div class="oa-settings" {
            h1 class="oa-heading" data-level="1" { "Settings" }
            section class="oa-settings-group" aria-labelledby="settings-profile" {
                h2 #settings-profile { "Profile" }
                div class="oa-settings-row" {
                    div class="oa-settings-who" {
                        (Avatar::new().name(name.to_owned()).size(AvatarSize::Px40))
                        span class="oa-settings-label" { (name) }
                    }
                }
            }
            section class="oa-settings-group" aria-labelledby="settings-appearance" {
                h2 #settings-appearance { "Appearance" }
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" { "Theme" }
                        span class="oa-settings-hint" { "Switch between light and dark." }
                    }
                    div class="oa-settings-control" {
                        (ThemeToggle::new().fallback_action(crate::theme::TOGGLE_PATH).return_to(PAGE))
                    }
                }
            }
            (chats_section(chats))
            (plan)
            @if let Some((hint, manage)) = claude {
                section class="oa-settings-group" aria-labelledby="settings-claude" {
                    h2 #settings-claude { "Claude" }
                    div class="oa-settings-row" {
                        div class="oa-settings-text" {
                            span class="oa-settings-label" { "Your Claude key" }
                            span class="oa-settings-hint" {
                                "Your own Anthropic API key or cloud credential for Claude Code."
                            }
                            span class="oa-settings-hint" { (hint) }
                        }
                        @if manage {
                            div class="oa-settings-control" { (action_link("Manage", CLAUDE)) }
                        }
                    }
                }
            }
        }
    }
}

/// Chats (#11039, #11038): they belong to the account, and can all be
/// deleted at once. The link shows only when there is something to delete.
fn chats_section(saved: Option<usize>) -> Markup {
    html! {
        section class="oa-settings-group" aria-labelledby="settings-chats" {
            h2 #settings-chats { "Chats" }
            div class="oa-settings-row" {
                div class="oa-settings-text" {
                    span class="oa-settings-label" { "Saved chats" }
                    span class="oa-settings-hint" {
                        @match saved {
                            Some(0) => { "You have no saved chats." }
                            Some(_) => { "Your chats are saved to your account, so they show wherever you sign in." }
                            None => { "Your chats can't be read right now." }
                        }
                    }
                }
                @if saved.is_some_and(|count| count > 0) {
                    div class="oa-settings-control" {
                        (action_link("Delete all chats", crate::pages::chat::delete_all::PATH))
                    }
                }
            }
        }
    }
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
    let (service, viewer) = viewer(app, headers, CLAUDE).await?;
    let Some(computers) = app.config.cloud_byo.as_deref() else {
        return Err(problem(
            StatusCode::NOT_FOUND,
            "This server doesn't keep Claude keys.",
            PAGE,
        ));
    };
    let Ok(owner) = Owner::from_viewer(&viewer) else {
        return Err(problem(
            StatusCode::FORBIDDEN,
            "Your account has no workspace yet, so there's nowhere to keep a key.",
            PAGE,
        ));
    };
    Ok(Context {
        service,
        viewer,
        owner,
        computers,
    })
}

fn problem(status: StatusCode, text: &str, back: &str) -> Response {
    let label = if back == PAGE { "Settings" } else { "Back" };
    protect(crate::layout::problem(
        status,
        "Claude credential",
        text,
        (back, label),
    ))
}

fn custody_failure(error: CustodyError) -> Response {
    let (status, text, back) = match error {
        CustodyError::Invalid => (
            StatusCode::BAD_REQUEST,
            "That isn't a valid key for this provider. Anthropic API keys start with sk-ant-api. Claude.ai logins and setup tokens aren't accepted.",
            CLAUDE,
        ),
        CustodyError::Consent => (
            StatusCode::BAD_REQUEST,
            "Check the box to save the key.",
            CLAUDE,
        ),
        CustodyError::Absent | CustodyError::Changed => (
            StatusCode::CONFLICT,
            "Your key changed. Reload the page.",
            CLAUDE,
        ),
        CustodyError::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            "This server can't save keys at the moment. Try again later.",
            PAGE,
        ),
    };
    problem(status, text, back)
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

/// The Claude credential page: what is saved (never the key itself), a
/// remove button, and the form that adds or replaces it.
fn claude_content(saved: Option<Material>, add: (&str, &str), remove: (&str, &str)) -> Markup {
    let material = Field::new("claude-credential-material", "Provider");
    let value = Field::new("claude-credential-value", "Key")
        .required(true)
        .description("Anthropic: the API key, from console.anthropic.com. Bedrock, Vertex, or Foundry: the JSON credential.");
    html! {
        p { (action_link("Settings", PAGE)) }
        (MarkdownRoot::new(html! {
            h1 { "Claude credential" }
            p { "Add your own Anthropic API key, or an Amazon Bedrock, Google Vertex AI, or Microsoft Foundry credential, to run Claude Code tasks in parallel. Usage bills to your own account." }
            @match saved {
                Some(material) => {
                    p { "Saved: " (material_label(material)) }
                    p class="oa-page-meta" aria-label="Key hidden" { "••••••••••••••••" }
                },
                None => p { "Nothing saved. Without a key, Claude Code runs one task at a time on your Claude plan." },
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
                (Checkbox::new("consent", "Save this key for my account")
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

    #[test]
    fn settings_hides_manage_when_the_claude_key_cant_be_kept() {
        let manage = "href=\"/settings/claude\"";
        for standing in [Standing::Empty, Standing::Saved(Material::AnthropicApiKey)] {
            let html =
                settings_content("Ada", Some(standing.hint()), Some(0), html! {}).into_string();
            assert!(html.contains(manage), "{html}");
            assert!(!html.contains("Unavailable"));
        }
        for standing in [Standing::NoWorkspace, Standing::Broken] {
            let (hint, can) = standing.hint();
            assert!(!can);
            let html =
                settings_content("Ada", Some((hint.clone(), can)), Some(0), html! {}).into_string();
            assert!(!html.contains(manage), "{html}");
            assert!(html.contains(&hint.replace('\'', "&#39;")) || html.contains(&hint));
        }
        // No key store: no Claude row at all.
        let html = settings_content("Ada", None, Some(0), html! {}).into_string();
        assert!(!html.contains("settings-claude"));
        for needle in [">Settings<", ">Profile<", ">Theme<", ">Ada<"] {
            assert!(html.contains(needle), "{needle}");
        }
        let text = oa_copy::visible_text(&html);
        assert_eq!(oa_copy::violations(&text, &[]), vec![], "{text}");
    }
}
