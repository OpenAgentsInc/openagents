//! Solver jobs retain their computer authority and requests across restarts.
//! Job credentials cannot read an account's other computers or other API groups.

use super::*;
use axum::{body::Body, extract::Request, middleware::Next};
use coder_environment::{ProjectLink, VersionPin};
use hmac::{Hmac, Mac};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Binding {
    pub schema: String,
    pub project: ProjectLink,
    pub branch: String,
    pub permission: String,
    pub environment: VersionPin,
    pub payer: String,
    pub image: String,
    pub create: Value,
    pub computer: Option<String>,
    pub commands: BTreeMap<String, Value>,
    pub command_ids: Vec<String>,
    pub cleanup_observed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex: Option<Vertex>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Vertex {
    value: oa_seal::Sealed,
    expires_unix: u64,
}

fn enrolled(account: &str) -> bool {
    std::env::var("OPENAGENTS_SOLVER_PUBLIC_ACCOUNTS")
        .unwrap_or_default()
        .split(',')
        .any(|a| a.trim() == account)
}

/// Recovery also runs without the dispatcher's disk or process. Only jobs
/// with an explicit shared-computer binding enter this cleanup index.
pub(crate) fn spawn_cleanup(app: App) {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    runtime.spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let _ = sweep(&app).await;
        }
    });
}

async fn sweep(app: &App) -> Result<(), Error> {
    let Some((bytes, _)) = app.config.chat_store.read_key(PUBLIC_QUEUE_KEY).await? else {
        return Ok(());
    };
    let queue: Queue = serde_json::from_slice(&bytes)
        .map_err(|_| Error::Invalid("The job list couldn't be read."))?;
    for id in queue.computer_jobs {
        let Some(mut run) = load_plain(&app.config.chat_store, &id).await? else {
            continue;
        };
        if run.clone().settle(now_unix()) {
            update_run(&app.config.chat_store, &id, |run| {
                (run.settle(now_unix()), ())
            })
            .await?;
            run = load_plain(&app.config.chat_store, &id).await?.unwrap();
        }
        if !run.cancel && !run.state.finished() {
            continue;
        }
        let run = match recover_binding(app, run).await {
            Ok(run) => run,
            Err(_) => continue,
        };
        let Some(binding) = run.computer.as_ref() else {
            continue;
        };
        let deleted = if let Some(computer) = binding.computer.as_deref() {
            cleanup_with_client(app, &run, computer).await
        } else {
            true
        };
        if deleted {
            update_run(&app.config.chat_store, &id, |run| {
                run.computer.as_mut().unwrap().cleanup_observed = true;
                if run.cancel && !run.state.finished() {
                    run.end(RunState::Cancelled, Some("Cancelled.".into()));
                }
                (true, ())
            })
            .await?;
            update(
                &app.config.chat_store,
                PUBLIC_QUEUE_KEY,
                fresh_queue,
                |queue| (queue.computer_jobs.remove(&id), ()),
            )
            .await?;
            unqueue(&app.config.chat_store, &id).await;
        }
    }
    Ok(())
}

async fn cleanup_with_client(app: &App, run: &WorkRun, computer: &str) -> bool {
    use openagents_sdk::{CancellationToken, Client, Deployment, computers::WaitFor};
    let Some(cloud) = app.config.cloud.as_deref() else {
        return false;
    };
    let api = app
        .config
        .api_host
        .as_ref()
        .map(|host| format!("https://{host}/v1"))
        .unwrap_or_else(|| format!("{}/v1", cloud.origin()));
    let result: Result<(), String> = async {
        let client = Client::new(Deployment::new(&api, cloud.origin()).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let session = client
            .authenticate_computer_job(&token(app, run))
            .await
            .map_err(|e| e.to_string())?;
        let computers = client.computers(&session).map_err(|e| e.to_string())?;
        let reference = computers.reference(computer).map_err(|e| e.to_string())?;
        let cancel = CancellationToken::new();
        if computers
            .get(&reference, &cancel)
            .await
            .map_err(|e| e.to_string())?
            .data()
            .state
            != "deleted"
        {
            computers
                .delete(&reference, &cancel)
                .await
                .map_err(|e| e.to_string())?;
            computers
                .wait(
                    &reference,
                    WaitFor::Deleted,
                    Duration::from_secs(5),
                    &cancel,
                )
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    .await;
    result.is_ok()
}

/// Rollout affects new jobs only. Saved bindings decide recovery and cleanup.
pub(super) fn prepare(app: &App, who: &Requester, ask: &Ask) -> Result<Option<Binding>, Refused> {
    if ask.engine != Engine::Solver || !enrolled(&who.account) {
        return Ok(None);
    }
    if ask.land != Land::Pr {
        return Err(Refused::Invalid(
            "Solver in the cloud opens a pull request. Choose that option to continue.",
        ));
    }
    if ask.github.is_none() {
        return Err(Refused::Invalid(
            "Connect this repository in Projects before starting Solver.",
        ));
    }
    if app.config.solver_keys.is_none() {
        return Err(Refused::Invalid(
            "Solver's computer setup isn't available right now.",
        ));
    }
    let studio = app.config.environments.studio().ok_or(Refused::Invalid(
        "Save an environment for this repository before starting Solver.",
    ))?;
    let selected = studio
        .list()
        .into_iter()
        .filter(|env| {
            env.account.as_deref() == Some(&who.account)
                && env.repository.eq_ignore_ascii_case(&ask.repo)
                && env.saved.is_some()
        })
        .max_by_key(|env| (env.updated_ms, env.id.clone()))
        .ok_or(Refused::Invalid(
            "Save an environment for this repository before starting Solver.",
        ))?;
    let (project, environment) = studio
        .computer_pin(&who.account, &selected.id, selected.saved.unwrap())
        .map_err(|_| {
            Refused::Invalid("Select a saved environment version before starting Solver.")
        })?;
    if environment
        .source
        .repository
        .as_deref()
        .is_none_or(|repo| !repo.eq_ignore_ascii_case(&ask.repo))
    {
        return Err(Refused::Invalid(
            "Choose an environment for this repository.",
        ));
    }
    let image = std::env::var("OPENAGENTS_SOLVER_PUBLIC_IMAGE")
        .ok()
        .filter(|image| {
            image.rsplit_once("@sha256:").is_some_and(|(_, digest)| {
                digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit())
            })
        })
        .ok_or(Refused::Invalid(
            "Solver's computer setup isn't available right now.",
        ))?;
    Ok(Some(Binding {
        schema: "openagents.solver.computer.v1".into(),
        project,
        branch: selected.branch,
        permission: format!(
            "{:x}",
            Sha256::digest(ask.github.as_ref().unwrap().0.as_bytes())
        ),
        payer: who.account.clone(),
        image,
        create: json!({"environment":environment.environment,"version":environment.number.to_string(),"size":"large","needs":{"command_sessions":true,"profiles":["core"]}}),
        environment,
        computer: None,
        commands: BTreeMap::new(),
        command_ids: Vec::new(),
        cleanup_observed: false,
        vertex: None,
    }))
}

fn credential(app: &App, run: &WorkRun, scope: &str) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(&app.config.ask_salt).expect("HMAC accepts 32 bytes");
    mac.update(b"openagents.solver.computer-job.v1\0");
    mac.update(scope.as_bytes());
    mac.update(&run.requester.members_epoch.to_be_bytes());
    mac.update(&serde_json::to_vec(&json!({"repository":run.repo,"issue":run.issue,"land":run.land,"binding":run.computer.as_ref().map(|b|json!([b.project,b.branch,b.permission,b.environment,b.payer,b.image,b.create]))})).expect("fixed job binding"));
    for field in [
        &run.id,
        &run.requester.account,
        &run.requester.workspace,
        run.host.as_deref().unwrap_or(""),
    ] {
        mac.update(field.as_bytes());
        mac.update(&[0]);
    }
    format!("{scope}{}.{:x}", run.id, mac.finalize().into_bytes())
}

fn token(app: &App, run: &WorkRun) -> String {
    credential(app, run, "scj_")
}

pub(super) fn claim(app: &App, run: &WorkRun) -> Option<Value> {
    let mut binding = serde_json::to_value(run.computer.as_ref()?).ok()?;
    binding.as_object_mut()?.remove("vertex");
    Some(
        json!({"binding":binding,"credential":token(app, run),"account":run.requester.account,"cancelled":run.cancel || run.state.finished()}),
    )
}

async fn recover_binding(app: &App, mut run: WorkRun) -> Result<WorkRun, Error> {
    if run.host.is_none() {
        return Ok(run);
    }
    if run
        .computer
        .as_ref()
        .is_some_and(|binding| binding.computer.is_none())
    {
        let client =
            crate::computers::client().ok_or(Error::Invalid("The computer couldn't be read."))?;
        let key = run.computer.as_ref().unwrap().create["idempotency_key"]
            .as_str()
            .unwrap_or_default();
        match client
            .call(
                reqwest::Method::GET,
                &format!("/v1/computers/by-request/{key}"),
                &run.owner,
                None,
            )
            .await
        {
            Ok((200, value))
                if crate::computers_api::cloud_id(value["id"].as_str().unwrap_or_default()) =>
            {
                let id = value["id"].as_str().unwrap().to_owned();
                update_run(&app.config.chat_store, &run.id, |current| {
                    if let Some(binding) = current.computer.as_mut() {
                        binding.computer = Some(id.clone());
                    }
                    (true, ())
                })
                .await?;
                run.computer.as_mut().unwrap().computer = Some(id);
            }
            Ok((404, _)) => {}
            _ => return Err(Error::Invalid("The computer couldn't be read.")),
        }
    }
    Ok(run)
}

pub(super) async fn claim_answer(app: &App, run: WorkRun) -> Response {
    let run = match recover_binding(app, run).await {
        Ok(run) => run,
        Err(error) => return stored(&error),
    };
    let github = match run.github.as_ref() {
        Some(ticket) => redeem(app, ticket, &run.repo).await,
        None => None,
    };
    let cancelled = github.is_none();
    let mut value = json!({"id":run.id,"repo":run.repo,"issue":run.issue,"land":run.land.word(),"engine":run.engine.word(),"env":{},"github_token":github,"computer":claim(app,&run)});
    if cancelled {
        value["computer"]["cancelled"] = json!(true);
        if !run.cancel && !run.state.finished() {
            value["computer"]["stop_reason"] = json!(
                "GitHub access to this repository ended. Reconnect it before starting another job."
            );
        }
    }
    let mut response = answer(StatusCode::OK, json!({"run":value}));
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

pub(super) async fn resume(
    app: &App,
    host: &str,
    exclude: &[String],
) -> Result<Option<WorkRun>, Error> {
    let Some((bytes, _)) = app.config.chat_store.read_key(PUBLIC_QUEUE_KEY).await? else {
        return Ok(None);
    };
    let queue: Queue = serde_json::from_slice(&bytes)
        .map_err(|_| Error::Invalid("The job list couldn't be read."))?;
    for id in queue.computer_jobs {
        if exclude.contains(&id) {
            continue;
        }
        if let Some(run) = load_plain(&app.config.chat_store, &id).await?
            && run.host.as_deref() == Some(host)
            && run
                .computer
                .as_ref()
                .is_some_and(|binding| !binding.cleanup_observed)
        {
            return Ok(Some(run));
        }
    }
    Ok(None)
}

pub(super) async fn cleaned(
    State(app): State<App>,
    headers: HeaderMap,
    Path((name, id)): Path<(String, String)>,
) -> Response {
    if !host_allowed(&app, &headers) {
        return denied();
    }
    let Some(run) = load_plain(&app.config.chat_store, &id).await.ok().flatten() else {
        return denied();
    };
    if run.host.as_deref() != Some(&name) {
        return denied();
    }
    let Some(binding) = run.computer.as_ref() else {
        return denied();
    };
    if let Some(computer) = binding.computer.as_deref() {
        let Some(client) = crate::computers::client() else {
            return denied();
        };
        if !matches!(client.call(reqwest::Method::GET,&format!("/v1/computers/{computer}"),&run.owner,None).await,Ok((200,ref value)) if value["state"]=="deleted")
        {
            return denied();
        }
    } else if !matches!(recover_binding(&app,run.clone()).await,Ok(ref recovered) if recovered.computer.as_ref().is_some_and(|b|b.computer.is_none()))
    {
        return denied();
    }
    if update_run(&app.config.chat_store, &id, |run| {
        run.computer.as_mut().unwrap().cleanup_observed = true;
        (true, ())
    })
    .await
    .is_err()
    {
        return denied();
    }
    if update(
        &app.config.chat_store,
        PUBLIC_QUEUE_KEY,
        fresh_queue,
        |queue| (queue.computer_jobs.remove(&id), ()),
    )
    .await
    .is_err()
    {
        return denied();
    }
    answer(StatusCode::OK, json!({"cleaned":true}))
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
fn denied() -> Response {
    refused(
        StatusCode::FORBIDDEN,
        "job_scope",
        "This job can't do that.",
    )
}

async fn resolve(app: &App, headers: &HeaderMap) -> Result<WorkRun, Response> {
    resolve_scope(app, headers, "scj_").await
}
async fn resolve_scope(app: &App, headers: &HeaderMap, scope: &str) -> Result<WorkRun, Response> {
    let supplied = bearer(headers).ok_or_else(denied)?;
    let id = supplied
        .strip_prefix(scope)
        .and_then(|v| v.split_once('.'))
        .map(|v| v.0)
        .filter(|v| valid_id(v))
        .ok_or_else(denied)?;
    let run = load_plain(&app.config.chat_store, id)
        .await
        .map_err(|_| denied())?
        .ok_or_else(denied)?;
    let expected = credential(app, &run, scope);
    let equal = expected.len() == supplied.len()
        && expected
            .bytes()
            .zip(supplied.bytes())
            .fold(0u8, |a, (x, y)| a | (x ^ y))
            == 0;
    if !equal
        || run.engine != Engine::Solver
        || run.computer.is_none()
        || run.host.is_none()
        || run.owner != account_owner(&run.requester.account)
        || run.computer.as_ref().is_some_and(|b| {
            b.payer != run.requester.account || b.schema != "openagents.solver.computer.v1"
        })
    {
        return Err(denied());
    }
    Ok(run)
}

pub(crate) async fn identity(
    app: &App,
    headers: &HeaderMap,
) -> Result<crate::computers_api::Identity, Response> {
    let run = resolve(app, headers).await?;
    Ok(crate::computers_api::Identity {
        account: run.requester.account.clone(),
        owner: run.owner,
        permitted: true,
        admin: false,
        environment_attempt: None,
        free_cloud: crate::computers_api::FreeCloud::Full,
    })
}

pub(super) async fn session(State(app): State<App>, headers: HeaderMap) -> Response {
    match resolve(&app, &headers).await {
        Ok(run) => answer(
            StatusCode::OK,
            json!({"account":{"id":run.requester.account,"label":"Solver job"},"workspaces":[]}),
        ),
        Err(response) => response,
    }
}

/// Classify before contacting a driver. Cleanup stays available after cancellation
/// or repository revocation; neither grants permission to create or start work.
fn operation(run: &WorkRun, method: &str, path: &str, body: &Value) -> Result<bool, ()> {
    let binding = run.computer.as_ref().ok_or(())?;
    policy(
        &binding.create,
        binding.computer.as_deref(),
        &binding.commands,
        &binding.command_ids,
        method,
        path,
        body,
    )
}

fn policy(
    create: &Value,
    computer: Option<&str>,
    commands: &BTreeMap<String, Value>,
    command_ids: &[String],
    method: &str,
    path: &str,
    body: &Value,
) -> Result<bool, ()> {
    if method == "GET" && path == "/v1/work/computer-session" {
        return Ok(false);
    }
    if method == "POST" && path == "/v1/computers" {
        return (body == create).then_some(true).ok_or(());
    }
    let computer = computer.ok_or(())?;
    let root = format!("/v1/computers/{computer}");
    if path == root {
        return match method {
            "GET" | "DELETE" => Ok(false),
            _ => Err(()),
        };
    }
    if method == "POST" && path == format!("{root}/stop") {
        return Ok(false);
    }
    if method == "POST" && path == format!("{root}/wake") {
        return Ok(true);
    }
    if method == "POST" && path == format!("{root}/commands") {
        let key = body["idempotency_key"].as_str().ok_or(())?;
        return commands
            .get(key)
            .filter(|saved| *saved == body)
            .map(|_| true)
            .ok_or(());
    }
    for command in command_ids {
        let path_root = format!("{root}/commands/{command}");
        if method == "GET" && (path == path_root || path == format!("{path_root}/output")) {
            return Ok(false);
        }
        if method == "POST" && path == format!("{path_root}/cancel") {
            return Ok(false);
        }
    }
    Err(())
}

async fn may_start(app: &App, run: &WorkRun) -> bool {
    app.config.agent_accounts.contains(&run.requester.account)
        && !run.cancel
        && run.state == RunState::Running
        && now_unix() <= run.created_unix + TICKET_SECONDS
        && match run.github.as_ref() {
            Some(ticket)
                if run.computer.as_ref().is_some_and(|binding| {
                    binding.permission == format!("{:x}", Sha256::digest(ticket.0.as_bytes()))
                }) =>
            {
                redeem(app, ticket, &run.repo).await.is_some()
            }
            Some(_) => false,
            None => false,
        }
}

fn token_aad(run: &WorkRun) -> Vec<u8> {
    serde_json::to_vec(&json!({"v":"openagents.solver.vertex.v1","job":run.id,"requester":run.requester,"host":run.host})).expect("fixed metadata")
}
async fn keep_token(app: &App, run: &WorkRun, value: &str) -> Result<(), Response> {
    if value.is_empty() || value.len() > 8192 || value.chars().any(char::is_control) {
        return Err(denied());
    }
    let keyring = app.config.solver_keys.as_deref().ok_or_else(denied)?;
    let sealed = keyring
        .seal(&token_aad(run), value.as_bytes())
        .map_err(|_| denied())?;
    update_run(&app.config.chat_store, &run.id, |current| {
        if current.state != RunState::Running || current.cancel {
            return (false, false);
        }
        current.computer.as_mut().unwrap().vertex = Some(Vertex {
            value: sealed.clone(),
            expires_unix: now_unix() + 150,
        });
        (true, true)
    })
    .await
    .map_err(|_| denied())?
    .filter(|saved| *saved)
    .ok_or_else(denied)?;
    Ok(())
}

#[derive(Deserialize)]
struct RefreshToken {
    token: String,
}

pub(super) async fn refresh_token(
    State(app): State<App>,
    headers: HeaderMap,
    Path((name, id)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    if !host_allowed(&app, &headers) {
        return denied();
    }
    let Some(run) = load_plain(&app.config.chat_store, &id).await.ok().flatten() else {
        return denied();
    };
    let Ok(sent) = serde_json::from_slice::<RefreshToken>(&body) else {
        return denied();
    };
    if run.host.as_deref() != Some(&name) || !may_start(&app, &run).await {
        return denied();
    }
    match keep_token(&app, &run, &sent.token).await {
        Ok(()) => answer(StatusCode::OK, json!({"refreshed":true})),
        Err(response) => response,
    }
}

/// A separate credential reads only this job's current Vertex token. Encryption
/// in the shared store supports multiple web instances and process restarts.
pub(super) async fn read_token(State(app): State<App>, headers: HeaderMap) -> Response {
    use axum::response::IntoResponse;
    let run = match resolve_scope(&app, &headers, "scv_").await {
        Ok(run) => run,
        Err(response) => return response,
    };
    if !may_start(&app, &run).await {
        return denied();
    }
    let Some(held) = run
        .computer
        .as_ref()
        .and_then(|binding| binding.vertex.as_ref())
        .filter(|held| held.expires_unix > now_unix())
    else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Some(keys) = app.config.solver_keys.as_deref() else {
        return denied();
    };
    let Ok(value) = keys.open(&token_aad(&run), &held.value) else {
        return denied();
    };
    let mut response = Response::new(Body::from(value.to_vec()));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// Restrict job credentials at the outer router as well as the account boundary.
/// No other API group accepts them, including older route aliases.
pub(crate) async fn guard(State(app): State<App>, request: Request, next: Next) -> Response {
    if !bearer(request.headers()).is_some_and(|t| t.starts_with("scj_")) {
        return next.run(request).await;
    }
    let run = match resolve(&app, request.headers()).await {
        Ok(run) => run,
        Err(r) => return r,
    };
    let method = request.method().to_string();
    let path = request.uri().path().to_owned();
    if method == "DELETE" && request.uri().query() != Some("kind=cloud") {
        return denied();
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
            Ok(b) => b,
            Err(_) => return denied(),
        }
    };
    let starts = match operation(&run, &method, &path, &body) {
        Ok(v) => v,
        Err(_) => return denied(),
    };
    if starts && !may_start(&app, &run).await {
        return denied();
    }
    let response = next
        .run(Request::from_parts(parts, Body::from(bytes)))
        .await;
    if !response.status().is_success() || method == "GET" {
        return response;
    }
    let (parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return denied(),
    };
    let value: Value = serde_json::from_slice(&bytes).unwrap_or_default();
    let saved = update_run(&app.config.chat_store, &run.id, |current| {
        let Some(binding) = current.computer.as_mut() else {
            return (false, false);
        };
        if path == "/v1/computers"
            && crate::computers_api::cloud_id(value["id"].as_str().unwrap_or_default())
        {
            let id = value["id"].as_str().unwrap().to_owned();
            if binding.computer.as_ref().is_some_and(|held| held != &id) {
                return (false, false);
            }
            binding.computer = Some(id);
        } else if path.ends_with("/commands")
            && value["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("trn_"))
        {
            let id = value["id"].as_str().unwrap().to_owned();
            if !binding.command_ids.contains(&id) {
                binding.command_ids.push(id);
            }
        }
        (true, true)
    })
    .await;
    if !matches!(saved, Ok(Some(true))) {
        return denied();
    }
    Response::from_parts(parts, Body::from(bytes))
}

#[derive(Deserialize)]
pub(super) struct PrepareCredentials {
    computer: String,
    command: Value,
    values: BTreeMap<String, String>,
}

pub(super) async fn credentials(
    State(app): State<App>,
    headers: HeaderMap,
    Path((name, id)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    if !host_allowed(&app, &headers) {
        return denied();
    }
    let Some(run) = load_plain(&app.config.chat_store, &id).await.ok().flatten() else {
        return denied();
    };
    let Ok(mut sent) = serde_json::from_slice::<PrepareCredentials>(&body) else {
        return denied();
    };
    let Some(binding) = run.computer.as_ref() else {
        return denied();
    };
    if run.host.as_deref() != Some(&name)
        || binding.computer.as_deref() != Some(&sent.computer)
        || !may_start(&app, &run).await
    {
        return denied();
    }
    let key = format!("solver-command-{id}");
    let reference = format!("cred_solver_{id}");
    let command = &sent.command;
    if command["idempotency_key"] != key
        || command["credential_ref"] != reference
        || command["command"]
            .as_str()
            .is_none_or(|v| v.is_empty() || v.len() > 65536)
        || command["timeout_seconds"] != 3600
        || command.get("env").is_some_and(|v| !v.is_null())
        || sent.values.len() != 3
        || [
            "OA_SOLVER_ENV_BODY",
            "OA_SOLVER_GOOGLE_TOKEN",
            "OA_SOLVER_REGISTRY_TOKEN",
        ]
        .iter()
        .any(|key| !sent.values.contains_key(*key))
    {
        return denied();
    }
    let saved = update_run(&app.config.chat_store, &id, |current| {
        let Some(binding) = current.computer.as_mut() else {
            return (false, false);
        };
        if binding
            .commands
            .get(&key)
            .is_some_and(|held| held != command)
        {
            return (false, false);
        }
        binding.commands.insert(key.clone(), command.clone());
        (true, true)
    })
    .await;
    if !matches!(saved, Ok(Some(true))) {
        return denied();
    }
    if let Err(response) = keep_token(&app, &run, &sent.values["OA_SOLVER_GOOGLE_TOKEN"]).await {
        return response;
    }
    let Some(cloud) = app.config.cloud.as_deref() else {
        return denied();
    };
    sent.values.insert(
        "OA_SOLVER_REFRESH_URL".into(),
        format!("{}/v1/work/computer-token", cloud.origin()),
    );
    sent.values.insert(
        "OA_SOLVER_REFRESH_CREDENTIAL".into(),
        credential(&app, &run, "scv_"),
    );
    let values: Vec<_> = sent.values.into_iter().collect();
    match crate::computers_api::commands::prepare_web_credentials(
        &run.owner,
        &sent.computer,
        &reference,
        &key,
        command["command"].as_str().unwrap(),
        &values,
    )
    .await
    {
        Ok(()) => answer(StatusCode::CREATED, json!({"prepared":true})),
        Err(_) => refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "The task couldn't start. Try again later.",
        ),
    }
}

#[cfg(test)]
mod tests;
