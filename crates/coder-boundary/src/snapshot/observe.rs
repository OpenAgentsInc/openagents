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

use std::collections::BTreeMap;
use std::ffi::{CStr, CString, OsStr, OsString};
use std::fs::File;
use std::io::Read as _;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{Entry, Fault, Id, Limits, Snapshot};

/// The most faults one walk itemizes. Past the cap the snapshot is
/// already unverifiable, and a hostile tree does not get to turn detail
/// into a memory problem.
const FAULT_MAX: usize = 64;

/// The longest link target the walk reads, so a malicious `readlink`
/// answer cannot grow without bound.
const LINK_MAX: usize = 64 * 1024;

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
    hashed: u64,
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
        hashed: 0,
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

    // Each pending directory holds its own descriptor, so listing it
    // reads the directory that was found, not whatever the path names by
    // the time the walk reaches it.
    let mut pending: Vec<(PathBuf, File)> = vec![(PathBuf::new(), root_fd)];
    'walk: while let Some((rel, dir)) = pending.pop() {
        if walk.attempts > walk.limits.entries {
            walk.fault(Fault::Entries {
                limit: limits.entries,
            });
            break;
        }
        let names = match list(&dir, walk.limits.entries - walk.attempts) {
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
                Kind::File => match file(&mut walk, &dir, &name, &path) {
                    Some(entry) => {
                        walk.entries.insert(path, entry);
                    }
                    None => {
                        if walk.hashed > walk.limits.bytes {
                            break 'walk;
                        }
                    }
                },
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
    finish(walk, canonical)
}

fn finish(walk: Walk, root: PathBuf) -> Snapshot {
    Snapshot {
        root,
        entries: walk.entries,
        faults: walk.faults,
    }
}

/// One file, hashed under the byte bound and checked for a write that
/// landed mid-read.
///
/// The entry comes from the open descriptor's `fstat`, not the earlier
/// `fstatat`: between the two the path may name something else, and the
/// descriptor pins the file that was actually hashed. A metadata change
/// across the read is a fault — the digest of a moving file is no
/// observation at all.
fn file(walk: &mut Walk, dir: &File, name: &OsStr, path: &Path) -> Option<Entry> {
    let mut file = match open_at(dir, name, FILE_FLAGS) {
        Ok(file) => file,
        Err(error) => {
            walk.read(path.to_path_buf(), error);
            return None;
        }
    };
    let before = match file.metadata() {
        Ok(before) if before.is_file() => before,
        // The entry named itself a file and opened as something else.
        Ok(_) => {
            walk.read(path.to_path_buf(), "changed while it was being opened");
            return None;
        }
        Err(error) => {
            walk.read(path.to_path_buf(), error);
            return None;
        }
    };
    let digest = match hash(&mut file, walk.limits.bytes, &mut walk.hashed) {
        Ok(Some(digest)) => digest,
        Ok(None) => {
            walk.fault(Fault::Bytes {
                limit: walk.limits.bytes,
            });
            return None;
        }
        Err(error) => {
            walk.read(path.to_path_buf(), error);
            return None;
        }
    };
    let after = match file.metadata() {
        Ok(after) => after,
        Err(error) => {
            walk.read(path.to_path_buf(), error);
            return None;
        }
    };
    if !steady(&before, &after) {
        walk.read(path.to_path_buf(), "changed while it was being read");
        return None;
    }
    Some(Entry::File {
        id: id(&before),
        length: before.len(),
        digest,
        modified: before.modified().ok(),
        mode: mode(&before),
    })
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
fn hash(file: &mut File, cap: u64, hashed: &mut u64) -> Result<Option<[u8; 32]>, String> {
    let mut sha = Sha256::new();
    let mut chunk = [0u8; 8 * 1024];
    loop {
        let read = file.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            return Ok(Some(sha.finalize().into()));
        }
        *hashed += read as u64;
        if *hashed > cap {
            return Ok(None);
        }
        sha.update(&chunk[..read]);
    }
}

/// The names one directory holds, listed through a duplicated descriptor
/// — `fdopendir` owns what it is handed, and the walk keeps its
/// descriptor for the children's `openat`.
fn list(dir: &File, limit: usize) -> std::io::Result<(Vec<OsString>, bool)> {
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
fn id_of(st: &libc::stat) -> Option<Id> {
    Some((st.st_dev as u64, st.st_ino))
}

/// Permission bits from an `fstatat` result.
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
        let (names, exceeded) = list(&fd, 3).unwrap();
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
        let mut walk = Walk {
            entries: BTreeMap::new(),
            faults: Vec::new(),
            hashed: 0,
            attempts: 0,
            limits: Limits::default(),
        };
        assert!(file(&mut walk, &fd, OsStr::new("fifo"), Path::new("fifo")).is_none());
        assert!(!walk.faults.is_empty());
    }
}
