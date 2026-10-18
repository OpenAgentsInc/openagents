//! Process-wide convenience functions backed by a lazily-built default client.
//!
//! The default client reads `ARCHIL_API_KEY` / `ARCHIL_REGION`; call
//! [`configure`] to pass options explicitly or to swap credentials mid-process.

use std::sync::Mutex;

use crate::client::{Archil, ClientOptions, ExecOptions};
use crate::disk::Disk;
use crate::disks::{CreateDiskResult, ListDisksOptions};
use crate::error::Result;
use crate::sandbox::{Sandbox, WaitOptions};
use crate::sandboxes::{CreateSandboxRequest, ListSandboxesOptions};
use crate::tokens::ListTokensOptions;
use crate::types::{
    ApiTokenResponse, CreateApiTokenRequest, CreateDiskRequest, CreatedApiToken, ExecDiskResult,
};
use crate::workspace::{Workspace, WorkspaceMount};

#[derive(Default)]
struct Global {
    options: Option<ClientOptions>,
    instance: Option<Archil>,
}

static GLOBAL: Mutex<Global> = Mutex::new(Global {
    options: None,
    instance: None,
});

/// Set the options for the module-level client and discard any cached one.
pub fn configure(options: ClientOptions) {
    let mut g = GLOBAL.lock().unwrap_or_else(|p| p.into_inner());
    g.options = Some(options);
    g.instance = None;
}

/// The module-level client, built on first use.
pub fn client() -> Result<Archil> {
    let mut g = GLOBAL.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(c) = &g.instance {
        return Ok(c.clone());
    }
    let c = Archil::new(g.options.clone().unwrap_or_default())?;
    g.instance = Some(c.clone());
    Ok(c)
}

pub async fn create_disk(req: CreateDiskRequest) -> Result<CreateDiskResult> {
    client()?.disks().create(req).await
}

pub async fn list_disks(opts: ListDisksOptions) -> Result<Vec<Disk>> {
    client()?.disks().list(opts).await
}

pub async fn get_disk(id: &str) -> Result<Disk> {
    client()?.disks().get(id).await
}

pub async fn create_sandbox(req: CreateSandboxRequest, opts: WaitOptions) -> Result<Sandbox> {
    client()?.sandboxes().create_with(req, opts).await
}

pub async fn list_sandboxes(opts: ListSandboxesOptions) -> Result<Vec<Sandbox>> {
    client()?.sandboxes().list(opts).await
}

pub async fn get_sandbox(id: &str) -> Result<Sandbox> {
    client()?.sandboxes().get(id).await
}

pub async fn list_api_keys(opts: ListTokensOptions) -> Result<Vec<ApiTokenResponse>> {
    client()?.tokens().list(opts).await
}

pub async fn create_api_key(req: CreateApiTokenRequest) -> Result<CreatedApiToken> {
    client()?.tokens().create(req).await
}

pub async fn delete_api_key(id: &str) -> Result<()> {
    client()?.tokens().delete(id).await
}

/// Run a command in a container with multiple disks mounted.
pub async fn exec(opts: ExecOptions) -> Result<ExecDiskResult> {
    client()?.exec(opts).await
}

/// Build a multi-disk workspace using the module-level client.
pub fn workspace<I, K, V>(mounts: I) -> Result<Workspace>
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<WorkspaceMount>,
{
    client()?.workspace(mounts)
}
