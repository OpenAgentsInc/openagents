//! Account-owned definitions; CAS and mutation receipts survive server restart.
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
    routing::get,
};
use openagents_sdk::agents::{
    AgentDefinition, AgentPage, AgentSpec, CreateAgent, DeleteAgent, UpdateAgent, valid_id,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

const MAX_BYTES: usize = 4 * 1024 * 1024;
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    definitions: BTreeMap<String, Vec<AgentDefinition>>,
    receipts: BTreeMap<String, Receipt>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Receipt {
    fingerprint: String,
    result: AgentDefinition,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    Invalid(&'static str),
    Missing,
    Version,
    Retry,
    Full,
}
impl Refusal {
    pub(super) fn response(&self) -> Response {
        let (status, code, message) = match self {
            Self::Invalid(message) => (StatusCode::BAD_REQUEST, "invalid_agent", *message),
            Self::Missing => (
                StatusCode::NOT_FOUND,
                "not_found",
                "This agent is no longer available.",
            ),
            Self::Version => (
                StatusCode::CONFLICT,
                "version_conflict",
                "This agent changed. Reload it before saving.",
            ),
            Self::Retry => (
                StatusCode::CONFLICT,
                "idempotency_conflict",
                "This request was already used for a different change.",
            ),
            Self::Full => (
                StatusCode::TOO_MANY_REQUESTS,
                "quota_exhausted",
                "Your agent storage is full.",
            ),
        };
        refused(status, code, message)
    }
}
#[derive(Clone, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Mutation {
    Create {
        request_id: String,
        spec: AgentSpec,
    },
    Update {
        id: String,
        request_id: String,
        expected_revision: u64,
        spec: AgentSpec,
    },
    Delete {
        id: String,
        request_id: String,
        expected_revision: u64,
    },
}
impl Mutation {
    fn request_id(&self) -> &str {
        match self {
            Self::Create { request_id, .. }
            | Self::Update { request_id, .. }
            | Self::Delete { request_id, .. } => request_id,
        }
    }
    fn spec(&self) -> Option<&AgentSpec> {
        match self {
            Self::Create { spec, .. } | Self::Update { spec, .. } => Some(spec),
            Self::Delete { .. } => None,
        }
    }
}
impl Book {
    fn get(&self, id: &str, revision: Option<u64>) -> Result<AgentDefinition, Refusal> {
        let history = self.definitions.get(id).ok_or(Refusal::Missing)?;
        history
            .iter()
            .find(|d| Some(d.revision) == revision)
            .or_else(|| revision.is_none().then(|| history.last()).flatten())
            .cloned()
            .ok_or(Refusal::Missing)
    }
    fn apply(&mut self, m: &Mutation) -> Result<AgentDefinition, Refusal> {
        if !valid_id(m.request_id()) {
            return Err(Refusal::Invalid("The request ID is invalid."));
        }
        if let Some(spec) = m.spec() {
            spec.validate().map_err(Refusal::Invalid)?;
        }
        let bytes =
            serde_json::to_vec(m).map_err(|_| Refusal::Invalid("The agent could not be saved."))?;
        let fingerprint = crate::chat_store::digest(&bytes);
        if let Some(previous) = self.receipts.get(m.request_id()) {
            return if previous.fingerprint == fingerprint {
                Ok(previous.result.clone())
            } else {
                Err(Refusal::Retry)
            };
        }
        if self.receipts.len() >= 4096 {
            return Err(Refusal::Full);
        }
        let result = match m {
            Mutation::Create { request_id, spec } => {
                if self
                    .definitions
                    .values()
                    .filter(|history| history.last().is_some_and(|d| !d.deleted))
                    .count()
                    >= 128
                {
                    return Err(Refusal::Full);
                }
                if self.definitions.contains_key(request_id) {
                    return Err(Refusal::Retry);
                }
                AgentDefinition {
                    id: request_id.clone(),
                    revision: 1,
                    spec: spec.clone(),
                    deleted: false,
                }
            }
            Mutation::Update {
                id,
                expected_revision,
                spec,
                ..
            } => {
                let old = self.get(id, None)?;
                if old.deleted {
                    return Err(Refusal::Missing);
                }
                if old.revision != *expected_revision {
                    return Err(Refusal::Version);
                }
                AgentDefinition {
                    revision: old.revision + 1,
                    spec: spec.clone(),
                    ..old
                }
            }
            Mutation::Delete {
                id,
                expected_revision,
                ..
            } => {
                let old = self.get(id, None)?;
                if old.deleted {
                    return Err(Refusal::Missing);
                }
                if old.revision != *expected_revision {
                    return Err(Refusal::Version);
                }
                AgentDefinition {
                    revision: old.revision + 1,
                    deleted: true,
                    ..old
                }
            }
        };
        self.definitions
            .entry(result.id.clone())
            .or_default()
            .push(result.clone());
        self.receipts.insert(
            m.request_id().into(),
            Receipt {
                fingerprint,
                result: result.clone(),
            },
        );
        Ok(result)
    }
}
async fn load(store: &Store, owner: &str) -> Result<(Book, Option<String>), Error> {
    let key = Store::owner_key(owner, "custom-agents.json")?;
    match store.read_key(&key).await? {
        None => Ok((Book::default(), None)),
        Some((bytes, generation)) => {
            if bytes.len() > MAX_BYTES {
                return Err(Error::Corrupt("Agent storage could not be opened."));
            }
            let book = serde_json::from_slice(&bytes)
                .map_err(|_| Error::Corrupt("Agent storage could not be opened."))?;
            Ok((book, Some(generation)))
        }
    }
}
pub(super) async fn definition(
    store: &Store,
    owner: &str,
    id: &str,
    revision: Option<u64>,
) -> Result<Result<AgentDefinition, Refusal>, Error> {
    Ok(load(store, owner).await?.0.get(id, revision))
}
async fn mutate(
    store: &Store,
    owner: &str,
    m: Mutation,
) -> Result<Result<AgentDefinition, Refusal>, Error> {
    let key = Store::owner_key(owner, "custom-agents.json")?;
    for _ in 0..16 {
        let (mut book, generation) = load(store, owner).await?;
        let result = match book.apply(&m) {
            Ok(d) => d,
            Err(r) => return Ok(Err(r)),
        };
        let bytes = serde_json::to_vec(&book)
            .map_err(|_| Error::Invalid("The agent could not be saved."))?;
        if bytes.len() > MAX_BYTES {
            return Ok(Err(Refusal::Full));
        }
        match store.write_key(&key, bytes, generation.as_deref()).await {
            Ok(_) => return Ok(Ok(result)),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}
pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/agent-definitions", get(list).post(create))
        .route(
            "/v1/agent-definitions/{id}",
            get(detail).put(update).delete(delete),
        )
        .layer(DefaultBodyLimit::max(24 * 1024))
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    after: Option<String>,
    limit: Option<usize>,
    revision: Option<u64>,
}
async fn list(State(app): State<App>, headers: HeaderMap, Query(q): Query<Page>) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let limit = q.limit.unwrap_or(100);
    if !(1..=100).contains(&limit)
        || q.after.as_deref().is_some_and(|id| !valid_id(id))
        || q.revision.is_some()
    {
        return Refusal::Invalid("Start again from the first page.").response();
    }
    let (book, _) = match load(&app.config.chat_store, &owner).await {
        Ok(b) => b,
        Err(e) => return stored(&e),
    };
    let mut data: Vec<_> = book
        .definitions
        .iter()
        .filter(|(id, _)| q.after.as_ref().is_none_or(|after| *id > after))
        .filter_map(|(_, h)| h.last())
        .filter(|d| !d.deleted)
        .take(limit + 1)
        .cloned()
        .collect();
    let next = (data.len() > limit).then(|| data[limit - 1].id.clone());
    data.truncate(limit);
    answer(StatusCode::OK, json!(AgentPage { data, next }))
}
async fn detail(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<Page>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    if !valid_id(&id) {
        return Refusal::Missing.response();
    }
    match load(&app.config.chat_store, &owner).await {
        Ok((book, _)) => match book.get(&id, q.revision) {
            Ok(d) => answer(StatusCode::OK, json!(d)),
            Err(r) => r.response(),
        },
        Err(e) => stored(&e),
    }
}
async fn change(app: App, headers: HeaderMap, m: Mutation) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    match mutate(&app.config.chat_store, &owner, m).await {
        Ok(Ok(d)) => answer(StatusCode::OK, json!(d)),
        Ok(Err(r)) => r.response(),
        Err(e) => stored(&e),
    }
}
async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<CreateAgent>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(r)) = body else {
        return Refusal::Invalid("Check the agent fields and try again.").response();
    };
    change(
        app,
        headers,
        Mutation::Create {
            request_id: r.request_id,
            spec: r.spec,
        },
    )
    .await
}
async fn update(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<UpdateAgent>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(r)) = body else {
        return Refusal::Invalid("Check the agent fields and try again.").response();
    };
    change(
        app,
        headers,
        Mutation::Update {
            id,
            request_id: r.request_id,
            expected_revision: r.expected_revision,
            spec: r.spec,
        },
    )
    .await
}
async fn delete(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<DeleteAgent>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(r)) = body else {
        return Refusal::Invalid("Check the agent fields and try again.").response();
    };
    change(
        app,
        headers,
        Mutation::Delete {
            id,
            request_id: r.request_id,
            expected_revision: r.expected_revision,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec() -> AgentSpec {
        AgentSpec {
            name: "Review helper".into(),
            description: "Review a Rust project".into(),
            instructions: "Explain findings with file references.".into(),
            runtime: "claude-code".into(),
            model: None,
            access: openagents_sdk::agents::Access::ReadOnly,
        }
    }
    fn create(n: u8) -> Mutation {
        Mutation::Create {
            request_id: format!("{n:064x}"),
            spec: spec(),
        }
    }
    #[tokio::test]
    async fn retries_revisions_deletion_restart_and_account_isolation() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::local(root.path().into());
        let first = mutate(
            &store,
            &crate::chat_store::account_owner("acct-test-one"),
            create(1),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            first,
            mutate(
                &store,
                &crate::chat_store::account_owner("acct-test-one"),
                create(1)
            )
            .await
            .unwrap()
            .unwrap()
        );
        let mut changed = create(1);
        if let Mutation::Create { spec, .. } = &mut changed {
            spec.name = "Another".into();
        }
        assert_eq!(
            mutate(
                &store,
                &crate::chat_store::account_owner("acct-test-one"),
                changed
            )
            .await
            .unwrap(),
            Err(Refusal::Retry)
        );
        assert!(
            load(&store, &crate::chat_store::account_owner("acct-test-two"))
                .await
                .unwrap()
                .0
                .definitions
                .is_empty()
        );
        let update = Mutation::Update {
            id: first.id.clone(),
            request_id: format!("{:064x}", 2),
            expected_revision: 1,
            spec: AgentSpec {
                name: "New name".into(),
                ..spec()
            },
        };
        assert_eq!(
            mutate(
                &store,
                &crate::chat_store::account_owner("acct-test-one"),
                update.clone()
            )
            .await
            .unwrap()
            .unwrap()
            .revision,
            2
        );
        assert_eq!(
            mutate(
                &store,
                &crate::chat_store::account_owner("acct-test-one"),
                update
            )
            .await
            .unwrap()
            .unwrap()
            .revision,
            2
        );
        let stale = Mutation::Delete {
            id: first.id.clone(),
            request_id: format!("{:064x}", 3),
            expected_revision: 1,
        };
        assert_eq!(
            mutate(
                &store,
                &crate::chat_store::account_owner("acct-test-one"),
                stale
            )
            .await
            .unwrap(),
            Err(Refusal::Version)
        );
        let delete = Mutation::Delete {
            id: first.id.clone(),
            request_id: format!("{:064x}", 4),
            expected_revision: 2,
        };
        assert!(
            mutate(
                &store,
                &crate::chat_store::account_owner("acct-test-one"),
                delete
            )
            .await
            .unwrap()
            .unwrap()
            .deleted
        );
        let reopened = Store::local(root.path().into());
        let (book, _) = load(
            &reopened,
            &crate::chat_store::account_owner("acct-test-one"),
        )
        .await
        .unwrap();
        assert_eq!(book.get(&first.id, Some(1)).unwrap(), first);
        assert!(book.get(&first.id, None).unwrap().deleted);
        assert!(
            mutate(
                &reopened,
                &crate::chat_store::account_owner("acct-test-one"),
                create(5)
            )
            .await
            .unwrap()
            .is_ok()
        );
    }
    #[tokio::test]
    async fn concurrent_creates_preserve_both_and_edits_have_one_winner() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::local(root.path().into());
        let owner = crate::chat_store::account_owner("acct-test-one");
        let (a, b) = tokio::join!(
            mutate(&store, &owner, create(1)),
            mutate(&store, &owner, create(2))
        );
        let first = a.unwrap().unwrap();
        b.unwrap().unwrap();
        assert_eq!(load(&store, &owner).await.unwrap().0.definitions.len(), 2);
        let edit = |n| Mutation::Update {
            id: first.id.clone(),
            request_id: format!("{n:064x}"),
            expected_revision: 1,
            spec: spec(),
        };
        let (a, b) = tokio::join!(
            mutate(&store, &owner, edit(3)),
            mutate(&store, &owner, edit(4))
        );
        let results = [a.unwrap(), b.unwrap()];
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert!(results.contains(&Err(Refusal::Version)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn sdk_uses_real_handlers_and_cannot_cross_accounts() {
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
        let agents = client.agents(&a).unwrap();
        let cancel = openagents_sdk::CancellationToken::new();
        let request = agents.create_request(spec()).unwrap();
        let saved = agents.create(&request, &cancel).await.unwrap();
        assert_eq!(saved, agents.create(&request, &cancel).await.unwrap());
        let restored = serde_json::from_value(serde_json::to_value(&request).unwrap()).unwrap();
        assert_eq!(saved, agents.create(&restored, &cancel).await.unwrap());
        assert!(
            client
                .agents(&b)
                .unwrap()
                .list(None, 100, &cancel)
                .await
                .unwrap()
                .data
                .is_empty()
        );
        assert!(matches!(
            client
                .agents(&b)
                .unwrap()
                .get(&saved.id, None, &cancel)
                .await,
            Err(openagents_sdk::Error::Refused { status: 404, .. })
        ));
        assert!(matches!(
            client.agents(&b).unwrap().create(&request, &cancel).await,
            Err(openagents_sdk::Error::WrongAccount)
        ));
        use openagents_sdk::agents::assignments::{
            AssignmentAction, AssignmentSpec, AssignmentState,
        };
        let assignment_request = agents
            .assignment_request(AssignmentSpec {
                agent: saved.id.clone(),
                agent_revision: 1,
                host: "fixture-host".into(),
                project: None,
                task: "Review fixture".into(),
            })
            .unwrap();
        let assignment = agents.assign(&assignment_request, &cancel).await.unwrap();
        assert_eq!(
            assignment,
            agents.assign(&assignment_request, &cancel).await.unwrap()
        );
        assert!(
            client
                .agents(&b)
                .unwrap()
                .assignments(None, None, &cancel)
                .await
                .unwrap()
                .data
                .is_empty()
        );
        assert!(
            client
                .agents(&b)
                .unwrap()
                .assignment(&assignment.id, &cancel)
                .await
                .is_err()
        );
        assert!(
            client
                .agents(&b)
                .unwrap()
                .assign(&assignment_request, &cancel)
                .await
                .is_err()
        );
        let report = agents
            .report_request(
                "fixture-host".into(),
                assignment.revision,
                AssignmentState::Running,
                None,
                Some(assignment_request.body.request_id.clone()),
                10,
            )
            .unwrap();
        let running = agents
            .report(&assignment.id, &report, &cancel)
            .await
            .unwrap();
        assert_eq!(
            running,
            agents
                .report(&assignment.id, &report, &cancel)
                .await
                .unwrap()
        );
        let report = agents
            .report_request(
                "fixture-host".into(),
                running.revision,
                AssignmentState::Completed,
                Some("Reviewed".into()),
                running.active_command.clone(),
                10,
            )
            .unwrap();
        let completed = agents
            .report(&assignment.id, &report, &cancel)
            .await
            .unwrap();
        assert_eq!(completed.result.as_deref(), Some("Reviewed"));
        let followup = agents
            .command_request(AssignmentAction::FollowUp {
                text: "Check again".into(),
            })
            .unwrap();
        let queued = agents
            .command(&assignment.id, &followup, &cancel)
            .await
            .unwrap();
        assert_eq!(queued.state, AssignmentState::Queued);
        assert!(
            agents
                .report(&assignment.id, &report, &cancel)
                .await
                .is_ok()
        ); // exact retry does not overwrite newer work
        assert_eq!(
            agents
                .assignment(&assignment.id, &cancel)
                .await
                .unwrap()
                .state,
            AssignmentState::Queued
        );
        let stale = agents
            .report_request(
                "fixture-host".into(),
                completed.revision,
                AssignmentState::Failed,
                None,
                completed.active_command.clone(),
                10,
            )
            .unwrap();
        assert!(
            agents
                .report(&assignment.id, &stale, &cancel)
                .await
                .is_err()
        );
        let reserve = agents
            .report_request(
                "fixture-host".into(),
                queued.revision,
                AssignmentState::Running,
                None,
                Some(followup.body.request_id),
                30,
            )
            .unwrap();
        let running = agents
            .report(&assignment.id, &reserve, &cancel)
            .await
            .unwrap();
        let stop = agents.command_request(AssignmentAction::Stop).unwrap();
        let stopping = agents
            .command(&assignment.id, &stop, &cancel)
            .await
            .unwrap();
        let report = agents
            .report_request(
                "fixture-host".into(),
                stopping.revision,
                AssignmentState::Stopped,
                None,
                running.active_command.clone(),
                30,
            )
            .unwrap();
        let stopped = agents
            .report(&assignment.id, &report, &cancel)
            .await
            .unwrap();
        let archive = agents.command_request(AssignmentAction::Archive).unwrap();
        assert!(
            agents
                .command(&assignment.id, &archive, &cancel)
                .await
                .unwrap()
                .archived
        );
        assert!(
            agents
                .assignments(None, None, &cancel)
                .await
                .unwrap()
                .data
                .is_empty()
        );
        let schema = crate::app_openapi::document();
        let validator=jsonschema::validator_for(&json!({"$ref":"#/components/schemas/AgentAssignmentValue","components":schema["components"]})).unwrap();
        assert!(validator.is_valid(&json!(stopped)));
        let edit = agents
            .update_request(
                1,
                AgentSpec {
                    name: "Edited".into(),
                    ..spec()
                },
            )
            .unwrap();
        let edited = agents.update(&saved.id, &edit, &cancel).await.unwrap();
        assert_eq!(edited.revision, 2);
        let stale = agents.update_request(1, spec()).unwrap();
        assert!(matches!(
            agents.update(&saved.id, &stale, &cancel).await,
            Err(openagents_sdk::Error::Refused {
                status: 409,
                code: Some("version_conflict")
            })
        ));
        assert_eq!(
            agents.get(&saved.id, Some(1), &cancel).await.unwrap(),
            saved
        );
        assert!(
            agents
                .delete(&saved.id, &agents.delete_request(2).unwrap(), &cancel)
                .await
                .unwrap()
                .deleted
        );
        assert!(
            agents
                .list(None, 100, &cancel)
                .await
                .unwrap()
                .data
                .is_empty()
        );
        cancel.cancel();
        assert!(matches!(
            agents.list(None, 100, &cancel).await,
            Err(openagents_sdk::Error::Cancelled)
        ));
        let schema = crate::app_openapi::document();
        let validator=jsonschema::validator_for(&json!({"$ref":"#/components/schemas/CustomAgentDefinition","components":schema["components"]})).unwrap();
        assert!(validator.is_valid(&json!(edited)));
        task.abort();
        auth_task.abort();
    }
}
