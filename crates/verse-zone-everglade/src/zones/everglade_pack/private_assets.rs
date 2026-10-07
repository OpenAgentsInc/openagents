//! Private packs on desktop (`docs/verse/private-assets.md`): the owner's
//! placements for a zone load in the background, each through the broker's
//! signed URL, under the pinned loader's rules: HTTPS only, no redirects, an
//! exact length and digest, bounded decoding, and a content-addressed cache.
//!
//! The cache directory is created with mode 0700 and each pack written with
//! mode 0600. A cached pack that verifies loads without asking the broker.
//! Every failure is reported once and leaves the zone as it was.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread::JoinHandle;

use nostr::domain::RelaySigner;
use verse_private::placements::{Placement, Placements};

use super::ZonePack;
use super::compile::private;
use super::pinned::PinnedFile;

/// One placement's outcome.
#[derive(Debug)]
pub enum PrivateEvent {
    /// The pack verified and decoded.
    Ready {
        placement: Placement,
        pack: Box<ZonePack>,
    },
    /// The pack could not load: not authorized, offline, or invalid.
    Failed { asset: String, reason: String },
}

/// The background load of one zone's private placements.
pub struct PrivateLoader {
    cancel: Arc<AtomicBool>,
    events: mpsc::Receiver<PrivateEvent>,
    handle: Option<JoinHandle<()>>,
}

/// A digest as the pinned loader holds one. Each distinct digest is kept
/// once for the process, so repeated entries don't grow memory.
fn interned(digest: &str) -> &'static str {
    static DIGESTS: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let mut digests = DIGESTS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    digests
        .entry(digest.to_owned())
        .or_insert_with(|| Box::leak(digest.to_owned().into_boxed_str()))
}

fn file(placement: &Placement) -> PinnedFile {
    PinnedFile {
        label: "Private asset",
        sha256: interned(&placement.sha256),
        bytes: placement.bytes,
        url: String::new(),
        extension: "vtp",
        temp_prefix: ".private-",
        history: &[],
    }
}

/// Creates `cache` with mode 0700, or checks the one there is a directory.
fn private_directory(cache: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(cache) {
        Ok(metadata) if metadata.is_dir() => return Ok(()),
        Ok(_) => return Err("private cache must be a directory".into()),
        Err(_) => {}
    }
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(cache)
        .map_err(|_| "private cache could not be created".into())
}

/// Loads one placement: from the cache when it verifies, otherwise through
/// `grant`, which returns a signed URL for it.
fn load(
    placement: &Placement,
    cache: &Path,
    cancel: &AtomicBool,
    grant: &dyn Fn(&Placement) -> Result<String, String>,
) -> Result<ZonePack, String> {
    private_directory(cache)?;
    let mut file = file(placement);
    if let Ok(bytes) = file.read_bounded(&cache.join(file.cache_name()))
        && let Ok(pack) = private::decode(&bytes)
    {
        return Ok(pack);
    }
    file.url = grant(placement)?;
    file.fetch(cache, cancel, &mut |_, _| {}, private::decode)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl PrivateLoader {
    /// Starts loading `placements`' entries for `zone`, signing grant
    /// requests with `signer` and caching under `cache`. `None` when the
    /// zone has no placements.
    #[must_use]
    pub fn start(
        placements: &Placements,
        zone: &str,
        signer: RelaySigner,
        cache: PathBuf,
    ) -> Option<Self> {
        let broker = placements.broker.clone();
        let grant = move |p: &Placement| {
            verse_private::client::request_grant(&broker, &signer, &p.asset, &p.sha256, now())
                .and_then(|grant| {
                    if grant.bytes == p.bytes {
                        Ok(grant.url)
                    } else {
                        Err(verse_private::client::GrantError::Unavailable(
                            "the grant's length differs from the placement's".into(),
                        ))
                    }
                })
                .map_err(|error| error.to_string())
        };
        Self::start_with(placements, zone, cache, grant)
    }

    /// As [`Self::start`], with `grant` standing in for the broker.
    pub fn start_with(
        placements: &Placements,
        zone: &str,
        cache: PathBuf,
        grant: impl Fn(&Placement) -> Result<String, String> + Send + 'static,
    ) -> Option<Self> {
        let wanted: Vec<Placement> = placements.in_zone(zone).cloned().collect();
        if wanted.is_empty() {
            return None;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (tx, events) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("verse-private-assets".into())
            .spawn(move || {
                for placement in wanted {
                    if worker_cancel.load(Ordering::Acquire) {
                        return;
                    }
                    let event = match load(&placement, &cache, &worker_cancel, &grant) {
                        Ok(pack) => PrivateEvent::Ready {
                            placement,
                            pack: Box::new(pack),
                        },
                        Err(reason) => PrivateEvent::Failed {
                            asset: placement.asset,
                            reason,
                        },
                    };
                    if worker_cancel.load(Ordering::Acquire) || tx.send(event).is_err() {
                        return;
                    }
                }
            })
            .ok()?;
        Some(Self {
            cancel,
            events,
            handle: Some(handle),
        })
    }

    /// A finished placement, without blocking.
    pub fn poll(&mut self) -> Option<PrivateEvent> {
        self.events.try_recv().ok()
    }

    /// Whether every placement has reported.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.handle.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

impl Drop for PrivateLoader {
    fn drop(&mut self) {
        // The worker stops between placements or at the transfer's next
        // block; it is not joined, so leaving a zone never waits on it.
        self.cancel.store(true, Ordering::Release);
        self.handle.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placements(sha256: &str, bytes: u64) -> Placements {
        let mut file = Placements::new("https://broker.example", "default");
        file.place(Placement {
            asset: "sample-guest".into(),
            sha256: sha256.into(),
            bytes,
            zone: "everglade".into(),
            at: [105.5, -31.2],
            yaw: -1.571,
            scale: 1.0,
            seat: None,
        });
        file
    }

    fn wait(loader: &mut PrivateLoader) -> PrivateEvent {
        for _ in 0..500 {
            if let Some(event) = loader.poll() {
                return event;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the private loader never reported");
    }

    #[test]
    fn a_cached_pack_loads_without_the_broker_and_stays_private() {
        let pack = private::sample();
        let sha256 = verse_private::sha256_hex(&pack);
        let home = tempfile::tempdir().unwrap();
        let cache = home.path().join("private-cache");
        let file = placements(&sha256, pack.len() as u64);
        // Seed the cache as a verified download would.
        let mut pinned = super::file(&file.placements[0]);
        pinned.url = "https://unused.example".into();
        private_directory(&cache).unwrap();
        pinned
            .install_cache(&cache, &pack, &AtomicBool::new(false))
            .unwrap();
        let mut loader = PrivateLoader::start_with(&file, "everglade", cache.clone(), |_| {
            Err("the broker must not be asked".into())
        })
        .unwrap();
        match wait(&mut loader) {
            PrivateEvent::Ready { placement, pack } => {
                assert_eq!(placement.asset, "sample-guest");
                assert!(pack.form(private::NEAR).is_some());
            }
            PrivateEvent::Failed { reason, .. } => panic!("{reason}"),
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&cache), 0o700);
            assert_eq!(mode(&cache.join(format!("{sha256}.vtp"))), 0o600);
        }
    }

    #[test]
    fn an_unauthorized_reader_gets_nothing_and_no_cache_entry() {
        let home = tempfile::tempdir().unwrap();
        let cache = home.path().join("private-cache");
        let file = placements(&"cd".repeat(32), 1234);
        let mut loader = PrivateLoader::start_with(&file, "everglade", cache.clone(), |_| {
            Err("the broker refused this key".into())
        })
        .unwrap();
        match wait(&mut loader) {
            PrivateEvent::Failed { asset, reason } => {
                assert_eq!(asset, "sample-guest");
                assert!(reason.contains("refused"), "{reason}");
            }
            PrivateEvent::Ready { .. } => panic!("an unauthorized load succeeded"),
        }
        assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 0);
        // A zone with no placements starts nothing.
        assert!(PrivateLoader::start_with(&file, "grid", cache, |_| Ok(String::new())).is_none());
    }

    #[test]
    fn a_grant_for_another_scheme_is_refused() {
        let home = tempfile::tempdir().unwrap();
        let file = placements(&"cd".repeat(32), 1234);
        let mut loader = PrivateLoader::start_with(
            &file,
            "everglade",
            home.path().join("private-cache"),
            |_| Ok("http://storage.example/pack".into()),
        )
        .unwrap();
        assert!(matches!(wait(&mut loader), PrivateEvent::Failed { .. }));
    }
}
