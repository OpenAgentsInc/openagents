//! Browser adapter for the same Rust Native Agent Lab workbench mounted on iOS.
use crate::App;
use agent_lab::{
    api::*,
    view::{Command, Workbench},
    *,
};
use axum::{
    Router,
    extract::{DefaultBodyLimit, Form, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use openagents_sdk::{CancellationToken, Client, Deployment, Session};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
#[derive(Serialize, Deserialize)]
struct Frame {
    owner: String,
    request: String,
    workbench: Workbench,
    pending: Option<Operation>,
}
#[derive(Clone, Serialize, Deserialize)]
enum Operation {
    Save(Option<String>, SaveDocument),
    Start(String, StartRun),
    Promote(String, Promote),
    Change(String, RunChange),
}
pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/labs/agent", get(page).post(submit))
        .route("/labs/agent/tasks/{id}", get(task))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
}
async fn access(app: &App, headers: &HeaderMap) -> Result<(Client, Session), Response> {
    let cloud = app
        .config
        .cloud
        .as_deref()
        .ok_or_else(|| Redirect::to("/login").into_response())?;
    let base = app
        .config
        .api_host
        .as_ref()
        .map(|h| format!("https://{h}/v1"))
        .unwrap_or_else(|| format!("{}/v1", cloud.origin()));
    let deployment = Deployment::new(&base, cloud.origin())
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE.into_response())?;
    let client = Client::with_timeouts(
        deployment,
        std::time::Duration::from_secs(5),
        std::time::Duration::from_secs(65),
    )
    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE.into_response())?;
    let session = cloud
        .computer_session(headers, &client)
        .await
        .map_err(|_| Redirect::to("/login").into_response())?;
    Ok((client, session))
}
async fn refresh(
    frame: &mut Frame,
    client: &Client,
    session: &Session,
) -> Result<(), openagents_sdk::Error> {
    let api = client.labs(session)?;
    let cancel = CancellationToken::new();
    let s = &mut frame.workbench;
    let mut data = vec![];
    let mut after = None;
    loop {
        let p = api.documents(after.as_deref(), &cancel).await?;
        data.extend(p.data);
        after = p.next;
        if after.is_none() {
            break;
        }
    }
    if let Some(id) = &s.document {
        if let Ok(d) = api.document(id, &cancel).await {
            data.retain(|d| &d.id != id);
            data.push(d);
        }
    }
    s.documents = data;
    s.runs.clear();
    after = None;
    loop {
        let p = api.runs(None, after.as_deref(), &cancel).await?;
        s.runs.extend(p.data);
        after = p.next;
        if after.is_none() {
            break;
        }
    }
    s.hosts = api.hosts(&cancel).await?;
    s.agents.clear();
    after = None;
    let agents = client.agents(session)?;
    loop {
        let p = agents.list(after.as_deref(), 100, &cancel).await?;
        s.agents.extend(p.data);
        after = p.next;
        if after.is_none() {
            break;
        }
    }
    if s.draft.host == "choose" {
        if let Some(h) = s.hosts.first() {
            s.draft.host = h.id.clone();
            s.draft.project = h
                .projects
                .first()
                .map(|p| p.id.clone())
                .unwrap_or_else(|| "choose".into());
        }
    }
    s.bump();
    Ok(())
}
#[derive(Default, Deserialize)]
struct Open {
    document: Option<String>,
    version: Option<u64>,
}
async fn page(
    State(app): State<App>,
    headers: HeaderMap,
    axum::extract::Query(open): axum::extract::Query<Open>,
) -> Response {
    let (client, session) = match access(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let id = match openagents_sdk::agents::request_id() {
        Ok(v) => v,
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let mut f = Frame {
        owner: session.account().id.clone(),
        request: id.clone(),
        workbench: Workbench::new(id),
        pending: None,
    };
    if refresh(&mut f, &client, &session).await.is_err() {
        f.workbench.notice = Some("Couldn't load Agent Lab. Try Refresh.".into());
    }
    if let Some(id) = open.document {
        if let Ok(api) = client.labs(&session) {
            if let Ok(d) = api.document(&id, &CancellationToken::new()).await {
                if let Some(v) = d
                    .versions
                    .iter()
                    .find(|v| Some(v.number) == open.version)
                    .or_else(|| d.versions.last())
                {
                    if let Ok(Content::Experiment { experiment }) =
                        serde_json::from_value(v.content.clone())
                    {
                        f.workbench.draft = experiment;
                        f.workbench.document = Some(id);
                        f.workbench.base = v.number;
                        f.workbench.dirty = false;
                        f.workbench.documents.retain(|old| old.id != d.id);
                        f.workbench.documents.push(d);
                    }
                }
            }
        }
    }
    render(&app, &headers, f).await
}
async fn render(app: &App, headers: &HeaderMap, f: Frame) -> Response {
    let Some(cloud) = app.config.cloud.as_deref() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let viewer = match cloud.authenticate(headers).await {
        Ok(v) => v,
        Err(_) => return Redirect::to("/login").into_response(),
    };
    let state = match serde_json::to_string(&f) {
        Ok(v) => v,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let ticket = match cloud.csrf(
        headers,
        &viewer,
        "agent-lab",
        &lab_kit::digest(&json!(state)),
    ) {
        Ok(v) => v,
        Err(_) => return StatusCode::FORBIDDEN.into_response(),
    };
    let view = match f.workbench.view().validate() {
        Ok(v) => v,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let body = match rust_native_web::render(&view) {
        Ok(v) => v,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let escape = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    // The adapter forwards native field and activation events. All actions stay in Rust.
    let html = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Agent Lab · OpenAgents</title><style>{}body{{max-width:780px;margin:24px auto;padding:16px;font:16px system-ui}}button,input,textarea{{font:inherit}}textarea{{min-height:100px}}.rn-stack{{gap:12px}}</style></head><body><a href="/">OpenAgents</a><form method="post"><input type="hidden" name="_frame" value="{}"><input type="hidden" name="_csrf" value="{}"><input type="hidden" name="_action" id="action">{}</form><script>document.addEventListener('click',function(e){{let b=e.target.closest('[data-rn-action="activate"]');if(!b||b.disabled)return;let n=b.closest('[data-rn-node]');if(!n)return;document.getElementById('action').value=n.dataset.rnNode;b.form.requestSubmit();}});</script></body></html>"#,
        rust_native_web::CSS,
        escape(&state),
        escape(&ticket),
        body
    );
    (
        [
            ("cache-control", "no-store, private"),
            ("referrer-policy", "same-origin"),
        ],
        Html(html),
    )
        .into_response()
}
async fn submit(
    State(app): State<App>,
    headers: HeaderMap,
    Form(mut fields): Form<BTreeMap<String, String>>,
) -> Response {
    let (client, session) = match access(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(raw) = fields.remove("_frame") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(ticket) = fields.remove("_csrf") else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let Some(cloud) = app.config.cloud.as_deref() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let viewer = match cloud.authenticate(&headers).await {
        Ok(v) => v,
        Err(_) => return StatusCode::FORBIDDEN.into_response(),
    };
    if cloud
        .verify_csrf(
            &headers,
            Some(&viewer),
            "agent-lab",
            &lab_kit::digest(&json!(raw)),
            &ticket,
        )
        .is_err()
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let mut f: Frame = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if f.owner != session.account().id {
        return StatusCode::FORBIDDEN.into_response();
    }
    let action = fields.remove("_action").unwrap_or_default();
    if let Some(id) = action.strip_prefix("session:") {
        let valid = f.workbench.view().validate().is_ok_and(|v| {
            v.activate(&rust_native::Activation {
                instance: f.workbench.instance.clone(),
                revision: f.workbench.revision,
                node: action.clone(),
            })
            .is_ok()
        });
        if valid {
            return Redirect::to(&format!("/labs/agent/tasks/{id}")).into_response();
        }
    }
    let result = apply(&mut f, &client, &session, &fields, &action).await;
    if let Err(e) = result {
        f.workbench.notice = Some(e);
    }
    f.workbench.pending = f.pending.is_some();
    f.workbench.busy = false;
    f.workbench.bump();
    render(&app, &headers, f).await
}
async fn apply(
    f: &mut Frame,
    client: &Client,
    session: &Session,
    fields: &BTreeMap<String, String>,
    key: &str,
) -> Result<(), String> {
    let s = &mut f.workbench;
    let view = s.view().validate().map_err(|_| "Reload Agent Lab.")?;
    let event = rust_native::Activation {
        instance: s.instance.clone(),
        revision: s.revision,
        node: key.into(),
    };
    let action = view
        .activate(&event)
        .map_err(|_| "Choose an available action.")?
        .0
        .clone();
    // Validate submitted edits against the original frame before applying any of them.
    let mut edits = vec![];
    for (key, value) in fields {
        let change = rust_native::FieldChange {
            activation: rust_native::Activation {
                instance: s.instance.clone(),
                revision: s.revision,
                node: key.clone(),
            },
            value: value.clone(),
        };
        if let Ok(intent) = view.change_field(&change) {
            edits.push((intent.0.clone(), value.clone()));
        }
    }
    let before = s.draft.clone();
    let was_dirty = s.dirty;
    for (key, value) in edits {
        s.field(&key, value)?;
    }
    if s.draft == before {
        s.dirty = was_dirty;
    }
    if matches!(action.as_str(), "start" | "promote") && s.dirty {
        return Err("Save your changes before using this version.".into());
    }
    let command = s.action(&action)?;
    let Some(command) = command else {
        return Ok(());
    };
    let cancel = CancellationToken::new();
    let api = client.labs(session).map_err(|e| e.to_string())?;
    match command {
        Command::Refresh => {
            refresh(f, client, session)
                .await
                .map_err(|_| "Couldn't refresh Agent Lab.")?;
            return Ok(());
        }
        Command::Load(id) => {
            let d = api
                .document(&id, &cancel)
                .await
                .map_err(|e| e.to_string())?;
            let v = d.versions.last().ok_or("Choose a saved experiment.")?;
            if let Content::Experiment { experiment } = serde_json::from_value(v.content.clone())
                .map_err(|_| "Couldn't open this experiment.")?
            {
                s.draft = experiment;
                s.document = Some(id.clone());
                s.base = v.number;
                s.dirty = false;
                s.inputs.clear();
                s.documents.retain(|d| d.id != id);
                s.documents.push(d);
            }
            return Ok(());
        }
        Command::Session(_) => return Err("Open this conversation in the OpenAgents app.".into()),
        Command::Retry => {}
        command => {
            let id = f.request.clone();
            f.pending = Some(match command {
                Command::Save => Operation::Save(
                    s.document.clone(),
                    SaveDocument {
                        request_id: id,
                        expected_revision: s
                            .documents
                            .iter()
                            .find(|d| Some(&d.id) == s.document.as_ref())
                            .and_then(|d| d.versions.last())
                            .map(|v| v.number)
                            .unwrap_or(0),
                        content: Content::Experiment {
                            experiment: s.draft.clone(),
                        },
                        summary: "Saved experiment".into(),
                    },
                ),
                Command::Start => Operation::Start(
                    s.document.clone().ok_or("Save this experiment first.")?,
                    StartRun {
                        request_id: id,
                        version: s.base,
                    },
                ),
                Command::Promote => Operation::Promote(
                    s.document.clone().ok_or("Save this experiment first.")?,
                    Promote {
                        request_id: id,
                        version: s.base,
                    },
                ),
                Command::Stop(run) | Command::Archive(run) => {
                    let r = s.runs.iter().find(|r| r.id == run).ok_or("Choose a run.")?;
                    Operation::Change(
                        run,
                        RunChange {
                            request_id: id,
                            expected_revision: r.revision,
                            action: if action.starts_with("stop:") {
                                RunAction::Stop
                            } else {
                                RunAction::Archive
                            },
                        },
                    )
                }
                Command::Review { run, trial, passed } => {
                    let r = s.runs.iter().find(|r| r.id == run).ok_or("Choose a run.")?;
                    Operation::Change(
                        run.clone(),
                        RunChange {
                            request_id: id,
                            expected_revision: r.revision,
                            action: RunAction::Review {
                                trial,
                                passed,
                                note: s
                                    .review_notes
                                    .get(&format!("review-note:{run}:{trial}"))
                                    .cloned()
                                    .unwrap_or_default(),
                            },
                        },
                    )
                }
                _ => return Err("Choose an available action.".into()),
            });
        }
    }
    let Some(op) = f.pending.clone() else {
        return Err("There is no saved change to retry.".into());
    };
    let result: Result<(), openagents_sdk::Error> = async {
        match op {
            Operation::Save(id, body) => {
                let d = api.save(id.as_deref(), &api.bind(body), &cancel).await?;
                s.document = Some(d.id.clone());
                s.base = d.versions.last().map(|v| v.number).unwrap_or(0);
                s.dirty = false;
                s.documents.retain(|old| old.id != d.id);
                s.documents.push(d);
            }
            Operation::Start(id, body) => {
                let r = api.start(&id, &api.bind(body), &cancel).await?;
                s.selected_run = Some(r.id.clone());
                s.runs.retain(|old| old.id != r.id);
                s.runs.push(r);
            }
            Operation::Promote(id, body) => {
                let d = api.promote(&id, &api.bind(body), &cancel).await?;
                s.notice = Some(format!("{} is ready in your agent picker.", d.spec.name));
            }
            Operation::Change(id, body) => {
                let r = api.change(&id, &api.bind(body), &cancel).await?;
                s.runs.retain(|old| old.id != r.id);
                if !r.archived {
                    s.runs.push(r)
                }
            }
        }
        Ok(())
    }
    .await;
    match result {
        Ok(()) => {
            f.pending = None;
            f.request = openagents_sdk::agents::request_id().map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(e) => {
            if matches!(
                e,
                openagents_sdk::Error::Refused { .. }
                    | openagents_sdk::Error::Forbidden
                    | openagents_sdk::Error::InvalidReference
            ) {
                f.pending = None;
                f.request = openagents_sdk::agents::request_id().map_err(|e| e.to_string())?;
            }
            Err(e.to_string())
        }
    }
}

async fn task(
    State(app): State<App>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let (client, session) = match access(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let agents = match client.agents(&session) {
        Ok(a) => a,
        Err(_) => return StatusCode::FORBIDDEN.into_response(),
    };
    // Conversations use a different ID from assignments; resolve only through this account's Lab reports.
    let labs = client.labs(&session).unwrap();
    let cancel = CancellationToken::new();
    let mut after = None;
    let mut assignment = None;
    loop {
        let page = match labs.runs(None, after.as_deref(), &cancel).await {
            Ok(p) => p,
            Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        };
        for run in page.data {
            for trial in run.trials {
                if trial.chat.as_deref() == Some(&id) {
                    assignment = Some(trial.assignment)
                }
            }
        }
        after = page.next;
        if assignment.is_some() || after.is_none() {
            break;
        }
    }
    let Some(assignment) = assignment else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let a = match agents.assignment(&assignment, &cancel).await {
        Ok(a) => a,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let escape = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    ([("cache-control","no-store, private")],Html(format!("<!doctype html><meta name=viewport content=width=device-width,initial-scale=1><title>Agent Lab task</title><a href=/labs/agent>Agent Lab</a><h1>{}</h1><h2>Task</h2><pre style='white-space:pre-wrap'>{}</pre><h2>Result</h2><pre style='white-space:pre-wrap'>{}</pre><p>Open this task in the mobile app to read its conversation.</p>",escape(&a.definition.spec.name),escape(&a.spec.task),escape(a.result.as_deref().unwrap_or("The result is not available yet."))))).into_response()
}
