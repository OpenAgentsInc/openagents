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
    /// Paths under these prefixes, such as `modules/`, load when first
    /// asked for ([`Follower::object`]) rather than with the manifest.
    pub lazy_prefixes: Vec<String>,
    /// Bytes of lazily loaded objects to keep; the oldest go first.
    pub lazy_budget: usize,
}

/// The default for [`FollowConfig::lazy_budget`].
pub const LAZY_BUDGET: usize = 256 * 1024 * 1024;

/// A loaded manifest: every object by path.
pub struct Content {
    pub manifest_hash: String,
    pub manifest: Manifest,
    pub pointer: Pointer,
    files: HashMap<String, Arc<[u8]>>,
    /// Handler components by name: their hash and bytes.
    handlers: HashMap<String, (String, Arc<[u8]>)>,
}

impl Content {
    pub fn bytes(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(|bytes| &bytes[..])
    }

    pub fn text(&self, path: &str) -> Option<&str> {
        std::str::from_utf8(self.bytes(path)?).ok()
    }

    /// The handler component `name`: its content hash and bytes.
    pub fn handler(&self, name: &str) -> Option<(&str, &[u8])> {
        self.handlers
            .get(name)
            .map(|(hash, bytes)| (hash.as_str(), &bytes[..]))
    }
}

pub struct Follower {
    config: FollowConfig,
    current: RwLock<Option<Arc<Content>>>,
    changes: watch::Sender<Option<String>>,
    /// Objects by hash, kept across manifests so a swap fetches only new ones.
    objects: RwLock<HashMap<String, Arc<[u8]>>>,
    lazy: std::sync::Mutex<Lazy>,
}

/// Lazily loaded objects by hash, oldest first, and their total size.
#[derive(Default)]
struct Lazy {
    objects: HashMap<String, Arc<[u8]>>,
    order: std::collections::VecDeque<String>,
    bytes: usize,
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
            lazy: std::sync::Mutex::new(Lazy::default()),
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

    /// The object at `path` in `content`: from memory, or for a lazy path
    /// from the store, then kept within the lazy budget.
    pub async fn object(&self, content: &Content, path: &str) -> Result<Option<Arc<[u8]>>> {
        if let Some(bytes) = content.files.get(path) {
            return Ok(Some(bytes.clone()));
        }
        let Some(hash) = content.manifest.objects.get(path) else {
            return Ok(None);
        };
        if let Some(bytes) = self.lazy.lock().expect("lazy").objects.get(hash) {
            return Ok(Some(bytes.clone()));
        }
        let bytes: Arc<[u8]> = Arc::from(self.config.store.get(hash).await?);
        let mut lazy = self.lazy.lock().expect("lazy");
        if !lazy.objects.contains_key(hash) {
            lazy.bytes += bytes.len();
            lazy.order.push_back(hash.clone());
            lazy.objects.insert(hash.clone(), bytes.clone());
            while lazy.bytes > self.config.lazy_budget && lazy.order.len() > 1 {
                let Some(oldest) = lazy.order.pop_front() else {
                    break;
                };
                if let Some(gone) = lazy.objects.remove(&oldest) {
                    lazy.bytes -= gone.len();
                }
            }
        }
        Ok(Some(bytes))
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
            let lazy = |path: &str| {
                self.config
                    .lazy_prefixes
                    .iter()
                    .any(|prefix| path.starts_with(prefix.as_str()))
            };
            let mut hashes: Vec<String> = manifest
                .objects
                .iter()
                .filter(|(path, _)| !lazy(path))
                .map(|(_, hash)| hash)
                .chain(manifest.handlers.values().map(|built| &built.object))
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
        // Keep only this manifest's objects; requests still on the old one
        // hold their own references.
        let wanted = manifest.object_hashes();
        objects.retain(|hash, _| wanted.contains(hash));
        let files = manifest
            .objects
            .iter()
            .filter_map(|(path, hash)| Some((path.clone(), objects.get(hash)?.clone())))
            .collect();
        let handlers = manifest
            .handlers
            .iter()
            .filter_map(|(name, built)| {
                Some((
                    name.clone(),
                    (built.object.clone(), objects.get(&built.object)?.clone()),
                ))
            })
            .collect();
        Ok(Content {
            manifest_hash: pointer.manifest.clone(),
            manifest,
            pointer,
            files,
            handlers,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use nostr::domain::RelaySigner;

    use super::*;
    use crate::manifest::{HOST_VERSION, SCHEMA};

    async fn manifest(store: &Store, files: &[(&str, &[u8])]) -> String {
        let mut objects = BTreeMap::new();
        for (path, bytes) in files {
            objects.insert((*path).to_owned(), store.put(bytes).await.unwrap().0);
        }
        let manifest = Manifest {
            schema: SCHEMA.into(),
            tenant: "site".into(),
            host_min: HOST_VERSION,
            parent: None,
            objects,
            routes: Vec::new(),
            modules: BTreeMap::new(),
            handlers: BTreeMap::new(),
            encodings: BTreeMap::new(),
            meta: None,
        };
        store.put(&manifest.to_bytes()).await.unwrap().0
    }

    #[tokio::test]
    async fn lazy_paths_load_on_request_within_the_budget() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let store = Store::parse(path).unwrap();
        let first = manifest(
            &store,
            &[
                ("content/a.md", b"a"),
                ("modules/x/x.wasm", &[1u8; 600]),
                ("modules/y/y.wasm", &[2u8; 600]),
            ],
        )
        .await;
        let key = RelaySigner::from_secret_hex(&"7c".repeat(32)).unwrap();
        let follower = Follower::new(FollowConfig {
            relay: "ws://127.0.0.1:9".into(),
            store: Store::parse(path).unwrap(),
            tenant: "site".into(),
            channel: "staging".into(),
            pubkey: key.pubkey().into(),
            lazy_prefixes: vec!["modules/".into()],
            lazy_budget: 1000,
        })
        .unwrap();
        let event = pointer::sign(&key, "site", "staging", &first, None, 1).unwrap();
        follower.receive(&event).await.unwrap();
        let content = follower.current().unwrap();
        assert_eq!(content.bytes("content/a.md"), Some(&b"a"[..]));
        assert_eq!(
            content.bytes("modules/x/x.wasm"),
            None,
            "lazy paths aren't loaded"
        );
        let x = follower
            .object(&content, "modules/x/x.wasm")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&x[..], &[1u8; 600][..]);
        follower
            .object(&content, "modules/y/y.wasm")
            .await
            .unwrap()
            .unwrap();
        // Over budget, the oldest lazy object went.
        assert_eq!(follower.lazy.lock().unwrap().objects.len(), 1);
        assert!(
            follower
                .object(&content, "modules/z")
                .await
                .unwrap()
                .is_none()
        );

        // A new manifest drops objects only the old one used.
        let second = manifest(&store, &[("content/b.md", b"b")]).await;
        let event = pointer::sign(&key, "site", "staging", &second, None, 2).unwrap();
        follower.receive(&event).await.unwrap();
        let objects = follower.objects.read().unwrap();
        assert_eq!(objects.len(), 1);
    }
}
