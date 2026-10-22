//! The zone on livehost (`crates/livehost`): the tenants' keys and the host
//! config, deploy with the first-play budget as the pre-publish check,
//! rollback, and waiting for a page's ack.
//!
//! Everything lives in one work directory:
//!
//! ```text
//! WORK/store/          livehost's object store, shared by every tenant
//! WORK/icecream.key    the Ice Cream Man tenant's deploy key
//! WORK/haunt.key       the Haunted House tenant's deploy key
//! WORK/grid.key        the Grid tenant's deploy key
//! WORK/host.json       the host config: every tenant on its live channel
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use livehost::deploy::{self, CheckInput, Options, Report, Target};
use livehost::host::{HostConfig, TenantConfig};
use livehost::store::Store;
use nostr::domain::RelaySigner;

/// The tenants this demo hosts.
pub const TENANTS: [&str; 3] = [icecream_zone::TENANT, "grid", crate::haunt::TENANT];

pub struct Work {
    pub dir: PathBuf,
    pub relay: String,
    /// The object store (`--store`): `WORK/store` when none, or a
    /// directory or `gs://BUCKET[/PREFIX]`, the website's store.
    pub store: Option<String>,
    /// A tenant's deploy key file (`--key`), in place of `WORK/TENANT.key`:
    /// the deploy to the website's store signs with the game tenant's own
    /// key, never another tenant's.
    pub key: Option<PathBuf>,
}

impl Work {
    fn key_path(&self, tenant: &str) -> PathBuf {
        self.dir.join(format!("{tenant}.key"))
    }

    /// A tenant's deploy key, made on first use.
    ///
    /// # Errors
    /// The key can't be read or written.
    pub fn key(&self, tenant: &str) -> Result<RelaySigner> {
        if let Some(path) = &self.key {
            return livehost::keys::load(path);
        }
        let path = self.key_path(tenant);
        if !path.exists() {
            livehost::keys::generate(&path)?;
        }
        livehost::keys::load(&path)
    }

    /// The object store.
    ///
    /// # Errors
    /// The directory can't be made.
    pub fn store(&self) -> Result<Store> {
        match &self.store {
            Some(spec) => Store::parse(spec),
            None => Store::parse(&self.dir.join("store").to_string_lossy()),
        }
    }

    /// The host config: each tenant follows its `live` channel under
    /// `/t/TENANT/`.
    ///
    /// # Errors
    /// A key can't be made.
    pub fn config(&self) -> Result<HostConfig> {
        let mut tenants = Vec::new();
        for (i, name) in TENANTS.iter().enumerate() {
            tenants.push(TenantConfig {
                name: (*name).into(),
                pubkey: self.key(name)?.pubkey().into(),
                channel: deploy::LIVE.into(),
                // The first tenant also answers host names nobody lists.
                hosts: if i == 0 {
                    vec!["127.0.0.1".into(), "localhost".into()]
                } else {
                    Vec::new()
                },
            });
        }
        let config = HostConfig { tenants };
        std::fs::write(
            self.dir.join("host.json"),
            serde_json::to_vec_pretty(&config)?,
        )?;
        Ok(config)
    }

    fn target<'a>(
        &'a self,
        store: &'a Store,
        signer: &'a RelaySigner,
        channel: &'a str,
    ) -> Target<'a> {
        Target {
            store,
            relay: &self.relay,
            signer,
            channel,
        }
    }

    /// Deploys `dir` as `tenant`'s `channel`. For a game zone (The Ice
    /// Cream Man, The Haunted House), the first-play budget is the
    /// pre-publish check: it blocks `live` unless `override_check`, and
    /// warns elsewhere.
    ///
    /// # Errors
    /// The relay or the store failed, or the check blocked the deploy.
    pub async fn deploy(
        &self,
        tenant: &str,
        channel: &str,
        dir: &Path,
        override_check: bool,
    ) -> Result<Report> {
        let store = self.store()?;
        let signer = self.key(tenant)?;
        let check = |input: &CheckInput<'_>| -> Result<()> { budget(input).map(|_| ()) };
        let options = Options {
            tenant: Some(tenant.into()),
            check: [icecream_zone::TENANT, crate::haunt::TENANT]
                .contains(&tenant)
                .then_some(&check as &deploy::Check),
            override_check,
            ..Options::default()
        };
        deploy::deploy(dir, &self.target(&store, &signer, channel), &options).await
    }

    /// Moves `tenant`'s `channel` back to its previous manifest.
    ///
    /// # Errors
    /// There's nothing to go back to, or the relay failed.
    pub async fn rollback(&self, tenant: &str, channel: &str) -> Result<Report> {
        let store = self.store()?;
        let signer = self.key(tenant)?;
        deploy::rollback(tenant, &self.target(&store, &signer, channel), None).await
    }
}

/// The first-play budget over a deploy's files.
///
/// # Errors
/// The zone has no index, a first-play file is missing, or it is over
/// budget.
pub fn budget(input: &CheckInput<'_>) -> Result<icecream_zone::budget::Report> {
    let index = input
        .files
        .get(icecream_zone::INDEX)
        .context("the zone has no zone.json")?;
    let sizes: BTreeMap<&str, u64> = input
        .files
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.len() as u64))
        .collect();
    icecream_zone::budget::check(
        std::str::from_utf8(index)?,
        |path| sizes.get(path).copied(),
        input.files.keys(),
    )
    .map_err(anyhow::Error::msg)
}

/// The host's stream of page acks, opened before a deploy so a fast page
/// isn't missed.
pub struct Acks {
    stream:
        std::pin::Pin<Box<dyn futures_util::Stream<Item = reqwest::Result<bytes::Bytes>> + Send>>,
    buffer: String,
}

impl Acks {
    /// Subscribes to `host`'s acks.
    ///
    /// # Errors
    /// The host can't be reached.
    pub async fn open(host: &str) -> Result<Self> {
        let response = reqwest::get(format!("{host}/_livehost/acks"))
            .await?
            .error_for_status()?;
        Ok(Self {
            stream: Box::pin(response.bytes_stream()),
            buffer: String::new(),
        })
    }

    /// Waits up to `timeout` for an ack that matches `want` on every field
    /// `want` names.
    ///
    /// # Errors
    /// No such ack came in time, or the stream closed.
    pub async fn wait(
        &mut self,
        want: &serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value> {
        let started = Instant::now();
        loop {
            while let Some(end) = self.buffer.find("\n\n") {
                let event: String = self.buffer.drain(..end + 2).collect();
                for line in event.lines() {
                    let Some(data) = line.strip_prefix("data:") else {
                        continue;
                    };
                    let Ok(ack) = serde_json::from_str::<serde_json::Value>(data.trim()) else {
                        continue;
                    };
                    let matches = want
                        .as_object()
                        .is_some_and(|fields| fields.iter().all(|(k, v)| &ack[k] == v));
                    if matches {
                        return Ok(ack);
                    }
                }
            }
            let left = timeout
                .checked_sub(started.elapsed())
                .with_context(|| format!("no page acked {want} in time"))?;
            let Ok(next) = tokio::time::timeout(left, self.stream.next()).await else {
                bail!("no page acked {want} within {} s", timeout.as_secs());
            };
            let Some(chunk) = next else {
                bail!("the host closed its ack stream");
            };
            self.buffer.push_str(&String::from_utf8_lossy(&chunk?));
        }
    }
}
