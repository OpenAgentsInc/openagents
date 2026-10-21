//! A follower: the light half of a host, for a program that serves its own
//! routes but reads content from a tenant's manifest (#12435). It follows
//! one tenant's pointer, loads the manifest and its objects, and keeps the
//! newest as a [`Content`]. It needs no wasmtime and no HTTP server.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use anyhow::{Result, bail};
use nostr::domain::Event;
use serde_json::json;
use tokio::sync::{mpsc, watch};

use crate::manifest::Manifest;
use crate::pointer::{self, Pointer};
use crate::store::Store;
use crate::{ms_since, relay};

/// One tenant's pointer to follow.
pub struct FollowConfig {
    pub relay: String,
    pub store: Store,
    pub tenant: String,
    pub channel: String,
    /// The hex public key whose pointers count.
    pub pubkey: String,
}

/// A loaded manifest: every object by path.
pub struct Content {
    pub manifest_hash: String,
    pub manifest: Manifest,
    pub pointer: Pointer,
    files: HashMap<String, Arc<[u8]>>,
}

impl Content {
    pub fn bytes(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(|bytes| &bytes[..])
    }

    pub fn text(&self, path: &str) -> Option<&str> {
        std::str::from_utf8(self.bytes(path)?).ok()
    }
}

pub struct Follower {
    config: FollowConfig,
    current: RwLock<Option<Arc<Content>>>,
    changes: watch::Sender<Option<String>>,
    /// Objects by hash, kept across manifests so a swap fetches only new ones.
    objects: RwLock<HashMap<String, Arc<[u8]>>>,
}

impl Follower {
    /// A follower that loads only the pointers given to [`Follower::receive`].
    pub fn new(config: FollowConfig) -> Result<Arc<Self>> {
        pointer::check_name("tenant", &config.tenant)?;
        pointer::check_name("channel", &config.channel)?;
        Ok(Arc::new(Follower {
            config,
            current: RwLock::new(None),
            changes: watch::channel(None).0,
            objects: RwLock::new(HashMap::new()),
        }))
    }

    /// Starts following the relay on the current Tokio runtime.
    pub fn start(config: FollowConfig) -> Result<Arc<Self>> {
        let follower = Self::new(config)?;
        let (sender, mut receiver) = mpsc::channel(64);
        let filter = json!({
            "kinds": [pointer::KIND],
            "authors": [follower.config.pubkey],
            "#d": [pointer::d_tag(&follower.config.tenant, &follower.config.channel)],
        });
        tokio::spawn(relay::follow(
            follower.config.relay.clone(),
            vec![filter],
            sender,
        ));
        let worker = follower.clone();
        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                if let Err(error) = worker.receive(&event).await {
                    eprintln!(
                        "livehost: {}/{}: pointer {}: {error:#}",
                        worker.config.tenant, worker.config.channel, event.id
                    );
                }
            }
        });
        Ok(follower)
    }

    pub fn tenant(&self) -> &str {
        &self.config.tenant
    }

    /// The newest loaded content, if any pointer has been followed yet.
    pub fn current(&self) -> Option<Arc<Content>> {
        self.current.read().expect("current content").clone()
    }

    /// The manifest hash each time it changes.
    pub fn changes(&self) -> watch::Receiver<Option<String>> {
        self.changes.subscribe()
    }

    /// Verifies a pointer event and, if it is newer, loads what it names.
    pub async fn receive(&self, event: &Event) -> Result<()> {
        let started = Instant::now();
        let config = &self.config;
        let pointer = pointer::verify(event, &config.pubkey, &config.tenant, &config.channel)?;
        if let Some(current) = self.current()
            && (pointer.created_at, &pointer.id)
                <= (current.pointer.created_at, &current.pointer.id)
        {
            return Ok(());
        }
        let content = self.load(pointer).await?;
        let hash = content.manifest_hash.clone();
        *self.current.write().expect("current content") = Some(Arc::new(content));
        let _ = self.changes.send(Some(hash.clone()));
        eprintln!(
            "livehost: {}/{} now reads {} ({:.0} ms)",
            config.tenant,
            config.channel,
            &hash[..12],
            ms_since(started)
        );
        Ok(())
    }

    async fn load(&self, pointer: Pointer) -> Result<Content> {
        let store = &self.config.store;
        let bytes = match &pointer.body {
            Some(body) => body.clone(),
            None => store.get(&pointer.manifest).await?,
        };
        let manifest = Manifest::open(&bytes, &pointer.manifest)?;
        if manifest.tenant != self.config.tenant {
            bail!("manifest belongs to tenant {}", manifest.tenant);
        }
        let missing: Vec<String> = {
            let objects = self.objects.read().expect("objects");
            let mut hashes: Vec<String> = manifest
                .objects
                .values()
                .filter(|hash| !objects.contains_key(*hash))
                .cloned()
                .collect();
            hashes.sort();
            hashes.dedup();
            hashes
        };
        // Every missing object at once, not one after another.
        let fetched =
            futures_util::future::try_join_all(missing.iter().map(|hash| store.get(hash))).await?;
        let mut objects = self.objects.write().expect("objects");
        for (hash, bytes) in missing.into_iter().zip(fetched) {
            objects.insert(hash, Arc::from(bytes));
        }
        let files = manifest
            .objects
            .iter()
            .filter_map(|(path, hash)| Some((path.clone(), objects.get(hash)?.clone())))
            .collect();
        Ok(Content {
            manifest_hash: pointer.manifest.clone(),
            manifest,
            pointer,
            files,
        })
    }
}
