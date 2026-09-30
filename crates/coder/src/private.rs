//! Directories and files only this user reaches: `0700` and `0600` on
//! Unix; on Windows an owner-only DACL the directory's files inherit
//! (`private-fs`).

use std::fs::OpenOptions;
use std::path::Path;

/// Creates `path` and any missing parents for this user alone.
#[cfg(unix)]
pub(crate) fn create_dir_all(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
}

/// Creates `path` and any missing parents for this user alone.
#[cfg(windows)]
pub(crate) fn create_dir_all(path: &Path) -> std::io::Result<()> {
    private_fs::create_dir_all(path)
}

/// Makes a file these options create `0600`; on Windows it inherits its
/// private directory's DACL.
pub(crate) fn file(options: &mut OpenOptions) -> &mut OpenOptions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}
