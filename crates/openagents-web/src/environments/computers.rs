//! Account authority for the existing Environment owners' shared computers.
//! The web keeps the user's authorization sealed separately from saved machine
//! requests. Only this boundary can prepare a command's repository credentials.
use crate::{
    App,
    cloud::session::{CloudSession, SessionError},
    computers_api::Identity,
};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use coder_environment_operator::{
    computers::{self as shared, Binding, Grant, Store},
    studio::github::Resolved,
};
use serde_json::{Value, json};

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
fn answer(status: StatusCode, value: Value) -> Response {
    let mut r = (status, Json(value)).into_response();
    r.headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    r
}
fn denied() -> Response {
    answer(
        StatusCode::FORBIDDEN,
        json!({"error":{"code":"environment_scope","message":"This setup can't do that. Open its Environment page to continue."}}),
    )
}
fn unavailable() -> Response {
    answer(
        StatusCode::SERVICE_UNAVAILABLE,
        json!({"error":{"code":"unavailable","message":"We couldn't check this setup's access. Try again shortly."}}),
    )
}
fn bearer(headers: &HeaderMap) -> Option<&str> {
    (headers.get_all(header::AUTHORIZATION).iter().count() == 1)
        .then(|| {
            headers
                .get(header::AUTHORIZATION)?
                .to_str()
                .ok()?
                .strip_prefix("Bearer ")
        })
        .flatten()
}
fn store(app: &App) -> Result<Store, Response> {
    app.config
        .environments
        .studio()
        .and_then(|s| s.computer_state())
        .map(Store::under)
        .ok_or_else(unavailable)
}
fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    if headers.get_all(header::COOKIE).iter().count() != 1 {
        return None;
    }
    let mut found = None;
    for item in headers.get(header::COOKIE)?.to_str().ok()?.split(';') {
        let Some((key, value)) = item.trim().split_once('=') else {
            continue;
        };
        if key == name {
            if found.is_some() {
                return None;
            }
            found = Some(value.to_owned())
        }
    }
    found
}
/// Renew authority only when the person explicitly starts another attempt.
/// Existing bindings retain their original workspace epoch and expiration.
pub(super) async fn refresh(app: &App, headers: &HeaderMap, id: &str) -> Result<(), String> {
    if !id.starts_with("envc-") {
        return Ok(());
    }
    let s = store(app).map_err(|_| "Environment setup isn't available right now.")?;
    let meta = s.metadata(id)?;
    if meta["public_computers"] != true {
        return Err("This Environment isn't available.".into());
    }
    let viewer = app
        .config
        .cloud
        .as_deref()
        .ok_or("Sign in to continue.")?
        .authenticate(headers)
        .await
        .map_err(|_| "Sign in again before retrying this Environment.")?;
    if meta["account"].as_str() != Some(&viewer.account_id) {
        return Err("This Environment belongs to another account.".into());
    }
    let env = coder_environment::store::Store::under(
        coder_environment_operator::Layout::under(&s.state).environments(),
    )
    .read(id)
    .map_err(|_| "The Environment couldn't be read.")?;
    let resolved = Resolved {
        repository: coder_environment_operator::studio::github::RepoName::parse(
            env.source.repository.as_deref().unwrap_or_default(),
        )
        .ok_or("The repository couldn't be read.")?,
        branch: meta["branch"].as_str().unwrap_or("main").into(),
        commit: env.source.revision,
        private: meta["private"].as_bool().unwrap_or(true),
    };
    let authorization = Authorization::prepare(app, headers, &resolved).await?;
    if authorization.grant.account != viewer.account_id {
        return Err("Sign in again before retrying this Environment.".into());
    }
    authorization.save(app, id)
}
fn aad(id: &str, grant: &Grant) -> Vec<u8> {
    serde_json::to_vec(&json!([
        "openagents.environment.authorization.v1",
        id,
        grant.account,
        grant.workspace,
        grant.repository,
        grant.private,
        grant.expires_unix
    ]))
    .unwrap()
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Credential {
    session: String,
    ticket: Option<String>,
}
pub(super) struct Authorization {
    grant: Grant,
    secret: Credential,
}
impl Authorization {
    pub(super) async fn prepare(
        app: &App,
        headers: &HeaderMap,
        resolved: &Resolved,
    ) -> Result<Self, String> {
        let service = app
            .config
            .cloud
            .as_deref()
            .ok_or("Sign in before starting this Environment.")?;
        let viewer = service
            .authenticate(headers)
            .await
            .map_err(|_| "Sign in again before starting this Environment.")?;
        if !crate::agent_work::permitted(app, &viewer) || app.config.solver_keys.is_none() {
            return Err("This Environment isn't available for this account.".into());
        }
        let token = cookie(headers, "oa_cloud_session")
            .ok_or("Sign in again before starting this Environment.")?;
        if !crate::cloud::session::session_token(&token) {
            return Err("Sign in again before starting this Environment.".into());
        }
        let workspace = viewer
            .workspace
            .as_ref()
            .map(|w| (w.id.clone(), w.members_epoch));
        let grant = Grant {
            account: viewer.account_id,
            workspace,
            repository: resolved.repository.full(),
            private: resolved.private,
            expires_unix: viewer.expires_at.min(now() + 24 * 3600),
            credential: Value::Null,
        };
        let ticket = if resolved.private {
            let issued = service
                .github_broker_ticket(
                    headers,
                    &grant.repository,
                    grant.expires_unix.saturating_sub(now()),
                )
                .await;
            match issued {
                Ok(ticket) => Some(ticket),
                Err(crate::cloud::session::github::RepoCallError::Repo(
                    oa_auth::repos::RepoError::NotFound,
                )) => {
                    service.create_project(headers,&oa_auth::repos::NewProject{repositories:vec![grant.repository.clone()],..Default::default()}).await.map_err(|_|"Connect this repository in Projects before starting its Environment.")?;
                    Some(service.github_broker_ticket(headers,&grant.repository,grant.expires_unix.saturating_sub(now())).await.map_err(|_|"Connect this repository in Projects before starting its Environment.")?)
                }
                Err(_) => {
                    return Err(
                        "Connect this repository in Projects before starting its Environment."
                            .into(),
                    );
                }
            }
        } else {
            None
        };
        Ok(Self {
            grant,
            secret: Credential {
                session: token,
                ticket,
            },
        })
    }
    pub(super) fn save(&self, app: &App, id: &str) -> Result<(), String> {
        let mut grant = self.grant.clone();
        let sealed = app
            .config
            .solver_keys
            .as_deref()
            .ok_or("This Environment isn't available right now.")?
            .seal(
                &aad(id, &grant),
                &serde_json::to_vec(&self.secret)
                    .map_err(|_| "This Environment couldn't be authorized.")?,
            )
            .map_err(|_| "This Environment couldn't be authorized.")?;
        grant.credential =
            serde_json::to_value(sealed).map_err(|_| "This Environment couldn't be authorized.")?;
        store(app)
            .map_err(|_| "This Environment isn't available right now.")?
            .save_grant(id, &grant)
    }
}

pub(crate) async fn resolve(app: &App, headers: &HeaderMap) -> Result<Binding, Response> {
    let supplied = bearer(headers).ok_or_else(denied)?;
    let (id, signature) = supplied
        .strip_prefix("ecj_")
        .and_then(|s| s.split_once('.'))
        .ok_or_else(denied)?;
    if !coder_environment::valid_id(id)
        || id.len() > 96
        || signature.len() != 64
        || !signature.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(denied());
    }
    let store = store(app)?;
    let binding = store
        .binding(id)
        .map_err(|_| unavailable())?
        .ok_or_else(denied)?;
    let computer = store.computer(id).map_err(|_| denied())?;
    let expected = shared::credential(&binding, &app.config.ask_salt);
    if !binding.validates(&computer)
        || expected.len() != supplied.len()
        || expected
            .bytes()
            .zip(supplied.bytes())
            .fold(0u8, |v, (a, b)| v | (a ^ b))
            != 0
        || store
            .metadata(&binding.attempt.environment)
            .ok()
            .and_then(|m| m["account"].as_str().map(str::to_owned))
            .as_deref()
            != Some(&binding.account)
    {
        return Err(denied());
    }
    Ok(binding)
}
pub(crate) async fn identity(app: &App, headers: &HeaderMap) -> Result<Identity, Response> {
    let binding = resolve(app, headers).await?;
    Ok(Identity {
        owner: crate::chat_store::account_owner(&binding.account),
        account: binding.account,
        permitted: true,
        admin: false,
        environment_attempt: Some(binding.native),
    })
}
async fn session(State(app): State<App>, headers: HeaderMap) -> Response {
    match resolve(&app, &headers).await {
        Ok(b) => answer(
            StatusCode::OK,
            json!({"account":{"id":b.account,"label":"Environment setup"},"workspaces":[]}),
        ),
        Err(r) => r,
    }
}
async fn authorization(app: &App, b: &Binding) -> Result<HeaderMap, Response> {
    let store = store(app)?;
    let grant = store
        .grant(&b.attempt.environment)
        .map_err(|_| unavailable())?
        .ok_or_else(denied)?;
    if now() > b.expires_unix
        || now() > grant.expires_unix
        || grant.account != b.account
        || grant.workspace != b.workspace
    {
        return Err(denied());
    }
    let sealed: oa_seal::Sealed =
        serde_json::from_value(grant.credential.clone()).map_err(|_| denied())?;
    let bytes = app
        .config
        .solver_keys
        .as_deref()
        .ok_or_else(unavailable)?
        .open(&aad(&b.attempt.environment, &grant), &sealed)
        .map_err(|_| denied())?;
    let secret: Credential = serde_json::from_slice(&bytes).map_err(|_| denied())?;
    let token = secret.session.as_str();
    if !crate::cloud::session::session_token(token) {
        return Err(denied());
    }
    let cookie = match &grant.workspace {
        Some((id, _)) => format!("oa_cloud_session={token}; oa_cloud_workspace={id}"),
        None => format!("oa_cloud_session={token}"),
    };
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, cookie.parse().map_err(|_| denied())?);
    let service = app.config.cloud.as_deref().ok_or_else(unavailable)?;
    let origin = url::Url::parse(service.origin()).map_err(|_| unavailable())?;
    headers.insert(
        header::HOST,
        origin[url::Position::BeforeHost..url::Position::AfterPort]
            .parse()
            .map_err(|_| unavailable())?,
    );
    if now().saturating_mul(1000) >= b.attempt.deadline_ms {
        return Err(denied());
    }
    let viewer = service.authenticate(&headers).await.map_err(|e| {
        if matches!(e, SessionError::Unavailable) {
            unavailable()
        } else {
            denied()
        }
    })?;
    if viewer.account_id != b.account
        || viewer
            .workspace
            .as_ref()
            .map(|w| (w.id.clone(), w.members_epoch))
            != b.workspace
        || !crate::agent_work::permitted(app, &viewer)
    {
        return Err(denied());
    }
    let layout = coder_environment_operator::Layout::under(&store.state);
    let env = coder_environment::store::Store::under(layout.environments())
        .read(&b.attempt.environment)
        .map_err(|_| denied())?;
    if env.retired_ms.is_some() {
        return Err(denied());
    }
    let computer = store.computer(&b.native).map_err(|_| denied())?;
    if grant.repository != computer.source.repository.as_deref().unwrap_or_default() {
        return Err(denied());
    }
    if grant.private {
        // The existing broker rechecks the requester's own repository grant;
        // there is no operator or staging-pool credential fallback.
        let ticket = secret.ticket.as_deref().ok_or_else(denied)?;
        if redeem(service, ticket, &grant.repository).await.is_none() {
            return Err(denied());
        }
    } else {
        public_source(&grant.repository, &computer.source.revision).await?;
    }
    Ok(headers)
}
async fn public_source(repository: &str, revision: &str) -> Result<(), Response> {
    if !oa_auth::repos::full_name(repository)
        || !matches!(revision.len(), 40 | 64)
        || !revision.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(denied());
    }
    let http = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("openagents-environments")
        .build()
        .map_err(|_| unavailable())?;
    // Ask the same anonymous Git service the source step will use. This
    // avoids GitHub's anonymous REST quota on command/output polling. The
    // source owner separately proves the exact frozen checkout before install.
    let response = http
        .get(format!(
            "https://github.com/{repository}.git/info/refs?service=git-upload-pack"
        ))
        .send()
        .await
        .map_err(|_| unavailable())?;
    if response.status().as_u16() == 429 || response.status().is_server_error() {
        return Err(unavailable());
    }
    if !response.status().is_success()
        || response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            != Some("application/x-git-upload-pack-advertisement")
    {
        return Err(denied());
    }
    Ok(())
}

async fn redeem(service: &CloudSession, ticket: &str, repository: &str) -> Option<String> {
    let form = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("ticket", ticket)
        .append_pair("protocol", "https")
        .append_pair("host", "github.com")
        .append_pair("path", &format!("{repository}.git"))
        .finish();
    let (status, body) = service.git_credential(form.as_bytes()).await;
    if status != 200 {
        return None;
    }
    body.lines()
        .find_map(|line| line.strip_prefix("password=").map(str::to_owned))
        .filter(|v| !v.is_empty())
}
async fn computer_id(app: &App, b: &Binding) -> Result<Option<String>, Response> {
    if let Some(reference) = &b.computer {
        return Ok(Some(reference.id().into()));
    }
    let client = crate::computers::client().ok_or_else(unavailable)?;
    let key = b.options.idempotency_key.as_deref().ok_or_else(denied)?;
    match client
        .call(
            reqwest::Method::GET,
            &format!("/v1/computers/by-request/{key}"),
            &crate::chat_store::account_owner(&b.account),
            None,
        )
        .await
    {
        Ok((404, _)) => Ok(None),
        Ok((200, value))
            if crate::computers_api::cloud_id(value["id"].as_str().unwrap_or_default()) =>
        {
            Ok(value["id"].as_str().map(str::to_owned))
        }
        _ => Err(unavailable()),
    }
}
fn policy(b: &Binding, id: Option<&str>, method: &str, path: &str, body: &Value) -> Option<bool> {
    if method == "GET" && path == "/v1/environment-computers/session" {
        return Some(false);
    }
    let private = format!("/v1/environment-computers/{}/", b.native);
    if let Some(action) = path.strip_prefix(&private) {
        if method != "POST" {
            return None;
        }
        return match action {
            "command" | "computer" => Some(false),
            "image" if body == "delete" => Some(false),
            "image" | "credentials" | "credentials-check" => Some(true),
            _ => None,
        };
    }
    if method == "POST" && path == "/v1/computers" {
        return (serde_json::to_value(&b.options).ok().as_ref() == Some(body)).then_some(true);
    }
    let id = id?;
    let base = format!("/v1/computers/{id}");
    if path == base {
        return match method {
            "GET" | "DELETE" => Some(false),
            _ => None,
        };
    }
    let suffix = path.strip_prefix(&(base + "/"))?;
    match (method, suffix) {
        ("POST", "stop") => Some(false),
        ("POST", "wake") => Some(true),
        ("GET", "files") => Some(true),
        ("PUT", "files") if b.attempt.role == shared::EnvironmentRole::Setup => Some(true),
        ("POST", "commands") => b
            .commands
            .values()
            .any(|c| serde_json::to_value(c.request.options()).ok().as_ref() == Some(body))
            .then_some(true),
        _ => {
            for command in b.commands.values().filter_map(|c| c.reference.as_ref()) {
                let path = format!("commands/{}", command.id());
                if method == "GET" && (suffix == path || suffix == format!("{path}/output")) {
                    return Some(true);
                }
                if method == "POST" && suffix == format!("{path}/cancel") {
                    return Some(false);
                }
            }
            None
        }
    }
}
pub(crate) async fn guard(State(app): State<App>, request: Request, next: Next) -> Response {
    let present = request
        .headers()
        .get_all(header::AUTHORIZATION)
        .iter()
        .any(|value| {
            value.to_str().is_ok_and(|value| {
                value.split_once(' ').is_some_and(|(kind, token)| {
                    kind.eq_ignore_ascii_case("bearer") && token.trim_start().starts_with("ecj_")
                })
            })
        });
    if !present {
        return next.run(request).await;
    }
    if !bearer(request.headers()).is_some_and(|t| t.starts_with("ecj_")) {
        return denied();
    }
    let b = match resolve(&app, request.headers()).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    let method = request.method().to_string();
    let path = request.uri().path().to_owned();
    let private = format!("/v1/environment-computers/{}/", b.native);
    if path != "/v1/environment-computers/session"
        && !(method == "POST" && path == "/v1/computers")
        && !path.starts_with(&private)
        && !path.starts_with("/v1/computers/cmp_")
    {
        return denied();
    }
    let id = if path.starts_with("/v1/environment-computers/")
        || method == "POST" && path == "/v1/computers"
    {
        b.computer.as_ref().map(|c| c.id().to_owned())
    } else {
        match computer_id(&app, &b).await {
            Ok(id) => id,
            Err(r) => return r,
        }
    };
    if method == "DELETE" && request.uri().query() != Some("kind=cloud") {
        return denied();
    }
    if method == "PUT" && path.ends_with("/files") {
        if policy(&b, id.as_deref(), &method, &path, &Value::Null) != Some(true) {
            return denied();
        }
        if let Err(r) = authorization(&app, &b).await {
            return r;
        }
        return next.run(request).await;
    }
    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, 73728).await {
        Ok(b) => b,
        Err(_) => return denied(),
    };
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => return denied(),
        }
    };
    let Some(check) = policy(&b, id.as_deref(), &method, &path, &body) else {
        return denied();
    };
    if check {
        if let Err(r) = authorization(&app, &b).await {
            return r;
        }
    }
    next.run(Request::from_parts(parts, Body::from(bytes)))
        .await
}
async fn image(
    State(app): State<App>,
    headers: HeaderMap,
    Path(native): Path<String>,
    Json(op): Json<String>,
) -> Response {
    let b = match resolve(&app, &headers).await {
        Ok(b) if b.native == native => b,
        Ok(_) => return denied(),
        Err(r) => return r,
    };
    if !matches!(op.as_str(), "read" | "delete" | "capture" | "hydrated") {
        return denied();
    }
    if op == "capture" {
        let s = match store(&app) {
            Ok(s) => s,
            Err(r) => return r,
        };
        if !shared::capture_ready(&s, &b) {
            return denied();
        }
    }
    let id = match computer_id(&app, &b).await {
        Ok(Some(id)) => id,
        _ => return unavailable(),
    };
    let Some(client) = crate::computers::client() else {
        return unavailable();
    };
    match client
        .call_scoped(
            reqwest::Method::POST,
            &format!("/v1/computers/{id}/environment-image"),
            &crate::chat_store::account_owner(&b.account),
            Some(json!(op)),
            Some(&b.native),
        )
        .await
    {
        Ok((200, body)) => answer(StatusCode::OK, body),
        _ => unavailable(),
    }
}
async fn computer(
    State(app): State<App>,
    headers: HeaderMap,
    Path(native): Path<String>,
) -> Response {
    let b = match resolve(&app, &headers).await {
        Ok(b) if b.native == native => b,
        Ok(_) => return denied(),
        Err(r) => return r,
    };
    match computer_id(&app, &b).await {
        Ok(id) => answer(StatusCode::OK, json!(id)),
        Err(r) => r,
    }
}
async fn command(
    State(app): State<App>,
    headers: HeaderMap,
    Path(native): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let b = match resolve(&app, &headers).await {
        Ok(b) if b.native == native => b,
        Ok(_) => return denied(),
        Err(r) => return r,
    };
    let Some(saved) = body["command"].as_str().and_then(|id| b.commands.get(id)) else {
        return denied();
    };
    let id = match computer_id(&app, &b).await {
        Ok(Some(id)) => id,
        _ => return unavailable(),
    };
    let Some(client) = crate::computers::client() else {
        return unavailable();
    };
    match client
        .call_scoped(
            reqwest::Method::GET,
            &format!(
                "/v1/computers/{id}/commands/by-request/{}",
                saved.request.key().as_str()
            ),
            &crate::chat_store::account_owner(&b.account),
            None,
            Some(&b.native),
        )
        .await
    {
        Ok((200, value)) => answer(StatusCode::OK, value["id"].clone()),
        _ => unavailable(),
    }
}
async fn git_values(
    app: &App,
    b: &Binding,
    names: &std::collections::BTreeSet<String>,
) -> Result<Vec<(String, String)>, Response> {
    let _headers = authorization(app, b).await?;
    if !names
        .iter()
        .all(|n| shared::GIT_CREDENTIALS.contains(&n.as_str()))
    {
        return Err(denied());
    }
    if names.is_empty() {
        return Ok(vec![]);
    }
    let s = store(app)?;
    let grant = s
        .grant(&b.attempt.environment)
        .map_err(|_| unavailable())?
        .ok_or_else(denied)?;
    if !grant.private {
        return Err(denied());
    }
    let service = app.config.cloud.as_deref().ok_or_else(unavailable)?;
    let sealed: oa_seal::Sealed =
        serde_json::from_value(grant.credential.clone()).map_err(|_| denied())?;
    let bytes = app
        .config
        .solver_keys
        .as_deref()
        .ok_or_else(unavailable)?
        .open(&aad(&b.attempt.environment, &grant), &sealed)
        .map_err(|_| denied())?;
    let secret: Credential = serde_json::from_slice(&bytes).map_err(|_| denied())?;
    let token = redeem(
        service,
        secret.ticket.as_deref().ok_or_else(denied)?,
        &grant.repository,
    )
    .await
    .ok_or_else(denied)?;
    Ok(names.iter().map(|n| (n.clone(), token.clone())).collect())
}
async fn credentials_check(
    State(app): State<App>,
    headers: HeaderMap,
    Path(native): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let b = match resolve(&app, &headers).await {
        Ok(b) if b.native == native => b,
        Ok(_) => return denied(),
        Err(r) => return r,
    };
    let names: std::collections::BTreeSet<String> =
        match serde_json::from_value(body["names"].clone()) {
            Ok(n) => n,
            Err(_) => return denied(),
        };
    let c = match store(&app).and_then(|s| s.computer(&native).map_err(|_| denied())) {
        Ok(c) => c,
        Err(r) => return r,
    };
    if names != c.credential_names {
        return denied();
    }
    match git_values(&app, &b, &names).await {
        Ok(_) => answer(StatusCode::OK, json!({"ready":true})),
        Err(r) => r,
    }
}
async fn credentials(
    State(app): State<App>,
    headers: HeaderMap,
    Path(native): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let b = match resolve(&app, &headers).await {
        Ok(b) if b.native == native => b,
        Ok(_) => return denied(),
        Err(r) => return r,
    };
    let Some(saved) = body["command"].as_str().and_then(|id| b.commands.get(id)) else {
        return denied();
    };
    let c = match store(&app).and_then(|s| s.computer(&native).map_err(|_| denied())) {
        Ok(c) => c,
        Err(r) => return r,
    };
    if !saved.spec.credential_names.is_subset(&c.credential_names) {
        return denied();
    }
    let values = match git_values(&app, &b, &saved.spec.credential_names).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let id = match computer_id(&app, &b).await {
        Ok(Some(id)) => id,
        _ => return unavailable(),
    };
    let Some(reference) = saved.request.options().credential_ref.as_deref() else {
        return denied();
    };
    match crate::computers_api::commands::prepare_environment_credentials(
        &crate::chat_store::account_owner(&b.account),
        &id,
        reference,
        saved.request.key().as_str(),
        &saved.request.options().command,
        &values,
        &b.native,
    )
    .await
    {
        Ok(()) => answer(StatusCode::OK, json!({"ready":true})),
        Err(_) => unavailable(),
    }
}
pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/environment-computers/session", get(session))
        .route("/v1/environment-computers/{native}/image", post(image))
        .route("/v1/environment-computers/{native}/command", post(command))
        .route(
            "/v1/environment-computers/{native}/computer",
            post(computer),
        )
        .route(
            "/v1/environment-computers/{native}/credentials",
            post(credentials),
        )
        .route(
            "/v1/environment-computers/{native}/credentials-check",
            post(credentials_check),
        )
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod qualification;
