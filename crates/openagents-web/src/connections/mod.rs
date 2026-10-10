//! Connections (#11238): third-party software connected to the account,
//! Google Drive first.
//!
//! - `/settings/connections` lists the connected services, with Connect
//!   and Remove. Connecting Google runs OAuth through the Google Cloud
//!   OAuth client in `openagentsgemini` ([`google`]): `drive.readonly` and
//!   `spreadsheets.readonly`, with incremental consent, and PKCE. Google
//!   returns to `/auth/google/callback`; because the session cookie is
//!   `SameSite=Strict`, that page continues with a same-site step to
//!   `/auth/google/finish`, which trades the code for tokens. The refresh
//!   token is sealed per account ([`crate::cloud::connections`]).
//! - `/projects/{id}/sources` attaches Drive folders and files to a
//!   project, from a picker or a pasted link ([`sources`]).
//! - The read-only Drive tools ([`oa_connections::google::drive`]) run
//!   here, in trusted code, with an access token refreshed from the sealed
//!   one ([`live`]). The web chat uses them in a project with sources
//!   ([`chat`]); Coder uses them through `POST
//!   /v1/connections/google/tools/{tool}` under its own sign-in.
//!   Every call is checked against the connection's grant and the tool's
//!   policy ([`oa_connections::effective_policy`]) first.

pub(crate) mod chat;
pub(crate) mod sources;
#[cfg(test)]
mod tests;

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use maud::{Markup, html};
use oa_connections::core::{Connection, Policy, Scope};
use oa_connections::google::drive::{self, Call, Drive, PdfText};
use oa_connections::google::oauth::{self, OAuthError, Pkce};
use oa_connections::google::{self as google_api, DRIVE_READONLY, Endpoints, SHEETS_READONLY};
use openagents_ui::actions::{Alert, Badge, Button, ButtonLink, ButtonType, ButtonVariant, Color};
use openagents_ui::content::MarkdownRoot;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Sha256;

use crate::App;
use crate::chat_store::account_owner;
use crate::cloud::connections::Stored;
use crate::cloud::session::{CloudSession, SessionError, Viewer, now};
use crate::cloud::{protect, refused};
use crate::ui_page::action_link;

/// The settings page.
pub(crate) const PAGE: &str = "/settings/connections";
const REMOVE: &str = "/settings/connections/google/remove";
/// Where connecting Google starts (`?return_to=`, `?scope=sheets`).
pub(crate) const CONNECT: &str = "/auth/google/connect";
const CALLBACK: &str = "/auth/google/callback";
const FINISH: &str = "/auth/google/finish";
/// The app API: the account's connections.
const API: &str = "/v1/connections";
/// The app API: run one Google tool.
const API_TOOL: &str = "/v1/connections/google/tools/{tool}";
const CSRF_SCOPE: &str = "connections";
const FLOW_COOKIE: &str = "oa_google_flow";
/// How long a trip to Google may take.
const FLOW_SECONDS: u64 = 600;
/// The connection's name; one Google account per OpenAgents account for now.
pub(crate) const DEFAULT: &str = "default";
/// The OAuth client's JSON, for a laptop or a test server.
pub(crate) const OAUTH_ENV: &str = "OPENAGENTS_WEB_GOOGLE_OAUTH_JSON";
/// The Secret Manager secret holding the OAuth client's JSON (the file
/// Google Cloud's console downloads), read with the server's Google
/// credential when [`OAUTH_ENV`] is unset.
pub(crate) const SECRET: &str = "openagents-web-google-oauth";
const SECRET_ENV: &str = "OPENAGENTS_WEB_GOOGLE_OAUTH_SECRET";
const GCP_PROJECT: &str = "openagentsgemini";
/// How often a server without the client looks for it again.
const LOOK_AGAIN: Duration = Duration::from_secs(300);

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(page))
        .route(REMOVE, post(remove))
        .route(CONNECT, get(connect))
        .route(CALLBACK, get(callback))
        .route(FINISH, get(finish))
        .route(API, get(api_list))
        .route(API_TOOL, post(api_tool))
        .merge(sources::routes())
}

/// Whether `path` is one of the app API's paths here.
pub(crate) fn owns(path: &str) -> bool {
    path == API || path.starts_with("/v1/connections/")
}

/// Google as this server reaches it: the OAuth client and endpoints.
pub struct Google {
    pub client: oauth::Client,
    pub endpoints: Endpoints,
    pub(crate) http: reqwest::Client,
}

impl Google {
    #[must_use]
    pub fn new(client: oauth::Client, endpoints: Endpoints) -> Self {
        Self {
            client,
            endpoints,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap_or_default(),
        }
    }
}

/// The Google OAuth client: the configured one, else found once in
/// [`OAUTH_ENV`] or Secret Manager ([`SECRET`]) and kept; a server without
/// it looks again every few minutes, so adding the secret needs no
/// restart.
pub(crate) async fn google(app: &App) -> Option<Arc<Google>> {
    if let Some(google) = &app.config.google {
        return Some(google.clone());
    }
    static FOUND: OnceLock<tokio::sync::Mutex<(Option<Arc<Google>>, Option<Instant>)>> =
        OnceLock::new();
    let mut found = FOUND
        .get_or_init(|| tokio::sync::Mutex::new((None, None)))
        .lock()
        .await;
    if let Some(google) = &found.0 {
        return Some(google.clone());
    }
    if found.1.is_some_and(|at| at.elapsed() < LOOK_AGAIN) {
        return None;
    }
    found.1 = Some(Instant::now());
    let text = match std::env::var(OAUTH_ENV) {
        Ok(text) if !text.trim().is_empty() => Some(text),
        _ => {
            let source = inference::upstream::google::TokenSource::from_env();
            let name = std::env::var(SECRET_ENV).unwrap_or_else(|_| SECRET.to_owned());
            match inference::upstream::google::access_secret(&source, GCP_PROJECT, &name).await {
                Ok(secret) => secret.map(|s| s.expose().to_owned()),
                Err(why) => {
                    eprintln!("openagents-web: connections: {why}");
                    None
                }
            }
        }
    }?;
    match oauth::Client::parse(&text) {
        Ok(client) => {
            let google = Arc::new(Google::new(client, Endpoints::default()));
            found.0 = Some(google.clone());
            println!("Google can be connected in Settings, Connections");
            Some(google)
        }
        Err(why) => {
            eprintln!("openagents-web: connections: {why}");
            None
        }
    }
}

/// Why a person's Google connection can't be used now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unready {
    /// This server keeps no connections or has no Google client.
    Off,
    NotConnected,
    /// Google stopped accepting it.
    Reconnect,
    Unavailable,
}

impl Unready {
    pub(crate) fn words(self) -> &'static str {
        match self {
            Self::Off => "Google can't be connected on this server yet.",
            Self::NotConnected => "Connect Google in Settings, Connections first.",
            Self::Reconnect => {
                "Google no longer accepts your connection. Connect Google again in Settings, Connections."
            }
            Self::Unavailable => "Google couldn't be reached. Try again in a minute.",
        }
    }
}

/// A usable connection: Drive with a fresh access token, and the
/// connection as the core sees it.
pub(crate) struct Live {
    pub drive: Drive,
    pub connection: Connection,
    pub owner: String,
}

/// `owner`'s Google connection, ready to call Drive.
pub(crate) async fn live(app: &App, owner: &str) -> Result<Live, Unready> {
    let store = app.config.connections.as_deref().ok_or(Unready::Off)?;
    let google = google(app).await.ok_or(Unready::Off)?;
    let account = store.load(owner).map_err(|_| Unready::Unavailable)?;
    let stored = account
        .connection(google_api::SLUG, DEFAULT)
        .ok_or(Unready::NotConnected)?
        .clone();
    if stored.reconnect {
        return Err(Unready::Reconnect);
    }
    // A fresh access token for each request, never kept after it
    // (docs/security/sensitive-data-vault.md: per-request access).
    let token = match oauth::refresh(
        &google.http,
        &google.endpoints,
        &google.client,
        &stored.refresh_token,
    )
    .await
    {
        Ok(access) => access.token,
        Err(OAuthError::Refused) => {
            let _ = store.update(owner, |account| {
                for c in &mut account.connections {
                    if c.integration == google_api::SLUG && c.name == DEFAULT {
                        c.reconnect = true;
                    }
                }
            });
            return Err(Unready::Reconnect);
        }
        Err(OAuthError::Unreachable) => return Err(Unready::Unavailable),
    };
    Ok(Live {
        drive: Drive::new(
            google.http.clone(),
            google.endpoints.clone(),
            token,
            drive::has_sheets(&stored.granted_scopes),
        ),
        connection: stored.connection(owner),
        owner: owner.to_owned(),
    })
}

/// What one tool call did: its result, and the files it read and listed.
pub(crate) type Outcome = drive::Outcome;

/// Run the Google tool `name` with `arguments` through `live`, after
/// checking the grant and the tool's policy.
pub(crate) async fn run_tool(
    live: &Live,
    name: &str,
    arguments: Value,
    pdf: Option<&PdfText>,
) -> Result<Outcome, String> {
    let integration = google_api::integration(&Endpoints::default());
    let spec = integration
        .tool(name)
        .ok_or_else(|| format!("There is no Google tool named {name}."))?;
    if !live.connection.grants(spec) {
        return Err("Google didn't grant the access this needs. Connect Google again.".into());
    }
    match oa_connections::effective_policy(
        &live.connection,
        spec,
        &[Scope::account(&live.owner)],
        &[],
    ) {
        Policy::Allow => {}
        Policy::RequireApproval => {
            return Err("That changes your files, which needs your approval first.".into());
        }
        Policy::Block => return Err("That tool is turned off.".into()),
    }
    let call = Call::parse(name, arguments)?;
    match drive::run(&live.drive, &call, pdf).await {
        Ok(outcome) => Ok(outcome),
        Err(error) => Err(error.to_string()),
    }
}

// Settings ------------------------------------------------------------------

/// The Settings row that opens this page.
pub(crate) fn settings_row(app: &App) -> Markup {
    if app.config.connections.is_none() {
        return html! {};
    }
    html! {
        section class="oa-settings-group" aria-labelledby="settings-connections" {
            h2 #settings-connections { "Connections" }
            div class="oa-settings-row" {
                div class="oa-settings-text" {
                    span class="oa-settings-label" { "Connected services" }
                    span class="oa-settings-hint" {
                        "Google Drive and other services your chats and Coder can read from."
                    }
                }
                div class="oa-settings-control" { (action_link("Manage", PAGE)) }
            }
        }
    }
}

#[derive(Deserialize, Default)]
struct PageQuery {
    #[serde(default)]
    problem: Option<String>,
}

async fn page(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> Response {
    let (service, viewer) = match crate::settings::viewer(&app, &headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let body = view(&app, service, &headers, &viewer, query.problem.as_deref()).await;
    crate::settings::page(&headers, service, &viewer, "Connections", PAGE, body)
}

async fn view(
    app: &App,
    service: &CloudSession,
    headers: &HeaderMap,
    viewer: &Viewer,
    problem: Option<&str>,
) -> Markup {
    let owner = account_owner(&viewer.account_id);
    let store = app.config.connections.as_deref();
    let google = google(app).await;
    let account = store.map(|store| store.load(&owner));
    let csrf = service
        .csrf(headers, viewer, CSRF_SCOPE, &viewer.account_id)
        .unwrap_or_default();
    let stored = account
        .as_ref()
        .and_then(|a| a.as_ref().ok())
        .and_then(|a| a.connection(google_api::SLUG, DEFAULT).cloned());
    let sources = account
        .as_ref()
        .and_then(|a| a.as_ref().ok())
        .map_or(0, |a| a.sources.len());
    let problem = match problem {
        Some("declined") => Some("Google wasn't connected. Connect again when you're ready."),
        Some("expired") => Some("That Google connection expired. Start again from this browser."),
        Some("failed") => Some("Google couldn't be connected right now. Try again in a minute."),
        Some("storage") => Some("Your connection couldn't be saved. Try again later."),
        _ => None,
    };
    html! {
        div class="oa-settings" {
            h1 class="oa-heading" data-level="1" { "Connections" }
            @if let Some(problem) = problem {
                (Alert::new().color(Color::Danger).description(problem))
            }
            section class="oa-settings-group" aria-labelledby="connections-google" {
                h2 #connections-google { "Google Drive" }
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        @match (&store, &stored) {
                            (None, _) => {
                                span class="oa-settings-label" { "Not available" }
                                span class="oa-settings-hint" { "This server can't keep connections yet." }
                            }
                            (_, Some(stored)) => {
                                span class="oa-settings-label" {
                                    "Connected"
                                    @if let Some(email) = &stored.identity { " as " (email) }
                                    @if stored.reconnect { " " (Badge::new("Connect again")) }
                                }
                                span class="oa-settings-hint" {
                                    @if drive::has_sheets(&stored.granted_scopes) {
                                        "Reads your Drive files and every tab of your Sheets. It never changes them."
                                    } @else {
                                        "Reads your Drive files. It never changes them. Allow Sheets to read every tab of a spreadsheet."
                                    }
                                }
                                @if sources > 0 {
                                    span class="oa-settings-hint" {
                                        (sources) @if sources == 1 { " file or folder is" } @else { " files and folders are" }
                                        " attached to your projects."
                                    }
                                }
                            }
                            (Some(_), None) => {
                                span class="oa-settings-label" { "Not connected" }
                                span class="oa-settings-hint" {
                                    "Connect Google so chats in your projects can read the Drive folders and files you attach. Read only."
                                }
                            }
                        }
                    }
                    div class="oa-settings-control" {
                        @if store.is_some() && google.is_some() {
                            @match &stored {
                                Some(stored) => {
                                    @if stored.reconnect {
                                        (ButtonLink::new("Connect again", CONNECT))
                                    } @else if !drive::has_sheets(&stored.granted_scopes) {
                                        (action_link("Allow Sheets", &format!("{CONNECT}?scope=sheets")))
                                    }
                                    form method="post" action=(REMOVE) {
                                        input type="hidden" name="csrf" value=(csrf);
                                        (Button::new("Remove")
                                            .kind(ButtonType::Submit)
                                            .variant(ButtonVariant::Ghost)
                                            .color(Color::Secondary))
                                    }
                                }
                                None => { (ButtonLink::new("Connect", CONNECT)) }
                            }
                        }
                    }
                }
                @if store.is_some() && google.is_none() {
                    (MarkdownRoot::new(html! { p { "Google can't be connected on this server yet." } }))
                }
            }
            (MarkdownRoot::new(html! {
                p { "Attach Drive folders and files to a project from its page in " a href=(crate::projects::PAGE) { "Projects" } "." }
            }))
        }
    }
}

#[derive(Deserialize)]
struct CsrfForm {
    csrf: String,
}

async fn remove(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form): Form<CsrfForm>,
) -> Response {
    let (service, viewer) = match crate::settings::viewer(&app, &headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        CSRF_SCOPE,
        &viewer.account_id,
        &form.csrf,
    ) {
        return refused(error);
    }
    let Some(store) = app.config.connections.as_deref() else {
        return refused(SessionError::Unavailable);
    };
    let owner = account_owner(&viewer.account_id);
    let removed = store.update(&owner, |account| {
        let token = account
            .connection(google_api::SLUG, DEFAULT)
            .map(|c| c.refresh_token.clone());
        account
            .connections
            .retain(|c| !(c.integration == google_api::SLUG && c.name == DEFAULT));
        token
    });
    match removed {
        Ok(token) => {
            if let (Some(token), Some(google)) = (token, google(&app).await) {
                oauth::revoke(&google.http, &google.endpoints, &token).await;
            }
            protect(Redirect::to(PAGE).into_response())
        }
        Err(_) => protect(Redirect::to(&format!("{PAGE}?problem=storage")).into_response()),
    }
}

// The trip to Google ----------------------------------------------------------

/// What the flow cookie carries across the trip, signed with this server's
/// key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Flow {
    state: String,
    verifier: String,
    return_to: String,
    owner: String,
    issued: u64,
}

fn flow_mac(app: &App, payload: &[u8]) -> Hmac<Sha256> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(&app.config.ask_salt).expect("HMAC accepts 32 bytes");
    mac.update(b"openagents.web.google-flow.v1:");
    mac.update(payload);
    mac
}

fn seal_flow(app: &App, flow: &Flow) -> String {
    let payload = serde_json::to_vec(flow).expect("a flow serializes");
    let tag = flow_mac(app, &payload).finalize().into_bytes();
    format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(&payload),
        URL_SAFE_NO_PAD.encode(tag)
    )
}

fn open_flow(app: &App, value: &str) -> Option<Flow> {
    let (payload, tag) = value.split_once('.')?;
    let payload = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let tag = URL_SAFE_NO_PAD.decode(tag).ok()?;
    flow_mac(app, &payload).verify_slice(&tag).ok()?;
    serde_json::from_slice(&payload).ok()
}

fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.to_owned())
}

fn flow_cookie(app: &App, value: &str, max_age: u64) -> HeaderValue {
    let secure = if app.config.secure_cookies {
        "; Secure"
    } else {
        ""
    };
    HeaderValue::from_str(&format!(
        "{FLOW_COOKIE}={value}; Path=/auth/google; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}"
    ))
    .expect("a cookie is a header value")
}

/// A same-site path to come back to, else Settings, Connections.
fn safe_return(value: Option<&str>) -> String {
    value
        .filter(|v| {
            v.starts_with('/') && !v.starts_with("//") && !v.contains('\\') && v.len() <= 300
        })
        .unwrap_or(PAGE)
        .to_owned()
}

fn redirect_uri(service: &CloudSession) -> String {
    format!("{}{CALLBACK}", service.origin().trim_end_matches('/'))
}

#[derive(Deserialize)]
struct ConnectQuery {
    #[serde(default)]
    return_to: Option<String>,
    /// `sheets`: ask only for Sheets (incremental consent).
    #[serde(default)]
    scope: Option<String>,
}

async fn connect(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<ConnectQuery>,
) -> Response {
    let (service, viewer) = match crate::settings::viewer(&app, &headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Some(google) = google(&app)
        .await
        .filter(|_| app.config.connections.is_some())
    else {
        return protect(crate::layout::problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "Connections",
            Unready::Off.words(),
            (PAGE, "Connections"),
        ));
    };
    let pkce = Pkce::new();
    let state: String = secp256k1::rand::random::<[u8; 16]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let flow = Flow {
        state: state.clone(),
        verifier: pkce.verifier.clone(),
        return_to: safe_return(query.return_to.as_deref()),
        owner: account_owner(&viewer.account_id),
        issued: now(),
    };
    let scopes: &[&str] = if query.scope.as_deref() == Some("sheets") {
        &[SHEETS_READONLY]
    } else {
        &[DRIVE_READONLY, SHEETS_READONLY]
    };
    let url = oauth::authorize_url(
        &google.endpoints,
        &google.client,
        &redirect_uri(service),
        scopes,
        &state,
        &pkce.challenge,
    );
    let mut response = protect(Redirect::to(&url).into_response());
    response.headers_mut().append(
        header::SET_COOKIE,
        flow_cookie(&app, &seal_flow(&app, &flow), FLOW_SECONDS),
    );
    response
}

#[derive(Deserialize)]
struct CallbackQuery {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// Google's callback: continue with a same-site step so the next request
/// carries the `SameSite=Strict` session cookie.
async fn callback(headers: HeaderMap, Query(query): Query<CallbackQuery>) -> Response {
    let mut next = format!(
        "{FINISH}?state={}",
        encode(query.state.as_deref().unwrap_or_default())
    );
    if let Some(code) = &query.code {
        next.push_str(&format!("&code={}", encode(code)));
    }
    if let Some(error) = &query.error {
        next.push_str(&format!("&error={}", encode(error)));
    }
    let content = openagents_ui::content::PageColumn::new(html! {
        (MarkdownRoot::new(html! {
            h1 { "Connecting Google" }
            p { a href=(next) { "Continue" } }
        }))
    });
    protect(
        crate::ui_page::UiPage::new("Connecting Google")
            .scriptless()
            .head(html! { meta http-equiv="refresh" content=(format!("0;url={next}")); })
            .content(content)
            .respond(&headers),
    )
}

async fn finish(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let mut response = finished(&app, &headers, query).await;
    response
        .headers_mut()
        .append(header::SET_COOKIE, flow_cookie(&app, "", 0));
    response
}

fn back_with(problem: &str) -> Response {
    protect(Redirect::to(&format!("{PAGE}?problem={problem}")).into_response())
}

async fn finished(app: &App, headers: &HeaderMap, query: CallbackQuery) -> Response {
    let (service, viewer) = match crate::settings::viewer(app, headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let flow = cookie(headers, FLOW_COOKIE)
        .and_then(|value| open_flow(app, &value))
        .filter(|flow| {
            flow.owner == owner
                && Some(flow.state.as_str()) == query.state.as_deref()
                && now().saturating_sub(flow.issued) <= FLOW_SECONDS
        });
    let Some(flow) = flow else {
        return back_with("expired");
    };
    if query.error.is_some() {
        return back_with("declined");
    }
    let (Some(code), Some(google), Some(store)) = (
        query.code.as_deref(),
        google(app).await,
        app.config.connections.as_deref(),
    ) else {
        return back_with("failed");
    };
    let grant = match oauth::exchange(
        &google.http,
        &google.endpoints,
        &google.client,
        &redirect_uri(service),
        code,
        &flow.verifier,
    )
    .await
    {
        Ok(grant) => grant,
        Err(_) => return back_with("failed"),
    };
    if !grant.scopes.iter().any(|s| s == DRIVE_READONLY) {
        // The person unticked Drive on Google's page.
        return back_with("declined");
    }
    let saved = store.update(&owner, |account| {
        let earlier = account.connection(google_api::SLUG, DEFAULT).cloned();
        let refresh = grant
            .refresh_token
            .clone()
            .or_else(|| earlier.as_ref().map(|c| c.refresh_token.clone()))?;
        account
            .connections
            .retain(|c| !(c.integration == google_api::SLUG && c.name == DEFAULT));
        account.connections.push(Stored {
            integration: google_api::SLUG.into(),
            name: DEFAULT.into(),
            identity: grant
                .email
                .clone()
                .or_else(|| earlier.and_then(|c| c.identity)),
            granted_scopes: grant.scopes.clone(),
            refresh_token: refresh,
            connected_at: now(),
            reconnect: false,
        });
        Some(())
    });
    match saved {
        Ok(Some(())) => protect(Redirect::to(&flow.return_to).into_response()),
        Ok(None) => back_with("failed"),
        Err(_) => back_with("storage"),
    }
}

// The app API --------------------------------------------------------------

/// The signed-in app's account owner, or its JSON refusal. Only an app's
/// own token (`Authorization: Bearer sess_...`) is accepted.
async fn app_owner(app: &App, headers: &HeaderMap) -> Result<String, Response> {
    let Some(token) = crate::account_export::app_token(headers) else {
        return Err(crate::coder_sync::refused(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Sign in with coder login.",
        ));
    };
    let (_, viewer) = crate::account_export::app_viewer(app, token).await?;
    Ok(account_owner(&viewer.account_id))
}

async fn api_list(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = match app_owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let available = app.config.connections.is_some() && google(&app).await.is_some();
    let account = app
        .config
        .connections
        .as_deref()
        .and_then(|store| store.load(&owner).ok())
        .unwrap_or_default();
    let integration = google_api::integration(&Endpoints::default());
    let connections: Vec<Value> = account
        .connections
        .iter()
        .map(|stored| {
            let connection = stored.connection(&owner);
            let tools: Vec<Value> = integration
                .tools
                .iter()
                .filter(|tool| connection.grants(tool))
                .map(|tool| {
                    json!({
                        "id": tool.id,
                        "address": connection.address(tool).to_string(),
                        "policy": oa_connections::effective_policy(&connection, tool, &[Scope::account(&owner)], &[]),
                        "description": tool.description,
                        "parameters": tool.input_schema,
                    })
                })
                .collect();
            json!({
                "integration": stored.integration,
                "name": stored.name,
                "identity": stored.identity,
                "reconnect": stored.reconnect,
                "tools": tools,
            })
        })
        .collect();
    let sources: Vec<Value> = account
        .sources
        .iter()
        .map(|s| json!({"project": s.project, "id": s.id, "name": s.name, "kind": s.kind, "link": s.link()}))
        .collect();
    axum::Json(json!({"available": available, "connections": connections, "sources": sources}))
        .into_response()
}

async fn api_tool(
    State(app): State<App>,
    headers: HeaderMap,
    Path(tool): Path<String>,
    body: axum::body::Bytes,
) -> Response {
    let owner = match app_owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let arguments: Value = match serde_json::from_slice(&body) {
        Ok(value @ Value::Object(_)) => value,
        _ => {
            return crate::coder_sync::refused(
                StatusCode::BAD_REQUEST,
                "invalid",
                "Send the tool's arguments as a JSON object.",
            );
        }
    };
    let live = match live(&app, &owner).await {
        Ok(live) => live,
        Err(unready) => {
            let status = match unready {
                Unready::Off => StatusCode::SERVICE_UNAVAILABLE,
                Unready::NotConnected | Unready::Reconnect => StatusCode::CONFLICT,
                Unready::Unavailable => StatusCode::BAD_GATEWAY,
            };
            return crate::coder_sync::refused(status, "not_connected", unready.words());
        }
    };
    let pdf = chat::pdf_reader(&app);
    match run_tool(&live, &tool, arguments, pdf.as_ref()).await {
        Ok(outcome) => axum::Json(json!({
            "result": outcome.result,
            "read": outcome.read.iter().map(|f| json!({"name": f.name, "link": f.link()})).collect::<Vec<_>>(),
        }))
        .into_response(),
        Err(message) => crate::coder_sync::refused(StatusCode::UNPROCESSABLE_ENTITY, "tool_failed", &message),
    }
}
