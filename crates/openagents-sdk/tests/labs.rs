use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{HeaderMap, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use base64::Engine;
use lab_api::{Access, Action, Api, ApiError, Authorizer, Gate, KindAdapter, Registry, Tool};
use lab_store::backend::FsBackend;
use openagents_sdk::{
    CancellationToken, Client, Deployment, Session,
    labs::{
        self, AddNote, AssetClass, Change, CreateDocument, CreateJob, EditDocument, EditResult,
        JobRequest, JobStatus, Locator, PageOptions, Reference, ToolCall, Upload, WaitOptions,
        Want,
    },
};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::Notify;

#[path = "labs/streams.rs"]
mod streams;

fn credential(account: &str) -> String {
    format!(
        "sess_{}",
        if account == "alice" { "a" } else { "b" }.repeat(64)
    )
}
fn account(headers: &HeaderMap) -> Result<&'static str, ApiError> {
    let bearer = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    if bearer == format!("Bearer {}", credential("alice")) {
        Ok("acct_alice")
    } else if bearer == format!("Bearer {}", credential("bob")) {
        Ok("acct_bob")
    } else {
        Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Sign in to continue.",
        ))
    }
}
struct Auth;
#[async_trait::async_trait]
impl Authorizer for Auth {
    async fn authorize(
        &self,
        headers: &HeaderMap,
        space: &str,
        _: Action,
    ) -> Result<Access, ApiError> {
        let account = account(headers)?;
        let space = if space == "me" {
            labs::SpaceRef::Personal {
                account: account.into(),
            }
        } else if space == "project:shared_project" {
            labs::SpaceRef::Project {
                project: "shared_project".into(),
            }
        } else {
            return Err(ApiError::missing());
        };
        Ok(Access {
            account: account.into(),
            space,
            experimental: true,
            cad: true,
        })
    }
}
fn echo_tool(name: &str, args: &Value) -> Result<Tool, ApiError> {
    if name != "echo_set" {
        return Err(ApiError::missing());
    }
    Ok(Tool::Change {
        document: args["document"]
            .as_str()
            .ok_or_else(ApiError::missing)?
            .into(),
        change: Change::Intent {
            intent: json!({"intent":"set","text":args["text"]}),
        },
        summary: "Tool edit".into(),
    })
}
#[derive(Clone, Debug)]
struct Seen {
    method: Method,
    path: String,
    query: String,
    key: Option<String>,
    body: Vec<u8>,
}
#[derive(Clone, Default)]
struct Controls {
    seen: Arc<Mutex<Vec<Seen>>>,
    lose_job: Arc<AtomicBool>,
    block_read: Arc<AtomicBool>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
}
async fn intercept(State(controls): State<Controls>, request: Request, next: Next) -> Response {
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, 1024 * 1024).await.unwrap();
    let seen = Seen {
        method: parts.method.clone(),
        path: parts.uri.path().into(),
        query: parts.uri.query().unwrap_or_default().into(),
        key: parts
            .headers
            .get("Idempotency-Key")
            .map(|h| h.to_str().unwrap().into()),
        body: bytes.to_vec(),
    };
    controls.seen.lock().unwrap().push(seen.clone());
    if seen.method == Method::GET
        && seen.path == "/v1/labs/documents/design"
        && controls.block_read.swap(false, Ordering::SeqCst)
    {
        controls.entered.notify_one();
        controls.release.notified().await;
    }
    let response = next
        .run(Request::from_parts(parts, Body::from(bytes)))
        .await;
    if seen.method == Method::POST
        && seen.path == "/v1/labs/jobs"
        && controls.lose_job.swap(false, Ordering::SeqCst)
    {
        assert_eq!(response.status(), StatusCode::CREATED);
        return Response::builder()
            .status(201)
            .header("content-type", "application/json")
            .body(Body::from_stream(futures_util::stream::once(async {
                Err::<Vec<u8>, _>(std::io::Error::other("synthetic lost response"))
            })))
            .unwrap();
    }
    response
}
struct Server {
    client: Client,
    controls: Controls,
    task: tokio::task::JoinHandle<()>,
    _root: tempfile::TempDir,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    async fn start() -> Self {
        let root = tempfile::tempdir().unwrap();
        let backend = Arc::new(FsBackend::open(root.path()).unwrap());
        let mut registry = Registry::default();
        registry
            .register(
                Arc::new(KindAdapter::<labs::echo::Echo>::new(echo_tool)),
                Gate::Experimental,
            )
            .unwrap();
        let api = Arc::new(Api {
            backend,
            authorizer: Arc::new(Auth),
            registry,
        });
        let controls = Controls::default();
        let router = api
            .router()
            .route(
                "/v1/account",
                get(|headers: HeaderMap| async move {
                    match account(&headers) {
                        Ok(account) => axum::Json(
                            json!({"account":{"id":account,"label":"Synthetic"},"workspaces":[]}),
                        )
                        .into_response(),
                        Err(error) => error.into_response(),
                    }
                }),
            )
            .layer(middleware::from_fn_with_state(controls.clone(), intercept));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            client: Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap())
                .unwrap(),
            controls,
            task,
            _root: root,
        }
    }
    async fn session(&self, account: &str) -> Session {
        self.client
            .authenticate(&credential(account))
            .await
            .unwrap()
    }
    fn attempts(&self, method: Method, path: &str) -> Vec<Seen> {
        self.controls
            .seen
            .lock()
            .unwrap()
            .iter()
            .filter(|seen| seen.method == method && seen.path == path)
            .cloned()
            .collect()
    }
}
fn create(id: &str, space: &str) -> CreateDocument {
    CreateDocument {
        space: space.into(),
        id: id.into(),
        kind: "echo".into(),
        schema: "echo.v1".into(),
        title: "Synthetic design".into(),
        content: json!({"text":"first"}),
        summary: "Created".into(),
    }
}
fn edit(base: u64, text: &str) -> EditDocument {
    EditDocument {
        space: "me".into(),
        base,
        change: Change::Intent {
            intent: json!({"intent":"set","text":text}),
        },
        summary: "Changed".into(),
    }
}
async fn collect(mut download: labs::ArtifactDownload) -> Vec<u8> {
    let mut bytes = Vec::new();
    while let Some(chunk) = download
        .next_chunk(&CancellationToken::new())
        .await
        .unwrap()
    {
        bytes.extend(chunk);
    }
    bytes
}

#[tokio::test]
async fn shared_handlers_cover_documents_versions_refusals_tools_files_notes_and_cleanup() {
    let server = Server::start().await;
    let session = server.session("alice").await;
    let client = server.client.labs(&session).unwrap();
    let cancel = CancellationToken::new();
    assert_eq!(client.kinds("me", &cancel).await.unwrap()[0].kind, "echo");
    assert_eq!(
        client.tools("me", "echo", &cancel).await.unwrap()[0].name,
        "echo_set"
    );
    let request = client.bind(create("design", "me")).unwrap();
    let first = client.create_document(&request, &cancel).await.unwrap();
    assert_eq!(first.number, 1);
    assert_eq!(
        client
            .create_document(&request, &cancel)
            .await
            .unwrap_err()
            .status(),
        Some(409)
    );
    let reference = client.document_ref("me", "design").unwrap();
    assert_eq!(
        client.document(&reference, &cancel).await.unwrap().latest,
        1
    );
    assert_eq!(
        client
            .documents("me", &PageOptions::default(), &cancel)
            .await
            .unwrap()
            .data
            .len(),
        1
    );
    assert!(
        matches!(client.edit(&reference,&client.bind(edit(1,"second")).unwrap(),&cancel).await.unwrap(),EditResult::Created(version) if version.number==2)
    );
    assert!(matches!(
        client
            .edit(&reference, &client.bind(edit(1, "stale")).unwrap(), &cancel)
            .await
            .unwrap(),
        EditResult::Stale
    ));
    assert!(matches!(
        client
            .edit(&reference, &client.bind(edit(2, "")).unwrap(), &cancel)
            .await
            .unwrap(),
        EditResult::Refused(_)
    ));
    let tool = client
        .bind(ToolCall {
            space: "me".into(),
            chat: None,
            base: Some(2),
            arguments: json!({"document":"design","text":"from tool"}),
        })
        .unwrap();
    assert_eq!(
        client
            .invoke("echo", "echo_set", &tool, &cancel)
            .await
            .unwrap()
            .result["version"]["number"],
        3
    );
    let first_page = client
        .versions(
            &reference,
            &PageOptions {
                limit: 1,
                ..PageOptions::default()
            },
            &cancel,
        )
        .await
        .unwrap();
    let next = first_page.next.unwrap();
    let second_page = client
        .versions(
            &reference,
            &PageOptions {
                limit: 1,
                after: next,
                kind: None,
            },
            &cancel,
        )
        .await
        .unwrap();
    assert_ne!(first_page.data[0].number, second_page.data[0].number);
    let version = client.version(&reference, 3, &cancel).await.unwrap();
    assert_eq!(version.content["text"], "from tool");
    assert_eq!(
        client.card(&reference, 3, &cancel).await.unwrap().document,
        "design"
    );
    client
        .add_note(
            &reference,
            &client
                .bind(AddNote {
                    space: "me".into(),
                    version: 3,
                    locator: Locator::None,
                    text: "Synthetic note".into(),
                })
                .unwrap(),
            &cancel,
        )
        .await
        .unwrap();
    assert_eq!(
        client
            .notes(&reference, &PageOptions::default(), &cancel)
            .await
            .unwrap()
            .data
            .len(),
        1
    );
    let bytes = b"synthetic file bytes";
    let uploaded = client
        .upload(
            &client
                .bind(Upload {
                    space: "me".into(),
                    version: version.reference(),
                    media_type: "application/octet-stream".into(),
                    class: AssetClass::Public,
                    no_training: true,
                    content_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                })
                .unwrap(),
            &cancel,
        )
        .await
        .unwrap();
    let artifact = client.artifact("me", &uploaded.id, &cancel).await.unwrap();
    assert!(artifact.signed_url().is_none());
    assert!(artifact.metadata().no_training);
    assert_eq!(
        collect(client.download(&artifact, None, &cancel).await.unwrap()).await,
        bytes
    );
    assert_eq!(
        collect(
            client
                .download(&artifact, Some(2..8), &cancel)
                .await
                .unwrap()
        )
        .await,
        &bytes[2..8]
    );
    let events = client
        .events_since(&reference, 0, 100, &cancel)
        .await
        .unwrap();
    assert!(!events.data.is_empty());
    let mut stream = client.events(&reference, 0).unwrap();
    assert_eq!(stream.next(&cancel).await.unwrap(), events.data[0]);
    assert!(client.delete_document(&reference, &cancel).await.unwrap());
    assert!(
        client
            .documents("me", &PageOptions::default(), &cancel)
            .await
            .unwrap()
            .data
            .is_empty()
    );
}

#[tokio::test]
async fn job_requests_survive_response_loss_and_restore_without_running_twice() {
    let server = Server::start().await;
    let session = server.session("alice").await;
    let client = server.client.labs(&session).unwrap();
    let cancel = CancellationToken::new();
    let version = client
        .create_document(&client.bind(create("design", "me")).unwrap(), &cancel)
        .await
        .unwrap();
    let document = client.document_ref("me", "design").unwrap();
    let prepared = client
        .job_request(CreateJob {
            space: "me".into(),
            version: version.reference(),
            want: Want::Card,
        })
        .unwrap();
    let retained = serde_json::to_vec(&prepared).unwrap();
    let prepared: JobRequest = serde_json::from_slice(&retained).unwrap();
    server.controls.lose_job.store(true, Ordering::SeqCst);
    let job = client.create_job(&prepared, &cancel).await.unwrap();
    let attempts = server.attempts(Method::POST, "/v1/labs/jobs");
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].key, attempts[1].key);
    assert_eq!(attempts[0].body, attempts[1].body);
    assert_eq!(attempts[0].key.as_deref(), Some(prepared.key().as_str()));
    assert_eq!(
        client.create_job(&prepared, &cancel).await.unwrap().id,
        job.id
    );
    assert_eq!(
        client
            .jobs(&document, &PageOptions::default(), &cancel)
            .await
            .unwrap()
            .data
            .len(),
        1
    );
    let reference = client.job_ref("me", &job.id).unwrap();
    assert_eq!(
        client.job(&reference, &cancel).await.unwrap().status,
        JobStatus::Queued
    );
    let stopped = CancellationToken::new();
    stopped.cancel();
    assert!(matches!(
        client
            .wait(&reference, WaitOptions::default(), &stopped)
            .await,
        Err(labs::Error::Cancelled)
    ));
    assert_eq!(
        client.job(&reference, &cancel).await.unwrap().status,
        JobStatus::Queued
    );
    assert!(matches!(
        client
            .wait(
                &reference,
                WaitOptions {
                    timeout: Duration::from_millis(100),
                    poll_interval: Duration::from_millis(50)
                },
                &cancel
            )
            .await,
        Err(labs::Error::WaitTimedOut)
    ));
    assert_eq!(
        client.cancel_job(&reference, &cancel).await.unwrap().status,
        JobStatus::Cancelled
    );
    assert_eq!(
        client
            .wait(&reference, WaitOptions::default(), &cancel)
            .await
            .unwrap()
            .status,
        JobStatus::Cancelled
    );
    assert!(client.delete_document(&document, &cancel).await.unwrap());
}

#[tokio::test]
async fn changing_accounts_discards_late_results_and_refuses_saved_mutations_and_references() {
    let server = Server::start().await;
    let alice = server.session("alice").await;
    let bob = server.session("bob").await;
    let client = server.client.labs(&alice).unwrap();
    let cancel = CancellationToken::new();
    let request = client.bind(create("design", "me")).unwrap();
    client.create_document(&request, &cancel).await.unwrap();
    let reference = client.document_ref("me", "design").unwrap();
    server.controls.block_read.store(true, Ordering::SeqCst);
    let reader = client.clone();
    let old_reference = reference.clone();
    let pending = tokio::spawn(async move {
        reader
            .document(&old_reference, &CancellationToken::new())
            .await
    });
    server.controls.entered.notified().await;
    client.set_session(&bob).unwrap();
    server.controls.release.notify_one();
    assert!(matches!(
        pending.await.unwrap(),
        Err(labs::Error::AccountChanged)
    ));
    let before = server.attempts(Method::POST, "/v1/labs/documents").len();
    assert!(matches!(
        client.create_document(&request, &cancel).await,
        Err(labs::Error::Session(openagents_sdk::Error::WrongAccount))
    ));
    assert_eq!(
        server.attempts(Method::POST, "/v1/labs/documents").len(),
        before
    );
    assert!(matches!(
        client.document(&reference, &cancel).await,
        Err(labs::Error::Session(openagents_sdk::Error::WrongAccount))
    ));
    let bob_ref = client.document_ref("me", "design").unwrap();
    assert_eq!(
        client
            .document(&bob_ref, &cancel)
            .await
            .unwrap_err()
            .status(),
        Some(404)
    );
    assert!(
        client
            .documents("me", &PageOptions::default(), &cancel)
            .await
            .unwrap()
            .data
            .is_empty()
    );
    client.set_session(&alice).unwrap();
    assert!(client.delete_document(&reference, &cancel).await.unwrap());
    client.clear_session();
    assert!(matches!(
        client
            .documents("me", &PageOptions::default(), &cancel)
            .await,
        Err(labs::Error::Session(openagents_sdk::Error::Unauthorized))
    ));
}

#[tokio::test]
async fn project_artifact_fallback_accepts_decoded_query_values_and_invalid_inputs_send_nothing() {
    let server = Server::start().await;
    let session = server.session("alice").await;
    let client = server.client.labs(&session).unwrap();
    let cancel = CancellationToken::new();
    let version = client
        .create_document(
            &client
                .bind(create("project-design", "project:shared_project"))
                .unwrap(),
            &cancel,
        )
        .await
        .unwrap();
    let document = client
        .document_ref("project:shared_project", "project-design")
        .unwrap();
    let uploaded = client
        .upload(
            &client
                .bind(Upload {
                    space: "project:shared_project".into(),
                    version: version.reference(),
                    media_type: "text/plain".into(),
                    class: AssetClass::Public,
                    no_training: false,
                    content_base64: "ZmlsZQ==".into(),
                })
                .unwrap(),
            &cancel,
        )
        .await
        .unwrap();
    let artifact = client
        .artifact("project:shared_project", &uploaded.id, &cancel)
        .await
        .unwrap();
    assert_eq!(
        collect(
            client
                .download(&artifact, Some(0..4), &cancel)
                .await
                .unwrap()
        )
        .await,
        b"file"
    );
    assert!(client.document_ref("me", "..").is_err());
    assert!(client.document_ref("account:other", "design").is_err());
    assert!(matches!(
        client
            .documents(
                "me",
                &PageOptions {
                    limit: 101,
                    ..PageOptions::default()
                },
                &cancel
            )
            .await,
        Err(labs::Error::InvalidInput)
    ));
    let before = server.attempts(Method::POST, "/v1/labs/uploads").len();
    assert!(matches!(
        client
            .upload(
                &client
                    .bind(Upload {
                        space: "me".into(),
                        version: Reference {
                            document: "design".into(),
                            version: 1
                        },
                        media_type: "text/plain".into(),
                        class: AssetClass::PrivateLocal,
                        no_training: false,
                        content_base64: "ZmlsZQ==".into()
                    })
                    .unwrap(),
                &cancel
            )
            .await,
        Err(labs::Error::InvalidInput)
    ));
    assert_eq!(
        server.attempts(Method::POST, "/v1/labs/uploads").len(),
        before
    );
    client.delete_document(&document, &cancel).await.unwrap();
}
