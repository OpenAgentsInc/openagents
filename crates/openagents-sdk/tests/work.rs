//! Public Work/Pool clients against a fake connection server, without a host
//! login, cloud SDK, or account.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::post,
};
use openagents_sdk::{
    CancellationToken,
    work::{
        self as api, LandMode, Placement, Pool, PoolStatus, Work, WorkEvents, WorkRequest, WorkRun,
    },
};
use serde_json::{Value, json};

#[derive(Default)]
struct Server {
    runs: BTreeMap<String, WorkRun>,
    calls: Vec<(String, Value)>,
}
struct Connection {
    url: String,
    client: reqwest::Client,
}
impl api::Transport for Connection {
    fn request<'a>(
        &'a self,
        method: &'static str,
        params: Value,
        _: &'a CancellationToken,
    ) -> api::Reply<'a> {
        Box::pin(async move {
            let response = self
                .client
                .post(format!("{}/{method}", self.url))
                .json(&params)
                .send()
                .await
                .map_err(|_| api::Error::refused("Connection stopped."))?;
            response
                .json::<Result<Value, api::Error>>()
                .await
                .map_err(|_| api::Error::refused("Unreadable response."))?
        })
    }
}
struct Fixture {
    connection: Connection,
    server: Arc<Mutex<Server>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn fixture() -> Fixture {
    async fn handle(
        State(state): State<Arc<Mutex<Server>>>,
        Path(method): Path<String>,
        Json(params): Json<Value>,
    ) -> Json<Result<Value, api::Error>> {
        if method == api::EVENTS && params["after"] == 999 {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let mut state = state.lock().unwrap();
        state.calls.push((method.clone(), params.clone()));
        let result = match method.as_str() {
            api::START => {
                let id = params["request_id"].as_str().unwrap().to_string();
                let request: WorkRequest =
                    serde_json::from_value(params["request"].clone()).unwrap();
                if let Some(run) = state.runs.get(&id) {
                    if run.request == request {
                        Ok(json!(run))
                    } else {
                        Err(api::Error {
                            code: "request_changed".into(),
                            message: "Choose a new request ID.".into(),
                            owner_command: None,
                        })
                    }
                } else {
                    let run = WorkRun {
                        id: id.clone(),
                        request,
                        state: "running".into(),
                        created_unix: 10,
                        updated_unix: 10,
                        host: Some("pool-host".into()),
                        result: None,
                        error: None,
                        usage: None,
                    };
                    state.runs.insert(id, run.clone());
                    Ok(json!(run))
                }
            }
            api::LIST => Ok(json!(state.runs.values().collect::<Vec<_>>())),
            api::EVENTS => {
                let run = state.runs[params["id"].as_str().unwrap()].clone();
                let after = params["after"].as_u64().unwrap();
                Ok(json!(WorkEvents {
                    run,
                    events: vec![json!({"event":"progress","sequence":after})],
                    next: after + 1,
                    complete: false
                }))
            }
            api::CANCEL => {
                state
                    .runs
                    .get_mut(params["id"].as_str().unwrap())
                    .unwrap()
                    .state = "cancelled".into();
                Ok(Value::Null)
            }
            api::POOL_STATUS => Ok(json!(PoolStatus {
                pool: Some("pool-fixture".into()),
                granted: true,
                slots: 2,
                ..PoolStatus::default()
            })),
            api::POOL_UP => Err(api::Error {
                code: "refused".into(),
                message: "The pool needs an owner grant.".into(),
                owner_command: Some(format!(
                    "openagents cloud up --hosts 1 --agent {}",
                    params["agent"].as_str().unwrap()
                )),
            }),
            api::POOL_DOWN => Ok(Value::Null),
            _ => Err(api::Error::invalid("Unknown operation.")),
        };
        Json(result)
    }
    let server = Arc::new(Mutex::new(Server::default()));
    let router = Router::new()
        .route("/{method}", post(handle))
        .with_state(server.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Fixture {
        connection: Connection {
            url,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap(),
        },
        server,
        task,
    }
}
fn request() -> WorkRequest {
    WorkRequest {
        repo: "example/project".into(),
        issue: 12,
        land: LandMode::Queue,
        engine: Some("briefed:codex".into()),
        on: Some(Placement::Gce),
        agent: "agt_builder".into(),
    }
}

#[tokio::test]
async fn work_retries_keep_identity_and_progress_cursors_and_cancel_only_the_run() {
    let fixture = fixture().await;
    let work = Work::new(&fixture.connection);
    let cancel = CancellationToken::new();
    let first = work.start("dispatch-12", request(), &cancel).await.unwrap();
    assert_eq!(
        work.start("dispatch-12", request(), &cancel).await.unwrap(),
        first
    );
    let mut changed = request();
    changed.issue = 13;
    assert_eq!(
        work.start("dispatch-12", changed, &cancel)
            .await
            .unwrap_err()
            .code,
        "request_changed"
    );
    assert_eq!(work.list(&cancel).await.unwrap().len(), 1);
    let first_page = work.events(&first.id, 0, &cancel).await.unwrap();
    let next_page = work
        .events(&first.id, first_page.next, &cancel)
        .await
        .unwrap();
    assert_eq!(next_page.events[0]["sequence"], 1);
    work.cancel(&first.id, &cancel).await.unwrap();
    assert_eq!(work.list(&cancel).await.unwrap()[0].state, "cancelled");
    assert_eq!(fixture.server.lock().unwrap().runs.len(), 1);
}

#[tokio::test]
async fn pool_status_and_mutations_preserve_the_agent_and_owner_command() {
    let fixture = fixture().await;
    let cancel = CancellationToken::new();
    let pool = Pool::new(&fixture.connection, "agt_reviewer").unwrap();
    assert_eq!(pool.status(&cancel).await.unwrap().slots, 2);
    let error = pool.up(1, &cancel).await.unwrap_err();
    assert_eq!(
        error.owner_command.as_deref(),
        Some("openagents cloud up --hosts 1 --agent agt_reviewer")
    );
    pool.down(&cancel).await.unwrap();
    assert!(
        fixture
            .server
            .lock()
            .unwrap()
            .calls
            .iter()
            .all(|(_, params)| params["agent"] == "agt_reviewer")
    );
}

#[tokio::test]
async fn cancelling_a_read_leaves_the_host_run_running_and_validation_never_sends() {
    let fixture = fixture().await;
    let work = Work::new(&fixture.connection);
    let cancel = CancellationToken::new();
    let run = work.start("dispatch-12", request(), &cancel).await.unwrap();
    let reader_cancel = CancellationToken::new();
    let (result, ()) = tokio::join!(work.events(&run.id, 999, &reader_cancel), async {
        tokio::time::sleep(Duration::from_millis(30)).await;
        reader_cancel.cancel();
    });
    assert_eq!(result.unwrap_err().code, "cancelled");
    assert_eq!(work.list(&cancel).await.unwrap()[0].state, "running");
    let before = fixture.server.lock().unwrap().calls.len();
    let mut invalid = request();
    invalid.repo = "example/../bad".into();
    assert!(work.start("id", invalid, &cancel).await.is_err());
    assert!(Pool::new(&fixture.connection, "../other").is_err());
    assert_eq!(fixture.server.lock().unwrap().calls.len(), before);
}

#[tokio::test]
async fn an_unreadable_server_response_is_a_typed_error() {
    struct Broken;
    impl api::Transport for Broken {
        fn request<'a>(
            &'a self,
            _: &'static str,
            _: Value,
            _: &'a CancellationToken,
        ) -> api::Reply<'a> {
            Box::pin(async { Ok(json!({"wrong":true})) })
        }
    }
    assert_eq!(
        Work::new(&Broken)
            .list(&CancellationToken::new())
            .await
            .unwrap_err()
            .code,
        "invalid_response"
    );
}
