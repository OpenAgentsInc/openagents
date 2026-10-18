//! Sandbox creation, lookup and listing.

use std::collections::BTreeMap;
use std::sync::Arc;

use reqwest::Method;
use serde::Serialize;

use crate::client::DiskRef;
use crate::error::Result;
use crate::http::{Http, Req, enc};
use crate::retry::RetryMode;
use crate::sandbox::{
    Sandbox, SandboxMountSpec, SandboxMountWire, SandboxNetwork, SandboxWire, WaitOptions,
};

/// Request body for [`Sandboxes::create`]. Unset fields use server defaults.
#[derive(Debug, Clone, Default)]
pub struct CreateSandboxRequest {
    /// Name for the sandbox; the server generates one when omitted.
    pub name: Option<String>,
    /// Virtual CPUs, 1 to 32 (default 1).
    pub vcpu_count: Option<u32>,
    /// Memory in MiB, 256 to 65536 (default 2048).
    pub mem_size_mib: Option<u32>,
    /// Public Linux OCI image reference. Mutually exclusive with `image_id`.
    pub base_image: Option<String>,
    /// Ready image from [`Images::build`](crate::Images::build). Mutually
    /// exclusive with `base_image`.
    pub image_id: Option<String>,
    pub env: Option<BTreeMap<String, String>>,
    pub max_ttl_seconds: Option<u64>,
    /// Seconds without a direct process connection before pausing; zero disables.
    pub idle_ttl_seconds: Option<u64>,
    /// Maximum concurrently attached exec sessions.
    pub max_concurrent_execs: Option<u32>,
    /// Creation-time network policy; egress is unrestricted when omitted.
    pub network: Option<SandboxNetwork>,
    /// TCP ports to expose publicly (1-65535).
    pub ports: Option<Vec<u16>>,
    /// Disks mounted for the first session; [`Sandbox::start_with`] can replace them.
    pub mounts: Option<Vec<SandboxMountSpec>>,
}

impl CreateSandboxRequest {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn name(mut self, v: impl Into<String>) -> Self {
        self.name = Some(v.into());
        self
    }
    pub fn vcpu_count(mut self, v: u32) -> Self {
        self.vcpu_count = Some(v);
        self
    }
    pub fn mem_size_mib(mut self, v: u32) -> Self {
        self.mem_size_mib = Some(v);
        self
    }
    pub fn base_image(mut self, v: impl Into<String>) -> Self {
        self.base_image = Some(v.into());
        self
    }
    pub fn image_id(mut self, v: impl Into<String>) -> Self {
        self.image_id = Some(v.into());
        self
    }
    pub fn env(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.env
            .get_or_insert_with(BTreeMap::new)
            .insert(k.into(), v.into());
        self
    }
    pub fn max_ttl_seconds(mut self, v: u64) -> Self {
        self.max_ttl_seconds = Some(v);
        self
    }
    pub fn idle_ttl_seconds(mut self, v: u64) -> Self {
        self.idle_ttl_seconds = Some(v);
        self
    }
    pub fn max_concurrent_execs(mut self, v: u32) -> Self {
        self.max_concurrent_execs = Some(v);
        self
    }
    pub fn network(mut self, v: SandboxNetwork) -> Self {
        self.network = Some(v);
        self
    }
    pub fn ports(mut self, v: impl IntoIterator<Item = u16>) -> Self {
        self.ports = Some(v.into_iter().collect());
        self
    }
    pub fn mounts(mut self, v: impl IntoIterator<Item = SandboxMountSpec>) -> Self {
        self.mounts = Some(v.into_iter().collect());
        self
    }
}

#[derive(Serialize)]
struct CreateBody<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vcpu_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mem_size_mib: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_image: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<&'a BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_ttl_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    idle_ttl_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_concurrent_execs: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network: Option<&'a SandboxNetwork>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ports: Option<&'a [u16]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mounts: Option<Vec<SandboxMountWire>>,
}

/// Options for [`Sandboxes::list`].
#[derive(Debug, Clone, Default)]
pub struct ListSandboxesOptions {
    /// Only return sandboxes that mount this disk.
    pub disk: Option<DiskRef>,
}

/// Sandbox operations, from [`Archil::sandboxes`](crate::Archil::sandboxes).
#[derive(Debug, Clone)]
pub struct Sandboxes {
    http: Arc<Http>,
}

impl Sandboxes {
    pub(crate) fn new(http: Arc<Http>) -> Self {
        Self { http }
    }

    /// List the account's sandboxes, oldest first.
    pub async fn list(&self, options: ListSandboxesOptions) -> Result<Vec<Sandbox>> {
        #[derive(serde::Deserialize)]
        struct Data {
            sandboxes: Option<Vec<SandboxWire>>,
        }
        let req = Req::get("/api/sandboxes").query_opt("filesystem", options.disk.map(|d| d.0));
        let data: Option<Data> = self.http.call(req).await?;
        Ok(data
            .and_then(|d| d.sandboxes)
            .unwrap_or_default()
            .into_iter()
            .map(|s| Sandbox::new(s, self.http.clone()))
            .collect())
    }

    pub async fn get(&self, id: &str) -> Result<Sandbox> {
        let data: SandboxWire = self
            .http
            .call(Req::get(format!("/api/sandboxes/{}", enc(id))))
            .await?;
        Ok(Sandbox::new(data, self.http.clone()))
    }

    /// Create a sandbox and wait for it to start.
    pub async fn create(&self, request: CreateSandboxRequest) -> Result<Sandbox> {
        self.create_with(request, WaitOptions::default()).await
    }

    /// Create a sandbox. With `wait` (the default) this polls until it leaves `pending`.
    pub async fn create_with(
        &self,
        request: CreateSandboxRequest,
        options: WaitOptions,
    ) -> Result<Sandbox> {
        let body = CreateBody {
            name: request.name.as_deref(),
            vcpu_count: request.vcpu_count,
            mem_size_mib: request.mem_size_mib,
            base_image: request.base_image.as_deref(),
            image_id: request.image_id.as_deref(),
            env: request.env.as_ref(),
            max_ttl_seconds: request.max_ttl_seconds,
            idle_ttl_seconds: request.idle_ttl_seconds,
            max_concurrent_execs: request.max_concurrent_execs,
            network: request.network.as_ref(),
            ports: request.ports.as_deref(),
            mounts: request
                .mounts
                .as_ref()
                .map(|m| m.iter().map(SandboxMountSpec::wire).collect()),
        };
        let req = Req::new(Method::POST, "/api/sandboxes", RetryMode::Connect)
            .query("wait", options.wait)
            .json(body);
        let data: SandboxWire = self.http.call(req).await?;
        let mut sandbox = Sandbox::new(data, self.http.clone());
        if options.wait {
            sandbox.wait_for_start().await?;
        }
        Ok(sandbox)
    }
}
