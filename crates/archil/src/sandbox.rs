//! A sandbox: lifecycle, ports, port tokens, network policy and processes.

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::DiskRef;
use crate::error::{Result, SandboxPauseError};
use crate::files::SandboxFiles;
use crate::http::{Http, Req, enc};
use crate::process::{
    ConnectRequest, Connector, ProcessConnectOptions, ProcessResult, ProcessStartOptions,
    SandboxProcess,
};
use crate::retry::RetryMode;

// ------------------------------------------------------------------ network

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxNetworkAction {
    Allow,
    Deny,
}

/// Outbound HTTPS request transformations for an egress rule.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxEgressTransform {
    /// Headers to set, overwriting values supplied by the sandbox.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
}

/// An allow rule with optional transformations and forwarding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxEgressRule {
    /// IPv4 address, CIDR range or domain pattern (`*` wildcards).
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<SandboxEgressTransform>,
    /// Absolute public HTTPS URL receiving this rule's permitted requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forward_url: Option<String>,
}

impl SandboxEgressRule {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            transform: None,
            forward_url: None,
        }
    }
}

/// An `allow` entry: a plain target string or a full rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SandboxEgressAllow {
    Target(String),
    Rule(SandboxEgressRule),
}

impl From<&str> for SandboxEgressAllow {
    fn from(s: &str) -> Self {
        Self::Target(s.to_string())
    }
}
impl From<String> for SandboxEgressAllow {
    fn from(s: String) -> Self {
        Self::Target(s)
    }
}
impl From<SandboxEgressRule> for SandboxEgressAllow {
    fn from(r: SandboxEgressRule) -> Self {
        Self::Rule(r)
    }
}

/// Egress policy: denies win over allows, then the default applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxEgressPolicy {
    pub default: SandboxNetworkAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<SandboxEgressAllow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny: Option<Vec<String>>,
    /// Hosts whose HTTP(S) requests should finish before pausing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drain_on_pause: Option<Vec<String>>,
}

impl SandboxEgressPolicy {
    pub fn new(default: SandboxNetworkAction) -> Self {
        Self {
            default,
            allow: None,
            deny: None,
            drain_on_pause: None,
        }
    }
}

/// Sandbox network policy. An empty policy (`egress: None`) restores
/// unrestricted egress on paid plans.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxNetwork {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub egress: Option<SandboxEgressPolicy>,
    /// True when the policy is Archil's managed free-plan allowlist.
    /// Ignored in requests.
    #[serde(default, skip_serializing)]
    pub managed: Option<bool>,
}

// ------------------------------------------------------------------ mounts

/// One Archil disk to mount inside a sandbox. `path` is the absolute guest
/// directory; it may be omitted only for a sole mount, which lands at
/// `/mnt/archil`. A plain mount is exclusive and `queue_ms` bounds the wait
/// for its root delegation; `read_only` and `conditional` mounts never take it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxMountSpec {
    pub disk: DiskRef,
    pub path: Option<String>,
    pub subdirectory: Option<String>,
    pub read_only: bool,
    pub conditional: bool,
    pub queue_ms: Option<u64>,
}

impl SandboxMountSpec {
    pub fn new(disk: impl Into<DiskRef>) -> Self {
        Self {
            disk: disk.into(),
            path: None,
            subdirectory: None,
            read_only: false,
            conditional: false,
            queue_ms: None,
        }
    }
    pub fn path(mut self, p: impl Into<String>) -> Self {
        self.path = Some(p.into());
        self
    }
    pub fn subdirectory(mut self, p: impl Into<String>) -> Self {
        self.subdirectory = Some(p.into());
        self
    }
    pub fn read_only(mut self, v: bool) -> Self {
        self.read_only = v;
        self
    }
    pub fn conditional(mut self, v: bool) -> Self {
        self.conditional = v;
        self
    }
    pub fn queue_ms(mut self, v: u64) -> Self {
        self.queue_ms = Some(v);
        self
    }

    pub(crate) fn wire(&self) -> SandboxMountWire {
        SandboxMountWire {
            disk_id: self.disk.0.clone(),
            path: self.path.clone(),
            subdirectory: self.subdirectory.clone(),
            read_only: self.read_only,
            conditional: self.conditional,
            queue_ms: self.queue_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SandboxMountWire {
    pub disk_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subdirectory: Option<String>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub conditional: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_ms: Option<u64>,
}

/// A disk mounted inside the sandbox, as currently configured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxMount {
    pub disk_id: String,
    /// Absolute guest directory the disk is mounted at.
    pub path: String,
    pub subdirectory: Option<String>,
    pub read_only: bool,
    pub conditional: bool,
    pub queue_ms: Option<u64>,
}

impl From<SandboxMountWire> for SandboxMount {
    fn from(w: SandboxMountWire) -> Self {
        Self {
            disk_id: w.disk_id,
            path: w.path.unwrap_or_default(),
            subdirectory: w.subdirectory,
            read_only: w.read_only,
            conditional: w.conditional,
            queue_ms: w.queue_ms,
        }
    }
}

// ------------------------------------------------------------------ wire + public state

/// Sandbox lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxStatus {
    Pending,
    Running,
    Pausing,
    Paused,
    Stopping,
    Stopped,
    Exited,
    Failed,
    Deleting,
    Deleted,
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for SandboxStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Pausing => "pausing",
            Self::Paused => "paused",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Exited => "exited",
            Self::Failed => "failed",
            Self::Deleting => "deleting",
            Self::Deleted => "deleted",
            Self::Unknown => "unknown",
        })
    }
}

/// A public hostname for a sandbox port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxEndpoint {
    pub port: u16,
    pub hostname: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SandboxWire {
    pub sandbox_id: String,
    pub name: String,
    pub status: SandboxStatus,
    pub vcpu_count: u32,
    pub mem_size_mib: u32,
    #[serde(default)]
    pub max_ttl_seconds: u64,
    #[serde(default)]
    pub idle_ttl_seconds: Option<u64>,
    #[serde(default)]
    pub max_concurrent_execs: u32,
    #[serde(default)]
    pub base_image: String,
    pub image_digest: Option<String>,
    pub platform: Option<String>,
    pub endpoints: Option<Vec<SandboxEndpoint>>,
    pub mounts: Option<Vec<SandboxMountWire>>,
    pub created_at: DateTime<Utc>,
    pub running_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub last_active_at: DateTime<Utc>,
    pub exit_reason: Option<String>,
    pub checkpoint: Option<String>,
}

/// Serializable snapshot of a sandbox (the TypeScript `SandboxResponse`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxResponse {
    pub id: String,
    pub name: String,
    pub status: SandboxStatus,
    pub vcpu_count: u32,
    pub mem_size_mib: u32,
    pub base_image: String,
    pub image_digest: String,
    pub platform: Option<String>,
    pub max_ttl_seconds: u64,
    pub idle_ttl_seconds: u64,
    pub max_concurrent_execs: u32,
    pub endpoints: Option<Vec<SandboxEndpoint>>,
    pub mounts: Vec<SandboxMount>,
    pub created_at: DateTime<Utc>,
    pub running_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub last_active_at: DateTime<Utc>,
    pub exit_reason: Option<String>,
    pub checkpoint: Option<String>,
}

/// Port-token metadata. The secret and hostname are returned only on creation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SandboxPortToken {
    pub id: String,
    pub port: u16,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// A freshly created port token.
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct CreatedSandboxPortToken {
    #[serde(flatten)]
    pub token_info: SandboxPortToken,
    /// Stable sandbox port hostname, shared with public access.
    pub hostname: String,
    /// Send in `X-Archil-Token`. Save it now; it cannot be retrieved again.
    pub token: String,
}

impl std::fmt::Debug for CreatedSandboxPortToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreatedSandboxPortToken")
            .field("token_info", &self.token_info)
            .field("hostname", &self.hostname)
            .field("token", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
pub struct CreateSandboxPortTokenOptions {
    /// Duration such as `"1h"` or `"30m"`, up to `"8760h"`. Omit for no expiry.
    pub ttl: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ListSandboxPortTokensOptions {
    /// Caps the total returned by `list_port_tokens`.
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SandboxPortTokenPage {
    pub tokens: Vec<SandboxPortToken>,
    pub next_cursor: Option<String>,
}

/// Whether to wait for an operation to finish. Waiting is the default; the SDK
/// polls if the server's wait budget expires first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitOptions {
    pub wait: bool,
}

impl Default for WaitOptions {
    fn default() -> Self {
        Self { wait: true }
    }
}

impl WaitOptions {
    pub fn no_wait() -> Self {
        Self { wait: false }
    }
}

/// Options for [`Sandbox::start_with`].
#[derive(Debug, Clone)]
pub struct SandboxStartOptions {
    pub wait: bool,
    /// Replace the disk mounts for this and later sessions. Cold start only.
    pub mounts: Option<Vec<SandboxMountSpec>>,
}

impl Default for SandboxStartOptions {
    fn default() -> Self {
        Self {
            wait: true,
            mounts: None,
        }
    }
}

/// Options for [`Sandbox::fork_with`].
#[derive(Debug, Clone)]
pub struct SandboxForkOptions {
    pub wait: bool,
    /// Name for the fork; the server generates one when omitted.
    pub name: Option<String>,
}

impl Default for SandboxForkOptions {
    fn default() -> Self {
        Self {
            wait: true,
            name: None,
        }
    }
}

/// Options for [`Sandbox::set_timeout`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SandboxTimeoutOptions {
    /// Hard lifetime in seconds; omit to leave the deadline unchanged.
    pub timeout_seconds: Option<u64>,
    /// Seconds without a direct process connection; zero disables idle expiry.
    pub idle_ttl_seconds: Option<u64>,
}

impl From<u64> for SandboxTimeoutOptions {
    fn from(secs: u64) -> Self {
        Self {
            timeout_seconds: Some(secs),
            idle_ttl_seconds: None,
        }
    }
}

// ------------------------------------------------------------------ Sandbox

/// A sandbox VM. Lifecycle methods update these fields in place, so they
/// take `&mut self`.
#[derive(Debug, Clone)]
pub struct Sandbox {
    pub id: String,
    pub name: String,
    pub status: SandboxStatus,
    pub vcpu_count: u32,
    pub mem_size_mib: u32,
    /// Empty for sandboxes created from `image_id`.
    pub base_image: String,
    /// Digest of the image the sandbox was created from; may be empty.
    pub image_digest: String,
    pub platform: Option<String>,
    pub max_ttl_seconds: u64,
    pub idle_ttl_seconds: u64,
    /// Maximum concurrently attached process sessions.
    pub max_concurrent_execs: u32,
    pub endpoints: Option<Vec<SandboxEndpoint>>,
    /// Disks mounted inside the guest on every boot.
    pub mounts: Vec<SandboxMount>,
    pub created_at: DateTime<Utc>,
    pub running_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub last_active_at: DateTime<Utc>,
    pub exit_reason: Option<String>,
    /// Disk checkpoint the current session leaves behind; present while
    /// pausing, paused, stopping or stopped.
    pub checkpoint: Option<String>,
    http: Arc<Http>,
}

#[derive(Deserialize)]
struct PortList {
    ports: Option<Vec<SandboxEndpoint>>,
}

#[derive(Deserialize)]
struct TokenList {
    tokens: Option<Vec<SandboxPortToken>>,
}

impl Sandbox {
    pub(crate) fn new(data: SandboxWire, http: Arc<Http>) -> Self {
        let mut s = Self {
            id: String::new(),
            name: String::new(),
            status: SandboxStatus::Pending,
            vcpu_count: 0,
            mem_size_mib: 0,
            base_image: String::new(),
            image_digest: String::new(),
            platform: None,
            max_ttl_seconds: 0,
            idle_ttl_seconds: 0,
            max_concurrent_execs: 0,
            endpoints: None,
            mounts: vec![],
            created_at: data.created_at,
            running_at: None,
            finished_at: None,
            last_active_at: data.last_active_at,
            exit_reason: None,
            checkpoint: None,
            http,
        };
        s.apply(data);
        s
    }

    /// Overwrite this sandbox's fields in place from a fresh wire snapshot.
    fn apply(&mut self, d: SandboxWire) {
        self.id = d.sandbox_id;
        self.name = d.name;
        self.status = d.status;
        self.vcpu_count = d.vcpu_count;
        self.mem_size_mib = d.mem_size_mib;
        self.base_image = d.base_image;
        self.image_digest = d.image_digest.unwrap_or_default();
        self.platform = d.platform;
        self.max_ttl_seconds = d.max_ttl_seconds;
        self.idle_ttl_seconds = d.idle_ttl_seconds.unwrap_or(0);
        self.max_concurrent_execs = d.max_concurrent_execs;
        self.endpoints = d.endpoints;
        self.mounts = d
            .mounts
            .unwrap_or_default()
            .into_iter()
            .map(Into::into)
            .collect();
        self.created_at = d.created_at;
        self.running_at = d.running_at;
        self.finished_at = d.finished_at;
        self.last_active_at = d.last_active_at;
        self.exit_reason = d.exit_reason;
        self.checkpoint = d.checkpoint;
    }

    /// Serializable snapshot of the current state.
    pub fn to_response(&self) -> SandboxResponse {
        SandboxResponse {
            id: self.id.clone(),
            name: self.name.clone(),
            status: self.status,
            vcpu_count: self.vcpu_count,
            mem_size_mib: self.mem_size_mib,
            base_image: self.base_image.clone(),
            image_digest: self.image_digest.clone(),
            platform: self.platform.clone(),
            max_ttl_seconds: self.max_ttl_seconds,
            idle_ttl_seconds: self.idle_ttl_seconds,
            max_concurrent_execs: self.max_concurrent_execs,
            endpoints: self.endpoints.clone(),
            mounts: self.mounts.clone(),
            created_at: self.created_at,
            running_at: self.running_at,
            finished_at: self.finished_at,
            last_active_at: self.last_active_at,
            exit_reason: self.exit_reason.clone(),
            checkpoint: self.checkpoint.clone(),
        }
    }

    fn path(&self, suffix: &str) -> String {
        format!("/api/sandboxes/{}{suffix}", enc(&self.id))
    }

    fn connector(&self) -> Connector {
        Connector {
            http: self.http.clone(),
            sandbox_id: self.id.clone(),
        }
    }

    /// File upload and download over the process API.
    pub fn files(&self) -> SandboxFiles {
        SandboxFiles::new(self.clone())
    }

    // ------------------------------------------------------------ processes

    /// Start a process and return its handle without waiting for exit.
    pub async fn run(&self, command: &str, options: ProcessStartOptions) -> Result<SandboxProcess> {
        SandboxProcess::connect(
            self.connector(),
            ConnectRequest::Start {
                command: command.to_string(),
                options,
            },
        )
        .await
    }

    /// Reattach to a process, optionally resuming output from a cursor.
    pub async fn attach(
        &self,
        process_id: &str,
        options: ProcessConnectOptions,
    ) -> Result<SandboxProcess> {
        SandboxProcess::connect(
            self.connector(),
            ConnectRequest::Attach {
                process_id: process_id.to_string(),
                options,
            },
        )
        .await
    }

    /// Run a process and wait for it to exit.
    pub async fn exec(&self, command: &str, options: ProcessStartOptions) -> Result<ProcessResult> {
        let process = self.run(command, options).await?;
        process.wait().await
    }

    // ------------------------------------------------------------ lifecycle

    fn apply_value(&mut self, data: SandboxWire) -> &mut Self {
        self.apply(data);
        self
    }

    /// Re-fetch this sandbox.
    pub async fn refresh(&mut self) -> Result<&mut Self> {
        let data: SandboxWire = self.http.call(Req::get(self.path(""))).await?;
        Ok(self.apply_value(data))
    }

    /// Cold-start this sandbox and wait for it to run.
    pub async fn start(&mut self) -> Result<&mut Self> {
        self.start_with(SandboxStartOptions::default()).await
    }

    pub async fn start_with(&mut self, options: SandboxStartOptions) -> Result<&mut Self> {
        let mut req = Req::new(
            Method::POST,
            self.path("/start"),
            // Replaying a mount update after a lost success conflicts with
            // the now-active sandbox.
            if options.mounts.is_none() {
                RetryMode::Transient
            } else {
                RetryMode::Connect
            },
        )
        .query("wait", options.wait);
        if let Some(mounts) = &options.mounts {
            let wire: Vec<SandboxMountWire> = mounts.iter().map(SandboxMountSpec::wire).collect();
            req = req.json(serde_json::json!({ "mounts": wire }));
        }
        let data: SandboxWire = self.http.call(req).await?;
        self.apply(data);
        if options.wait {
            self.wait_for_start().await?;
        }
        Ok(self)
    }

    /// Stop this sandbox and wait until it has stopped.
    pub async fn stop(&mut self) -> Result<&mut Self> {
        self.stop_with(WaitOptions::default()).await
    }

    pub async fn stop_with(&mut self, options: WaitOptions) -> Result<&mut Self> {
        let req = Req::new(Method::POST, self.path("/stop"), RetryMode::Transient);
        let data: SandboxWire = self.http.call(req).await?;
        self.apply(data);
        if options.wait {
            self.wait_while(SandboxStatus::Stopping).await?;
        }
        Ok(self)
    }

    /// Pause this sandbox, preserving CPU and memory state.
    ///
    /// Fails with [`SandboxPauseError`] if the sandbox leaves `pausing`
    /// without reaching `paused`.
    pub async fn pause(&mut self) -> Result<&mut Self> {
        self.pause_with(WaitOptions::default()).await
    }

    pub async fn pause_with(&mut self, options: WaitOptions) -> Result<&mut Self> {
        let req = Req::new(Method::POST, self.path("/pause"), RetryMode::Transient);
        let data: SandboxWire = self.http.call(req).await?;
        self.apply(data);
        if !options.wait {
            return Ok(self);
        }
        self.wait_while(SandboxStatus::Pausing).await?;
        if self.status != SandboxStatus::Paused {
            return Err(SandboxPauseError {
                latest: Box::new(self.clone()),
            }
            .into());
        }
        Ok(self)
    }

    /// Resume this sandbox from its preserved CPU and memory state.
    pub async fn resume(&mut self) -> Result<&mut Self> {
        self.resume_with(WaitOptions::default()).await
    }

    pub async fn resume_with(&mut self, options: WaitOptions) -> Result<&mut Self> {
        let req = Req::new(Method::POST, self.path("/resume"), RetryMode::Transient)
            .query("wait", options.wait);
        let data: SandboxWire = self.http.call(req).await?;
        self.apply(data);
        if options.wait {
            self.wait_for_start().await?;
        }
        Ok(self)
    }

    /// Create an isolated writable branch from this sandbox's current state.
    pub async fn fork(&mut self) -> Result<Sandbox> {
        self.fork_with(SandboxForkOptions::default()).await
    }

    /// Create an isolated writable branch from this sandbox's current state.
    ///
    /// A running sandbox is paused for the snapshot and resumed once the fork
    /// is accepted; a paused or stopped sandbox is left as it is. The fork
    /// names the checkpoint the pause returned, so it does not depend on the
    /// source still being paused when the request lands. Resuming the source
    /// is best effort: the child is returned even if the source could not be
    /// resumed, so check the source's status afterwards if that matters.
    pub async fn fork_with(&mut self, options: SandboxForkOptions) -> Result<Sandbox> {
        // Pause is idempotent: "pausing" means the sandbox was live and is
        // ours to resume; "paused" or "stopped" means it was already inactive.
        let req = Req::new(Method::POST, self.path("/pause"), RetryMode::Transient);
        let paused: SandboxWire = self.http.call(req).await?;
        self.apply(paused);
        let resume_after_fork = self.status == SandboxStatus::Pausing;
        let checkpoint = self.checkpoint.clone();

        // A source we paused resumes as soon as the fork is accepted, not
        // after the child boots.
        let wait = if resume_after_fork {
            false
        } else {
            options.wait
        };
        let created: Result<Sandbox> = async {
            self.wait_while(SandboxStatus::Pausing).await?;
            let mut body = serde_json::Map::new();
            if let Some(n) = &options.name {
                body.insert("name".into(), n.clone().into());
            }
            if let Some(c) = &checkpoint {
                body.insert("checkpoint".into(), c.clone().into());
            }
            let req = Req::new(Method::POST, self.path("/fork"), RetryMode::Connect)
                .query("wait", wait)
                .json(serde_json::Value::Object(body));
            let data: SandboxWire = self.http.call(req).await?;
            Ok(Sandbox::new(data, self.http.clone()))
        }
        .await;
        if resume_after_fork {
            let _ = self.resume_with(WaitOptions::no_wait()).await;
        }
        let mut fork = created?;
        if !options.wait {
            return Ok(fork);
        }
        fork.wait_for_start().await?;
        if resume_after_fork {
            let _ = self.wait_for_start().await;
        }
        Ok(fork)
    }

    // ------------------------------------------------------------ ports

    /// Expose a TCP port publicly (1-65535), returning its hostname.
    pub async fn expose_port(&self, port: u16) -> Result<String> {
        let req = Req::new(
            Method::PUT,
            self.path(&format!("/ports/{port}")),
            RetryMode::Transient,
        );
        let endpoint: SandboxEndpoint = self.http.call(req).await?;
        Ok(endpoint.hostname)
    }

    /// List explicitly exposed public ports. Service-published ports are in `endpoints`.
    pub async fn list_ports(&self) -> Result<Vec<SandboxEndpoint>> {
        let data: PortList = self.http.call(Req::get(self.path("/ports"))).await?;
        Ok(data.ports.unwrap_or_default())
    }

    /// Remove explicit public exposure. A service publishing the same port stays reachable.
    pub async fn unexpose_port(&self, port: u16) -> Result<()> {
        let req = Req::new(
            Method::DELETE,
            self.path(&format!("/ports/{port}")),
            RetryMode::Transient,
        );
        self.http.call_empty(req).await
    }

    /// Authorize HTTP access to one port without making it public.
    pub async fn create_port_token(
        &self,
        port: u16,
        options: CreateSandboxPortTokenOptions,
    ) -> Result<CreatedSandboxPortToken> {
        let mut body = serde_json::json!({ "port": port });
        if let Some(ttl) = options.ttl {
            body["ttl"] = ttl.into();
        }
        let req = Req::new(Method::POST, self.path("/port-tokens"), RetryMode::Connect).json(body);
        self.http.call(req).await
    }

    /// Get token metadata. Expired and revoked tokens return not found.
    pub async fn get_port_token(&self, token_id: &str) -> Result<SandboxPortToken> {
        self.http
            .call(Req::get(
                self.path(&format!("/port-tokens/{}", enc(token_id))),
            ))
            .await
    }

    /// List token metadata across pages; `limit` caps the total returned.
    pub async fn list_port_tokens(
        &self,
        options: ListSandboxPortTokensOptions,
    ) -> Result<Vec<SandboxPortToken>> {
        let mut tokens = Vec::new();
        let mut cursor = options.cursor;
        loop {
            let remaining = options.limit.map(|l| l.saturating_sub(tokens.len()));
            if remaining == Some(0) {
                return Ok(tokens);
            }
            let page = self
                .list_port_tokens_page(ListSandboxPortTokensOptions {
                    limit: Some(remaining.map_or(100, |r| r.min(100))),
                    cursor: cursor.clone(),
                })
                .await?;
            tokens.extend(page.tokens);
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => return Ok(tokens),
            }
        }
    }

    /// Fetch one page of token metadata; pass `next_cursor` back as `cursor`.
    pub async fn list_port_tokens_page(
        &self,
        options: ListSandboxPortTokensOptions,
    ) -> Result<SandboxPortTokenPage> {
        let req = Req::get(self.path("/port-tokens"))
            .query("limit", options.limit.unwrap_or(100))
            .query_opt("cursor", options.cursor);
        let (data, next): (TokenList, Option<String>) = self.http.call_page(req).await?;
        Ok(SandboxPortTokenPage {
            tokens: data.tokens.unwrap_or_default(),
            next_cursor: next.filter(|c| !c.is_empty()),
        })
    }

    /// Revoke a token for new connections. Existing connections remain open.
    pub async fn delete_port_token(&self, token_id: &str) -> Result<()> {
        let req = Req::new(
            Method::DELETE,
            self.path(&format!("/port-tokens/{}", enc(token_id))),
            RetryMode::Transient,
        );
        self.http.call_empty(req).await
    }

    // ------------------------------------------------------------ network, timeouts, delete

    /// Get the sandbox's effective network policy.
    pub async fn get_network(&self) -> Result<SandboxNetwork> {
        self.http.call(Req::get(self.path("/network"))).await
    }

    /// Replace a running sandbox's complete network policy and return the
    /// effective policy.
    pub async fn update_network(&self, network: &SandboxNetwork) -> Result<SandboxNetwork> {
        self.http
            .call(Req::new(Method::PUT, self.path("/network"), RetryMode::Transient).json(network))
            .await
    }

    /// Update either or both TTLs in seconds. Omitted settings remain unchanged.
    pub async fn set_timeout(
        &mut self,
        timeout: impl Into<SandboxTimeoutOptions>,
    ) -> Result<&mut Self> {
        let options = timeout.into();
        let mut body = serde_json::Map::new();
        if let Some(t) = options.timeout_seconds {
            body.insert("timeout".into(), t.into());
        }
        if let Some(t) = options.idle_ttl_seconds {
            body.insert("idle_ttl_seconds".into(), t.into());
        }
        let req = Req::new(Method::POST, self.path("/timeout"), RetryMode::Transient)
            .json(serde_json::Value::Object(body));
        let data: SandboxWire = self.http.call(req).await?;
        self.apply(data);
        Ok(self)
    }

    /// Delete this sandbox and its backing disk.
    pub async fn delete(&self) -> Result<()> {
        self.http
            .call_empty(Req::new(
                Method::DELETE,
                self.path(""),
                RetryMode::Transient,
            ))
            .await
    }

    // ------------------------------------------------------------ waiting

    /// Continue waiting if the server returned before startup completed.
    pub async fn wait_for_start(&mut self) -> Result<&mut Self> {
        self.wait_while(SandboxStatus::Pending).await?;
        Ok(self)
    }

    async fn wait_while(&mut self, status: SandboxStatus) -> Result<()> {
        while self.status == status {
            tokio::time::sleep(self.http.poll_interval).await;
            self.refresh().await?;
        }
        Ok(())
    }
}
