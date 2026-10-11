//! Opt-in paid lifecycle check. Default test runs never contact Boat.
mod support;

use boat::{ApiKey, Client, CommandFrame, Nullable, Signal, WaitOptions, models::*};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn wait(seconds: u64) -> WaitOptions {
    WaitOptions {
        timeout: Duration::from_secs(seconds),
        interval: Duration::from_secs(1),
        ..Default::default()
    }
}

const ACTIONS: &[&str] = &[
    "sandbox.create",
    "sandbox.read",
    "exec",
    "file.read",
    "file.write",
    "sandbox.stop",
    "sandbox.delete",
    "account.read",
];

fn validate_scope(usage: &ApiKeyUsageResponse) -> Result<(), &'static str> {
    if usage.credential_lane.as_deref() != Some("scoped-v1") || usage.expired != Some(false) {
        return Err("Use a current scoped key.");
    }
    let scope = usage.scope.as_ref().ok_or("Missing key scope.")?;
    if !scope
        .expires_at
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty())
    {
        return Err("Use an expiring key.");
    }
    let actions = scope
        .actions
        .as_ref()
        .ok_or("Missing action restrictions.")?;
    if actions.is_empty() || actions.iter().any(|a| !ACTIONS.contains(&a.as_str())) {
        return Err("The key permits unrelated actions.");
    }
    Ok(())
}

fn affordable(dollars: f64) -> bool {
    dollars.is_finite() && (0.0..0.01).contains(&dollars)
}

#[test]
fn cost_guard_rejects_invalid_and_one_cent_totals() {
    for cost in [f64::NAN, f64::INFINITY, -0.001, 0.01, 1.0] {
        assert!(!affordable(cost));
    }
    assert!(affordable(0.0));
    assert!(affordable(0.009));
}

#[test]
fn scope_guard_rejects_unrestricted_and_nonexpiring_keys() {
    let mut usage = ApiKeyUsageResponse {
        credential_lane: Some("scoped-v1".into()),
        expired: Some(false),
        scope: Some(ApiKeyUsageResponseScope {
            expires_at: Nullable::Value("2026-10-03T00:00:00Z".into()),
            actions: Some(ACTIONS.iter().map(|a| a.to_string()).collect()),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(validate_scope(&usage).is_ok());
    usage.scope.as_mut().unwrap().actions = Some(vec!["*".into()]);
    assert!(validate_scope(&usage).is_err());
    usage.scope.as_mut().unwrap().actions = Some(vec!["exec".into()]);
    usage.scope.as_mut().unwrap().expires_at = Nullable::Null;
    assert!(validate_scope(&usage).is_err());
    usage.expired = Some(true);
    assert!(validate_scope(&usage).is_err());
}

async fn exercise(client: &Client, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    client
        .wait_until_ready(
            id,
            &WaitOptions {
                timeout: Duration::from_secs(120),
                ..Default::default()
            },
        )
        .await?;
    let output = client
        .exec_stream(
            id,
            CommandRequest {
                command: "uname -a".into(),
                timeout_seconds: Some(10),
                ..Default::default()
            },
        )
        .await?
        .collect()
        .await?;
    if output.exit_code() != Some(0) || output.stdout.trim().is_empty() {
        return Err("The streamed command did not succeed.".into());
    }
    client
        .write_text(id, "/tmp/oa-boat-live.txt", "Boat SDK lifecycle check\n")
        .await?;
    if client.read_text(id, "/tmp/oa-boat-live.txt").await? != "Boat SDK lifecycle check\n" {
        return Err("The file did not round-trip.".into());
    }
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
    if followed.stdout != "one\nthree\n" {
        return Err("Unexpected followed stdout.".into());
    }
    if followed.stderr != "two\n" {
        return Err("Unexpected followed stderr.".into());
    }
    if !matches!(
        followed.last,
        Some(CommandFrame::Exit {
            exit_code: Some(0),
            ..
        })
    ) {
        return Err("The followed command did not exit successfully.".into());
    }

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
    if !client.kill_command(id, sleeper.pid, Signal::Term).await? {
        return Err("The process was not running.".into());
    }
    let status = client
        .wait_command(id, sleeper.process_id, &wait(30))
        .await?;
    if status.running {
        return Err("The process is still running.".into());
    }
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
    if leftover.stdout.trim() != "0" {
        return Err("The killed tree left a process.".into());
    }
    Ok(())
}

#[tokio::test]
#[ignore = "Requires explicit cost consent and a scoped, expiring Boat key."]
async fn paid_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("OA_BOAT_LIVE").as_deref() != Ok("I_ACCEPT_BOAT_COST") {
        return Err("Set OA_BOAT_LIVE=I_ACCEPT_BOAT_COST to run the paid check.".into());
    }
    // This check exercises hosted Boat's scoped keys: it needs BOAT_HOSTED=1.
    let client = Client::builder(ApiKey::new(std::env::var("BOAT_API_KEY")?)?)
        .base_url(boat::HOSTED_BASE_URL)
        .timeout(Duration::from_secs(20))
        .build()?;
    // Metadata reads require account authority; the exercised key remains scoped.
    let metadata = match std::env::var("OA_BOAT_LIVE_METADATA_KEY") {
        Ok(key) => Client::builder(ApiKey::new(key)?)
            .base_url(boat::HOSTED_BASE_URL)
            .timeout(Duration::from_secs(20))
            .build()?,
        Err(_) => client.clone(),
    };
    let key_id = std::env::var("OA_BOAT_LIVE_KEY_ID")?;
    let selected = metadata
        .api_keys()
        .await?
        .api_keys
        .into_iter()
        .find(|k| k.id == key_id)
        .ok_or("The selected scoped key is not owned by the metadata account.")?;
    let secret = std::env::var("BOAT_API_KEY")?;
    if !secret.starts_with(&selected.key_prefix) || !secret.ends_with(&selected.key_last_four) {
        return Err("The scope metadata does not identify the exercised credential.".into());
    }
    let usage = ApiKeyUsageResponse {
        credential_lane: selected.credential_lane,
        expired: selected.expired,
        scope: serde_json::from_value(serde_json::to_value(selected.scope)?)?,
        ..Default::default()
    };
    validate_scope(&usage)?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let created = client
        .create(&CreateParams {
            idempotency_key: Some(format!("oa-boat-live-{}-{nonce}", std::process::id())),
            body: Some(CreateSandboxRequest {
                type_: Some("small".into()),
                ttl_seconds: Nullable::Value(600),
                no_env: Some(true),
                snapshots: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await?;
    let id = created.sandbox.id;
    lifecycle(&client, &id).await
}

async fn lifecycle(client: &Client, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let id = id.to_owned();
    // Capture failures instead of returning before cleanup.
    let result = tokio::time::timeout(Duration::from_secs(150), exercise(client, &id)).await;
    let stopped = client
        .stop(&StopParams {
            sandbox_id: id.clone(),
            ..Default::default()
        })
        .await;
    let stopped = match stopped {
        Ok(response) => match response.sandbox {
            Nullable::Value(sandbox) => match sandbox.stop {
                Nullable::Value(operation) => client
                    .wait_for_stop(&id, &operation.id, &wait(120))
                    .await
                    .map(|_| ()),
                _ => Err(boat::Error::TerminalState),
            },
            _ => Err(boat::Error::TerminalState),
        },
        Err(error) => Err(error),
    };
    // Deletion removes the scoped usage route. Read the final meter after stop.
    let billing = client
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
    let deletion = match deleted {
        Ok(response) => client
            .wait_for_deletion(
                &response.operation.id,
                &WaitOptions {
                    timeout: Duration::from_secs(120),
                    ..Default::default()
                },
            )
            .await
            .map(|_| ()),
        Err(error) => Err(error),
    };
    match deletion {
        Err(boat::Error::DeletionBlocked(operation)) if operation.target_id == id => {
            let absent = matches!(client.get(&GetParams { sandbox_id:id.clone(), ..Default::default() }).await,
                Err(boat::Error::Api(error)) if error.status.as_u16()==404);
            if !absent {
                return Err("Deletion is blocked and the sandbox still exists.".into());
            }
            println!(
                "Sandbox deletion confirmed; provider storage cleanup remains blocked (operation {}).",
                operation.id
            );
        }
        result => {
            result.map_err(|e| format!("Deletion confirmation failed: {e}"))?;
        }
    }
    stopped.map_err(|e| format!("Stop request failed: {e}"))?;
    result
        .map_err(|e| format!("Exercise deadline: {e}"))?
        .map_err(|e| format!("Exercise failed: {e}"))?;
    let billing = billing.map_err(|e| format!("Usage read failed: {e}"))?;
    if billing.running || !affordable(billing.dollars) {
        return Err(
            "The sandbox is still billed as running or its cost is not below one cent.".into(),
        );
    }
    println!(
        "Stopped meter: ${:.6}, {} seconds, running=false.",
        billing.dollars, billing.seconds
    );
    Ok(())
}

#[tokio::test]
async fn cleanup_runs_after_exercise_and_stop_fail() {
    use support::{Reply, serve_sequence};
    let (client, job) = serve_sequence(vec![
        Reply::new(400, &[], b""), // Readiness fails.
        Reply::new(400, &[], b""), // Stop fails; deletion must still run.
        Reply::new(400, &[], b""), // The meter is read before deletion.
        Reply::new(202, &[], br#"{"ok":true,"type":"sandbox.deleting","operation":{"id":"op-test","status":"completed","kind":"sandbox","targetId":"bx_test","reason":"test","attemptCount":1,"requestedAt":"now","completedAt":"now"}}"#),
        Reply::new(200, &[], br#"{"ok":true,"type":"deletion.operation","operation":{"id":"op-test","status":"completed","kind":"sandbox","targetId":"bx_test","reason":"test","attemptCount":1,"requestedAt":"now","completedAt":"now"}}"#),
    ], |b| b).await;
    assert!(lifecycle(&client, "bx_test").await.is_err());
    let seen = job.await.expect("server");
    assert_eq!(
        seen.iter().map(|r| r.method.as_str()).collect::<Vec<_>>(),
        ["GET", "POST", "GET", "DELETE", "GET"]
    );
    assert_eq!(seen[3].headers["x-ascii-confirm-delete"], "bx_test");
    assert_eq!(seen[4].target, "/api/v1/deletion-operations/op-test");
    assert_eq!(seen[2].target, "/api/v1/sandboxes/bx_test/usage");
}
