use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;
type MockServer = (
    String,
    Receiver<Result<CapturedRequest, String>>,
    JoinHandle<()>,
);

#[test]
fn registration_posts_signed_snapshot_and_applies_desired_mode() -> TestResult {
    let state_dir = unique_state_dir("nexus-register");
    fs::create_dir_all(&state_dir)?;
    run_oa_node_json(&[
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let (base_url, captured, handle) = serve_once(
        "HTTP/1.1 200 OK",
        r#"{"status":"accepted","desired_mode":"online","registration_expires_at_ms":9999999999999,"detail":"ok"}"#,
    )?;
    let output = run_oa_node_json(&[
        "nexus",
        "register",
        "--base-url",
        base_url.as_str(),
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    assert_eq!(
        output.pointer("/action").and_then(Value::as_str),
        Some("register")
    );
    assert_eq!(
        output.pointer("/status").and_then(Value::as_str),
        Some("accepted")
    );
    assert_eq!(
        output.pointer("/degraded").and_then(Value::as_bool),
        Some(false)
    );
    let output_digest = output
        .pointer("/snapshot_digest")
        .and_then(Value::as_str)
        .ok_or("missing output snapshot digest")?;
    assert!(output_digest.starts_with("sha256:"));

    let request = captured.recv_timeout(Duration::from_secs(5))??;
    handle
        .join()
        .map_err(|_| "mock nexus server thread panicked")?;
    assert_eq!(request.path, "/v1/cloud/nodes/register");
    let body: Value = serde_json::from_str(&request.body)?;
    assert_eq!(
        body.pointer("/schema_version").and_then(Value::as_str),
        Some("openagents.oa_node.nexus_registry.v1")
    );
    assert_eq!(
        body.pointer("/action").and_then(Value::as_str),
        Some("register")
    );
    assert_eq!(
        body.pointer("/snapshot_digest").and_then(Value::as_str),
        Some(output_digest)
    );
    assert!(body
        .pointer("/signature/digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| digest.starts_with("sha256:")));
    assert!(body
        .pointer("/signature/signing_key_ref")
        .and_then(Value::as_str)
        .is_some_and(|key_ref| key_ref.starts_with("local-keychain://")));

    let status = run_oa_node_json(&["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        status
            .pointer("/lifecycle/desired_mode")
            .and_then(Value::as_str),
        Some("online")
    );
    assert_eq!(
        status
            .pointer("/lifecycle/observed_status")
            .and_then(Value::as_str),
        Some("offline")
    );

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn rejected_heartbeat_degrades_safely() -> TestResult {
    let state_dir = unique_state_dir("nexus-rejected");
    fs::create_dir_all(&state_dir)?;
    run_oa_node_json(&[
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let (base_url, captured, handle) = serve_once(
        "HTTP/1.1 403 Forbidden",
        r#"{"status":"rejected","detail":"bad signature"}"#,
    )?;
    let output = run_oa_node_json(&[
        "nexus",
        "heartbeat",
        "--base-url",
        base_url.as_str(),
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    assert_eq!(
        output.pointer("/action").and_then(Value::as_str),
        Some("heartbeat")
    );
    assert_eq!(
        output.pointer("/status").and_then(Value::as_str),
        Some("rejected")
    );
    assert_eq!(
        output.pointer("/degraded").and_then(Value::as_bool),
        Some(true)
    );
    let request = captured.recv_timeout(Duration::from_secs(5))??;
    handle
        .join()
        .map_err(|_| "mock nexus server thread panicked")?;
    assert_eq!(request.path, "/v1/cloud/nodes/heartbeat");

    let status = run_oa_node_json(&["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        status
            .pointer("/lifecycle/observed_status")
            .and_then(Value::as_str),
        Some("degraded")
    );
    assert!(status
        .pointer("/lifecycle/degradation_reason")
        .and_then(Value::as_str)
        .is_some_and(|reason| reason.contains("nexus_registration_rejected")));

    let events = run_oa_node_json(&[
        "admin",
        "health",
        "list",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        events.pointer("/events/0/code").and_then(Value::as_str),
        Some("nexus_registration_rejected")
    );

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn stale_registration_degrades_safely() -> TestResult {
    let state_dir = unique_state_dir("nexus-stale");
    fs::create_dir_all(&state_dir)?;
    run_oa_node_json(&[
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let (base_url, captured, handle) = serve_once(
        "HTTP/1.1 200 OK",
        r#"{"status":"stale","detail":"snapshot digest is stale"}"#,
    )?;
    let output = run_oa_node_json(&[
        "nexus",
        "register",
        "--base-url",
        base_url.as_str(),
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    assert_eq!(
        output.pointer("/status").and_then(Value::as_str),
        Some("stale")
    );
    assert_eq!(
        output.pointer("/degraded").and_then(Value::as_bool),
        Some(true)
    );
    let request = captured.recv_timeout(Duration::from_secs(5))??;
    handle
        .join()
        .map_err(|_| "mock nexus server thread panicked")?;
    assert_eq!(request.path, "/v1/cloud/nodes/register");

    let status = run_oa_node_json(&["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert!(status
        .pointer("/lifecycle/degradation_reason")
        .and_then(Value::as_str)
        .is_some_and(|reason| reason.contains("nexus_registration_stale")));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[derive(Debug)]
struct CapturedRequest {
    path: String,
    body: String,
}

fn serve_once(
    status_line: &'static str,
    body: &'static str,
) -> Result<MockServer, Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let (sender, receiver) = mpsc::channel();
    let body = body.to_string();
    let handle = thread::spawn(move || {
        let result = listener
            .accept()
            .map_err(|error| format!("mock nexus accept failed: {error}"))
            .and_then(|(mut stream, _)| {
                let request = read_http_request(&mut stream)?;
                let response = format!(
                    "{status_line}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .map_err(|error| format!("mock nexus response write failed: {error}"))?;
                Ok(request)
            });
        let _ = sender.send(result);
    });
    Ok((format!("http://{address}"), receiver, handle))
}

fn read_http_request(stream: &mut TcpStream) -> Result<CapturedRequest, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("mock nexus read timeout failed: {error}"))?;
    let mut raw = Vec::new();
    let mut buffer = [0u8; 1024];
    let header_end = loop {
        let count = stream
            .read(&mut buffer)
            .map_err(|error| format!("mock nexus request read failed: {error}"))?;
        if count == 0 {
            return Err("mock nexus request closed before headers".to_string());
        }
        raw.extend_from_slice(&buffer[..count]);
        if let Some(header_end) = find_header_end(raw.as_slice()) {
            break header_end;
        }
    };

    let header = std::str::from_utf8(&raw[..header_end])
        .map_err(|error| format!("mock nexus request header not utf8: {error}"))?;
    let request_line = header
        .lines()
        .next()
        .ok_or_else(|| "mock nexus request missing request line".to_string())?;
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "mock nexus request missing path".to_string())?
        .to_string();
    let content_length = header
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .ok_or_else(|| "mock nexus request missing content-length".to_string())?;

    while raw.len() < header_end + content_length {
        let count = stream
            .read(&mut buffer)
            .map_err(|error| format!("mock nexus body read failed: {error}"))?;
        if count == 0 {
            return Err("mock nexus request closed before body".to_string());
        }
        raw.extend_from_slice(&buffer[..count]);
    }

    let body = String::from_utf8(raw[header_end..header_end + content_length].to_vec())
        .map_err(|error| format!("mock nexus body not utf8: {error}"))?;
    Ok(CapturedRequest { path, body })
}

fn find_header_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
}

fn run_oa_node_json(args: &[&str]) -> Result<Value, Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_oa-node"))
        .env_remove("OPENAGENTS_PSIONIC_ENDPOINT")
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "oa-node failed: status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn unique_state_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "oa-node-{label}-{}-{}",
        std::process::id(),
        unique_suffix()
    ))
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn state_path(path: &Path) -> Result<&str, Box<dyn std::error::Error>> {
    path.to_str().ok_or_else(|| "state dir is not utf-8".into())
}
