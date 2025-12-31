//! Namespace mount table and resolution.

use crate::fs::{AccessLevel, FileService};
use std::sync::Arc;

/// Agent's view of mounted capabilities.
pub struct Namespace {
    mounts: Vec<Mount>,
}

struct Mount {
    path: String,
    service: Arc<dyn FileService>,
    access: AccessLevel,
}

impl Namespace {
    /// Create an empty namespace.
    pub fn new() -> Self {
        Self { mounts: Vec::new() }
    }

    /// Mount a service at a path.
    pub fn mount(&mut self, path: &str, service: Arc<dyn FileService>, access: AccessLevel) {
        self.mounts.push(Mount {
            path: path.to_string(),
            service,
            access,
        });
        self.mounts
            .sort_by(|a, b| b.path.len().cmp(&a.path.len()));
    }

    /// Unmount a path.
    pub fn unmount(&mut self, path: &str) {
        self.mounts.retain(|m| m.path != path);
    }

    /// List all mount points.
    pub fn mounts(&self) -> Vec<&str> {
        self.mounts.iter().map(|m| m.path.as_str()).collect()
    }

    /// Resolve a path to its service and relative path.
    pub fn resolve<'a>(&self, path: &'a str) -> Option<(Arc<dyn FileService>, &'a str, AccessLevel)> {
        for mount in &self.mounts {
            if path == mount.path || path.starts_with(&format!("{}/", mount.path)) {
                let mut relative = &path[mount.path.len()..];
                if relative.starts_with('/') {
                    relative = &relative[1..];
                }
                return Some((mount.service.clone(), relative, mount.access.clone()));
            }
        }
        None
    }
}

impl Default for Namespace {
    fn default() -> Self {
        Self::new()
    }
}
