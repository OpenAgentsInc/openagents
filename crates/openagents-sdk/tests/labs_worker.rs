use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use base64::Engine;
use openagents_sdk::{
    CancellationToken, Client, Deployment,
    labs::{
        self, JobSpec, JobStatus, Reference, Report, Requirements, SpaceRef, Version,
        worker::{
            self, Assignment, Capabilities, EffectiveRequirements, Enrolled, Lease, Placement,
            Usage, Worker,
            protocol::{Attempt, Available, Enroll, Finish, Output, Poll, Work},
        },
    },
};
use serde_json::json;
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::Notify;
fn key() -> String {
    format!("labw_{}.{}", "a".repeat(32), "b".repeat(64))
}
fn account_key(name: &str) -> String {
    format!(
        "sess_{}",
        if name == "alice" { "a" } else { "b" }.repeat(64)
    )
}
fn id() -> String {
    "a".repeat(32)
}
fn caps() -> Capabilities {
    Capabilities {
        worker: id(),
        placement: Placement::Enrolled,
        cpu_millis: 1000,
        memory_bytes: 64 * 1024 * 1024,
        gpu: false,
        tools: BTreeSet::from(["echo".into()]),
        jobs: BTreeSet::from(["echo/render".into()]),
        local_assets: BTreeSet::new(),
        max_seconds: 10,
    }
}
fn enrolled() -> Enrolled {
    Enrolled {
        worker: Worker {
            id: id(),
            account: "acct_alice".into(),
            space: SpaceRef::Personal {
                account: "acct_alice".into(),
            },
            capabilities: caps(),
            expires_unix_ms: 2_000_000_000_000,
            revoked: false,
            issuer_session: None,
        },
        token: key(),
    }
}
fn work(started: bool) -> Work {
    let content = json!({"text":"synthetic job input"});
    let version = Version {
        document: "design".into(),
        number: 1,
        parent: None,
        author: "acct_alice".into(),
        summary: "Created".into(),
        created_unix_ms: 1,
        schema: "echo.v1".into(),
        content_sha256: labs::digest(&content),
        content,
        report: Report::default(),
        assets: vec![],
    };
    Work {
        assignment: Assignment {
            lease: Lease {
                job: labs::JobState {
                    id: "job".into(),
                    spec: JobSpec {
                        kind: "echo".into(),
                        job_type: "render".into(),
                        version: Reference {
                            document: "design".into(),
                            version: 1,
                        },
                        params: json!({}),
                        requirements: Requirements::default(),
                    },
                    status: if started {
                        JobStatus::Running
                    } else {
                        JobStatus::Queued
                    },
                    message: None,
                    estimate_seconds: None,
                    artifacts: vec![],
                },
                worker: id(),
                attempt: 1,
                expires_unix_ms: 2_000_000_000_000,
                started,
            },
            started_unix_ms: started.then_some(1_000),
        },
        version,
        requirements: EffectiveRequirements {
            cpu_millis: 1000,
            memory_bytes: 64 * 1024 * 1024,
            gpu: false,
            tools: BTreeSet::from(["echo".into()]),
            max_seconds: 10,
            private_local: false,
            local_assets: BTreeSet::new(),
            no_training: false,
            job: "echo/render".into(),
        },
    }
}
#[derive(Clone, Default)]
struct Controls {
    seen: Arc<Mutex<Vec<(String, String)>>>,
    finishes: Arc<Mutex<Vec<Vec<u8>>>>,
    started: Arc<AtomicBool>,
    lose_finish: Arc<AtomicBool>,
    mismatch_finish: Arc<AtomicBool>,
    corrupt: Arc<AtomicBool>,
    redirect: Arc<AtomicBool>,
    block: Arc<AtomicBool>,
    tamper: Arc<AtomicBool>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
}
fn auth(headers: &HeaderMap) -> &'static str {
    let value = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if value == format!("Bearer {}", key()) {
        "worker"
    } else if value == format!("Bearer {}", account_key("alice")) {
        "alice"
    } else if value == format!("Bearer {}", account_key("bob")) {
        "bob"
    } else {
        "unknown"
    }
}
async fn capture(State(c): State<Controls>, request: Request, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    let kind = auth(request.headers()).to_owned();
    c.seen.lock().unwrap().push((path.clone(), kind.clone()));
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, 12 * 1024 * 1024).await.unwrap();
    if path.ends_with("/finish") {
        c.finishes.lock().unwrap().push(bytes.to_vec());
    }
    next.run(Request::from_parts(parts, Body::from(bytes)))
        .await
}
async fn poll(
    State(c): State<Controls>,
    headers: HeaderMap,
    axum::Json(request): axum::Json<Poll>,
) -> Response {
    assert_eq!(auth(&headers), "worker");
    if c.redirect.load(Ordering::SeqCst) {
        return Response::builder()
            .status(302)
            .header("Location", "/v1/account")
            .body(Body::empty())
            .unwrap();
    }
    c.entered.notify_one();
    if c.block.load(Ordering::SeqCst) {
        c.release.notified().await;
    }
    if request.wait_seconds > 0 {
        tokio::time::sleep(Duration::from_secs(request.wait_seconds)).await;
    }
    let mut value = work(c.started.load(Ordering::SeqCst));
    if c.corrupt.load(Ordering::SeqCst) {
        value.version.content_sha256 = "0".repeat(64);
    }
    axum::Json(Available { work: Some(value) }).into_response()
}
async fn start(
    State(c): State<Controls>,
    headers: HeaderMap,
    axum::Json(request): axum::Json<Attempt>,
) -> axum::Json<Work> {
    assert_eq!(auth(&headers), "worker");
    assert_eq!(request.job, "job");
    assert_eq!(request.attempt, 1);
    c.started.store(true, Ordering::SeqCst);
    axum::Json(work(true))
}
async fn heartbeat(
    State(c): State<Controls>,
    headers: HeaderMap,
    axum::Json(request): axum::Json<Attempt>,
) -> axum::Json<Work> {
    assert_eq!(auth(&headers), "worker");
    assert_eq!(request.job, "job");
    axum::Json(work(c.started.load(Ordering::SeqCst)))
}
async fn finish(
    State(c): State<Controls>,
    headers: HeaderMap,
    axum::Json(request): axum::Json<Finish>,
) -> Response {
    assert_eq!(auth(&headers), "worker");
    assert_eq!(request.job, "job");
    if c.lose_finish.swap(false, Ordering::SeqCst) {
        return Response::builder()
            .status(200)
            .header("Content-Type", "application/json")
            .body(Body::from_stream(futures_util::stream::once(async {
                Err::<Vec<u8>, _>(std::io::Error::other(
                    "synthetic lost finish acknowledgement",
                ))
            })))
            .unwrap();
    }
    let mut job = work(true).assignment.lease.job;
    job.status = if c.mismatch_finish.load(Ordering::SeqCst) {
        JobStatus::Running
    } else {
        request.status
    };
    axum::Json(job).into_response()
}
const MESH: &[u8] = b"synthetic mesh";
fn mesh() -> labs::Artifact {
    use sha2::Digest;
    labs::Artifact {
        id: "mesh".into(),
        version: Reference {
            document: "robot".into(),
            version: 2,
        },
        role: labs::ArtifactRole::Source,
        media_type: "model/gltf-binary".into(),
        sha256: format!("{:x}", sha2::Sha256::digest(MESH)),
        size: MESH.len() as u64,
        class: labs::AssetClass::Public,
        no_training: false,
    }
}
async fn inputs(
    State(c): State<Controls>,
    headers: HeaderMap,
    axum::Json(request): axum::Json<Attempt>,
) -> axum::Json<worker::protocol::Inputs> {
    assert_eq!(auth(&headers), "worker");
    assert_eq!((request.job.as_str(), request.attempt), ("job", 1));
    let mut robot = work(true).version;
    robot.document = "robot".into();
    robot.number = 2;
    let mut versions = vec![work(true).version, robot];
    if c.tamper.load(Ordering::SeqCst) {
        versions.swap(0, 1);
    }
    axum::Json(worker::protocol::Inputs {
        versions,
        files: vec![mesh()],
    })
}
async fn input_file(
    State(c): State<Controls>,
    headers: HeaderMap,
    axum::Json(request): axum::Json<worker::protocol::InputFile>,
) -> Vec<u8> {
    assert_eq!(auth(&headers), "worker");
    assert_eq!(request.artifact, "mesh");
    if c.tamper.load(Ordering::SeqCst) {
        b"synthetic mesx".to_vec()
    } else {
        MESH.to_vec()
    }
}
struct Server {
    client: Client,
    controls: Controls,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    async fn new() -> Self {
        let controls = Controls::default();
        let router=Router::new()
            .route("/v1/account",get(|headers:HeaderMap|async move {
                let name=auth(&headers);if !matches!(name,"alice"|"bob"){return StatusCode::UNAUTHORIZED.into_response();}
                axum::Json(json!({"account":{"id":format!("acct_{name}"),"label":"Synthetic"},"workspaces":[]})).into_response()
            }))
            .route("/v1/labs/workers",post(|headers:HeaderMap|async move {assert_eq!(auth(&headers),"alice");axum::Json(enrolled())}))
            .route("/v1/labs/workers/{id}",delete(|headers:HeaderMap|async move {assert!(matches!(auth(&headers),"alice"|"worker"));axum::Json(json!({"revoked":true}))}))
            .route("/v1/labs/workers/poll",post(poll))
            .route("/v1/labs/workers/start",post(start))
            .route("/v1/labs/workers/heartbeat",post(heartbeat))
            .route("/v1/labs/workers/finish",post(finish))
            .route("/v1/labs/workers/inputs",post(inputs))
            .route("/v1/labs/workers/inputs/file",post(input_file))
            .route("/v1/labs/jobs/job/usage",get(|headers:HeaderMap|async move {assert_eq!(auth(&headers),"alice");axum::Json(vec![Usage{job:"job".into(),attempt:1,worker:id(),account:"acct_alice".into(),space:SpaceRef::Personal{account:"acct_alice".into()},placement:Placement::Enrolled,started_unix_ms:Some(1000),observed_until_unix_ms:1500,observed_wall_ms:Some(500),outcome:JobStatus::Done,charge:None}])}))
            .layer(middleware::from_fn_with_state(controls.clone(),capture)).with_state(controls.clone());
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
        }
    }
}
fn finish_request() -> Finish {
    Finish {
        job: "job".into(),
        attempt: 1,
        status: JobStatus::Done,
        outputs: vec![Output {
            role: labs::ArtifactRole::Export,
            media_type: "text/plain".into(),
            content_base64: base64::engine::general_purpose::STANDARD.encode(b"synthetic output"),
        }],
    }
}

#[tokio::test]
async fn account_enrollment_and_usage_stay_separate_from_retained_job_only_requests() {
    let server = Server::new().await;
    let cancel = CancellationToken::new();
    let session = server
        .client
        .authenticate(&account_key("alice"))
        .await
        .unwrap();
    let labs = server.client.labs(&session).unwrap();
    let enrolled = labs
        .enroll_worker(
            &labs
                .bind(Enroll {
                    space: "me".into(),
                    capabilities: caps(),
                    lifetime_ms: 3600000,
                })
                .unwrap(),
            &cancel,
        )
        .await
        .unwrap();
    assert!(!format!("{enrolled:?}").contains(&key()));
    let scoped = server.client.labs_worker(&enrolled.token).unwrap();
    assert_eq!(scoped.worker_id(), enrolled.worker.id);
    let record = scoped.record();
    assert!(!format!("{record:?}").contains(&key()));
    assert!(!format!("{scoped:?}").contains(&key()));
    let restored: worker::CredentialRecord =
        serde_json::from_slice(&serde_json::to_vec(&record).unwrap()).unwrap();
    let scoped = server.client.restore_labs_worker(&restored).unwrap();
    assert!(matches!(
        Client::new(Deployment::production())
            .unwrap()
            .restore_labs_worker(&restored),
        Err(labs::Error::Session(openagents_sdk::Error::WrongDeployment))
    ));
    let assignment = scoped
        .poll(&Poll::default(), &cancel)
        .await
        .unwrap()
        .work
        .unwrap();
    assert!(!assignment.assignment.lease.started);
    let attempt = Attempt {
        job: assignment.assignment.lease.job.id,
        attempt: assignment.assignment.lease.attempt,
    };
    assert!(
        scoped
            .start(&attempt, &cancel)
            .await
            .unwrap()
            .assignment
            .lease
            .started
    );
    assert!(
        scoped
            .heartbeat(&attempt, &cancel)
            .await
            .unwrap()
            .assignment
            .lease
            .started
    );
    let finish: Finish =
        serde_json::from_slice(&serde_json::to_vec(&finish_request()).unwrap()).unwrap();
    server.controls.lose_finish.store(true, Ordering::SeqCst);
    assert!(matches!(
        scoped.finish(&finish, &cancel).await,
        Err(labs::Error::Session(openagents_sdk::Error::Network { .. }))
    ));
    assert_eq!(
        server.controls.finishes.lock().unwrap().len(),
        1,
        "finish must not retry automatically"
    );
    assert_eq!(
        scoped.finish(&finish, &cancel).await.unwrap().status,
        JobStatus::Done
    );
    let finishes = server.controls.finishes.lock().unwrap().clone();
    assert_eq!(finishes[0], finishes[1]);
    let usage = labs
        .job_usage(&labs.job_ref("me", "job").unwrap(), &cancel)
        .await
        .unwrap();
    assert_eq!(usage[0].observed_wall_ms, Some(500));
    assert_eq!(usage[0].charge, None);
    assert!(scoped.retire(&cancel).await.unwrap());
    let calls = server.controls.seen.lock().unwrap().len();
    assert!(matches!(
        scoped.poll(&Poll::default(), &cancel).await,
        Err(labs::Error::Cancelled)
    ));
    assert_eq!(server.controls.seen.lock().unwrap().len(), calls);
    assert!(
        labs.revoke_worker(&labs.worker_ref("me", &id()).unwrap(), &cancel)
            .await
            .unwrap()
    );
    assert!(
        server
            .controls
            .seen
            .lock()
            .unwrap()
            .iter()
            .all(|(path, auth)| {
                if path == "/v1/account" || path == "/v1/labs/workers" || path.ends_with("/usage") {
                    auth == "alice"
                } else {
                    auth == "worker" || path.ends_with(&id())
                }
            })
    );
}
#[tokio::test]
async fn invalid_keys_bounds_account_changes_and_redirects_do_not_send_credentials_elsewhere() {
    let server = Server::new().await;
    let cancel = CancellationToken::new();
    for key in [
        account_key("alice"),
        "oak_example".into(),
        "labw_short".into(),
        key().to_uppercase(),
        format!("{}\n", key()),
    ] {
        assert!(server.client.labs_worker(&key).is_err());
    }
    assert!(server.controls.seen.lock().unwrap().is_empty());
    let alice = server
        .client
        .authenticate(&account_key("alice"))
        .await
        .unwrap();
    let bob = server
        .client
        .authenticate(&account_key("bob"))
        .await
        .unwrap();
    let labs = server.client.labs(&alice).unwrap();
    let worker = labs.worker_ref("me", &id()).unwrap();
    let bound = labs
        .bind(Enroll {
            space: "me".into(),
            capabilities: caps(),
            lifetime_ms: 1000,
        })
        .unwrap();
    labs.set_session(&bob).unwrap();
    let before = server.controls.seen.lock().unwrap().len();
    assert!(matches!(
        labs.enroll_worker(&bound, &cancel).await,
        Err(labs::Error::Session(openagents_sdk::Error::WrongAccount))
    ));
    assert!(matches!(
        labs.revoke_worker(&worker, &cancel).await,
        Err(labs::Error::Session(openagents_sdk::Error::WrongAccount))
    ));
    let scoped = server.client.labs_worker(&key()).unwrap();
    assert!(matches!(
        scoped.poll(&Poll { wait_seconds: 26 }, &cancel).await,
        Err(labs::Error::InvalidInput)
    ));
    assert!(matches!(
        scoped
            .start(
                &Attempt {
                    job: "job".into(),
                    attempt: 0
                },
                &cancel
            )
            .await,
        Err(labs::Error::InvalidInput)
    ));
    let mut finish = finish_request();
    finish.outputs = vec![finish.outputs[0].clone(); 17];
    assert!(matches!(
        scoped.finish(&finish, &cancel).await,
        Err(labs::Error::InvalidInput)
    ));
    finish.outputs.truncate(1);
    finish.outputs[0].content_base64 =
        base64::engine::general_purpose::STANDARD.encode(vec![0; 8 * 1024 * 1024 + 1]);
    assert!(matches!(
        scoped.finish(&finish, &cancel).await,
        Err(labs::Error::TooLarge)
    ));
    assert_eq!(server.controls.seen.lock().unwrap().len(), before);
    assert!(server.client.authenticate(&key()).await.is_err());
    assert_eq!(server.controls.seen.lock().unwrap().len(), before);
    server.controls.redirect.store(true, Ordering::SeqCst);
    assert!(matches!(
        scoped.poll(&Poll::default(), &cancel).await,
        Err(labs::Error::Session(openagents_sdk::Error::Redirect))
    ));
    assert_eq!(server.controls.seen.lock().unwrap().len(), before + 1);
}
#[tokio::test]
async fn canceled_worker_clones_drop_late_work_and_invalid_versions_never_grant_start() {
    let server = Server::new().await;
    let scoped = server.client.labs_worker(&key()).unwrap();
    let cancel = CancellationToken::new();
    server.controls.corrupt.store(true, Ordering::SeqCst);
    assert!(matches!(
        scoped.poll(&Poll::default(), &cancel).await,
        Err(labs::Error::InvalidResponse)
    ));
    assert!(
        !server
            .controls
            .seen
            .lock()
            .unwrap()
            .iter()
            .any(|(path, _)| path.ends_with("/start"))
    );
    server.controls.corrupt.store(false, Ordering::SeqCst);
    server.controls.block.store(true, Ordering::SeqCst);
    // Consume the notification from the earlier completed poll.
    server.controls.entered.notified().await;
    let clone = scoped.clone();
    let operation = tokio::spawn(async move {
        clone
            .poll(&Poll::default(), &CancellationToken::new())
            .await
    });
    server.controls.entered.notified().await;
    scoped.clear_credential();
    assert!(matches!(
        operation.await.unwrap(),
        Err(labs::Error::Cancelled)
    ));
    server.controls.release.notify_one();
}
#[tokio::test]
async fn a_twenty_five_second_poll_has_a_thirty_second_request_deadline() {
    let server = Server::new().await;
    let scoped = server.client.labs_worker(&key()).unwrap();
    let operation = tokio::spawn(async move {
        scoped
            .poll(&Poll { wait_seconds: 25 }, &CancellationToken::new())
            .await
    });
    server.controls.entered.notified().await;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(24)).await;
    tokio::task::yield_now().await;
    assert!(!operation.is_finished());
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::time::resume();
    assert!(operation.await.unwrap().unwrap().work.is_some());
}

#[tokio::test]
async fn uncertain_and_failed_finishes_preserve_status_without_uploading_outputs() {
    let server = Server::new().await;
    let scoped = server.client.labs_worker(&key()).unwrap();
    let cancel = CancellationToken::new();
    for status in [JobStatus::Uncertain, JobStatus::Failed] {
        let mut request = finish_request();
        request.status = status;
        let before = server.controls.finishes.lock().unwrap().len();
        assert!(matches!(
            scoped.finish(&request, &cancel).await,
            Err(labs::Error::InvalidInput)
        ));
        assert_eq!(server.controls.finishes.lock().unwrap().len(), before);
        request.outputs.clear();
        server.controls.lose_finish.store(true, Ordering::SeqCst);
        assert!(matches!(
            scoped.finish(&request, &cancel).await,
            Err(labs::Error::Session(openagents_sdk::Error::Network { .. }))
        ));
        assert_eq!(server.controls.finishes.lock().unwrap().len(), before + 1);
        assert_eq!(
            scoped.finish(&request, &cancel).await.unwrap().status,
            status
        );
        let finishes = server.controls.finishes.lock().unwrap().clone();
        assert_eq!(finishes[before], finishes[before + 1]);
        let sent: Finish = serde_json::from_slice(&finishes[before]).unwrap();
        assert_eq!(sent.status, status);
        assert!(sent.outputs.is_empty());
        server
            .controls
            .mismatch_finish
            .store(true, Ordering::SeqCst);
        assert!(matches!(
            scoped.finish(&request, &cancel).await,
            Err(labs::Error::InvalidResponse)
        ));
        server
            .controls
            .mismatch_finish
            .store(false, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn inputs_are_bound_to_the_job_and_files_to_their_checksums() {
    let server = Server::new().await;
    let scoped = server.client.labs_worker(&key()).unwrap();
    let cancel = CancellationToken::new();
    let work = work(true);
    let inputs = scoped.inputs(&work, &cancel).await.unwrap();
    assert_eq!(inputs.versions.len(), 2);
    assert_eq!(inputs.versions[0].reference(), work.version.reference());
    assert_eq!(
        scoped
            .input_file(&work, &inputs.files[0], &cancel)
            .await
            .unwrap(),
        MESH
    );
    let mut unlisted = mesh();
    unlisted.size += 1;
    assert!(matches!(
        scoped.input_file(&work, &unlisted, &cancel).await,
        Err(labs::Error::InvalidResponse)
    ));
    server.controls.tamper.store(true, Ordering::SeqCst);
    assert!(matches!(
        scoped.inputs(&work, &cancel).await,
        Err(labs::Error::InvalidResponse)
    ));
    assert!(matches!(
        scoped.input_file(&work, &mesh(), &cancel).await,
        Err(labs::Error::InvalidResponse)
    ));
    assert!(
        server
            .controls
            .seen
            .lock()
            .unwrap()
            .iter()
            .all(|(_, auth)| auth == "worker")
    );
}
