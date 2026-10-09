//! Same-origin Cloud pages over current native account authority. The public
//! site, local task browser, and separately granted services remain distinct.

mod agents;
mod billing;
pub mod byo;
pub(crate) mod composer;
mod controls;
pub mod custody;
mod effects;
mod environment;
pub mod hosts;
mod operator;
mod partners;
mod private;
pub mod retail;
pub mod sales;
pub mod session;
pub mod team;
#[cfg(test)]
mod tests;
mod ui;
mod verse;
mod work;
mod workbench;

use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{DefaultBodyLimit, Form, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Json, Redirect, Response};
use axum::routing::{get, post};
use coder_ui::workspace;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::App;
use crate::layout::{document, escape, problem};
use session::{CloudSession, SessionError, Viewer};

const POLICY: &str = "default-src 'none'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";
const SECTIONS: [(&str, &str, &str); 12] = [
    (
        "Projects",
        "projects",
        "Connect a host with project observation rights.",
    ),
    (
        "Agents",
        "agents",
        "Connect an admitted agent or Studio host.",
    ),
    (
        "Computers",
        "computers",
        "Enroll a computer explicitly; account membership supplies no host grant.",
    ),
    ("Workbench", "workbench", "Connect a granted host session."),
    (
        "Verse",
        "verse",
        "World, computer, and private work need separate connections.",
    ),
    (
        "Plugins",
        "plugins",
        "Admit an exact plugin release and its native purchase authority.",
    ),
    (
        "Team",
        "team",
        "Browser team controls require their own browser qualification.",
    ),
    (
        "Billing",
        "billing",
        "Select a workspace to read its original statements and admitted billing lanes.",
    ),
    (
        "Sales",
        "sales",
        "Private sales access requires a separate sales-owner grant.",
    ),
    (
        "Partners",
        "partners",
        "Accepted assignments and original payee authority are required.",
    ),
    (
        "Settings",
        "settings",
        "Current account and session standing.",
    ),
    (
        "Tasks",
        "tasks",
        "Connect canonical resident task observation.",
    ),
];

pub(crate) fn routes() -> Router<App> {
    let mut router = Router::new()
        .route("/cloud/sign-in", get(sign_in).post(sign_in_submit))
        .route("/cloud/sign-out", post(sign_out))
        .route("/cloud/select-workspace", post(select_workspace))
        .route("/cloud/app", get(overview))
        .route("/cloud/app/session", get(standing))
        .route("/cloud/assets/{file}", get(asset))
        .route("/cloud/app/tasks/{id}", get(work::task_alias))
        .merge(work::routes())
        .merge(controls::routes())
        .merge(operator::routes())
        .merge(environment::routes())
        .merge(workbench::routes())
        .merge(verse::routes())
        .merge(agents::routes())
        .merge(retail::routes())
        .merge(sales::routes())
        .merge(billing::routes())
        .merge(partners::routes())
        .merge(byo::routes())
        .merge(team::routes())
        .layer(DefaultBodyLimit::max(8192));
    for (_, slug, _) in SECTIONS {
        if !matches!(
            slug,
            "agents"
                | "computers"
                | "projects"
                | "workbench"
                | "verse"
                | "billing"
                | "partners"
                | "team"
                | "sales"
        ) {
            router = router.route(&format!("/cloud/app/{slug}"), get(section));
        }
    }
    router
}

pub(crate) fn ready(app: &App) -> bool {
    app.config.cloud.is_some()
        && app.config.cloud_build.as_ref().is_some_and(|dir| {
            dir.join("coder_cloud_web.js").is_file()
                && dir.join("coder_cloud_web_bg.wasm").is_file()
        })
}

fn service(app: &App) -> Result<&CloudSession, Response> {
    if !ready(app) {
        return Err(failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Workspace unavailable",
            "This server has no qualified browser account connection. You can explore the components or use the OpenAgents apps.",
        ));
    }
    let service = app
        .config
        .cloud
        .as_ref()
        .expect("ready checks configuration");
    service.health().map_err(|_| failure(StatusCode::SERVICE_UNAVAILABLE, "Account connection unavailable", "The account configuration changed or is unavailable. The operator must reload its explicit configuration."))?;
    Ok(service)
}

pub(crate) fn protect(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    // Native forms retain an exact Origin, while outside destinations receive
    // no referrer from these private pages.
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(POLICY),
    );
    headers.insert(header::VARY, HeaderValue::from_static("Cookie"));
    response
}

/// The reconnect answer of an observation stream that can no longer be
/// admitted. A browser `EventSource` treats an error status as a failed
/// connection and `htmx-sse` keeps re-creating it, so the page would go on
/// saying it observes. A reconnect therefore receives one `retire` event,
/// which every Cloud observer closes on (`sse-close="retire"`), and the
/// page shows that observation stopped. It carries no private record.
pub(crate) fn retired_stream(message: &'static str) -> Response {
    let event = Event::default()
        .event("retire")
        .data(format!("<p>{message}</p>"));
    let mut response = protect(
        Sse::new(futures_util::stream::once(async move {
            Ok::<_, std::convert::Infallible>(event)
        }))
        .into_response(),
    );
    response
        .headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    response
}

/// Whether this request is a browser `EventSource` reconnect: one that
/// carries the last event id it saw.
pub(crate) fn reconnect(headers: &HeaderMap) -> bool {
    headers.contains_key("last-event-id")
}

fn failure(status: StatusCode, title: &str, message: &str) -> Response {
    protect(problem(
        status,
        title,
        message,
        ("/cloud", "Cloud overview"),
    ))
}

fn refused(error: SessionError) -> Response {
    let status = match error {
        SessionError::Unauthenticated => StatusCode::UNAUTHORIZED,
        SessionError::Forbidden | SessionError::Csrf => StatusCode::FORBIDDEN,
        SessionError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        SessionError::InvalidRequest => StatusCode::BAD_REQUEST,
        SessionError::Conflict => StatusCode::CONFLICT,
    };
    failure(status, "Account request refused", &error.to_string())
}

fn colors() -> workspace::Palette {
    let color = |value| {
        rust_native::style::Color::rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
    };
    workspace::Palette {
        text: color(coder_ui::coder_noir::CONTENT),
        heading: color(coder_ui::coder_noir::CONTENT),
        secondary: color(coder_ui::coder_noir::CONTENT_SECONDARY),
        border: color(coder_ui::coder_noir::STROKE_SUBTLE),
    }
}

fn render(view: &rust_native::View<workspace::WorkspaceIntent>) -> Result<String, Response> {
    rust_native_web::render_view(view).map_err(|_| {
        failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "View unavailable",
            "The shared workspace view could not be validated.",
        )
    })
}

fn ticket(token: &str) -> String {
    format!(
        "<input type=\"hidden\" name=\"csrf\" value=\"{}\">",
        escape(token)
    )
}

fn page(body: &str) -> Response {
    let body = format!(
        "<div class=\"cloud\">{body}</div><script type=\"module\" src=\"/cloud/assets/start.js\"></script>"
    );
    let html = document("Workspace", Some("/cloud"), &body).replace(
        "</head>",
        &format!(
            "{}<link rel=\"stylesheet\" href=\"/cloud/assets/cloud.css\"><link rel=\"stylesheet\" href=\"/cloud/assets/native.css\"></head>",
            crate::chat_html::head().into_string()
        ),
    );
    protect(axum::response::Html(html).into_response())
}

async fn sign_in(State(app): State<App>, headers: HeaderMap) -> Response {
    let service = match service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Ok(viewer) = service.authenticate(&headers).await {
        let cookies = match service.refresh_cookies(&headers, &viewer) {
            Ok(value) => value,
            Err(error) => return refused(error),
        };
        let mut response = protect(Redirect::to("/cloud/app").into_response());
        for cookie in cookies {
            response.headers_mut().append(header::SET_COOKIE, cookie);
        }
        return response;
    }
    let csrf = match service.login_csrf(&headers, "sign-in", "") {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let field = match render(&workspace::sign_in(colors())) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mut response = page(&format!(
        "<h1>Sign in to your workspace</h1><p>Use an existing native account API key. The selected account service issues a revocable session. This form creates no computer, execution, sales, or spending grant.</p><form method=\"post\" action=\"/cloud/sign-in\">{}{field}<p><button type=\"submit\">Sign in</button></p></form><p class=\"dim\">Account creation and recovery remain with the native account owner. After recovery or key rotation, sign in with the new key.{}</p>",
        ticket(&csrf.token),
        if team::lane(&app, team::Lane::Recovery) {
            " <a href=\"/cloud/recover\">Redeem a recovery token</a>"
        } else {
            ""
        }
    ));
    for cookie in csrf.legacy_cookies {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    if let Some(cookie) = csrf.cookie {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SignIn {
    credential: String,
    csrf: String,
}

async fn sign_in_submit(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<SignIn>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let service = match service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(&headers, None, "sign-in", "", &form.csrf) {
        return refused(error);
    }
    // An OpenAgents account credential only: a claude.ai login or
    // `claude setup-token` value is refused before it goes anywhere.
    if coder_cloud::claude::admit_value(&form.credential).is_err() {
        return refused(SessionError::InvalidRequest);
    }
    let grant = match service.sign_in(&form.credential).await {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let cookies = match grant.cookies() {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let mut response = protect(Redirect::to("/cloud/app").into_response());
    for cookie in cookies {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Switch {
    workspace: String,
    csrf: String,
}

async fn select_workspace(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<Switch>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let service = match service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let viewer = match service.authenticate(&headers).await {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        "select-workspace",
        &form.workspace,
        &form.csrf,
    ) {
        return refused(error);
    }
    if let Err(error) = service.select_workspace(&headers, &form.workspace).await {
        return refused(error);
    }
    let mut response = protect(Redirect::to("/cloud/app").into_response());
    match service.workspace_cookies(&form.workspace) {
        Ok(cookies) => {
            for cookie in cookies {
                response.headers_mut().append(header::SET_COOKIE, cookie);
            }
        }
        Err(error) => return refused(error),
    }
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SignOut {
    csrf: String,
}

async fn sign_out(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<SignOut>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let Some(service) = app.config.cloud.as_deref() else {
        return refused(SessionError::Unavailable);
    };
    // Clearing this browser narrows authority even when its native owner or
    // selected membership is unavailable. Other actions still require standing.
    if let Err(error) = service.verify_logout_csrf(&headers, &form.csrf) {
        return refused(error);
    }
    let mut response = match service.sign_out_current(&headers).await {
        Ok(()) => protect(Redirect::to("/cloud").into_response()),
        Err(_) => failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Browser session cleared",
            "The native logout outcome is unknown. Browser credentials were cleared; inspect the current session through the account owner before treating native logout as complete.",
        ),
    };
    for cookie in service.clear_cookies() {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

async fn overview(State(app): State<App>, headers: HeaderMap) -> Response {
    workspace_page(&app, &headers, "overview").await
}

async fn section(
    State(app): State<App>,
    headers: HeaderMap,
    request: axum::extract::OriginalUri,
) -> Response {
    workspace_page(
        &app,
        &headers,
        request.0.path().rsplit('/').next().unwrap_or("overview"),
    )
    .await
}

async fn workspace_page(app: &App, headers: &HeaderMap, selected: &str) -> Response {
    let service = match service(app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let viewer = match service.authenticate(headers).await {
        Ok(value) => value,
        Err(SessionError::Unauthenticated) => {
            return protect(Redirect::to("/cloud/sign-in").into_response());
        }
        Err(error) => return refused(error),
    };
    workspace_shell(app, headers, service, &viewer, selected, None, None)
}

fn workspace_shell(
    app: &App,
    headers: &HeaderMap,
    service: &CloudSession,
    viewer: &Viewer,
    selected: &str,
    supplied_content: Option<&str>,
    resource: Option<serde_json::Value>,
) -> Response {
    let view = workspace::session(
        &workspace::Session {
            account: &viewer.account_label,
            account_id: &viewer.account_id,
            workspace: viewer.workspace.as_ref().map(|v| v.name.as_str()),
            workspace_id: viewer.workspace.as_ref().map(|v| v.id.as_str()),
            role: viewer.workspace.as_ref().map(|v| v.role.as_str()),
            members_epoch: viewer.workspace.as_ref().map(|v| v.members_epoch),
            expires_at: viewer.expires_at,
        },
        colors(),
    );
    let summary = match render(&view) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mut switcher = String::new();
    for workspace in &viewer.workspaces {
        let csrf = match service.csrf(headers, &viewer, "select-workspace", &workspace.id) {
            Ok(value) => value,
            Err(error) => return refused(error),
        };
        switcher.push_str(&format!("<form method=\"post\" action=\"/cloud/select-workspace\">{}<input type=\"hidden\" name=\"workspace\" value=\"{}\"><button type=\"submit\">{} · {}</button></form>", ticket(&csrf), escape(&workspace.id), escape(&workspace.name), escape(&workspace.role)));
    }
    let csrf = match service.logout_csrf(headers, &viewer) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let mut nav = String::from("<a href=\"/cloud/app\">Overview</a>");
    for (label, slug, reason) in SECTIONS {
        if matches!(slug, "tasks" | "computers" | "projects" | "agents")
            && app
                .config
                .cloud_hosts
                .as_ref()
                .is_some_and(|hosts| !hosts.current(viewer).is_empty())
        {
            nav.push_str(&format!("<a href=\"/cloud/app/{slug}\">{label}</a>"));
        } else if slug == "workbench" && workbench::available(app, viewer) {
            nav.push_str("<a href=\"/cloud/app/workbench\">Workbench</a>");
        } else if slug == "billing" && billing::available(viewer) {
            nav.push_str("<a href=\"/cloud/app/billing\">Billing</a>");
        } else if slug == "partners" && partners::available(viewer) {
            nav.push_str("<a href=\"/cloud/app/partners\">Partners</a>");
        } else if slug == "team" && team::available(app) {
            nav.push_str("<a href=\"/cloud/app/team\">Team</a>");
        } else if slug == "sales" && sales::available(app, viewer) {
            nav.push_str("<a href=\"/cloud/app/sales\">Sales</a>");
        } else if slug == "settings" {
            nav.push_str("<a href=\"/cloud/app/settings\">Settings</a>");
        } else if slug == "verse" {
            // Public worlds keep this page useful without any host connection.
            nav.push_str("<a href=\"/cloud/app/verse\">Verse</a>");
        } else {
            nav.push_str(&format!(
                "<span aria-disabled=\"true\" title=\"{}\">{} · Unavailable</span>",
                escape(reason),
                label
            ));
        }
    }
    // Screen readers announce which section this page is.
    let current = if selected == "overview" {
        "<a href=\"/cloud/app\">".to_owned()
    } else {
        format!("<a href=\"/cloud/app/{selected}\">")
    };
    let nav = nav.replacen(
        &current,
        &current.replace("<a ", "<a aria-current=\"page\" "),
        1,
    );
    let mut content = String::new();
    if selected == "overview" {
        let configured = app
            .config
            .cloud_hosts
            .as_ref()
            .is_some_and(|hosts| !hosts.current(viewer).is_empty());
        for (key, label, reason) in [
            (
                "world",
                "World",
                "No admitted world connection. You can explore the public Verse demos.",
            ),
            (
                "computer",
                "Computer",
                "No explicitly enrolled host connection for this account and workspace.",
            ),
            (
                "private-work",
                "Private work",
                "No current observe, operate, review, typist, or sales grant. Account sign-in grants none of these rights.",
            ),
        ] {
            let world = app.config.cloud_hosts.as_ref().is_some_and(|hosts| {
                hosts
                    .current(viewer)
                    .iter()
                    .any(|binding| binding.declared_world().is_some())
            });
            let (state, reason) = if world && key == "world" {
                (
                    "Not checked",
                    "A host world is configured for this workspace. Open Verse to check its admission and join. Joining grants no private-work right.",
                )
            } else if configured && key == "computer" {
                (
                    "Not checked",
                    "This account has an explicit resident binding. Open Computers to verify the current native grant and resident generation.",
                )
            } else if configured && key == "private-work" {
                (
                    "Not checked",
                    "Canonical task observation is configured. Open Tasks to check current Observe authority. Browser enrollment and every effect require their own exact review.",
                )
            } else {
                ("Unavailable", reason)
            };
            match render(&workspace::connection_state(
                key,
                label,
                state,
                reason,
                colors(),
            )) {
                Ok(value) => {
                    content.push_str(&format!("<section class=\"cloud-card\">{value}</section>"))
                }
                Err(response) => return response,
            }
        }
        content.push_str("<p>No connected work to report. Costs, waiting tasks, outcomes, and unread counts are unavailable until their canonical owners are connected.</p><p><a href=\"/cloud/app/verse\">Verse connections</a> · <a href=\"/grid\">Open the Grid</a> · <a href=\"/components\">Explore shared components</a></p>");
    } else if selected == "tasks" {
        content.push_str("<h2>Resident tasks</h2><p>Choose an explicitly bound resident host. Every task page checks current native Observe authority.</p><ul>");
        let bindings = app
            .config
            .cloud_hosts
            .as_ref()
            .map_or_else(Vec::new, |hosts| hosts.current(viewer));
        if bindings.is_empty() {
            content.push_str(
                "<li>No resident task connection is admitted for this account and workspace.</li>",
            );
        }
        for binding in bindings {
            content.push_str(&format!(
                "<li><a href=\"/cloud/app/hosts/{}/tasks\">Open resident connection {}</a></li>",
                escape(binding.id()),
                escape(binding.id())
            ));
        }
        content.push_str("</ul>");
    } else if selected == "settings" {
        content.push_str("<h2>Account and sessions</h2><p>This is your current native session. The selected native account API does not offer browser session enumeration or recovery-token issuance. Use the native account owner's recovery and credential controls; recovery, rotation, and revoked membership fence this browser on its next standing check.</p><h2>Integrations and sync</h2><p>Host enrollment, provider custody, notifications, private-memory sync, and disclosure are unavailable until separately admitted. Signing in enables none of them.</p>");
        if byo::available(app) {
            content.push_str("<h2>Claude credential</h2><p>Add your own Anthropic API key or Bedrock, Vertex, or Foundry credential for your own computers and parallel Claude Code tasks. Usage bills to your own Anthropic or cloud account. <a href=\"/cloud/app/settings/claude\">Manage Claude credential</a></p>");
        }
    } else if let Some((label, _, reason)) = SECTIONS.iter().find(|(_, slug, _)| *slug == selected)
    {
        content = format!("<h2>{label} · Unavailable</h2><p>{}</p>", escape(reason));
    }
    if let Some(supplied) = supplied_content {
        content = supplied.into();
    }
    let initial = escape(&standing_value(&viewer).to_string());
    let resource = resource.map_or_else(String::new, |value| {
        format!(
            "<pre id=\"cloud-resource-standing\" hidden>{}</pre>",
            escape(&value.to_string())
        )
    });
    let local_logout = format!(
        "<form method=\"post\" action=\"/cloud/sign-out\">{}<button type=\"submit\">Sign out of this browser</button></form>",
        ticket(&csrf)
    );
    page(&format!(
        "<section id=\"cloud-resume\" aria-live=\"polite\"><h1>Workspace</h1><p>Reopen this view to check current account standing.</p><p><a href=\"/cloud/app\">Reopen workspace</a></p>{local_logout}</section><div id=\"cloud-private\" hx-history=\"false\" hidden><pre id=\"cloud-standing\" hidden>{initial}</pre>{resource}<div class=\"cloud-layout\"><aside class=\"cloud-sidebar\"><h1>Workspace</h1><nav aria-label=\"Workspace\">{nav}</nav><h2>Choose workspace</h2><div class=\"cloud-switcher\">{switcher}</div><form method=\"post\" action=\"/cloud/sign-out\">{}<button type=\"submit\">Sign out</button></form></aside><section class=\"cloud-main\">{summary}<hr>{content}</section></div></div>",
        ticket(&csrf)
    ))
}

fn standing_value(viewer: &Viewer) -> serde_json::Value {
    let projection = json!({"account":viewer.account_id,"label":viewer.account_label,"workspaces":viewer.workspaces,"selected":viewer.workspace});
    let digest = Sha256::digest(projection.to_string().as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    json!({"active":true,"session_id":viewer.session_id,"account":viewer.account_id,"workspace":viewer.workspace.as_ref().map(|v|&v.id),"members_epoch":viewer.workspace.as_ref().map(|v|v.members_epoch),"projection_digest":format!("sha256:{digest}"),"expires_at":viewer.expires_at})
}

async fn standing(State(app): State<App>, headers: HeaderMap) -> Response {
    let service = match service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match service.authenticate(&headers).await {
        Ok(viewer) => protect(Json(standing_value(&viewer)).into_response()),
        Err(error) => refused(error),
    }
}

/// Generated Rust/Wasm files served from the `--cloud-build` directory. The
/// site images build and check every one (`Dockerfile`,
/// `Dockerfile.components`); a test keeps the lists equal.
pub(crate) const BUILD_ASSETS: [&str; 4] = [
    "coder_cloud_web.js",
    "coder_cloud_web_bg.wasm",
    "coder_browser_web.js",
    "coder_browser_web_bg.wasm",
];

async fn asset(State(app): State<App>, Path(file): Path<String>) -> Response {
    let (mime, bytes) = match file.as_str() {
        "cloud.css" => (
            "text/css; charset=utf-8",
            crate::palette::stylesheet(include_str!("../../static/cloud.css")).into_bytes(),
        ),
        "native.css" => (
            "text/css; charset=utf-8",
            include_bytes!("../../../rust-native-web/src/style.css").to_vec(),
        ),
        "start.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../../static/cloud-start.js").to_vec(),
        ),
        name if BUILD_ASSETS.contains(&name) => {
            let Some(dir) = &app.config.cloud_build else {
                return StatusCode::NOT_FOUND.into_response();
            };
            let Ok(bytes) = tokio::fs::read(dir.join(&file)).await else {
                return StatusCode::NOT_FOUND.into_response();
            };
            if bytes.len() > 64 * 1024 * 1024 {
                return StatusCode::NOT_FOUND.into_response();
            }
            (
                if file.ends_with(".wasm") {
                    "application/wasm"
                } else {
                    "text/javascript; charset=utf-8"
                },
                bytes,
            )
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        bytes,
    )
        .into_response()
}
