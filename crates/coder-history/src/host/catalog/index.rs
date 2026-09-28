//! The catalog's index on disk: what it read of each source (its first
//! record and its first prompt), keyed by root ID and path and kept with the
//! file identity and length it was read at. A new process loads it into the
//! same memory the catalog keeps, where every entry is checked against the
//! file's current stat before it is used exactly as one read moments ago.
//! Nothing in it is authority: a missing, unreadable, oversized, or
//! unrecognized index is ignored, and the catalog reads the sources again.

use super::{HEADS, KnownHead, PROMPTS, memo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const SCHEMA: &str = "openagents.coder-history.catalog-index.v1";
const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
pub(super) struct Index {
    schema: String,
    heads: Vec<HeadRow>,
    prompts: Vec<PromptRow>,
}

#[derive(Serialize, Deserialize)]
struct HeadRow {
    root: String,
    path: String,
    dev: u64,
    ino: u64,
    size: u64,
    settled: bool,
    native: Option<String>,
    title: Option<String>,
    engine: bool,
    spawned: bool,
}

#[derive(Serialize, Deserialize)]
struct PromptRow {
    root: String,
    path: String,
    dev: u64,
    ino: u64,
    size: u64,
    settled: bool,
    prompt: Option<String>,
}

/// Counts what the catalog learned: each head or first prompt it read.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The index files this process has loaded, and the generation each one
/// last held.
static FILES: std::sync::Mutex<Option<HashMap<PathBuf, u64>>> = std::sync::Mutex::new(None);

fn files() -> std::sync::MutexGuard<'static, Option<HashMap<PathBuf, u64>>> {
    FILES.lock().unwrap_or_else(|p| p.into_inner())
}

pub(super) fn changed() {
    GENERATION.fetch_add(1, Ordering::AcqRel);
}

/// Load `path` into memory once per process. What memory already holds
/// wins: it was read later.
pub(super) fn load(path: &Path) {
    {
        let mut files = files();
        let files = files.get_or_insert_with(HashMap::new);
        if files.contains_key(path) {
            return;
        }
        // What memory holds now counts as written: a page writes the index
        // again once it reads something new.
        files.insert(path.to_path_buf(), GENERATION.load(Ordering::Acquire));
    }
    let Some(index) = read(path) else {
        return;
    };
    {
        let mut heads = memo(&HEADS);
        for row in index.heads {
            heads
                .entry((row.root, PathBuf::from(row.path)))
                .or_insert(KnownHead {
                    dev: row.dev,
                    ino: row.ino,
                    size: row.size,
                    settled: row.settled,
                    native: row.native,
                    title: row.title,
                    engine: row.engine,
                    spawned: row.spawned,
                });
        }
    }
    let mut prompts = memo(&PROMPTS);
    for row in index.prompts {
        prompts
            .entry((row.root, PathBuf::from(row.path)))
            .or_insert((row.dev, row.ino, row.size, row.settled, row.prompt));
    }
}

pub(super) fn read(path: &Path) -> Option<Index> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_INDEX_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_INDEX_BYTES {
        return None;
    }
    serde_json::from_slice::<Index>(&bytes)
        .ok()
        .filter(|index| index.schema == SCHEMA)
}

/// Write memory to `path` when it holds something new: a private file
/// written beside it and renamed over it, so a reader sees one whole index.
pub(super) fn save(path: &Path) {
    let generation = GENERATION.load(Ordering::Acquire);
    if files()
        .as_ref()
        .and_then(|files| files.get(path))
        .is_some_and(|saved| *saved == generation)
    {
        return;
    }
    let index = Index {
        schema: SCHEMA.into(),
        heads: memo(&HEADS)
            .iter()
            .filter_map(|((root, relative), h)| {
                Some(HeadRow {
                    root: root.clone(),
                    path: relative.to_str()?.to_owned(),
                    dev: h.dev,
                    ino: h.ino,
                    size: h.size,
                    settled: h.settled,
                    native: h.native.clone(),
                    title: h.title.clone(),
                    engine: h.engine,
                    spawned: h.spawned,
                })
            })
            .collect(),
        prompts: memo(&PROMPTS)
            .iter()
            .filter_map(|((root, relative), (dev, ino, size, settled, prompt))| {
                Some(PromptRow {
                    root: root.clone(),
                    path: relative.to_str()?.to_owned(),
                    dev: *dev,
                    ino: *ino,
                    size: *size,
                    settled: *settled,
                    prompt: prompt.clone(),
                })
            })
            .collect(),
    };
    // A failed write is tried again after the next page.
    if write(path, &index).is_ok() {
        files()
            .get_or_insert_with(HashMap::new)
            .insert(path.to_path_buf(), generation);
    }
}

fn write(path: &Path, index: &Index) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let bytes = serde_json::to_vec(index)?;
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("index path has no name"))?;
    let mut temporary = name.to_owned();
    temporary.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let temporary = path.with_file_name(temporary);
    let result = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .and_then(|mut file| file.write_all(&bytes))
        .and_then(|()| std::fs::rename(&temporary, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
pub(super) fn forget() {
    *files() = None;
}
