//! The [`Archil`] client and its configuration.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT as UA_HEADER};

use crate::disk::Disk;
use crate::disks::Disks;
use crate::error::{Error, Result};
use crate::http::{Http, Req};
use crate::images::Images;
use crate::regions::{derive_s3_base_url, resolve_base_url};
use crate::retry::RetryMode;
use crate::sandboxes::Sandboxes;
use crate::tokens::Tokens;
use crate::types::{ExecDiskResult, ExecMountWire, ExecRequest};
use crate::workspace::Workspace;

/// SDK version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// `User-Agent` sent on every control-plane request.
pub const USER_AGENT: &str = concat!("archil-rs/", env!("CARGO_PKG_VERSION"));

/// Reference to a disk by id or by an already-fetched [`Disk`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskRef(pub String);

impl DiskRef {
    pub fn id(&self) -> &str {
        &self.0
    }
}
impl From<&str> for DiskRef {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}
impl From<String> for DiskRef {
    fn from(s: String) -> Self {
        Self(s)
    }
}
impl From<&String> for DiskRef {
    fn from(s: &String) -> Self {
        Self(s.clone())
    }
}
impl From<&Disk> for DiskRef {
    fn from(d: &Disk) -> Self {
        Self(d.id.clone())
    }
}
impl From<Disk> for DiskRef {
    fn from(d: Disk) -> Self {
        Self(d.id.clone())
    }
}

/// Options for one disk in an exec request ([`ExecMountSpec`] in the TS SDK).
///
/// Use this form to pin the mount to a subdirectory, mount it read-only or in
/// conditional mode, or request delegation checkouts. For the default case
/// (disk root, read-write) pass a plain disk to [`ExecMount::from`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecMountSpec {
    pub disk: DiskRef,
    /// Relative subdirectory to expose; no `.` or `..` segments.
    pub subdirectory: Option<String>,
    pub read_only: bool,
    /// Send mutations directly to the server without a delegation checkout.
    pub conditional: bool,
    /// Milliseconds to wait in the delegation queue for each checkout.
    pub queue_ms: Option<u64>,
    /// Paths relative to the exposed root to check out before the command runs.
    pub checkout_paths: Option<Vec<String>>,
}

impl ExecMountSpec {
    pub fn new(disk: impl Into<DiskRef>) -> Self {
        Self {
            disk: disk.into(),
            subdirectory: None,
            read_only: false,
            conditional: false,
            queue_ms: None,
            checkout_paths: None,
        }
    }
    pub fn subdirectory(mut self, s: impl Into<String>) -> Self {
        self.subdirectory = Some(s.into());
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
    pub fn checkout_paths<I, S>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.checkout_paths = Some(paths.into_iter().map(Into::into).collect());
        self
    }

    pub(crate) fn wire(&self) -> ExecMountWire {
        ExecMountWire {
            disk: self.disk.0.clone(),
            subdirectory: self.subdirectory.clone(),
            read_only: self.read_only,
            conditional: self.conditional,
            queue_ms: self.queue_ms,
            checkout_paths: self.checkout_paths.clone(),
        }
    }
}

/// One disk to mount in an exec request: a bare disk (root, read-write) or a
/// full [`ExecMountSpec`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecMount {
    Disk(DiskRef),
    Spec(ExecMountSpec),
}

impl<T: Into<DiskRef>> From<T> for ExecMount {
    fn from(d: T) -> Self {
        Self::Disk(d.into())
    }
}
impl From<ExecMountSpec> for ExecMount {
    fn from(s: ExecMountSpec) -> Self {
        Self::Spec(s)
    }
}

/// Options for [`Archil::exec`].
#[derive(Debug, Clone)]
pub struct ExecOptions {
    /// Disks keyed by the relative path under `/mnt/archil` to mount each at.
    pub disks: BTreeMap<String, ExecMount>,
    /// Shell command to run.
    pub command: String,
}

impl ExecOptions {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            disks: BTreeMap::new(),
            command: command.into(),
        }
    }
    pub fn disk(mut self, path: impl Into<String>, mount: impl Into<ExecMount>) -> Self {
        self.disks.insert(path.into(), mount.into());
        self
    }
}

/// Client configuration. Unset fields fall back to the environment
/// (`ARCHIL_API_KEY`, `ARCHIL_REGION`, `ARCHIL_S3_BASE_URL`).
#[derive(Debug, Clone, Default)]
pub struct ClientOptions {
    /// API key. Falls back to `ARCHIL_API_KEY`.
    pub api_key: Option<String>,
    /// Region such as `aws-us-east-1`. Falls back to `ARCHIL_REGION`.
    pub region: Option<String>,
    /// Override the control-plane base URL (useful for testing).
    pub base_url: Option<String>,
    /// Override the S3-compatible API base URL. Falls back to
    /// `ARCHIL_S3_BASE_URL`, then to the control-plane URL with `control.`
    /// swapped for `s3.`.
    pub s3_base_url: Option<String>,
    /// Trusted PEM CA certificates. Replaces the built-in roots; certificate
    /// and hostname verification stay enabled.
    pub ca_pem: Vec<Vec<u8>>,
    /// How often to poll while waiting for sandbox state changes
    /// (default 500 ms). Image builds poll at 2x this interval, backing off
    /// to 10x (1 s and 5 s by default).
    pub poll_interval: Option<Duration>,
}

impl ClientOptions {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn api_key(mut self, v: impl Into<String>) -> Self {
        self.api_key = Some(v.into());
        self
    }
    pub fn region(mut self, v: impl Into<String>) -> Self {
        self.region = Some(v.into());
        self
    }
    pub fn base_url(mut self, v: impl Into<String>) -> Self {
        self.base_url = Some(v.into());
        self
    }
    pub fn s3_base_url(mut self, v: impl Into<String>) -> Self {
        self.s3_base_url = Some(v.into());
        self
    }
    /// Add a trusted PEM CA certificate (may be a bundle).
    pub fn ca_pem(mut self, pem: impl Into<Vec<u8>>) -> Self {
        self.ca_pem.push(pem.into());
        self
    }
    pub fn poll_interval(mut self, v: Duration) -> Self {
        self.poll_interval = Some(v);
        self
    }
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// The Archil client. Cheap to clone; clones share one connection pool.
#[derive(Debug, Clone)]
pub struct Archil {
    http: Arc<Http>,
}

impl Archil {
    /// Build a client from explicit options, falling back to the environment.
    pub fn new(options: ClientOptions) -> Result<Self> {
        let api_key = options
            .api_key
            .filter(|v| !v.is_empty())
            .or_else(|| env_var("ARCHIL_API_KEY"));
        let region = options
            .region
            .filter(|v| !v.is_empty())
            .or_else(|| env_var("ARCHIL_REGION"));
        let Some(api_key) = api_key else {
            return Err(Error::Config(
                "Missing API key: pass api_key in options or set ARCHIL_API_KEY environment variable".into(),
            ));
        };
        let Some(region) = region else {
            return Err(Error::Config(
                "Missing region: pass region in options or set ARCHIL_REGION environment variable"
                    .into(),
            ));
        };
        let base_url = match options.base_url {
            Some(u) => u,
            None => resolve_base_url(&region)?,
        };
        let base_url = base_url.trim_end_matches('/').to_string();
        let s3_base_url = options
            .s3_base_url
            .or_else(|| env_var("ARCHIL_S3_BASE_URL"))
            .or_else(|| derive_s3_base_url(&base_url))
            .map(|u| u.trim_end_matches('/').to_string());

        let api_key = format!("key-{}", api_key.strip_prefix("key-").unwrap_or(&api_key));
        let mut headers = HeaderMap::new();
        let mut auth = HeaderValue::from_str(&api_key)
            .map_err(|_| Error::Config("API key contains invalid header characters".into()))?;
        auth.set_sensitive(true);
        headers.insert(AUTHORIZATION, auth);
        headers.insert(UA_HEADER, HeaderValue::from_static(USER_AGENT));

        let mut builder = reqwest::Client::builder()
            .default_headers(headers)
            // The load balancer closes idle connections after 60 s without
            // counting HTTP/2 PINGs as activity; retire ours first.
            .pool_idle_timeout(Duration::from_secs(30));
        if !options.ca_pem.is_empty() {
            builder = builder.tls_built_in_root_certs(false);
            for pem in &options.ca_pem {
                let certs = reqwest::Certificate::from_pem_bundle(pem)
                    .map_err(|e| Error::Config(format!("invalid CA certificate: {e}")))?;
                if certs.is_empty() {
                    return Err(Error::Config(
                        "CA bundle contains no PEM certificates".into(),
                    ));
                }
                for cert in certs {
                    builder = builder.add_root_certificate(cert);
                }
            }
        }
        let client = builder
            .build()
            .map_err(|e| Error::Config(format!("failed to build HTTP client: {e}")))?;
        Ok(Self {
            http: Arc::new(Http {
                client,
                base_url,
                s3_base_url,
                poll_interval: options.poll_interval.unwrap_or(Duration::from_millis(500)),
            }),
        })
    }

    /// Build a client entirely from `ARCHIL_API_KEY` / `ARCHIL_REGION`.
    pub fn from_env() -> Result<Self> {
        Self::new(ClientOptions::default())
    }

    /// The control-plane base URL in use.
    pub fn base_url(&self) -> &str {
        &self.http.base_url
    }

    /// The S3-compatible endpoint in use, if one could be determined.
    pub fn s3_base_url(&self) -> Option<&str> {
        self.http.s3_base_url.as_deref()
    }

    /// Disk operations.
    pub fn disks(&self) -> Disks {
        Disks::new(self.http.clone())
    }
    /// Image builds.
    pub fn images(&self) -> Images {
        Images::new(self.http.clone())
    }
    /// Sandbox operations.
    pub fn sandboxes(&self) -> Sandboxes {
        Sandboxes::new(self.http.clone())
    }
    /// API key operations.
    pub fn tokens(&self) -> Tokens {
        Tokens::new(self.http.clone())
    }

    /// Run a command in a container with several disks mounted at once, each
    /// under `/mnt/archil/<path>`. Blocks until it completes.
    pub async fn exec(&self, options: ExecOptions) -> Result<ExecDiskResult> {
        let mut disks = BTreeMap::new();
        for (rel, mount) in &options.disks {
            let value = match mount {
                ExecMount::Disk(d) => serde_json::Value::String(d.0.clone()),
                ExecMount::Spec(spec) => serde_json::to_value(spec.wire()).expect("serializes"),
            };
            disks.insert(rel.clone(), value);
        }
        let body = ExecRequest {
            disks,
            command: options.command,
        };
        self.http
            .call(Req::new(reqwest::Method::POST, "/api/exec", RetryMode::Connect).json(body))
            .await
    }

    /// Build a key-routed filesystem spanning several disks.
    pub fn workspace<I, K, V>(&self, mounts: I) -> Result<Workspace>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<crate::workspace::WorkspaceMount>,
    {
        Workspace::new(self.clone(), mounts)
    }
}
