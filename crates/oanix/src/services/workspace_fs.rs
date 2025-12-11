//! Real filesystem wrapper with path restrictions
//!
//! A FileService that wraps a real directory on the host filesystem.
//! Provides path security to prevent escaping the root directory.
//!
//! **Note:** This service is only available on native targets, not in WASM.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::error::FsError;
use crate::service::{DirEntry, FileHandle, FileService, Metadata, OpenFlags};

/// Real filesystem wrapper with path restrictions
///
/// Wraps a directory on the host filesystem, ensuring all access
/// is restricted to within that directory. Prevents path traversal
/// attacks using `..` or symlinks.
///
/// # Example
///
/// ```rust,ignore
/// use oanix::services::WorkspaceFs;
/// use oanix::service::{FileService, OpenFlags};
///
/// // Wrap a project directory
/// let workspace = WorkspaceFs::new("/home/user/project").unwrap();
///
/// // Read files within the workspace
/// let mut handle = workspace.open("/src/main.rs", OpenFlags::read_only()).unwrap();
///
/// // Attempting to escape fails
/// let result = workspace.open("/../../../etc/passwd", OpenFlags::read_only());
/// assert!(result.is_err());
/// ```
///
/// # Security
///
/// - All paths are canonicalized and checked to be within the root
/// - Symlinks that escape the root are rejected
/// - The root directory must exist and be accessible
pub struct WorkspaceFs {
    /// Canonicalized root directory
    root: PathBuf,
    /// Whether writes are allowed
    readonly: bool,
}

impl WorkspaceFs {
    /// Create a new WorkspaceFs wrapping the given directory
    ///
    /// The directory must exist. Returns an error if the path
    /// doesn't exist or isn't a directory.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, FsError> {
        let root = root.as_ref();

        // Canonicalize to resolve symlinks and get absolute path
        let canonical = fs::canonicalize(root).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FsError::NotFound(root.display().to_string())
            } else {
                FsError::Io(format!("failed to canonicalize path: {}", e))
            }
        })?;

        // Verify it's a directory
        if !canonical.is_dir() {
            return Err(FsError::NotADirectory(root.display().to_string()));
        }

        Ok(WorkspaceFs {
            root: canonical,
            readonly: false,
        })
    }

    /// Create a read-only WorkspaceFs
    pub fn readonly(root: impl AsRef<Path>) -> Result<Self, FsError> {
        let mut ws = Self::new(root)?;
        ws.readonly = true;
        Ok(ws)
    }

    /// Check if this workspace is read-only
    pub fn is_readonly(&self) -> bool {
        self.readonly
    }

    /// Get the root directory path
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a path within the workspace, ensuring it doesn't escape
    ///
    /// Returns the full host path if valid, or an error if the path
    /// would escape the workspace root.
    fn resolve_path(&self, path: &str) -> Result<PathBuf, FsError> {
        // Normalize the path
        let path = path.trim_start_matches('/');

        // Build the full path
        let full_path = if path.is_empty() || path == "." {
            self.root.clone()
        } else {
            self.root.join(path)
        };

        // For existing paths, canonicalize and check
        if full_path.exists() {
            let canonical = fs::canonicalize(&full_path).map_err(|e| {
                FsError::Io(format!("failed to resolve path: {}", e))
            })?;

            // Ensure it's within the root
            if !canonical.starts_with(&self.root) {
                return Err(FsError::PermissionDenied(format!(
                    "path escapes workspace: {}",
                    path
                )));
            }

            return Ok(canonical);
        }

        // For non-existing paths (new files), check the parent
        // and ensure we're not creating outside the workspace
        let normalized = self.normalize_path_components(path)?;
        let target = self.root.join(&normalized);

        // Verify the parent directory exists and is within workspace
        if let Some(parent) = target.parent() {
            if parent.exists() {
                let canonical_parent = fs::canonicalize(parent).map_err(|e| {
                    FsError::Io(format!("failed to resolve parent: {}", e))
                })?;

                if !canonical_parent.starts_with(&self.root) {
                    return Err(FsError::PermissionDenied(format!(
                        "path escapes workspace: {}",
                        path
                    )));
                }
            }
        }

        Ok(target)
    }

    /// Normalize path components, resolving . and .. safely
    fn normalize_path_components(&self, path: &str) -> Result<PathBuf, FsError> {
        let mut components = Vec::new();

        for part in path.split('/') {
            match part {
                "" | "." => continue,
                ".." => {
                    if components.is_empty() {
                        return Err(FsError::PermissionDenied(
                            "path escapes workspace root".into(),
                        ));
                    }
                    components.pop();
                }
                part => components.push(part),
            }
        }

        Ok(PathBuf::from(components.join("/")))
    }
}

impl FileService for WorkspaceFs {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError> {
        // Check readonly
        if self.readonly && (flags.write || flags.create || flags.truncate) {
            return Err(FsError::ReadOnly);
        }

        let full_path = self.resolve_path(path)?;

        // Check if it's a directory
        if full_path.exists() && full_path.is_dir() {
            return Err(FsError::NotAFile(path.to_string()));
        }

        // Build open options
        let mut opts = OpenOptions::new();

        if flags.read || (!flags.write && !flags.create) {
            opts.read(true);
        }
        if flags.write {
            opts.write(true);
        }
        if flags.create {
            opts.create(true);
        }
        if flags.truncate {
            opts.truncate(true);
        }
        if flags.append {
            opts.append(true);
        }

        let file = opts.open(&full_path).map_err(|e| {
            match e.kind() {
                std::io::ErrorKind::NotFound => FsError::NotFound(path.to_string()),
                std::io::ErrorKind::PermissionDenied => {
                    FsError::PermissionDenied(path.to_string())
                }
                std::io::ErrorKind::AlreadyExists => FsError::AlreadyExists(path.to_string()),
                _ => FsError::Io(format!("failed to open file: {}", e)),
            }
        })?;

        Ok(Box::new(RealFileHandle { file }))
    }

    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        let full_path = self.resolve_path(path)?;

        if !full_path.is_dir() {
            return Err(FsError::NotADirectory(path.to_string()));
        }

        let mut entries = Vec::new();

        for entry in fs::read_dir(&full_path).map_err(|e| {
            FsError::Io(format!("failed to read directory: {}", e))
        })? {
            let entry = entry.map_err(|e| {
                FsError::Io(format!("failed to read entry: {}", e))
            })?;

            let metadata = entry.metadata().map_err(|e| {
                FsError::Io(format!("failed to get metadata: {}", e))
            })?;

            let name = entry.file_name().to_string_lossy().to_string();

            entries.push(DirEntry {
                name,
                is_dir: metadata.is_dir(),
                size: if metadata.is_dir() { 0 } else { metadata.len() },
            });
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    fn stat(&self, path: &str) -> Result<Metadata, FsError> {
        let full_path = self.resolve_path(path)?;

        let metadata = fs::metadata(&full_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FsError::NotFound(path.to_string())
            } else {
                FsError::Io(format!("failed to get metadata: {}", e))
            }
        })?;

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(Metadata {
            is_dir: metadata.is_dir(),
            size: if metadata.is_dir() { 0 } else { metadata.len() },
            modified,
            readonly: self.readonly || metadata.permissions().readonly(),
        })
    }

    fn mkdir(&self, path: &str) -> Result<(), FsError> {
        if self.readonly {
            return Err(FsError::ReadOnly);
        }

        let full_path = self.resolve_path(path)?;

        if full_path.exists() {
            return Err(FsError::AlreadyExists(path.to_string()));
        }

        fs::create_dir(&full_path).map_err(|e| {
            match e.kind() {
                std::io::ErrorKind::NotFound => {
                    FsError::NotFound("parent directory does not exist".to_string())
                }
                std::io::ErrorKind::PermissionDenied => {
                    FsError::PermissionDenied(path.to_string())
                }
                _ => FsError::Io(format!("failed to create directory: {}", e)),
            }
        })
    }

    fn remove(&self, path: &str) -> Result<(), FsError> {
        if self.readonly {
            return Err(FsError::ReadOnly);
        }

        let full_path = self.resolve_path(path)?;

        if !full_path.exists() {
            return Err(FsError::NotFound(path.to_string()));
        }

        if full_path.is_dir() {
            fs::remove_dir(&full_path)
        } else {
            fs::remove_file(&full_path)
        }
        .map_err(|e| {
            match e.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    FsError::PermissionDenied(path.to_string())
                }
                _ => FsError::Io(format!("failed to remove: {}", e)),
            }
        })
    }

    fn rename(&self, from: &str, to: &str) -> Result<(), FsError> {
        if self.readonly {
            return Err(FsError::ReadOnly);
        }

        let from_path = self.resolve_path(from)?;
        let to_path = self.resolve_path(to)?;

        if !from_path.exists() {
            return Err(FsError::NotFound(from.to_string()));
        }

        fs::rename(&from_path, &to_path).map_err(|e| {
            FsError::Io(format!("failed to rename: {}", e))
        })
    }
}

/// File handle for real filesystem files
struct RealFileHandle {
    file: File,
}

impl FileHandle for RealFileHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FsError> {
        self.file
            .read(buf)
            .map_err(|e| FsError::Io(format!("read failed: {}", e)))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, FsError> {
        self.file
            .write(buf)
            .map_err(|e| FsError::Io(format!("write failed: {}", e)))
    }

    fn seek(&mut self, pos: u64) -> Result<(), FsError> {
        self.file
            .seek(SeekFrom::Start(pos))
            .map_err(|e| FsError::Io(format!("seek failed: {}", e)))?;
        Ok(())
    }

    fn position(&self) -> u64 {
        // Clone file to get position without &mut self
        // This is a bit awkward but necessary for the trait
        0 // Position tracking would need internal state
    }

    fn flush(&mut self) -> Result<(), FsError> {
        self.file
            .flush()
            .map_err(|e| FsError::Io(format!("flush failed: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_workspace() -> (TempDir, WorkspaceFs) {
        let temp = TempDir::new().unwrap();

        // Create some test files
        fs::create_dir(temp.path().join("src")).unwrap();
        fs::write(temp.path().join("README.md"), "# Test Project").unwrap();
        fs::write(temp.path().join("src/main.rs"), "fn main() {}").unwrap();

        let ws = WorkspaceFs::new(temp.path()).unwrap();
        (temp, ws)
    }

    #[test]
    fn test_read_file() {
        let (_temp, ws) = setup_test_workspace();

        let mut handle = ws.open("/README.md", OpenFlags::read_only()).unwrap();
        let mut buf = vec![0u8; 1024];
        let n = handle.read(&mut buf).unwrap();

        assert_eq!(&buf[..n], b"# Test Project");
    }

    #[test]
    fn test_read_nested_file() {
        let (_temp, ws) = setup_test_workspace();

        let mut handle = ws.open("/src/main.rs", OpenFlags::read_only()).unwrap();
        let mut buf = vec![0u8; 1024];
        let n = handle.read(&mut buf).unwrap();

        assert_eq!(&buf[..n], b"fn main() {}");
    }

    #[test]
    fn test_write_file() {
        let (temp, ws) = setup_test_workspace();

        // Write new file
        {
            let mut handle = ws
                .open(
                    "/new_file.txt",
                    OpenFlags {
                        write: true,
                        create: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            handle.write(b"Hello, World!").unwrap();
            handle.flush().unwrap();
        }

        // Verify on disk
        let content = fs::read_to_string(temp.path().join("new_file.txt")).unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[test]
    fn test_readdir() {
        let (_temp, ws) = setup_test_workspace();

        let entries = ws.readdir("/").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();

        assert!(names.contains(&"README.md"));
        assert!(names.contains(&"src"));
    }

    #[test]
    fn test_stat() {
        let (_temp, ws) = setup_test_workspace();

        let meta = ws.stat("/README.md").unwrap();
        assert!(!meta.is_dir);
        assert_eq!(meta.size, 14); // "# Test Project"

        let meta = ws.stat("/src").unwrap();
        assert!(meta.is_dir);
    }

    #[test]
    fn test_mkdir() {
        let (temp, ws) = setup_test_workspace();

        ws.mkdir("/new_dir").unwrap();
        assert!(temp.path().join("new_dir").is_dir());
    }

    #[test]
    fn test_remove_file() {
        let (temp, ws) = setup_test_workspace();

        assert!(temp.path().join("README.md").exists());
        ws.remove("/README.md").unwrap();
        assert!(!temp.path().join("README.md").exists());
    }

    #[test]
    fn test_rename() {
        let (temp, ws) = setup_test_workspace();

        ws.rename("/README.md", "/RENAMED.md").unwrap();

        assert!(!temp.path().join("README.md").exists());
        assert!(temp.path().join("RENAMED.md").exists());
    }

    #[test]
    fn test_path_escape_dotdot() {
        let (_temp, ws) = setup_test_workspace();

        // Try to escape with ..
        let result = ws.open("/../../../etc/passwd", OpenFlags::read_only());
        assert!(result.is_err());
    }

    #[test]
    fn test_path_escape_normalized() {
        let (_temp, ws) = setup_test_workspace();

        // Try various escape attempts
        let result = ws.resolve_path("../../../etc/passwd");
        assert!(result.is_err());

        let result = ws.resolve_path("src/../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_readonly_mode() {
        let (_temp, ws) = setup_test_workspace();
        let ws_ro = WorkspaceFs::readonly(ws.root()).unwrap();

        // Reading should work
        let handle = ws_ro.open("/README.md", OpenFlags::read_only());
        assert!(handle.is_ok());

        // Writing should fail
        let result = ws_ro.open(
            "/new.txt",
            OpenFlags {
                write: true,
                create: true,
                ..Default::default()
            },
        );
        assert!(result.is_err());

        // mkdir should fail
        assert!(ws_ro.mkdir("/newdir").is_err());

        // remove should fail
        assert!(ws_ro.remove("/README.md").is_err());
    }

    #[test]
    fn test_nonexistent_root() {
        let result = WorkspaceFs::new("/definitely/does/not/exist");
        assert!(result.is_err());
    }
}
