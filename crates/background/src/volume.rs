//! Free space on a volume, behind a trait so tests choose it.

use std::os::unix::fs::MetadataExt;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// One volume's size and free space.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Space {
    /// The device ID.
    pub device: u64,
    /// Bytes available to this user (`f_bavail × f_frsize`).
    pub free: u64,
    /// The volume's size.
    pub total: u64,
}

/// Reads a volume's free space.
pub trait Volumes: Send + Sync {
    /// The volume holding `path`.
    ///
    /// # Errors
    /// The path or its volume cannot be read.
    fn space(&self, path: &Path) -> std::io::Result<Space>;
}

/// `statvfs`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Statvfs;

impl Volumes for Statvfs {
    fn space(&self, path: &Path) -> std::io::Result<Space> {
        use std::os::unix::ffi::OsStrExt;
        let device = std::fs::metadata(path)?.dev();
        let name = std::ffi::CString::new(path.as_os_str().as_bytes())
            .map_err(|_| std::io::Error::other("a path with a NUL byte"))?;
        let mut stat = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
        // SAFETY: `name` is a valid C string and `stat` is a writable
        // `statvfs` the call fills on success.
        let result = unsafe { libc::statvfs(name.as_ptr(), stat.as_mut_ptr()) };
        if result != 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: statvfs returned 0, so it initialized `stat`.
        let stat = unsafe { stat.assume_init() };
        #[allow(clippy::unnecessary_cast, clippy::useless_conversion)]
        let unit = u64::from(stat.f_frsize as u64);
        #[allow(clippy::unnecessary_cast, clippy::useless_conversion)]
        let (available, blocks) = (
            u64::from(stat.f_bavail as u64),
            u64::from(stat.f_blocks as u64),
        );
        Ok(Space {
            device,
            free: available.saturating_mul(unit),
            total: blocks.saturating_mul(unit),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statvfs_reads_a_plausible_volume() {
        let dir = tempfile::tempdir().unwrap();
        let space = Statvfs.space(dir.path()).unwrap();
        assert!(space.total > 0);
        assert!(space.free <= space.total);
    }
}
