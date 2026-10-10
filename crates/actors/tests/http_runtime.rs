use actors::{
    example,
    http::{self, Authenticator},
    *,
};
use axum::{
    body::{Body, to_bytes},
    http::{HeaderMap, Request, StatusCode},
};
use futures_util::future::BoxFuture;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tower::ServiceExt;

struct Auth {
    revoked: AtomicBool,
}
impl Authenticator for Auth {
    fn authenticate<'a>(
        &'a self,
        headers: &'a HeaderMap,
        workspace: &'a str,
    ) -> BoxFuture<'a, Result<Caller>> {
        Box::pin(async move {
            if self.revoked.load(Ordering::Relaxed)
                || headers.get("authorization").and_then(|v| v.to_str().ok())
                    != Some("Bearer test-only")
            {
                return Err(ActorError::new("forbidden", "Access is unavailable."));
            }
            Ok(caller(workspace))
        })
    }
    fn revalidate<'a>(&'a self, caller: &'a Caller) -> BoxFuture<'a, Result<Caller>> {
        Box::pin(async move {
            if self.revoked.load(Ordering::Relaxed) {
                Err(ActorError::new("forbidden", "Access was removed."))
            } else {
                Ok(caller.clone())
            }
        })
    }
}
fn caller(ws: &str) -> Caller {
    Caller {
        principal: "test-user".into(),
        workspace_id: ws.into(),
        account_id: Some("test-user".into()),
        role: Role::Owner,
        executor: None,
    }
}
fn request(path: &str, body: Value, auth: bool) -> Request<Body> {
    let mut request = Request::builder()
        .uri(path)
        .method("POST")
        .header("content-type", "application/json");
    if auth {
        request = request.header("authorization", "Bearer test-only");
    }
    request.body(Body::from(body.to_string())).unwrap()
}
#[tokio::test]
async fn http_requires_host_auth_rejects_caller_injection_and_revalidates_queued_work() {
    let Ok(dsn) = std::env::var("ACTORS_TEST_DATABASE_URL") else {
        eprintln!("Skipping isolated PostgreSQL test: ACTORS_TEST_DATABASE_URL is unset.");
        return;
    };
    let mut nonce = [0; 8];
    getrandom::fill(&mut nonce).unwrap();
    let ws = format!("http-test-{}", u64::from_le_bytes(nonce));
    let auth = Arc::new(Auth {
        revoked: AtomicBool::new(false),
    });
    let store = http::with_authenticator(
        PgStore::new(
            Pool::new(&dsn, 4).unwrap(),
            Arc::new(example::registry().unwrap()),
        ),
        auth.clone(),
    );
    store.migrate().await.unwrap();
    let app = http::router(store.clone(), auth.clone());
    let path = format!("/v1/w/{ws}/actors/example.counter/test/actions/add@1");
    let response = app
        .clone()
        .oneshot(request(
            &path,
            json!({"args":{"delta":1},"input":{"initial":2}}),
            false,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = app
        .clone()
        .oneshot(request(
            &path,
            json!({"args":{"delta":1},"input":{"initial":2},"caller":{"role":"admin"}}),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let response = app
        .clone()
        .oneshot(request(
            &path,
            json!({"args":{"delta":1},"input":{"initial":2}}),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let reply: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(reply["reply"], 3);
    let internal = format!("/v1/w/{ws}/actors/example.counter/test/actions/WorkDone");
    let response = app
        .clone()
        .oneshot(request(&internal, json!({"args":{}}), true))
        .await
        .unwrap();
    assert_ne!(response.status(), StatusCode::OK);
    let actor = ActorId {
        workspace_id: ws.clone(),
        actor_type: "example.counter".into(),
        key: "test".into(),
    };
    let pending = store
        .enqueue(
            &caller(&ws),
            &actor,
            Envelope {
                name: "add@1".into(),
                args: json!({"delta":100}),
                origin: Origin::Inbox,
            },
            Some("pending"),
        )
        .await
        .unwrap();
    auth.revoked.store(true, Ordering::Relaxed);
    store.dispatch_once(64).await.unwrap();
    assert_eq!(
        store
            .inbox(&caller(&ws), &actor, pending.seq)
            .await
            .unwrap()
            .state,
        "error"
    );
    assert_eq!(
        store.view(&caller(&ws), &actor).await.unwrap().view["value"],
        3
    );
}

#[tokio::test]
async fn background_runtime_delivers_alarm_and_typed_client_shares_store() {
    let Ok(dsn) = std::env::var("ACTORS_TEST_DATABASE_URL") else {
        eprintln!("Skipping isolated PostgreSQL test: ACTORS_TEST_DATABASE_URL is unset.");
        return;
    };
    let mut nonce = [0; 8];
    getrandom::fill(&mut nonce).unwrap();
    let ws = format!("runtime-test-{}", u64::from_le_bytes(nonce));
    let store = http::with_authenticator(
        PgStore::new(
            Pool::new(&dsn, 4).unwrap(),
            Arc::new(example::registry().unwrap()),
        ),
        Arc::new(Auth {
            revoked: AtomicBool::new(false),
        }),
    );
    store.migrate().await.unwrap();
    let client = Client::new(store.clone(), caller(&ws));
    let actor = client
        .actor::<example::Counter>("timer")
        .unwrap()
        .with_input(&example::CounterInput { initial: 0 })
        .unwrap();
    actor
        .call(
            &example::Add { delta: 1 },
            CallOptions {
                idempotency_key: Some("initial".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    actor
        .call(
            &example::Later {
                delta: 5,
                delay_ms: 0,
            },
            CallOptions {
                idempotency_key: Some("timer".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let runtime = Runtime::new(store)
        .config(RuntimeConfig {
            poll_interval: std::time::Duration::from_millis(20),
            listen: true,
            ..Default::default()
        })
        .start()
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if actor.view().await.unwrap().view["value"] == 6 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(runtime.stats().alarms >= 1);
    runtime.shutdown().await;
}

#[tokio::test]
async fn streams_are_bounded_and_recheck_access_before_the_first_frame() {
    use futures_util::StreamExt;
    let Ok(dsn) = std::env::var("ACTORS_TEST_DATABASE_URL") else {
        eprintln!("Skipping isolated PostgreSQL test: ACTORS_TEST_DATABASE_URL is unset.");
        return;
    };
    let mut nonce = [0; 8];
    getrandom::fill(&mut nonce).unwrap();
    let ws = format!("stream-test-{}", u64::from_le_bytes(nonce));
    let auth = Arc::new(Auth {
        revoked: AtomicBool::new(false),
    });
    let store = http::with_authenticator(
        PgStore::new(
            Pool::new(&dsn, 4).unwrap(),
            Arc::new(example::registry().unwrap()),
        ),
        auth.clone(),
    );
    store.migrate().await.unwrap();
    let actor = Client::new(store.clone(), caller(&ws))
        .actor::<example::Counter>("stream")
        .unwrap()
        .with_input(&example::CounterInput { initial: 0 })
        .unwrap();
    actor
        .call(&example::Add { delta: 1 }, CallOptions::default())
        .await
        .unwrap();
    let app = http::router(store, auth.clone());
    let get = || {
        Request::builder()
            .uri(format!("/v1/w/{ws}/actors/example.counter/stream/events"))
            .header("authorization", "Bearer test-only")
            .body(Body::empty())
            .unwrap()
    };
    let mut responses = Vec::new();
    for _ in 0..64 {
        let response = app.clone().oneshot(get()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        responses.push(response);
    }
    assert_eq!(
        app.clone().oneshot(get()).await.unwrap().status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    drop(responses);
    let response = app.clone().oneshot(get()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    auth.revoked.store(true, Ordering::Relaxed);
    let mut stream = response.into_body().into_data_stream();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
            .await
            .unwrap()
            .is_none()
    );
}
