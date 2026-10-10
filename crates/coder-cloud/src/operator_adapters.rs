//! Provider adapters with explicit credentials and pool custody.

use crate::{
    operator::{Adapter, BackendDriver, Driver, Profile, private, read_private},
    runtime::Credentials,
};
use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tokio::io::AsyncWriteExt;

pub(crate) fn configured(p: &Profile) -> Result<Option<Arc<dyn Driver>>, String> {
    if !qualified_identity(p) {
        return Ok(None);
    }
    let values = p
        .credentials
        .iter()
        .map(|(name, path)| {
            Ok((
                name.clone(),
                String::from_utf8(
                    read_private(path, 1024 * 1024)
                        .map_err(|_| "The selected operator credential is unavailable.")?,
                )
                .map_err(|_| "The selected operator credential encoding is invalid.")?,
            ))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>, String>>()?;
    if values.get("OA_CODEX_AUTH").is_some_and(|value| {
        !serde_json::from_str::<serde_json::Value>(value).is_ok_and(|auth| auth.is_object())
    }) {
        return Ok(None);
    }
    // Codex needs a selected engine credential. Claude Code may use the
    // user's own API key, or else the login the user made inside their own
    // computer, which never passes through this profile.
    let engine_keys: &[&str] = if p.executor == crate::claude::ENGINE {
        &[
            crate::claude::API_KEY,
            crate::claude::BEDROCK,
            crate::claude::VERTEX,
            crate::claude::FOUNDRY,
        ]
    } else {
        &["OPENAI_API_KEY", "OA_CODEX_AUTH"]
    };
    let selected = |name: &&str| {
        values
            .get(*name)
            .is_some_and(|value| !value.trim().is_empty())
    };
    let refused = if p.executor == crate::claude::ENGINE {
        engine_keys
            .iter()
            .any(|name| values.contains_key(*name) && !selected(name))
    } else {
        !engine_keys.iter().any(selected)
    };
    if refused {
        return Ok(None);
    }
    let credentials =
        Credentials::from_names(&values.keys().cloned().collect::<Vec<_>>(), |name| {
            values.get(name).cloned()
        })?;
    match &p.adapter {
        Adapter::Unavailable => Ok(None),
        Adapter::Boat { origin, token_file } => {
            let token = String::from_utf8(
                read_private(token_file, 1024 * 1024)
                    .map_err(|_| "The explicit Boat credential is unavailable.")?,
            )
            .map_err(|_| "The explicit Boat credential encoding is invalid.")?;
            let key = boat::auth::ApiKey::new(token)
                .map_err(|_| "The explicit Boat credential is invalid.")?;
            let client = boat::Client::builder(key)
                .base_url(origin)
                .build()
                .map_err(|_| "The explicit Boat origin is invalid.")?;
            Ok(Some(Arc::new(BackendDriver(crate::boat_backend::Boat {
                client,
                credentials,
            }))))
        }
        Adapter::Gce {
            pool_file,
            gcloud_binary,
            config_directory,
            credential_file,
            hosts,
        } => {
            private(config_directory, true)
                .map_err(|_| "The explicit gcloud configuration must be private.")?;
            read_private(credential_file, 1024 * 1024)
                .map_err(|_| "The explicit GCE credential is unavailable.")?;
            let pool: crate::pool::Pool = serde_json::from_slice(
                &read_private(pool_file, 65536)
                    .map_err(|_| "The explicit GCE pool is unavailable.")?,
            )
            .map_err(|_| "The explicit GCE pool is invalid.")?;
            if pool.revoked_at.is_some()
                || pool.pool != p.pool
                || pool.computer != "gce"
                || pool.slots_per_host == 0
                || hosts.len() as u64 > pool.max_hosts
            {
                return Err("The explicit GCE pool is not currently granted.".into());
            }
            if !gcloud_binary.is_absolute()
                || !gcloud_binary.is_file()
                || gcloud_binary.canonicalize().ok().as_ref() != Some(gcloud_binary)
            {
                return Err("The GCE adapter needs an explicit gcloud executable.".into());
            }
            Ok(Some(Arc::new(BackendDriver(crate::gce_backend::Gce {
                transport: ExplicitGce {
                    pool_file: pool_file.clone(),
                    pool,
                    gcloud: gcloud_binary.clone(),
                    configuration: config_directory.clone(),
                    credential: credential_file.clone(),
                    hosts: hosts.clone(),
                },
                credentials,
            }))))
        }
    }
}
pub(crate) fn qualified_identity(p: &Profile) -> bool {
    let tools = |name: &str| matches!(name, "GH_TOKEN" | "GITHUB_TOKEN");
    p.mode == crate::Mode::Coder
        && match p.executor.as_str() {
            "codex" => {
                p.credentials
                    .keys()
                    .any(|name| matches!(name.as_str(), "OPENAI_API_KEY" | "OA_CODEX_AUTH"))
                    && p.credentials.keys().all(|name| {
                        matches!(name.as_str(), "OPENAI_API_KEY" | "OA_CODEX_AUTH") || tools(name)
                    })
            }
            // Claude Code never takes a claude.ai login from a profile
            // (docs/cloud/claude-code-byo.md); only the user's own API key
            // or Bedrock/Vertex/Foundry credential (BYO-04), at most one. A
            // subscription token reaches a run only as the person's own
            // release from Settings (#11204), never from an operator file.
            crate::claude::ENGINE => {
                let own = |name: &str| {
                    crate::claude::OwnCredential::from_name(name)
                        .is_some_and(|c| c != crate::claude::OwnCredential::SubscriptionToken)
                };
                p.credentials.keys().all(|name| own(name) || tools(name))
                    && p.credentials.keys().filter(|name| own(name)).count() <= 1
            }
            _ => false,
        }
}

/// A pinned pool and named hosts. This adapter never grows a pool or selects
/// the operator's ambient gcloud account, configuration, or SSH directory.
pub struct ExplicitGce {
    pub pool_file: PathBuf,
    pub pool: crate::pool::Pool,
    pub gcloud: PathBuf,
    pub configuration: PathBuf,
    pub credential: PathBuf,
    pub hosts: Vec<crate::pool::Host>,
}
impl crate::gce_backend::Transport for ExplicitGce {
    fn granted(&self) -> crate::Result<crate::pool::Pool> {
        let current: crate::pool::Pool = serde_json::from_slice(
            &read_private(&self.pool_file, 65536)
                .map_err(|_| "The explicit GCE pool is unavailable.")?,
        )
        .map_err(|_| "The explicit GCE pool is invalid.")?;
        if current != self.pool || current.revoked_at.is_some() {
            return Err("The explicit GCE pool grant changed.".into());
        }
        Ok(current)
    }
    fn hosts(&self, pool: &crate::pool::Pool) -> crate::Result<Vec<crate::pool::Host>> {
        if self.granted()? != *pool {
            return Err("The explicit GCE pool differs.".into());
        }
        Ok(self.hosts.clone())
    }
    fn start(&self, _: &crate::pool::Pool) -> crate::Result<crate::pool::Host> {
        Err(
            "The explicit operator profile admits no pool growth. Configure a host separately."
                .into(),
        )
    }
    async fn prepare_runtime(
        &self,
        _: &crate::pool::Pool,
        _: &crate::pool::Host,
    ) -> crate::Result<()> {
        Ok(())
    }
    async fn execute(
        &self,
        pool: &crate::pool::Pool,
        host: &crate::pool::Host,
        script: &str,
        input: Option<&[u8]>,
    ) -> crate::Result<String> {
        if self.granted()? != *pool
            || !self
                .hosts
                .iter()
                .any(|h| h == host && h.status == "RUNNING")
        {
            return Err("The explicit GCE host is not admitted.".into());
        }
        private(&self.configuration, true)
            .map_err(|_| "The explicit gcloud configuration is unavailable.")?;
        read_private(&self.credential, 1024 * 1024)
            .map_err(|_| "The explicit GCE credential is unavailable.")?;
        let ssh = self.configuration.join("operator-ssh");
        let mut process = tokio::process::Command::new(&self.gcloud);
        process
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &self.configuration)
            .env("CLOUDSDK_CONFIG", &self.configuration)
            .env("CLOUDSDK_AUTH_CREDENTIAL_FILE_OVERRIDE", &self.credential)
            .args([
                "compute",
                "ssh",
                &host.name,
                "--zone",
                &host.zone,
                "--project",
                &pool.project,
                "--quiet",
                "--tunnel-through-iap",
                "--ssh-key-file",
            ])
            .arg(&ssh)
            .arg("--command")
            .arg(format!(
                "timeout 610 env -i HOME=\"$HOME\" PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin sh -c {}",
                boat::shell_quote(script)
            ))
            .kill_on_drop(true)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = process
            .spawn()
            .map_err(|_| "Cannot start the explicit GCE transport.")?;
        if let Some(bytes) = input {
            let mut stdin = child
                .stdin
                .take()
                .ok_or("Missing explicit GCE upload input.")?;
            tokio::time::timeout(Duration::from_secs(120), stdin.write_all(bytes))
                .await
                .map_err(|_| "The explicit GCE upload timed out.")?
                .map_err(|_| "The explicit GCE upload disconnected.")?;
        }
        let output = tokio::time::timeout(Duration::from_secs(650), child.wait_with_output())
            .await
            .map_err(|_| "The explicit GCE command timed out.")?
            .map_err(|_| "The explicit GCE command disconnected.")?;
        if !output.status.success() || output.stdout.len() > 32 * 1024 * 1024 {
            return Err("The explicit GCE command failed or exceeded its bound.".into());
        }
        String::from_utf8(output.stdout)
            .map_err(|_| "The explicit GCE response encoding is invalid.".into())
    }
}
