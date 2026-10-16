//! Bounded local transport for the resident managed-session host.
use crate::{
    Result,
    host::{Binding, Bundle, SessionHost},
    transport::Reply,
};
use nostr::domain::Event;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
// An admitted operation has up to 300 seconds; incomplete frames keep their
// independent five-second I/O limit.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(310);

pub(crate) fn claim(root: &Path) -> Result<File> {
    let owner = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(root.join("runtime.lock"))
        .map_err(|e| e.to_string())?;
    owner
        .try_lock()
        .map_err(|_| "conflict: a resident already owns these engines")?;
    Ok(owner)
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Version {
    #[serde(rename = "openagents.session-host-transport.v1")]
    V1,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub v: Version,
    pub id: String,
    pub operation: Operation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Execute {
        event: Event,
        artifacts: BTreeMap<String, Vec<u8>>,
        binding: Binding,
    },
    Poll {
        event: Event,
    },
    Continue {
        event: Event,
        bundle: Bundle,
    },
    Submit {
        bundle: Bundle,
    },
    Read {
        event: Event,
    },
    ChildPolicy {
        event: Event,
    },
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
    Ok { reply: Body },
    Error { code: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Body {
    Event {
        event: Event,
        artifacts: BTreeMap<String, Vec<u8>>,
    },
    Control {
        result: Value,
    },
}
impl From<Reply> for Body {
    fn from((event, artifacts): Reply) -> Self {
        Self::Event { event, artifacts }
    }
}
impl Request {
    pub fn new(operation: Operation) -> Self {
        Self {
            v: Version::V1,
            id: workspace_host::artifact::token(),
            operation,
        }
    }
    fn dispatch(self, host: &SessionHost, now: u64) -> Response {
        let result = if self.id.len() != 64
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            Err("malformed: transport request ID".into())
        } else {
            match self.operation {
                Operation::Execute {
                    event,
                    artifacts,
                    binding,
                } => host
                    .serve(&event, &artifacts, &binding, now)
                    .map(Body::from),
                Operation::Poll { event } => host.poll(&event, now).map(Body::from),
                Operation::Continue { event, bundle } => host
                    .continue_turn(&event, &bundle, now)
                    .map(|result| Body::Control { result }),
                Operation::Submit { bundle } => host
                    .submit(&bundle, None, now)
                    .map(|result| Body::Control { result }),
                Operation::Read { event } => host.read(&event, now).map(Body::from),
                Operation::ChildPolicy { event } => host
                    .child_policy(&event, now)
                    .map(|result| Body::Control { result }),
            }
        };
        Response {
            v: Version::V1,
            id: self.id,
            outcome: match result {
                Ok(reply) => Outcome::Ok { reply },
                Err(error) => Outcome::Error {
                    code: crate::host::code(&error).into(),
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
    host: &'a SessionHost,
    listener: UnixListener,
    clients: Vec<Client>,
    path: PathBuf,
    identity: (u64, u64),
    _owner: File,
}
impl<'a> Server<'a> {
    pub fn bind(host: &'a SessionHost, path: PathBuf) -> Result<Self> {
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
        })
    }

    /// Advance the watchdog and bounded connection I/O once.
    pub fn step(&mut self, now: u64) -> Result<()> {
        let started = Instant::now();
        let host = self.host;
        host.advance(now)?;
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
    fn advance(&mut self, host: &SessionHost, now: u64) -> Result<bool> {
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
            .ok_or("unavailable: session host timed out")?;
        stream
            .set_write_timeout(Some(remaining))
            .map_err(|e| e.to_string())?;
        let n = stream.write(&bytes[sent..]).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("unavailable: session host disconnected".into());
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
            .ok_or("unavailable: session host timed out")?;
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|e| e.to_string())?;
        let n = stream.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("unavailable: session host disconnected".into());
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
                serde_json::from_value(value).map_err(|_| "malformed: session host reply")?;
            if response.id != request.id {
                return Err("identity_mismatch: session reply ID".into());
            }
            return Ok(response);
        }
    }
}

/// Verify a private read reply before using its session state or artifact bytes.
pub fn read_reply(
    reply: &Reply,
    host: &str,
    secret: &secp256k1::SecretKey,
    query: &Value,
) -> Result<Value> {
    let opened = nostr::private_artifact::open(&reply.0, secret).map_err(|e| e.to_string())?;
    let caller = secret
        .x_only_public_key(&secp256k1::Secp256k1::new())
        .0
        .to_string();
    if opened.signer() != host || opened.recipient() != caller {
        return Err("identity_mismatch: session read reply identity".into());
    }
    let body = nostr::session::parse(opened.inline_bytes().ok_or("malformed: read reply bytes")?)
        .map_err(|e| e.to_string())?;
    if body["v"] != "openagents.session-read-result.v1"
        || body["scope"] != query["scope"]
        || body["query"] != workspace_host::artifact::json_ref(query)?
    {
        return Err("identity_mismatch: session read reply query".into());
    }
    let refs = body["artifacts"].as_array().ok_or("artifacts")?;
    if !refs.contains(&body["state"])
        || reply.1.values().map(Vec::len).sum::<usize>() > 4 * 1_048_576
    {
        return Err("limit_exceeded: session read reply artifacts".into());
    }
    let mut expected = std::collections::BTreeSet::new();
    for reference in refs {
        if *reference != body["state"]
            && !query["artifacts"]
                .as_array()
                .is_some_and(|r| r.contains(reference))
        {
            return Err("identity_mismatch: unrequested read artifact".into());
        }
        let digest = reference["digest"].as_str().ok_or("digest")?;
        expected.insert(digest.to_string());
        let bytes = reply
            .1
            .get(digest)
            .ok_or("content_unavailable: read artifact")?;
        if workspace_host::artifact::reference(
            bytes,
            reference["media_type"].as_str().ok_or("media_type")?,
            reference["schema"].as_str(),
        ) != *reference
        {
            return Err("identity_mismatch: read artifact bytes".into());
        }
    }
    if reply
        .1
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        != expected
    {
        return Err("identity_mismatch: unexpected read artifacts".into());
    }
    let state = nostr::session::verify(
        &body["state"],
        &reply.1[body["state"]["digest"].as_str().ok_or("state")?],
        "openagents.session-state.v1",
    )
    .map_err(|e| e.to_string())?;
    if state["scope"] != query["scope"] {
        return Err("identity_mismatch: read state scope".into());
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoBindings;
    impl crate::host::Bindings for NoBindings {
        fn check(&self, _: &str, _: &str, _: u64) -> Result<()> {
            Err("not_admitted: fixture has no execution".into())
        }
    }
    #[test]
    fn socket_ownership_and_partial_clients_do_not_replace_or_block_the_host() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("state");
        let key = secp256k1::SecretKey::from_byte_array([90; 32]).unwrap();
        SessionHost::initialize(root.clone(), key, crate::host::Policy::default()).unwrap();
        let host = SessionHost::open(root.clone(), key, vec![], Box::new(NoBindings)).unwrap();
        let path = root.join("host.sock");
        let mut server = Server::bind(&host, path.clone()).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(Server::bind(&host, root.join("other.sock")).is_err());
        assert!(claim(&root).is_err());
        let mut partial = UnixStream::connect(&path).unwrap();
        partial.write_all(b"{").unwrap();
        let event = workspace_host::host::request(
            &key,
            host.pubkey(),
            serde_json::json!({"v":"unsupported"}),
            crate::now(),
        )
        .unwrap();
        let mut request = Request::new(Operation::Poll { event });
        request.id = "invalid".into();
        let client_path = path.clone();
        let client = std::thread::spawn(move || super::request(&client_path, &request));
        let started = Instant::now();
        while !client.is_finished() {
            assert!(started.elapsed() < Duration::from_secs(3));
            server.step(crate::now()).unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            matches!(client.join().unwrap().unwrap().outcome,Outcome::Error {code} if code=="malformed")
        );
        assert!(partial.take_error().unwrap().is_none());
        fs::remove_file(&path).unwrap();
        fs::write(&path, b"keep this file").unwrap();
        drop(server);
        assert_eq!(fs::read(&path).unwrap(), b"keep this file");
        assert!(Server::bind(&host, path.clone()).is_err());
        fs::remove_file(&path).unwrap();
        let stale = UnixListener::bind(&path).unwrap();
        drop(stale);
        let restarted = Server::bind(&host, path.clone()).unwrap();
        drop(restarted);
        assert!(!path.exists());
    }
}
