//! The pinned Everglade zone pack, loaded only after portal entry.
//!
//! The pack holds the admitted Quaternius models with base-color textures and
//! material flags (`format`), compiled from `assets/verse/everglade/` by
//! `compile`. It loads under the pinned loader's rules (`pinned`): HTTPS only,
//! no redirects, an exact length and digest, bounded decoding, and a
//! content-addressed disk cache. It carries no scripts, URLs, or authority.
//!
//! Rebuild the pack after changing an admitted source (after rebuilding a
//! generated model, readmit it first with
//! `python3 scripts/blender/everglade_admit.py`):
//!
//! ```text
//! cargo run --release -p verse --example everglade_pack -- assets/verse/everglade
//! ```
//!
//! then set [`PACK_SHA256`] and [`PACK_BYTES`] to the values it prints, and
//! add the previous digest to `EVERGLADE_PACK_HISTORY`.

pub mod compile;
pub mod format;
pub mod kit;
pub mod kit_bake;
pub mod pinned;
#[cfg(not(target_arch = "wasm32"))]
pub mod private_assets;

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
pub const PACK_SHA256: &str = "a82df378ca7d06d9c755ae24076c89270d8a8097509c54a166d941da05f9de2f";
/// Transfer size of the reviewed Everglade pack.
pub const PACK_BYTES: u64 = 10636202;
/// The most triangles the Everglade city may place where they draw: from
/// any point in the clearing, every merged 8 m cell at the level of detail
/// it draws at from there, in every direction and at every distance
/// (`zones::everglade::detail`). A building, kit piece, or tree draws its own
/// model within 80 m and a far level of 20 to 60 percent of its triangles
/// beyond, grass, flowers, and furniture draw only within 52 m, and shrubs,
/// deadwood, and ivy within 72 m. A frame draws less again: only the cells
/// in view, nearer than the fog's close at 180 m, and large enough to see at
/// their distance ([`DRAWN_TRIANGLE_BUDGET`]); a shadow cascade draws only
/// the cells within its sides, at the same levels. It was 1,250,000 before
/// the foliage round, which also moved the far levels' switch from 60 m to
/// 80 m and made them watertight, so houses at middle distances keep their
/// whole roofs and walls; a frame still draws within
/// [`DRAWN_TRIANGLE_BUDGET`].
pub const PLACED_TRIANGLE_BUDGET: u64 = 1_650_000;
/// The most triangles a street-level frame of the city may draw, from the
/// views `tests::a_frame_draws_a_fraction_of_the_city` measures: the frame
/// cost the levels of detail bound.
pub const DRAWN_TRIANGLE_BUDGET: u64 = 600_000;
/// The most triangles the city may place, every level of detail and the
/// ground counted, each instance counted in full: what the light bake
/// merges on the CPU and lights one texel a vertex
/// (`pbr::textured::MAX_MERGE_BYTES`). Trees and roofs are the largest
/// shares: a kit tree is 3,000 to 6,000 triangles, a house roof 2,464 (the
/// kit's thinned by `scripts/blender/kit_lod.py`), and a generated building
/// 7,800 to 19,200, about a tenth more once split on its block lattice
/// (`demolition::carve`). It was 2,900,000 while the renderer uploaded
/// every placement merged, about 209 MiB; repeated models now upload once
/// and draw as instances (`pbr::instanced`), so GPU memory is bounded on
/// its own ([`RESIDENT_BYTES_BUDGET`]) and this bound follows the bake.
pub const MERGED_TRIANGLE_BUDGET: u64 = 4_000_000;
/// The most bytes the city keeps on the GPU: merged cells, shared meshes,
/// instance records, and the light texture
/// (`pbr::textured::TexturedScene::gpu_bytes`). It leaves the low and
/// medium tiers' 320 MiB geometry budget 160 MiB, of which 64 MiB is the
/// reserve for destruction's debris and the character
/// (`verse_engine::quality::Budget::dynamic_geometry_bytes`). The city took
/// 209 MiB before instancing and the compact vertex, and about 117 MiB
/// after.
pub const RESIDENT_BYTES_BUDGET: u64 = 160 * 1024 * 1024;
/// Where packs are committed, relative to the repository root.
pub const PACK_DIRECTORY: &str = "assets/verse/everglade";
/// The pack file extension.
pub const PACK_EXTENSION: &str = "vtp";
// Retain previous reviewed digests here when changing PACK_SHA256. Other
// zones share the cache directory; arbitrary digest names are not ours.
const EVERGLADE_PACK_HISTORY: &[&str] = &[
    PACK_SHA256,
    "367da275afc505543d77841dd4f44efafbb6d784b783a17742985b13ce922fb7",
    "16df95549aca71e5707fab89031ab1491e2a9a1eb3e99a438835c66a419556d1",
    "b59146d570a9c33d883bda97d9d2dafe6d3ac093ccc3d07ea014d9c6713ab305",
    "d839b5a2dde5c44b7c53cef248869d5e7554dc5cf1e8f87fa987471671f25c3f",
    "f4d81b5746fc47507b16d07b230b78f9b95533b63c5431d5dadc3a182888ff17",
    "96dfe642f64ea116b6e0dbe3270969bdcb5df5bb3f85fc76a25e9b236a500e39",
    "52207ffbac04ab9c3a2c8a62ffcd9254e6ef4738e49e24046e73c3dafbd7eac7",
    "8c411484f7421585fe5b923e323306fd8f6e62a7410a5fb260c14c5486902bfc",
    "4cfbbe2bfe74bba084436b5a5fd8dc09ec0c770fcdd613f9ef92986749950abb",
    "a39c609a19caf4d5dea67031ad7d09c6d55683bc667d77aa59ee7ae52cf870ed",
    "97fcd942ba966ad9781e0db312c1d87441f2a2d87cfd14669898a808fae26a67",
    "d4f412c3cc22b0a0fe597152d2dbe9bc0d4508e8146dc0229a12f9750547708d",
    "cf21c824a787cf991a363c20c5a353478f6950a0c0d8201c9b8619588a01e9bf",
    "5c31644b18f99ec104be76d251b3d78705044b516365ecc7922d8e8dd33bb133",
    "cd10ed0eb0383fa054ed59cb3799143c2a49327047159c89a440679004da81f8",
    "760c51994331a5a0121ce635acf7239f8fe8acf699bc9382cab3d9f3665330a7",
    "47f3397f3a9edf6cb442be6abe3c84ec77486c513fbab7cbffcbb03d9f752914",
    "13d249d9decf596a38a7195420191aef05ba1382d3a494d2456912b158b5d022",
    "07518262b981824433b7a5dfdde4ace337eec29774a78b4f3632c235271aa6ee",
    "ad2d5b83a2fd57db6de98857cfc15622b9c47f604eab07c549499cc4f8ed8d07",
    "13812d5c3853d43560f4dd427971d717183f90420c3c993228129e0f9e9827af",
    "848d9685395916b2d400222820d3cca85de6d4aa3d3a759c148abe3e2ec5f1cb",
    "89203bcad2a6673b50754b0be679100ce19e461b1af01f52f3b109d2655fdb45",
    "06cc9dfc6474d060a990ae1e48cf8b955641c8892111085608dc9d5e687e7d61",
    "4dc0ae16b0367af352875a607fcb9aeaa9512be332c334eafa72772d3566cba0",
    "8ec5ae4fd86ac68a72894ddb24137cbc377a1445ae64277471037a8075d701d4",
    "e7c14a2626117be6723832ba6519edc14daad7a06da9de834def7ee633e3aa25",
    "0f05290ed5632d488337d115609aa7dd2d3df8b2765fd6e030a3657c60395ee3",
    "baa1a9ef9837d2a21b705cf5ef9657b07216ce47f889e511523caadd0be7f78b",
    "136a938924805db23211d5c98ac9a072b2b62ec7373b9ffa12f8eba59917912b",
    "a49bdf62aa3c014548bbc375fc66546030b4ee733e4832502dfd389533ea9f59",
    "df0e440c73d10dad949a036d68ecf0f42674a165d0749711751abc33f4f226e1",
    "2c6d0e58a10d2549b9fc897b31eec65b0af903eaa2f149a3126f15d387dc89df",
    "f29673c6a8fd0fe3728418084f373e077bb5dee72c87679d15e668918fbc4d46",
    "3bbdbfc043c8f0e02159e93190889b69fd5d58d873bbad990147d39a68b689e4",
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
    /// Verifies the pinned digest, then decodes the bounded pack, with the
    /// medieval kit's proxies in place of the licensed kit (`kit`).
    pub fn decode_pinned(bytes: &[u8]) -> Result<Self, String> {
        pinned().verify(bytes)?;
        let mut pack = format::decode(bytes, &Limits::EVERGLADE)?;
        kit::install(&mut pack, None);
        Ok(pack)
    }

    /// Loads the same verified pack from a local file for offline captures.
    /// With [`kit::LOCAL_ENV`] naming a kit pack, the licensed kit draws in
    /// place of its proxies, and with [`kit_bake::LOCAL_ENV`] naming a light
    /// layer file too, the town offers its layers ([`kit_bake::offer`]).
    pub fn load_local(path: &Path) -> Result<Self, String> {
        let mut pack = Self::decode_pinned(&pinned().read_bounded(path)?)?;
        if let Some(local) = std::env::var_os(kit::LOCAL_ENV) {
            let pieces = kit::load_local(Path::new(&local))?;
            let report = kit::install(&mut pack, Some(&pieces));
            if !report.refused.is_empty() {
                eprintln!(
                    "verse: kit pieces outside their boxes: {:?}",
                    report.refused
                );
            }
            if let Some(layers) = std::env::var_os(kit_bake::LOCAL_ENV) {
                kit_bake::offer(kit_bake::load_local(Path::new(&layers))?);
            }
        }
        Ok(pack)
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
    download_kit: bool,
    kit_tier: kit::Tier,
}

impl Loader {
    /// Records a cache directory without opening files or starting a transfer.
    pub fn new(cache_path: PathBuf) -> Self {
        Self {
            cache: cache_path,
            worker: None,
            download_kit: false,
            kit_tier: kit::Tier::Full,
        }
    }

    /// Whether an entry downloads the pinned kit pack when the cache lacks
    /// it. Off by default, so tests and tools never reach the network; the
    /// kit in the cache is used either way.
    /// Which kit and light files this client fetches (#10908).
    pub fn kit_tier(&mut self, tier: kit::Tier) {
        self.kit_tier = tier;
    }
    pub fn download_kit(&mut self, download: bool) {
        self.download_kit = download;
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
        let download_kit = self.download_kit;
        let tier = self.kit_tier;
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
                let result = pinned()
                    .fetch(
                        &cache,
                        &worker_cancel,
                        &mut progress,
                        ZonePack::decode_pinned,
                    )
                    .map(|mut pack| {
                        // The licensed kit when it is cached or published;
                        // its committed proxies otherwise.
                        if let Ok(pieces) =
                            kit::fetch_tier(&cache, download_kit, &worker_cancel, tier)
                        {
                            kit::install(&mut pack, Some(&pieces));
                            // The kit town's baked light, when published;
                            // the town bakes at load otherwise.
                            if let Ok(layers) =
                                kit_bake::fetch_tier(&cache, download_kit, &worker_cancel, tier)
                            {
                                kit_bake::offer(layers);
                            }
                        }
                        pack
                    });
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
