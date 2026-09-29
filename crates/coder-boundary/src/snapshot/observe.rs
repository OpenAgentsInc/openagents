//! The Unix observation walk: descriptor-relative, and never following
//! a link.
//!
//! A path-based walk has the hole twice over. `symlink_metadata` then
//! `File::open` opens whatever the path names at open time — a link
//! swapped in between gets followed into whatever it names — and a
//! directory queued for later can be replaced by a link before it is
//! listed. So the walk never opens a path beneath the root: every child
//! is opened with `openat` relative to its parent's descriptor, under
//! `O_NOFOLLOW`, and `fstat` on the opened descriptor confirms the kind
//! (`O_DIRECTORY` helps where the platform has it). `O_NONBLOCK` keeps a
//! fifo swapped in mid-walk from blocking the open. A directory's
//! descriptor is taken when the entry is found, so a path swapped
//! afterwards cannot redirect the listing, and a file is stat'd before
//! and after it is hashed so a write landing mid-read is a fault rather
//! than a digest of two halves.
//!
//! Links are never opened. A link's entry is its `readlinkat` target,
//! which is why a link pointing outside the root — into anything at all
//! — pulls nothing in.
//!
//! Files are hashed by a few worker threads while the walk lists
//! directories; each worker opens its file relative to the directory
//! descriptor the walk found it in, exactly as the walk would. A digest
//! is reused, without reading the file again, only for a file whose
//! device, inode, length, mode, modification time, and status-change time
//! all equal those of a file this process hashed, and only when both
//! times were at least [`SETTLED`] seconds old when it was hashed. A
//! write moves the status-change time, which no unprivileged process can
//! set back, so a reused digest is the digest of the same bytes; a file
//! written moments before an observation is always read again.
//!
//! A caller may keep those digests between processes ([`remember`] and
//! [`recall`]) in a file only it can write. A recalled digest is reused
//! under exactly the same rule: only for a file whose whole stamp still
//! matches the one it was hashed under, so a changed file is read again.

use std::collections::{BTreeMap, HashMap};
use std::ffi::{CStr, CString, OsStr, OsString};
use std::fs::File;
use std::io::Read as _;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

use super::{Entry, Fault, Id, Limits, Snapshot};

/// The most faults one walk itemizes. Past the cap the snapshot is
/// already unverifiable, and a hostile tree does not get to turn detail
/// into a memory problem.
const FAULT_MAX: usize = 64;

/// The longest link target the walk reads, so a malicious `readlink`
/// answer cannot grow without bound.
const LINK_MAX: usize = 64 * 1024;

/// How old a file's modification and status-change times must be, when
/// it is hashed, for its digest to be reused by a later observation.
const SETTLED: Duration = Duration::from_secs(2);

/// The most digests this process keeps for reuse; past it, the store is
/// emptied and refilled.
const REUSED_MAX: usize = 400_000;

/// The most file workers one walk runs.
const WORKERS_MAX: usize = 8;

/// The most files queued for the workers at once, so the directory
/// descriptors they hold stay few.
const QUEUED_MAX: usize = 64;

/// What `fstatat` says an entry is.
enum Kind {
    Directory,
    File,
    Link,
    Other(&'static str),
}

/// The walk in progress: what was observed, what went wrong, and the
/// bounds both are held to.
struct Walk {
    entries: BTreeMap<PathBuf, Entry>,
    faults: Vec<Fault>,
    /// Every listed entry counts against the entries bound — a failed
    /// one included — so a tree cannot exhaust the budget on successes
    /// alone or on failures alone.
    attempts: usize,
    limits: Limits,
}

impl Walk {
    /// Records a fault, itemized only until [`FAULT_MAX`]. A snapshot
    /// with more faults than that is still faulted; it just stops
    /// spending memory describing them.
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

/// The walk behind [`Snapshot::observe_within`].
pub(super) fn walk(root: &Path, limits: Limits) -> Snapshot {
    let mut walk = Walk {
        entries: BTreeMap::new(),
        faults: Vec::new(),
        attempts: 0,
        limits,
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
    // Open the root rather than trust the name: `O_NOFOLLOW` refuses a
    // root that is a link, and the `fstat` that follows refuses a root
    // that is not a directory.
    let root_fd = match open(&canonical, DIR_FLAGS) {
        Ok(fd) => fd,
        Err(error) => {
            walk.fault(Fault::Root {
                path: canonical,
                error: error.to_string(),
            });
            return finish(walk, root.to_path_buf());
        }
    };
    match root_fd.metadata() {
        Ok(metadata) if metadata.is_dir() => {
            walk.entries.insert(
                PathBuf::new(),
                Entry::Directory {
                    id: id(&metadata),
                    mode: mode(&metadata),
                },
            );
            walk.attempts = 1;
        }
        Ok(_) => {
            walk.fault(Fault::Root {
                path: canonical,
                error: "not a directory".to_string(),
            });
            return finish(walk, root.to_path_buf());
        }
        Err(error) => {
            walk.fault(Fault::Root {
                path: canonical,
                error: error.to_string(),
            });
            return finish(walk, root.to_path_buf());
        }
    }

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
        list(&mut walk, root_fd, &jobs, &hashed, &over);
        drop(jobs);
    });
    drop(done);
    // Files are recorded after the listing; their faults follow the
    // listing's, in path order, so the same tree faults the same way.
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

/// One file for a worker: the directory descriptor it was found in, its
/// name there, and its path under the root.
struct Job {
    dir: Arc<File>,
    name: OsString,
    path: PathBuf,
}

/// What a worker made of one file.
enum Hashed {
    File(Entry),
    Read(String),
    /// The byte bound was reached.
    Bytes,
}

/// List the tree from its root descriptor, recording directories, links,
/// and other entries, and queueing every file for the workers. It stops
/// at the entry bound, or once a worker reached the byte bound.
fn list(
    walk: &mut Walk,
    root_fd: File,
    jobs: &mpsc::SyncSender<Job>,
    hashed: &AtomicU64,
    over: &AtomicBool,
) {
    let limits = walk.limits;
    // Each pending directory holds its own descriptor, so listing it
    // reads the directory that was found, not whatever the path names by
    // the time the walk reaches it.
    let mut pending: Vec<(PathBuf, File)> = vec![(PathBuf::new(), root_fd)];
    'walk: while let Some((rel, dir)) = pending.pop() {
        if over.load(Ordering::Relaxed) {
            break;
        }
        let dir = Arc::new(dir);
        if walk.attempts > walk.limits.entries {
            walk.fault(Fault::Entries {
                limit: limits.entries,
            });
            break;
        }
        let names = match listing(&dir, walk.limits.entries - walk.attempts) {
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
        for name in names {
            walk.attempts += 1;
            if walk.attempts > walk.limits.entries {
                walk.fault(Fault::Entries {
                    limit: limits.entries,
                });
                break 'walk;
            }
            let path = rel.join(&name);
            let st = match stat_at(&dir, &name) {
                Ok(st) => st,
                Err(error) => {
                    walk.read(path, error);
                    continue;
                }
            };
            match kind_of(&st) {
                Kind::Directory => match open_at(&dir, &name, DIR_FLAGS) {
                    Ok(child) => match child.metadata() {
                        Ok(metadata) if metadata.is_dir() => {
                            walk.entries.insert(
                                path.clone(),
                                Entry::Directory {
                                    id: id(&metadata),
                                    mode: mode(&metadata),
                                },
                            );
                            pending.push((path, child));
                        }
                        // The entry named itself a directory and opened
                        // as something else — a mid-walk swap, which is
                        // a fault rather than a listing.
                        Ok(_) => walk.read(path, "changed while it was being opened"),
                        Err(error) => walk.read(path, error),
                    },
                    Err(error) => walk.read(path, error),
                },
                // A file whose digest this process already took, and that
                // nothing moved since, needs no worker.
                Kind::File if let Some(digest) = reused(&Stamp::at(&st)) => {
                    let length = u64::try_from(st.st_size).unwrap_or(u64::MAX);
                    if hashed.fetch_add(length, Ordering::Relaxed) + length > limits.bytes {
                        over.store(true, Ordering::Relaxed);
                        break 'walk;
                    }
                    walk.entries.insert(
                        path,
                        Entry::File {
                            id: id_of(&st),
                            length,
                            digest,
                            modified: modified_of(&st),
                            mode: mode_of(&st),
                        },
                    );
                }
                Kind::File => {
                    let job = Job {
                        dir: dir.clone(),
                        name,
                        path,
                    };
                    if jobs.send(job).is_err() || over.load(Ordering::Relaxed) {
                        break 'walk;
                    }
                }
                Kind::Link => match link_target(&dir, &name) {
                    Ok(target) => {
                        walk.entries.insert(
                            path,
                            Entry::Link {
                                id: id_of(&st),
                                target,
                                mode: mode_of(&st),
                            },
                        );
                    }
                    Err(error) => walk.read(path, error),
                },
                Kind::Other(kind) => {
                    walk.entries.insert(
                        path,
                        Entry::Other {
                            id: id_of(&st),
                            kind,
                            mode: mode_of(&st),
                        },
                    );
                }
            }
        }
    }
}

fn finish(walk: Walk, root: PathBuf) -> Snapshot {
    Snapshot {
        root,
        entries: walk.entries,
        faults: walk.faults,
    }
}

/// One file, hashed under the byte bound and checked for a write that
/// landed mid-read, or its earlier digest when nothing about it moved.
///
/// The entry comes from the open descriptor's `fstat`, not the walk's
/// `fstatat`: between the two the path may name something else, and the
/// descriptor pins the file that was actually hashed. A metadata change
/// across the read is a fault — the digest of a moving file is no
/// observation at all.
fn file(dir: &File, name: &OsStr, cap: u64, hashed: &AtomicU64) -> Hashed {
    let mut file = match open_at(dir, name, FILE_FLAGS) {
        Ok(file) => file,
        Err(error) => return Hashed::Read(error.to_string()),
    };
    let before = match file.metadata() {
        Ok(before) if before.is_file() => before,
        // The entry named itself a file and opened as something else.
        Ok(_) => return Hashed::Read("changed while it was being opened".into()),
        Err(error) => return Hashed::Read(error.to_string()),
    };
    let stamp = Stamp::of(&before);
    let entry = |digest| {
        Hashed::File(Entry::File {
            id: id(&before),
            length: before.len(),
            digest,
            modified: before.modified().ok(),
            mode: mode(&before),
        })
    };
    if let Some(digest) = reused(&stamp) {
        // A reused digest still counts its bytes: the bound says how big a
        // tree may be, not how much of it was read this time.
        if hashed.fetch_add(before.len(), Ordering::Relaxed) + before.len() > cap {
            return Hashed::Bytes;
        }
        return entry(digest);
    }
    let started = SystemTime::now();
    let digest = match hash(&mut file, cap, hashed) {
        Ok(Some(digest)) => digest,
        Ok(None) => return Hashed::Bytes,
        Err(error) => return Hashed::Read(error),
    };
    let after = match file.metadata() {
        Ok(after) => after,
        Err(error) => return Hashed::Read(error.to_string()),
    };
    if !steady(&before, &after) || Stamp::of(&after) != stamp {
        return Hashed::Read("changed while it was being read".into());
    }
    if stamp.settled(started) {
        keep(stamp, digest);
    }
    entry(digest)
}

/// Everything about a file that a write to it moves: its identity,
/// length, mode, and modification and status-change times.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Stamp {
    dev: u64,
    ino: u64,
    len: u64,
    mode: u32,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl Stamp {
    fn of(metadata: &std::fs::Metadata) -> Self {
        Stamp {
            dev: metadata.dev(),
            ino: metadata.ino(),
            len: metadata.size(),
            mode: metadata.mode(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }

    /// The stamp of an `fstatat` result, as [`Stamp::of`] reads an open
    /// descriptor's.
    #[allow(clippy::unnecessary_cast, clippy::useless_conversion)]
    fn at(st: &libc::stat) -> Self {
        Stamp {
            dev: st.st_dev as u64,
            ino: st.st_ino as u64,
            len: u64::try_from(st.st_size).unwrap_or(u64::MAX),
            mode: st.st_mode as u32,
            modified: (st.st_mtime as i64, st.st_mtime_nsec as i64),
            changed: (st.st_ctime as i64, st.st_ctime_nsec as i64),
        }
    }

    /// Whether both times were at least [`SETTLED`] old at `started`, so a
    /// write after the hash cannot leave them unchanged.
    fn settled(&self, started: SystemTime) -> bool {
        let Some(horizon) = started
            .checked_sub(SETTLED)
            .and_then(|at| at.duration_since(SystemTime::UNIX_EPOCH).ok())
        else {
            return false;
        };
        let horizon = (
            i64::try_from(horizon.as_secs()).unwrap_or(i64::MAX),
            i64::from(horizon.subsec_nanos()),
        );
        self.modified < horizon && self.changed < horizon
    }
}

/// Digests this process hashed, by the stamp of the file they were taken of.
fn reuse() -> &'static Mutex<HashMap<Stamp, [u8; 32]>> {
    static REUSE: OnceLock<Mutex<HashMap<Stamp, [u8; 32]>>> = OnceLock::new();
    REUSE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The first bytes of a digest file.
const DIGESTS_MAGIC: &[u8; 16] = b"oa-digests-v1\n\0\0";

/// Bytes of one kept digest: its stamp, then the SHA-256.
const DIGEST_RECORD: usize = 8 * 3 + 4 + 8 * 4 + 32;

/// Adds the digests `path` holds to this process's reuse, and returns how
/// many it read. A missing, unreadable, or malformed file adds none.
pub(super) fn recall(path: &Path) -> usize {
    let Ok(bytes) = std::fs::read(path) else {
        return 0;
    };
    let Some(records) = bytes.strip_prefix(DIGESTS_MAGIC.as_slice()) else {
        return 0;
    };
    if records.len() % DIGEST_RECORD != 0 || records.len() / DIGEST_RECORD > REUSED_MAX {
        return 0;
    }
    let Ok(mut map) = reuse().lock() else {
        return 0;
    };
    if map.len() + records.len() / DIGEST_RECORD > REUSED_MAX {
        map.clear();
    }
    let mut read = 0;
    for record in records.chunks_exact(DIGEST_RECORD) {
        let u64_at =
            |at: usize| u64::from_le_bytes(record[at..at + 8].try_into().unwrap_or_default());
        let i64_at =
            |at: usize| i64::from_le_bytes(record[at..at + 8].try_into().unwrap_or_default());
        let stamp = Stamp {
            dev: u64_at(0),
            ino: u64_at(8),
            len: u64_at(16),
            mode: u32::from_le_bytes(record[24..28].try_into().unwrap_or_default()),
            modified: (i64_at(28), i64_at(36)),
            changed: (i64_at(44), i64_at(52)),
        };
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&record[60..92]);
        map.insert(stamp, digest);
        read += 1;
    }
    read
}

/// Writes every digest this process may reuse to `path`, readable and
/// writable only by this user, replacing the file whole.
pub(super) fn remember(path: &Path) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut bytes = DIGESTS_MAGIC.to_vec();
    {
        let map = reuse()
            .lock()
            .map_err(|_| std::io::Error::other("the digest store is poisoned"))?;
        bytes.reserve(map.len() * DIGEST_RECORD);
        for (stamp, digest) in map.iter() {
            bytes.extend_from_slice(&stamp.dev.to_le_bytes());
            bytes.extend_from_slice(&stamp.ino.to_le_bytes());
            bytes.extend_from_slice(&stamp.len.to_le_bytes());
            bytes.extend_from_slice(&stamp.mode.to_le_bytes());
            for time in [stamp.modified, stamp.changed] {
                bytes.extend_from_slice(&time.0.to_le_bytes());
                bytes.extend_from_slice(&time.1.to_le_bytes());
            }
            bytes.extend_from_slice(digest);
        }
    }
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(&bytes)?;
    drop(file);
    std::fs::rename(&temporary, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temporary);
    })
}

fn reused(stamp: &Stamp) -> Option<[u8; 32]> {
    reuse().lock().ok()?.get(stamp).copied()
}

fn keep(stamp: Stamp, digest: [u8; 32]) {
    if let Ok(mut map) = reuse().lock() {
        if map.len() >= REUSED_MAX {
            map.clear();
        }
        map.insert(stamp, digest);
    }
}

/// Whether the file read is the file hashed: same length, same
/// modification time, same mode. The descriptor pins the inode, so those
/// are the parts a writer could still move.
fn steady(before: &std::fs::Metadata, after: &std::fs::Metadata) -> bool {
    before.len() == after.len()
        && before.modified().ok() == after.modified().ok()
        && mode(before) == mode(after)
}

/// The SHA-256 of the open file's contents, charged against the walk's
/// byte bound. `Ok(None)` is the bound reached.
fn hash(file: &mut File, cap: u64, hashed: &AtomicU64) -> Result<Option<[u8; 32]>, String> {
    let mut sha = Sha256::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            return Ok(Some(sha.finalize().into()));
        }
        if hashed.fetch_add(read as u64, Ordering::Relaxed) + read as u64 > cap {
            return Ok(None);
        }
        sha.update(&chunk[..read]);
    }
}

/// The names one directory holds, listed through a duplicated descriptor
/// — `fdopendir` owns what it is handed, and the walk keeps its
/// descriptor for the children's `openat`.
fn listing(dir: &File, limit: usize) -> std::io::Result<(Vec<OsString>, bool)> {
    let dup = unsafe { libc::fcntl(dir.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if dup < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let dp = unsafe { libc::fdopendir(dup) };
    if dp.is_null() {
        let error = std::io::Error::last_os_error();
        unsafe { libc::close(dup) };
        return Err(error);
    }
    let mut names = Vec::new();
    let result = loop {
        errno_set(0);
        let entry = unsafe { libc::readdir(dp) };
        if entry.is_null() {
            let error = std::io::Error::last_os_error();
            break match error.raw_os_error() {
                // A zero errno at the end of the listing is the end of
                // the listing.
                Some(0) | None => Ok((names, false)),
                _ => Err(error),
            };
        }
        // `readdir` points into the DIR's own buffer; the name there is
        // NUL-terminated.
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        let bytes = name.to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        if names.len() == limit {
            break Ok((names, true));
        }
        names.push(OsStr::from_bytes(bytes).to_os_string());
    };
    unsafe { libc::closedir(dp) };
    result
}

/// An entry's metadata without following a link: `fstatat` under
/// `AT_SYMLINK_NOFOLLOW`.
fn stat_at(dir: &File, name: &OsStr) -> std::io::Result<libc::stat> {
    let name = cstring(name)?;
    let mut st = unsafe { std::mem::zeroed::<libc::stat>() };
    if unsafe {
        libc::fstatat(
            dir.as_raw_fd(),
            name.as_ptr(),
            &mut st,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(st)
    }
}

/// A link's target, by `readlinkat` — the link itself, never what it
/// names.
fn link_target(dir: &File, name: &OsStr) -> std::io::Result<OsString> {
    let name = cstring(name)?;
    let mut buf = vec![0u8; 1024];
    loop {
        let read = unsafe {
            libc::readlinkat(
                dir.as_raw_fd(),
                name.as_ptr(),
                buf.as_mut_ptr().cast(),
                buf.len(),
            )
        };
        if read < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let read = read as usize;
        if read < buf.len() {
            return Ok(OsStr::from_bytes(&buf[..read]).to_os_string());
        }
        if buf.len() >= LINK_MAX {
            return Err(std::io::Error::other(
                "a link target exceeded the read bound",
            ));
        }
        buf.resize((buf.len() * 2).min(LINK_MAX), 0);
    }
}

/// Opens `path` itself — used for the root, where there is no parent
/// descriptor yet.
fn open(path: &Path, flags: libc::c_int) -> std::io::Result<File> {
    use std::path::Component;
    if !path.is_absolute() {
        return Err(std::io::Error::other("snapshot root must be absolute"));
    }
    let fd = unsafe { libc::open(c"/".as_ptr(), DIR_FLAGS | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: this descriptor is freshly opened and transferred once.
    let mut dir = unsafe { File::from_raw_fd(fd) };
    let mut components = path.components().peekable();
    while let Some(component) = components.next() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                let child_flags = if components.peek().is_some() {
                    DIR_FLAGS
                } else {
                    flags
                };
                dir = open_at(&dir, name, child_flags)?;
            }
            _ => return Err(std::io::Error::other("snapshot root is not canonical")),
        }
    }
    Ok(dir)
}

/// Opens `name` relative to its parent's descriptor, so a path that was
/// swapped mid-walk cannot redirect the open — `O_NOFOLLOW` refuses a
/// link where one was raced into place.
fn open_at(dir: &File, name: &OsStr, flags: libc::c_int) -> std::io::Result<File> {
    let name = cstring(name)?;
    let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags | libc::O_CLOEXEC) };
    if fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

/// The flags a directory opens under. `O_DIRECTORY` does not exist on
/// every Unix — macOS lacks it — so where it is missing the `fstat`
/// after the open checks the kind instead, and `O_NONBLOCK` keeps a fifo
/// swapped in mid-walk from blocking the open.
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
const DIR_FLAGS: libc::c_int =
    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_NONBLOCK;

/// The flags a directory opens under, where `O_DIRECTORY` does not
/// exist.
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
)))]
const DIR_FLAGS: libc::c_int = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK;

/// The flags a file opens under: no link, and `O_NONBLOCK` so a fifo
/// swapped in mid-walk fails the kind check instead of blocking the
/// open.
const FILE_FLAGS: libc::c_int = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK;

/// A name as a C string for the `*at` calls. A NUL inside a name cannot
/// appear in a real directory listing; one that does is an error.
fn cstring(name: &OsStr) -> std::io::Result<CString> {
    CString::new(name.as_bytes())
        .map_err(|_| std::io::Error::other("a name containing NUL cannot be opened"))
}

/// What an entry is, from its `fstatat` mode.
fn kind_of(st: &libc::stat) -> Kind {
    let format = (st.st_mode as u64) & (libc::S_IFMT as u64);
    if format == libc::S_IFDIR as u64 {
        Kind::Directory
    } else if format == libc::S_IFREG as u64 {
        Kind::File
    } else if format == libc::S_IFLNK as u64 {
        Kind::Link
    } else if format == libc::S_IFIFO as u64 {
        Kind::Other("fifo")
    } else if format == libc::S_IFSOCK as u64 {
        Kind::Other("socket")
    } else if format == libc::S_IFCHR as u64 {
        Kind::Other("char device")
    } else if format == libc::S_IFBLK as u64 {
        Kind::Other("block device")
    } else {
        Kind::Other("other")
    }
}

/// Device and inode from an `fstatat` result, for rename pairing.
///
/// `st_dev` is `i32` on macOS and `u64` on Linux, so the cast widens on one
/// platform and is an identity on the other.
#[allow(clippy::unnecessary_cast)]
fn id_of(st: &libc::stat) -> Option<Id> {
    Some((st.st_dev as u64, st.st_ino))
}

/// The modification time of an `fstatat` result, as `Metadata::modified`
/// reads it from a descriptor.
#[allow(clippy::unnecessary_cast)]
fn modified_of(st: &libc::stat) -> Option<SystemTime> {
    let seconds = st.st_mtime as i64;
    let nanos = u32::try_from(st.st_mtime_nsec as i64).ok()?;
    if seconds >= 0 {
        SystemTime::UNIX_EPOCH.checked_add(Duration::new(seconds as u64, nanos))
    } else {
        SystemTime::UNIX_EPOCH
            .checked_sub(Duration::from_secs(seconds.unsigned_abs()))?
            .checked_add(Duration::from_nanos(u64::from(nanos)))
    }
}

/// Permission bits from an `fstatat` result.
///
/// `st_mode` is `u16` on macOS and `u32` on Linux.
#[allow(clippy::unnecessary_cast)]
fn mode_of(st: &libc::stat) -> Option<u32> {
    Some(st.st_mode as u32 & 0o7777)
}

/// Device and inode from an open descriptor's metadata.
fn id(metadata: &std::fs::Metadata) -> Option<Id> {
    use std::os::unix::fs::MetadataExt as _;
    Some((metadata.dev(), metadata.ino()))
}

/// Permission bits from an open descriptor's metadata.
fn mode(metadata: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::MetadataExt as _;
    Some(metadata.mode() & 0o7777)
}

/// Clears the calling thread's errno before reading a directory entry.
fn errno_set(value: libc::c_int) {
    errno::set_errno(errno::Errno(value));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_collection_stops_at_its_budget() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..32 {
            std::fs::write(dir.path().join(i.to_string()), b"x").unwrap();
        }
        let fd = open(&dir.path().canonicalize().unwrap(), DIR_FLAGS).unwrap();
        let (names, exceeded) = listing(&fd, 3).unwrap();
        assert_eq!(names.len(), 3);
        assert!(exceeded);
    }

    #[test]
    fn opening_a_root_rejects_symlinked_parent_components() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("real")).unwrap();
        std::fs::create_dir(dir.path().join("real/child")).unwrap();
        std::os::unix::fs::symlink("real", dir.path().join("alias")).unwrap();
        let path = dir.path().canonicalize().unwrap().join("alias/child");
        assert!(open(&path, DIR_FLAGS).is_err());
    }

    #[test]
    fn a_file_replaced_by_a_fifo_refuses_without_waiting_for_a_writer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        let name = cstring(path.join("fifo").as_os_str()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        let fd = open(&path, DIR_FLAGS).unwrap();
        let hashed = AtomicU64::new(0);
        assert!(matches!(
            file(&fd, OsStr::new("fifo"), u64::MAX, &hashed),
            Hashed::Read(_)
        ));
    }

    /// A digest is reused only while nothing about the file moved: a
    /// rewrite of the same length with its modification time put back
    /// still moves its status-change time, so it is read again.
    #[test]
    fn a_rewrite_that_restores_the_modification_time_is_read_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        let target = path.join("file");
        std::fs::write(&target, b"first").unwrap();
        let old = SystemTime::now() - Duration::from_secs(3600);
        File::options()
            .write(true)
            .open(&target)
            .unwrap()
            .set_modified(old)
            .unwrap();
        // Past the settling time, the digest is kept for reuse.
        std::thread::sleep(SETTLED + Duration::from_millis(200));
        let first = crate::Snapshot::observe(&path);
        let again = crate::Snapshot::observe(&path);
        assert!(first.is_complete() && again.is_complete());
        assert_eq!(first.digest(), again.digest());
        std::fs::write(&target, b"other").unwrap();
        File::options()
            .write(true)
            .open(&target)
            .unwrap()
            .set_modified(old)
            .unwrap();
        let rewritten = crate::Snapshot::observe(&path);
        assert!(rewritten.is_complete());
        assert_ne!(first.digest(), rewritten.digest());
        assert!(rewritten.matches_file(Path::new("file"), b"other"));
    }

    /// Digests kept in a file are reused by a later process only while the
    /// file's whole stamp is unchanged: a forged digest for an unchanged
    /// file is what the snapshot then reports, which shows it was reused,
    /// and once the file changes it is read again.
    #[test]
    fn remembered_digests_are_reused_only_for_an_unchanged_file() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        let tree = path.join("tree");
        std::fs::create_dir(&tree).unwrap();
        let target = tree.join("file");
        std::fs::write(&target, b"bytes").unwrap();
        let old = SystemTime::now() - Duration::from_secs(3600);
        File::options()
            .write(true)
            .open(&target)
            .unwrap()
            .set_modified(old)
            .unwrap();
        std::thread::sleep(SETTLED + Duration::from_millis(200));
        assert!(crate::Snapshot::observe(&tree).matches_file(Path::new("file"), b"bytes"));
        let kept = path.join("digests");
        remember(&kept).unwrap();
        assert_eq!(
            std::fs::metadata(&kept).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // Forge the file's digest, as a later process would read it.
        let ino = std::fs::metadata(&target).unwrap().ino();
        let mut bytes = std::fs::read(&kept).unwrap();
        let forged: [u8; 32] = Sha256::digest(b"fake!").into();
        let mut found = false;
        for record in bytes[DIGESTS_MAGIC.len()..].chunks_exact_mut(DIGEST_RECORD) {
            if u64::from_le_bytes(record[8..16].try_into().unwrap()) == ino {
                record[60..92].copy_from_slice(&forged);
                found = true;
            }
        }
        assert!(found);
        std::fs::write(&kept, &bytes).unwrap();
        reuse().lock().unwrap().clear();
        assert!(recall(&kept) >= 1);
        assert!(crate::Snapshot::observe(&tree).matches_file(Path::new("file"), b"fake!"));
        // A change moves the stamp, so the file is read again.
        std::fs::write(&target, b"other").unwrap();
        assert!(crate::Snapshot::observe(&tree).matches_file(Path::new("file"), b"other"));
        // A missing or malformed file adds nothing.
        assert_eq!(recall(&path.join("missing")), 0);
        std::fs::write(&kept, b"oa-digests-v1\n\0\0short").unwrap();
        assert_eq!(recall(&kept), 0);
        std::fs::write(&kept, b"not a digest file").unwrap();
        assert_eq!(recall(&kept), 0);
    }

    #[test]
    fn a_fresh_file_is_never_kept_for_reuse() {
        let now = SystemTime::now();
        let since_epoch = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();
        let fresh = (since_epoch.as_secs() as i64, 0);
        let stamp = Stamp {
            dev: 1,
            ino: 2,
            len: 3,
            mode: 0o644,
            modified: (0, 0),
            changed: fresh,
        };
        assert!(!stamp.settled(now));
        let old = Stamp {
            changed: (0, 0),
            ..stamp
        };
        assert!(old.settled(now));
    }
}
