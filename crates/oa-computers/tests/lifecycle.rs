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

#[derive(Clone)]
struct CommandGuest(oa_boat::remote::Local);
impl CommandGuest {
    fn map(&self, host: &str, script: &str) -> String {
        let root = self.0.root.join(host);
        script.replace(
            "/tmp/oa-public-commands",
            root.join("commands").to_str().unwrap(),
        )
    }
}
impl oa_boat::remote::Remote for CommandGuest {
    async fn run(
        &self,
        host: &str,
        script: &str,
        stdin: Vec<u8>,
        limit: Duration,
        cap: usize,
    ) -> oa_boat::remote::Output {
        let output = self
            .0
            .run(host, &self.map(host, script), stdin, limit, cap)
            .await;
        if script.contains("oa-public-commands") && script.contains("nohup") {
            // Lose the start reply after the guest has already made effects.
            tokio::time::sleep(Duration::from_secs(8)).await;
        }
        output
    }
    async fn stream(
        &self,
        host: &str,
        script: &str,
        limit: Duration,
    ) -> tokio::sync::mpsc::Receiver<oa_boat::remote::Chunk> {
        self.0.stream(host, &self.map(host, script), limit).await
    }
    async fn forget(&self, host: &str) {
        self.0.forget(host).await
    }
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
        CommandGuest(oa_boat::remote::Local {
            root: boat_dir.path().to_owned(),
        }),
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

    // Kill the real API while its detached start reply is still in flight.
    // The guest has made its effect; no provider reply has been committed.
    let command_request = json!({"command":"echo effect >> counter; printf initial; sleep 4; printf final","idempotency_key":"process-death"});
    let command_path = format!("{base}/v1/computers/{id}/commands");
    let (status, command) = call(
        reqwest::Method::POST,
        command_path.clone(),
        "acct-k",
        Some(command_request.clone()),
    )
    .await;
    assert_eq!(status, 202, "{command}");
    let command_id = command["id"].as_str().unwrap();
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let retained = store
            .command("acct-k", &id, command_id)
            .await
            .unwrap()
            .unwrap();
        let effect = std::fs::read_dir(boat_dir.path())
            .unwrap()
            .any(|e| e.is_ok_and(|e| e.path().join("home/workspace/counter").exists()));
        if effect {
            assert_eq!(
                retained.status, "dispatching",
                "the start reply arrived before the crash"
            );
            break;
        }
        assert!(
            Instant::now() < until,
            "command produced no output: {}",
            retained.status
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    second.kill().unwrap();
    second.wait().unwrap();
    let mut second = start();
    ready(base.clone()).await;
    let (status, retry) = call(
        reqwest::Method::POST,
        command_path.clone(),
        "acct-k",
        Some(command_request),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(retry["id"], command_id);
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let (_, result) = call(
            reqwest::Method::GET,
            format!("{command_path}/{command_id}"),
            "acct-k",
            None,
        )
        .await;
        if terminal_command(&result) {
            assert_eq!(result["state"], "succeeded", "{result}");
            break;
        }
        assert!(Instant::now() < until, "command did not recover: {result}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let counters = std::fs::read_dir(boat_dir.path())
        .unwrap()
        .filter_map(|e| std::fs::read_to_string(e.ok()?.path().join("home/workspace/counter")).ok())
        .collect::<Vec<_>>();
    assert_eq!(counters, vec!["effect\n".to_owned()]);

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
            let stopped = wait_for(&base, "acct-t", &id, |v| {
                v["state"] == "stopped" && v["desired"] == "stopped"
            })
            .await;
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

    // An older service can still write the original key column during a
    // rolling deployment, after the new alias table has been backfilled.
    let legacy_request = json!({"idempotency_key":"old-writer-key"});
    let (status, legacy) = call(
        reqwest::Method::POST,
        create_url.clone(),
        "legacy-account",
        Some(legacy_request.clone()),
    )
    .await;
    assert_eq!(status, 202);
    store
        .db
        .client()
        .await
        .unwrap()
        .execute(
            "DELETE FROM computers.create_requests WHERE owner = $1",
            &[&"legacy-account"],
        )
        .await
        .unwrap();
    let (status, legacy_retry) = call(
        reqwest::Method::POST,
        create_url.clone(),
        "legacy-account",
        Some(legacy_request),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(legacy_retry["id"], legacy["id"]);

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

async fn wait_command(
    base: &str,
    computer: &str,
    command: &str,
    predicate: impl Fn(&Value) -> bool,
) -> Value {
    let until = Instant::now() + Duration::from_secs(40);
    loop {
        let (status, v) = call(
            reqwest::Method::GET,
            format!("{base}/v1/computers/{computer}/commands/{command}"),
            "acct-a",
            None,
        )
        .await;
        assert_eq!(status, 200, "{v}");
        if predicate(&v) {
            return v;
        }
        assert!(Instant::now() < until, "command timed out: {v}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
fn terminal_command(v: &Value) -> bool {
    matches!(
        v["state"].as_str(),
        Some("succeeded" | "failed" | "cancelled" | "timed_out" | "unknown")
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn public_commands_retain_output_resume_cancel_and_refuse_foreign_or_invalid_requests() {
    use base64::Engine;
    let Some(admin) = admin() else { return };
    let (db, _) = Db::scratch(&admin).await.unwrap();
    let store = Store::new(db.clone());
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeDriver::new(dir.path());
    let base = api(store.clone(), fake.capabilities()).await;
    let r = Rc::new(Reconciler {
        store: store.clone(),
        driver: fake.clone(),
        settings: fast("commands"),
        turns: Secrets::default(),
    });
    let local = tokio::task::LocalSet::new();
    local.spawn_local(r.run());
    local.run_until(async {
        let (_,computer)=call(reqwest::Method::POST,format!("{base}/v1/computers"),"acct-a",Some(json!({"idempotency_key":"public-commands"}))).await;
        let id=computer["id"].as_str().unwrap();
        let up=wait_for(&base,"acct-a",id,|v|v["state"]=="awake").await;
        let resource=up["resource"].as_str().unwrap();
        let path=format!("{base}/v1/computers/{id}/commands");
        let request=json!({"command":"echo one >> counter; printf early; printf error >&2","idempotency_key":"first"});
        fake.inject("start_command",computer_driver::fake::Inject::LoseReply);
        let (status,accepted)=call(reqwest::Method::POST,path.clone(),"acct-a",Some(request.clone())).await;
        assert_eq!(status,202,"{accepted}");let command=accepted["id"].as_str().unwrap();
        // Simulate a lost create response: repeat the exact request.
        let (status,retry)=call(reqwest::Method::POST,path.clone(),"acct-a",Some(request)).await;
        assert_eq!(status,200);assert_eq!(retry["id"],command);
        let final_state=wait_command(&base,id,command,terminal_command).await;
        assert_eq!(final_state["state"],"succeeded","{final_state}");assert_eq!(final_state["exit_code"],0);
        let (_,legacy)=call(reqwest::Method::GET,format!("{base}/v1/computers/{id}/turns/{command}"),"acct-a",None).await;
        assert_eq!(legacy["state"],"done");assert_eq!(legacy["stdout"],"early");assert_eq!(legacy["stderr"],"error");
        let (_,output)=call(reqwest::Method::GET,format!("{path}/{command}/output?max_bytes=2"),"acct-a",None).await;
        let b64=base64::engine::general_purpose::STANDARD;
        assert_eq!(b64.decode(output["stdout"].as_str().unwrap()).unwrap(),b"ea");
        assert_eq!(output["next_cursor"]["stdout"],2);
        let (_,tail)=call(reqwest::Method::GET,format!("{path}/{command}/output?stdout=2&stderr=2"),"acct-a",None).await;
        assert_eq!(b64.decode(tail["stdout"].as_str().unwrap()).unwrap(),b"rly");
        assert_eq!(b64.decode(tail["stderr"].as_str().unwrap()).unwrap(),b"ror");
        let (status,_)=call(reqwest::Method::GET,format!("{path}/{command}/output"),"acct-b",None).await;assert_eq!(status,404);
        let (status,_)=call(reqwest::Method::POST,path.clone(),"acct-a",Some(json!({"command":"echo changed","idempotency_key":"first"}))).await;assert_eq!(status,409);
        assert_eq!(std::fs::read_to_string(dir.path().join(resource).join("counter")).unwrap(),"one\n");
        assert_eq!(fake.calls().iter().filter(|s|s.starts_with("start_command:")).count(),1);
        for bad in [json!({"command":"true","cwd":"../outside","idempotency_key":"bad"}),json!({"command":"true","cwd":"/etc","idempotency_key":"bad"}),json!({"command":"true","timeout_seconds":0,"idempotency_key":"bad"}),json!({"command":"true","env":{"API_KEY":"secret-canary"},"idempotency_key":"bad"}),json!({"command":"true","credentials":["foreign"],"idempotency_key":"bad"})] {
            let (status,v)=call(reqwest::Method::POST,path.clone(),"acct-a",Some(bad)).await;assert_eq!(status,400,"{v}");assert!(!v.to_string().contains("secret-canary"));
        }
        let leaked:i64=db.client().await.unwrap().query_one("SELECT count(*) FROM computers.commands WHERE options::text LIKE '%secret-canary%'",&[]).await.unwrap().get(0);
        assert_eq!(leaked,0,"rejected credentials must not enter command metadata");
        // Lost cancellation reply: the worker must observe exit, not trust its ack.
        let (_,accepted)=call(reqwest::Method::POST,path.clone(),"acct-a",Some(json!({"command":"echo $$ > parent; trap '' TERM; sleep 100 & echo $! > child; printf started; wait","idempotency_key":"cancel"}))).await;
        let cancel_id=accepted["id"].as_str().unwrap();
        wait_command(&base,id,cancel_id,|v|v["state"]=="running" && v["stdout_bytes"].as_u64().unwrap_or(0)>0).await;
        let until=Instant::now()+Duration::from_secs(5);
        while !dir.path().join(resource).join("child").exists(){assert!(Instant::now()<until);tokio::time::sleep(Duration::from_millis(20)).await;}
        fake.inject("stop_command",computer_driver::fake::Inject::LoseReply);
        let (status,_) =call(reqwest::Method::POST,format!("{path}/{cancel_id}/cancel"),"acct-a",None).await;assert_eq!(status,202);
        assert_eq!(wait_command(&base,id,cancel_id,terminal_command).await["state"],"cancelled");
        for name in ["parent","child"] {
            let pid=std::fs::read_to_string(dir.path().join(resource).join(name)).unwrap();
            assert!(std::fs::read_to_string(format!("/proc/{}/stat",pid.trim())).ok().is_none_or(|s|s.contains(") Z ")));
        }
        // A non-reading client does not grow the guest or database output.
        let (_,accepted)=call(reqwest::Method::POST,path.clone(),"acct-a",Some(json!({"command":"head -c 8388608 /dev/zero; head -c 8388608 /dev/zero >&2","idempotency_key":"flood"}))).await;
        let flood=accepted["id"].as_str().unwrap();
        let result=wait_command(&base,id,flood,terminal_command).await;assert_eq!(result["state"],"succeeded");assert_eq!(result["stdout_truncated"],true);assert_eq!(result["stderr_truncated"],true);
        let record=store.command("acct-a",id,flood).await.unwrap().unwrap();assert_eq!(record.stdout.len(),262144);assert_eq!(record.stderr.len(),262144);
        let (_,first_chunk)=call(reqwest::Method::GET,format!("{path}/{flood}/output"),"acct-a",None).await;
        assert_eq!(first_chunk["terminal"],true);assert_eq!(first_chunk["has_more"],true);
        assert_eq!(b64.decode(first_chunk["stdout"].as_str().unwrap()).unwrap().len(),65536);
        let (_,output)=call(reqwest::Method::GET,format!("{path}/{flood}/output?stdout=262144&stderr=262144"),"acct-a",None).await;assert_eq!(output["gaps"]["stdout"],true);
        assert_eq!(output["has_more"],false);
        db.client().await.unwrap().execute("UPDATE computers.commands SET expires_at=now()-interval '1 second' WHERE turn=$1",&[&flood]).await.unwrap();
        assert_eq!(call(reqwest::Method::GET,format!("{path}/{flood}/output"),"acct-a",None).await.0,410);
        assert!(store.command("acct-a",id,flood).await.unwrap().unwrap().stdout.is_empty());
        let (_,accepted)=call(reqwest::Method::POST,path.clone(),"acct-a",Some(json!({"command":"sleep 100","timeout_seconds":1,"idempotency_key":"deadline"}))).await;
        assert_eq!(wait_command(&base,id,accepted["id"].as_str().unwrap(),terminal_command).await["state"],"timed_out");
        let (_,active)=call(reqwest::Method::POST,path.clone(),"acct-a",Some(json!({"command":"sleep 100","idempotency_key":"computer-stop-active"}))).await;
        let active=active["id"].as_str().unwrap();
        wait_command(&base,id,active,|v|v["state"]=="running").await;
        let (_,queued)=call(reqwest::Method::POST,path.clone(),"acct-a",Some(json!({"command":"echo forbidden >> counter","idempotency_key":"computer-stop-queued"}))).await;
        let queued=queued["id"].as_str().unwrap();
        let (status,legacy)=call(reqwest::Method::POST,format!("{base}/v1/computers/{id}/turns/{queued}/stop"),"acct-a",None).await;
        assert_eq!(status,202);assert_eq!(legacy["state"],"failed");
        let (_,queued)=call(reqwest::Method::POST,path.clone(),"acct-a",Some(json!({"command":"echo forbidden >> counter","idempotency_key":"computer-stop-queued-2"}))).await;
        let queued=queued["id"].as_str().unwrap();
        assert_eq!(call(reqwest::Method::POST,format!("{base}/v1/computers/{id}/stop"),"acct-a",None).await.0,202);
        assert_eq!(wait_command(&base,id,queued,terminal_command).await["state"],"cancelled");
        assert_eq!(wait_command(&base,id,active,terminal_command).await["state"],"cancelled");
        wait_for(&base,"acct-a",id,|v|v["state"]=="stopped" && v["desired"]=="stopped").await;
        assert_eq!(std::fs::read_to_string(dir.path().join(resource).join("counter")).unwrap(),"one\n");
        call(reqwest::Method::DELETE,format!("{base}/v1/computers/{id}"),"acct-a",None).await;
        wait_for(&base,"acct-a",id,|v|v["state"]=="deleted").await;
    }).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn command_owner_restart_observes_the_same_execution_and_never_replays_absent_dispatch() {
    let Some(admin) = admin() else { return };
    let (db, _) = Db::scratch(&admin).await.unwrap();
    let store = Store::new(db.clone());
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeDriver::new(dir.path());
    let base = api(store.clone(), fake.capabilities()).await;
    let (id, command, resource) = {
        let local = tokio::task::LocalSet::new();
        local.spawn_local(
            Rc::new(Reconciler {
                store: store.clone(),
                driver: fake.clone(),
                settings: fast("before-crash"),
                turns: Secrets::default(),
            })
            .run(),
        );
        local.run_until(async {
            let (_,computer)=call(reqwest::Method::POST,format!("{base}/v1/computers"),"acct-a",Some(json!({"idempotency_key":"restart-command"}))).await;
            let id=computer["id"].as_str().unwrap().to_owned();
            let up=wait_for(&base,"acct-a",&id,|v|v["state"]=="awake").await;
            let (_,accepted)=call(reqwest::Method::POST,format!("{base}/v1/computers/{id}/commands"),"acct-a",Some(json!({"command":"echo effect >> counter; echo initial; sleep 4; echo final","idempotency_key":"once"}))).await;
            let command=accepted["id"].as_str().unwrap().to_owned();
            wait_command(&base,&id,&command,|v|v["state"]=="running").await;
            (id,command,up["resource"].as_str().unwrap().to_owned())
        }).await
        // Dropping LocalSet aborts the owner, heartbeat, and lease renewer;
        // the separately detached shell is still running.
    };
    tokio::time::sleep(Duration::from_millis(2200)).await;
    let local = tokio::task::LocalSet::new();
    local.spawn_local(
        Rc::new(Reconciler {
            store: store.clone(),
            driver: fake.clone(),
            settings: fast("after-crash"),
            turns: Secrets::default(),
        })
        .run(),
    );
    local.run_until(async {
        assert_eq!(wait_command(&base,&id,&command,terminal_command).await["state"],"succeeded");
        assert_eq!(std::fs::read_to_string(dir.path().join(&resource).join("counter")).unwrap(),"effect\n");
        assert_eq!(fake.calls().iter().filter(|s|s.starts_with("start_command:")).count(),1);
        wait_for(&base,"acct-a",&id,|v|v["state"]=="stopped" && v["desired"]=="stopped").await;
        db.client().await.unwrap().execute("UPDATE computers.computers SET lease_holder='paused-crash-window',lease_until=now()+interval '1 hour' WHERE id=$1",&[&id]).await.unwrap();
        // Simulate death after durable dispatch ownership, before any wire send.
        let request=oa_computers::commands::Request{credential_ref:None,command:"echo must-not-run >> counter".into(),cwd:".".into(),timeout_seconds:10,env:Default::default(),idempotency_key:"absent-dispatch".into()};
        let (absent,_)=store.add_command("acct-a",&id,&request).await.unwrap();
        // This is the query shipped by the pre-command worker. It must never
        // see a public command as a legacy foreground turn during rollout.
        let legacy_count:i64=db.client().await.unwrap().query_one("SELECT count(*) FROM computers.turns WHERE computer=$1 AND state IN ('queued','running')",&[&id]).await.unwrap().get(0);
        assert_eq!(legacy_count,0);
        db.client().await.unwrap().execute("UPDATE computers.commands SET status='dispatching',dispatched_at=now() WHERE turn=$1",&[&absent]).await.unwrap();
        db.client().await.unwrap().execute("UPDATE computers.computers SET lease_holder=NULL,lease_until=NULL WHERE id=$1",&[&id]).await.unwrap();
        assert_eq!(wait_command(&base,&id,&absent,terminal_command).await["state"],"unknown");
        assert_eq!(fake.calls().iter().filter(|s|s.starts_with("start_command:")).count(),1);
        assert_eq!(std::fs::read_to_string(dir.path().join(&resource).join("counter")).unwrap(),"effect\n");
        call(reqwest::Method::DELETE,format!("{base}/v1/computers/{id}"),"acct-a",None).await;
        wait_for(&base,"acct-a",&id,|v|v["state"]=="deleted").await;
    }).await;
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread")]
async fn bounded_files_share_owner_and_never_wake_or_recreate_computers() {
    use computer_driver::boat::BoatDriver;
    use computer_files::{MAX_BYTES, digest};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let Some(admin) = admin() else {
        eprintln!("skipping: OA_COMPUTERS_TEST_DATABASE_URL is unset");
        return;
    };
    let (db, _) = Db::scratch(&admin).await.unwrap();
    let store = Store::new(db);
    let dir = tempfile::tempdir().unwrap();
    let fake = oa_boat::testing::Fake::default();
    fake.0.lock().unwrap().root = Some(dir.path().to_owned());
    let helper = std::env::var_os("OA_COMPUTER_FILES_HELPER")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("computer-files")
        });
    assert!(
        helper.is_file(),
        "build the computer-files binary before protected-file tests"
    );
    let mut cfg = oa_boat::Config::for_tests();
    cfg.workspace_helper = Some(helper);
    let service = oa_boat::Service::new(
        cfg,
        fake.clone(),
        oa_boat::remote::Local {
            root: dir.path().to_owned(),
        },
    );
    let boat_app = oa_boat::api::router(service, oa_boat::api::Token::new(TOKEN));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let boat_port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, boat_app).await.unwrap();
    });
    let boat = boat::Client::builder(boat::ApiKey::new(TOKEN).unwrap())
        .base_url(format!("http://127.0.0.1:{boat_port}/api/v1"))
        .build()
        .unwrap();
    let driver = BoatDriver::connect(boat.clone()).await.unwrap();
    let caps = driver.capabilities();
    let files = oa_computers::files::spawn(store.clone(), driver.clone());
    let turns = Secrets::default();
    let app = oa_computers::app_with_files(store.clone(), caps, TOKEN, turns.clone(), Some(files));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://127.0.0.1:{port}");
    let local = tokio::task::LocalSet::new();
    local.spawn_local(
        Rc::new(Reconciler {
            store: store.clone(),
            driver,
            settings: fast("t-files"),
            turns,
        })
        .run(),
    );
    local.run_until(async {
        let (status,made)=call(reqwest::Method::POST,format!("{base}/v1/computers"),"acct-files",Some(json!({"idempotency_key":"files-one"}))).await;
        assert_eq!(status,202,"{made}");let id=made["id"].as_str().unwrap();
        wait_for(&base,"acct-files",id,|v|v["state"]=="awake").await;
        let bytes:Vec<u8>=(0..=255).cycle().take(9001).collect();let checksum=digest(&bytes);
        let file_url=format!("{base}/v1/computers/{id}/files?path=input.bin");
        let put=|account:&str,bytes:Vec<u8>,checksum:&str|http().put(&file_url).bearer_auth(TOKEN).header("x-openagents-account",account).header("content-type","application/octet-stream").header("x-content-sha256",checksum).body(bytes);
        let reply=put("acct-files",bytes.clone(),&checksum).send().await.unwrap();assert_eq!(reply.status(),200);assert_eq!(reply.json::<Value>().await.unwrap()["sha256"],checksum);
        let get=|account:&str|http().get(&file_url).bearer_auth(TOKEN).header("x-openagents-account",account);
        let r=get("acct-files").send().await.unwrap();assert_eq!(r.status(),200);assert_eq!(r.headers()["x-content-sha256"],checksum);assert_eq!(r.bytes().await.unwrap().as_ref(),bytes);
        assert_eq!(put("acct-other",b"bad".to_vec(),&digest(b"bad")).send().await.unwrap().status(),404);
        assert_eq!(get("acct-other").send().await.unwrap().status(),404);
        assert_eq!(put("acct-files",bytes.clone(),&checksum).send().await.unwrap().status(),200);
        assert_eq!(put("acct-files",b"bad".to_vec(),&digest(b"bad")).send().await.unwrap().status(),412);
        assert_eq!(put("acct-files",b"bad".to_vec(),&checksum).send().await.unwrap().status(),422);
        for path in ["../outside","%2e%2e/outside","%FF","input.bin&token=secret"] {
            let url=format!("{base}/v1/computers/{id}/files?path={path}");
            assert_eq!(http().get(url).bearer_auth(TOKEN).header("x-openagents-account","acct-files").send().await.unwrap().status(),400);
        }
        // A declared over-limit body is rejected before any body byte is sent.
        let mut socket=tokio::net::TcpStream::connect(("127.0.0.1",port)).await.unwrap();
        socket.write_all(format!("PUT /v1/computers/{id}/files?path=too-big HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\nX-OpenAgents-Account: acct-files\r\nContent-Type: application/octet-stream\r\nX-Content-SHA256: {checksum}\r\nContent-Length: {}\r\n\r\n",MAX_BYTES+1).as_bytes()).await.unwrap();
        let mut reply=[0u8;2048];let n=tokio::time::timeout(Duration::from_secs(3),socket.read(&mut reply)).await.unwrap().unwrap();assert!(std::str::from_utf8(&reply[..n]).unwrap().starts_with("HTTP/1.1 413"));drop(socket);
        // Two unfinished uploads hold both slots; a third transfer is refused.
        let mut waiting=Vec::new();
        for _ in 0..2 {
            let mut s=tokio::net::TcpStream::connect(("127.0.0.1",port)).await.unwrap();
            s.write_all(format!("PUT /v1/computers/{id}/files?path=pending HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\nX-OpenAgents-Account: acct-files\r\nContent-Type: application/octet-stream\r\nX-Content-SHA256: {}\r\nContent-Length: 1\r\n\r\n",digest(b"x")).as_bytes()).await.unwrap();waiting.push(s);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(get("acct-files").send().await.unwrap().status(),429);drop(waiting);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(get("acct-files").send().await.unwrap().bytes().await.unwrap().as_ref(),bytes);
        call(reqwest::Method::POST,format!("{base}/v1/computers/{id}/stop"),"acct-files",None).await;
        wait_for(&base,"acct-files",id,|v|v["state"]=="stopped").await;
        assert_eq!(get("acct-files").send().await.unwrap().status(),409);
        assert_eq!(put("acct-files",bytes.clone(),&checksum).send().await.unwrap().status(),409);
        let (_,still)=call(reqwest::Method::GET,format!("{base}/v1/computers/{id}"),"acct-files",None).await;assert_eq!(still["desired"],"stopped");
        call(reqwest::Method::POST,format!("{base}/v1/computers/{id}/wake"),"acct-files",None).await;wait_for(&base,"acct-files",id,|v|v["state"]=="awake").await;
        assert_eq!(get("acct-files").send().await.unwrap().bytes().await.unwrap().as_ref(),bytes);
        call(reqwest::Method::DELETE,format!("{base}/v1/computers/{id}"),"acct-files",None).await;wait_for(&base,"acct-files",id,|v|v["state"]=="deleted").await;
        assert_eq!(get("acct-files").send().await.unwrap().status(),404);
        assert_eq!(fake.0.lock().unwrap().instances.len(),0);
    }).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn public_command_credentials_are_scoped_ephemeral_and_never_replayed() {
    let Some(admin) = admin() else {
        return;
    };
    let (db, _) = Db::scratch(&admin).await.unwrap();
    let store = Store::new(db);
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeDriver::new(dir.path());
    let secrets = Secrets::default();
    let base = api_with(store.clone(), fake.capabilities(), secrets.clone()).await;
    let local = tokio::task::LocalSet::new();
    local.spawn_local(
        Rc::new(Reconciler {
            store: store.clone(),
            driver: fake.clone(),
            settings: fast("credentials"),
            turns: secrets,
        })
        .run(),
    );
    local.run_until(async {
        let (_, computer) = call(reqwest::Method::POST,format!("{base}/v1/computers"),"acct-a",Some(json!({"idempotency_key":"credentials-computer"}))).await;
        let id = computer["id"].as_str().unwrap();
        let up = wait_for(&base,"acct-a",id,|v|v["state"]=="awake").await;
        let command = "test -n \"$OA_TEST_CREDENTIAL\" || exit 1; echo once >> credential-counter; echo finished";
        let prepare = json!({"reference":"cred_one","request":"credential-command","command_sha256":computer_files::digest(command.as_bytes()),"values":{"OA_TEST_CREDENTIAL":"synthetic-private-value"}});
        let private_path = format!("{base}/v1/computers/{id}/command-credentials");
        assert_eq!(call(reqwest::Method::POST,private_path.clone(),"acct-b",Some(prepare.clone())).await.0,404);
        assert_eq!(call(reqwest::Method::POST,private_path,"acct-a",Some(prepare)).await.0,201);
        let request = json!({"command":command,"credential_ref":"cred_one","idempotency_key":"credential-command"});
        let path = format!("{base}/v1/computers/{id}/commands");
        fake.inject("start_command",computer_driver::fake::Inject::LoseReply);
        let (status, accepted) = call(reqwest::Method::POST,path.clone(),"acct-a",Some(request.clone())).await;
        assert_eq!(status,202);
        let turn = accepted["id"].as_str().unwrap();
        let result = wait_command(&base,id,turn,terminal_command).await;
        assert_eq!(result["state"],"succeeded","{result}");
        let saved = store.command("acct-a",id,turn).await.unwrap().unwrap();
        assert!(!saved.options.to_string().contains("synthetic-private-value"));
        assert_eq!(call(reqwest::Method::POST,path.clone(),"acct-a",Some(request)).await.1["id"],turn);
        let missing = json!({"command":"echo must-not-run >> credential-counter","credential_ref":"cred_lost_on_restart","idempotency_key":"missing-credentials"});
        let (_, accepted) = call(reqwest::Method::POST,path,"acct-a",Some(missing)).await;
        assert_eq!(wait_command(&base,id,accepted["id"].as_str().unwrap(),terminal_command).await["state"],"failed");
        assert_eq!(fake.calls().iter().filter(|s|s.starts_with("start_command:")).count(),1);
        let resource = up["resource"].as_str().unwrap();
        assert_eq!(std::fs::read_to_string(dir.path().join(resource).join("credential-counter")).unwrap(),"once\n");
        call(reqwest::Method::DELETE,format!("{base}/v1/computers/{id}"),"acct-a",None).await;
        wait_for(&base,"acct-a",id,|v|v["state"]=="deleted").await;
    }).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wake_during_command_cleanup_keeps_its_new_intent_after_restart() {
    let Some(admin) = admin() else {
        return;
    };
    let (db, _) = Db::scratch(&admin).await.unwrap();
    let store = Store::new(db.clone());
    let dir = tempfile::tempdir().unwrap();
    let base = api(store.clone(), FakeDriver::new(dir.path()).capabilities()).await;
    let (_, made) = call(
        reqwest::Method::POST,
        format!("{base}/v1/computers"),
        "acct-race",
        Some(json!({"idempotency_key":"wake-race"})),
    )
    .await;
    let id = made["id"].as_str().unwrap();
    let (_, command) = call(
        reqwest::Method::POST,
        format!("{base}/v1/computers/{id}/commands"),
        "acct-race",
        Some(json!({"command":"true","idempotency_key":"one-command"})),
    )
    .await;
    assert!(command["id"].as_str().unwrap().starts_with("trn_"));
    let finishing = store.get("acct-race", id).await.unwrap().unwrap();
    assert_eq!(
        call(
            reqwest::Method::POST,
            format!("{base}/v1/computers/{id}/stop"),
            "acct-race",
            None
        )
        .await
        .0,
        202
    );
    assert_eq!(
        call(
            reqwest::Method::POST,
            format!("{base}/v1/computers/{id}/wake"),
            "acct-race",
            None
        )
        .await
        .0,
        202
    );
    let restarted = Store::new(db);
    use oa_computers::store::Desired;
    assert!(
        !restarted
            .settle_desired(
                id,
                finishing.intent_generation,
                Desired::Running,
                Desired::Stopped
            )
            .await
            .unwrap()
    );
    let woken = restarted.get("acct-race", id).await.unwrap().unwrap();
    assert_eq!(woken.desired, Desired::Running);
    assert!(woken.intent_generation > finishing.intent_generation);
    let (_, next) = call(
        reqwest::Method::POST,
        format!("{base}/v1/computers/{id}/commands"),
        "acct-race",
        Some(json!({"command":"echo next","idempotency_key":"next-command"})),
    )
    .await;
    assert!(next["id"].is_string());
    assert!(
        !restarted
            .settle_desired(
                id,
                woken.intent_generation,
                Desired::Running,
                Desired::Stopped
            )
            .await
            .unwrap()
    );
    let latest = restarted.get("acct-race", id).await.unwrap().unwrap();
    assert!(
        restarted
            .settle_desired(
                id,
                latest.intent_generation,
                Desired::Running,
                Desired::Stopped
            )
            .await
            .unwrap()
    );
    assert_eq!(
        restarted
            .get("acct-race", id)
            .await
            .unwrap()
            .unwrap()
            .desired,
        Desired::Stopped
    );
    // Repeating the automatic completion cannot affect a later wake.
    assert_eq!(
        call(
            reqwest::Method::POST,
            format!("{base}/v1/computers/{id}/wake"),
            "acct-race",
            None
        )
        .await
        .0,
        202
    );
    assert!(
        !restarted
            .settle_desired(
                id,
                latest.intent_generation,
                Desired::Running,
                Desired::Stopped
            )
            .await
            .unwrap()
    );
    assert_eq!(
        restarted
            .get("acct-race", id)
            .await
            .unwrap()
            .unwrap()
            .desired,
        Desired::Running
    );
}
