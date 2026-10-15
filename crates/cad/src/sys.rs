//! What the engine needs from the machine, the same natively and in the
//! browser (wasm32): files (the disk natively, the kernel's in-memory file
//! store in the browser), work side by side (threads natively, one after
//! another in the browser) and a clock for timings (none in the browser).

use std::path::Path;

use solidcraft_engine::io::vfs;

/// The largest file the engine reads back (a part's STEP, its notes).
const MAX_READ: usize = 512 << 20;

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Whether a file is there.
#[must_use]
pub fn exists(path: &Path) -> bool {
    vfs::exists(&text(path))
}

/// A file's bytes, if it's there.
#[must_use]
pub fn read(path: &Path) -> Option<Vec<u8>> {
    vfs::read(&text(path), MAX_READ).ok()
}

/// A file's text, if it's there.
#[must_use]
pub fn read_text(path: &Path) -> Option<String> {
    read(path).and_then(|b| String::from_utf8(b).ok())
}

/// Makes a folder (and its parents); nothing to make in the browser.
///
/// # Errors
/// The folder can't be made.
pub fn ensure_dir(path: &Path) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::create_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
        Ok(())
    }
}

/// Writes a file whole: natively through a temporary file and a rename,
/// so a reader never sees half of it.
///
/// # Errors
/// The file can't be written.
pub fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(dir) = path.parent() {
            ensure_dir(dir)?;
        }
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&temporary, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        std::fs::rename(&temporary, path).map_err(|e| format!("{}: {e}", path.display()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        vfs::write(&text(path), bytes).map_err(|e| e.to_string())
    }
}

/// Moves a file.
///
/// # Errors
/// It isn't there, or can't be moved.
pub fn rename(from: &Path, to: &Path) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::rename(from, to).map_err(|e| e.to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let bytes = read(from).ok_or_else(|| format!("{} isn't there", from.display()))?;
        vfs::write(&text(to), &bytes).map_err(|e| e.to_string())?;
        vfs::remove(&text(from)).map_err(|e| e.to_string())
    }
}

/// `f` over every item, side by side where there are threads (one thread
/// per item, at most `limit` at once), one after another in the browser;
/// the results in the items' order, `None` where `f` panicked.
pub fn parallel<I: Sync, R: Send>(
    items: &[I],
    limit: usize,
    f: impl Fn(&I) -> R + Sync,
) -> Vec<Option<R>> {
    let guarded =
        |item: &I| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(item))).ok();
    #[cfg(not(target_arch = "wasm32"))]
    {
        if items.len() <= 1 {
            return items.iter().map(guarded).collect();
        }
        let size = items.len().div_ceil(limit.max(1)).max(1);
        let guarded = &guarded;
        std::thread::scope(|scope| {
            let jobs: Vec<_> = items
                .chunks(size)
                .map(|chunk| {
                    scope.spawn(move || chunk.iter().map(guarded).collect::<Vec<Option<R>>>())
                })
                .collect();
            jobs.into_iter()
                .flat_map(|job| job.join().unwrap_or_default())
                .collect()
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = limit;
        items.iter().map(guarded).collect()
    }
}

/// This process's id, to keep temporary files apart (0 in the browser).
#[must_use]
pub fn pid() -> u32 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::process::id()
    }
    #[cfg(target_arch = "wasm32")]
    {
        0
    }
}

/// How many threads this machine runs well at once (at most 12).
#[must_use]
pub fn cores() -> usize {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .clamp(1, 12)
    }
    #[cfg(target_arch = "wasm32")]
    {
        1
    }
}

/// A stopwatch: milliseconds since it started (0 in the browser, which has
/// no clock the engine may read without the page's help).
pub struct Timer {
    #[cfg(not(target_arch = "wasm32"))]
    started: std::time::Instant,
}

impl Timer {
    #[must_use]
    pub fn start() -> Self {
        Timer {
            #[cfg(not(target_arch = "wasm32"))]
            started: std::time::Instant::now(),
        }
    }

    #[must_use]
    pub fn ms(&self) -> u64 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.started.elapsed().as_millis() as u64
        }
        #[cfg(target_arch = "wasm32")]
        {
            0
        }
    }
}
