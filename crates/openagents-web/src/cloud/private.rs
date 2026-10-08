//! Explicit private files remain pinned to their original path and bytes.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::path::{Component, Path, PathBuf};

const UNAVAILABLE: &str = "Private Cloud configuration is unavailable or changed.";

/// A file opened without following any path component's symbolic links.
pub(super) struct ProtectedFile {
    path: PathBuf,
    file: File,
    maximum: usize,
    digest: [u8; 32],
}

impl ProtectedFile {
    pub(super) fn open(path: &Path, maximum: usize) -> Result<(Self, Vec<u8>), String> {
        let file = open(path, maximum)?;
        let bytes = read(&file, maximum)?;
        let digest = Sha256::digest(&bytes).into();
        Ok((
            Self {
                path: path.into(),
                file,
                maximum,
                digest,
            },
            bytes,
        ))
    }

    pub(super) fn check(&self) -> Result<(), String> {
        let current = open(&self.path, self.maximum)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let original = self.file.metadata().map_err(|_| UNAVAILABLE)?;
            let observed = current.metadata().map_err(|_| UNAVAILABLE)?;
            if original.dev() != observed.dev() || original.ino() != observed.ino() {
                return Err(UNAVAILABLE.into());
            }
        }
        if <[u8; 32]>::from(Sha256::digest(read(&current, self.maximum)?)) != self.digest {
            return Err(UNAVAILABLE.into());
        }
        Ok(())
    }
}

#[cfg(unix)]
fn read(file: &File, maximum: usize) -> Result<Vec<u8>, String> {
    use std::os::unix::fs::FileExt;
    let length = file.metadata().map_err(|_| UNAVAILABLE)?.len();
    if length > maximum as u64 {
        return Err(UNAVAILABLE.into());
    }
    let mut bytes = vec![0; maximum + 1];
    let mut offset = 0;
    while offset < bytes.len() {
        let count = file
            .read_at(&mut bytes[offset..], offset as u64)
            .map_err(|_| UNAVAILABLE)?;
        if count == 0 {
            break;
        }
        offset += count;
    }
    if offset > maximum {
        return Err(UNAVAILABLE.into());
    }
    bytes.truncate(offset);
    Ok(bytes)
}

#[cfg(not(unix))]
fn read(_: &File, _: usize) -> Result<Vec<u8>, String> {
    Err(UNAVAILABLE.into())
}

#[cfg(unix)]
fn open(path: &Path, maximum: usize) -> Result<File, String> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;

    if !path.is_absolute() {
        return Err(UNAVAILABLE.into());
    }
    let parts = path.components().collect::<Vec<_>>();
    if parts.len() < 3
        || parts[0] != Component::RootDir
        || parts[1..]
            .iter()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(UNAVAILABLE.into());
    }
    let user = unsafe { libc::geteuid() };
    let mut directory = File::open("/").map_err(|_| UNAVAILABLE)?;
    for (index, part) in parts[1..].iter().enumerate() {
        let Component::Normal(name) = part else {
            return Err(UNAVAILABLE.into());
        };
        let final_part = index + 2 == parts.len();
        let parent = directory.metadata().map_err(|_| UNAVAILABLE)?;
        // A root-owned sticky temporary base cannot rename another user's
        // private child. The final containing directory must still be private.
        let protected_sticky = parent.uid() == 0 && parent.mode() & 0o1000 != 0;
        if !parent.is_dir()
            || parent.uid() != 0 && parent.uid() != user
            || parent.mode() & 0o022 != 0 && !protected_sticky
            || final_part && (parent.uid() != user || parent.mode() & 0o077 != 0)
        {
            return Err(UNAVAILABLE.into());
        }
        let name = CString::new(name.as_bytes()).map_err(|_| UNAVAILABLE)?;
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | if final_part { 0 } else { libc::O_DIRECTORY };
        let descriptor = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if descriptor < 0 {
            return Err(UNAVAILABLE.into());
        }
        let file = unsafe { File::from_raw_fd(descriptor) };
        if final_part {
            let metadata = file.metadata().map_err(|_| UNAVAILABLE)?;
            if !metadata.is_file()
                || metadata.uid() != user
                || metadata.mode() & 0o077 != 0
                || metadata.nlink() != 1
                || metadata.len() > maximum as u64
            {
                return Err(UNAVAILABLE.into());
            }
            return Ok(file);
        }
        directory = file;
    }
    Err(UNAVAILABLE.into())
}

#[cfg(not(unix))]
fn open(_: &Path, _: usize) -> Result<File, String> {
    Err(UNAVAILABLE.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        // Canonicalize the fixture root because macOS's temporary base is a symlink.
        let root = tempfile::tempdir().unwrap();
        let private = root.path().canonicalize().unwrap().join("private");
        std::fs::create_dir(&private).unwrap();
        std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = private.join("config");
        std::fs::write(&path, b"original").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        (root, path)
    }

    #[test]
    fn pins_bytes_and_inode() {
        let (_root, path) = fixture();
        let (original, bytes) = ProtectedFile::open(&path, 64).unwrap();
        assert_eq!(bytes, b"original");
        original.check().unwrap();
        std::fs::write(&path, b"changed").unwrap();
        assert!(original.check().is_err());
        std::fs::write(&path, b"original").unwrap();
        original.check().unwrap();
        let replacement = path.with_file_name("replacement");
        std::fs::write(&replacement, b"original").unwrap();
        std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::rename(replacement, path).unwrap();
        assert!(original.check().is_err());
    }

    #[test]
    fn refuses_links_modes_and_bounds() {
        let (_root, path) = fixture();
        assert!(ProtectedFile::open(&path, 3).is_err());
        let link = path.with_file_name("link");
        symlink(&path, &link).unwrap();
        assert!(ProtectedFile::open(&link, 64).is_err());
        let linked_parent = path.parent().unwrap().with_file_name("linked-parent");
        symlink(path.parent().unwrap(), &linked_parent).unwrap();
        assert!(ProtectedFile::open(&linked_parent.join("config"), 64).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(ProtectedFile::open(&path, 64).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::set_permissions(
            path.parent().unwrap(),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        assert!(ProtectedFile::open(&path, 64).is_err());
    }
}
