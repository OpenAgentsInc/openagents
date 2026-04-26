use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::Manager;

use crate::health;
use crate::pylon::{
    self, ProofRunOptions, PylonBinaryStatus, PylonManager, PylonStatusProjection, redact_sensitive,
};

pub const CONTROL_SCHEMA_VERSION: u16 = 1;
pub const CONTROL_MANIFEST_ENV: &str = "OPENAGENTS_AUTOPILOT_CONTROL_MANIFEST";
pub const CONTROL_BIND_ENV: &str = "OPENAGENTS_AUTOPILOT_CONTROL_BIND";
pub const CONTROL_AUTH_TOKEN_ENV: &str = "OPENAGENTS_AUTOPILOT_CONTROL_AUTH_TOKEN";
pub const CONTROL_DISABLED_ENV: &str = "OPENAGENTS_AUTOPILOT_CONTROL_DISABLED";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ControlManifest {
    pub schema_version: u16,
    pub product: String,
    pub control: String,
    pub base_url: String,
    pub auth_token: String,
    pub pid: u32,
    pub started_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ControlSnapshot {
    schema_version: u16,
    product: &'static str,
    control: &'static str,
    generated_at: String,
    pid: u32,
    pylon_binary: PylonBinaryStatus,
    pylon_status: PylonStatusProjection,
    active_proof: Option<pylon::ProofRunProjection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderModeRequest {
    mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NamespaceRequest {
    namespace: String,
}

pub fn start_control_plane(app: tauri::AppHandle) -> Result<(), String> {
    if env::var(CONTROL_DISABLED_ENV)
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return Ok(());
    }

    let bind_addr = env::var(CONTROL_BIND_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "127.0.0.1:0".to_string());
    let listener = TcpListener::bind(bind_addr.as_str())
        .map_err(|error| format!("failed to bind Autopilot Tauri control plane: {error}"))?;
    let local_addr = listener
        .local_addr()
        .map_err(|error| format!("failed to read Autopilot Tauri control address: {error}"))?;
    let auth_token = env::var(CONTROL_AUTH_TOKEN_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(generate_control_token);
    let manifest = ControlManifest {
        schema_version: CONTROL_SCHEMA_VERSION,
        product: "Autopilot".to_string(),
        control: "tauri".to_string(),
        base_url: format!("http://{local_addr}"),
        auth_token: auth_token.clone(),
        pid: process::id(),
        started_at: now_epoch_ms_string(),
    };
    let manifest_path = control_manifest_path();
    write_manifest(&manifest_path, &manifest)?;

    thread::Builder::new()
        .name("autopilot-tauri-control".to_string())
        .spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let app = app.clone();
                        let auth_token = auth_token.clone();
                        let _ = thread::Builder::new()
                            .name("autopilot-tauri-control-client".to_string())
                            .spawn(move || handle_stream(stream, app, auth_token));
                    }
                    Err(error) => {
                        eprintln!("Autopilot Tauri control accept failed: {error}");
                    }
                }
            }
        })
        .map_err(|error| format!("failed to spawn Autopilot Tauri control plane: {error}"))?;

    Ok(())
}

pub fn control_manifest_path() -> PathBuf {
    env::var(CONTROL_MANIFEST_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_control_manifest_path)
}

fn default_control_manifest_path() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot")
        .join("tauri-control.json")
}

fn write_manifest(path: &PathBuf, manifest: &ControlManifest) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create Autopilot Tauri control manifest dir {}: {error}",
                parent.display()
            )
        })?;
    }
    let payload = serde_json::to_string_pretty(manifest)
        .map_err(|error| format!("failed to encode Autopilot Tauri control manifest: {error}"))?;
    fs::write(path, format!("{payload}\n")).map_err(|error| {
        format!(
            "failed to write Autopilot Tauri control manifest {}: {error}",
            path.display()
        )
    })
}

fn handle_stream(mut stream: TcpStream, app: tauri::AppHandle, auth_token: String) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let response = match read_http_request(&mut stream) {
        Ok(request) => handle_request(request, app, auth_token),
        Err(error) => http_json(
            400,
            json!({ "ok": false, "error": redact_sensitive(&error) }),
        ),
    };
    let _ = stream.write_all(response.as_bytes());
}

fn handle_request(request: HttpRequest, app: tauri::AppHandle, auth_token: String) -> String {
    if request.path == "/healthz" {
        return http_json(200, json!({ "ok": true, "control": "tauri" }));
    }
    if !request.is_authorized(auth_token.as_str()) {
        return http_json(401, json!({ "ok": false, "error": "unauthorized" }));
    }

    let result = route_request(request, app);
    match result {
        Ok(value) => http_json(200, value),
        Err(error) => http_json(
            500,
            json!({ "ok": false, "error": redact_sensitive(&error) }),
        ),
    }
}

fn route_request(request: HttpRequest, app: tauri::AppHandle) -> Result<Value, String> {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/v1/status") => serde_json::to_value(control_snapshot(&app))
            .map_err(|error| format!("failed to encode control snapshot: {error}")),
        ("GET", "/v1/pylon/status") => serde_json::to_value(pylon::pylon_get_status(app.state()))
            .map_err(|error| format!("failed to encode pylon status: {error}")),
        ("GET", "/v1/homework/status") => {
            serde_json::to_value(pylon::pylon_homework_get(app.state()))
                .map_err(|error| format!("failed to encode homework status: {error}"))
        }
        ("GET", "/v1/health/nexus/status") => serde_json::to_value(
            health::nexus_health_status_blocking(health::NexusHealthRequest::default())?,
        )
        .map_err(|error| format!("failed to encode Nexus health status: {error}")),
        ("POST", "/v1/pylon/start") => {
            serde_json::to_value(pylon::pylon_start(app.clone(), app.state(), None))
                .map_err(|error| format!("failed to encode pylon start response: {error}"))?
                .as_result()
        }
        ("POST", "/v1/pylon/stop") => {
            serde_json::to_value(pylon::pylon_stop(app.clone(), app.state()))
                .map_err(|error| format!("failed to encode pylon stop response: {error}"))?
                .as_result()
        }
        ("POST", "/v1/pylon/restart") => {
            serde_json::to_value(pylon::pylon_restart(app.clone(), app.state(), None))
                .map_err(|error| format!("failed to encode pylon restart response: {error}"))?
                .as_result()
        }
        ("POST", "/v1/pylon/mode") => {
            let body: ProviderModeRequest = request.json_body()?;
            serde_json::to_value(pylon::pylon_set_mode(body.mode, app.state()))
                .map_err(|error| format!("failed to encode pylon mode response: {error}"))?
                .as_result()
        }
        ("POST", "/v1/pylon/logs") => serde_json::to_value(pylon::pylon_open_logs())
            .map_err(|error| format!("failed to encode pylon logs response: {error}"))?
            .as_result(),
        ("POST", "/v1/proof/run") => {
            let body: ProofRunOptions = request.json_body()?;
            serde_json::to_value(pylon::proof_run(app.clone(), app.state(), body))
                .map_err(|error| format!("failed to encode proof run response: {error}"))?
                .as_result()
        }
        ("POST", "/v1/proof/get") => {
            let body: NamespaceRequest = request.json_body()?;
            serde_json::to_value(pylon::proof_get(app.state(), body.namespace))
                .map_err(|error| format!("failed to encode proof get response: {error}"))
        }
        ("POST", "/v1/proof/doctor") => {
            let body: NamespaceRequest = request.json_body()?;
            serde_json::to_value(pylon::proof_doctor(app.state(), body.namespace))
                .map_err(|error| format!("failed to encode proof doctor response: {error}"))?
                .as_result()
        }
        ("POST", "/v1/proof/stop") => {
            let body: NamespaceRequest = request.json_body()?;
            serde_json::to_value(pylon::proof_stop(body.namespace))
                .map_err(|error| format!("failed to encode proof stop response: {error}"))?
                .as_result()
        }
        ("POST", "/v1/proof/reset") => {
            let body: NamespaceRequest = request.json_body()?;
            serde_json::to_value(pylon::proof_reset(body.namespace))
                .map_err(|error| format!("failed to encode proof reset response: {error}"))?
                .as_result()
        }
        ("POST", "/v1/proof/artifacts") => {
            let body: NamespaceRequest = request.json_body()?;
            serde_json::to_value(pylon::proof_open_artifacts(body.namespace))
                .map_err(|error| format!("failed to encode proof artifacts response: {error}"))?
                .as_result()
        }
        _ => Err(format!(
            "unsupported control route: {} {}",
            request.method, request.path
        )),
    }
}

fn control_snapshot(app: &tauri::AppHandle) -> ControlSnapshot {
    let state = app.state::<PylonManager>();
    ControlSnapshot {
        schema_version: CONTROL_SCHEMA_VERSION,
        product: "Autopilot",
        control: "tauri",
        generated_at: now_epoch_ms_string(),
        pid: process::id(),
        pylon_binary: pylon::pylon_detect(),
        pylon_status: pylon::pylon_get_status(app.state()),
        active_proof: state.proof_snapshot(),
    }
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl HttpRequest {
    fn is_authorized(&self, token: &str) -> bool {
        self.headers.iter().any(|(key, value)| {
            key.eq_ignore_ascii_case("authorization") && value.trim() == format!("Bearer {token}")
        })
    }

    fn json_body<T: for<'de> Deserialize<'de>>(&self) -> Result<T, String> {
        serde_json::from_slice(self.body.as_slice())
            .map_err(|error| format!("failed to decode request JSON: {error}"))
    }
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut buffer = Vec::new();
    let mut temp = [0_u8; 2048];
    let header_end = loop {
        let read = stream
            .read(&mut temp)
            .map_err(|error| format!("failed to read request: {error}"))?;
        if read == 0 {
            return Err("request closed before headers completed".to_string());
        }
        buffer.extend_from_slice(&temp[..read]);
        if let Some(index) = find_header_end(buffer.as_slice()) {
            break index;
        }
        if buffer.len() > 64 * 1024 {
            return Err("request headers exceeded 64 KiB".to_string());
        }
    };

    let header_bytes = &buffer[..header_end];
    let header_text = String::from_utf8_lossy(header_bytes);
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_string();
    let path = parts
        .next()
        .ok_or_else(|| "missing path".to_string())?
        .to_string();
    let mut headers = Vec::new();
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            headers.push((key.trim().to_string(), value.trim().to_string()));
        }
    }
    let content_length = headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = header_end + 4;
    let mut body = buffer[body_start..].to_vec();
    while body.len() < content_length {
        let read = stream
            .read(&mut temp)
            .map_err(|error| format!("failed to read request body: {error}"))?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&temp[..read]);
    }
    body.truncate(content_length);
    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

trait JsonResultExt {
    fn as_result(self) -> Result<Value, String>;
}

impl JsonResultExt for Value {
    fn as_result(self) -> Result<Value, String> {
        if let Some(ok) = self.get("Ok") {
            return Ok(ok.clone());
        }
        if let Some(error) = self.get("Err").and_then(Value::as_str) {
            return Err(error.to_string());
        }
        Ok(self)
    }
}

fn http_json(status: u16, value: Value) -> String {
    let payload = serde_json::to_string(&value).unwrap_or_else(|_| {
        "{\"ok\":false,\"error\":\"failed to encode control response\"}".to_string()
    });
    let label = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        500 => "Internal Server Error",
        _ => "OK",
    };
    format!(
        "HTTP/1.1 {status} {label}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
        payload.len()
    )
}

fn generate_control_token() -> String {
    format!(
        "tauri-{}-{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    )
}

fn now_epoch_ms_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[allow(dead_code)]
fn _socket_addr_for_docs(_: SocketAddr) {}
