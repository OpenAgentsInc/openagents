//! Private Labs storage and an enrolled-host provider using the existing assignment ledger.
use crate::{
    App,
    chat_store::{Error, Store},
    coder_sync::{answer, refused, stored},
};
use agent_contract::valid_id;
use agent_lab::{api::*, *};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
};
use lab_kit::LabKind;
use openagents_sdk::agents::assignments::{
    AssignmentAction, AssignmentCommand, AssignmentSpec, AssignmentState, CreateAssignment,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
const MAX_BYTES: usize = 16 * 1024 * 1024;
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Book {
    hosts: BTreeMap<String, Host>,
    documents: BTreeMap<String, Document>,
    runs: BTreeMap<String, Run>,
    receipts: BTreeMap<String, (String, Value)>,
}
fn invalid(message: &str) -> Response {
    refused(StatusCode::BAD_REQUEST, "invalid_lab", message)
}
fn missing() -> Response {
    refused(
        StatusCode::NOT_FOUND,
        "not_found",
        "This experiment is no longer available.",
    )
}
fn conflict() -> Response {
    refused(
        StatusCode::CONFLICT,
        "version_conflict",
        "This experiment changed. Reload it before saving.",
    )
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
async fn load(store: &Store, owner: &str) -> Result<(Book, Option<String>), Error> {
    let key = Store::owner_key(owner, "labs-agent.json")?;
    match store.read_key(&key).await? {
        None => Ok((Book::default(), None)),
        Some((v, g)) if v.len() <= MAX_BYTES => Ok((
            serde_json::from_slice(&v).map_err(|_| Error::Corrupt("Couldn't open Agent Lab."))?,
            Some(g),
        )),
        _ => Err(Error::Corrupt("Couldn't open Agent Lab.")),
    }
}
async fn mutation<F>(app: &App, owner: &str, request: &str, body: &Value, f: F) -> Response
where
    F: Fn(&mut Book) -> Result<Value, Response>,
{
    if !valid_id(request) {
        return invalid("Start this request again.");
    }
    let fingerprint = lab_kit::digest(body);
    for _ in 0..16 {
        let (mut b, g) = match load(&app.config.chat_store, owner).await {
            Ok(b) => b,
            Err(e) => return stored(&e),
        };
        if let Some((fp, result)) = b.receipts.get(request) {
            return if fp == &fingerprint {
                answer(StatusCode::OK, result.clone())
            } else {
                conflict()
            };
        }
        if b.receipts.len() >= 8192 {
            return refused(
                StatusCode::TOO_MANY_REQUESTS,
                "quota_exhausted",
                "Your experiment storage is full.",
            );
        }
        let v = match f(&mut b) {
            Ok(v) => v,
            Err(r) => return r,
        };
        b.receipts
            .insert(request.into(), (fingerprint.clone(), v.clone()));
        let bytes = match serde_json::to_vec(&b) {
            Ok(v) if v.len() <= MAX_BYTES => v,
            _ => {
                return refused(
                    StatusCode::TOO_MANY_REQUESTS,
                    "quota_exhausted",
                    "Your experiment storage is full.",
                );
            }
        };
        let key = match Store::owner_key(owner, "labs-agent.json") {
            Ok(k) => k,
            Err(e) => return stored(&e),
        };
        match app
            .config
            .chat_store
            .write_key(&key, bytes, g.as_deref())
            .await
        {
            Ok(_) => return answer(StatusCode::OK, v),
            Err(Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    conflict()
}
fn experiment(b: &Book, id: &str, version: u64) -> Result<Experiment, Response> {
    let v = b
        .documents
        .get(id)
        .filter(|d| !d.deleted)
        .and_then(|d| d.versions.iter().find(|v| v.number == version))
        .ok_or_else(missing)?;
    match serde_json::from_value::<Content>(v.content.clone()) {
        Ok(Content::Experiment { experiment }) => Ok(experiment),
        _ => Err(invalid("Choose an experiment document.")),
    }
}
pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/labs/agent/kinds", get(kinds))
        .route("/v1/labs/agent/hosts", get(hosts).post(heartbeat))
        .route("/v1/labs/agent/documents", get(documents).post(create))
        .route(
            "/v1/labs/agent/documents/{id}",
            get(document).put(update).delete(delete_document),
        )
        .route("/v1/labs/agent/documents/{id}/jobs", post(start))
        .route("/v1/labs/agent/documents/{id}/promote", post(promote))
        .route("/v1/labs/agent/jobs", get(runs))
        .route("/v1/labs/agent/jobs/{id}", get(run))
        .route("/v1/labs/agent/jobs/{id}/actions", post(change))
        .route("/v1/labs/agent/jobs/{id}/host", post(host_update))
        .layer(DefaultBodyLimit::max(192 * 1024))
}
async fn kinds(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Err(r) = crate::coder_sync::owner(&app, &headers).await {
        return r;
    }
    answer(StatusCode::OK, json!({"data":[AgentLab::manifest()]}))
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct List {
    after: Option<String>,
    host: Option<String>,
}
async fn documents(State(app): State<App>, headers: HeaderMap, Query(q): Query<List>) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let (b, _) = match load(&app.config.chat_store, &owner).await {
        Ok(b) => b,
        Err(e) => return stored(&e),
    };
    let mut data: Vec<_> = b
        .documents
        .values()
        .filter(|d| !d.deleted && q.after.as_ref().is_none_or(|a| d.id > *a))
        .take(17)
        .cloned()
        .collect();
    let next = (data.len() > 16).then(|| data[15].id.clone());
    data.truncate(16);
    for d in &mut data {
        if let Some(v) = d.versions.pop() {
            d.versions = vec![v];
        }
    }
    answer(StatusCode::OK, json!(Page { data, next }))
}
async fn document(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    match load(&app.config.chat_store, &owner).await {
        Ok((b, _)) => b
            .documents
            .get(&id)
            .map(|d| answer(StatusCode::OK, json!(d)))
            .unwrap_or_else(missing),
        Err(e) => stored(&e),
    }
}
async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<SaveDocument>,
) -> Response {
    save(app, headers, None, body).await
}
async fn update(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SaveDocument>,
) -> Response {
    save(app, headers, Some(id), body).await
}
async fn save(app: App, headers: HeaderMap, id: Option<String>, body: SaveDocument) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    if let Err(e) = body.content.validate() {
        return invalid(&e.message);
    }
    if body.summary.trim().is_empty() || body.summary.len() > 160 {
        return invalid("Add a short change summary.");
    }
    let content = json!(body.content);
    let report = match AgentLab::check(&content, &Value::Null) {
        Ok(r) => r,
        Err(e) => return invalid(&e.message),
    };
    let fingerprint = json!({"id":id,"save":body});
    mutation(&app, &owner, &body.request_id, &fingerprint, |b| {
        let key = id.as_ref().unwrap_or(&body.request_id);
        if !valid_id(key) {
            return Err(missing());
        }
        if id.is_none() && b.documents.len() >= 64 {
            return Err(invalid("Your experiment list is full."));
        }
        if id.is_some() && !b.documents.contains_key(key) {
            return Err(missing());
        }
        let d = b.documents.entry(key.clone()).or_insert(Document {
            id: key.clone(),
            versions: vec![],
            deleted: false,
        });
        if d.versions.len() as u64 != body.expected_revision || d.deleted || d.versions.len() >= 64
        {
            return Err(conflict());
        }
        let n = d.versions.len() as u64 + 1;
        d.versions.push(lab_kit::Version {
            document: key.clone(),
            number: n,
            parent: (n > 1).then_some(n - 1),
            author: owner.clone(),
            summary: body.summary.clone(),
            created_unix_ms: now(),
            schema: content["schema"].as_str().unwrap_or_default().into(),
            content: content.clone(),
            content_sha256: lab_kit::digest(&content),
            report: report.clone(),
            assets: vec![],
        });
        Ok(json!(d))
    })
    .await
}
async fn start(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<StartRun>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let (b, _) = match load(&app.config.chat_store, &owner).await {
        Ok(b) => b,
        Err(e) => return stored(&e),
    };
    if let Some((fp, result)) = b.receipts.get(&body.request_id) {
        return if fp == &lab_kit::digest(&json!({"document":id,"start":body})) {
            answer(StatusCode::OK, result.clone())
        } else {
            conflict()
        };
    }
    let e = match experiment(&b, &id, body.version) {
        Ok(e) => e,
        Err(r) => return r,
    };
    if let Err(e) = e.validate() {
        return invalid(&e.message);
    }
    let mut candidates = vec![agent_contract::AgentDefinition {
        id: lab_kit::digest(&json!({"run":body.request_id,"candidate":0})),
        revision: 1,
        spec: e.draft.spec.clone(),
        deleted: false,
    }];
    if let Some(pin) = &e.compare {
        match crate::custom_agents::definition(
            &app.config.chat_store,
            &owner,
            &pin.id,
            Some(pin.revision),
        )
        .await
        {
            Ok(Ok(d)) if !d.deleted => candidates.push(d),
            Ok(Err(r)) => return r.response(),
            Err(e) => return stored(&e),
            _ => return invalid("Choose an available comparison agent."),
        }
    }
    mutation(
        &app,
        &owner,
        &body.request_id,
        &json!({"document":id,"start":body}),
        |b| {
            if b.runs
                .values()
                .filter(|r| !r.archived && !r.complete())
                .count()
                >= 8
                || b.runs.len() >= 128
            {
                return Err(invalid("Finish or archive another experiment first."));
            }
            if b.runs.contains_key(&body.request_id) {
                return Err(conflict());
            }
            let mut trials = vec![];
            for candidate in 0..candidates.len() {
                for repetition in 0..e.repetitions {
                    for case in 0..e.suite.cases.len() {
                        let index = trials.len();
                        trials.push(Trial {
                            index,
                            candidate,
                            case,
                            repetition,
                            assignment: trial_id(&body.request_id, index),
                            chat: None,
                            outcome: Outcome::Pending,
                            result: None,
                            evidence: None,
                            started_ms: None,
                            finished_ms: None,
                            cost_usd: None,
                            reviewer: None,
                            review_note: None,
                        });
                    }
                }
            }
            let time = now();
            let run = Run {
                id: body.request_id.clone(),
                revision: 1,
                version: lab_kit::Reference {
                    document: id.clone(),
                    version: body.version,
                },
                experiment: e.clone(),
                candidates: candidates.clone(),
                trials,
                created_ms: time,
                deadline_ms: time + e.max_seconds * 1000,
                stopping: false,
                archived: false,
                host_settled: false,
            };
            b.runs.insert(run.id.clone(), run.clone());
            Ok(json!(run))
        },
    )
    .await
}
async fn runs(State(app): State<App>, headers: HeaderMap, Query(q): Query<List>) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let (b, _) = match load(&app.config.chat_store, &owner).await {
        Ok(b) => b,
        Err(e) => return stored(&e),
    };
    let mut data: Vec<_> = b
        .runs
        .values()
        .filter(|r| {
            (if q.host.is_some() {
                !r.host_settled
            } else {
                !r.archived
            }) && q.after.as_ref().is_none_or(|a| r.id > *a)
                && q.host.as_ref().is_none_or(|h| &r.experiment.host == h)
        })
        .take(9)
        .cloned()
        .collect();
    let next = (data.len() > 8).then(|| data[7].id.clone());
    data.truncate(8);
    answer(StatusCode::OK, json!(Page { data, next }))
}
async fn run(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    match load(&app.config.chat_store, &owner).await {
        Ok((b, _)) => b
            .runs
            .get(&id)
            .map(|r| answer(StatusCode::OK, json!(r)))
            .unwrap_or_else(missing),
        Err(e) => stored(&e),
    }
}
async fn change(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<RunChange>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    mutation(
        &app,
        &owner,
        &body.request_id,
        &json!({"id":id,"change":body}),
        |b| {
            let r = b.runs.get_mut(&id).ok_or_else(missing)?;
            if r.revision != body.expected_revision {
                return Err(conflict());
            }
            match &body.action {
                RunAction::Stop => r.stopping = true,
                RunAction::Archive => {
                    if !r.complete() {
                        return Err(invalid(
                            "Stop this experiment and wait for it to finish first.",
                        ));
                    }
                    r.archived = true;
                    r.host_settled = false
                }
                RunAction::Review {
                    trial,
                    passed,
                    note,
                } => {
                    if note.trim().is_empty() || note.len() > 2048 {
                        return Err(invalid("Add a short review note."));
                    }
                    let t = r.trials.get_mut(*trial).ok_or_else(missing)?;
                    if t.outcome != Outcome::NeedsReview {
                        return Err(conflict());
                    }
                    t.outcome = if *passed {
                        Outcome::Passed
                    } else {
                        Outcome::Failed
                    };
                    t.reviewer = Some(owner.clone());
                    t.review_note = Some(note.clone());
                }
            }
            r.revision += 1;
            Ok(json!(r))
        },
    )
    .await
}
async fn promote(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Promote>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let response=mutation(&app,&owner,&body.request_id,&json!({"promote":body,"document":id}),|b|{
        let e=experiment(b,&id,body.version)?;let spec=e.draft.agent_spec().map_err(|e|invalid(&e.message))?;
        Ok(json!({"spec":spec,"source":e.draft.source,"request":lab_kit::digest(&json!({"document":id,"version":body.version,"request":body.request_id}))}))
    }).await;
    if !response.status().is_success() {
        return response;
    }
    let bytes = match axum::body::to_bytes(response.into_body(), 32768).await {
        Ok(b) => b,
        Err(_) => return invalid("Couldn't open this saved change."),
    };
    #[derive(Deserialize)]
    struct Intent {
        spec: agent_contract::AgentSpec,
        source: Option<Pin>,
        request: String,
    }
    let intent: Intent = match serde_json::from_slice(&bytes) {
        Ok(i) => i,
        Err(_) => return invalid("Couldn't open this saved change."),
    };
    let mutation = if let Some(source) = intent.source {
        crate::custom_agents::Mutation::Update {
            id: source.id,
            request_id: intent.request,
            expected_revision: source.revision,
            spec: intent.spec,
        }
    } else {
        crate::custom_agents::Mutation::Create {
            request_id: intent.request,
            spec: intent.spec,
        }
    };
    match crate::custom_agents::mutate(&app.config.chat_store, &owner, mutation).await {
        Ok(Ok(d)) => answer(StatusCode::OK, json!(d)),
        Ok(Err(r)) => r.response(),
        Err(e) => stored(&e),
    }
}
async fn host_update(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<HostUpdate>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    if matches!(body.action, HostAction::Advance) {
        let (b, _) = match load(&app.config.chat_store, &owner).await {
            Ok(b) => b,
            Err(e) => return stored(&e),
        };
        if b.runs
            .get(&id)
            .is_none_or(|r| r.experiment.host != body.host)
        {
            return missing();
        }
        return match synchronize(&app, &owner, &id).await {
            Ok(r) => answer(StatusCode::OK, json!(r)),
            Err(r) => r,
        };
    }
    let response = mutation(
        &app,
        &owner,
        &body.request_id,
        &json!({"id":id,"host":body}),
        |b| {
            let r = b.runs.get_mut(&id).ok_or_else(missing)?;
            if r.experiment.host != body.host {
                return Err(missing());
            }
            match &body.action {
                HostAction::Advance => {}
                HostAction::Settled => {
                    if !r.complete() {
                        return Err(conflict());
                    }
                    r.host_settled = true
                }
                HostAction::Ready { trial, evidence } => {
                    if r.stopping || now() >= r.deadline_ms {
                        return Err(invalid("This experiment has stopped."));
                    }
                    if !valid_id(&evidence.source_sha256)
                        || !valid_id(&evidence.context_sha256)
                        || evidence.project != r.experiment.project
                        || evidence
                            .context
                            .as_ref()
                            .is_some_and(|s| s.len() > 16384 || s.contains('\0'))
                        || !valid_id(&evidence.runtime_version)
                        || evidence.context_sha256
                            != lab_kit::digest(&json!(
                                evidence.context.as_deref().unwrap_or_default()
                            ))
                    {
                        return Err(invalid("Check the project copy and runtime."));
                    }
                    if r.trials.iter().any(|t| {
                        t.evidence.as_ref().is_some_and(|e| {
                            e.source_sha256 != evidence.source_sha256
                                || e.context_sha256 != evidence.context_sha256
                        })
                    }) {
                        return Err(invalid(
                            "The project or references changed. Start a new experiment.",
                        ));
                    }
                    let next = r
                        .trials
                        .iter()
                        .find(|t| !t.outcome.finished())
                        .map(|t| t.index);
                    if next != Some(*trial) {
                        return Err(conflict());
                    }
                    let t = &mut r.trials[*trial];
                    if let Some(old) = &t.evidence {
                        if old != evidence {
                            return Err(conflict());
                        }
                    } else {
                        t.evidence = Some(evidence.clone());
                        t.started_ms = Some(now());
                    }
                }
                HostAction::Unavailable { trial, reason } => {
                    if reason.is_empty() || reason.len() > 1024 {
                        return Err(invalid("Add a short reason."));
                    }
                    let t = r.trials.get_mut(*trial).ok_or_else(missing)?;
                    if t.outcome != Outcome::Pending || t.evidence.is_some() {
                        return Err(conflict());
                    }
                    t.outcome = Outcome::Unavailable;
                    t.result = Some(reason.clone());
                    t.finished_ms = Some(now());
                }
            }
            r.revision += 1;
            Ok(json!({"saved":true}))
        },
    )
    .await;
    if !response.status().is_success() {
        return response;
    }
    match synchronize(&app, &owner, &id).await {
        Ok(r) => answer(StatusCode::OK, json!(r)),
        Err(r) => r,
    }
}
async fn synchronize(app: &App, owner: &str, id: &str) -> Result<Run, Response> {
    for _ in 0..16 {
        let (mut book, g) = load(&app.config.chat_store, owner)
            .await
            .map_err(|e| stored(&e))?;
        let r = book.runs.get_mut(id).ok_or_else(missing)?;
        let before = r.clone();
        let timed_out = now() >= r.deadline_ms;
        let stopping = r.stopping || timed_out;
        let (assignments, _) = crate::agent_assignments::load(&app.config.chat_store, owner)
            .await
            .map_err(|e| stored(&e))?;
        for t in &mut r.trials {
            if t.outcome.finished() {
                continue;
            }
            if let Some(a) = assignments.assignments.get(&t.assignment) {
                let candidate = &r.candidates[t.candidate];
                let expected_task = format!(
                    "{}{}",
                    t.evidence
                        .as_ref()
                        .and_then(|e| e.context.as_deref())
                        .map(|c| format!("{c}\n\n"))
                        .unwrap_or_default(),
                    r.experiment.task(t.case)
                );
                if t.evidence.is_none()
                    || a.definition != *candidate
                    || a.spec.agent != candidate.id
                    || a.spec.agent_revision != candidate.revision
                    || a.spec.host != r.experiment.host
                    || a.spec.project.as_deref() != Some(r.experiment.project.as_str())
                    || a.spec.task != expected_task
                {
                    t.outcome = Outcome::Unknown;
                    t.result = Some("This task does not match the saved test.".into());
                    t.finished_ms = Some(now());
                    continue;
                }
                t.chat = Some(a.chat.clone());
                t.result = a.result.clone();
                t.outcome = match a.state {
                    AssignmentState::Queued | AssignmentState::Running => {
                        if stopping {
                            let command = AssignmentCommand {
                                request_id: lab_kit::digest(&json!({"stop":t.assignment})),
                                action: AssignmentAction::Stop,
                            };
                            match crate::agent_assignments::mutate(
                                &app.config.chat_store,
                                owner,
                                crate::agent_assignments::Change::Command {
                                    id: a.id.clone(),
                                    body: command,
                                },
                            )
                            .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => return Err(e.response()),
                                Err(e) => return Err(stored(&e)),
                            }
                        }
                        Outcome::Running
                    }
                    AssignmentState::Completed => match a.result.as_deref() {
                        Some(v) if v.len() < 4096 => {
                            match r.experiment.suite.cases[t.case].check.evaluate(v) {
                                Some(true) => Outcome::Passed,
                                Some(false) => Outcome::Failed,
                                None => Outcome::NeedsReview,
                            }
                        }
                        _ => Outcome::Unknown,
                    },
                    AssignmentState::Failed => Outcome::Failed,
                    AssignmentState::Stopped => {
                        if timed_out {
                            Outcome::TimedOut
                        } else {
                            Outcome::Stopped
                        }
                    }
                    AssignmentState::Unknown => Outcome::Unknown,
                };
                if t.outcome.finished() {
                    t.finished_ms = Some(now());
                }
            } else if stopping {
                t.outcome = if timed_out {
                    Outcome::TimedOut
                } else {
                    Outcome::Stopped
                };
                t.finished_ms = Some(now());
            } else if let Some(evidence) = &t.evidence {
                let definition = &r.candidates[t.candidate];
                if t.candidate == 0 {
                    let change = crate::custom_agents::Mutation::Create {
                        request_id: definition.id.clone(),
                        spec: definition.spec.clone(),
                    };
                    match crate::custom_agents::mutate(&app.config.chat_store, owner, change).await
                    {
                        Ok(Ok(_)) => {}
                        Ok(Err(e)) => return Err(e.response()),
                        Err(e) => return Err(stored(&e)),
                    }
                }
                let body = CreateAssignment {
                    request_id: t.assignment.clone(),
                    spec: AssignmentSpec {
                        agent: definition.id.clone(),
                        agent_revision: definition.revision,
                        host: r.experiment.host.clone(),
                        project: Some(evidence.project.clone()),
                        task: format!(
                            "{}{}",
                            evidence
                                .context
                                .as_deref()
                                .map(|c| format!("{c}\n\n"))
                                .unwrap_or_default(),
                            r.experiment.task(t.case)
                        ),
                    },
                };
                match crate::agent_assignments::mutate(
                    &app.config.chat_store,
                    owner,
                    crate::agent_assignments::Change::Create {
                        body,
                        definition: definition.clone(),
                    },
                )
                .await
                {
                    Ok(Ok(a)) => {
                        t.chat = Some(a.chat);
                        t.outcome = Outcome::Running
                    }
                    Ok(Err(e)) => return Err(e.response()),
                    Err(e) => return Err(stored(&e)),
                }
            }
        }
        if *r == before {
            return Ok(r.clone());
        }
        r.revision += 1;
        let result = r.clone();
        let bytes =
            serde_json::to_vec(&book).map_err(|_| invalid("Couldn't save this experiment."))?;
        if bytes.len() > MAX_BYTES {
            return Err(invalid("Your experiment storage is full."));
        }
        let key = Store::owner_key(owner, "labs-agent.json").map_err(|e| stored(&e))?;
        match app
            .config
            .chat_store
            .write_key(&key, bytes, g.as_deref())
            .await
        {
            Ok(_) => return Ok(result),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(stored(&e)),
        }
    }
    Err(conflict())
}
async fn hosts(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    match load(&app.config.chat_store, &owner).await {
        Ok((b, _)) => answer(
            StatusCode::OK,
            json!(
                b.hosts
                    .into_values()
                    .filter(|h| now().saturating_sub(h.seen_ms) < 90_000)
                    .collect::<Vec<_>>()
            ),
        ),
        Err(e) => stored(&e),
    }
}
async fn heartbeat(
    State(app): State<App>,
    headers: HeaderMap,
    Json(mut host): Json<Host>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    if host.id.is_empty()
        || host.id.len() > 128
        || host.name.is_empty()
        || host.name.len() > 128
        || host.projects.len() > 256
        || host.runners.len() > 128
        || host.runtimes.len() > 32
        || host.tools.len() > 32
        || host
            .projects
            .iter()
            .any(|p| p.id.is_empty() || p.id.len() > 128 || p.name.len() > 256)
        || host.runners.iter().any(|s| s.len() > 256)
    {
        return invalid("Check this computer's Agent Lab settings.");
    }
    for _ in 0..16 {
        let (mut b, g) = match load(&app.config.chat_store, &owner).await {
            Ok(b) => b,
            Err(e) => return stored(&e),
        };
        host.seen_ms = now();
        if let Some(previous) = b.hosts.get(&host.id) {
            if now().saturating_sub(previous.seen_ms) < 30_000
                && previous
                    .projects
                    .iter()
                    .map(|p| &p.id)
                    .eq(host.projects.iter().map(|p| &p.id))
            {
                return answer(StatusCode::OK, json!(previous));
            }
        }
        if b.hosts.len() >= 64 && !b.hosts.contains_key(&host.id) {
            return invalid("Your computer list is full.");
        }
        b.hosts.insert(host.id.clone(), host.clone());
        let key = match Store::owner_key(&owner, "labs-agent.json") {
            Ok(k) => k,
            Err(e) => return stored(&e),
        };
        let bytes = match serde_json::to_vec(&b) {
            Ok(v) if v.len() <= MAX_BYTES => v,
            _ => return invalid("Your experiment storage is full."),
        };
        match app
            .config
            .chat_store
            .write_key(&key, bytes, g.as_deref())
            .await
        {
            Ok(_) => return answer(StatusCode::OK, json!(host)),
            Err(Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    conflict()
}

pub(super) async fn hidden_definitions(
    store: &Store,
    owner: &str,
) -> Result<std::collections::BTreeSet<String>, Error> {
    let (b, _) = load(store, owner).await?;
    Ok(b.runs
        .values()
        .filter_map(|r| r.candidates.first().map(|d| d.id.clone()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn sdk_jobs_replay_isolate_stop_and_report() {
        use axum::response::IntoResponse;
        use std::{os::unix::fs::PermissionsExt, sync::Arc};
        async fn auth(headers: HeaderMap) -> Response {
            let token = headers
                .get("authorization")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("");
            let id = if token == format!("Bearer sess_{}", "a".repeat(64)) {
                "agent-api-alice"
            } else if token == format!("Bearer sess_{}", "b".repeat(64)) {
                "agent-api-bob"
            } else {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error":{"code":"signed_out"}})),
                )
                    .into_response();
            };
            Json(json!({"account":{"id":id,"label":id,"admin":false},"workspaces":[]}))
                .into_response()
        }
        async fn session(headers: HeaderMap) -> Response {
            let response = auth(headers).await;
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap();
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            if status != StatusCode::OK {
                return (status, Json(value)).into_response();
            }
            Json(json!({"session":{"id":"fixture-session","account":value["account"]["id"],"kind":"user","state":"active","expires_at":crate::cloud::session::now()+3600}})).into_response()
        }
        async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
            (origin, task)
        }
        let (auth_base, auth_task) = serve(
            Router::new()
                .route("/v1/account", get(auth))
                .route("/v1/session", get(session)),
        )
        .await;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let key = path.join("csrf");
        std::fs::write(&key, [42; 32]).unwrap();
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
        let config_path = path.join("cloud.json");
        std::fs::write(&config_path,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":"http://127.0.0.1:4300","account_service":auth_base,"csrf_secret":key})).unwrap()).unwrap();
        std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let mut config = crate::Config::development(path.join("tasks"));
        config.cloud = Some(Arc::new(
            crate::cloud::session::CloudSession::load(&config_path).unwrap(),
        ));
        let app = App(Arc::new(crate::Inner { config }));
        let (base, task) = serve(
            routes()
                .merge(crate::custom_agents::routes())
                .merge(crate::agent_assignments::routes())
                .with_state(app)
                .route("/v1/account", get(auth)),
        )
        .await;
        let client = openagents_sdk::Client::new(
            openagents_sdk::Deployment::new(&format!("{base}/v1"), &base).unwrap(),
        )
        .unwrap();
        let a = client
            .authenticate(&format!("sess_{}", "a".repeat(64)))
            .await
            .unwrap();
        let b = client
            .authenticate(&format!("sess_{}", "b".repeat(64)))
            .await
            .unwrap();

        let cancel = openagents_sdk::CancellationToken::new();
        let labs = client.agent_lab(&a).unwrap();
        let other = client.agent_lab(&b).unwrap();
        let agents = client.agents(&a).unwrap();
        let mut e = starter();
        e.host = "fixture-host".into();
        e.project = "fixture-project".into();
        e.suite.cases.push(Case {
            id: "review".into(),
            title: "Review".into(),
            task: "Reply for review".into(),
            check: Check::Review {
                criteria: "Read the reply".into(),
            },
        });
        let save = labs.bind(SaveDocument {
            request_id: "1".repeat(64),
            expected_revision: 0,
            content: Content::Experiment {
                experiment: e.clone(),
            },
            summary: "Created tests".into(),
        });
        let d = labs.save(None, &save, &cancel).await.unwrap();
        assert_eq!(d, labs.save(None, &save, &cancel).await.unwrap());
        assert!(other.save(None, &save, &cancel).await.is_err());
        assert!(other.document(&d.id, &cancel).await.is_err());
        assert!(
            other
                .documents(None, &cancel)
                .await
                .unwrap()
                .data
                .is_empty()
        );
        let start = labs.bind(StartRun {
            request_id: "2".repeat(64),
            version: 1,
        });
        let mut r = labs.start(&d.id, &start, &cancel).await.unwrap();
        assert_eq!(r, labs.start(&d.id, &start, &cancel).await.unwrap());
        assert!(
            agents
                .assignments(None, None, &cancel)
                .await
                .unwrap()
                .data
                .is_empty()
        );
        for i in 0..2 {
            let ready = labs.bind(HostUpdate {
                request_id: format!("{:064x}", 30 + i),
                host: e.host.clone(),
                action: HostAction::Ready {
                    trial: i,
                    evidence: Evidence {
                        project: e.project.clone(),
                        source_sha256: "a".repeat(64),
                        context_sha256: lab_kit::digest(&json!("")),
                        runtime_version: "b".repeat(64),
                        context: None,
                    },
                },
            });
            r = labs.host_update(&r.id, &ready, &cancel).await.unwrap();
            let assigned = agents
                .assignment(&r.trials[i].assignment, &cancel)
                .await
                .unwrap();
            assert_eq!(assigned.spec.task, e.task(i));
            let report = agents
                .report_request(
                    e.host.clone(),
                    assigned.revision,
                    AssignmentState::Completed,
                    Some("HELLO".into()),
                    Some(assigned.commands[0].request_id.clone()),
                    0,
                )
                .unwrap();
            agents.report(&assigned.id, &report, &cancel).await.unwrap();
            let advance = labs.bind(HostUpdate {
                request_id: format!("{:064x}", 40 + i),
                host: e.host.clone(),
                action: HostAction::Advance,
            });
            r = labs.host_update(&r.id, &advance, &cancel).await.unwrap();
        }
        assert!(r.complete());
        assert_eq!(r.trials[0].outcome, Outcome::Passed);
        assert_eq!(r.trials[1].outcome, Outcome::NeedsReview);
        assert_eq!(r.counts(0).planned, 2);
        assert_eq!(r.counts(0).review, 1);
        assert!(r.trials.iter().all(|t| t.cost_usd.is_none()));
        assert!(
            agents
                .list(None, 100, &cancel)
                .await
                .unwrap()
                .data
                .is_empty()
        );
        let review = labs.bind(RunChange {
            request_id: "3".repeat(64),
            expected_revision: r.revision,
            action: RunAction::Review {
                trial: 1,
                passed: false,
                note: "Did not complete the task".into(),
            },
        });
        r = labs.change(&r.id, &review, &cancel).await.unwrap();
        assert_eq!(r.counts(0).failed, 1);
        assert!(r.trials[1].reviewer.is_some());
        let promote = labs.bind(Promote {
            request_id: "4".repeat(64),
            version: 1,
        });
        let promoted = labs.promote(&d.id, &promote, &cancel).await.unwrap();
        assert_eq!(
            promoted,
            labs.promote(&d.id, &promote, &cancel).await.unwrap()
        );
        assert_eq!(agents.list(None, 100, &cancel).await.unwrap().data.len(), 1);
        let start2 = labs.bind(StartRun {
            request_id: "5".repeat(64),
            version: 1,
        });
        let stopped = labs.start(&d.id, &start2, &cancel).await.unwrap();
        let stop = labs.bind(RunChange {
            request_id: "6".repeat(64),
            expected_revision: stopped.revision,
            action: RunAction::Stop,
        });
        labs.change(&stopped.id, &stop, &cancel).await.unwrap();
        let advance = labs.bind(HostUpdate {
            request_id: "7".repeat(64),
            host: e.host.clone(),
            action: HostAction::Advance,
        });
        let stopped = labs
            .host_update(&stopped.id, &advance, &cancel)
            .await
            .unwrap();
        assert!(stopped.trials.iter().all(|t| t.outcome == Outcome::Stopped));
        let reopened = labs.document(&d.id, &cancel).await.unwrap();
        assert_eq!(reopened, d);
        // A legal version history can exceed the ordinary account JSON limit.
        let mut large = starter();
        large.draft.spec.instructions = "a".repeat(12000);
        large.suite.cases = (0..8)
            .map(|i| Case {
                id: format!("case-{i}"),
                title: format!("Test {i}"),
                task: "b".repeat(12000),
                check: Check::Contains {
                    expected: "HELLO".into(),
                },
            })
            .collect();
        let mut document = None;
        let mut last = None;
        for n in 0..10 {
            let request = labs.bind(SaveDocument {
                request_id: format!("{:064x}", 100 + n),
                expected_revision: n,
                content: Content::Experiment {
                    experiment: large.clone(),
                },
                summary: "Updated test version".into(),
            });
            let saved = labs
                .save(document.as_deref(), &request, &cancel)
                .await
                .unwrap();
            document = Some(saved.id.clone());
            last = Some(saved);
        }
        let saved = labs
            .document(document.as_deref().unwrap(), &cancel)
            .await
            .unwrap();
        assert_eq!(Some(saved.clone()), last);
        assert!(serde_json::to_vec(&saved).unwrap().len() > 1024 * 1024);
        task.abort();
        auth_task.abort();
    }
}

async fn delete_document(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<DeleteDocument>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    mutation(
        &app,
        &owner,
        &body.request_id,
        &json!({"delete":id,"body":body}),
        |b| {
            if b.runs
                .values()
                .any(|r| r.version.document == id && !r.complete())
            {
                return Err(invalid("Stop this experiment's tests before deleting it."));
            }
            let d = b.documents.get_mut(&id).ok_or_else(missing)?;
            if d.versions.last().map(|v| v.number) != Some(body.expected_revision) {
                return Err(conflict());
            }
            d.deleted = true;
            Ok(json!(d))
        },
    )
    .await
}
