//! Bounded private Unix transport for one resident LIVE controller.
use crate::{
    Result,
    controller::{Bundle, Controller},
};
use nostr::domain::Event;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
pub const MAX_FRAME: usize = 40 * 1_048_576;
const MAX_CLIENTS: usize = 8;
const TIMEOUT: Duration = Duration::from_secs(5);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
fn claim(root: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(root.join("runtime.lock"))
        .map_err(|e| e.to_string())?;
    file.try_lock()
        .map_err(|_| "conflict: a resident already owns these media adapters")?;
    Ok(file)
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Version {
    #[serde(rename = "openagents.livehost-transport.v1")]
    V1,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub event: Event,
    pub artifacts: BTreeMap<String, Vec<u8>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Submit { bundle: Bundle },
    Inspect { event: Event },
    Frame { event: Event },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub v: Version,
    pub id: String,
    pub operation: Operation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub v: Version,
    pub id: String,
    pub outcome: Outcome,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Ok { reply: Reply },
    Error { code: String },
}
impl Request {
    pub fn new(operation: Operation) -> Self {
        Self {
            v: Version::V1,
            id: workspace_host::artifact::token(),
            operation,
        }
    }
    fn dispatch(self, host: &Controller, now: u64) -> Response {
        let result = if self.id.len() != 64
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            Err("malformed: request id".into())
        } else {
            match self.operation {
                Operation::Submit { bundle } => host.serve(&bundle, now),
                Operation::Inspect { event } => host.inspect(&event, now),
                Operation::Frame { event } => host.frame(&event, now),
            }
        };
        Response {
            v: Version::V1,
            id: self.id,
            outcome: match result {
                Ok(reply) => Outcome::Ok { reply },
                Err(error) => Outcome::Error {
                    code: crate::controller::code(&error).into(),
                },
            },
        }
    }
}
struct Client {
    stream: UnixStream,
    input: Vec<u8>,
    output: Option<Vec<u8>>,
    offset: usize,
    started: Instant,
}

/// One engine owner per state directory; each connection carries one request.
pub struct Server<'a> {
    host: &'a Controller,
    listener: UnixListener,
    clients: Vec<Client>,
    path: PathBuf,
    identity: (u64, u64),
    _owner: File,
    last_expiry_check: Option<u64>,
}
impl<'a> Server<'a> {
    pub fn bind(host: &'a Controller, path: PathBuf) -> Result<Self> {
        let parent = path.parent().ok_or("malformed: socket parent")?;
        let meta = fs::symlink_metadata(parent).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.permissions().mode() & 0o077 != 0 {
            return Err("not_admitted: socket directory must be private".into());
        }
        let owner = claim(host.store().root())?;
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if !meta.file_type().is_socket() {
                return Err("conflict: socket path is already in use".into());
            }
            match UnixStream::connect(&path) {
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                    fs::remove_file(&path).map_err(|e| e.to_string())?
                }
                _ => return Err("conflict: socket is already in use".into()),
            }
        }
        let listener = UnixListener::bind(&path).map_err(|e| e.to_string())?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        Ok(Self {
            host,
            listener,
            clients: Vec::new(),
            path,
            identity: (meta.dev(), meta.ino()),
            _owner: owner,
            last_expiry_check: None,
        })
    }

    /// Advance the watchdog and bounded connection I/O once.
    pub fn step(&mut self, now: u64) -> Result<()> {
        let started = Instant::now();
        let host = self.host;
        if self.last_expiry_check != Some(now) {
            host.expire_media(now)?;
            self.last_expiry_check = Some(now);
        }
        for _ in 0..MAX_CLIENTS {
            match self.listener.accept() {
                Ok((stream, _)) if self.clients.len() < MAX_CLIENTS => {
                    stream.set_nonblocking(true).map_err(|e| e.to_string())?;
                    self.clients.push(Client {
                        stream,
                        input: Vec::new(),
                        output: None,
                        offset: 0,
                        started: Instant::now(),
                    });
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.to_string()),
            }
        }
        self.clients.retain_mut(|client| {
            if client.started.elapsed() >= TIMEOUT {
                return false;
            }
            match client.advance(host, now.saturating_add(started.elapsed().as_secs())) {
                Ok(keep) => keep,
                Err(_) => false,
            }
        });
        Ok(())
    }

    pub fn run(&mut self, stop: &AtomicBool) -> Result<()> {
        while !stop.load(Ordering::Relaxed) {
            self.step(crate::now())?;
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
}
impl Drop for Server<'_> {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.path).is_ok_and(|m| (m.dev(), m.ino()) == self.identity) {
            let _ = fs::remove_file(&self.path);
        }
    }
}
impl Client {
    fn advance(&mut self, host: &Controller, now: u64) -> Result<bool> {
        let started = Instant::now();
        if let Some(output) = &self.output {
            let end = output.len().min(self.offset + 1_048_576);
            match self.stream.write(&output[self.offset..end]) {
                Ok(0) => Ok(false),
                Ok(n) => {
                    self.offset += n;
                    Ok(self.offset < output.len())
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(true),
                Err(e) => Err(e.to_string()),
            }
        } else {
            let mut chunk = [0u8; 65_536];
            for _ in 0..16 {
                match self.stream.read(&mut chunk) {
                    Ok(0) => return Ok(false),
                    Ok(n) => {
                        if self.input.len() + n > MAX_FRAME {
                            return Err("limit_exceeded: transport frame".into());
                        }
                        self.input.extend_from_slice(&chunk[..n]);
                        if chunk[..n].contains(&b'\n') {
                            let end = self
                                .input
                                .iter()
                                .position(|b| *b == b'\n')
                                .ok_or("malformed: frame")?;
                            if end + 1 != self.input.len() || self.input[..end].contains(&b'\n') {
                                return Err("malformed: one transport frame required".into());
                            }
                            let value = nostr::contracts::parse_strict_bounded(
                                &self.input[..end],
                                MAX_FRAME,
                            )
                            .map_err(|e| e.to_string())?;
                            let request: Request = serde_json::from_value(value)
                                .map_err(|_| "malformed: transport request")?;
                            let response = request
                                .dispatch(host, now.saturating_add(started.elapsed().as_secs()));
                            let mut bytes =
                                serde_json::to_vec(&response).map_err(|e| e.to_string())?;
                            if bytes.len() >= MAX_FRAME {
                                return Err("limit_exceeded: transport response".into());
                            }
                            bytes.push(b'\n');
                            self.output = Some(bytes);
                            self.input.clear();
                            self.started = Instant::now();
                            return self.advance(host, now);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(true),
                    Err(e) => return Err(e.to_string()),
                }
            }
            Ok(true)
        }
    }
}

/// Send one request without the host key. Callers verify signed event replies
/// with the existing CJ or private-artifact reader and their own identity.
pub fn request(path: &Path, request: &Request) -> Result<Response> {
    let mut bytes = serde_json::to_vec(request).map_err(|e| e.to_string())?;
    if bytes.len() >= MAX_FRAME {
        return Err("limit_exceeded: transport request".into());
    }
    bytes.push(b'\n');
    let mut stream = UnixStream::connect(path).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let mut sent = 0;
    while sent < bytes.len() {
        let remaining = TIMEOUT
            .checked_sub(started.elapsed())
            .filter(|d| !d.is_zero())
            .ok_or("unavailable: live host timed out")?;
        stream
            .set_write_timeout(Some(remaining))
            .map_err(|e| e.to_string())?;
        let n = stream.write(&bytes[sent..]).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("unavailable: live host disconnected".into());
        }
        sent += n;
    }
    let mut response = Vec::new();
    let mut chunk = [0u8; 65_536];
    let response_started = Instant::now();
    loop {
        let remaining = RESPONSE_TIMEOUT
            .checked_sub(response_started.elapsed())
            .filter(|d| !d.is_zero())
            .ok_or("unavailable: live host timed out")?;
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|e| e.to_string())?;
        let n = stream.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("unavailable: live host disconnected".into());
        }
        response.extend_from_slice(&chunk[..n]);
        if response.len() > MAX_FRAME {
            return Err("limit_exceeded: transport response".into());
        }
        if chunk[..n].contains(&b'\n') {
            let end = response
                .iter()
                .position(|b| *b == b'\n')
                .ok_or("malformed: response frame")?;
            if end + 1 != response.len() {
                return Err("malformed: response frame".into());
            }
            let value = nostr::contracts::parse_strict_bounded(&response[..end], MAX_FRAME)
                .map_err(|e| e.to_string())?;
            let response: Response =
                serde_json::from_value(value).map_err(|_| "malformed: live host reply")?;
            if response.id != request.id {
                return Err("identity_mismatch: live reply ID".into());
            }
            return Ok(response);
        }
    }
}

/// Verify the exact host, caller, request, reply type, and attached bytes.
pub fn read_reply(
    reply: &Reply,
    host: &str,
    secret: &secp256k1::SecretKey,
    query: &serde_json::Value,
) -> Result<serde_json::Value> {
    use serde_json::Value;
    let opened = nostr::private_artifact::open(&reply.event, secret).map_err(|e| e.to_string())?;
    let caller = secret
        .x_only_public_key(&secp256k1::Secp256k1::new())
        .0
        .to_string();
    if opened.signer() != host || opened.recipient() != caller {
        return Err("identity_mismatch: live reply identity".into());
    }
    let body = nostr::contracts::parse_strict_bounded(
        opened.inline_bytes().ok_or("malformed: reply bytes")?,
        1_048_576,
    )
    .map_err(|e| e.to_string())?;
    let expected = match query["v"].as_str() {
        Some("openagents.livehost-frame.v1") => "openagents.livehost-frame-result.v1",
        Some("openagents.livehost-media.v1") => "openagents.livehost-media-result.v1",
        Some("openagents.livehost-frame-status.v1") => "openagents.livehost-frame-status-result.v1",
        Some("openagents.livehost-read.v1") => "openagents.livehost-read-result.v1",
        Some("openagents.livehost-operation.v1") => "openagents.livehost-operation-result.v1",
        Some("openagents.livehost-credential.v1") => "openagents.livehost-credential-result.v1",
        Some("openagents.livehost-input-policy.v1") => "openagents.livehost-input-policy-result.v1",
        _ => "openagents.live-result.v1",
    };
    if body["v"] != expected || body["request"] != workspace_host::artifact::json_ref(query)? {
        return Err("identity_mismatch: live reply request".into());
    }
    if expected == "openagents.live-result.v1" {
        nostr::live::validate(&body).map_err(|e| e.to_string())?;
    }
    let refs: Vec<&Value> = body["artifacts"].as_array().into_iter().flatten().collect();
    if query["v"] == "openagents.livehost-read.v1" && body["artifacts"] != query["artifacts"] {
        return Err("identity_mismatch: unrequested artifacts".into());
    }
    if reply.artifacts.values().map(Vec::len).sum::<usize>() > 4 * 1_048_576 {
        return Err("limit_exceeded: reply artifacts".into());
    }
    let mut expected = std::collections::BTreeSet::new();
    for reference in refs {
        let parsed = nostr::contracts::parse_artifact(reference).map_err(|e| e.to_string())?;
        let bytes = reply
            .artifacts
            .get(&parsed.digest)
            .ok_or("content_unavailable: reply artifact")?;
        nostr::contracts::check_artifact_bytes(&parsed, bytes).map_err(|e| e.to_string())?;
        expected.insert(parsed.digest);
    }
    if expected != reply.artifacts.keys().cloned().collect() {
        return Err("identity_mismatch: extra reply artifacts".into());
    }
    Ok(body)
}
