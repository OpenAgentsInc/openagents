//! oa-computers against a throwaway Postgres. Runs only with
//! `OA_COMPUTERS_TEST_DATABASE_URL` set to a server the tests may create
//! databases on (libpq key=value form, no `dbname`), for example
//! `host=127.0.0.1 port=55432 user=postgres`.
//!
//! - The API records intent and answers at once, refusing what the driver
//!   cannot do before writing anything.
//! - The reconciler drives a computer up and deletes it.
//! - Killed mid-provision (`SIGKILL` on the real binary while the computer
//!   boots on oa-boat), a restarted process finishes the same computer:
//!   exactly one sandbox, then a clean delete.

use computer_driver::fake::FakeDriver;
use computer_driver::{Capabilities, Driver, Profile};
use oa_computers::reconcile::{Reconciler, Settings};
use oa_computers::turns::Secrets;
use oa_computers::{Db, Store};
use serde_json::{Value, json};
use std::rc::Rc;
use std::time::{Duration, Instant};

const TOKEN: &str = "test-service-token-0123456789";

fn admin() -> Option<String> {
    let url = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL").ok()?;
    (!url.is_empty()).then_some(url)
}

async fn api(store: Store, caps: Capabilities) -> String {
    api_with(store, caps, Secrets::default()).await
}

async fn api_with(store: Store, caps: Capabilities, turns: Secrets) -> String {
    let app = oa_computers::app(store, caps, TOKEN, turns);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://127.0.0.1:{port}")
}

fn http() -> reqwest::Client {
    reqwest::Client::new()
}

async fn call(
    method: reqwest::Method,
    url: String,
    account: &str,
    body: Option<Value>,
) -> (u16, Value) {
    let mut r = http()
        .request(method, url)
        .bearer_auth(TOKEN)
        .header("X-OpenAgents-Account", account);
    if let Some(b) = body {
        r = r.json(&b);
    }
    let resp = r.send().await.unwrap();
    let status = resp.status().as_u16();
    let v = resp.json().await.unwrap_or(Value::Null);
    (status, v)
}

fn fast(holder: &str) -> Settings {
    let mut s = Settings::new(holder);
    s.lease = Duration::from_secs(2);
    s.poll = Duration::from_millis(50);
    s.revisit = Duration::from_millis(200);
    s.backoff = Duration::from_millis(100);
    s.driver_poll = Duration::from_millis(20);
    s.driver_deadline = Duration::from_secs(20);
    s.heartbeat = Duration::from_millis(100);
    s
}

async fn wait_for(base: &str, account: &str, id: &str, what: impl Fn(&Value) -> bool) -> Value {
    let until = Instant::now() + Duration::from_secs(40);
    loop {
        let (status, v) = call(
            reqwest::Method::GET,
            format!("{base}/v1/computers/{id}"),
            account,
            None,
        )
        .await;
        if status == 200 && what(&v) {
            return v;
        }
        if status == 404 && what(&Value::Null) {
            return Value::Null;
        }
        assert!(Instant::now() < until, "timed out; last {status} {v}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_api_records_intent_at_once_and_refuses_before_writing() {
    let Some(admin) = admin() else {
        eprintln!("skipping: OA_COMPUTERS_TEST_DATABASE_URL is unset");
        return;
    };
    let (db, _) = Db::scratch(&admin).await.unwrap();
    let store = Store::new(db.clone());
    let dir = tempfile::tempdir().unwrap();
    let base = api(store.clone(), FakeDriver::new(dir.path()).capabilities()).await;

    // Unauthenticated and unnamed callers are refused.
    let anon = http()
        .get(format!("{base}/v1/computers"))
        .send()
        .await
        .unwrap();
    assert_eq!(anon.status(), 401);
    let (status, _) = call(
        reqwest::Method::GET,
        format!("{base}/v1/computers"),
        "",
        None,
    )
    .await;
    assert_eq!(status, 400);

    let started = Instant::now();
    let (status, made) = call(
        reqwest::Method::POST,
        format!("{base}/v1/computers"),
        "acct-a",
        Some(json!({"chat": "chat-1"})),
    )
    .await;
    assert_eq!(status, 202, "{made}");
    assert!(started.elapsed() < Duration::from_secs(2));
    let id = made["id"].as_str().unwrap().to_owned();
    assert!(id.starts_with("cmp_"));
    assert_eq!(made["state"], "new");
    assert_eq!(made["desired"], "running");
    assert_eq!(made["size"], "small");

    // Legacy unkeyed chat ensures keep the original computer and image.
    let (status, again) = call(
        reqwest::Method::POST,
        format!("{base}/v1/computers"),
        "acct-a",
        Some(json!({"chat": "chat-1", "image": "another-template"})),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(again["id"], id.as_str());
    assert_eq!(store.get("acct-a", &id).await.unwrap().unwrap().image, None);

    // Another account neither sees nor reaches it.
    let (status, _) = call(
        reqwest::Method::GET,
        format!("{base}/v1/computers/{id}"),
        "acct-b",
        None,
    )
    .await;
    assert_eq!(status, 404);
    let (_, theirs) = call(
        reqwest::Method::GET,
        format!("{base}/v1/computers"),
        "acct-b",
        None,
    )
    .await;
    assert_eq!(theirs["computers"], json!([]));
    let (status, _) = call(
        reqwest::Method::DELETE,
        format!("{base}/v1/computers/{id}"),
        "acct-b",
        None,
    )
    .await;
    assert_eq!(status, 404);

    // A bad size is refused and nothing is written.
    let (status, _) = call(
        reqwest::Method::POST,
        format!("{base}/v1/computers"),
        "acct-a",
        Some(json!({"size": "huge"})),
    )
    .await;
    assert_eq!(status, 400);
    let (_, mine) = call(
        reqwest::Method::GET,
        format!("{base}/v1/computers"),
        "acct-a",
        None,
    )
    .await;
    assert_eq!(mine["computers"].as_array().unwrap().len(), 1);

    // A driver that cannot suspend cannot hold a chat computer: refused
    // before a row exists.
    let core = api(
        store.clone(),
        FakeDriver::with_profiles(dir.path(), [Profile::Core]).capabilities(),
    )
    .await;
    let (status, refused) = call(
        reqwest::Method::POST,
        format!("{core}/v1/computers"),
        "acct-c",
        Some(json!({"chat": "chat-9"})),
    )
    .await;
    assert_eq!(status, 422, "{refused}");
    assert_eq!(refused["error"], "unsupported");
    assert!(store.list("acct-c").await.unwrap().is_empty());

    // Delete records intent at once.
    let (status, gone) = call(
        reqwest::Method::DELETE,
        format!("{base}/v1/computers/{id}"),
        "acct-a",
        None,
    )
    .await;
    assert_eq!(status, 202);
    assert_eq!(gone["desired"], "deleted");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_reconciler_brings_a_computer_up_and_deletes_it() {
    let Some(admin) = admin() else {
        eprintln!("skipping: OA_COMPUTERS_TEST_DATABASE_URL is unset");
        return;
    };
    let (db, _) = Db::scratch(&admin).await.unwrap();
    let store = Store::new(db);
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeDriver::new(dir.path()).boot_observations(3);
    // The first create's reply is lost: the retry must find the same one.
    fake.inject("create", computer_driver::fake::Inject::LoseReply);
    let base = api(store.clone(), fake.capabilities()).await;
    let r = Rc::new(Reconciler {
        store: store.clone(),
        driver: fake.clone(),
        settings: fast("t1"),
        turns: Secrets::default(),
    });
    let local = tokio::task::LocalSet::new();
    local.spawn_local(r.run());
    local
        .run_until(async {
            let (_, made) = call(
                reqwest::Method::POST,
                format!("{base}/v1/computers"),
                "acct-a",
                Some(json!({"chat": "c1"})),
            )
            .await;
            let id = made["id"].as_str().unwrap().to_owned();
            let up = wait_for(&base, "acct-a", &id, |v| v["state"] == "awake").await;
            assert!(up["resource"].as_str().is_some());
            assert_eq!(fake.live().len(), 1, "one machine despite the lost reply");

            // Stop, then wake: same machine.
            call(
                reqwest::Method::POST,
                format!("{base}/v1/computers/{id}/stop"),
                "acct-a",
                None,
            )
            .await;
            wait_for(&base, "acct-a", &id, |v| v["state"] == "stopped").await;
            call(
                reqwest::Method::POST,
                format!("{base}/v1/computers/{id}/wake"),
                "acct-a",
                None,
            )
            .await;
            let again = wait_for(&base, "acct-a", &id, |v| v["state"] == "awake").await;
            assert_eq!(again["resource"], up["resource"]);
            assert_eq!(fake.live().len(), 1);

            call(
                reqwest::Method::DELETE,
                format!("{base}/v1/computers/{id}"),
                "acct-a",
                None,
            )
            .await;
            wait_for(&base, "acct-a", &id, |v| {
                v.is_null() || v["state"] == "deleted"
            })
            .await;
            let until = Instant::now() + Duration::from_secs(20);
            while !store.list("acct-a").await.unwrap().is_empty() {
                assert!(Instant::now() < until, "never settled as deleted");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            assert!(fake.live().is_empty());
        })
        .await;
}

/// The real binary on a real oa-boat (loopback, Compute Engine faked with a
/// slow insert), killed while the computer boots, then restarted.
#[tokio::test(flavor = "multi_thread")]
async fn a_process_killed_mid_provision_finishes_the_same_computer_after_restart() {
    let Some(admin) = admin() else {
        eprintln!("skipping: OA_COMPUTERS_TEST_DATABASE_URL is unset");
        return;
    };
    let (db, dsn) = Db::scratch(&admin).await.unwrap();
    let store = Store::new(db);

    // oa-boat on loopback.
    let boat_dir = tempfile::tempdir().unwrap();
    let fake = oa_boat::testing::Fake::default();
    {
        let mut w = fake.0.lock().unwrap();
        w.root = Some(boat_dir.path().to_owned());
        w.insert_delay = Some(Duration::from_millis(1500));
    }
    let service = oa_boat::Service::new(
        oa_boat::Config::for_tests(),
        fake.clone(),
        oa_boat::remote::Local {
            root: boat_dir.path().to_owned(),
        },
    );
    let boat_token = "boat-test-token-0123456789abcdef";
    let app = oa_boat::api::router(service, oa_boat::api::Token::new(boat_token));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let boat_port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let base = format!("http://127.0.0.1:{port}");
    let start = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_oa-computers"))
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("OA_COMPUTERS_DATABASE_URL", &dsn)
            .env("OA_COMPUTERS_TOKEN", TOKEN)
            .env("OA_COMPUTERS_DRIVER", "boat")
            .env(
                "BOAT_API_BASE",
                format!("http://127.0.0.1:{boat_port}/api/v1"),
            )
            .env("BOAT_API_KEY", boat_token)
            .env("OA_COMPUTERS_LISTEN", format!("127.0.0.1:{port}"))
            .env("OA_COMPUTERS_LEASE_SECONDS", "2")
            .env("OA_COMPUTERS_REVISIT_SECONDS", "1")
            .env("OA_COMPUTERS_BACKOFF_MS", "200")
            .env("OA_COMPUTERS_POLL_MS", "50")
            .env("OA_COMPUTERS_DRIVER_POLL_MS", "50")
            .spawn()
            .unwrap()
    };
    let ready = |base: String| async move {
        let until = Instant::now() + Duration::from_secs(20);
        while http().get(format!("{base}/healthz")).send().await.is_err() {
            assert!(Instant::now() < until, "oa-computers did not start");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };

    let mut first = start();
    ready(base.clone()).await;
    let (status, made) = call(
        reqwest::Method::POST,
        format!("{base}/v1/computers"),
        "acct-k",
        Some(json!({"chat": "crash"})),
    )
    .await;
    assert_eq!(status, 202, "{made}");
    let id = made["id"].as_str().unwrap().to_owned();

    // Kill it while the create is retained and the boot is in flight.
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let e = store.get("acct-k", &id).await.unwrap().unwrap();
        if matches!(e.phase.as_str(), "creating" | "booting") {
            break;
        }
        assert!(
            Instant::now() < until,
            "never started provisioning ({})",
            e.phase
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    first.kill().unwrap();
    first.wait().unwrap();
    let killed_at = store.get("acct-k", &id).await.unwrap().unwrap();
    assert_ne!(killed_at.phase, "awake", "killed too late to test anything");
    eprintln!("killed in phase {}", killed_at.phase);

    let mut second = start();
    ready(base.clone()).await;
    let up = wait_for(&base, "acct-k", &id, |v| v["state"] == "awake").await;
    let e = store.get("acct-k", &id).await.unwrap().unwrap();
    assert_eq!(e.record.creates.len(), 1, "a second create was attempted");
    let boat = boat::Client::builder(boat::ApiKey::new(boat_token).unwrap())
        .base_url(format!("http://127.0.0.1:{boat_port}/api/v1"))
        .build()
        .unwrap();
    let sandboxes = boat.sandboxes(&Default::default()).await.unwrap();
    assert_eq!(sandboxes.sandboxes.len(), 1, "exactly one sandbox");
    assert_eq!(up["resource"], sandboxes.sandboxes[0].id.as_str());

    // Clean up through the same API: nothing is left on oa-boat.
    call(
        reqwest::Method::DELETE,
        format!("{base}/v1/computers/{id}"),
        "acct-k",
        None,
    )
    .await;
    let until = Instant::now() + Duration::from_secs(30);
    while !store.list("acct-k").await.unwrap().is_empty() {
        assert!(Instant::now() < until, "never deleted");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let left = boat.sandboxes(&Default::default()).await.unwrap();
    assert!(left.sandboxes.is_empty(), "sandbox left behind");
    second.kill().unwrap();
    second.wait().unwrap();
}

/// A chat's turns run on its own computer: the first turn's files are there
/// for the second, each turn's credential reaches its own command only and
/// is never stored, the computer stops with its disk after each turn, and a
/// turn whose credential did not survive a restart fails instead of running
/// without it.
#[tokio::test(flavor = "multi_thread")]
async fn turns_run_on_the_chats_computer_with_credentials_in_memory_only() {
    let Some(admin) = admin() else {
        eprintln!("skipping: OA_COMPUTERS_TEST_DATABASE_URL is unset");
        return;
    };
    let (db, _) = Db::scratch(&admin).await.unwrap();
    let store = Store::new(db.clone());
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeDriver::new(dir.path());
    let secrets = Secrets::default();
    let base = api_with(store.clone(), fake.capabilities(), secrets.clone()).await;
    let r = Rc::new(Reconciler {
        store: store.clone(),
        driver: fake.clone(),
        settings: fast("t-turns"),
        turns: secrets.clone(),
    });
    let local = tokio::task::LocalSet::new();
    local.spawn_local(r.run());
    let secret = "sk-ant-oat01-turn-secret-value";
    local
        .run_until(async {
            let (_, made) = call(
                reqwest::Method::POST,
                format!("{base}/v1/computers"),
                "acct-t",
                Some(json!({"chat": "chat-t"})),
            )
            .await;
            let id = made["id"].as_str().unwrap().to_owned();
            let turn = |command: &str, env: Value| {
                let base = base.clone();
                let id = id.clone();
                let command = command.to_owned();
                async move {
                    let (status, t) = call(
                        reqwest::Method::POST,
                        format!("{base}/v1/computers/{id}/turns"),
                        "acct-t",
                        Some(json!({"command": command, "env": env})),
                    )
                    .await;
                    assert_eq!(status, 202, "{t}");
                    let tid = t["id"].as_str().unwrap().to_owned();
                    let until = Instant::now() + Duration::from_secs(30);
                    loop {
                        let (_, v) = call(
                            reqwest::Method::GET,
                            format!("{base}/v1/computers/{id}/turns/{tid}"),
                            "acct-t",
                            None,
                        )
                        .await;
                        if v["state"] == "done" || v["state"] == "failed" {
                            return v;
                        }
                        assert!(Instant::now() < until, "turn never finished: {v}");
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            };
            let one = turn(
                "echo first > notes.txt; printf %s \"$CLAUDE_CODE_OAUTH_TOKEN\" | wc -c",
                json!({"CLAUDE_CODE_OAUTH_TOKEN": secret}),
            )
            .await;
            assert_eq!(one["state"], "done", "{one}");
            assert_eq!(one["exit_code"], 0);
            assert_eq!(
                one["stdout"].as_str().unwrap().trim(),
                secret.len().to_string()
            );
            assert_eq!(one["env_names"], json!(["CLAUDE_CODE_OAUTH_TOKEN"]));
            // The computer checkpointed by stopping, and is not woken again.
            let stopped = wait_for(&base, "acct-t", &id, |v| v["state"] == "stopped").await;
            assert_eq!(stopped["desired"], "stopped");
            assert_eq!(stopped["turns"], 1);

            let two = turn(
                "cat notes.txt; echo \"[${CLAUDE_CODE_OAUTH_TOKEN:-}]\"",
                json!({}),
            )
            .await;
            assert_eq!(two["state"], "done", "{two}");
            assert_eq!(two["stdout"], "first\n[]\n");
            assert_eq!(fake.live().len(), 1, "one machine for the chat");

            // Nothing in the database holds the secret.
            let c = db.client().await.unwrap();
            for table in ["computers.computers", "computers.turns"] {
                let rows = c
                    .query(&format!("SELECT t::text FROM {table} t"), &[])
                    .await
                    .unwrap();
                for row in rows {
                    let text: String = row.get(0);
                    assert!(!text.contains(secret), "{table} holds the secret");
                }
            }

            // Stop ends a running turn's command.
            let (_, t) = call(
                reqwest::Method::POST,
                format!("{base}/v1/computers/{id}/turns"),
                "acct-t",
                Some(json!({"command": "sleep 30"})),
            )
            .await;
            let tid = t["id"].as_str().unwrap().to_owned();
            let until = Instant::now() + Duration::from_secs(30);
            loop {
                let (_, v) = call(
                    reqwest::Method::GET,
                    format!("{base}/v1/computers/{id}/turns/{tid}"),
                    "acct-t",
                    None,
                )
                .await;
                if v["state"] == "running" {
                    break;
                }
                assert!(Instant::now() < until, "never ran: {v}");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            let started = Instant::now();
            let (status, _) = call(
                reqwest::Method::POST,
                format!("{base}/v1/computers/{id}/turns/{tid}/stop"),
                "acct-t",
                None,
            )
            .await;
            assert_eq!(status, 202);
            let stopped = loop {
                let (_, v) = call(
                    reqwest::Method::GET,
                    format!("{base}/v1/computers/{id}/turns/{tid}"),
                    "acct-t",
                    None,
                )
                .await;
                if v["state"] == "failed" {
                    break v;
                }
                assert!(
                    started.elapsed() < Duration::from_secs(15),
                    "stop did not end it: {v}"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            };
            assert_eq!(stopped["error"], "Stopped.");

            // A restart forgets waiting credentials: the turn fails rather
            // than running without them.
            let (status, t) = call(
                reqwest::Method::POST,
                format!("{base}/v1/computers/{id}/turns"),
                "acct-t",
                Some(json!({"command": "true", "env": {"ANTHROPIC_API_KEY": "sk-ant-api-x"}})),
            )
            .await;
            assert_eq!(status, 202);
            let tid = t["id"].as_str().unwrap().to_owned();
            secrets.take(&tid);
            let until = Instant::now() + Duration::from_secs(30);
            let lost = loop {
                let (_, v) = call(
                    reqwest::Method::GET,
                    format!("{base}/v1/computers/{id}/turns/{tid}"),
                    "acct-t",
                    None,
                )
                .await;
                if v["state"] == "failed" || v["state"] == "done" {
                    break v;
                }
                assert!(Instant::now() < until);
                tokio::time::sleep(Duration::from_millis(50)).await;
            };
            assert_eq!(lost["state"], "failed");
            assert!(lost["error"].as_str().unwrap().contains("Send it again"));

            call(
                reqwest::Method::DELETE,
                format!("{base}/v1/computers/{id}"),
                "acct-t",
                None,
            )
            .await;
            let until = Instant::now() + Duration::from_secs(20);
            while !store.list("acct-t").await.unwrap().is_empty() {
                assert!(Instant::now() < until, "never deleted");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            assert!(fake.live().is_empty());
        })
        .await;
}

/// A computer left stopped past the keep period is deleted by itself.
#[tokio::test(flavor = "multi_thread")]
async fn a_computer_left_stopped_is_deleted_after_the_keep_period() {
    let Some(admin) = admin() else {
        eprintln!("skipping: OA_COMPUTERS_TEST_DATABASE_URL is unset");
        return;
    };
    let (db, _) = Db::scratch(&admin).await.unwrap();
    let store = Store::new(db);
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeDriver::new(dir.path());
    let base = api(store.clone(), fake.capabilities()).await;
    let mut settings = fast("t-keep");
    settings.keep_stopped = Duration::from_millis(500);
    let r = Rc::new(Reconciler {
        store: store.clone(),
        driver: fake.clone(),
        settings,
        turns: Secrets::default(),
    });
    let local = tokio::task::LocalSet::new();
    local.spawn_local(r.run());
    local
        .run_until(async {
            let (_, made) = call(
                reqwest::Method::POST,
                format!("{base}/v1/computers"),
                "acct-k",
                Some(json!({"chat": "k1"})),
            )
            .await;
            let id = made["id"].as_str().unwrap().to_owned();
            wait_for(&base, "acct-k", &id, |v| v["state"] == "awake").await;
            call(
                reqwest::Method::POST,
                format!("{base}/v1/computers/{id}/stop"),
                "acct-k",
                None,
            )
            .await;
            wait_for(&base, "acct-k", &id, |v| v["state"] == "deleted").await;
            assert!(fake.live().is_empty());
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn create_retries_preserve_settings_keys_and_atomic_account_limits() {
    let Some(admin) = admin() else {
        eprintln!("skipping: OA_COMPUTERS_TEST_DATABASE_URL is unset");
        return;
    };
    let (db, _) = Db::scratch(&admin).await.unwrap();
    let store = Store::new(db.clone());
    let dir = tempfile::tempdir().unwrap();
    let caps = FakeDriver::new(dir.path()).capabilities();
    let base = api(store.clone(), caps.clone()).await;
    let create_url = format!("{base}/v1/computers");
    let request = json!({"chat":"retry-chat", "size":"small", "idempotency_key":"key-one"});
    let (status, first) = call(
        reqwest::Method::POST,
        create_url.clone(),
        "alice",
        Some(request.clone()),
    )
    .await;
    assert_eq!(status, 202, "{first}");
    let id = first["id"].as_str().unwrap();
    let mut conflict = request.clone();
    conflict["image"] = json!("different-template");
    let (status, failure) = call(
        reqwest::Method::POST,
        create_url.clone(),
        "alice",
        Some(conflict),
    )
    .await;
    assert_eq!(status, 409, "{failure}");
    assert_eq!(failure["error"], "idempotency_conflict");
    assert_eq!(store.get("alice", id).await.unwrap().unwrap().size, "small");

    let mismatched = http()
        .post(&create_url)
        .bearer_auth(TOKEN)
        .header("X-OpenAgents-Account", "alice")
        .header("Idempotency-Key", "key-two")
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(mismatched.status(), 400);
    let restarted = api(store.clone(), caps).await;
    let (status, retried) = call(
        reqwest::Method::POST,
        format!("{restarted}/v1/computers"),
        "alice",
        Some(request.clone()),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(retried["id"], first["id"]);

    // A fresh key for an existing chat must remain bound even after deletion.
    let mut alias = request;
    alias["idempotency_key"] = json!("key-alias");
    let (status, aliased) = call(
        reqwest::Method::POST,
        create_url.clone(),
        "alice",
        Some(alias.clone()),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(aliased["id"], first["id"]);
    store
        .set_desired("alice", id, oa_computers::store::Desired::Stopped)
        .await
        .unwrap();
    let (status, stopped_retry) = call(
        reqwest::Method::POST,
        create_url.clone(),
        "alice",
        Some(alias.clone()),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(stopped_retry["desired"], "stopped");
    assert_eq!(
        store.get("alice", id).await.unwrap().unwrap().desired,
        oa_computers::store::Desired::Stopped
    );
    store
        .set_desired("alice", id, oa_computers::store::Desired::Deleted)
        .await
        .unwrap();
    let (status, deleted_retry) = call(
        reqwest::Method::POST,
        create_url.clone(),
        "alice",
        Some(alias.clone()),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(deleted_retry["id"], first["id"]);
    assert_eq!(deleted_retry["desired"], "deleted");
    let (status, replacement) = call(
        reqwest::Method::POST,
        create_url.clone(),
        "alice",
        Some(json!({"chat":"retry-chat","idempotency_key":"new-chat-request"})),
    )
    .await;
    assert_eq!(status, 202);
    assert_ne!(replacement["id"], first["id"]);
    let (status, old_retry) = call(
        reqwest::Method::POST,
        create_url.clone(),
        "alice",
        Some(alias),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(
        old_retry["id"], first["id"],
        "an old key stays bound after the chat gets a new computer"
    );

    let mut creates = tokio::task::JoinSet::new();
    for n in 0..25 {
        let url = create_url.clone();
        creates.spawn(async move {
            let key = format!("quota-{n}");
            (
                key.clone(),
                call(
                    reqwest::Method::POST,
                    url,
                    "quota-account",
                    Some(json!({"idempotency_key": key})),
                )
                .await,
            )
        });
    }
    let mut admitted = Vec::new();
    let mut refused = 0;
    while let Some(result) = creates.join_next().await {
        let (key, (status, body)) = result.unwrap();
        match status {
            202 => admitted.push((key, body)),
            429 => {
                assert_eq!(body["error"], "quota_exhausted");
                refused += 1;
            }
            _ => panic!("unexpected {status}: {body}"),
        }
    }
    assert_eq!(admitted.len(), 20);
    assert_eq!(refused, 5);
    let (key, old) = &admitted[0];
    let (status, retry) = call(
        reqwest::Method::POST,
        create_url.clone(),
        "quota-account",
        Some(json!({"idempotency_key":key})),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(retry["id"], old["id"]);
    let (status, independent) = call(
        reqwest::Method::POST,
        create_url.clone(),
        "bob",
        Some(json!({"idempotency_key":key})),
    )
    .await;
    assert_eq!(status, 202);
    assert_ne!(independent["id"], old["id"]);

    let mut cursor = None;
    let mut seen = std::collections::BTreeSet::new();
    loop {
        let url = match cursor {
            Some(ref id) => format!("{create_url}?limit=3&after={id}"),
            None => format!("{create_url}?limit=3"),
        };
        let (status, page) = call(reqwest::Method::GET, url, "quota-account", None).await;
        assert_eq!(status, 200, "{page}");
        for item in page["computers"].as_array().unwrap() {
            assert!(seen.insert(item["id"].as_str().unwrap().to_owned()));
        }
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen.len(), 20);
    let (status, _) = call(
        reqwest::Method::GET,
        format!("{create_url}?after={id}"),
        "bob",
        None,
    )
    .await;
    assert_eq!(status, 400);
}
