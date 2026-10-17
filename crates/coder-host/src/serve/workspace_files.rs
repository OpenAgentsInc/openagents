//! File operations anchored to an explicitly admitted workspace directory.
//!
//! The caller must check current grants before each call. This module grants
//! no authority and is not an enrollment or command-execution adapter.

use std::ffi::{CString, OsStr, OsString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use base64::Engine as _;
use coder_access::Code;
use coder_access::computer::{Answer, CHUNK_BYTES, FileInfo, FilePut, MAX_FILE_BYTES, Request};
use sha2::{Digest, Sha256};

static WRITES: Mutex<()> = Mutex::new(());

/// A pinned workspace root. Path traversal uses directory descriptors and
/// refuses symbolic links, hard-linked files, and special files.
pub struct WorkspaceFiles {
    root: PathBuf,
    directory: File,
}

impl WorkspaceFiles {
    /// Open the host-configured root, independently of any client path.
    ///
    /// # Errors
    /// Refuses an unavailable root or one that changes to a link while opening.
    pub fn open(root: &Path) -> Result<Self, Code> {
        let root = root.canonicalize().map_err(io_code)?;
        let mut directory = File::open("/").map_err(io_code)?;
        for part in root.components() {
            if let Component::Normal(name) = part {
                directory = open_at(&directory, name, libc::O_RDONLY | libc::O_DIRECTORY)
                    .map_err(io_code)?;
            }
        }
        Ok(Self { root, directory })
    }

    /// The root's resolved spelling, used in absolute file requests.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Identity of the opened directory, independent of its current pathname.
    pub fn identity(&self) -> Result<String, Code> {
        let metadata = self.directory.metadata().map_err(io_code)?;
        Ok(coder_access::enrolled::hash(
            &[metadata.dev().to_be_bytes(), metadata.ino().to_be_bytes()].concat(),
        ))
    }

    /// Answer a bounded file request within this root. Screen capture and
    /// window listing are outside a workspace and always refuse here.
    ///
    /// # Errors
    /// Refuses path escapes, links, special files, invalid chunks, and conflicts.
    pub fn answer(&self, request: &Request) -> Result<Answer, Code> {
        request.validate().map_err(|e| e.code)?;
        match request {
            Request::Stat { path } => {
                let (parent, name) = self.parent(path)?;
                let mut file = regular(&parent, &name, libc::O_RDONLY)?;
                let (size, digest) = digest(&mut file)?;
                Ok(Answer::File {
                    file: FileInfo {
                        path: path.clone(),
                        size,
                        digest,
                        media_type: None,
                    },
                })
            }
            Request::Read {
                path,
                offset,
                length,
            } => {
                let (parent, name) = self.parent(path)?;
                let mut file = regular(&parent, &name, libc::O_RDONLY)?;
                file.seek(SeekFrom::Start(*offset)).map_err(io_code)?;
                let mut bytes = Vec::with_capacity(*length as usize);
                file.take(*length)
                    .read_to_end(&mut bytes)
                    .map_err(io_code)?;
                Ok(Answer::Chunk {
                    offset: *offset,
                    data: base64::engine::general_purpose::STANDARD.encode(bytes),
                })
            }
            Request::Write { put } => self.write(put),
            Request::Screenshot { .. } | Request::Apps {} => Err(Code::Forbidden),
        }
    }

    fn parent(&self, path: &str) -> Result<(File, OsString), Code> {
        let path = Path::new(path);
        // Reject traversal before strip_prefix: normalization must not hide it.
        if path.components().any(|p| matches!(p, Component::ParentDir)) {
            return Err(Code::Forbidden);
        }
        let relative = path.strip_prefix(&self.root).map_err(|_| Code::Forbidden)?;
        let mut parts = relative.components().peekable();
        let mut directory = self.directory.try_clone().map_err(io_code)?;
        while let Some(part) = parts.next() {
            let Component::Normal(name) = part else {
                return Err(Code::Forbidden);
            };
            if parts.peek().is_none() {
                return Ok((directory, name.to_owned()));
            }
            directory =
                open_at(&directory, name, libc::O_RDONLY | libc::O_DIRECTORY).map_err(io_code)?;
        }
        Err(Code::Forbidden)
    }

    fn write(&self, put: &FilePut) -> Result<Answer, Code> {
        let bytes = put.bytes().map_err(|e| e.code)?;
        let (parent, name) = self.parent(&put.path)?;
        let _writing = WRITES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // A lost completion reply is safe to repeat without replacing anything.
        match open_at(&parent, &name, libc::O_RDONLY) {
            Ok(mut target) => {
                check_regular(&target)?;
                if digest(&mut target)? == (put.size, put.digest.clone()) {
                    return Ok(Answer::Written {
                        received: put.size,
                        complete: true,
                    });
                }
                if !put.overwrite {
                    return Err(Code::Conflict);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_code(e)),
        }
        let hex = coder_access::computer::digest_hex(&put.digest).map_err(|e| e.code)?;
        let partial = OsString::from(format!(
            ".{}.{}.oa-partial",
            name.to_string_lossy(),
            &hex[..16]
        ));
        let chunks = OsString::from(format!(
            ".{}.{}.oa-chunks",
            name.to_string_lossy(),
            &hex[..16]
        ));
        let mut file = regular(&parent, &partial, libc::O_RDWR | libc::O_CREAT)?;
        let mut record_file = regular(&parent, &chunks, libc::O_RDWR | libc::O_CREAT)?;
        let count = put.size.div_ceil(CHUNK_BYTES).max(1) as usize;
        let mut held = Vec::new();
        (&mut record_file)
            .take(count as u64 + 1)
            .read_to_end(&mut held)
            .map_err(io_code)?;
        if held.is_empty() {
            held.resize(count, 0);
        } else if held.len() != count || held.iter().any(|b| *b > 1) {
            return Err(Code::Conflict);
        }
        let index = (put.offset / CHUNK_BYTES) as usize;
        let slot = held.get_mut(index).ok_or(Code::Malformed)?;
        if *slot == 0 {
            file.seek(SeekFrom::Start(put.offset)).map_err(io_code)?;
            file.write_all(&bytes).map_err(io_code)?;
            file.sync_all().map_err(io_code)?;
            *slot = 1;
            record_file.seek(SeekFrom::Start(0)).map_err(io_code)?;
            record_file.write_all(&held).map_err(io_code)?;
            record_file.sync_all().map_err(io_code)?;
        }
        if held.iter().any(|b| *b == 0) {
            let contiguous = held.iter().take_while(|b| **b == 1).count() as u64;
            return Ok(Answer::Written {
                received: (contiguous * CHUNK_BYTES).min(put.size),
                complete: false,
            });
        }
        file.seek(SeekFrom::Start(0)).map_err(io_code)?;
        if digest(&mut file)? != (put.size, put.digest.clone()) {
            unlink(&parent, &partial)?;
            unlink(&parent, &chunks)?;
            return Err(Code::Conflict);
        }
        let from = cstring(&partial)?;
        let to = cstring(&name)?;
        // SAFETY: both names are NUL-terminated, single components, and the
        // directory descriptor stays open across the atomic operation.
        let result = unsafe {
            if put.overwrite {
                libc::renameat(
                    parent.as_raw_fd(),
                    from.as_ptr(),
                    parent.as_raw_fd(),
                    to.as_ptr(),
                )
            } else {
                libc::linkat(
                    parent.as_raw_fd(),
                    from.as_ptr(),
                    parent.as_raw_fd(),
                    to.as_ptr(),
                    0,
                )
            }
        };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            return Err(if error.kind() == std::io::ErrorKind::AlreadyExists {
                Code::Conflict
            } else {
                io_code(error)
            });
        }
        if !put.overwrite {
            unlink(&parent, &partial)?;
        }
        unlink(&parent, &chunks)?;
        parent.sync_all().map_err(io_code)?;
        Ok(Answer::Written {
            received: put.size,
            complete: true,
        })
    }
}

fn cstring(name: &OsStr) -> Result<CString, Code> {
    CString::new(name.as_bytes()).map_err(|_| Code::Malformed)
}

fn open_at(parent: &File, name: &OsStr, flags: i32) -> std::io::Result<File> {
    let name = CString::new(name.as_bytes()).map_err(std::io::Error::other)?;
    // SAFETY: the directory is open, the name is NUL-terminated, and the
    // successful descriptor transfers exactly once to File.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: openat returned a new owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn regular(parent: &File, name: &OsStr, flags: i32) -> Result<File, Code> {
    let file = open_at(parent, name, flags).map_err(io_code)?;
    check_regular(&file)?;
    Ok(file)
}

fn check_regular(file: &File) -> Result<(), Code> {
    let metadata = file.metadata().map_err(io_code)?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(Code::Forbidden);
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(Code::Bounds);
    }
    Ok(())
}

fn digest(file: &mut File) -> Result<(u64, String), Code> {
    let before = file.metadata().map_err(io_code)?;
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer).map_err(io_code)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > MAX_FILE_BYTES {
            return Err(Code::Bounds);
        }
        hash.update(&buffer[..n]);
    }
    let after = file.metadata().map_err(io_code)?;
    if size != before.len()
        || after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
        || after.ctime() != before.ctime()
        || after.ctime_nsec() != before.ctime_nsec()
        || after.nlink() != 1
    {
        return Err(Code::Conflict);
    }
    let hex: String = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok((size, format!("sha256:{hex}")))
}

fn unlink(parent: &File, name: &OsStr) -> Result<(), Code> {
    let name = cstring(name)?;
    // SAFETY: the directory is open and name is a NUL-terminated component.
    if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) } == 0 {
        Ok(())
    } else {
        Err(io_code(std::io::Error::last_os_error()))
    }
}

fn io_code(error: std::io::Error) -> Code {
    match error.raw_os_error() {
        Some(libc::ELOOP | libc::ENOTDIR) => Code::Forbidden,
        _ => Code::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn path(root: &WorkspaceFiles, name: &str) -> String {
        root.root().join(name).to_str().unwrap().into()
    }

    fn put(
        root: &WorkspaceFiles,
        name: &str,
        bytes: &[u8],
        offset: u64,
        overwrite: bool,
    ) -> Request {
        let start = offset as usize;
        let end = (start + CHUNK_BYTES as usize).min(bytes.len());
        Request::Write {
            put: FilePut {
                path: path(root, name),
                size: bytes.len() as u64,
                digest: coder_access::computer::digest(bytes),
                offset,
                data: base64::engine::general_purpose::STANDARD.encode(&bytes[start..end]),
                overwrite,
            },
        }
    }

    #[test]
    fn bounded_file_round_trip_and_chunk_retry() {
        let temp = tempfile::tempdir().unwrap();
        let files = WorkspaceFiles::open(temp.path()).unwrap();
        let bytes = vec![b'x'; CHUNK_BYTES as usize + 19];
        let last = put(&files, "result", &bytes, CHUNK_BYTES, false);
        assert_eq!(
            files.answer(&last).unwrap(),
            Answer::Written {
                received: 0,
                complete: false
            }
        );
        assert!(!temp.path().join("result").exists());
        let first = put(&files, "result", &bytes, 0, false);
        assert_eq!(
            files.answer(&first).unwrap(),
            Answer::Written {
                received: bytes.len() as u64,
                complete: true
            }
        );
        assert_eq!(
            files.answer(&last).unwrap(),
            Answer::Written {
                received: bytes.len() as u64,
                complete: true
            }
        );
        assert_eq!(std::fs::read(temp.path().join("result")).unwrap(), bytes);
        let stat = files
            .answer(&Request::Stat {
                path: path(&files, "result"),
            })
            .unwrap();
        assert!(
            matches!(stat, Answer::File { file } if file.size == bytes.len() as u64 && file.digest == coder_access::computer::digest(&bytes))
        );
        let read = files
            .answer(&Request::Read {
                path: path(&files, "result"),
                offset: CHUNK_BYTES,
                length: 19,
            })
            .unwrap();
        assert!(
            matches!(read, Answer::Chunk { data, .. } if base64::engine::general_purpose::STANDARD.decode(&data).unwrap() == b"xxxxxxxxxxxxxxxxxxx")
        );
        assert_eq!(
            files
                .answer(&put(&files, "result", b"different", 0, false))
                .unwrap_err(),
            Code::Conflict
        );
        files
            .answer(&put(&files, "result", b"different", 0, true))
            .unwrap();
        assert_eq!(
            std::fs::read(temp.path().join("result")).unwrap(),
            b"different"
        );
    }

    #[test]
    fn path_escapes_links_and_screen_operations_refuse() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("workspace");
        let outside = temp.path().join("outside");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("secret"), b"secret").unwrap();
        let files = WorkspaceFiles::open(&root).unwrap();
        symlink(&outside, root.join("escape")).unwrap();
        symlink(outside.join("secret"), root.join("link")).unwrap();
        std::fs::hard_link(outside.join("secret"), root.join("hard-link")).unwrap();
        for name in ["../outside/secret", "escape/secret", "link", "hard-link"] {
            assert_eq!(
                files
                    .answer(&Request::Stat {
                        path: path(&files, name)
                    })
                    .unwrap_err(),
                Code::Forbidden,
                "{name}"
            );
            assert_eq!(
                files
                    .answer(&put(&files, name, b"changed", 0, true))
                    .unwrap_err(),
                Code::Forbidden,
                "{name}"
            );
        }
        assert_eq!(
            files
                .answer(&Request::Stat {
                    path: outside.join("secret").display().to_string()
                })
                .unwrap_err(),
            Code::Forbidden
        );
        assert_eq!(
            files.answer(&Request::Apps {}).unwrap_err(),
            Code::Forbidden
        );
        assert_eq!(std::fs::read(outside.join("secret")).unwrap(), b"secret");
    }

    #[test]
    fn partial_and_chunk_record_links_never_write_outside_the_workspace() {
        for suffix in ["oa-partial", "oa-chunks"] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("workspace");
            std::fs::create_dir(&root).unwrap();
            let outside = temp.path().join("secret");
            std::fs::write(&outside, b"secret").unwrap();
            let files = WorkspaceFiles::open(&root).unwrap();
            let request = put(&files, "result", b"new", 0, false);
            let digest = coder_access::computer::digest(b"new");
            let hex = coder_access::computer::digest_hex(&digest).unwrap();
            let trap = root.join(format!(".result.{}.{}", &hex[..16], suffix));
            symlink(&outside, trap).unwrap();
            assert_eq!(files.answer(&request).unwrap_err(), Code::Forbidden);
            assert_eq!(std::fs::read(&outside).unwrap(), b"secret");
            assert!(!root.join("result").exists());
        }
    }

    #[test]
    fn replacing_the_root_path_does_not_redirect_an_open_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("workspace");
        let moved = temp.path().join("moved");
        let outside = temp.path().join("outside");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(root.join("data"), b"allowed").unwrap();
        std::fs::write(outside.join("data"), b"secret").unwrap();
        let files = WorkspaceFiles::open(&root).unwrap();
        std::fs::rename(&root, &moved).unwrap();
        symlink(&outside, &root).unwrap();
        let result = files
            .answer(&Request::Read {
                path: path(&files, "data"),
                offset: 0,
                length: 7,
            })
            .unwrap();
        assert!(
            matches!(result, Answer::Chunk { data, .. } if base64::engine::general_purpose::STANDARD.decode(&data).unwrap() == b"allowed")
        );
        files
            .answer(&put(&files, "result", b"safe", 0, false))
            .unwrap();
        drop(files);
        assert_eq!(std::fs::read(moved.join("result")).unwrap(), b"safe");
        assert!(!outside.join("result").exists());
        assert_eq!(std::fs::read(outside.join("data")).unwrap(), b"secret");
    }

    #[test]
    fn special_files_do_not_block_reads() {
        let temp = tempfile::tempdir().unwrap();
        let files = WorkspaceFiles::open(temp.path()).unwrap();
        let fifo = files.root().join("fifo");
        let name = cstring(fifo.as_os_str()).unwrap();
        // SAFETY: name is a valid NUL-terminated path in the test directory.
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        assert_eq!(
            files
                .answer(&Request::Read {
                    path: path(&files, "fifo"),
                    offset: 0,
                    length: 1
                })
                .unwrap_err(),
            Code::Forbidden
        );
    }
}
