//! Exercise the supported CLI against an isolated engine RPC fixture.

use std::process::{Command, Output};
use std::thread::JoinHandle;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};

const HOST: &str = "00000000-0000-4000-8000-000000000001";

fn command(home: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_openagents"));
    command.env("HOME", home).env("USERPROFILE", home);
    command
}

fn fixture(responses: Vec<Value>) -> (u16, JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let worker = std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                // A freshly linked binary's first launch can take many seconds on macOS.
                let (stream, _) =
                    tokio::time::timeout(std::time::Duration::from_secs(120), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
                for (index, response) in responses.into_iter().enumerate() {
                    let frame = tokio::time::timeout(std::time::Duration::from_secs(10), ws.next())
                        .await
                        .unwrap()
                        .unwrap()
                        .unwrap();
                    let request: Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
                    assert_eq!(request["method"], "ListAgentAccounts");
                    assert_eq!(request["params"]["targetDeviceId"], HOST);
                    assert_eq!(request["params"]["forceUsage"], index > 0);
                    ws.send(tokio_tungstenite::tungstenite::Message::text(
                        json!({"id": request["id"], "ok": response}).to_string(),
                    ))
                    .await
                    .unwrap();
                }
                // Keep the socket alive until the client consumes its last reply.
                let _ = tokio::time::timeout(std::time::Duration::from_secs(10), ws.next()).await;
            });
    });
    (port, worker)
}

fn snapshot(windows: Value, error: Value) -> Value {
    json!({"accounts": [{
        "id": "fixture-account", "harness": "codex", "active": true,
        "email": null, "planLabel": "Fixture plan", "usageFetchedAt": 1000,
        "usageWindows": windows, "usageError": error,
    }], "warnings": []})
}

fn read(responses: Vec<Value>, refresh: bool) -> Output {
    let home = tempfile::tempdir().unwrap();
    let (port, worker) = fixture(responses);
    let mut command = command(home.path());
    command.args([
        "accounts",
        "usage",
        "--device",
        HOST,
        "--account",
        "active",
        "--json",
        "--ipc-port",
        &port.to_string(),
    ]);
    if refresh {
        command.arg("--refresh");
    }
    let output = command.output().unwrap();
    assert!(
        worker.join().is_ok(),
        "fixture failed; CLI stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn json_keeps_cached_windows_and_backoff_error_with_success_exit() {
    let windows = json!([
        {"label": "5 hours", "usedFraction": 0.25, "resetsAt": null},
        {"label": "Weekly", "usedFraction": 0.5, "resetsAt": "2030-01-01T00:00:00Z"},
    ]);
    let output = read(
        vec![
            snapshot(windows.clone(), Value::Null),
            snapshot(windows, json!("Rate limited — retrying in 2m")),
        ],
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema"], "openagents.accounts.usage.v1");
    assert_eq!(value["deviceId"], HOST);
    assert_eq!(value["account"]["id"], "fixture-account");
    assert_eq!(value["account"]["planLabel"], "Fixture plan");
    assert_eq!(value["state"], "stale");
    assert_eq!(value["stale"], true);
    assert_eq!(value["usageFetchedAt"], 1000);
    assert_eq!(value["cachedAt"], 1000);
    assert_eq!(value["windows"].as_array().unwrap().len(), 2);
    assert_eq!(value["windows"][0]["usedPercent"], 25.0);
    assert!(value["windows"][0]["resetsAt"].is_null());
    assert_eq!(value["windows"][1]["resetsAt"], "2030-01-01T00:00:00Z");
    assert_eq!(value["refresh"]["result"], "cached");
    assert_eq!(value["usageError"], "Rate limited — retrying in 2m");
    assert!(!value.to_string().contains("email"));
}

#[test]
fn unavailable_account_is_a_successful_read_with_empty_windows() {
    let output = read(vec![snapshot(json!([]), json!("Sign in again"))], false);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["state"], "unavailable");
    assert_eq!(value["available"], false);
    assert_eq!(value["windows"], json!([]));
}

#[test]
fn missing_account_is_a_selection_failure() {
    let output = read(vec![json!({"accounts": [], "warnings": []})], false);
    assert_eq!(output.status.code(), Some(1));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        value["error"]
            .as_str()
            .unwrap()
            .contains("No matching Codex account")
    );
}

#[test]
fn invalid_selection_is_a_usage_error_before_connecting() {
    let home = tempfile::tempdir().unwrap();
    let output = command(home.path())
        .args(["accounts", "usage", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(64));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["usage"], true);
}
