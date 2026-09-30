//! The Windows observation walk: handle-relative, and never following a
//! link.
//!
//! The same walk as on Unix, in Windows terms. The root is opened one
//! component at a time from its drive, each relative to its parent's
//! handle, so no component may be a link. Every child is then opened
//! with `NtCreateFile` relative to its parent directory's handle under
//! `FILE_OPEN_REPARSE_POINT`, and the opened handle's attributes confirm
//! its kind: a symbolic link, junction, or other reparse point raced into
//! place is a fault, never a followed link. A directory's handle is taken
//! when its entry is found, so a path swapped afterwards cannot redirect
//! its listing, and a file's identity, length, write time, and attributes
//! are read before and after it is hashed.
//!
//! A reparse point that names another path (a symbolic link or a
//! junction) is recorded as a link with the target its reparse data
//! holds, read from the point itself; any other reparse point is
//! recorded by its tag and data. Neither is opened as what it names.
//!
//! An entry's identity is the volume serial number and file index, which
//! a rename keeps. Its mode is the read-only, hidden, and system
//! attributes. Digests are not reused between observations here: a
//! Windows file's change time can be set by its owner, so it cannot prove
//! that a file is unchanged the way a Unix status-change time does, and
//! every file is read each time.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use sha2::{Digest, Sha256};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_READONLY,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_SYSTEM,
};

use super::{Entry, Fault, Id, Limits, Snapshot};
use crate::windows::nt::{self, Listed, Want};

/// The most faults one walk itemizes, as on Unix.
const FAULT_MAX: usize = 64;

/// The most file workers one walk runs.
const WORKERS_MAX: usize = 8;

/// The most files queued for the workers at once.
const QUEUED_MAX: usize = 64;

/// The attributes an entry's mode records.
const MODE_ATTRIBUTES: u32 =
    FILE_ATTRIBUTE_READONLY | FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM;

struct Walk {
    entries: BTreeMap<PathBuf, Entry>,
    faults: Vec<Fault>,
    attempts: usize,
    limits: Limits,
    /// The root's volume, for entries identified from the listing alone.
    volume: u32,
}

impl Walk {
    fn fault(&mut self, fault: Fault) {
        if self.faults.len() < FAULT_MAX {
            self.faults.push(fault);
        }
    }

    fn read(&mut self, path: PathBuf, error: impl std::fmt::Display) {
        self.fault(Fault::Read {
            path,
            error: error.to_string(),
        });
    }
}

fn finish(walk: Walk, root: PathBuf) -> Snapshot {
    Snapshot {
        root,
        entries: walk.entries,
        faults: walk.faults,
    }
}

/// The walk behind [`Snapshot::observe_within`].
pub(super) fn walk(root: &Path, limits: Limits) -> Snapshot {
    let mut walk = Walk {
        entries: BTreeMap::new(),
        faults: Vec::new(),
        attempts: 0,
        limits,
        volume: 0,
    };
    let canonical = match root.canonicalize() {
        Ok(canonical) => canonical,
        Err(error) => {
            walk.fault(Fault::Root {
                path: root.to_path_buf(),
                error: error.to_string(),
            });
            return finish(walk, root.to_path_buf());
        }
    };
    let root_dir = match nt::open_dir(&canonical).and_then(|dir| Ok((nt::info(&dir)?, dir))) {
        Ok((info, dir)) => {
            walk.volume = info.volume;
            walk.entries.insert(
                PathBuf::new(),
                Entry::Directory {
                    id: Some((u64::from(info.volume), info.index)),
                    mode: Some(info.attributes & MODE_ATTRIBUTES),
                },
            );
            walk.attempts = 1;
            dir
        }
        Err(error) => {
            walk.fault(Fault::Root {
                path: canonical,
                error: error.to_string(),
            });
            return finish(walk, root.to_path_buf());
        }
    };

    let hashed = AtomicU64::new(0);
    let over = AtomicBool::new(false);
    let workers = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .clamp(1, WORKERS_MAX);
    let (jobs, queue) = mpsc::sync_channel::<Job>(QUEUED_MAX);
    let queue = Mutex::new(queue);
    let (done, results) = mpsc::channel::<(PathBuf, Hashed)>();
    std::thread::scope(|scope| {
        for _ in 0..workers {
            let (queue, done, hashed, over) = (&queue, done.clone(), &hashed, &over);
            scope.spawn(move || {
                loop {
                    let job = match queue.lock() {
                        Ok(queue) => queue.recv(),
                        Err(_) => return,
                    };
                    let Ok(job) = job else { return };
                    let result = if over.load(Ordering::Relaxed) {
                        Hashed::Bytes
                    } else {
                        file(&job.dir, &job.name, limits.bytes, hashed)
                    };
                    if matches!(result, Hashed::Bytes) {
                        over.store(true, Ordering::Relaxed);
                    }
                    if done.send((job.path, result)).is_err() {
                        return;
                    }
                }
            });
        }
        list(&mut walk, root_dir, &jobs, &over);
        drop(jobs);
    });
    drop(done);
    let mut faults: Vec<(PathBuf, Fault)> = Vec::new();
    let mut bytes = false;
    for (path, result) in results {
        match result {
            Hashed::File(entry) => {
                walk.entries.insert(path, entry);
            }
            Hashed::Read(error) => faults.push((path.clone(), Fault::Read { path, error })),
            Hashed::Bytes => bytes = true,
        }
    }
    faults.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, fault) in faults {
        walk.fault(fault);
    }
    if bytes {
        walk.fault(Fault::Bytes {
            limit: limits.bytes,
        });
    }
    finish(walk, canonical)
}

struct Job {
    dir: Arc<File>,
    name: Vec<u16>,
    path: PathBuf,
}

enum Hashed {
    File(Entry),
    Read(String),
    Bytes,
}

fn list(walk: &mut Walk, root: File, jobs: &mpsc::SyncSender<Job>, over: &AtomicBool) {
    let limits = walk.limits;
    let mut pending: Vec<(PathBuf, File)> = vec![(PathBuf::new(), root)];
    'walk: while let Some((rel, dir)) = pending.pop() {
        if over.load(Ordering::Relaxed) {
            break;
        }
        let dir = Arc::new(dir);
        if walk.attempts > limits.entries {
            walk.fault(Fault::Entries {
                limit: limits.entries,
            });
            break;
        }
        let names = match nt::listing(&dir, limits.entries - walk.attempts) {
            Ok((_, true)) => {
                walk.fault(Fault::Entries {
                    limit: limits.entries,
                });
                break;
            }
            Ok((names, false)) => names,
            Err(error) => {
                walk.read(rel, error);
                continue;
            }
        };
        for listed in names {
            walk.attempts += 1;
            if walk.attempts > limits.entries {
                walk.fault(Fault::Entries {
                    limit: limits.entries,
                });
                break 'walk;
            }
            let path = rel.join(listed.name_os());
            if !nt::plain_name(&listed.name) {
                walk.read(path, "a name that cannot be opened as one entry");
                continue;
            }
            if listed.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                reparse(walk, &dir, &listed, path);
            } else if listed.attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
                match nt::open_at(&dir, &listed.name, Want::Directory)
                    .and_then(|child| Ok((nt::info(&child)?, child)))
                {
                    Ok((info, child)) => {
                        walk.entries.insert(
                            path.clone(),
                            Entry::Directory {
                                id: Some((u64::from(info.volume), info.index)),
                                mode: Some(info.attributes & MODE_ATTRIBUTES),
                            },
                        );
                        pending.push((path, child));
                    }
                    Err(error) => walk.read(path, error),
                }
            } else {
                let job = Job {
                    dir: dir.clone(),
                    name: listed.name,
                    path,
                };
                if jobs.send(job).is_err() || over.load(Ordering::Relaxed) {
                    break 'walk;
                }
            }
        }
    }
}

/// Records a reparse point by what it holds, never by what it names.
fn reparse(walk: &mut Walk, dir: &File, listed: &Listed, path: PathBuf) {
    let id: Option<Id> = Some((u64::from(walk.volume), listed.file_id));
    let mode = Some(listed.attributes & MODE_ATTRIBUTES);
    // A symbolic link or junction comes back as the path it holds; any
    // other reparse point (a cloud placeholder, a deduplicated file) as its
    // tag and data, which stand for its contents. Neither is opened as what
    // it names.
    match nt::link_target(dir, &listed.name) {
        Ok(target) => {
            walk.entries.insert(path, Entry::Link { id, target, mode });
        }
        Err(error) => walk.read(path, error),
    }
}

/// What about an open file a write to it moves.
#[derive(PartialEq, Eq)]
struct Stamp {
    volume: u32,
    index: u64,
    length: u64,
    modified: Option<std::time::SystemTime>,
    attributes: u32,
}

fn stamp(file: &File) -> std::io::Result<Stamp> {
    let info = nt::info(file)?;
    let metadata = file.metadata()?;
    Ok(Stamp {
        volume: info.volume,
        index: info.index,
        length: metadata.len(),
        modified: metadata.modified().ok(),
        attributes: info.attributes,
    })
}

/// One file, hashed under the byte bound and checked for a write that
/// landed mid-read. The entry comes from the open handle, which pins the
/// file that was actually hashed.
fn file(dir: &File, name: &[u16], cap: u64, hashed: &AtomicU64) -> Hashed {
    let mut file = match nt::open_at(dir, name, Want::File) {
        Ok(file) => file,
        Err(error) => return Hashed::Read(error.to_string()),
    };
    let before = match stamp(&file) {
        Ok(before) => before,
        Err(error) => return Hashed::Read(error.to_string()),
    };
    let mut sha = Sha256::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let read = match file.read(&mut chunk) {
            Ok(read) => read,
            Err(error) => return Hashed::Read(error.to_string()),
        };
        if read == 0 {
            break;
        }
        if hashed.fetch_add(read as u64, Ordering::Relaxed) + read as u64 > cap {
            return Hashed::Bytes;
        }
        sha.update(&chunk[..read]);
    }
    match stamp(&file) {
        Ok(after) if after == before => {}
        Ok(_) => return Hashed::Read("changed while it was being read".into()),
        Err(error) => return Hashed::Read(error.to_string()),
    }
    Hashed::File(Entry::File {
        id: Some((u64::from(before.volume), before.index)),
        length: before.length,
        digest: sha.finalize().into(),
        modified: before.modified,
        mode: Some(before.attributes & MODE_ATTRIBUTES),
    })
}
