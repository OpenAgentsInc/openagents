//! Explicit tenant bindings to resident host grants. The site owns no task store.

use super::private::ProtectedFile;
use super::session::{SessionError, Viewer, now};
use coder_access::protocol::{Operation, Outcome};
use coder_access::{Access, RelayPolicy, Right};
use coder_host::client::{Device, Link, WebSocketTls, connect_websocket};
use secp256k1::SecretKey;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const UNAVAILABLE: &str = "The explicit resident host connection is unavailable.";
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema: String,
    bindings: Vec<Declared>,
    #[serde(default)]
    controls: Option<Controls>,
}

#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Controls {
    directory: PathBuf,
    bindings: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declared {
    id: String,
    account: String,
    workspace: String,
    members_epoch: u64,
    host_workspace: String,
    host_generation: u64,
    route: String,
    access_file: PathBuf,
    device_secret: PathBuf,
    #[serde(default)]
    browser: Option<Browser>,
    /// The former Verse world directory. Worlds left with the Cloud pages
    /// (docs/web/cloud-reset.md); an existing entry is accepted and ignored.
    #[serde(default, rename = "world")]
    _world: Option<serde::de::IgnoredAny>,
}

/// Public transport hints for a separately enrolled page device.
#[derive(Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Browser {
    route: Option<String>,
    capabilities: Vec<String>,
}

enum Route {
    Local(SocketAddr),
    WebSocket(String),
}

/// A provisioned device remains in its protected native adapter.
pub struct Binding {
    id: String,
    account: String,
    workspace: String,
    members_epoch: u64,
    host_workspace: String,
    generation: u64,
    identity: String,
    route: Route,
    device: Arc<Device>,
    files: [ProtectedFile; 2],
    browser: Option<Browser>,
    loopback: bool,
}

/// Operator-provisioned read bindings; account sign-in never creates one.
pub struct Hosts {
    config: ProtectedFile,
    bindings: Vec<Binding>,
    effects: Option<super::effects::Effects>,
    control_bindings: Vec<String>,
    control_policy: String,
}

impl Hosts {
    pub fn load(path: &Path) -> Result<Self, String> {
        let (config, bytes) = ProtectedFile::open(path, 64 * 1024)?;
        let declared: Configuration = serde_json::from_slice(&bytes).map_err(|_| UNAVAILABLE)?;
        if declared.schema != "openagents.cloud.host-bindings.v1" || declared.bindings.len() > 64 {
            return Err(UNAVAILABLE.into());
        }
        let mut bindings = Vec::new();
        for declared in declared.bindings {
            if bindings.iter().any(|b: &Binding| b.id == declared.id) {
                return Err(UNAVAILABLE.into());
            }
            bindings.push(Binding::load(declared)?);
        }
        let mut effects = None;
        let mut control_bindings = Vec::new();
        let mut control_policy = String::new();
        if let Some(controls) = declared.controls {
            if controls.bindings.len() > 64
                || controls.bindings.iter().enumerate().any(|(index, id)| {
                    !bindings.iter().any(|b| b.id == *id) || controls.bindings[..index].contains(id)
                })
            {
                return Err(UNAVAILABLE.into());
            }
            control_policy = digest(&serde_json::to_value(&controls).map_err(|_| UNAVAILABLE)?);
            effects =
                Some(super::effects::Effects::open(&controls.directory).map_err(|_| UNAVAILABLE)?);
            control_bindings = controls.bindings;
        }
        Ok(Self {
            config,
            bindings,
            effects,
            control_bindings,
            control_policy,
        })
    }

    /// Resolve only the current native account, workspace, and membership epoch.
    pub(crate) fn get(&self, viewer: &Viewer, id: &str) -> Result<&Binding, SessionError> {
        self.config.check().map_err(|_| SessionError::Unavailable)?;
        let binding = self
            .bindings
            .iter()
            .find(|b| b.id == id)
            .ok_or(SessionError::Forbidden)?;
        binding.admit(viewer)?;
        Ok(binding)
    }

    /// Find a binding by ID without account admission, for ticketed reads.
    pub(crate) fn find(&self, id: &str) -> Result<&Binding, SessionError> {
        self.config.check().map_err(|_| SessionError::Unavailable)?;
        self.bindings
            .iter()
            .find(|b| b.id == id)
            .ok_or(SessionError::Forbidden)
    }

    pub(crate) fn current<'a>(&'a self, viewer: &Viewer) -> Vec<&'a Binding> {
        if self.config.check().is_err() {
            return Vec::new();
        }
        self.bindings
            .iter()
            .filter(|binding| binding.admit(viewer).is_ok())
            .collect()
    }

    pub(crate) fn effects(
        &self,
        viewer: &Viewer,
        id: &str,
    ) -> Result<&super::effects::Effects, SessionError> {
        self.get(viewer, id)?;
        if !self.control_bindings.iter().any(|allowed| allowed == id) {
            return Err(SessionError::Forbidden);
        }
        self.effects.as_ref().ok_or(SessionError::Forbidden)
    }

    pub(crate) fn control_scope(&self, viewer: &Viewer, binding: &Binding) -> serde_json::Value {
        let mut standing = super::session::standing_value(viewer);
        standing
            .as_object_mut()
            .expect("standing is an object")
            .remove("expires_at");
        serde_json::json!({"account_standing":standing,"binding":binding.identity(),"binding_id":binding.id(),"host":binding.host(),"host_generation":binding.generation(),"host_workspace":binding.workspace(),"policy":self.control_policy,"native_grant":digest(&serde_json::to_value(&binding.access().grant).expect("grant serializes"))})
    }
}

impl Binding {
    fn load(declared: Declared) -> Result<Self, String> {
        let query = coder_access::task_read::ListQuery {
            workspace: declared.host_workspace.clone(),
            cursor: None,
            limit: 1,
        };
        if !valid_id(&declared.id)
            || !scope_id(&declared.account)
            || !scope_id(&declared.workspace)
            || query.validate().is_err()
        {
            return Err(UNAVAILABLE.into());
        }
        if declared.members_epoch > 9_007_199_254_740_991
            || declared.host_generation == 0
            || declared.host_generation > 9_007_199_254_740_991
        {
            return Err(UNAVAILABLE.into());
        }
        let (route, policy) = route(&declared.route)?;
        let (access_file, bytes) = ProtectedFile::open(&declared.access_file, 64 * 1024)?;
        let access = Access::parse(&bytes).map_err(|_| UNAVAILABLE)?;
        let (secret_file, mut bytes) = ProtectedFile::open(&declared.device_secret, 128)?;
        let secret = parse_secret(&bytes);
        bytes.fill(0);
        let secret = secret?;
        access
            .verify(&secret, now(), policy)
            .map_err(|_| UNAVAILABLE)?;
        if !access.grant.rights.contains(Right::Observe) {
            return Err(UNAVAILABLE.into());
        }
        let local_observer = matches!(route, Route::Local(_));
        if let Some(browser) = &declared.browser {
            browser.validate(&access.grant.relay, local_observer)?;
        }
        let loopback = declared.browser.as_ref().is_some_and(|browser| {
            std::iter::once(access.grant.relay.as_str())
                .chain(browser.route.as_deref())
                .any(|value| value.starts_with("ws://"))
        });
        let identity = digest(&serde_json::json!({
            "world":serde_json::Value::Null,
            "binding":declared.id, "account":declared.account,
            "workspace":declared.workspace,"members_epoch":declared.members_epoch,
            "host":access.grant.host,"generation":declared.host_generation,
            "host_workspace":declared.host_workspace,"device":access.grant.device,
            "grant":access.grant.grant,"epoch":access.grant.epoch,
            "browser":declared.browser
        }));
        let device = Device::new(access, secret, policy).map_err(|_| UNAVAILABLE)?;
        Ok(Self {
            id: declared.id,
            account: declared.account,
            workspace: declared.workspace,
            members_epoch: declared.members_epoch,
            host_workspace: declared.host_workspace,
            generation: declared.host_generation,
            identity,
            route,
            device: Arc::new(device),
            files: [access_file, secret_file],
            browser: declared.browser,
            loopback,
        })
    }

    fn admit(&self, viewer: &Viewer) -> Result<(), SessionError> {
        let Some(workspace) = &viewer.workspace else {
            return Err(SessionError::Forbidden);
        };
        if viewer.account_id != self.account
            || workspace.id != self.workspace
            || workspace.members_epoch != self.members_epoch
            || viewer.expires_at <= now()
            || self.device.access().grant.expires_at <= now()
        {
            return Err(SessionError::Forbidden);
        }
        for file in &self.files {
            file.check().map_err(|_| SessionError::Unavailable)?;
        }
        Ok(())
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }
    pub(crate) fn workspace(&self) -> &str {
        &self.host_workspace
    }
    pub(crate) fn host(&self) -> &str {
        self.device.host()
    }
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
    pub(crate) fn identity(&self) -> &str {
        &self.identity
    }

    pub(crate) fn account(&self) -> &str {
        &self.account
    }
    pub(crate) fn account_workspace(&self) -> &str {
        &self.workspace
    }
    pub(crate) fn members_epoch(&self) -> u64 {
        self.members_epoch
    }
    /// Whether the resident route is a loopback channel rather than a relay.
    pub(crate) fn direct(&self) -> bool {
        matches!(self.route, Route::Local(_))
    }
    pub(crate) fn capabilities(&self) -> &[String] {
        self.browser
            .as_ref()
            .map_or(&[], |browser| browser.capabilities.as_slice())
    }

    pub(crate) fn browser_config(
        &self,
        viewer: &Viewer,
    ) -> Result<serde_json::Value, SessionError> {
        self.admit(viewer)?;
        let browser = self.browser.as_ref().ok_or(SessionError::Unavailable)?;
        Ok(serde_json::json!({
            "host":self.host(),"generation":self.generation(),
            "workspace":self.workspace(),"relay":self.device.relay(),
            "route":browser.route,"capabilities":browser.capabilities,
            "loopback":self.loopback
        }))
    }

    pub(crate) fn browser_origins(&self) -> Vec<String> {
        let Some(browser) = &self.browser else {
            return Vec::new();
        };
        std::iter::once(self.device.relay())
            .chain(browser.route.as_deref())
            .filter_map(|value| url::Url::parse(value).ok())
            .map(|url| url.origin().ascii_serialization())
            .collect()
    }

    pub(crate) fn access(&self) -> &Access {
        self.device.access()
    }

    pub(crate) fn prepare(
        &self,
        viewer: &Viewer,
        operation: Operation,
        id: String,
    ) -> Result<coder_access::client::Pending, SessionError> {
        self.admit(viewer)?;
        if operation
            .required()
            .is_none_or(|right| !self.access().grant.rights.contains(right))
        {
            return Err(SessionError::Forbidden);
        }
        self.device
            .prepare_operation(operation, id)
            .map_err(native_error)
    }

    pub(crate) async fn send(
        &self,
        viewer: &Viewer,
        pending: &coder_access::client::Pending,
    ) -> Result<Outcome, SessionError> {
        self.admit(viewer)?;
        self.device
            .validate_pending(pending)
            .map_err(native_error)?;
        let link = self.connect().await?;
        let result = tokio::time::timeout(TIMEOUT, link.send_pending(pending))
            .await
            .map_err(|_| SessionError::Unavailable)?
            .map_err(native_error)?;
        self.admit(viewer)?;
        Ok(result)
    }

    async fn connect(&self) -> Result<Link, SessionError> {
        match &self.route {
            Route::Local(address) => {
                let stream = tokio::time::timeout(TIMEOUT, tokio::net::TcpStream::connect(address))
                    .await
                    .map_err(|_| SessionError::Unavailable)?
                    .map_err(|_| SessionError::Unavailable)?;
                Link::direct(
                    self.device.clone(),
                    stream,
                    address.to_string(),
                    self.generation,
                    TIMEOUT,
                )
                .await
            }
            Route::WebSocket(url) => {
                let stream = connect_websocket(url, &WebSocketTls::webpki(), TIMEOUT)
                    .await
                    .map_err(|_| SessionError::Unavailable)?;
                Link::direct(
                    self.device.clone(),
                    stream,
                    url.clone(),
                    self.generation,
                    TIMEOUT,
                )
                .await
            }
        }
        .map_err(native_error)
    }

    /// Release the account's own Claude credential for one turn of its job
    /// (BYO-05) over a new authenticated channel. Unlike an effect it is
    /// never staged in the effect book, and the resident retains neither the
    /// request nor its reply.
    pub(crate) async fn release(
        &self,
        viewer: &Viewer,
        operation: Operation,
    ) -> Result<Outcome, SessionError> {
        self.admit(viewer)?;
        if !matches!(operation, Operation::CloudRelease { .. })
            || !self.access().grant.rights.contains(Right::Operate)
        {
            return Err(SessionError::Forbidden);
        }
        let link = self.connect().await?;
        let answer = tokio::time::timeout(TIMEOUT, link.call(operation))
            .await
            .map_err(|_| SessionError::Unavailable)?
            .map_err(native_error)?;
        self.admit(viewer)?;
        Ok(answer)
    }

    /// One read opens a new authenticated channel and rechecks native authority.
    pub(crate) async fn read(
        &self,
        viewer: &Viewer,
        operation: Operation,
    ) -> Result<Outcome, SessionError> {
        self.admit(viewer)?;
        if !operation.reads_only()
            || operation.required() != Some(Right::Observe)
            || !matches!(
                operation,
                Operation::ListTasks { .. }
                    | Operation::ReadTask { .. }
                    | Operation::ReadTaskOriginal { .. }
                    | Operation::ListWorkspaces {}
                    | Operation::ReviewTask { .. }
                    | Operation::RequestOperation { .. }
                    | Operation::ProjectList { .. }
                    | Operation::ProjectRead { .. }
                    | Operation::ProjectOriginal { .. }
                    | Operation::CloudProjects { .. }
                    | Operation::CloudCatalog { .. }
                    | Operation::CloudList { .. }
                    | Operation::CloudRead { .. }
                    | Operation::CloudOriginal { .. }
                    | Operation::EnvironmentRead { .. }
                    | Operation::EnvironmentEvidence { .. }
                    | Operation::ListAgents {}
                    | Operation::ListAgentJobs { .. }
                    | Operation::ListAgentMemory { .. }
                    | Operation::StudioSnapshot {}
                    | Operation::StudioUpdate { .. }
                    | Operation::OpenReview { .. }
            )
        {
            return Err(SessionError::Forbidden);
        }
        let link = self.connect().await?;
        let answer = tokio::time::timeout(TIMEOUT, link.call(operation))
            .await
            .map_err(|_| SessionError::Unavailable)?
            .map_err(native_error)?;
        self.admit(viewer)?;
        if serde_json::to_vec(&answer)
            .map_err(|_| SessionError::Conflict)?
            .len()
            > 64 * 1024
        {
            return Err(SessionError::Conflict);
        }
        Ok(answer)
    }

    pub(crate) async fn read_queue(
        &self,
        viewer: &Viewer,
        operation: Operation,
    ) -> Result<Outcome, SessionError> {
        self.admit(viewer)?;
        if !matches!(
            &operation,
            Operation::QueueTaskAtRevision {
                edit: coder_access::protocol::QueueEdit::List {},
                ..
            }
        ) {
            return Err(SessionError::Forbidden);
        }
        let pending = self.prepare(viewer, operation, coder_access::protocol::random_id())?;
        self.send(viewer, &pending).await
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

impl Browser {
    fn validate(&self, relay: &str, loopback: bool) -> Result<(), String> {
        browser_url(relay, loopback)?;
        if let Some(route) = &self.route {
            browser_url(route, loopback)?;
        }
        if self.capabilities.len() > 32
            || self.capabilities.iter().enumerate().any(|(index, value)| {
                value.is_empty()
                    || value.len() > 128
                    || !value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-:".contains(&b))
                    || self.capabilities[..index].contains(value)
            })
        {
            return Err(UNAVAILABLE.into());
        }
        Ok(())
    }
}

fn browser_url(value: &str, loopback: bool) -> Result<(), String> {
    let url = url::Url::parse(value).map_err(|_| UNAVAILABLE)?;
    let local = url.host_str().is_some_and(|host| {
        host.trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
    });
    if value.len() > 2048
        || url.host_str().is_none()
        || !(url.scheme() == "wss" || loopback && local && url.scheme() == "ws")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || value
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
    {
        return Err(UNAVAILABLE.into());
    }
    Ok(())
}

fn scope_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

fn route(value: &str) -> Result<(Route, RelayPolicy), String> {
    if let Some(address) = value.strip_prefix("tcp://") {
        let address: SocketAddr = address.parse().map_err(|_| UNAVAILABLE)?;
        if address.ip().is_loopback() && address.port() != 0 {
            return Ok((Route::Local(address), RelayPolicy::LoopbackTest));
        }
        return Err(UNAVAILABLE.into());
    }
    let url = url::Url::parse(value).map_err(|_| UNAVAILABLE)?;
    if value.len() > 2048
        || url.scheme() != "wss"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || value
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
    {
        return Err(UNAVAILABLE.into());
    }
    Ok((Route::WebSocket(value.into()), RelayPolicy::Production))
}

fn parse_secret(bytes: &[u8]) -> Result<SecretKey, String> {
    if bytes.len() == 32 {
        return SecretKey::from_byte_array(bytes.try_into().map_err(|_| UNAVAILABLE)?)
            .map_err(|_| UNAVAILABLE.into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| UNAVAILABLE)?.trim();
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(UNAVAILABLE.into());
    }
    let mut key = [0; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).map_err(|_| UNAVAILABLE)?;
    }
    let secret = SecretKey::from_byte_array(key).map_err(|_| UNAVAILABLE.into());
    key.fill(0);
    secret
}

fn native_error(error: coder_host::Error) -> SessionError {
    match error {
        coder_host::Error::Access(error) => match error.code {
            coder_access::Code::Revoked
            | coder_access::Code::Expired
            | coder_access::Code::Forbidden
            | coder_access::Code::MissingRight => SessionError::Forbidden,
            coder_access::Code::Bounds | coder_access::Code::Malformed => {
                SessionError::InvalidRequest
            }
            coder_access::Code::Conflict | coder_access::Code::Stale => SessionError::Conflict,
            _ => SessionError::Unavailable,
        },
        _ => SessionError::Unavailable,
    }
}

fn digest(value: &serde_json::Value) -> String {
    format!(
        "sha256:{}",
        Sha256::digest(value.to_string().as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_are_explicit_and_cannot_use_ambient_credentials() {
        assert!(route("tcp://127.0.0.1:4567").is_ok());
        assert!(route("tcp://[::1]:4567").is_ok());
        assert!(route("wss://host.example/").is_ok());
        for value in [
            "tcp://192.168.1.10:4567",
            "tcp://localhost:4567",
            "tcp://127.0.0.1:0",
            "ws://127.0.0.1:4567",
            "https://host.example",
            "wss://user:secret@host.example",
            "wss://host.example/?token=x",
            "wss://host.example/#x",
            "wss://host.example/tasks",
        ] {
            assert!(route(value).is_err());
        }
    }
}
