//! Durable assignment intents. Execution and transcripts stay with the app host.
use crate::{
    App,
    chat_store::{Error, Store},
    coder_sync::{answer, refused, stored},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
};
use openagents_sdk::agents::{AgentDefinition, assignments::*, valid_id};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
const MAX_BYTES: usize = 4 * 1024 * 1024;
/// The largest `result` a host may report: an agent's final answer, in UTF-8 bytes.
pub(crate) const RESULT_BYTES: usize = 16 * 1024;
#[derive(Default, Serialize, Deserialize)]
pub(super) struct Book {
    pub(super) assignments: BTreeMap<String, Assignment>,
    receipts: BTreeMap<String, Receipt>,
}
#[derive(Serialize, Deserialize)]
struct Receipt {
    fingerprint: String,
    result: Assignment,
}
#[derive(Clone, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub(super) enum Change {
    Create {
        body: CreateAssignment,
        definition: AgentDefinition,
    },
    Command {
        id: String,
        body: AssignmentCommand,
    },
    Report {
        id: String,
        body: AssignmentReport,
    },
}
impl Change {
    fn key(&self) -> &str {
        match self {
            Self::Create { body, .. } => &body.request_id,
            Self::Command { body, .. } => &body.request_id,
            Self::Report { body, .. } => &body.request_id,
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    Invalid,
    Missing,
    Conflict,
    Full,
}
impl Refusal {
    pub(super) fn response(self) -> Response {
        let (s, c, m) = match self {
            Self::Invalid => (
                StatusCode::BAD_REQUEST,
                "invalid_assignment",
                "Check the assignment and try again.",
            ),
            Self::Missing => (
                StatusCode::NOT_FOUND,
                "not_found",
                "This assignment is no longer available.",
            ),
            Self::Conflict => (
                StatusCode::CONFLICT,
                "assignment_conflict",
                "This assignment changed. Reload it and try again.",
            ),
            Self::Full => (
                StatusCode::TOO_MANY_REQUESTS,
                "quota_exhausted",
                "Your assignment storage is full.",
            ),
        };
        refused(s, c, m)
    }
}
fn chat_id(id: &str) -> String {
    format!(
        "{}-{}-{}-{}-{}",
        &id[..8],
        &id[8..12],
        &id[12..16],
        &id[16..20],
        &id[20..32]
    )
}
impl Book {
    fn apply(&mut self, c: &Change) -> Result<Assignment, Refusal> {
        if !valid_id(c.key()) {
            return Err(Refusal::Invalid);
        }
        let fingerprint =
            crate::chat_store::digest(&serde_json::to_vec(c).map_err(|_| Refusal::Invalid)?);
        if let Some(r) = self.receipts.get(c.key()) {
            return if r.fingerprint == fingerprint {
                Ok(r.result.clone())
            } else {
                Err(Refusal::Conflict)
            };
        }
        if self.receipts.len() >= 4096 {
            return Err(Refusal::Full);
        }
        let result = match c {
            Change::Create { body, definition } => {
                body.spec.validate().map_err(|_| Refusal::Invalid)?;
                if self.assignments.values().filter(|a| !a.archived).count() >= 256 {
                    return Err(Refusal::Full);
                }
                if self.assignments.contains_key(&body.request_id)
                    || definition.deleted
                    || definition.id != body.spec.agent
                    || definition.revision != body.spec.agent_revision
                {
                    return Err(Refusal::Conflict);
                }
                Assignment {
                    id: body.request_id.clone(),
                    revision: 1,
                    spec: body.spec.clone(),
                    definition: definition.clone(),
                    chat: chat_id(&body.request_id),
                    state: AssignmentState::Queued,
                    commands: vec![AssignmentCommand {
                        request_id: body.request_id.clone(),
                        action: AssignmentAction::FollowUp {
                            text: body.spec.task.clone(),
                        },
                    }],
                    result: None,
                    archived: false,
                    active_command: None,
                    after_seq: 0,
                }
            }
            Change::Command { id, body } => {
                let mut a = self.assignments.get(id).cloned().ok_or(Refusal::Missing)?;
                if a.archived {
                    return Err(Refusal::Missing);
                }
                if matches!(body.action, AssignmentAction::Archive) {
                    if matches!(a.state, AssignmentState::Queued | AssignmentState::Running) {
                        return Err(Refusal::Conflict);
                    }
                    a.archived = true;
                }
                if a.commands.len() >= 128 {
                    return Err(Refusal::Full);
                }
                if let AssignmentAction::FollowUp { text } = &body.action {
                    if text.trim().is_empty() || text.len() > 64 * 1024 || text.contains('\0') {
                        return Err(Refusal::Invalid);
                    }
                }
                if !matches!(body.action, AssignmentAction::Archive) {
                    a.commands.push(body.clone());
                }
                a.revision += 1;
                if matches!(body.action, AssignmentAction::FollowUp { .. }) {
                    a.state = AssignmentState::Queued;
                    a.result = None;
                }
                a
            }
            Change::Report { id, body } => {
                let mut a = self.assignments.get(id).cloned().ok_or(Refusal::Missing)?;
                if body.host != a.spec.host || body.expected_revision != a.revision {
                    return Err(Refusal::Conflict);
                }
                if matches!(body.state, AssignmentState::Queued)
                    || body
                        .result
                        .as_ref()
                        .is_some_and(|s| s.len() > RESULT_BYTES || s.contains('\0'))
                {
                    return Err(Refusal::Invalid);
                }
                let Some(active) = body.active_command.as_deref() else {
                    return Err(Refusal::Invalid);
                };
                let index = a
                    .commands
                    .iter()
                    .position(|c| {
                        c.request_id == active
                            && matches!(c.action, AssignmentAction::FollowUp { .. })
                    })
                    .ok_or(Refusal::Invalid)?;
                if let Some(previous) = a.active_command.as_deref() {
                    let old = a
                        .commands
                        .iter()
                        .position(|c| c.request_id == previous)
                        .ok_or(Refusal::Invalid)?;
                    if index < old || index == old && body.after_seq != a.after_seq {
                        return Err(Refusal::Conflict);
                    }
                }
                a.active_command = body.active_command.clone();
                a.after_seq = body.after_seq;
                a.revision += 1;
                a.state = body.state;
                a.result = body.result.clone();
                a
            }
        };
        self.assignments.insert(result.id.clone(), result.clone());
        self.receipts.insert(
            c.key().into(),
            Receipt {
                fingerprint,
                result: result.clone(),
            },
        );
        Ok(result)
    }
}
pub(super) async fn load(store: &Store, owner: &str) -> Result<(Book, Option<String>), Error> {
    let key = Store::owner_key(owner, "agent-assignments.json")?;
    match store.read_key(&key).await? {
        None => Ok((Book::default(), None)),
        Some((bytes, g)) => {
            if bytes.len() > MAX_BYTES {
                return Err(Error::Corrupt("Assignments could not be opened."));
            }
            Ok((
                serde_json::from_slice(&bytes)
                    .map_err(|_| Error::Corrupt("Assignments could not be opened."))?,
                Some(g),
            ))
        }
    }
}
pub(super) async fn mutate(
    store: &Store,
    owner: &str,
    c: Change,
) -> Result<Result<Assignment, Refusal>, Error> {
    let key = Store::owner_key(owner, "agent-assignments.json")?;
    for _ in 0..16 {
        let (mut b, g) = load(store, owner).await?;
        let result = match b.apply(&c) {
            Ok(a) => a,
            Err(e) => return Ok(Err(e)),
        };
        let bytes = serde_json::to_vec(&b)
            .map_err(|_| Error::Invalid("The assignment could not be saved."))?;
        if bytes.len() > MAX_BYTES {
            return Ok(Err(Refusal::Full));
        }
        match store.write_key(&key, bytes, g.as_deref()).await {
            Ok(_) => return Ok(Ok(result)),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}
pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/agent-assignments", get(list).post(create))
        .route("/v1/agent-assignments/{id}", get(detail))
        .route("/v1/agent-assignments/{id}/commands", post(command))
        .route("/v1/agent-assignments/{id}/report", post(report))
        .layer(DefaultBodyLimit::max(96 * 1024))
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    host: Option<String>,
    after: Option<String>,
}
async fn list(State(app): State<App>, headers: HeaderMap, Query(q): Query<Page>) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    if q.after.as_deref().is_some_and(|s| !valid_id(s)) {
        return Refusal::Invalid.response();
    }
    let (book, _) = match load(&app.config.chat_store, &owner).await {
        Ok(b) => b,
        Err(e) => return stored(&e),
    };
    let mut data: Vec<_> = book
        .assignments
        .values()
        .filter(|a| {
            !a.archived
                && q.host.as_ref().is_none_or(|h| h == &a.spec.host)
                && q.after.as_ref().is_none_or(|id| a.id > *id)
        })
        .take(9)
        .cloned()
        .collect();
    let next = (data.len() > 8).then(|| data[7].id.clone());
    data.truncate(8);
    answer(StatusCode::OK, json!(AssignmentPage { data, next }))
}
async fn detail(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    match load(&app.config.chat_store, &owner).await {
        Ok((b, _)) => match b.assignments.get(&id) {
            Some(a) => answer(StatusCode::OK, json!(a)),
            None => Refusal::Missing.response(),
        },
        Err(e) => stored(&e),
    }
}
async fn change(app: App, headers: HeaderMap, c: Change) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    match mutate(&app.config.chat_store, &owner, c).await {
        Ok(Ok(a)) => answer(StatusCode::OK, json!(a)),
        Ok(Err(e)) => e.response(),
        Err(e) => stored(&e),
    }
}
async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<CreateAssignment>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return Refusal::Invalid.response();
    };
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    // A keyed replay uses its pinned revision, even if the definition changed later.
    if let Ok((book, _)) = load(&app.config.chat_store, &owner).await {
        if let Some(a) = book.assignments.get(&body.request_id) {
            if a.spec != body.spec {
                return Refusal::Conflict.response();
            }
            return answer(StatusCode::OK, json!(a));
        }
    }
    let definition = match crate::custom_agents::definition(
        &app.config.chat_store,
        &owner,
        &body.spec.agent,
        Some(body.spec.agent_revision),
    )
    .await
    {
        Ok(Ok(d)) => d,
        Ok(Err(e)) => return e.response(),
        Err(e) => return stored(&e),
    };
    let latest = match crate::custom_agents::definition(
        &app.config.chat_store,
        &owner,
        &body.spec.agent,
        None,
    )
    .await
    {
        Ok(Ok(d)) => d,
        Ok(Err(e)) => return e.response(),
        Err(e) => return stored(&e),
    };
    if latest.deleted {
        return Refusal::Missing.response();
    }
    change(app, headers, Change::Create { body, definition }).await
}
async fn command(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<AssignmentCommand>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return Refusal::Invalid.response();
    };
    change(app, headers, Change::Command { id, body }).await
}
async fn report(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<AssignmentReport>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return Refusal::Invalid.response();
    };
    change(app, headers, Change::Report { id, body }).await
}
#[cfg(test)]
mod tests {
    use super::*;
    fn definition() -> AgentDefinition {
        AgentDefinition {
            id: format!("{:064x}", 1),
            revision: 1,
            deleted: false,
            spec: openagents_sdk::agents::AgentSpec {
                name: "Review helper".into(),
                description: String::new(),
                instructions: "Review Rust".into(),
                runtime: "claude-code".into(),
                model: None,
                access: openagents_sdk::agents::Access::ReadOnly,
                fallbacks: vec![],
            },
        }
    }
    fn create() -> Change {
        let definition = definition();
        Change::Create {
            body: CreateAssignment {
                request_id: format!("{:064x}", 2),
                spec: AssignmentSpec {
                    agent: definition.id.clone(),
                    agent_revision: 1,
                    host: "test-host".into(),
                    project: None,
                    task: "Review the fixture".into(),
                },
            },
            definition,
        }
    }
    #[tokio::test]
    async fn assignment_identity_commands_reports_and_recovery() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::local(root.path().into());
        let owner = crate::chat_store::account_owner("assignment-test");
        let a = mutate(&store, &owner, create()).await.unwrap().unwrap();
        assert_eq!(a, mutate(&store, &owner, create()).await.unwrap().unwrap());
        let cmd = Change::Command {
            id: a.id.clone(),
            body: AssignmentCommand {
                request_id: format!("{:064x}", 3),
                action: AssignmentAction::Stop,
            },
        };
        let stopped = mutate(&store, &owner, cmd.clone()).await.unwrap().unwrap();
        assert_eq!(stopped.commands.len(), 2);
        assert_eq!(stopped, mutate(&store, &owner, cmd).await.unwrap().unwrap());
        let report = Change::Report {
            id: a.id.clone(),
            body: AssignmentReport {
                active_command: Some(a.commands[0].request_id.clone()),
                after_seq: 0,
                request_id: format!("{:064x}", 4),
                host: "other-host".into(),
                expected_revision: 2,
                state: AssignmentState::Stopped,
                result: None,
            },
        };
        assert_eq!(
            mutate(&store, &owner, report).await.unwrap(),
            Err(Refusal::Conflict)
        );
        let reopened = Store::local(root.path().into());
        assert_eq!(
            load(&reopened, &owner).await.unwrap().0.assignments[&a.id],
            stopped
        );
        assert!(
            load(
                &reopened,
                &crate::chat_store::account_owner("other-account")
            )
            .await
            .unwrap()
            .0
            .assignments
            .is_empty()
        );
    }
}
