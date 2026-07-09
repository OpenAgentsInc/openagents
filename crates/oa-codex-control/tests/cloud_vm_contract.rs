//! Contract test for the production Cloud-VM provisioner HTTP surface (#6200).
//!
//! Proves the `oa-codex-control` `/v1/cloud-vm/sessions` route satisfies the
//! qa-runner's typed `CloudVmProvisionerV2` / `CloudVmHandle` seam from
//! `apps/qa-runner/src/backend.ts`:
//!
//!   provision -> exec -> copyOut -> teardown
//!
//! The qa-runner (TypeScript) calls this cloud-side service over HTTP. This test
//! is the cloud-side end of that wire contract: it boots the real daemon, POSTs
//! a request shaped exactly like the seam (run id + `CloudVmOs` tier + redacted
//! target/owner + in-VM session command), and asserts the response carries every
//! lifecycle stage:
//!
//!   - provision: an opaque `vmId` (mirrors `CloudVmHandle.id`) + `os` tier.
//!   - exec:      a `{ code, output }` transcript (mirrors `CloudVmHandle.exec`).
//!   - copyOut:   `extractedTo` host dir + a dereferenceable `result.json` whose
//!                schemaVersion is the public-safe `QaRunResult`.
//!   - teardown:  a cleanup receipt with `tornDown = true`.
//!
//! The route runs the **fake** lane here (no KVM on CI/dev), so the contract is
//! proven deterministically with no microVM boot. The live firecracker lane is
//! the deploy step on a Linux KVM host (see the runbook + the `#[ignore]` live
//! proof in `src/cloud_vm.rs`).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use serde_json::Value;

const TOKEN: &str = "cloud-vm-contract-test-token";

struct Daemon {
    child: Child,
    addr: String,
    state_dir: PathBuf,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.state_dir);
    }
}

fn unique_state_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "oa-codex-control-{label}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

/// Pick a free localhost port by binding and immediately dropping the listener.
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    listener.local_addr().expect("local addr").port()
}

fn bin_path() -> PathBuf {
    // `CARGO_BIN_EXE_<name>` points at the freshly built binary under test.
    PathBuf::from(env!("CARGO_BIN_EXE_oa-codex-control"))
}

fn start_daemon(label: &str) -> Daemon {
    let port = free_port();
    let addr = format!("127.0.0.1:{port}");
    let state_dir = unique_state_dir(label);
    std::fs::create_dir_all(&state_dir).expect("state dir");
    let auth_root = state_dir.join("auth");
    std::fs::create_dir_all(&auth_root).expect("auth root");

    let child = Command::new(bin_path())
        .env("OA_CODEX_CONTROL_TOKEN", TOKEN)
        .env("OA_CODEX_CONTROL_BIND", &addr)
        .env("OA_CODEX_CONTROL_STATE_ROOT", &state_dir)
        .env("OA_CODEX_AUTH_JSON_ROOT", &auth_root)
        // Local-only auth so the daemon does not require a grant resolver.
        .env("OA_CODEX_CONTROL_ALLOW_LOCAL_AUTH_ONLY", "true")
        // Default cloud-vm lane is fake; be explicit for clarity.
        .env("OA_CLOUD_VM_PROVISIONER", "fake")
        .spawn()
        .expect("spawn oa-codex-control");

    let daemon = Daemon {
        child,
        addr,
        state_dir,
    };
    wait_healthy(&daemon.addr);
    daemon
}

fn wait_healthy(addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Ok((status, _)) = http_request(addr, "GET", "/healthz", None, None) {
            if status == 200 {
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("daemon did not become healthy at {addr}");
}

/// Minimal HTTP/1.1 client (std only, matching the repo's no-extra-deps style).
fn http_request(
    addr: &str,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
    bearer: Option<&str>,
) -> Result<(u16, Vec<u8>), String> {
    let mut stream =
        TcpStream::connect(addr).map_err(|error| format!("connect {addr}: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|error| error.to_string())?;
    let body = body.unwrap_or(&[]);
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nhost: {addr}\r\ncontent-length: {}\r\nconnection: close\r\n",
        body.len()
    );
    if let Some(token) = bearer {
        request.push_str(&format!("authorization: Bearer {token}\r\n"));
    }
    if !body.is_empty() {
        request.push_str("content-type: application/json\r\n");
    }
    request.push_str("\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|error| error.to_string())?;
    stream.write_all(body).map_err(|error| error.to_string())?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| error.to_string())?;
    let split = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| "no header terminator".to_string())?;
    let header = String::from_utf8_lossy(&response[..split]);
    let status = header
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| "no status line".to_string())?;
    let body = response[split + 4..].to_vec();
    Ok((status, body))
}

fn post_json(daemon: &Daemon, path: &str, value: &Value) -> (u16, Value) {
    let body = serde_json::to_vec(value).expect("encode body");
    let (status, raw) =
        http_request(&daemon.addr, "POST", path, Some(&body), Some(TOKEN)).expect("http request");
    let parsed: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
    (status, parsed)
}

/// The headline contract test: the route fulfils the full
/// provision -> exec -> copyOut -> teardown lifecycle in one call, in the exact
/// wire shape the qa-runner `CloudVmProvisionerV2` sends.
#[test]
fn cloud_vm_session_route_satisfies_provisioner_v2_lifecycle() {
    let daemon = start_daemon("cloud-vm-lifecycle");

    // The wire request the qa-runner's CloudVmProvisionerV2 would send:
    //   { runId, os, targetName, ownerRef, sessionCommand }.
    let request = serde_json::json!({
        "runId": "run_contract_demo",
        "os": "linux",
        "targetName": "openagents.com-staging",
        "ownerRef": "owner://sha256/contract",
        "sessionCommand": ["sh", "-c", "qa-session --emit /qa/artifacts"],
    });

    let (status, response) = post_json(&daemon, "/v1/cloud-vm/sessions", &request);
    assert_eq!(status, 200, "lifecycle response: {response}");

    // provision: opaque vm id + os tier (CloudVmHandle.id / CloudVmHandle.os).
    let vm_id = response
        .pointer("/vmId")
        .and_then(Value::as_str)
        .expect("vmId present");
    assert!(vm_id.starts_with("cloud-vm-ref://"), "vmId = {vm_id}");
    assert_eq!(
        response.pointer("/os").and_then(Value::as_str),
        Some("linux")
    );
    assert_eq!(
        response.pointer("/provisionerKind").and_then(Value::as_str),
        Some("fake")
    );
    assert_eq!(
        response
            .pointer("/provisionReceipt/healthy")
            .and_then(Value::as_bool),
        Some(true)
    );

    // exec: { code, output } transcript (CloudVmHandle.exec).
    assert_eq!(
        response.pointer("/exec/code").and_then(Value::as_i64),
        Some(0)
    );
    let output = response
        .pointer("/exec/output")
        .and_then(Value::as_str)
        .expect("exec output");
    assert!(output.contains("qa-session"), "exec output = {output}");

    // copyOut: extractedTo host dir + a dereferenceable public-safe result.json.
    let extracted_to = response
        .pointer("/extractedTo")
        .and_then(Value::as_str)
        .expect("extractedTo present");
    let result_path = PathBuf::from(extracted_to).join("result.json");
    assert!(
        result_path.exists(),
        "extracted result.json must exist at {result_path:?}"
    );
    let result: Value =
        serde_json::from_slice(&std::fs::read(&result_path).expect("read result.json"))
            .expect("parse result.json");
    assert_eq!(
        result.pointer("/schemaVersion").and_then(Value::as_str),
        Some("openagents.qa_runner.result.v1")
    );
    assert_eq!(
        result.pointer("/backend").and_then(Value::as_str),
        Some("cloud-vm")
    );

    // teardown: cleanup receipt with tornDown = true.
    assert_eq!(
        response
            .pointer("/cleanupReceipt/tornDown")
            .and_then(Value::as_bool),
        Some(true),
        "teardown must have torn the VM down"
    );
    assert_eq!(
        response
            .pointer("/cleanupReceipt/artifactsExtracted")
            .and_then(Value::as_bool),
        Some(true)
    );

    // refs-only: the whole response must not leak forbidden material.
    let body = response.to_string();
    for marker in ["bearer ", ".sock", "/dev/kvm", "-----begin"] {
        assert!(
            !body.to_ascii_lowercase().contains(marker),
            "response leaked '{marker}': {body}"
        );
    }
}

/// A non-linux OS tier refuses honestly (400) rather than booting the wrong OS
/// or faking a green — the macOS/Windows tiers are tracked as they come online.
#[test]
fn cloud_vm_session_route_refuses_unavailable_os_tier_for_live_lane() {
    // The fake lane accepts all tiers; this asserts the *parse + route* path is
    // honest about an unknown tier (400). The OS-tier-unavailable refusal for the
    // live lane is unit-tested directly in src/cloud_vm.rs (no KVM on CI).
    let daemon = start_daemon("cloud-vm-bad-os");
    let request = serde_json::json!({
        "runId": "run_bad_os",
        "os": "plan9",
        "targetName": "openagents.com-staging",
        "ownerRef": "owner://sha256/contract",
        "sessionCommand": ["sh", "-c", "true"],
    });
    let (status, response) = post_json(&daemon, "/v1/cloud-vm/sessions", &request);
    assert_eq!(status, 400, "bad os tier must be rejected: {response}");
    assert_eq!(
        response.pointer("/status").and_then(Value::as_str),
        Some("failed")
    );
}

/// The route is owner-gated by the daemon bearer token, mirroring every other
/// `/v1/*` route. An unauthorized call is refused.
#[test]
fn cloud_vm_session_route_requires_authorization() {
    let daemon = start_daemon("cloud-vm-auth");
    let request = serde_json::json!({
        "runId": "run_auth",
        "os": "linux",
        "targetName": "openagents.com-staging",
        "ownerRef": "owner://sha256/contract",
        "sessionCommand": ["sh", "-c", "true"],
    });
    let body = serde_json::to_vec(&request).unwrap();
    let (status, _) = http_request(
        &daemon.addr,
        "POST",
        "/v1/cloud-vm/sessions",
        Some(&body),
        None, // no bearer
    )
    .expect("http request");
    assert_eq!(status, 401, "unauthorized cloud-vm request must be refused");
}
