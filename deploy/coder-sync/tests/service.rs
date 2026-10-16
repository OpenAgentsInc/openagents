use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use coder_sync::server::{
    StateData, authenticate,
    config::{AuthMode, Config},
    db::Database,
    rooms::{Room, Rooms},
    storage::Bucket,
};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{Mutex, oneshot};
use tower::ServiceExt;

fn vars(env: Option<&str>, auth: &str) -> HashMap<String, String> {
    let mut vars = HashMap::from([
        (
            "CODER_SYNC_DATABASE_URL".into(),
            "host=localhost dbname=test".into(),
        ),
        ("CODER_SYNC_BUCKET".into(), "test-bucket".into()),
        ("CODER_SYNC_AUTH".into(), auth.into()),
    ]);
    if let Some(env) = env {
        vars.insert("CODER_SYNC_ENV".into(), env.into());
    }
    vars
}

#[test]
fn production_and_unknown_environments_refuse_dev_auth_even_with_flag() {
    for env in [None, Some("production"), Some("prod"), Some("stagng")] {
        assert!(Config::parse(vars(env, "dev"), true).is_err());
        assert_eq!(
            Config::parse(vars(env, "disabled"), false).unwrap().auth,
            AuthMode::Disabled
        );
    }
    for env in ["local", "staging"] {
        assert!(Config::parse(vars(Some(env), "dev"), false).is_err());
        assert_eq!(
            Config::parse(vars(Some(env), "dev"), true).unwrap().auth,
            AuthMode::Dev
        );
    }
    assert!(Config::parse(vars(Some("local"), "workos"), true).is_err());
}

#[test]
fn openagents_auth_is_the_default_and_requires_a_valid_account_deployment() {
    let mut config = vars(Some("production"), "openagents");
    config.remove("CODER_SYNC_AUTH");
    assert_eq!(
        Config::parse(config.clone(), false).unwrap().auth,
        AuthMode::OpenAgents
    );
    config.insert(
        "CODER_SYNC_ACCOUNT_API".into(),
        "http://remote.example/v1".into(),
    );
    assert!(Config::parse(config, false).is_err());
    assert!(authenticate(AuthMode::OpenAgents, "alice@workspace").is_none());
}

#[test]
fn disabled_auth_accepts_no_token_and_dev_identity_is_scoped() {
    for token in ["", "test-user", "test-user@workspace", "sess_fake"] {
        assert!(authenticate(AuthMode::Disabled, token).is_none());
    }
    let identity = authenticate(AuthMode::Dev, "alice@workspace").unwrap();
    assert_eq!(identity.account, "alice");
    assert_eq!(identity.workspace.as_deref(), Some("workspace"));
    for token in ["", "@org", "alice@", "alice\nsecret"] {
        assert!(authenticate(AuthMode::Dev, token).is_none());
    }
}

#[tokio::test]
async fn room_writer_serializes_jobs_but_other_rooms_can_progress() {
    let rooms = Rooms::default();
    let values = Arc::new(Mutex::new(Vec::new()));
    let (release, gate) = oneshot::channel();
    let (begun, started) = oneshot::channel();
    let first = {
        let rooms = rooms.clone();
        let values = values.clone();
        tokio::spawn(async move {
            rooms
                .run(Room::Chat("a".into()), async move {
                    begun.send(()).unwrap();
                    gate.await.unwrap();
                    values.lock().await.push(1);
                })
                .await
                .unwrap();
        })
    };
    started.await.unwrap();
    let second = {
        let rooms = rooms.clone();
        let values = values.clone();
        tokio::spawn(async move {
            rooms
                .run(Room::Chat("a".into()), async move {
                    values.lock().await.push(2);
                })
                .await
                .unwrap();
        })
    };
    let independent = rooms.run(Room::Registry("alice".into()), async { 42 });
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), independent)
            .await
            .unwrap()
            .unwrap(),
        42
    );
    assert!(values.lock().await.is_empty());
    release.send(()).unwrap();
    first.await.unwrap();
    second.await.unwrap();
    assert_eq!(*values.lock().await, [1, 2]);
}

#[tokio::test]
async fn database_migrations_health_and_auth_with_fake_bucket() {
    let dsn = std::env::var("CODER_SYNC_TEST_DATABASE_URL")
        .expect("Run scripts/coder-sync/test.sh to use a disposable Postgres database");
    let db = Database::new(&dsn).unwrap();
    db.migrate().await.unwrap();
    db.migrate().await.unwrap();
    let client = db.connect().await.unwrap();
    for table in [
        "chat_rows",
        "chat_checkpoints",
        "registry_rows",
        "devices",
        "device_sessions",
        "blobs",
    ] {
        assert!(
            client
                .query_one("SELECT to_regclass($1)::text", &[&table])
                .await
                .unwrap()
                .get::<_, Option<String>>(0)
                .is_some()
        );
    }
    // The database backs up the actor's uniqueness and ordering constraints.
    client
        .execute(
            "INSERT INTO chats(chat_id,account) VALUES ('fixture','alice')",
            &[],
        )
        .await
        .unwrap();
    client.execute("INSERT INTO chat_rows(chat_id,seq,batch_id,bytes) VALUES ('fixture',1,'batch',decode('00','hex'))", &[]).await.unwrap();
    for (seq, batch) in [(0, "zero"), (1, "duplicate-seq"), (2, "batch")] {
        assert!(client.execute("INSERT INTO chat_rows(chat_id,seq,batch_id,bytes) VALUES ('fixture',$1,$2,decode('00','hex'))", &[&(seq as i64), &batch]).await.is_err());
    }
    let bucket = Bucket::memory();
    bucket.probe().await.unwrap();
    let app = coder_sync::server::router(StateData {
        database: db.clone(),
        bucket,
        rooms: Rooms::default(),
        devices: Default::default(),
        auth: AuthMode::Disabled,
        accounts: None,
        revision: "fixture".into(),
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-request-id"));
    let value: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(value["auth"], "disabled");
    for uri in ["/chat2/fixture/ws", "/chat2/fixture/ws?token=alice@org"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header("authorization", "Bearer alice@org")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn disconnecting_a_caller_does_not_discard_accepted_work() {
    let rooms = Rooms::default();
    let values = Arc::new(Mutex::new(Vec::new()));
    let (release, gate) = oneshot::channel();
    let (begun, started) = oneshot::channel();
    let caller = {
        let rooms = rooms.clone();
        let values = values.clone();
        tokio::spawn(async move {
            rooms
                .run(Room::Device("a".into()), async move {
                    begun.send(()).unwrap();
                    gate.await.unwrap();
                    values.lock().await.push(1);
                })
                .await
        })
    };
    started.await.unwrap();
    caller.abort();
    release.send(()).unwrap();
    let observed = rooms
        .run(Room::Device("a".into()), async move {
            values.lock().await.clone()
        })
        .await
        .unwrap();
    assert_eq!(observed, [1]);
}
