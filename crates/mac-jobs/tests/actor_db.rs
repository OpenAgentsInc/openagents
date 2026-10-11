//! The `mac.job` actor against a scratch PostgreSQL (`ACTORS_TEST_DATABASE_URL`):
//! submit and retry, the Mac's fenced reports, single-use approvals and a
//! denial that sticks, cancel, and a Mac killed mid-job (a separate process,
//! SIGKILL) recovered by fenced expiry.
use actors::{postgres::PgStore, *};
use mac_jobs::actor::{self, Heard};
use mac_jobs::{Recipe, Spec};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

fn dsn() -> Option<String> {
    let dsn = std::env::var("ACTORS_TEST_DATABASE_URL").ok();
    if dsn.is_none() {
        eprintln!("Skipping: ACTORS_TEST_DATABASE_URL is unset.");
    }
    dsn
}

fn unique(prefix: &str) -> String {
    let mut nonce = [0; 8];
    getrandom::fill(&mut nonce).unwrap();
    format!("{prefix}-{:x}", u64::from_le_bytes(nonce))
}

const ACCOUNT: &str = "acct_test";
const COMPUTER: &str = "Chris's Mac";

/// The website, acting for the account (submit, approve, deny).
fn host(ws: &str) -> Caller {
    Caller {
        principal: format!("account:{ACCOUNT}"),
        workspace_id: ws.into(),
        account_id: Some(ACCOUNT.into()),
        role: Role::Service,
        executor: None,
    }
}

/// The owner through the API (cancel, view).
fn owner(ws: &str) -> Caller {
    Caller {
        role: Role::Owner,
        ..host(ws)
    }
}

/// The linked Mac: the owner's sign-in with the Mac's executor grant.
fn mac(ws: &str) -> Caller {
    Caller {
        executor: Some(ExecutorGrant {
            id: actor::target(COMPUTER),
            queues: vec![actor::QUEUE.into()],
            targets: vec![actor::target(COMPUTER)],
            generation: 1,
            expires_at: i64::MAX / 2,
            max_claims: 1,
        }),
        ..owner(ws)
    }
}

fn store(dsn: &str) -> PgStore {
    PgStore::new(
        Pool::new(dsn, 4).unwrap(),
        Arc::new(actor::registry().unwrap()),
    )
}

fn job(ws: &str, id: &str) -> ActorId {
    ActorId {
        workspace_id: ws.into(),
        actor_type: actor::TYPE.into(),
        key: id.into(),
    }
}

fn request(
    ws: &str,
    id: &str,
    message: &str,
    args: Value,
    input: Option<Value>,
    key: Option<&str>,
    fence: Option<WorkFence>,
) -> ActionRequest {
    ActionRequest {
        id: job(ws, id),
        message: Envelope {
            name: message.into(),
            args,
            origin: Origin::Action,
        },
        input,
        idempotency_key: key.map(str::to_owned),
        expected_version: None,
        fence,
    }
}

async fn submit(
    store: &PgStore,
    ws: &str,
    recipe: Recipe,
    args: &[&str],
    key: &str,
) -> Result<(String, ActionReply)> {
    let id = actor::job_id(ACCOUNT, key);
    let spec = Spec {
        repo: "OpenAgentsInc/openagents".into(),
        git_ref: "main".into(),
        recipe,
        args: args.iter().map(|a| (*a).to_owned()).collect(),
    };
    let input = json!({"id": id, "computer": COMPUTER, "spec": spec, "lease_ms": 4000});
    let reply = store
        .call(
            &host(ws),
            request(ws, &id, "submit@1", json!({}), Some(input), Some(key), None),
        )
        .await?;
    Ok((id, reply))
}

async fn view(store: &PgStore, ws: &str, id: &str) -> Value {
    store.view(&owner(ws), &job(ws, id)).await.unwrap().view
}

async fn report(
    store: &PgStore,
    ws: &str,
    id: &str,
    claim: &ClaimedWork,
    report: Value,
) -> Result<Heard> {
    let reply = store
        .call(
            &mac(ws),
            request(
                ws,
                id,
                "report@1",
                report,
                None,
                None,
                Some(WorkFence {
                    item_id: claim.item_id.clone(),
                    epoch: claim.epoch,
                }),
            ),
        )
        .await?;
    Ok(serde_json::from_value(reply.reply).unwrap())
}

async fn claim(store: &PgStore, ws: &str) -> Vec<ClaimedWork> {
    store
        .claim_work_wait(
            &mac(ws),
            actor::QUEUE,
            Some(&actor::target(COMPUTER)),
            1,
            Duration::from_secs(2),
        )
        .await
        .unwrap()
}

/// Run the runtime's steps until `done` holds of the job's view.
async fn settle(store: &PgStore, ws: &str, id: &str, done: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        store.fire_alarms(16).await.unwrap();
        store.expire_work(16).await.unwrap();
        store.dispatch_once(16).await.unwrap();
        let seen = view(store, ws, id).await;
        if done(&seen) {
            return seen;
        }
        assert!(Instant::now() < deadline, "never settled: {seen}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_release_gate_runs_once_and_a_retry_names_the_same_run() {
    let Some(dsn) = dsn() else { return };
    let store = store(&dsn);
    store.migrate().await.unwrap();
    let ws = unique("macjob-gate");
    let (id, first) = submit(&store, &ws, Recipe::IosReleaseGate, &[], "request-1")
        .await
        .unwrap();
    // The same request again is the same job: no second run, no second item.
    let (again, replay) = submit(&store, &ws, Recipe::IosReleaseGate, &[], "request-1")
        .await
        .unwrap();
    assert_eq!(again, id);
    assert!(replay.replayed && replay.reply == first.reply);
    assert_eq!(
        store
            .list_own(&owner(&ws), actor::TYPE, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    // The same key with another body is refused, not a new job.
    assert!(
        submit(&store, &ws, Recipe::DesktopCapture, &[], "request-1")
            .await
            .is_err()
    );
    let claims = claim(&store, &ws).await;
    assert_eq!(claims.len(), 1);
    let again = store
        .claim_work(&mac(&ws), actor::QUEUE, Some(&actor::target(COMPUTER)), 1)
        .await
        .unwrap();
    assert!(again.is_empty(), "claimed once");
    let work = &claims[0];
    let offered: actor::Offered = serde_json::from_value(work.payload.clone()).unwrap();
    assert_eq!(offered.id, id);
    // Only the claim reports: the owner can't, nor a wrong epoch.
    assert!(
        store
            .call(
                &owner(&ws),
                request(
                    &ws,
                    &id,
                    "report@1",
                    json!({"lines":["x"]}),
                    None,
                    None,
                    Some(WorkFence {
                        item_id: work.item_id.clone(),
                        epoch: work.epoch
                    })
                ),
            )
            .await
            .is_err()
    );
    let mut wrong = work.clone();
    wrong.epoch += 1;
    assert!(
        report(&store, &ws, &id, &wrong, json!({"lines":["x"]}))
            .await
            .is_err()
    );
    let heard = report(
        &store,
        &ws,
        &id,
        work,
        json!({"lines":["Checking out.", "token sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789"], "commit": "0123456789abcdef"}),
    )
    .await
    .unwrap();
    assert_eq!(heard, Heard::default());
    let seen = view(&store, &ws, &id).await;
    assert_eq!(seen["state"], "running");
    assert_eq!(seen["commit"], "0123456789abcdef");
    assert!(
        !seen["lines"].to_string().contains("abcdefghijklmnop"),
        "redacted"
    );
    // A file, in parts, fenced by the claim.
    for (part, last) in [(0, false), (1, true)] {
        store
            .call(
                &mac(&ws),
                request(
                    &ws,
                    &id,
                    "artifact@1",
                    json!({"name":"summary.json","part":part,"size":10,"last":last}),
                    None,
                    None,
                    Some(WorkFence {
                        item_id: work.item_id.clone(),
                        epoch: work.epoch,
                    }),
                ),
            )
            .await
            .unwrap();
    }
    store
        .finish_work(
            &mac(&ws),
            &work.uid,
            &work.item_id,
            work.epoch,
            actor::outcome_done("Passed 12 tests."),
        )
        .await
        .unwrap();
    let seen = settle(&store, &ws, &id, |v| v["state"] == "done").await;
    assert_eq!(seen["summary"], "Passed 12 tests.");
    assert_eq!(seen["artifacts"][0]["size"], 20);
    assert_eq!(seen["artifacts"][0]["done"], true);
    // History records each transition for operators.
    let admin = Caller {
        role: Role::Admin,
        principal: "operator".into(),
        ..owner(&ws)
    };
    assert!(
        store
            .history(&admin, &job(&ws, &id), 0, 50)
            .await
            .unwrap()
            .len()
            >= 5
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_upload_waits_for_the_owner_answers_are_single_use_and_a_denial_sticks() {
    let Some(dsn) = dsn() else { return };
    let store = store(&dsn);
    store.migrate().await.unwrap();
    let ws = unique("macjob-upload");
    let (id, _) = submit(
        &store,
        &ws,
        Recipe::IosTestflight,
        &["--validate-only"],
        "upload-1",
    )
    .await
    .unwrap();
    let work = claim(&store, &ws).await.remove(0);
    let ask =
        json!({"id":"upload-0123456789ab","text":"Upload?","subject":"o/r@0123 ios-testflight"});
    let heard = report(&store, &ws, &id, &work, json!({"ask": ask}))
        .await
        .unwrap();
    assert_eq!(heard.approval, None);
    assert_eq!(view(&store, &ws, &id).await["state"], "asking");
    // Nobody but the host's own pages answers.
    for caller in [owner(&ws), mac(&ws)] {
        assert!(
            store
                .call(
                    &caller,
                    request(
                        &ws,
                        &id,
                        "approve@1",
                        json!({"question":"upload-0123456789ab","via":"web"}),
                        None,
                        None,
                        None
                    )
                )
                .await
                .is_err()
        );
    }
    let answer = |message: &'static str, question: &'static str| {
        let store = store.clone();
        let ws = ws.clone();
        let id = id.clone();
        async move {
            store
                .call(
                    &host(&ws),
                    request(
                        &ws,
                        &id,
                        message,
                        json!({"question": question, "via": "phone"}),
                        None,
                        None,
                        None,
                    ),
                )
                .await
                .unwrap()
                .reply
        }
    };
    assert_eq!(
        answer("approve@1", "some-other-question").await,
        "not_asking"
    );
    assert_eq!(answer("deny@1", "upload-0123456789ab").await, "recorded");
    assert_eq!(
        answer("approve@1", "upload-0123456789ab").await,
        "not_asking",
        "denial sticks"
    );
    let heard = report(&store, &ws, &id, &work, json!({"ask": ask}))
        .await
        .unwrap();
    assert_eq!(heard.approval.as_ref().unwrap().decision, "denied");
    // Handed once; a new question gets the same denial, never a new ask.
    let heard = report(&store, &ws, &id, &work, json!({})).await.unwrap();
    assert_eq!(heard.approval, None);
    let again = json!({"id":"upload-second","text":"Upload?","subject":"o/r@0123 ios-testflight"});
    let heard = report(&store, &ws, &id, &work, json!({"ask": again}))
        .await
        .unwrap();
    assert_eq!(heard.approval.unwrap().decision, "denied");
    assert_ne!(view(&store, &ws, &id).await["state"], "asking");
    store
        .finish_work(
            &mac(&ws),
            &work.uid,
            &work.item_id,
            work.epoch,
            actor::outcome_failed("You denied the upload, so nothing was sent."),
        )
        .await
        .unwrap();
    settle(&store, &ws, &id, |v| v["state"] == "failed").await;

    // An approval goes to the epoch that asked, once.
    let (id, _) = submit(&store, &ws, Recipe::IosTestflight, &[], "upload-2")
        .await
        .unwrap();
    let work = claim(&store, &ws).await.remove(0);
    report(&store, &ws, &id, &work, json!({"ask": ask}))
        .await
        .unwrap();
    let approved = store
        .call(
            &host(&ws),
            request(
                &ws,
                &id,
                "approve@1",
                json!({"question":"upload-0123456789ab","via":"web"}),
                None,
                None,
                None,
            ),
        )
        .await
        .unwrap();
    assert_eq!(approved.reply, "recorded");
    let heard = report(&store, &ws, &id, &work, json!({"ask": ask}))
        .await
        .unwrap();
    assert_eq!(heard.approval.unwrap().decision, "approved");
    let heard = report(&store, &ws, &id, &work, json!({"ask": ask}))
        .await
        .unwrap();
    assert_eq!(heard.approval, None, "single use");
    // The Mac goes quiet mid-upload: never repeated on its own.
    let seen = settle(&store, &ws, &id, |v| v["state"] == "uncertain").await;
    assert!(seen["why"].as_str().unwrap().contains("operator"));
    assert!(
        report(&store, &ws, &id, &work, json!({})).await.is_err(),
        "fenced"
    );
    assert!(
        claim(&store, &ws).await.is_empty(),
        "an uncertain upload holds the Mac's slot"
    );
    // An operator records what happened; the slot frees.
    let admin = Caller {
        role: Role::Admin,
        principal: "operator".into(),
        ..owner(&ws)
    };
    let row = store.inspect(&admin, &job(&ws, &id)).await.unwrap();
    let epoch = work.epoch + 1;
    store
        .resolve_work(
            &admin,
            &row.uid,
            &work.item_id,
            epoch,
            false,
            Some(actor::outcome_done(
                "Checked in App Store Connect: uploaded.",
            )),
        )
        .await
        .unwrap();
    let seen = settle(&store, &ws, &id, |v| v["state"] == "done").await;
    assert!(
        seen["summary"]
            .as_str()
            .unwrap()
            .starts_with("Recorded by an operator")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_reaches_the_mac_and_its_release_frees_the_slot() {
    let Some(dsn) = dsn() else { return };
    let store = store(&dsn);
    store.migrate().await.unwrap();
    let ws = unique("macjob-cancel");
    // Waiting: cancelled at once.
    let (waiting, _) = submit(&store, &ws, Recipe::DesktopCapture, &[], "c-1")
        .await
        .unwrap();
    let cancelled = store
        .call(
            &owner(&ws),
            request(&ws, &waiting, "cancel@1", json!({}), None, None, None),
        )
        .await
        .unwrap();
    assert_eq!(cancelled.reply, true);
    assert_eq!(view(&store, &ws, &waiting).await["state"], "cancelled");
    assert!(claim(&store, &ws).await.is_empty());
    // Running: the Mac hears it, stops, and releases.
    let (id, _) = submit(&store, &ws, Recipe::DesktopCapture, &[], "c-2")
        .await
        .unwrap();
    let work = claim(&store, &ws).await.remove(0);
    report(&store, &ws, &id, &work, json!({"lines":["Step: capture"]}))
        .await
        .unwrap();
    store
        .call(
            &owner(&ws),
            request(&ws, &id, "cancel@1", json!({}), None, None, None),
        )
        .await
        .unwrap();
    let heard = report(&store, &ws, &id, &work, json!({"lines":["stopping"]}))
        .await
        .unwrap();
    assert!(heard.cancel);
    store
        .release_work(&mac(&ws), &work.uid, &work.item_id, work.epoch, "cancelled")
        .await
        .unwrap();
    settle(&store, &ws, &id, |v| v["state"] == "cancelled").await;
    let (next, _) = submit(&store, &ws, Recipe::DesktopCapture, &[], "c-3")
        .await
        .unwrap();
    let work = claim(&store, &ws).await;
    assert_eq!(work.len(), 1, "the slot is free again");
    let payload: actor::Offered = serde_json::from_value(work[0].payload.clone()).unwrap();
    assert_eq!(payload.id, next);
}

/// The Mac's side, in its own process: claim, report, and keep reporting
/// until killed. Runs only when the crash test starts it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn child_mac_process() {
    let (Ok(ws), Some(dsn)) = (std::env::var("MAC_JOBS_CHILD_WS"), dsn()) else {
        return;
    };
    let store = store(&dsn);
    let work = claim(&store, &ws).await;
    let Some(work) = work.into_iter().next() else {
        println!("CLAIM null");
        return;
    };
    let offered: actor::Offered = serde_json::from_value(work.payload.clone()).unwrap();
    println!(
        "CLAIM {}",
        json!({"id": offered.id, "uid": work.uid, "item": work.item_id, "epoch": work.epoch})
    );
    let mut n = 0;
    loop {
        n += 1;
        let _ = report(
            &store,
            &ws,
            &offered.id,
            &work,
            json!({"lines":[format!("step {n}")]}),
        )
        .await;
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

fn spawn_mac(dsn: &str, ws: &str) -> (std::process::Child, Value) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "child_mac_process",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("MAC_JOBS_CHILD_WS", ws)
        .env("ACTORS_TEST_DATABASE_URL", dsn)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        assert!(
            reader.read_line(&mut line).unwrap() > 0,
            "the Mac process ended"
        );
        // The harness's own "test NAME ... " shares the line.
        if let Some(claim) = line.find("CLAIM ").map(|at| line[at + 6..].trim()) {
            let claim: Value = serde_json::from_str(claim).unwrap();
            std::thread::spawn(move || for _ in reader.lines() {});
            return (child, claim);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_mac_killed_mid_job_is_fenced_and_the_job_runs_again_once() {
    if std::env::var("MAC_JOBS_CHILD_WS").is_ok() {
        return;
    }
    let Some(dsn) = dsn() else { return };
    let store = store(&dsn);
    store.migrate().await.unwrap();
    let ws = unique("macjob-crash");
    let (id, _) = submit(&store, &ws, Recipe::IosReleaseGate, &[], "crash-1")
        .await
        .unwrap();
    let (mut first, claim_one) = spawn_mac(&dsn, &ws);
    assert_eq!(claim_one["id"], id.as_str());
    // Alive, its reports keep the 4 s lease going.
    tokio::time::sleep(Duration::from_millis(5000)).await;
    let seen = view(&store, &ws, &id).await;
    assert_eq!(seen["state"], "running");
    assert!(seen["lines"].as_array().unwrap().len() >= 3);
    first.kill().unwrap();
    first.wait().unwrap();

    // The lease ends; the runtime fences the claim; the job waits again.
    let seen = settle(&store, &ws, &id, |v| v["state"] == "waiting").await;
    assert!(seen["lines"].to_string().contains("stopped answering"));
    // A restarted Mac takes it at a new epoch and finishes it.
    let (mut second, claim_two) = spawn_mac(&dsn, &ws);
    assert_eq!(claim_two["id"], id.as_str());
    assert!(claim_two["epoch"].as_u64() > claim_one["epoch"].as_u64());
    second.kill().unwrap();
    second.wait().unwrap();
    let old = ClaimedWork {
        actor: job(&ws, &id),
        uid: claim_one["uid"].as_str().unwrap().into(),
        item_id: claim_one["item"].as_str().unwrap().into(),
        queue: actor::QUEUE.into(),
        payload: Value::Null,
        epoch: claim_one["epoch"].as_u64().unwrap(),
        heartbeat_until: 0,
        cancel: false,
        attempts: 1,
    };
    let new = ClaimedWork {
        epoch: claim_two["epoch"].as_u64().unwrap(),
        ..old.clone()
    };
    // The dead process's epoch is refused everywhere.
    assert!(
        report(&store, &ws, &id, &old, json!({"lines":["late"]}))
            .await
            .is_err()
    );
    assert!(
        store
            .finish_work(
                &mac(&ws),
                &old.uid,
                &old.item_id,
                old.epoch,
                actor::outcome_done("old")
            )
            .await
            .is_err()
    );
    report(&store, &ws, &id, &new, json!({"lines":["again"]}))
        .await
        .unwrap();
    store
        .finish_work(
            &mac(&ws),
            &new.uid,
            &new.item_id,
            new.epoch,
            actor::outcome_done("Passed."),
        )
        .await
        .unwrap();
    let seen = settle(&store, &ws, &id, |v| v["state"] == "done").await;
    assert_eq!(seen["summary"], "Passed.");
    assert_eq!(
        store
            .list_own(&owner(&ws), actor::TYPE, 10)
            .await
            .unwrap()
            .len(),
        1
    );
}
