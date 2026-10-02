// The one test that reaches the real Boat API. It costs machine time, so it is
// ignored and also needs `OA_BOAT_LIVE=I_ACCEPT_BOAT_COST`:
//
//   OA_BOAT_LIVE=I_ACCEPT_BOAT_COST cargo test -p boat --test live -- --ignored --nocapture
//
// The key comes only from `BOAT_API_KEY` in the environment and is never
// printed. One `small` sandbox (2 vCPU, $0.018 an hour) runs for well under a
// minute; it is stopped and deleted whatever happens, and the test fails if it
// cost a cent or more or is still running.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use boat::{ApiKey, Client, CommandFrame, Nullable, Signal, WaitOptions, models::*};

const GATE: &str = "I_ACCEPT_BOAT_COST";
const ACTIVE: [&str; 5] = ["provisioned", "cloning", "ready", "idle", "running"];

fn wait(seconds: u64) -> WaitOptions {
    WaitOptions {
        timeout: Duration::from_secs(seconds),
        interval: Duration::from_secs(1),
        ..Default::default()
    }
}

/// Everything the sandbox does between ready and stop.
async fn exercise(client: &Client, id: &str) -> boat::Result<()> {
    client.wait_until_ready(id, &wait(180)).await?;

    let output = client
        .exec_stream(
            id,
            CommandRequest {
                command: "echo boat-live".into(),
                timeout_seconds: Some(30),
                ..Default::default()
            },
        )
        .await?
        .collect()
        .await?;
    assert_eq!(output.stdout, "boat-live\n");
    assert_eq!(output.exit_code(), Some(0));

    client
        .write_text(id, "/tmp/boat-live.txt", "round trip")
        .await?;
    assert_eq!(
        client.read_text(id, "/tmp/boat-live.txt").await?,
        "round trip"
    );

    // A detached command followed from a byte cursor.
    let process = client
        .exec_detached(
            id,
            CommandRequest {
                command: "printf 'one\\n'; printf 'two\\n' >&2; sleep 1; printf 'three\\n'".into(),
                ..Default::default()
            },
        )
        .await?;
    let followed = client
        .follow_command(id, process.process_id, wait(60))?
        .collect()
        .await?;
    assert_eq!(followed.stdout, "one\nthree\n");
    assert_eq!(followed.stderr, "two\n");
    assert!(matches!(
        followed.last,
        Some(CommandFrame::Exit {
            exit_code: Some(0),
            ..
        })
    ));

    // A detached process tree, killed.
    let sleeper = client
        .exec_detached(
            id,
            CommandRequest {
                command: "sh -c 'sleep 300' & sleep 300".into(),
                ..Default::default()
            },
        )
        .await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(client.kill_command(id, sleeper.pid, Signal::Term).await?);
    let status = client
        .wait_command(id, sleeper.process_id, &wait(30))
        .await?;
    assert!(!status.running);
    let leftover = client
        .exec_stream(
            id,
            CommandRequest {
                command: "pgrep -c -f 'slee[p] 300' || true".into(),
                timeout_seconds: Some(30),
                ..Default::default()
            },
        )
        .await?
        .collect()
        .await?;
    assert_eq!(
        leftover.stdout.trim(),
        "0",
        "the killed tree left a process"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "creates a billable Boat sandbox; needs OA_BOAT_LIVE=I_ACCEPT_BOAT_COST and BOAT_API_KEY"]
async fn a_small_sandbox_runs_echo_and_stops_for_under_a_cent() {
    if std::env::var("OA_BOAT_LIVE").as_deref() != Ok(GATE) {
        eprintln!("skipped: set OA_BOAT_LIVE={GATE} to run the live Boat test");
        return;
    }
    let key = ApiKey::new(std::env::var("BOAT_API_KEY").expect("BOAT_API_KEY in the environment"))
        .expect("a non-empty BOAT_API_KEY");
    let client = Client::builder(key).build().expect("client");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let created = client
        .create(&CreateParams {
            idempotency_key: Some(format!("oa-boat-live-{nonce}")),
            body: Some(CreateSandboxRequest {
                type_: Some("small".into()),
                ttl_seconds: Nullable::Value(600),
                no_env: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await
        .expect("create a small sandbox");
    let id = created.sandbox.id;
    eprintln!("live: sandbox {id}");

    let result = exercise(&client, &id).await;

    // Teardown runs whatever happened above.
    let stopped = client
        .stop(&StopParams {
            sandbox_id: id.clone(),
            ..Default::default()
        })
        .await;
    let mut state = String::new();
    for _ in 0..60 {
        state = client
            .get(&GetParams {
                sandbox_id: id.clone(),
                ..Default::default()
            })
            .await
            .map(|info| info.sandbox.state)
            .unwrap_or_default();
        if !ACTIVE.contains(&state.as_str()) && !state.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    let usage = client
        .usage(&UsageParams {
            sandbox_id: id.clone(),
            ..Default::default()
        })
        .await;
    let deleted = client
        .delete_sandbox(&DeleteSandboxParams {
            sandbox_id: id.clone(),
            x_ascii_confirm_delete: id.clone(),
            ..Default::default()
        })
        .await;
    if let Ok(deleted) = &deleted {
        let _ = client
            .wait_for_deletion(&deleted.operation.id, &wait(120))
            .await;
    }

    result.expect("the sandbox exercise");
    stopped.expect("stop");
    assert!(
        !ACTIVE.contains(&state.as_str()),
        "the sandbox is still {state} after stop"
    );
    let usage = usage.expect("usage");
    eprintln!(
        "live: {} billable seconds, ${:.6} at list price, final state {state}",
        usage.seconds, usage.dollars
    );
    assert!(!usage.running, "usage says the sandbox is still running");
    assert!(
        usage.dollars < 0.01,
        "cost ${} is a cent or more",
        usage.dollars
    );
    deleted.expect("delete");
}
