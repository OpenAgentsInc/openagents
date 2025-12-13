//! Namespace and mount abstractions
//!
//! Inspired by Plan 9's per-process namespaces, OANIX namespaces define
//! the "world" an agent sees by composing mounted FileService instances.

use std::sync::Arc;

use crate::service::FileService;

/// A mount point in a namespace
pub struct Mount {
    /// Path prefix (e.g., "/task", "/workspace")
    pub path: String,
    /// The service handling this mount
    pub service: Arc<dyn FileService>,
}

/// A namespace is a collection of mounts that define an environment's view
///
/// Namespaces are immutable after construction for thread-safety.
#[derive(Clone)]
pub struct Namespace {
    mounts: Arc<Vec<Mount>>,
}

impl Namespace {
    /// Create a new namespace builder
    pub fn builder() -> NamespaceBuilder {
        NamespaceBuilder { mounts: Vec::new() }
    }

    /// Resolve a path to its service and remaining path
    ///
    /// Uses longest-prefix matching to find the appropriate mount.
    pub fn resolve<'a>(&'a self, full_path: &'a str) -> Option<(&'a dyn FileService, &'a str)> {
        let mut best_match: Option<(&Mount, &str)> = None;
        let mut best_len = 0;

        for mount in self.mounts.iter() {
            if full_path.starts_with(&mount.path) {
                let mount_len = mount.path.len();
                if mount_len > best_len {
                    // Get the remainder after the mount path
                    let remainder = if full_path.len() > mount_len {
                        &full_path[mount_len..]
                    } else {
                        "/"
                    };
                    best_match = Some((mount, remainder));
                    best_len = mount_len;
                }
            }
        }

        best_match.map(|(mount, remainder)| (mount.service.as_ref(), remainder))
    }

    /// Get all mount points
    pub fn mounts(&self) -> &[Mount] {
        &self.mounts
    }
}

/// Builder for constructing namespaces
pub struct NamespaceBuilder {
    mounts: Vec<Mount>,
}

impl NamespaceBuilder {
    /// Mount a service at the given path
    pub fn mount<S: FileService + 'static>(mut self, path: &str, service: S) -> Self {
        self.mounts.push(Mount {
            path: path.to_string(),
            service: Arc::new(service),
        });
        self
    }

    /// Build the namespace
    pub fn build(self) -> Namespace {
        Namespace {
            mounts: Arc::new(self.mounts),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::FsError;
    use crate::service::{DirEntry, FileHandle, Metadata, OpenFlags};

    struct DummyFs;

    impl FileService for DummyFs {
        fn open(&self, _path: &str, _flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError> {
            Err(FsError::NotFound("dummy".into()))
        }

        fn readdir(&self, _path: &str) -> Result<Vec<DirEntry>, FsError> {
            Ok(vec![])
        }

        fn stat(&self, _path: &str) -> Result<Metadata, FsError> {
            Err(FsError::NotFound("dummy".into()))
        }
    }

    #[test]
    fn test_namespace_resolution() {
        let ns = Namespace::builder()
            .mount("/task", DummyFs)
            .mount("/workspace", DummyFs)
            .build();

        assert!(ns.resolve("/task/spec.json").is_some());
        assert!(ns.resolve("/workspace/src/main.rs").is_some());
        assert!(ns.resolve("/unknown/path").is_none());
    }
}
