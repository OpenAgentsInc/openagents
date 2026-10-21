//! The workspace filesystem: an in-memory tree of directories and files
//! keyed by normalized relative paths, with a compact snapshot format the
//! page persists to OPFS and hands to each tool worker.
//!
//! The root is the empty path. Paths never leave the root: `..` past it is
//! refused, so a tool can't name anything outside the workspace.

use std::collections::BTreeMap;

/// One node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Dir { mtime: u64 },
    File { data: Vec<u8>, mtime: u64 },
}

impl Node {
    #[must_use]
    pub fn is_dir(&self) -> bool {
        matches!(self, Self::Dir { .. })
    }

    #[must_use]
    pub fn mtime(&self) -> u64 {
        match self {
            Self::Dir { mtime } | Self::File { mtime, .. } => *mtime,
        }
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        match self {
            Self::Dir { .. } => 0,
            Self::File { data, .. } => data.len() as u64,
        }
    }
}

/// Why a filesystem call failed, as WASI names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    NotFound,
    Exists,
    NotDir,
    IsDir,
    NotEmpty,
    Invalid,
}

/// The workspace tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vfs {
    nodes: BTreeMap<String, Node>,
}

impl Default for Vfs {
    fn default() -> Self {
        let mut nodes = BTreeMap::new();
        nodes.insert(String::new(), Node::Dir { mtime: 0 });
        Self { nodes }
    }
}

/// Join `path` onto `base` and normalize it, refusing to climb above the
/// root. A leading `/` means the root.
///
/// # Errors
///
/// [`FsError::Invalid`] for a path that climbs past the root or holds a NUL.
pub fn resolve(base: &str, path: &str) -> Result<String, FsError> {
    if path.contains('\0') {
        return Err(FsError::Invalid);
    }
    let mut parts: Vec<&str> = if path.starts_with('/') {
        Vec::new()
    } else {
        base.split('/').filter(|p| !p.is_empty()).collect()
    };
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop().ok_or(FsError::Invalid)?;
            }
            name => parts.push(name),
        }
    }
    Ok(parts.join("/"))
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(p, _)| p)
}

impl Vfs {
    /// The node at a normalized path.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<&Node> {
        self.nodes.get(path)
    }

    /// Every node, in path order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Node)> {
        self.nodes.iter()
    }

    /// The file count and total bytes.
    #[must_use]
    pub fn usage(&self) -> (usize, u64) {
        self.nodes
            .values()
            .filter(|n| !n.is_dir())
            .fold((0, 0), |(n, b), node| (n + 1, b + node.size()))
    }

    /// The names directly inside directory `path`, sorted.
    ///
    /// # Errors
    ///
    /// When `path` is missing or a file.
    pub fn list(&self, path: &str) -> Result<Vec<(String, bool)>, FsError> {
        match self.nodes.get(path) {
            None => return Err(FsError::NotFound),
            Some(node) if !node.is_dir() => return Err(FsError::NotDir),
            Some(_) => {}
        }
        let prefix = if path.is_empty() {
            String::new()
        } else {
            format!("{path}/")
        };
        Ok(self
            .nodes
            .range(prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&prefix))
            .filter(|(k, _)| !k.is_empty() && !k[prefix.len()..].contains('/'))
            .map(|(k, n)| (k[prefix.len()..].to_string(), n.is_dir()))
            .collect())
    }

    /// Read a file.
    ///
    /// # Errors
    ///
    /// When `path` is missing or a directory.
    pub fn read(&self, path: &str) -> Result<&[u8], FsError> {
        match self.nodes.get(path) {
            Some(Node::File { data, .. }) => Ok(data),
            Some(Node::Dir { .. }) => Err(FsError::IsDir),
            None => Err(FsError::NotFound),
        }
    }

    fn parent_is_dir(&self, path: &str) -> Result<(), FsError> {
        match self.nodes.get(parent(path)) {
            Some(node) if node.is_dir() => Ok(()),
            Some(_) => Err(FsError::NotDir),
            None => Err(FsError::NotFound),
        }
    }

    /// Create an empty file, or with `truncate` empty an existing one.
    ///
    /// # Errors
    ///
    /// When the parent is missing, or `path` is a directory.
    pub fn create(&mut self, path: &str, truncate: bool, mtime: u64) -> Result<(), FsError> {
        if path.is_empty() {
            return Err(FsError::IsDir);
        }
        self.parent_is_dir(path)?;
        match self.nodes.get_mut(path) {
            Some(Node::Dir { .. }) => Err(FsError::IsDir),
            Some(Node::File { data, mtime: m }) => {
                if truncate {
                    data.clear();
                    *m = mtime;
                }
                Ok(())
            }
            None => {
                self.nodes.insert(
                    path.to_string(),
                    Node::File {
                        data: Vec::new(),
                        mtime,
                    },
                );
                Ok(())
            }
        }
    }

    /// Write `bytes` at `offset` in an existing file, growing it.
    ///
    /// # Errors
    ///
    /// When `path` is missing or a directory.
    pub fn write_at(
        &mut self,
        path: &str,
        offset: u64,
        bytes: &[u8],
        mtime: u64,
    ) -> Result<(), FsError> {
        match self.nodes.get_mut(path) {
            Some(Node::File { data, mtime: m }) => {
                let offset = usize::try_from(offset).map_err(|_| FsError::Invalid)?;
                let end = offset + bytes.len();
                if data.len() < end {
                    data.resize(end, 0);
                }
                data[offset..end].copy_from_slice(bytes);
                *m = mtime;
                Ok(())
            }
            Some(Node::Dir { .. }) => Err(FsError::IsDir),
            None => Err(FsError::NotFound),
        }
    }

    /// Set a file's length.
    ///
    /// # Errors
    ///
    /// When `path` is missing or a directory.
    pub fn truncate(&mut self, path: &str, len: u64) -> Result<(), FsError> {
        match self.nodes.get_mut(path) {
            Some(Node::File { data, .. }) => {
                data.resize(usize::try_from(len).map_err(|_| FsError::Invalid)?, 0);
                Ok(())
            }
            Some(Node::Dir { .. }) => Err(FsError::IsDir),
            None => Err(FsError::NotFound),
        }
    }

    /// Write a whole file, creating its parent directories.
    ///
    /// # Errors
    ///
    /// When a parent is a file or `path` is a directory.
    pub fn put(&mut self, path: &str, data: &[u8], mtime: u64) -> Result<(), FsError> {
        let mut at = String::new();
        let parent = parent(path);
        if !parent.is_empty() {
            for part in parent.split('/') {
                if !at.is_empty() {
                    at.push('/');
                }
                at.push_str(part);
                match self.nodes.get(&at) {
                    Some(node) if !node.is_dir() => return Err(FsError::NotDir),
                    Some(_) => {}
                    None => {
                        self.nodes.insert(at.clone(), Node::Dir { mtime });
                    }
                }
            }
        }
        if self.nodes.get(path).is_some_and(Node::is_dir) {
            return Err(FsError::IsDir);
        }
        self.nodes.insert(
            path.to_string(),
            Node::File {
                data: data.to_vec(),
                mtime,
            },
        );
        Ok(())
    }

    /// Make one directory.
    ///
    /// # Errors
    ///
    /// When it exists or its parent is missing.
    pub fn mkdir(&mut self, path: &str, mtime: u64) -> Result<(), FsError> {
        if self.nodes.contains_key(path) {
            return Err(FsError::Exists);
        }
        self.parent_is_dir(path)?;
        self.nodes.insert(path.to_string(), Node::Dir { mtime });
        Ok(())
    }

    /// Remove a file.
    ///
    /// # Errors
    ///
    /// When `path` is missing or a directory.
    pub fn unlink(&mut self, path: &str) -> Result<(), FsError> {
        match self.nodes.get(path) {
            Some(Node::File { .. }) => {
                self.nodes.remove(path);
                Ok(())
            }
            Some(Node::Dir { .. }) => Err(FsError::IsDir),
            None => Err(FsError::NotFound),
        }
    }

    /// Remove an empty directory.
    ///
    /// # Errors
    ///
    /// When it is missing, a file, the root, or not empty.
    pub fn rmdir(&mut self, path: &str) -> Result<(), FsError> {
        if path.is_empty() {
            return Err(FsError::Invalid);
        }
        if !self.list(path)?.is_empty() {
            return Err(FsError::NotEmpty);
        }
        self.nodes.remove(path);
        Ok(())
    }

    /// Rename a file or directory, with everything under it.
    ///
    /// # Errors
    ///
    /// When the source is missing or the target's parent is.
    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), FsError> {
        if from.is_empty() || to.is_empty() || to.starts_with(&format!("{from}/")) {
            return Err(FsError::Invalid);
        }
        if !self.nodes.contains_key(from) {
            return Err(FsError::NotFound);
        }
        self.parent_is_dir(to)?;
        let prefix = format!("{from}/");
        let moved: Vec<String> = self
            .nodes
            .keys()
            .filter(|k| *k == from || k.starts_with(&prefix))
            .cloned()
            .collect();
        for key in moved {
            if let Some(node) = self.nodes.remove(&key) {
                let rest = &key[from.len()..];
                self.nodes.insert(format!("{to}{rest}"), node);
            }
        }
        Ok(())
    }

    /// The paths whose content differs between `self` and `next`: changed
    /// or added in `next`, then removed.
    #[must_use]
    pub fn diff(&self, next: &Self) -> (Vec<String>, Vec<String>) {
        let changed = next
            .nodes
            .iter()
            .filter(|(k, n)| self.nodes.get(*k) != Some(n))
            .map(|(k, _)| k.clone())
            .collect();
        let removed = self
            .nodes
            .keys()
            .filter(|k| !next.nodes.contains_key(*k))
            .cloned()
            .collect();
        (changed, removed)
    }

    /// Encode the tree: `KILNFS1\n`, then per node a kind byte (`d` or
    /// `f`), the path length (u32 LE), the path, the mtime (u64 LE), and
    /// for a file the data length (u32 LE) and the data.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = b"KILNFS1\n".to_vec();
        for (path, node) in &self.nodes {
            out.push(if node.is_dir() { b'd' } else { b'f' });
            out.extend_from_slice(&(path.len() as u32).to_le_bytes());
            out.extend_from_slice(path.as_bytes());
            out.extend_from_slice(&node.mtime().to_le_bytes());
            if let Node::File { data, .. } = node {
                out.extend_from_slice(&(data.len() as u32).to_le_bytes());
                out.extend_from_slice(data);
            }
        }
        out
    }

    /// Decode [`Vfs::encode`]'s format.
    ///
    /// # Errors
    ///
    /// When the bytes are not a snapshot.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut rest = bytes
            .strip_prefix(b"KILNFS1\n")
            .ok_or("not a Kiln snapshot")?;
        let mut take = |n: usize| -> Result<&[u8], String> {
            if rest.len() < n {
                return Err("the snapshot is truncated".into());
            }
            let (head, tail) = rest.split_at(n);
            rest = tail;
            Ok(head)
        };
        let mut vfs = Self::default();
        loop {
            let Ok(kind) = take(1) else { break };
            let kind = kind[0];
            let len = u32::from_le_bytes(take(4)?.try_into().map_err(|_| "length")?) as usize;
            let path = String::from_utf8(take(len)?.to_vec()).map_err(|_| "path is not UTF-8")?;
            let mtime = u64::from_le_bytes(take(8)?.try_into().map_err(|_| "mtime")?);
            let node = match kind {
                b'd' => Node::Dir { mtime },
                b'f' => {
                    let len =
                        u32::from_le_bytes(take(4)?.try_into().map_err(|_| "length")?) as usize;
                    Node::File {
                        data: take(len)?.to_vec(),
                        mtime,
                    }
                }
                _ => return Err("unknown node kind".into()),
            };
            if resolve("", &path)? != path {
                return Err(format!("snapshot path {path:?} is not normalized"));
            }
            vfs.nodes.insert(path, node);
        }
        vfs.nodes.insert(String::new(), Node::Dir { mtime: 0 });
        Ok(vfs)
    }
}

impl From<FsError> for String {
    fn from(error: FsError) -> Self {
        format!("{error:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_inside_the_root() {
        assert_eq!(resolve("src", "../a/./b").unwrap(), "a/b");
        assert_eq!(resolve("src", "/x").unwrap(), "x");
        assert_eq!(resolve("", "..").unwrap_err(), FsError::Invalid);
        assert_eq!(resolve("a", "../../etc").unwrap_err(), FsError::Invalid);
    }

    #[test]
    fn files_and_directories_behave() {
        let mut fs = Vfs::default();
        assert_eq!(fs.create("a/b", false, 1), Err(FsError::NotFound));
        fs.put("a/b.txt", b"hi", 1).unwrap();
        fs.write_at("a/b.txt", 4, b"!", 2).unwrap();
        assert_eq!(fs.read("a/b.txt").unwrap(), b"hi\0\0!");
        assert_eq!(fs.list("").unwrap(), vec![("a".to_string(), true)]);
        assert_eq!(fs.list("a").unwrap(), vec![("b.txt".to_string(), false)]);
        assert_eq!(fs.rmdir("a"), Err(FsError::NotEmpty));
        fs.rename("a", "c").unwrap();
        assert_eq!(fs.read("c/b.txt").unwrap().len(), 5);
        fs.unlink("c/b.txt").unwrap();
        fs.rmdir("c").unwrap();
        assert_eq!(fs.usage(), (0, 0));
    }

    #[test]
    fn a_snapshot_round_trips_and_diffs() {
        let mut fs = Vfs::default();
        fs.put("src/fib.rhai", b"fn fib(n) { n }", 5).unwrap();
        let back = Vfs::decode(&fs.encode()).unwrap();
        assert_eq!(back, fs);
        let mut next = back.clone();
        next.put("src/fib.rhai", b"changed", 6).unwrap();
        next.put("README", b"x", 6).unwrap();
        let (changed, removed) = fs.diff(&next);
        assert_eq!(changed, vec!["README", "src/fib.rhai"]);
        assert!(removed.is_empty());
        assert!(Vfs::decode(b"KILNFS1\nf\x05\0\0\0../ab").is_err());
    }
}
