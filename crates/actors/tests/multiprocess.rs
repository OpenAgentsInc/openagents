//! Crash and restart across processes: an executor and the runtime each run
//! as their own process (`actors-crash`) and are killed with SIGKILL in the
//! middle of their work. Runs only with `ACTORS_TEST_DATABASE_URL` set to a
//! scratch database.
use actors::{example, postgres::PgStore, *};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

const BIN: &str = env!("CARGO_BIN_EXE_actors-crash");

fn dsn() -> Option<String> {
    match std::env::var("ACTORS_TEST_DATABASE_URL") {
        Ok(dsn) => Some(dsn),
        Err(_) => {
            eprintln!("Skipping multi-process test: ACTORS_TEST_DATABASE_URL is unset.");
            None
        }
    }
}

fn unique(prefix: &str) -> String {
    let mut nonce = [0; 8];
    getrandom::fill(&mut nonce).unwrap();
    format!("{prefix}-{:x}", u64::from_le_bytes(nonce))
}

fn store(dsn: &str) -> (PgStore, Pool) {
    let pool = Pool::new(dsn, 4).unwrap();
    (
        PgStore::new(pool.clone(), Arc::new(example::registry().unwrap())),
        pool,
    )
}

fn owner(ws: &str) -> Caller {
    Caller {
        principal: "account:crash-owner".into(),
        workspace_id: ws.into(),
        account_id: Some("crash-owner".into()),
        role: Role::Owner,
        executor: None,
    }
}

fn admin(ws: &str) -> Caller {
    Caller {
        role: Role::Admin,
        principal: "operator".into(),
        ..owner(ws)
    }
}

fn call(
    id: &ActorId,
    message: &str,
    args: Value,
    input: Option<Value>,
    key: &str,
) -> ActionRequest {
    ActionRequest {
        id: id.clone(),
        message: Envelope {
            name: message.into(),
            args,
            origin: Origin::Action,
        },
        input,
        idempotency_key: Some(key.into()),
        expected_version: None,
        fence: None,
    }
}

/// An `actors-crash executor` process, its claim read from its first line.
fn spawn_executor(dsn: &str, ws: &str, executor: &str) -> (Child, Value) {
    let mut child = Command::new(BIN)
        .args(["executor", ws, "crash-owner", executor, "jobs", "computer"])
        .env("ACTORS_TEST_DATABASE_URL", dsn)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    (child, serde_json::from_str(&line).unwrap())
}

fn run_runtime(dsn: &str, millis: u64) -> Value {
    let output = Command::new(BIN)
        .args(["runtime", &millis.to_string()])
        .env("ACTORS_TEST_DATABASE_URL", dsn)
        .output()
        .unwrap();
    assert!(output.status.success(), "runtime process failed");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn finish(dsn: &str, ws: &str, executor: &str, claim: &Value) -> String {
    let output = Command::new(BIN)
        .args([
            "finish",
            ws,
            "crash-owner",
            executor,
            "jobs",
            "computer",
            claim["uid"].as_str().unwrap(),
            claim["item"].as_str().unwrap(),
            &claim["epoch"].to_string(),
        ])
        .env("ACTORS_TEST_DATABASE_URL", dsn)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn kill(child: &mut Child) {
    // SIGKILL: no destructor, no release, no last heartbeat.
    child.kill().unwrap();
    child.wait().unwrap();
}

async fn work_row(pool: &Pool, uid: &str) -> (String, i64) {
    let row = pool
        .acquire()
        .await
        .unwrap()
        .query_one("SELECT state,epoch FROM actor.work WHERE uid=$1", &[&uid])
        .await
        .unwrap();
    (row.get(0), row.get(1))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_killed_executor_is_fenced_by_expiry_and_its_work_reclaimed_once() {
    let Some(dsn) = dsn() else { return };
    let (store, pool) = store(&dsn);
    store.migrate().await.unwrap();
    let ws = unique("crash-exec");
    let id = ActorId {
        workspace_id: ws.clone(),
        actor_type: "example.counter".into(),
        key: "job".into(),
    };
    // The example's offer leases for one second and must be reconciled.
    store
        .call(
            &owner(&ws),
            call(
                &id,
                "offer@1",
                json!({"queue":"jobs","target":"computer"}),
                Some(json!({})),
                "offer",
            ),
        )
        .await
        .unwrap();
    let (mut first, claim) = spawn_executor(&dsn, &ws, "mac-a");
    assert_eq!(claim["claimed"], true);
    let uid = claim["uid"].as_str().unwrap().to_owned();
    // The process keeps the lease alive while it runs.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(work_row(&pool, &uid).await.0, "claimed");
    kill(&mut first);

    // Another process's runtime fences the dead claim when its lease ends.
    let deadline = Instant::now() + Duration::from_secs(20);
    while work_row(&pool, &uid).await.0 == "claimed" {
        assert!(Instant::now() < deadline, "the claim never expired");
        run_runtime(&dsn, 800);
    }
    let (state, fenced_epoch) = work_row(&pool, &uid).await;
    assert_eq!(
        state, "uncertain",
        "non-repeatable work waits for a decision"
    );
    assert!(fenced_epoch > claim["epoch"].as_i64().unwrap());
    // The dead process's epoch can never complete, even if it comes back.
    assert_eq!(finish(&dsn, &ws, "mac-a", &claim), "stale_claim");

    // An operator decides to run it again; a fresh process claims it once.
    store
        .resolve_work(
            &admin(&ws),
            &uid,
            claim["item"].as_str().unwrap(),
            fenced_epoch as u64,
            true,
            None,
        )
        .await
        .unwrap();
    let (mut second, reclaimed) = spawn_executor(&dsn, &ws, "mac-a");
    assert_eq!(reclaimed["claimed"], true);
    assert!(reclaimed["epoch"].as_i64() > claim["epoch"].as_i64());
    assert_eq!(finish(&dsn, &ws, "mac-a", &claim), "stale_claim");
    assert_eq!(finish(&dsn, &ws, "mac-a", &reclaimed), "ok");
    assert_eq!(
        finish(&dsn, &ws, "mac-a", &reclaimed),
        "ok",
        "a retry is a duplicate"
    );
    kill(&mut second);

    run_runtime(&dsn, 1500);
    let view = store.view(&owner(&ws), &id).await.unwrap();
    assert_eq!(view.view["completed"], 1, "exactly one completion");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn killed_and_parallel_runtimes_deliver_every_queued_message_once() {
    let Some(dsn) = dsn() else { return };
    let (store, pool) = store(&dsn);
    store.migrate().await.unwrap();
    let ws = unique("crash-runtime");
    let id = ActorId {
        workspace_id: ws.clone(),
        actor_type: "example.counter".into(),
        key: "count".into(),
    };
    store
        .call(
            &owner(&ws),
            call(&id, "add@1", json!({"delta":0}), Some(json!({})), "made"),
        )
        .await
        .unwrap();
    const MESSAGES: i64 = 300;
    for n in 0..MESSAGES {
        store
            .enqueue(
                &owner(&ws),
                &id,
                Envelope {
                    name: "add@1".into(),
                    args: json!({"delta":1}),
                    origin: Origin::Inbox,
                },
                Some(&format!("message-{n}")),
            )
            .await
            .unwrap();
    }
    // Start one runtime and kill it mid-stream, more than once.
    for _ in 0..3 {
        let mut runtime = Command::new(BIN)
            .args(["runtime", "60000"])
            .env("ACTORS_TEST_DATABASE_URL", &dsn)
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        kill(&mut runtime);
    }
    // Then two at once until the inbox drains.
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let pending: i64 = pool
            .acquire()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM actor.inbox i JOIN actor.instances a USING(uid) \
                 WHERE a.workspace_id=$1 AND i.state='pending'",
                &[&ws],
            )
            .await
            .unwrap()
            .get(0);
        if pending == 0 {
            break;
        }
        assert!(Instant::now() < deadline, "the inbox never drained");
        let dsn_a = dsn.clone();
        let dsn_b = dsn.clone();
        let a = std::thread::spawn(move || run_runtime(&dsn_a, 1500));
        let b = std::thread::spawn(move || run_runtime(&dsn_b, 1500));
        a.join().unwrap();
        b.join().unwrap();
    }
    let view = store.view(&owner(&ws), &id).await.unwrap();
    assert_eq!(
        view.view["value"], MESSAGES,
        "each message applied exactly once"
    );
    let done: i64 = pool
        .acquire()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM actor.inbox i JOIN actor.instances a USING(uid) \
             WHERE a.workspace_id=$1 AND i.state='done'",
            &[&ws],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(done, MESSAGES);
}
