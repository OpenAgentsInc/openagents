//! The pinned Everglade zone pack, loaded only after portal entry.
//!
//! The pack holds the admitted Quaternius models with base-color textures and
//! material flags (`format`), compiled from `assets/verse/everglade/` by
//! `compile`. It loads under the Ruins loader's rules (`pinned`): HTTPS only,
//! no redirects, an exact length and digest, bounded decoding, and a
//! content-addressed disk cache. It carries no scripts, URLs, or authority.
//!
//! Rebuild the pack after changing an admitted source:
//!
//! ```text
//! cargo run --release -p verse --example everglade_pack -- assets/verse/everglade
//! ```
//!
//! then set [`PACK_SHA256`] and [`PACK_BYTES`] to the values it prints, and
//! add the previous digest to `EVERGLADE_PACK_HISTORY`.

pub mod compile;
pub mod format;
pub mod pinned;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;

pub use format::{
    AlphaMode, Character, Clip, Joint, Limits, Material, Model, Primitive, SkinnedPrimitive,
    SkinnedVertex, Texture, Track, Vertex, ZonePack,
};
use pinned::PinnedFile;

/// Exact content identity of the reviewed Everglade pack.
pub const PACK_SHA256: &str = "3bbdbfc043c8f0e02159e93190889b69fd5d58d873bbad990147d39a68b689e4";
/// Transfer size of the reviewed Everglade pack.
pub const PACK_BYTES: u64 = 15020817;
/// The most triangles the Everglade layout may place, counting each placement.
/// The city is sixteen times the first glade's area and places about 1.8
/// million with the ground, which merges to about 107 MiB, under the
/// renderer's 128 MiB bound (`pbr::textured::MAX_BYTES`). Roofs are the
/// largest share, about 4,500 triangles each. A frame draws only the merged
/// 8 m cells in view, nearer than the fog's close at 180 m, and large
/// enough to see at their distance, so a street-level view draws a third
/// to a half of them (`tests::a_frame_draws_a_fraction_of_the_city`), and a
/// shadow cascade draws only the cells within its sides.
pub const PLACED_TRIANGLE_BUDGET: u64 = 1_850_000;
/// Where packs are committed, relative to the repository root.
pub const PACK_DIRECTORY: &str = "assets/verse/everglade";
/// The pack file extension.
pub const PACK_EXTENSION: &str = "vtp";
// Retain previous reviewed digests here when changing PACK_SHA256. Other
// zones share the cache directory; arbitrary digest names are not ours.
const EVERGLADE_PACK_HISTORY: &[&str] = &[
    PACK_SHA256,
    "12b5f3d7b48e655590119e2b55b42567375c553099df28ada93715be076226f4",
    "4bbd3b18ae0f698a37da40e738176b180e3d7b2a4e72944102424a16ce0b598c",
    "b57e33f733865ff639c87e6c0314f7e8f55c59d6ef313271880459ffb64bc49c",
];

/// The pack's content digest: the 32 bytes [`PACK_SHA256`] spells. The zone
/// loader verifies the pack against this pin, and a hosted Everglade
/// instance's login challenge carries the same digest, so a client and the
/// host agree on the content before a session starts.
///
/// # Errors
///
/// Returns a message when the pin is not 64 hexadecimal digits.
pub fn content_digest() -> Result<[u8; 32], String> {
    verse_world::social::world::content_digest(PACK_SHA256)
}

/// The reviewed pack and its source.
pub fn pinned() -> PinnedFile {
    PinnedFile {
        label: "Everglade pack",
        sha256: PACK_SHA256,
        bytes: PACK_BYTES,
        url: format!(
            "https://raw.githubusercontent.com/OpenAgentsInc/openagents/main/{PACK_DIRECTORY}/{PACK_SHA256}.{PACK_EXTENSION}"
        ),
        extension: PACK_EXTENSION,
        temp_prefix: ".everglade-",
        history: EVERGLADE_PACK_HISTORY,
    }
}

impl ZonePack {
    /// Verifies the pinned digest, then decodes the bounded pack.
    pub fn decode_pinned(bytes: &[u8]) -> Result<Self, String> {
        pinned().verify(bytes)?;
        format::decode(bytes, &Limits::EVERGLADE)
    }

    /// Loads the same verified pack from a local file for offline captures.
    pub fn load_local(path: &Path) -> Result<Self, String> {
        Self::decode_pinned(&pinned().read_bounded(path)?)
    }
}

/// A bounded transfer update or terminal result from the worker.
#[derive(Debug)]
pub enum LoadEvent {
    /// Bytes transferred from the reviewed source.
    Progress { received: u64, total: u64 },
    /// The pack passed its content, structure, and allocation checks.
    Ready(Box<ZonePack>),
    /// The current entry attempt failed and can be retried explicitly.
    Failed(String),
}

struct Worker {
    cancel: Arc<AtomicBool>,
    events: mpsc::Receiver<LoadEvent>,
    handle: JoinHandle<()>,
}

/// One lazy worker and one content-addressed cache entry per Everglade entry.
pub struct Loader {
    cache: PathBuf,
    worker: Option<Worker>,
}

impl Loader {
    /// Records a cache directory without opening files or starting a transfer.
    pub fn new(cache_path: PathBuf) -> Self {
        Self {
            cache: cache_path,
            worker: None,
        }
    }

    /// Starts one entry attempt. A canceled worker must finish before retrying.
    /// Whether [`Self::request`] would start a load now: no load is running
    /// and a canceled one has finished.
    pub fn idle(&mut self) -> bool {
        self.reap_canceled();
        self.worker.is_none()
    }

    pub fn request(&mut self) -> bool {
        self.reap_canceled();
        if self.worker.is_some() {
            return false;
        }
        let cache = self.cache.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (tx, events) = mpsc::channel();
        let handle = match std::thread::Builder::new()
            .name("verse-everglade-assets".into())
            .spawn(move || {
                let progress_tx = tx.clone();
                let mut progress = |received: u64, total: u64| {
                    let _ = progress_tx.send(LoadEvent::Progress { received, total });
                };
                let result = pinned().fetch(
                    &cache,
                    &worker_cancel,
                    &mut progress,
                    ZonePack::decode_pinned,
                );
                if !worker_cancel.load(Ordering::Acquire) {
                    let event = match result {
                        Ok(pack) => LoadEvent::Ready(Box::new(pack)),
                        Err(error) => LoadEvent::Failed(error),
                    };
                    let _ = tx.send(event);
                }
            }) {
            Ok(handle) => handle,
            Err(_) => return false,
        };
        self.worker = Some(Worker {
            cancel,
            events,
            handle,
        });
        true
    }

    /// Returns a queued update without blocking the render thread.
    pub fn poll(&mut self) -> Option<LoadEvent> {
        self.reap_canceled();
        let worker = self.worker.as_ref()?;
        if worker.cancel.load(Ordering::Acquire) {
            return None;
        }
        match worker.events.try_recv() {
            Ok(event) => {
                if matches!(event, LoadEvent::Ready(_) | LoadEvent::Failed(_)) {
                    // The terminal send is the worker's final operation.
                    self.worker.take();
                }
                Some(event)
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.worker.take();
                Some(LoadEvent::Failed(
                    "Everglade loader stopped before completion".into(),
                ))
            }
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }

    /// Cancels this generation. Late results cannot enter another zone.
    pub fn cancel(&mut self) {
        if let Some(worker) = &self.worker {
            worker.cancel.store(true, Ordering::Release);
        }
        self.reap_canceled();
    }

    fn reap_canceled(&mut self) {
        if self
            .worker
            .as_ref()
            .is_some_and(|w| w.cancel.load(Ordering::Acquire) && w.handle.is_finished())
            && let Some(worker) = self.worker.take()
        {
            let _ = worker.handle.join();
        }
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests;
