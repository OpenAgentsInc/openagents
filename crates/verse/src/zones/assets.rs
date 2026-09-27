//! A pinned forest asset pack, loaded only after explicit portal entry.
//!
//! The pack contains bounded geometry and sampled animation frames. It carries
//! no executable scripts, URLs, textures, or application authority.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::mesh::{Mesh, Vertex};

/// Exact content identity of the reviewed forest pack.
pub const PACK_SHA256: &str = "7c1535256a4687e70a0f624f4b97c651bfd0b36ba91347a09041698ef8d246a7";
/// Transfer size of the reviewed forest pack.
pub const PACK_BYTES: u64 = 6_629_578;
const PACK_URL: &str = "https://raw.githubusercontent.com/OpenAgentsInc/openagents/main/assets/verse/forest/7c1535256a4687e70a0f624f4b97c651bfd0b36ba91347a09041698ef8d246a7.vzp";
const MAX_PACK_BYTES: u64 = 25 * 1024 * 1024;
const MAX_DECODED_BYTES: usize = 96 * 1024 * 1024;
const MAX_VERTICES: usize = 400_000;
const MAX_FRAMES: usize = 24;
const MAGIC: &[u8; 8] = b"VZP1\r\n\x1a\n";
static TEMP_ID: AtomicU64 = AtomicU64::new(0);
// Retain previous reviewed forest digests here when changing PACK_SHA256.
// Other zones can share the cache directory; arbitrary digest names are not ours.
const FOREST_PACK_HISTORY: &[&str] = &[PACK_SHA256];
#[cfg(unix)]
const STALE_TEMP_SECONDS: i64 = 24 * 60 * 60;

/// Sampled poses from an original glTF skeletal animation.
#[derive(Debug)]
pub struct AnimatedMesh {
    /// Frames in playback order. The validated pack always contains a frame.
    pub frames: Vec<Mesh>,
    /// Time between frames, in seconds.
    pub frame_seconds: f32,
}

impl AnimatedMesh {
    /// Returns a looping pose. Invalid clocks use the first frame.
    pub fn sample(&self, seconds: f32) -> &Mesh {
        let index = if seconds.is_finite() && seconds >= 0.0 {
            ((seconds / self.frame_seconds) as usize) % self.frames.len()
        } else {
            0
        };
        &self.frames[index]
    }
}

/// Original forest meshes in meters, with feet at the local ground plane.
#[derive(Debug)]
pub struct LoadedAssets {
    /// An 8-meter tree with baked leaf cutouts.
    pub tree: Mesh,
    /// The original wizard's zero-duration Still pose, normalized to 1.8 meters.
    pub wizard_still: Mesh,
    /// The original wizard's Waiting animation.
    pub wizard: AnimatedMesh,
    /// The original zombie's Idle animation, normalized to 1.8 meters.
    pub zombie: AnimatedMesh,
    /// The original zombie's Walk animation.
    pub zombie_walk: AnimatedMesh,
}

impl LoadedAssets {
    /// Verifies the pinned digest, then decodes the bounded pack.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        verify_bytes(bytes)?;
        Self::decode_structure(bytes)
    }

    /// Loads the same verified pack from a local file for offline captures.
    pub fn load_local(path: &Path) -> Result<Self, String> {
        Self::decode(&read_bounded(path)?)
    }

    fn decode_structure(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_PACK_BYTES {
            return Err("Forest pack exceeds the transfer limit".into());
        }
        let mut reader = PackReader {
            bytes,
            offset: 0,
            decoded_bytes: 0,
        };
        if reader.take(8)? != MAGIC {
            return Err("Forest pack has an unsupported format".into());
        }
        let tree = reader.mesh()?;
        let wizard_still = reader.mesh()?;
        let wizard = reader.animation()?;
        let zombie = reader.animation()?;
        let zombie_walk = reader.animation()?;
        if reader.offset != bytes.len() {
            return Err("Forest pack has trailing data".into());
        }
        Ok(Self {
            tree,
            wizard_still,
            wizard,
            zombie,
            zombie_walk,
        })
    }
}

struct PackReader<'a> {
    bytes: &'a [u8],
    offset: usize,
    decoded_bytes: usize,
}

impl<'a> PackReader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or("Forest pack size overflow")?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or("Forest pack is truncated")?;
        self.offset = end;
        Ok(value)
    }

    fn u16(&mut self) -> Result<u16, String> {
        let v = self.take(2)?;
        Ok(u16::from_le_bytes([v[0], v[1]]))
    }

    fn u32(&mut self) -> Result<u32, String> {
        let v = self.take(4)?;
        Ok(u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
    }

    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32()?))
    }

    fn mesh(&mut self) -> Result<Mesh, String> {
        let count = self.u32()? as usize;
        if count == 0 || count > MAX_VERTICES || !count.is_multiple_of(3) {
            return Err("Forest mesh has an invalid vertex count".into());
        }
        self.decoded_bytes = self
            .decoded_bytes
            .checked_add(count * size_of::<Vertex>())
            .ok_or("Forest mesh size overflow")?;
        if self.decoded_bytes > MAX_DECODED_BYTES {
            return Err("Forest pack exceeds the decoded memory limit".into());
        }
        let packed = self.take(count * 15)?;
        let mut faces = Vec::new();
        faces
            .try_reserve_exact(count)
            .map_err(|_| "Forest mesh allocation failed")?;
        for v in packed.chunks_exact(15) {
            let read_float = |at| f32::from_le_bytes([v[at], v[at + 1], v[at + 2], v[at + 3]]);
            let pos = [read_float(0), read_float(4), read_float(8)];
            if pos.iter().any(|n| !n.is_finite() || n.abs() > 32.0) {
                return Err("Forest mesh has an invalid position".into());
            }
            faces.push(Vertex {
                pos,
                color: [
                    v[12] as f32 / 255.0,
                    v[13] as f32 / 255.0,
                    v[14] as f32 / 255.0,
                ],
                fog: 1.0,
            });
        }
        Ok(Mesh {
            faces,
            lines: Vec::new(),
        })
    }

    fn animation(&mut self) -> Result<AnimatedMesh, String> {
        let count = self.u16()? as usize;
        let frame_seconds = self.f32()?;
        if count == 0
            || count > MAX_FRAMES
            || !frame_seconds.is_finite()
            || !(0.02..=5.0).contains(&frame_seconds)
        {
            return Err("Forest animation has invalid timing".into());
        }
        let frames = (0..count)
            .map(|_| self.mesh())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AnimatedMesh {
            frames,
            frame_seconds,
        })
    }
}

/// A bounded transfer update or terminal result from the worker.
#[derive(Debug)]
pub enum LoadEvent {
    /// Bytes transferred from the reviewed source.
    Progress { received: u64, total: u64 },
    /// All geometry passed content and allocation checks.
    Ready(Box<LoadedAssets>),
    /// The current entry attempt failed and can be retried explicitly.
    Failed(String),
}

struct Worker {
    cancel: Arc<AtomicBool>,
    events: mpsc::Receiver<LoadEvent>,
    handle: JoinHandle<()>,
}

/// One lazy worker and one content-addressed cache entry per forest loader.
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
            .name("verse-forest-assets".into())
            .spawn(move || {
                let result = load(&cache, &worker_cancel, &tx);
                if !worker_cancel.load(Ordering::Acquire) {
                    let event = match result {
                        Ok(assets) => LoadEvent::Ready(Box::new(assets)),
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
                    // The terminal send is the worker's final operation. Dropping
                    // its handle avoids waiting for thread teardown on the UI.
                    self.worker.take();
                }
                Some(event)
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.worker.take();
                Some(LoadEvent::Failed(
                    "Forest loader stopped before completion".into(),
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

fn verify_bytes(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() as u64 != PACK_BYTES || bytes.len() as u64 > MAX_PACK_BYTES {
        return Err("Forest pack has an unexpected size".into());
    }
    if format!("{:x}", Sha256::digest(bytes)) != PACK_SHA256 {
        return Err("Forest pack failed its content check".into());
    }
    Ok(())
}

#[cfg(unix)]
fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    use std::os::unix::fs::OpenOptionsExt;
    // Inspect the opened file, not a path checked before a potentially racing open.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "Forest cache could not be opened")?;
    let metadata = file.metadata().map_err(|_| "Forest cache is unavailable")?;
    if !metadata.is_file() || metadata.len() != PACK_BYTES {
        return Err("Forest cache has an unexpected size or file type".into());
    }
    let mut bytes = Vec::with_capacity(PACK_BYTES as usize);
    file.take(PACK_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Forest cache could not be read")?;
    verify_bytes(&bytes)?;
    Ok(bytes)
}

#[cfg(not(unix))]
fn read_bounded(_path: &Path) -> Result<Vec<u8>, String> {
    Err("Forest file loading requires no-follow file support on this platform".into())
}

fn canceled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err("Forest entry canceled".into())
    } else {
        Ok(())
    }
}

fn load(
    cache: &Path,
    cancel: &AtomicBool,
    tx: &mpsc::Sender<LoadEvent>,
) -> Result<LoadedAssets, String> {
    canceled(cancel)?;
    validate_cache_directory(cache)?;
    let path = cache.join(format!("{PACK_SHA256}.vzp"));
    if let Ok(bytes) = read_bounded(&path) {
        canceled(cancel)?;
        if let Ok(assets) = LoadedAssets::decode(&bytes) {
            canceled(cancel)?;
            prune_cache(cache, FOREST_PACK_HISTORY);
            return Ok(assets);
        }
        // Leave the old entry in place until a downloaded replacement passes
        // every check. Explicit retry can repair an unreadable cache entry.
    }
    canceled(cancel)?;
    let client = reqwest::blocking::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(12))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|_| "Forest download could not start")?;
    let mut response = client
        .get(PACK_URL)
        .send()
        .map_err(|_| "Forest download could not connect")?;
    if !response.status().is_success() {
        return Err("Forest download is unavailable; try again later".into());
    }
    if response
        .content_length()
        .is_some_and(|size| size != PACK_BYTES)
    {
        return Err("Forest download has an unexpected size".into());
    }
    let mut bytes = Vec::with_capacity(PACK_BYTES as usize);
    let mut block = [0; 64 * 1024];
    let mut last_update = 0;
    loop {
        canceled(cancel)?;
        let count = response
            .read(&mut block)
            .map_err(|_| "Forest download was interrupted")?;
        if count == 0 {
            break;
        }
        if bytes.len() + count > PACK_BYTES as usize {
            return Err("Forest download exceeds its declared size".into());
        }
        bytes.extend_from_slice(&block[..count]);
        // At most 100 progress records under the transfer budget.
        if bytes.len() - last_update >= 256 * 1024 || bytes.len() as u64 == PACK_BYTES {
            let _ = tx.send(LoadEvent::Progress {
                received: bytes.len() as u64,
                total: PACK_BYTES,
            });
            last_update = bytes.len();
        }
    }
    canceled(cancel)?;
    let assets = LoadedAssets::decode(&bytes)?;
    canceled(cancel)?;
    install_cache(cache, &bytes, cancel)?;
    Ok(assets)
}

fn install_cache(cache: &Path, bytes: &[u8], cancel: &AtomicBool) -> Result<(), String> {
    verify_bytes(bytes)?;
    canceled(cancel)?;
    validate_cache_directory(cache)?;
    std::fs::create_dir_all(cache).map_err(|_| "Forest cache directory could not be created")?;
    if !std::fs::symlink_metadata(cache).is_ok_and(|m| m.is_dir()) {
        return Err("Forest cache directory must not be a symbolic link".into());
    }
    let final_path = cache.join(format!("{PACK_SHA256}.vzp"));
    let temp = cache.join(format!(
        ".forest-{}-{}.part",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temp)
            .map_err(|_| "Forest cache could not be created")?;
        file.write_all(bytes)
            .map_err(|_| "Forest cache could not be written")?;
        file.sync_all()
            .map_err(|_| "Forest cache could not be saved")?;
        canceled(cancel)?;
        std::fs::rename(&temp, &final_path).map_err(|_| "Forest cache could not be installed")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    } else {
        // Cleanup is best effort after publication. Failure cannot invalidate a
        // verified scene, and unknown files never become cleanup candidates.
        prune_cache(cache, FOREST_PACK_HISTORY);
    }
    result
}

fn validate_cache_directory(cache: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(cache) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err("Forest cache directory must be a regular directory".into()),
    }
}

#[cfg(unix)]
fn forest_temp_name(name: &str) -> bool {
    let Some(body) = name
        .strip_prefix(".forest-")
        .and_then(|s| s.strip_suffix(".part"))
    else {
        return false;
    };
    let Some((pid, counter)) = body.split_once('-') else {
        return false;
    };
    [pid, counter]
        .iter()
        .all(|s| !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit()))
}

/// Prune only named previous forest revisions and abandoned forest temp files.
/// Directory-relative unlinking cannot follow a replaced cache-directory path.
#[cfg(unix)]
fn prune_cache(cache: &Path, history: &[&str]) {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;
    let Ok(directory) = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(cache)
    else {
        return;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs().min(i64::MAX as u64) as i64);
    let remove = |name: &str, temporary: bool| {
        let Ok(name) = std::ffi::CString::new(name) else {
            return;
        };
        let mut state = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: the directory descriptor remains live, name is NUL terminated,
        // and state points to writable storage for exactly one stat record.
        let result = unsafe {
            libc::fstatat(
                directory.as_raw_fd(),
                name.as_ptr(),
                state.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if result != 0 {
            return;
        }
        // SAFETY: successful fstatat initialized the complete stat record.
        let state = unsafe { state.assume_init() };
        if state.st_mode & libc::S_IFMT != libc::S_IFREG {
            return;
        }
        if temporary
            && (state.st_size < 0
                || state.st_size as u64 > MAX_PACK_BYTES
                || now.saturating_sub(state.st_mtime) < STALE_TEMP_SECONDS)
        {
            return;
        }
        // SAFETY: name has no path separators and the directory stays open.
        // Flags zero unlinks one entry; it never follows a symlink or recurses.
        let _ = unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), 0) };
    };
    for digest in history.iter().take(32).filter(|&&d| d != PACK_SHA256) {
        if digest.len() == 64
            && digest
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            remove(&format!("{digest}.vzp"), false);
        }
    }
    // The cap bounds cleanup work even if unrelated software fills this directory.
    if let Ok(entries) = std::fs::read_dir(cache) {
        for entry in entries.take(256).flatten() {
            if let Some(name) = entry
                .file_name()
                .to_str()
                .filter(|name| forest_temp_name(name))
            {
                remove(name, true);
            }
        }
    }
}

#[cfg(not(unix))]
fn prune_cache(_cache: &Path, _history: &[&str]) {}

#[cfg(test)]
mod tests;
