//! `/settings`: the signed-in account's profile, theme, and plan
//! ([`crate::plan`]), and `/settings/claude`, where the customer adds or
//! removes their own Claude credential ([`crate::cloud::byo`]). Both open
//! from the account menu.

use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, RawQuery, State};
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
        .route(
            crate::api_keys::KEYS,
            get(crate::api_keys::keys).post(crate::api_keys::make),
        )
        .route(crate::api_keys::REVOKE, post(crate::api_keys::revoke))
        .route(crate::api_keys::OWN, post(crate::api_keys::own_save))
        .route(
            crate::api_keys::OWN_REMOVE,
            post(crate::api_keys::own_remove),
        )
        .route(crate::plan::EXTRA, post(extra_hours))
        .route(crate::plan::SUBSCRIBE, post(subscribe))
        .route(crate::plan::MANAGE, post(manage))
}

/// The signed-in viewer, or the answer to give instead (sign in first).
///
/// A session that opened without a workspace (GitHub sign-ins before they
/// picked one) gets the account's own workspace selected here and comes
/// back to `back`; the next request checks that selection as usual.
pub(crate) async fn viewer<'a>(
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
pub(crate) fn page(
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
        admin: viewer.admin,
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

async fn settings(
    State(app): State<App>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Response {
    let returned = query.is_some_and(|q| q.split('&').any(|p| p == "plan=started"));
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
    let computers = crate::device::computers_section(&app, service, &headers, &viewer).await;
    let plan = plan_section(&app, service, &headers, &viewer, returned);
    let macs = crate::mac_jobs_page::settings_row(
        &app.config.chat_store,
        &crate::chat_store::account_owner(&viewer.account_id),
    )
    .await;
    let body = html! {
        (settings_content(&viewer.account_label, claude, chats, plan))
        (computers)
        (macs)
    };
    page(&headers, service, &viewer, "Settings", PAGE, body)
}

/// The plan section for the viewer: the server's plan (or the checked-in
/// one when none is set up here) and, with a meter, their month.
fn plan_section(
    app: &App,
    service: &CloudSession,
    headers: &HeaderMap,
    viewer: &Viewer,
    returned: bool,
) -> Markup {
    let fallback;
    let plans = match app.config.plan.as_deref() {
        Some(plans) => plans,
        None => {
            fallback = crate::plan::Plans::with_meter(None, None);
            &fallback
        }
    };
    let view = plans.view(&viewer.account_id, now() as i64, returned);
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
    let checkout = plans.checkout().and_then(|_| {
        service
            .csrf(
                headers,
                viewer,
                crate::plan::CHECKOUT_SCOPE,
                &plan_target(viewer, &request),
            )
            .ok()
    });
    crate::plan::section(
        &view,
        ticket.as_deref().map(|t| (t, request.as_str())),
        checkout.as_deref().map(|t| (t, request.as_str())),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckoutForm {
    csrf: String,
    request: String,
}

fn billing_problem(status: StatusCode, text: &str) -> Response {
    protect(crate::layout::problem(
        status,
        "Subscription",
        text,
        (PAGE, "Settings"),
    ))
}

/// The sentence for a refused checkout or billing page.
fn billing_refusal(error: &jev::Error) -> &'static str {
    let code = match error {
        jev::Error::Api(api) => api
            .body
            .as_ref()
            .and_then(|b| b.as_json())
            .and_then(|v| v["error"]["code"].as_str().map(str::to_owned)),
        _ => None,
    };
    match code.as_deref() {
        Some("already_subscribed") => {
            "You're already subscribed. Reload Settings to see your plan."
        }
        Some("checkout_pending") => {
            "You already started a checkout. Finish it in the tab where it opened, or try again in an hour."
        }
        Some("no_subscription") => "There's no subscription to manage yet.",
        Some("forbidden") => "Only the owner of your workspace can change its plan.",
        _ => "Stripe couldn't be reached right now. Try again in a minute.",
    }
}

/// The checked ticket, plans with checkout, and the workspace to bill: the
/// person's own (the first they own).
async fn checkout_request(
    app: &App,
    headers: &HeaderMap,
    form: Result<Form<CheckoutForm>, FormRejection>,
) -> Result<(Viewer, String, String), Response> {
    let Ok(Form(form)) = form else {
        return Err(refused(SessionError::InvalidRequest));
    };
    let (service, viewer) = viewer(app, headers, PAGE).await?;
    service
        .verify_csrf(
            headers,
            Some(&viewer),
            crate::plan::CHECKOUT_SCOPE,
            &plan_target(&viewer, &form.request),
            &form.csrf,
        )
        .map_err(refused)?;
    let Some(plan) = app.config.plan.as_deref().and_then(|p| p.checkout()) else {
        return Err(billing_problem(
            StatusCode::NOT_FOUND,
            "Subscribing isn't open on this server yet.",
        ));
    };
    let Some(workspace) = viewer.workspaces.iter().find(|w| w.role == "owner") else {
        return Err(billing_problem(
            StatusCode::CONFLICT,
            "Your account has no workspace of its own to bill yet.",
        ));
    };
    let workspace = workspace.id.clone();
    Ok((viewer, plan.to_owned(), workspace))
}

/// Subscribe: open Stripe Checkout for the plan and send the browser there.
async fn subscribe(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<CheckoutForm>, FormRejection>,
) -> Response {
    let (viewer, plan, workspace) = match checkout_request(&app, &headers, form).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match viewer
        .client()
        .account()
        .plan_checkout(&workspace, &plan)
        .await
    {
        Ok(url) => protect(Redirect::to(&url).into_response()),
        Err(error) => billing_problem(StatusCode::BAD_GATEWAY, billing_refusal(&error)),
    }
}

/// Manage subscription: Stripe's billing page, to change the card or cancel.
async fn manage(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<CheckoutForm>, FormRejection>,
) -> Response {
    let (viewer, _, workspace) = match checkout_request(&app, &headers, form).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match viewer.client().account().billing_portal(&workspace).await {
        Ok(url) => protect(Redirect::to(&url).into_response()),
        Err(error) => billing_problem(StatusCode::BAD_GATEWAY, billing_refusal(&error)),
    }
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
            section class="oa-settings-group" aria-labelledby="settings-api" {
                h2 #settings-api { "API" }
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" { "API keys" }
                        span class="oa-settings-hint" {
                            "Keys for calling models through the OpenAgents API from your own code."
                        }
                    }
                    div class="oa-settings-control" { (action_link("Manage", crate::api_keys::KEYS)) }
                }
            }
            section class="oa-settings-group" aria-labelledby="settings-memory" {
                h2 #settings-memory { "Memory" }
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" { "Your notes" }
                        span class="oa-settings-hint" {
                            "What Coder remembers about you, from computers with sync on. The chat uses them too."
                        }
                    }
                    div class="oa-settings-control" { (action_link("Manage", crate::account_memory::PAGE)) }
                }
            }
            section class="oa-settings-group" aria-labelledby="settings-schedules" {
                h2 #settings-schedules { "Scheduled prompts" }
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" { "Your scheduled prompts" }
                        span class="oa-settings-hint" {
                            "Prompts that run on your computers on a schedule, as a new Coder run or in one of their chats."
                        }
                    }
                    div class="oa-settings-control" { (action_link("Manage", crate::account_schedules::PAGE)) }
                }
            }
            section class="oa-settings-group" aria-labelledby="settings-traces" {
                h2 #settings-traces { "Traces" }
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" { "Your traces" }
                        span class="oa-settings-hint" {
                            "Agent runs you uploaded with coder trace upload. Private unless you share one."
                        }
                    }
                    div class="oa-settings-control" { (action_link("Open", crate::traces::PAGE)) }
                }
            }
            @if let Some((hint, manage)) = claude {
                section class="oa-settings-group" aria-labelledby="settings-claude" {
                    h2 #settings-claude { "Claude" }
                    div class="oa-settings-row" {
                        div class="oa-settings-text" {
                            span class="oa-settings-label" { "Claude Code sign-in" }
                            span class="oa-settings-hint" {
                                "Sign in inside your environment, or add your own Anthropic API key or cloud credential."
                            }
                            span class="oa-settings-hint" { (hint) }
                        }
                        @if manage {
                            div class="oa-settings-control" { (action_link("Manage", CLAUDE)) }
                        }
                    }
                }
            }
            (export_section())
        }
    }
}

/// Export (#11134): everything on the account in one file.
fn export_section() -> Markup {
    html! {
        section class="oa-settings-group" aria-labelledby="settings-export" {
            h2 #settings-export { "Your data" }
            div class="oa-settings-row" {
                div class="oa-settings-text" {
                    span class="oa-settings-label" { "Export everything" }
                    span class="oa-settings-hint" {
                        "One file with your chats, projects, traces, computers, and settings, readable without OpenAgents. Keys and passwords are never in it."
                    }
                }
                div class="oa-settings-control" {
                    (action_link("Download", crate::account_export::PATH))
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

const SUBSCRIPTION_LABEL: &str = "Claude subscription token (from claude setup-token)";

pub(crate) fn material_label(material: Material) -> &'static str {
    match material {
        Material::AnthropicApiKey => "Anthropic API key",
        Material::ClaudeSubscriptionToken => SUBSCRIPTION_LABEL,
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
            "That isn't a valid credential for this provider. Anthropic API keys start with sk-ant-api, and Claude subscription tokens (from claude setup-token) start with sk-ant-oat.",
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

/// Why a credential failed its check with Anthropic. Nothing was kept.
fn check_failure(material: Material, error: byo::CheckError) -> Response {
    let text = match (material, error) {
        (Material::ClaudeSubscriptionToken, byo::CheckError::Refused) => {
            "Anthropic didn't accept that subscription token. Run claude setup-token again and paste the new token. Nothing was saved."
        }
        (_, byo::CheckError::Refused) => {
            "Anthropic didn't accept that API key. Check it at console.anthropic.com and try again. Nothing was saved."
        }
        (_, byo::CheckError::Unreachable) => {
            "Anthropic couldn't be reached to check it, so nothing was saved. Try again in a minute."
        }
    };
    let status = match error {
        byo::CheckError::Refused => StatusCode::BAD_REQUEST,
        byo::CheckError::Unreachable => StatusCode::BAD_GATEWAY,
    };
    problem(status, text, CLAUDE)
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
    // Subscription tokens only for the allowlist until Anthropic approves
    // them in writing (#11235).
    let tokens = app.config.claude_tokens.admits_viewer(&context.viewer);
    let body = claude_content(
        status.map(|status| status.material),
        tokens,
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

/// Where Settings, Claude sends people for Max and Team monthly API
/// credits.
const API_CREDITS: &str =
    "https://support.claude.com/en/articles/17154008-monthly-api-credits-for-max-and-team-plans";
/// Claude Code's one-line install.
pub(crate) const INSTALL: &str = "curl -fsSL https://claude.ai/install.sh | bash";
/// Why a subscription token was not saved for an account off the
/// allowlist (#11235).
const TOKEN_REFUSED: &str = "Subscription tokens can't be saved here. Sign in to Claude inside your environment instead, or save an Anthropic API key. Nothing was saved.";

/// The Claude page (#11235): signing in inside your environment first,
/// through Claude Code itself; then your own key, with the Max and Team
/// monthly API credits; what is saved (never the key itself), a remove
/// button, and the form that adds or replaces it. `tokens` offers a
/// subscription token (`claude setup-token`), for the allowlist only
/// ([`byo::TokenAllow`]).
fn claude_content(
    saved: Option<Material>,
    tokens: bool,
    add: (&str, &str),
    remove: (&str, &str),
) -> Markup {
    let material = Field::new("claude-credential-material", "Provider");
    let value = Field::new("claude-credential-value", "Key")
        .required(true)
        .description(if tokens {
            "Claude subscription token: the token claude setup-token prints (sk-ant-oat…). Anthropic API key: from console.anthropic.com (sk-ant-api…). Either is recognized by how it starts. Bedrock, Vertex, or Foundry: the JSON credential."
        } else {
            "Anthropic API key: from console.anthropic.com (sk-ant-api…). Bedrock, Vertex, or Foundry: the JSON credential."
        });
    let mut providers = Select::new("material").aria(material.aria());
    if tokens {
        providers = providers.option("claude_subscription_token", SUBSCRIPTION_LABEL);
    }
    let providers = providers
        .option("anthropic_api_key", "Anthropic API key")
        .option("bedrock_credential", "Amazon Bedrock")
        .option("vertex_credential", "Google Vertex AI")
        .option("foundry_credential", "Microsoft Foundry");
    let retired_token = !tokens && saved == Some(Material::ClaudeSubscriptionToken);
    html! {
        p { (action_link("Settings", PAGE)) }
        (MarkdownRoot::new(html! {
            h1 { "Claude" }
            h2 #sign-in { "Sign in inside your environment" }
            p {
                "Use your Claude Pro, Max, Team, or Enterprise plan by signing in to Claude Code where it runs. You sign in on Anthropic's own page, and OpenAgents never sees your login. On a plan, Claude Code runs one task at a time."
            }
            ul {
                li {
                    "On your computer: run " code { "claude" } " in a terminal, then type "
                    code { "/login" } ". Coder uses that sign-in. To install Claude Code, run "
                    code { (INSTALL) } "."
                }
                li {
                    "On a Cloud computer: use Sign in to Claude on that computer. Its terminal opens with Claude Code; paste the code Anthropic shows back there. You sign in once per computer."
                }
            }
            h2 #key { "Or use your own key" }
            p {
                "Add your own Anthropic API key, or an Amazon Bedrock, Google Vertex AI, or Microsoft Foundry credential. Usage bills to your own account, and a key can run tasks in parallel. Create an API key at "
                a href="https://console.anthropic.com/settings/keys" { "console.anthropic.com" } "."
            }
            p {
                "On Claude Max or Team? Your plan comes with monthly API credits: $100 on Max 5x, $200 on Max 20x, and on Team a share per seat, pooled up to $500. Claim them into a Claude Console organization and create a key there. It works here like any API key: tasks run in parallel and use those credits, not your plan's limits. "
                a href=(API_CREDITS) { "How to claim them" } "."
            }
            @if tokens {
                p {
                    "You can also save a subscription token: run "
                    code { "claude setup-token" }
                    " in a terminal on your computer and paste the token it prints. It bills your Claude plan and runs one task at a time."
                }
            }
            @match saved {
                Some(material) => {
                    p { "Saved: " (material_label(material)) }
                    p class="oa-page-meta" aria-label="Key hidden" { "••••••••••••••••" }
                    @if retired_token {
                        p role="status" {
                            "Subscription tokens can't be saved here anymore. Yours keeps working until you remove it. To keep using your plan, sign in inside your environment instead."
                        }
                    }
                },
                None => p { "Nothing saved. Without a key, Claude Code runs on the sign-in inside your environment, one task at a time." },
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
            (material.clone().control(providers))
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
    // A pasted subscription token or API key is told apart by its prefix,
    // whichever of the two was picked.
    let material = byo::detect(form.material, &form.value);
    // Subscription tokens only for the allowlist until Anthropic approves
    // them in writing (#11235); the value is dropped unread.
    if material == Material::ClaudeSubscriptionToken
        && !app.config.claude_tokens.admits_viewer(&context.viewer)
    {
        return problem(StatusCode::BAD_REQUEST, TOKEN_REFUSED, CLAUDE);
    }
    let key = match Key::for_material(material, std::mem::take(&mut form.value)) {
        Ok(value) => value,
        Err(error) => return custody_failure(error),
    };
    let consent = form.consent.as_deref() == Some("custody");
    if !consent {
        return custody_failure(CustodyError::Consent);
    }
    if let Err(error) = context.computers.check(material, &key).await {
        return check_failure(material, error);
    }
    match context
        .computers
        .store(&context.owner, material, key, consent, now())
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
        let html = claude_content(None, true, ("t", "r"), ("t", "r")).into_string();
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
        let saved = claude_content(
            Some(Material::BedrockCredential),
            true,
            ("t", "r"),
            ("t", "r"),
        )
        .into_string();
        assert!(saved.contains("Saved: Amazon Bedrock"));
        assert!(saved.contains("action=\"/settings/claude/remove\""));
        assert!(byo::TERMS.contains(
            "never put in a checkpoint, saved environment image, export, log, or evidence"
        ));
        // Both Anthropic kinds have their own clear label.
        assert!(html.contains("value=\"claude_subscription_token\""));
        assert!(html.contains("Claude subscription token (from claude setup-token)"));
        assert!(html.contains(">Anthropic API key<"));
        assert!(html.contains("claude setup-token"));
        let token = claude_content(
            Some(Material::ClaudeSubscriptionToken),
            true,
            ("t", "r"),
            ("t", "r"),
        )
        .into_string();
        assert!(token.contains("Saved: Claude subscription token (from claude setup-token)"));
        assert!(!token.contains("can't be saved here anymore"));
    }

    #[test]
    fn the_claude_page_leads_with_signing_in_inside_your_environment() {
        let html = claude_content(None, false, ("t", "r"), ("t", "r")).into_string();
        // Sign-in first, through Claude Code itself (#11235).
        let sign_in = html.find("Sign in inside your environment").unwrap();
        let key = html.find("Or use your own key").unwrap();
        let form = html.find("<form").unwrap();
        assert!(sign_in < key && key < form, "{html}");
        for needle in [
            "<code>claude</code>",
            "<code>/login</code>",
            INSTALL,
            "Sign in to Claude on that computer",
            "OpenAgents never sees your login",
        ] {
            assert!(html.contains(needle), "{needle}");
        }
        // The Max and Team monthly API credits, used through a Console key.
        for needle in [
            "monthly API credits",
            "$100 on Max 5x",
            "$200 on Max 20x",
            "pooled up to $500",
            "Claude Console organization",
            API_CREDITS,
        ] {
            assert!(html.contains(needle), "{needle}");
        }
        let text = oa_copy::visible_text(&html);
        assert_eq!(oa_copy::violations(&text, &[]), vec![], "{text}");
    }

    #[test]
    fn subscription_tokens_show_only_for_the_allowlist() {
        let off = claude_content(None, false, ("t", "r"), ("t", "r")).into_string();
        assert!(!off.contains("claude_subscription_token"), "{off}");
        assert!(!off.contains("setup-token"), "{off}");
        assert!(!off.contains("sk-ant-oat"), "{off}");
        assert!(off.contains("value=\"anthropic_api_key\""));
        let on = claude_content(None, true, ("t", "r"), ("t", "r")).into_string();
        assert!(on.contains("value=\"claude_subscription_token\""));
        assert!(on.contains("<code>claude setup-token</code>"));
        // A token saved before keeps working, with a notice to switch.
        let kept = claude_content(
            Some(Material::ClaudeSubscriptionToken),
            false,
            ("t", "r"),
            ("t", "r"),
        )
        .into_string();
        assert!(kept.contains("Saved: Claude subscription token"));
        assert!(kept.contains("Yours keeps working until you remove it."));
        assert!(kept.contains("action=\"/settings/claude/remove\""));
        assert!(!kept.contains("value=\"claude_subscription_token\""));
        let text = oa_copy::visible_text(&kept);
        assert_eq!(oa_copy::violations(&text, &[]), vec![], "{text}");
    }

    #[test]
    fn a_check_failure_never_repeats_the_credential_and_says_nothing_was_saved() {
        for material in [Material::AnthropicApiKey, Material::ClaudeSubscriptionToken] {
            for error in [byo::CheckError::Refused, byo::CheckError::Unreachable] {
                let response = check_failure(material, error);
                assert!(response.status().is_client_error() || response.status().is_server_error());
            }
        }
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
        // The export is always offered (#11134).
        assert!(html.contains("href=\"/settings/export\""), "{html}");
        for needle in [
            ">Settings<",
            ">Profile<",
            ">Theme<",
            ">Ada<",
            ">Export everything<",
        ] {
            assert!(html.contains(needle), "{needle}");
        }
        let text = oa_copy::visible_text(&html);
        assert_eq!(oa_copy::violations(&text, &[]), vec![], "{text}");
    }
}
