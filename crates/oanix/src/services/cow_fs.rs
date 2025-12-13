//! Copy-on-Write filesystem implementation
//!
//! Layers a writable overlay on top of a read-only base filesystem.
//! Perfect for workspace snapshots, undo/redo, and branching experiments.

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use crate::error::FsError;
use crate::service::{DirEntry, FileHandle, FileService, Metadata, OpenFlags};
use crate::services::MemFs;

/// Copy-on-Write filesystem
///
/// Layers a writable `MemFs` overlay on top of any read-only base filesystem.
/// - Reads check overlay first, then fall back to base
/// - Writes always go to overlay (copying from base if needed)
/// - Deletes are tracked as "tombstones"
///
/// This is perfect for:
/// - Workspace snapshots for benchmarking
/// - Undo/redo support
/// - Branching experiments without modifying the original
///
/// # Example
///
/// ```rust
/// use oanix::services::{MapFs, CowFs};
/// use oanix::service::{FileService, OpenFlags};
///
/// // Create a read-only base
/// let base = MapFs::builder()
///     .file("/readme.txt", b"Original content")
///     .build();
///
/// // Wrap with copy-on-write
/// let cow = CowFs::new(base);
///
/// // Modify - creates copy in overlay
/// {
///     let mut handle = cow.open("/readme.txt", OpenFlags::read_write()).unwrap();
///     handle.write(b"Modified!").unwrap();
/// }
///
/// // Original base is unchanged, reads come from overlay
/// ```
pub struct CowFs {
    base: Arc<dyn FileService>,
    overlay: MemFs,
    tombstones: RwLock<HashSet<String>>,
}

impl CowFs {
    /// Create a new CowFs wrapping the given base filesystem
    pub fn new<F: FileService + 'static>(base: F) -> Self {
        CowFs {
            base: Arc::new(base),
            overlay: MemFs::new(),
            tombstones: RwLock::new(HashSet::new()),
        }
    }

    /// Create from an Arc'd base (useful when base is shared)
    pub fn from_arc(base: Arc<dyn FileService>) -> Self {
        CowFs {
            base,
            overlay: MemFs::new(),
            tombstones: RwLock::new(HashSet::new()),
        }
    }

    /// Check if a path is tombstoned (deleted)
    fn is_tombstoned(&self, path: &str) -> bool {
        let normalized = normalize_path(path);
        let tombstones = self.tombstones.read().unwrap();

        // Check exact match
        if tombstones.contains(&normalized) {
            return true;
        }

        // Check if any parent is tombstoned
        let mut current = String::new();
        for component in normalized.split('/').filter(|s| !s.is_empty()) {
            if !current.is_empty() {
                current.push('/');
            }
            current.push_str(component);
            if tombstones.contains(&current) {
                return true;
            }
        }

        false
    }

    /// Copy a file from base to overlay
    fn copy_to_overlay(&self, path: &str) -> Result<(), FsError> {
        let normalized = normalize_path(path);

        // Read from base
        let mut base_handle = self.base.open(path, OpenFlags::read_only())?;
        let stat = self.base.stat(path)?;

        let mut content = vec![0u8; stat.size as usize];
        let mut total_read = 0;
        while total_read < content.len() {
            let n = base_handle.read(&mut content[total_read..])?;
            if n == 0 {
                break;
            }
            total_read += n;
        }
        content.truncate(total_read);

        // Ensure parent directories exist in overlay
        self.ensure_overlay_parents(&normalized)?;

        // Write to overlay
        let mut overlay_handle = self.overlay.open(
            path,
            OpenFlags {
                write: true,
                create: true,
                truncate: true,
                ..Default::default()
            },
        )?;
        overlay_handle.write(&content)?;
        overlay_handle.flush()?;

        Ok(())
    }

    /// Ensure parent directories exist in overlay
    fn ensure_overlay_parents(&self, path: &str) -> Result<(), FsError> {
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        let mut current = String::new();
        for component in &components[..components.len().saturating_sub(1)] {
            if !current.is_empty() {
                current.push('/');
            }
            current.push_str(component);

            // Try to create directory (ignore if exists)
            let dir_path = format!("/{}", current);
            if self.overlay.stat(&dir_path).is_err() {
                let _ = self.overlay.mkdir(&dir_path);
            }
        }

        Ok(())
    }
}

impl FileService for CowFs {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError> {
        let normalized = normalize_path(path);

        // Check tombstones
        if self.is_tombstoned(path) {
            if flags.create {
                // Remove from tombstones and create in overlay
                self.tombstones.write().unwrap().remove(&normalized);
                self.ensure_overlay_parents(&normalized)?;
                return self.overlay.open(path, flags);
            }
            return Err(FsError::NotFound(path.to_string()));
        }

        // If writing, ensure file is in overlay
        if flags.write {
            // Check if already in overlay
            if self.overlay.stat(path).is_ok() {
                return self.overlay.open(path, flags);
            }

            // Check if exists in base - if so, copy to overlay
            if self.base.stat(path).is_ok() {
                self.copy_to_overlay(path)?;
                return self.overlay.open(path, flags);
            }

            // Creating new file
            if flags.create {
                self.ensure_overlay_parents(&normalized)?;
                return self.overlay.open(path, flags);
            }

            return Err(FsError::NotFound(path.to_string()));
        }

        // Read-only: try overlay first, then base
        if let Ok(handle) = self.overlay.open(path, flags) {
            return Ok(handle);
        }

        self.base.open(path, flags)
    }

    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        // Check tombstones
        if self.is_tombstoned(path) {
            return Err(FsError::NotFound(path.to_string()));
        }

        let mut entries = std::collections::HashMap::new();

        // Get entries from base (if not tombstoned)
        if let Ok(base_entries) = self.base.readdir(path) {
            for entry in base_entries {
                let entry_path = if path == "/" || path.is_empty() {
                    entry.name.clone()
                } else {
                    format!("{}/{}", normalize_path(path), entry.name)
                };

                if !self.is_tombstoned(&entry_path) {
                    entries.insert(entry.name.clone(), entry);
                }
            }
        }

        // Get entries from overlay (these override base)
        if let Ok(overlay_entries) = self.overlay.readdir(path) {
            for entry in overlay_entries {
                entries.insert(entry.name.clone(), entry);
            }
        }

        // If no entries and path doesn't exist, return error
        if entries.is_empty() {
            // Check if the directory itself exists
            let base_exists = self.base.stat(path).is_ok();
            let overlay_exists = self.overlay.stat(path).is_ok();

            if !base_exists && !overlay_exists {
                return Err(FsError::NotFound(path.to_string()));
            }
        }

        let mut result: Vec<DirEntry> = entries.into_values().collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    fn stat(&self, path: &str) -> Result<Metadata, FsError> {
        // Check tombstones
        if self.is_tombstoned(path) {
            return Err(FsError::NotFound(path.to_string()));
        }

        // Try overlay first
        if let Ok(meta) = self.overlay.stat(path) {
            return Ok(meta);
        }

        // Fall back to base
        self.base.stat(path)
    }

    fn mkdir(&self, path: &str) -> Result<(), FsError> {
        let normalized = normalize_path(path);

        // Remove from tombstones if present
        self.tombstones.write().unwrap().remove(&normalized);

        // Check if already exists
        if self.stat(path).is_ok() {
            return Err(FsError::AlreadyExists(path.to_string()));
        }

        // Ensure parents exist
        self.ensure_overlay_parents(&normalized)?;

        // Create in overlay
        self.overlay.mkdir(path)
    }

    fn remove(&self, path: &str) -> Result<(), FsError> {
        let normalized = normalize_path(path);

        // Check that it exists somewhere
        self.stat(path)?;

        // Add to tombstones
        self.tombstones.write().unwrap().insert(normalized);

        // Remove from overlay if present
        let _ = self.overlay.remove(path);

        Ok(())
    }

    fn rename(&self, from: &str, to: &str) -> Result<(), FsError> {
        let from_normalized = normalize_path(from);
        let to_normalized = normalize_path(to);

        // Check source exists
        let stat = self.stat(from)?;

        if stat.is_dir {
            // For directories, we need to handle this carefully
            // For now, just error - full implementation would recursively copy
            return Err(FsError::PermissionDenied(
                "directory rename not yet supported in CowFs".into(),
            ));
        }

        // Copy content to new location
        let content = {
            let mut handle = self.open(from, OpenFlags::read_only())?;
            let mut buf = vec![0u8; stat.size as usize];
            let mut total = 0;
            while total < buf.len() {
                let n = handle.read(&mut buf[total..])?;
                if n == 0 {
                    break;
                }
                total += n;
            }
            buf.truncate(total);
            buf
        };

        // Ensure parents exist for destination
        self.ensure_overlay_parents(&to_normalized)?;

        // Write to new location
        {
            let mut handle = self.overlay.open(
                to,
                OpenFlags {
                    write: true,
                    create: true,
                    truncate: true,
                    ..Default::default()
                },
            )?;
            handle.write(&content)?;
            handle.flush()?;
        }

        // Tombstone the old path
        self.tombstones.write().unwrap().insert(from_normalized);

        // Remove from overlay if it was there
        let _ = self.overlay.remove(from);

        Ok(())
    }
}

/// Normalize path: remove leading/trailing slashes
fn normalize_path(path: &str) -> String {
    let path = path.trim_start_matches('/');
    let path = path.trim_end_matches('/');
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MapFs;

    #[test]
    fn test_read_through() {
        let base = MapFs::builder()
            .file("/readme.txt", b"Hello from base!")
            .build();

        let cow = CowFs::new(base);

        // Read should go to base
        let mut handle = cow.open("/readme.txt", OpenFlags::read_only()).unwrap();
        let mut buf = [0u8; 32];
        let n = handle.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"Hello from base!");
    }

    #[test]
    fn test_write_creates_copy() {
        let base = MapFs::builder().file("/readme.txt", b"Original").build();

        let cow = CowFs::new(base);

        // Write triggers copy-on-write
        {
            let mut handle = cow
                .open(
                    "/readme.txt",
                    OpenFlags {
                        read: true,
                        write: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            handle.write(b"Modified!").unwrap();
            handle.flush().unwrap();
        }

        // Read should return modified content
        let mut handle = cow.open("/readme.txt", OpenFlags::read_only()).unwrap();
        let mut buf = [0u8; 32];
        let n = handle.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"Modified!");
    }

    #[test]
    fn test_create_new_file() {
        let base = MapFs::builder().file("/existing.txt", b"exists").build();

        let cow = CowFs::new(base);

        // Create new file in overlay
        {
            let mut handle = cow
                .open(
                    "/new.txt",
                    OpenFlags {
                        write: true,
                        create: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            handle.write(b"New file!").unwrap();
            handle.flush().unwrap();
        }

        // Verify both files exist
        assert!(cow.stat("/existing.txt").is_ok());
        assert!(cow.stat("/new.txt").is_ok());

        // Read new file
        let mut handle = cow.open("/new.txt", OpenFlags::read_only()).unwrap();
        let mut buf = [0u8; 32];
        let n = handle.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"New file!");
    }

    #[test]
    fn test_delete_tombstones() {
        let base = MapFs::builder()
            .file("/to_delete.txt", b"delete me")
            .build();

        let cow = CowFs::new(base);

        // Verify exists
        assert!(cow.stat("/to_delete.txt").is_ok());

        // Delete
        cow.remove("/to_delete.txt").unwrap();

        // Should be gone
        assert!(cow.stat("/to_delete.txt").is_err());
    }

    #[test]
    fn test_recreate_after_delete() {
        let base = MapFs::builder().file("/file.txt", b"original").build();

        let cow = CowFs::new(base);

        // Delete
        cow.remove("/file.txt").unwrap();
        assert!(cow.stat("/file.txt").is_err());

        // Recreate
        {
            let mut handle = cow
                .open(
                    "/file.txt",
                    OpenFlags {
                        write: true,
                        create: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            handle.write(b"recreated").unwrap();
            handle.flush().unwrap();
        }

        // Verify new content
        let mut handle = cow.open("/file.txt", OpenFlags::read_only()).unwrap();
        let mut buf = [0u8; 32];
        let n = handle.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"recreated");
    }

    #[test]
    fn test_readdir_merged() {
        let base = MapFs::builder()
            .file("/base_file.txt", b"from base")
            .dir("/shared")
            .file("/shared/base.txt", b"base")
            .build();

        let cow = CowFs::new(base);

        // Add overlay file
        {
            let mut handle = cow
                .open(
                    "/overlay_file.txt",
                    OpenFlags {
                        write: true,
                        create: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            handle.write(b"from overlay").unwrap();
            handle.flush().unwrap();
        }

        // Root should have both
        let entries = cow.readdir("/").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"base_file.txt"));
        assert!(names.contains(&"overlay_file.txt"));
        assert!(names.contains(&"shared"));
    }

    #[test]
    fn test_readdir_excludes_tombstoned() {
        let base = MapFs::builder()
            .file("/keep.txt", b"keep")
            .file("/delete.txt", b"delete")
            .build();

        let cow = CowFs::new(base);

        // Delete one file
        cow.remove("/delete.txt").unwrap();

        // Should only see kept file
        let entries = cow.readdir("/").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "keep.txt");
    }

    #[test]
    fn test_mkdir_in_overlay() {
        let base = MapFs::builder().file("/file.txt", b"file").build();

        let cow = CowFs::new(base);

        // Create directory
        cow.mkdir("/newdir").unwrap();

        // Verify it exists
        let meta = cow.stat("/newdir").unwrap();
        assert!(meta.is_dir);

        // Create file inside
        {
            let mut handle = cow
                .open(
                    "/newdir/file.txt",
                    OpenFlags {
                        write: true,
                        create: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            handle.write(b"nested").unwrap();
            handle.flush().unwrap();
        }

        // Verify
        let entries = cow.readdir("/newdir").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "file.txt");
    }

    #[test]
    fn test_rename_file() {
        let base = MapFs::builder().file("/old.txt", b"content").build();

        let cow = CowFs::new(base);

        // Rename
        cow.rename("/old.txt", "/new.txt").unwrap();

        // Old should be gone
        assert!(cow.stat("/old.txt").is_err());

        // New should have content
        let mut handle = cow.open("/new.txt", OpenFlags::read_only()).unwrap();
        let mut buf = [0u8; 32];
        let n = handle.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"content");
    }

    #[test]
    fn test_nested_directory_in_base() {
        let base = MapFs::builder()
            .file("/docs/readme.md", b"# README")
            .file("/docs/notes/todo.txt", b"TODO")
            .build();

        let cow = CowFs::new(base);

        // Should be able to read nested files
        let mut handle = cow
            .open("/docs/notes/todo.txt", OpenFlags::read_only())
            .unwrap();
        let mut buf = [0u8; 32];
        let n = handle.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"TODO");

        // Should be able to list directories
        let entries = cow.readdir("/docs").unwrap();
        assert_eq!(entries.len(), 2); // readme.md, notes
    }
}
