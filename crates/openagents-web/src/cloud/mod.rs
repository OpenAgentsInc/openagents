//! Same-origin Cloud pages over current native account authority. The public
//! site, local task browser, and separately granted services remain distinct.

pub mod hosts;
mod private;
pub mod session;
#[cfg(test)]
mod tests;
mod work;

use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{DefaultBodyLimit, Form, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
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
        "Browser team controls require their own native qualification.",
    ),
    (
        "Billing",
        "billing",
        "Configure an explicit native financial delegation.",
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
        .layer(DefaultBodyLimit::max(8192));
    for (_, slug, _) in SECTIONS {
        router = router.route(&format!("/cloud/app/{slug}"), get(section));
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
        text: color(crate::palette::Intensity::ThreeQuarters.color()),
        heading: color(crate::palette::Intensity::Full.color()),
        secondary: color(crate::palette::Intensity::Half.color()),
        border: color(crate::palette::Intensity::Quarter.color()),
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
    let html = document("Workspace", Some("/cloud"), &body).replace("</head>", "<link rel=\"stylesheet\" href=\"/cloud/assets/cloud.css\"><link rel=\"stylesheet\" href=\"/cloud/assets/native.css\"></head>");
    protect(axum::response::Html(html).into_response())
}

async fn sign_in(State(app): State<App>, headers: HeaderMap) -> Response {
    let service = match service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if service.authenticate(&headers).await.is_ok() {
        return protect(Redirect::to("/cloud/app").into_response());
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
        "<h1>Sign in to your workspace</h1><p>Use an existing native account API key. The selected account service issues a revocable session. This form creates no computer, execution, sales, or spending grant.</p><form method=\"post\" action=\"/cloud/sign-in\">{}{field}<p><button type=\"submit\">Sign in</button></p></form><p class=\"dim\">Account creation and recovery remain with the native account owner. After recovery or key rotation, sign in with the new key.</p>",
        ticket(&csrf.token)
    ));
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
    match service.workspace_cookie(&form.workspace) {
        Ok(cookie) => {
            response.headers_mut().append(header::SET_COOKIE, cookie);
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
        if slug == "tasks"
            && app
                .config
                .cloud_hosts
                .as_ref()
                .is_some_and(|hosts| !hosts.current(viewer).is_empty())
        {
            nav.push_str("<a href=\"/cloud/app/tasks\">Tasks</a>");
        } else if slug == "settings" {
            nav.push_str("<a href=\"/cloud/app/settings\">Settings</a>");
        } else {
            nav.push_str(&format!(
                "<span aria-disabled=\"true\" title=\"{}\">{} · Unavailable</span>",
                escape(reason),
                label
            ));
        }
    }
    let mut content = String::new();
    if selected == "overview" {
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
            match render(&workspace::connection(key, label, reason, colors())) {
                Ok(value) => {
                    content.push_str(&format!("<section class=\"cloud-card\">{value}</section>"))
                }
                Err(response) => return response,
            }
        }
        content.push_str("<p>No connected work to report. Costs, waiting tasks, outcomes, and unread counts are unavailable until their canonical owners are connected.</p><p><a href=\"/grid\">Open the Grid</a> · <a href=\"/components\">Explore shared components</a></p>");
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
        "<section id=\"cloud-resume\" aria-live=\"polite\"><h1>Workspace</h1><p>Reopen this view to check current account standing.</p><p><a href=\"/cloud/app\">Reopen workspace</a></p>{local_logout}</section><div id=\"cloud-private\" hidden><pre id=\"cloud-standing\" hidden>{initial}</pre>{resource}<div class=\"cloud-layout\"><aside class=\"cloud-sidebar\"><h1>Workspace</h1><nav aria-label=\"Workspace\">{nav}</nav><h2>Choose workspace</h2><div class=\"cloud-switcher\">{switcher}</div><form method=\"post\" action=\"/cloud/sign-out\">{}<button type=\"submit\">Sign out</button></form></aside><section class=\"cloud-main\">{summary}<hr>{content}</section></div></div>",
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

async fn asset(State(app): State<App>, Path(file): Path<String>) -> Response {
    let (mime, bytes) = match file.as_str() {
        "cloud.css" => (
            "text/css; charset=utf-8",
            include_bytes!("../../static/cloud.css").to_vec(),
        ),
        "native.css" => (
            "text/css; charset=utf-8",
            include_bytes!("../../../rust-native-web/src/style.css").to_vec(),
        ),
        "start.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../../static/cloud-start.js").to_vec(),
        ),
        "coder_cloud_web.js" | "coder_cloud_web_bg.wasm" => {
            let Some(dir) = &app.config.cloud_build else {
                return StatusCode::NOT_FOUND.into_response();
            };
            let Ok(bytes) = tokio::fs::read(dir.join(&file)).await else {
                return StatusCode::NOT_FOUND.into_response();
            };
            if bytes.len() > 16 * 1024 * 1024 {
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
