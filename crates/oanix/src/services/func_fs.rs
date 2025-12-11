//! Computed/dynamic filesystem implementation
//!
//! Files whose content is computed on-the-fly via closures.
//! Perfect for status files, control files, and live system info.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::error::FsError;
use crate::service::{DirEntry, FileHandle, FileService, Metadata, OpenFlags};

/// A function that produces file content when read
pub type ReadFn = Arc<dyn Fn() -> Vec<u8> + Send + Sync>;

/// A function that handles file writes
pub type WriteFn = Arc<dyn Fn(Vec<u8>) + Send + Sync>;

/// Computed/dynamic filesystem
///
/// Each file is backed by closures that compute content on read
/// and optionally handle writes. This is perfect for:
/// - `/task/status` - Returns current task state as JSON
/// - `/sys/time` - Returns current timestamp
/// - `/cap/*/control` - Control files that trigger actions
///
/// # Example
///
/// ```rust
/// use oanix::services::FuncFs;
/// use oanix::service::{FileService, OpenFlags};
/// use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
///
/// let counter = Arc::new(AtomicU64::new(0));
/// let counter_read = counter.clone();
/// let counter_write = counter.clone();
///
/// let fs = FuncFs::builder()
///     .read_only("/time", || {
///         format!("{}", std::time::SystemTime::now()
///             .duration_since(std::time::UNIX_EPOCH)
///             .unwrap().as_secs()).into_bytes()
///     })
///     .read_write(
///         "/counter",
///         move || counter_read.load(Ordering::SeqCst).to_string().into_bytes(),
///         move |data| {
///             if let Ok(s) = String::from_utf8(data) {
///                 if let Ok(n) = s.trim().parse::<u64>() {
///                     counter_write.store(n, Ordering::SeqCst);
///                 }
///             }
///         }
///     )
///     .build();
/// ```
pub struct FuncFs {
    files: HashMap<String, FuncFile>,
    // Implicit directories derived from file paths
    directories: HashSet<String>,
}

struct FuncFile {
    read_fn: ReadFn,
    write_fn: Option<WriteFn>,
}

/// Builder for FuncFs
pub struct FuncFsBuilder {
    files: HashMap<String, FuncFile>,
}

impl FuncFsBuilder {
    fn new() -> Self {
        FuncFsBuilder {
            files: HashMap::new(),
        }
    }

    /// Add a read-only computed file
    pub fn read_only<R>(mut self, path: &str, read_fn: R) -> Self
    where
        R: Fn() -> Vec<u8> + Send + Sync + 'static,
    {
        let normalized = normalize_path(path);
        self.files.insert(
            normalized,
            FuncFile {
                read_fn: Arc::new(read_fn),
                write_fn: None,
            },
        );
        self
    }

    /// Add a read-write computed file
    pub fn read_write<R, W>(mut self, path: &str, read_fn: R, write_fn: W) -> Self
    where
        R: Fn() -> Vec<u8> + Send + Sync + 'static,
        W: Fn(Vec<u8>) + Send + Sync + 'static,
    {
        let normalized = normalize_path(path);
        self.files.insert(
            normalized,
            FuncFile {
                read_fn: Arc::new(read_fn),
                write_fn: Some(Arc::new(write_fn)),
            },
        );
        self
    }

    /// Add a write-only control file (reads return empty)
    pub fn write_only<W>(mut self, path: &str, write_fn: W) -> Self
    where
        W: Fn(Vec<u8>) + Send + Sync + 'static,
    {
        let normalized = normalize_path(path);
        self.files.insert(
            normalized,
            FuncFile {
                read_fn: Arc::new(|| Vec::new()),
                write_fn: Some(Arc::new(write_fn)),
            },
        );
        self
    }

    /// Build the FuncFs
    pub fn build(self) -> FuncFs {
        // Compute implicit directories from file paths
        let mut directories = HashSet::new();
        directories.insert(String::new()); // Root

        for path in self.files.keys() {
            let mut current = String::new();
            for component in path.split('/').filter(|s| !s.is_empty()) {
                // Don't include the file itself as a directory
                if !current.is_empty() || directories.contains(&current) {
                    if !current.is_empty() {
                        current.push('/');
                    }
                    // Check if this is the last component (the file)
                    let test_path = if current.is_empty() {
                        component.to_string()
                    } else {
                        format!("{}/{}", current, component)
                    };

                    if self.files.contains_key(&test_path) {
                        // This is a file, don't add as directory
                        break;
                    }

                    current = test_path;
                    directories.insert(current.clone());
                }
            }
        }

        // Re-derive directories more simply
        let mut directories = HashSet::new();
        directories.insert(String::new()); // Root

        for path in self.files.keys() {
            let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let mut current = String::new();
            for part in &parts[..parts.len().saturating_sub(1)] {
                if !current.is_empty() {
                    current.push('/');
                }
                current.push_str(part);
                directories.insert(current.clone());
            }
        }

        FuncFs {
            files: self.files,
            directories,
        }
    }
}

impl FuncFs {
    /// Create a new builder for FuncFs
    pub fn builder() -> FuncFsBuilder {
        FuncFsBuilder::new()
    }
}

impl FileService for FuncFs {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError> {
        let normalized = normalize_path(path);

        // Check if it's a directory
        if self.directories.contains(&normalized) && !self.files.contains_key(&normalized) {
            return Err(FsError::NotAFile(path.to_string()));
        }

        let file = self
            .files
            .get(&normalized)
            .ok_or_else(|| FsError::NotFound(path.to_string()))?;

        // Check write permission
        if flags.write && file.write_fn.is_none() {
            return Err(FsError::ReadOnly);
        }

        Ok(Box::new(FuncFileHandle {
            read_fn: file.read_fn.clone(),
            write_fn: file.write_fn.clone(),
            read_buffer: None,
            read_position: 0,
            write_buffer: Vec::new(),
            flags,
        }))
    }

    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        let normalized = normalize_path(path);

        // Check if this is a valid directory
        if !self.directories.contains(&normalized) {
            if self.files.contains_key(&normalized) {
                return Err(FsError::NotADirectory(path.to_string()));
            }
            return Err(FsError::NotFound(path.to_string()));
        }

        let prefix = if normalized.is_empty() {
            String::new()
        } else {
            format!("{}/", normalized)
        };

        let mut entries = HashMap::new();

        // Find direct children (files)
        for file_path in self.files.keys() {
            if normalized.is_empty() {
                // Root directory - get first component
                if let Some(first) = file_path.split('/').next() {
                    if !first.is_empty() {
                        let is_dir = self.directories.contains(first);
                        entries.insert(
                            first.to_string(),
                            DirEntry {
                                name: first.to_string(),
                                is_dir,
                                size: if is_dir { 0 } else {
                                    self.files.get(first).map(|f| (f.read_fn)().len() as u64).unwrap_or(0)
                                },
                            },
                        );
                    }
                }
            } else if file_path.starts_with(&prefix) {
                let rest = &file_path[prefix.len()..];
                if let Some(first) = rest.split('/').next() {
                    if !first.is_empty() {
                        let child_path = format!("{}{}", prefix, first);
                        let child_path_normalized = child_path.trim_start_matches('/').to_string();
                        let is_dir = self.directories.contains(&child_path_normalized);
                        entries.insert(
                            first.to_string(),
                            DirEntry {
                                name: first.to_string(),
                                is_dir,
                                size: if is_dir { 0 } else {
                                    self.files.get(&child_path_normalized).map(|f| (f.read_fn)().len() as u64).unwrap_or(0)
                                },
                            },
                        );
                    }
                }
            }
        }

        // Find direct children (directories)
        for dir_path in &self.directories {
            if dir_path.is_empty() {
                continue;
            }

            if normalized.is_empty() {
                // Root - get first component of each directory
                if let Some(first) = dir_path.split('/').next() {
                    if !first.is_empty() && !entries.contains_key(first) {
                        entries.insert(
                            first.to_string(),
                            DirEntry {
                                name: first.to_string(),
                                is_dir: true,
                                size: 0,
                            },
                        );
                    }
                }
            } else if dir_path.starts_with(&prefix) {
                let rest = &dir_path[prefix.len()..];
                if let Some(first) = rest.split('/').next() {
                    if !first.is_empty() && !entries.contains_key(first) {
                        entries.insert(
                            first.to_string(),
                            DirEntry {
                                name: first.to_string(),
                                is_dir: true,
                                size: 0,
                            },
                        );
                    }
                }
            }
        }

        let mut result: Vec<DirEntry> = entries.into_values().collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    fn stat(&self, path: &str) -> Result<Metadata, FsError> {
        let normalized = normalize_path(path);

        // Check if it's a file
        if let Some(file) = self.files.get(&normalized) {
            let content = (file.read_fn)();
            return Ok(Metadata {
                is_dir: false,
                size: content.len() as u64,
                modified: 0, // Dynamic - no meaningful modification time
                readonly: file.write_fn.is_none(),
            });
        }

        // Check if it's a directory
        if self.directories.contains(&normalized) {
            return Ok(Metadata {
                is_dir: true,
                size: 0,
                modified: 0,
                readonly: true,
            });
        }

        Err(FsError::NotFound(path.to_string()))
    }

    fn mkdir(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied(
            "FuncFs structure is fixed at construction".into(),
        ))
    }

    fn remove(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied(
            "FuncFs structure is fixed at construction".into(),
        ))
    }

    fn rename(&self, _from: &str, _to: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied(
            "FuncFs structure is fixed at construction".into(),
        ))
    }
}

/// Handle to an open file in FuncFs
struct FuncFileHandle {
    read_fn: ReadFn,
    write_fn: Option<WriteFn>,
    read_buffer: Option<Vec<u8>>,
    read_position: usize,
    write_buffer: Vec<u8>,
    flags: OpenFlags,
}

impl FileHandle for FuncFileHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FsError> {
        if !self.flags.read {
            return Err(FsError::PermissionDenied(
                "file not opened for reading".into(),
            ));
        }

        // Lazy-load content on first read
        if self.read_buffer.is_none() {
            self.read_buffer = Some((self.read_fn)());
        }

        let content = self.read_buffer.as_ref().unwrap();
        if self.read_position >= content.len() {
            return Ok(0);
        }

        let available = &content[self.read_position..];
        let to_read = buf.len().min(available.len());
        buf[..to_read].copy_from_slice(&available[..to_read]);
        self.read_position += to_read;
        Ok(to_read)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, FsError> {
        if !self.flags.write {
            return Err(FsError::PermissionDenied(
                "file not opened for writing".into(),
            ));
        }

        if self.write_fn.is_none() {
            return Err(FsError::ReadOnly);
        }

        self.write_buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, pos: u64) -> Result<(), FsError> {
        self.read_position = pos as usize;
        Ok(())
    }

    fn position(&self) -> u64 {
        self.read_position as u64
    }

    fn flush(&mut self) -> Result<(), FsError> {
        if let Some(write_fn) = &self.write_fn {
            if !self.write_buffer.is_empty() {
                let data = std::mem::take(&mut self.write_buffer);
                (write_fn)(data);
            }
        }
        Ok(())
    }
}

impl Drop for FuncFileHandle {
    fn drop(&mut self) {
        // Flush on drop
        let _ = self.flush();
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
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn test_read_only_file() {
        let fs = FuncFs::builder()
            .read_only("/status", || b"running".to_vec())
            .build();

        let mut handle = fs.open("/status", OpenFlags::read_only()).unwrap();
        let mut buf = [0u8; 32];
        let n = handle.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"running");
    }

    #[test]
    fn test_read_write_file() {
        let counter = Arc::new(AtomicU64::new(42));
        let counter_read = counter.clone();
        let counter_write = counter.clone();

        let fs = FuncFs::builder()
            .read_write(
                "/counter",
                move || counter_read.load(Ordering::SeqCst).to_string().into_bytes(),
                move |data| {
                    if let Ok(s) = String::from_utf8(data) {
                        if let Ok(n) = s.trim().parse::<u64>() {
                            counter_write.store(n, Ordering::SeqCst);
                        }
                    }
                },
            )
            .build();

        // Read initial value
        {
            let mut handle = fs.open("/counter", OpenFlags::read_only()).unwrap();
            let mut buf = [0u8; 32];
            let n = handle.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], b"42");
        }

        // Write new value
        {
            let mut handle = fs.open("/counter", OpenFlags::write_only()).unwrap();
            handle.write(b"100").unwrap();
            handle.flush().unwrap();
        }

        // Read updated value
        {
            let mut handle = fs.open("/counter", OpenFlags::read_only()).unwrap();
            let mut buf = [0u8; 32];
            let n = handle.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], b"100");
        }
    }

    #[test]
    fn test_nested_structure() {
        let fs = FuncFs::builder()
            .read_only("/task/status", || b"pending".to_vec())
            .read_only("/task/id", || b"abc123".to_vec())
            .read_only("/sys/time", || b"1234567890".to_vec())
            .build();

        // Check root directory
        let root_entries = fs.readdir("/").unwrap();
        assert_eq!(root_entries.len(), 2); // task, sys

        // Check task directory
        let task_entries = fs.readdir("/task").unwrap();
        assert_eq!(task_entries.len(), 2); // status, id

        // Stat directory
        let task_meta = fs.stat("/task").unwrap();
        assert!(task_meta.is_dir);

        // Stat file
        let status_meta = fs.stat("/task/status").unwrap();
        assert!(!status_meta.is_dir);
    }

    #[test]
    fn test_write_to_readonly_fails() {
        let fs = FuncFs::builder()
            .read_only("/readonly", || b"data".to_vec())
            .build();

        let result = fs.open("/readonly", OpenFlags::write_only());
        assert!(result.is_err());
    }

    #[test]
    fn test_write_only_file() {
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let fs = FuncFs::builder()
            .write_only("/control", move |data| {
                *received_clone.lock().unwrap() = data;
            })
            .build();

        // Write to control file
        {
            let mut handle = fs.open("/control", OpenFlags::write_only()).unwrap();
            handle.write(b"command").unwrap();
            handle.flush().unwrap();
        }

        // Verify data was received
        assert_eq!(*received.lock().unwrap(), b"command");
    }

    #[test]
    fn test_dynamic_content() {
        let call_count = Arc::new(AtomicU64::new(0));
        let call_count_clone = call_count.clone();

        let fs = FuncFs::builder()
            .read_only("/calls", move || {
                let count = call_count_clone.fetch_add(1, Ordering::SeqCst) + 1;
                count.to_string().into_bytes()
            })
            .build();

        // Each open should get fresh content
        for expected in 1..=3 {
            let mut handle = fs.open("/calls", OpenFlags::read_only()).unwrap();
            let mut buf = [0u8; 32];
            let n = handle.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], expected.to_string().as_bytes());
        }
    }
}
