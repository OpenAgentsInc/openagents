//! Static/immutable filesystem implementation
//!
//! A read-only filesystem built from static data at construction time.
//! Perfect for bundled assets, embedded documentation, or read-only task specs.

use std::collections::HashMap;

use crate::error::FsError;
use crate::service::{DirEntry, FileHandle, FileService, Metadata, OpenFlags};

/// Static/immutable filesystem
///
/// Once constructed, the filesystem cannot be modified. All write operations
/// return errors. This makes it perfect for:
/// - Bundled assets
/// - Read-only task specifications
/// - Embedded documentation
/// - Snapshot baselines for CowFs
///
/// # Example
///
/// ```rust
/// use oanix::services::MapFs;
/// use oanix::service::{FileService, OpenFlags};
///
/// let fs = MapFs::builder()
///     .file("/readme.txt", b"Hello!")
///     .dir("/src")
///     .file("/src/main.rs", b"fn main() {}")
///     .build();
///
/// let meta = fs.stat("/readme.txt").unwrap();
/// assert_eq!(meta.size, 6);
/// ```
pub struct MapFs {
    root: MapNode,
}

#[derive(Debug, Clone)]
enum MapNode {
    File { content: Vec<u8> },
    Dir { children: HashMap<String, MapNode> },
}

impl MapNode {
    fn is_dir(&self) -> bool {
        matches!(self, MapNode::Dir { .. })
    }

    fn size(&self) -> u64 {
        match self {
            MapNode::File { content } => content.len() as u64,
            MapNode::Dir { children } => children.len() as u64,
        }
    }
}

/// Builder for MapFs
pub struct MapFsBuilder {
    root: MapNode,
}

impl MapFsBuilder {
    fn new() -> Self {
        MapFsBuilder {
            root: MapNode::Dir {
                children: HashMap::new(),
            },
        }
    }

    /// Add a file at the given path
    ///
    /// Parent directories are created automatically.
    pub fn file(mut self, path: &str, content: impl Into<Vec<u8>>) -> Self {
        let content = content.into();
        let components = normalize_and_split(path);

        if components.is_empty() {
            return self;
        }

        let name = components.last().unwrap().to_string();
        let parent_path = &components[..components.len() - 1];

        // Ensure parent directories exist
        self.ensure_parents(parent_path);

        // Navigate to parent and insert file
        let parent = self.navigate_mut(parent_path);
        if let MapNode::Dir { children } = parent {
            children.insert(name, MapNode::File { content });
        }

        self
    }

    /// Add an empty directory at the given path
    ///
    /// Parent directories are created automatically.
    pub fn dir(mut self, path: &str) -> Self {
        let components = normalize_and_split(path);

        if components.is_empty() {
            return self;
        }

        // Ensure all directories exist (including the target)
        self.ensure_parents(&components);

        self
    }

    /// Build the immutable MapFs
    pub fn build(self) -> MapFs {
        MapFs { root: self.root }
    }

    fn ensure_parents(&mut self, components: &[&str]) {
        let mut current = &mut self.root;

        for component in components {
            if let MapNode::Dir { children } = current {
                if !children.contains_key(*component) {
                    children.insert(
                        component.to_string(),
                        MapNode::Dir {
                            children: HashMap::new(),
                        },
                    );
                }
                current = children.get_mut(*component).unwrap();
            }
        }
    }

    fn navigate_mut(&mut self, components: &[&str]) -> &mut MapNode {
        let mut current = &mut self.root;

        for component in components {
            if let MapNode::Dir { children } = current {
                current = children.get_mut(*component).unwrap();
            }
        }

        current
    }
}

impl MapFs {
    /// Create a new builder for MapFs
    pub fn builder() -> MapFsBuilder {
        MapFsBuilder::new()
    }

    /// Get a node at the given path
    fn get_node(&self, path: &str) -> Result<&MapNode, FsError> {
        let components = normalize_and_split(path);

        let mut current = &self.root;
        for component in components {
            match current {
                MapNode::Dir { children } => {
                    current = children
                        .get(component)
                        .ok_or_else(|| FsError::NotFound(path.to_string()))?;
                }
                MapNode::File { .. } => {
                    return Err(FsError::NotADirectory(path.to_string()));
                }
            }
        }

        Ok(current)
    }
}

impl FileService for MapFs {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError> {
        // Reject any write operations
        if flags.write || flags.create || flags.truncate || flags.append {
            return Err(FsError::ReadOnly);
        }

        let node = self.get_node(path)?;

        match node {
            MapNode::File { content } => Ok(Box::new(MapFileHandle {
                content: content.clone(),
                position: 0,
            })),
            MapNode::Dir { .. } => Err(FsError::NotAFile(path.to_string())),
        }
    }

    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        let node = self.get_node(path)?;

        match node {
            MapNode::Dir { children } => {
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
            MapNode::File { .. } => Err(FsError::NotADirectory(path.to_string())),
        }
    }

    fn stat(&self, path: &str) -> Result<Metadata, FsError> {
        let node = self.get_node(path)?;

        Ok(Metadata {
            is_dir: node.is_dir(),
            size: node.size(),
            modified: 0, // Static filesystem has no modification time
            readonly: true,
        })
    }

    fn mkdir(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::ReadOnly)
    }

    fn remove(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::ReadOnly)
    }

    fn rename(&self, _from: &str, _to: &str) -> Result<(), FsError> {
        Err(FsError::ReadOnly)
    }
}

/// Handle to an open file in MapFs (read-only)
struct MapFileHandle {
    content: Vec<u8>,
    position: usize,
}

impl FileHandle for MapFileHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FsError> {
        if self.position >= self.content.len() {
            return Ok(0);
        }

        let available = &self.content[self.position..];
        let to_read = buf.len().min(available.len());
        buf[..to_read].copy_from_slice(&available[..to_read]);
        self.position += to_read;
        Ok(to_read)
    }

    fn write(&mut self, _buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::ReadOnly)
    }

    fn seek(&mut self, pos: u64) -> Result<(), FsError> {
        self.position = pos as usize;
        Ok(())
    }

    fn position(&self) -> u64 {
        self.position as u64
    }

    fn flush(&mut self) -> Result<(), FsError> {
        Ok(())
    }
}

/// Normalize path and split into components
fn normalize_and_split(path: &str) -> Vec<&str> {
    let path = path.trim_start_matches('/');
    let path = path.trim_end_matches('/');
    if path.is_empty() {
        return vec![];
    }
    path.split('/').filter(|s| !s.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_and_read() {
        let fs = MapFs::builder()
            .file("/hello.txt", b"Hello, World!")
            .build();

        let mut handle = fs.open("/hello.txt", OpenFlags::read_only()).unwrap();
        let mut buf = [0u8; 32];
        let n = handle.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"Hello, World!");
    }

    #[test]
    fn test_nested_structure() {
        let fs = MapFs::builder()
            .dir("/docs")
            .file("/docs/readme.md", b"# README")
            .file("/docs/notes/todo.txt", b"TODO list")
            .file("/src/main.rs", b"fn main() {}")
            .build();

        // Check structure
        let root_entries = fs.readdir("/").unwrap();
        assert_eq!(root_entries.len(), 2);

        let docs_entries = fs.readdir("/docs").unwrap();
        assert_eq!(docs_entries.len(), 2); // readme.md and notes/

        // Read nested file
        let mut handle = fs
            .open("/docs/notes/todo.txt", OpenFlags::read_only())
            .unwrap();
        let mut buf = [0u8; 32];
        let n = handle.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"TODO list");
    }

    #[test]
    fn test_write_fails() {
        let fs = MapFs::builder().file("/test.txt", b"content").build();

        // Try to open for writing
        let result = fs.open("/test.txt", OpenFlags::write_only());
        assert!(result.is_err());

        // Try to create
        let result = fs.open(
            "/new.txt",
            OpenFlags {
                write: true,
                create: true,
                ..Default::default()
            },
        );
        assert!(result.is_err());

        // Try mkdir
        assert!(fs.mkdir("/newdir").is_err());

        // Try remove
        assert!(fs.remove("/test.txt").is_err());
    }

    #[test]
    fn test_stat() {
        let fs = MapFs::builder()
            .file("/file.txt", b"12345")
            .dir("/folder")
            .build();

        let file_meta = fs.stat("/file.txt").unwrap();
        assert!(!file_meta.is_dir);
        assert_eq!(file_meta.size, 5);
        assert!(file_meta.readonly);

        let dir_meta = fs.stat("/folder").unwrap();
        assert!(dir_meta.is_dir);
        assert!(dir_meta.readonly);
    }

    #[test]
    fn test_seek() {
        let fs = MapFs::builder().file("/test.txt", b"0123456789").build();

        let mut handle = fs.open("/test.txt", OpenFlags::read_only()).unwrap();

        // Seek to middle
        handle.seek(5).unwrap();
        let mut buf = [0u8; 5];
        let n = handle.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"56789");

        // Seek back
        handle.seek(0).unwrap();
        let n = handle.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"01234");
    }
}
