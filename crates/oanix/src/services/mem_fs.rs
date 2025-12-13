//! In-memory filesystem implementation
//!
//! A thread-safe, fully read/write filesystem that stores all data in memory.
//! Inspired by wanix's memfs but implemented in Rust.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[cfg(target_arch = "wasm32")]
use js_sys;

use crate::error::FsError;
use crate::service::{DirEntry, FileHandle, FileService, Metadata, OpenFlags};

/// In-memory filesystem
///
/// Provides a fully functional filesystem stored entirely in memory.
/// Thread-safe via RwLock.
///
/// # Example
///
/// ```rust
/// use oanix::services::MemFs;
/// use oanix::service::{FileService, OpenFlags};
///
/// let fs = MemFs::new();
/// fs.mkdir("/docs").unwrap();
///
/// let mut file = fs.open("/docs/readme.txt", OpenFlags {
///     write: true,
///     create: true,
///     ..Default::default()
/// }).unwrap();
///
/// file.write(b"Hello, OANIX!").unwrap();
/// ```
pub struct MemFs {
    root: Arc<RwLock<MemNode>>,
}

#[derive(Debug, Clone)]
enum MemNode {
    File {
        content: Vec<u8>,
        modified: u64,
    },
    Dir {
        children: HashMap<String, MemNode>,
        modified: u64,
    },
}

impl MemNode {
    fn new_file() -> Self {
        MemNode::File {
            content: Vec::new(),
            modified: now(),
        }
    }

    fn new_dir() -> Self {
        MemNode::Dir {
            children: HashMap::new(),
            modified: now(),
        }
    }

    fn is_dir(&self) -> bool {
        matches!(self, MemNode::Dir { .. })
    }

    #[allow(dead_code)]
    fn is_file(&self) -> bool {
        matches!(self, MemNode::File { .. })
    }

    fn modified(&self) -> u64 {
        match self {
            MemNode::File { modified, .. } => *modified,
            MemNode::Dir { modified, .. } => *modified,
        }
    }

    fn size(&self) -> u64 {
        match self {
            MemNode::File { content, .. } => content.len() as u64,
            MemNode::Dir { children, .. } => children.len() as u64,
        }
    }
}

/// Get current timestamp in seconds since Unix epoch
#[cfg(target_arch = "wasm32")]
pub(crate) fn now() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Normalize path: remove leading/trailing slashes, handle empty path
fn normalize_path(path: &str) -> &str {
    let path = path.trim_start_matches('/');
    let path = path.trim_end_matches('/');
    if path.is_empty() { "." } else { path }
}

/// Split path into components
fn path_components(path: &str) -> Vec<&str> {
    let normalized = normalize_path(path);
    if normalized == "." {
        return vec![];
    }
    normalized.split('/').filter(|s| !s.is_empty()).collect()
}

impl MemFs {
    /// Create a new empty in-memory filesystem
    pub fn new() -> Self {
        MemFs {
            root: Arc::new(RwLock::new(MemNode::new_dir())),
        }
    }

    /// Get a node at the given path (read lock)
    fn get_node(&self, path: &str) -> Result<MemNode, FsError> {
        let components = path_components(path);
        let root = self
            .root
            .read()
            .map_err(|_| FsError::Io("lock poisoned".into()))?;

        let mut current = root.clone();
        for component in components {
            match current {
                MemNode::Dir { children, .. } => {
                    current = children
                        .get(component)
                        .cloned()
                        .ok_or_else(|| FsError::NotFound(path.to_string()))?;
                }
                MemNode::File { .. } => {
                    return Err(FsError::NotADirectory(path.to_string()));
                }
            }
        }

        Ok(current)
    }

    /// Get mutable access to parent directory and return (parent_children, name)
    fn with_parent_mut<F, T>(&self, path: &str, f: F) -> Result<T, FsError>
    where
        F: FnOnce(&mut HashMap<String, MemNode>, &str) -> Result<T, FsError>,
    {
        let components = path_components(path);
        if components.is_empty() {
            return Err(FsError::PermissionDenied("cannot modify root".into()));
        }

        let name = components.last().unwrap().to_string();
        let parent_components = &components[..components.len() - 1];

        let mut root = self
            .root
            .write()
            .map_err(|_| FsError::Io("lock poisoned".into()))?;

        // Navigate to parent
        let mut current = &mut *root;
        for component in parent_components {
            match current {
                MemNode::Dir { children, .. } => {
                    current = children
                        .get_mut(*component)
                        .ok_or_else(|| FsError::NotFound(path.to_string()))?;
                }
                MemNode::File { .. } => {
                    return Err(FsError::NotADirectory(path.to_string()));
                }
            }
        }

        match current {
            MemNode::Dir { children, .. } => f(children, &name),
            MemNode::File { .. } => Err(FsError::NotADirectory(path.to_string())),
        }
    }
}

impl Default for MemFs {
    fn default() -> Self {
        Self::new()
    }
}

impl FileService for MemFs {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError> {
        let node = self.get_node(path);

        match node {
            Ok(MemNode::File { content, .. }) => {
                // File exists
                if flags.truncate && flags.write {
                    // Truncate the file
                    self.with_parent_mut(path, |children, name| {
                        if let Some(MemNode::File { content, modified }) = children.get_mut(name) {
                            content.clear();
                            *modified = now();
                        }
                        Ok(())
                    })?;
                    Ok(Box::new(MemFileHandle::new(
                        self.root.clone(),
                        path.to_string(),
                        Vec::new(),
                        flags,
                    )))
                } else {
                    Ok(Box::new(MemFileHandle::new(
                        self.root.clone(),
                        path.to_string(),
                        content,
                        flags,
                    )))
                }
            }
            Ok(MemNode::Dir { .. }) => Err(FsError::NotAFile(path.to_string())),
            Err(FsError::NotFound(_)) if flags.create => {
                // Create new file
                self.with_parent_mut(path, |children, name| {
                    children.insert(name.to_string(), MemNode::new_file());
                    Ok(())
                })?;
                Ok(Box::new(MemFileHandle::new(
                    self.root.clone(),
                    path.to_string(),
                    Vec::new(),
                    flags,
                )))
            }
            Err(e) => Err(e),
        }
    }

    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        let node = self.get_node(path)?;

        match node {
            MemNode::Dir { children, .. } => {
                let mut entries: Vec<DirEntry> = children
                    .iter()
                    .map(|(name, node)| DirEntry {
                        name: name.clone(),
                        is_dir: node.is_dir(),
                        size: node.size(),
                    })
                    .collect();
                entries.sort_by(|a, b| a.name.cmp(&b.name));
                Ok(entries)
            }
            MemNode::File { .. } => Err(FsError::NotADirectory(path.to_string())),
        }
    }

    fn stat(&self, path: &str) -> Result<Metadata, FsError> {
        let node = self.get_node(path)?;

        Ok(Metadata {
            is_dir: node.is_dir(),
            size: node.size(),
            modified: node.modified(),
            readonly: false,
        })
    }

    fn mkdir(&self, path: &str) -> Result<(), FsError> {
        // Check if already exists
        if self.get_node(path).is_ok() {
            return Err(FsError::AlreadyExists(path.to_string()));
        }

        self.with_parent_mut(path, |children, name| {
            children.insert(name.to_string(), MemNode::new_dir());
            Ok(())
        })
    }

    fn remove(&self, path: &str) -> Result<(), FsError> {
        // Check that it exists
        self.get_node(path)?;

        self.with_parent_mut(path, |children, name| {
            children
                .remove(name)
                .ok_or_else(|| FsError::NotFound(path.to_string()))?;
            Ok(())
        })
    }

    fn rename(&self, from: &str, to: &str) -> Result<(), FsError> {
        // Get the node to move
        let node = self.get_node(from)?;

        // Remove from source
        self.with_parent_mut(from, |children, name| {
            children.remove(name);
            Ok(())
        })?;

        // Add to destination
        self.with_parent_mut(to, |children, name| {
            children.insert(name.to_string(), node.clone());
            Ok(())
        })
    }
}

/// Handle to an open file in MemFs
struct MemFileHandle {
    root: Arc<RwLock<MemNode>>,
    path: String,
    buffer: Vec<u8>,
    position: u64,
    flags: OpenFlags,
    dirty: bool,
}

impl MemFileHandle {
    fn new(root: Arc<RwLock<MemNode>>, path: String, content: Vec<u8>, flags: OpenFlags) -> Self {
        let position = if flags.append {
            content.len() as u64
        } else {
            0
        };
        MemFileHandle {
            root,
            path,
            buffer: content,
            position,
            flags,
            dirty: false,
        }
    }

    fn write_back(&self) -> Result<(), FsError> {
        if !self.dirty {
            return Ok(());
        }

        let components = path_components(&self.path);
        if components.is_empty() {
            return Err(FsError::PermissionDenied("cannot modify root".into()));
        }

        let name = components.last().unwrap().to_string();
        let parent_components = &components[..components.len() - 1];

        let mut root = self
            .root
            .write()
            .map_err(|_| FsError::Io("lock poisoned".into()))?;

        // Navigate to parent
        let mut current = &mut *root;
        for component in parent_components {
            match current {
                MemNode::Dir { children, .. } => {
                    current = children
                        .get_mut(*component)
                        .ok_or_else(|| FsError::NotFound(self.path.clone()))?;
                }
                MemNode::File { .. } => {
                    return Err(FsError::NotADirectory(self.path.clone()));
                }
            }
        }

        match current {
            MemNode::Dir { children, .. } => {
                if let Some(MemNode::File { content, modified }) = children.get_mut(&name) {
                    *content = self.buffer.clone();
                    *modified = now();
                }
                Ok(())
            }
            MemNode::File { .. } => Err(FsError::NotADirectory(self.path.clone())),
        }
    }
}

impl FileHandle for MemFileHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FsError> {
        if !self.flags.read {
            return Err(FsError::PermissionDenied(
                "file not opened for reading".into(),
            ));
        }

        let pos = self.position as usize;
        if pos >= self.buffer.len() {
            return Ok(0);
        }

        let available = &self.buffer[pos..];
        let to_read = buf.len().min(available.len());
        buf[..to_read].copy_from_slice(&available[..to_read]);
        self.position += to_read as u64;
        Ok(to_read)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, FsError> {
        if !self.flags.write {
            return Err(FsError::PermissionDenied(
                "file not opened for writing".into(),
            ));
        }

        let pos = self.position as usize;

        // Extend buffer if needed
        if pos + buf.len() > self.buffer.len() {
            self.buffer.resize(pos + buf.len(), 0);
        }

        self.buffer[pos..pos + buf.len()].copy_from_slice(buf);
        self.position += buf.len() as u64;
        self.dirty = true;
        Ok(buf.len())
    }

    fn seek(&mut self, pos: u64) -> Result<(), FsError> {
        self.position = pos;
        Ok(())
    }

    fn position(&self) -> u64 {
        self.position
    }

    fn flush(&mut self) -> Result<(), FsError> {
        self.write_back()
    }
}

impl Drop for MemFileHandle {
    fn drop(&mut self) {
        // Write back on drop (ignore errors)
        let _ = self.write_back();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_read_file() {
        let fs = MemFs::new();

        // Create and write
        {
            let mut file = fs
                .open(
                    "/test.txt",
                    OpenFlags {
                        write: true,
                        create: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            file.write(b"Hello, World!").unwrap();
            file.flush().unwrap();
        }

        // Read back
        {
            let mut file = fs.open("/test.txt", OpenFlags::read_only()).unwrap();
            let mut buf = [0u8; 32];
            let n = file.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], b"Hello, World!");
        }
    }

    #[test]
    fn test_mkdir_and_nested_files() {
        let fs = MemFs::new();

        fs.mkdir("/docs").unwrap();
        fs.mkdir("/docs/notes").unwrap();

        // Create file in nested dir
        {
            let mut file = fs
                .open(
                    "/docs/notes/readme.txt",
                    OpenFlags {
                        write: true,
                        create: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            file.write(b"Nested content").unwrap();
        }

        // Verify structure
        let entries = fs.readdir("/docs").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "notes");
        assert!(entries[0].is_dir);

        let entries = fs.readdir("/docs/notes").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "readme.txt");
        assert!(!entries[0].is_dir);
    }

    #[test]
    fn test_stat() {
        let fs = MemFs::new();

        fs.mkdir("/folder").unwrap();
        {
            let mut file = fs
                .open(
                    "/file.txt",
                    OpenFlags {
                        write: true,
                        create: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            file.write(b"12345").unwrap();
        }

        let folder_meta = fs.stat("/folder").unwrap();
        assert!(folder_meta.is_dir);

        let file_meta = fs.stat("/file.txt").unwrap();
        assert!(!file_meta.is_dir);
        assert_eq!(file_meta.size, 5);
    }

    #[test]
    fn test_remove() {
        let fs = MemFs::new();

        {
            let mut file = fs
                .open(
                    "/to_delete.txt",
                    OpenFlags {
                        write: true,
                        create: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            file.write(b"delete me").unwrap();
        }

        assert!(fs.stat("/to_delete.txt").is_ok());
        fs.remove("/to_delete.txt").unwrap();
        assert!(fs.stat("/to_delete.txt").is_err());
    }

    #[test]
    fn test_rename() {
        let fs = MemFs::new();

        {
            let mut file = fs
                .open(
                    "/old.txt",
                    OpenFlags {
                        write: true,
                        create: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            file.write(b"content").unwrap();
        }

        fs.rename("/old.txt", "/new.txt").unwrap();

        assert!(fs.stat("/old.txt").is_err());

        let mut file = fs.open("/new.txt", OpenFlags::read_only()).unwrap();
        let mut buf = [0u8; 32];
        let n = file.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"content");
    }

    #[test]
    fn test_readdir_root() {
        let fs = MemFs::new();

        fs.mkdir("/a").unwrap();
        fs.mkdir("/b").unwrap();
        {
            let mut file = fs
                .open(
                    "/c.txt",
                    OpenFlags {
                        write: true,
                        create: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            file.write(b"c").unwrap();
        }

        let entries = fs.readdir("/").unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].name, "a");
        assert_eq!(entries[1].name, "b");
        assert_eq!(entries[2].name, "c.txt");
    }

    #[test]
    fn test_truncate() {
        let fs = MemFs::new();

        // Write initial content
        {
            let mut file = fs
                .open(
                    "/file.txt",
                    OpenFlags {
                        write: true,
                        create: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            file.write(b"original content").unwrap();
        }

        // Truncate and write new content
        {
            let mut file = fs
                .open(
                    "/file.txt",
                    OpenFlags {
                        write: true,
                        truncate: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            file.write(b"new").unwrap();
        }

        // Verify
        {
            let mut file = fs.open("/file.txt", OpenFlags::read_only()).unwrap();
            let mut buf = [0u8; 32];
            let n = file.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], b"new");
        }
    }
}
