//! A key-routed filesystem spanning several disks.

use bytes::Bytes;
use futures_util::future::join_all;

use crate::client::{Archil, ExecMount, ExecMountSpec, ExecOptions};
use crate::disk::{Disk, GrepOptions};
use crate::error::{Error, Result};
use crate::filesystem::FileSystem;
use crate::paths::to_segments;
use crate::s3::{
    ListObjectsOptions, ListObjectsResult, PutObjectOptions, PutObjectResult, S3Object,
};
use crate::types::{ExecResult, GrepMatch, GrepResult, GrepStoppedReason};

/// A disk to mount in a [`Workspace`], with optional mount options.
///
/// Unlike [`Archil::exec`], a workspace needs full [`Disk`] objects (it
/// routes object calls through them); bare disk ids are not accepted.
#[derive(Debug, Clone)]
pub struct WorkspaceMount {
    pub disk: Disk,
    pub read_only: bool,
    pub subdirectory: Option<String>,
    pub conditional: bool,
    pub queue_ms: Option<u64>,
    pub checkout_paths: Option<Vec<String>>,
}

impl WorkspaceMount {
    pub fn new(disk: Disk) -> Self {
        Self {
            disk,
            read_only: false,
            subdirectory: None,
            conditional: false,
            queue_ms: None,
            checkout_paths: None,
        }
    }
    pub fn read_only(mut self, v: bool) -> Self {
        self.read_only = v;
        self
    }
    pub fn subdirectory(mut self, s: impl Into<String>) -> Self {
        self.subdirectory = Some(s.into());
        self
    }
    pub fn conditional(mut self, v: bool) -> Self {
        self.conditional = v;
        self
    }
    pub fn queue_ms(mut self, v: u64) -> Self {
        self.queue_ms = Some(v);
        self
    }
    pub fn checkout_paths<I, S: Into<String>>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
    {
        self.checkout_paths = Some(paths.into_iter().map(Into::into).collect());
        self
    }
}

impl From<Disk> for WorkspaceMount {
    fn from(d: Disk) -> Self {
        Self::new(d)
    }
}
impl From<&Disk> for WorkspaceMount {
    fn from(d: &Disk) -> Self {
        Self::new(d.clone())
    }
}

/// A key-routed filesystem spanning several Archil disks.
///
/// Each disk is mounted under a name and addressed as the first segment of a
/// key (`<name>/...`); `get_object`, `put_object`, `delete_object`,
/// `list_objects` and `grep` route to the right disk by that segment, and
/// `list_objects` / `grep` fan out across all of them when the key or prefix
/// doesn't name one.
#[derive(Debug, Clone)]
pub struct Workspace {
    client: Archil,
    mounts: Vec<(String, WorkspaceMount)>,
}

fn trim_slashes(s: &str) -> &str {
    s.trim_matches('/')
}

impl Workspace {
    pub(crate) fn new<I, K, V>(client: Archil, mounts: I) -> Result<Self>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<WorkspaceMount>,
    {
        let mut ws = Self {
            client,
            mounts: Vec::new(),
        };
        for (name, mount) in mounts {
            ws.add_disk(name, mount)?;
        }
        if ws.mounts.is_empty() {
            return Err(Error::InvalidArgument(
                "workspace() needs at least one disk".into(),
            ));
        }
        Ok(ws)
    }

    /// Mount (or replace) a disk at `name`; its objects are addressed as `<name>/...`.
    pub fn add_disk(
        &mut self,
        name: impl Into<String>,
        disk: impl Into<WorkspaceMount>,
    ) -> Result<&mut Self> {
        let name = name.into();
        let rel = trim_slashes(&name).to_string();
        if rel.is_empty() {
            return Err(Error::InvalidArgument(
                "workspace mount name must be non-empty".into(),
            ));
        }
        // Routing is by the first key segment, so a mount name can't contain "/".
        if rel.contains('/') {
            return Err(Error::InvalidArgument(format!(
                "workspace mount name '{name}' must not contain '/'"
            )));
        }
        if rel == "." || rel == ".." {
            return Err(Error::InvalidArgument(format!(
                "workspace mount name '{name}' is reserved ('.' and '..' are path navigation, not disk names)"
            )));
        }
        let mount = disk.into();
        match self.mounts.iter_mut().find(|(n, _)| *n == rel) {
            Some(slot) => slot.1 = mount,
            None => self.mounts.push((rel, mount)),
        }
        Ok(self)
    }

    /// Unmount the disk at `name`; returns whether one was removed. Refuses to
    /// remove the last disk.
    pub fn remove_disk(&mut self, name: &str) -> Result<bool> {
        let rel = trim_slashes(name);
        let Some(pos) = self.mounts.iter().position(|(n, _)| n == rel) else {
            return Ok(false);
        };
        if self.mounts.len() == 1 {
            return Err(Error::InvalidArgument(
                "cannot remove the last disk from a workspace; a workspace must keep at least one"
                    .into(),
            ));
        }
        self.mounts.remove(pos);
        Ok(true)
    }

    /// Names of the currently-mounted disks, in mount order.
    pub fn disk_names(&self) -> Vec<String> {
        self.mounts.iter().map(|(n, _)| n.clone()).collect()
    }

    fn entry(&self, name: &str) -> Option<&WorkspaceMount> {
        self.mounts.iter().find(|(n, _)| n == name).map(|(_, m)| m)
    }

    fn unknown_disk(&self, name: &str) -> Error {
        let names: Vec<String> = self.mounts.iter().map(|(n, _)| format!("{n}/…")).collect();
        Error::InvalidArgument(format!(
            "No disk named '{name}'. Address objects by disk: {}",
            names.join(", ")
        ))
    }

    /// Resolve a workspace key (`<name>/...`) to the mount and the disk-relative key.
    fn route(&self, key: &str) -> Result<(&WorkspaceMount, String)> {
        let segs = to_segments(key, None);
        let Some(first) = segs.first() else {
            return Err(Error::InvalidArgument(
                "that key is the workspace root, not an object; name a disk, e.g. data/file.txt"
                    .into(),
            ));
        };
        let entry = self.entry(first).ok_or_else(|| self.unknown_disk(first))?;
        Ok((entry, disk_key(entry, &segs[1..].join("/"))))
    }

    /// Mounts touched by a key prefix; an empty prefix fans out to all of them.
    fn covered(&self, prefix: &str) -> Result<Vec<(String, &WorkspaceMount, String)>> {
        let segs = to_segments(prefix, None);
        match segs.first() {
            None => Ok(self
                .mounts
                .iter()
                .map(|(n, m)| (n.clone(), m, disk_key(m, "")))
                .collect()),
            Some(first) => {
                let entry = self.entry(first).ok_or_else(|| self.unknown_disk(first))?;
                Ok(vec![(
                    first.clone(),
                    entry,
                    disk_key(entry, &segs[1..].join("/")),
                )])
            }
        }
    }
}

/// Map a workspace-relative key to the disk's key, applying the mount's subdirectory.
fn disk_key(entry: &WorkspaceMount, rel: &str) -> String {
    let trimmed = rel.trim_start_matches('/');
    let sub = trim_slashes(entry.subdirectory.as_deref().unwrap_or(""));
    if sub.is_empty() {
        trimmed.to_string()
    } else if trimmed.is_empty() {
        sub.to_string()
    } else {
        format!("{sub}/{trimmed}")
    }
}

/// Map a disk key back to its workspace key, stripping the mount subdirectory.
fn abs(name: &str, entry: &WorkspaceMount, disk_key: &str) -> String {
    let sub = trim_slashes(entry.subdirectory.as_deref().unwrap_or(""));
    let mut rel = disk_key;
    if !sub.is_empty() {
        if let Some(rest) = disk_key.strip_prefix(&format!("{sub}/")) {
            rel = rest;
        } else if disk_key == sub {
            rel = "";
        }
    }
    let rel = trim_slashes(rel);
    if rel.is_empty() {
        name.to_string()
    } else {
        format!("{name}/{rel}")
    }
}

impl Workspace {
    pub async fn get_object(&self, key: &str) -> Result<Bytes> {
        let (entry, dk) = self.route(key)?;
        entry.disk.get_object(&dk).await
    }

    pub async fn put_object(&self, key: &str, body: impl Into<Bytes>) -> Result<PutObjectResult> {
        self.put_object_with(key, body, PutObjectOptions::default())
            .await
    }

    pub async fn put_object_with(
        &self,
        key: &str,
        body: impl Into<Bytes>,
        options: impl Into<PutObjectOptions>,
    ) -> Result<PutObjectResult> {
        let (entry, dk) = self.route(key)?;
        if entry.read_only {
            return Err(Error::InvalidArgument(format!(
                "{key} is on a read-only disk and cannot be written."
            )));
        }
        entry.disk.put_object_with(&dk, body, options).await
    }

    pub async fn delete_object(&self, key: &str) -> Result<()> {
        let (entry, dk) = self.route(key)?;
        if entry.read_only {
            return Err(Error::InvalidArgument(format!(
                "{key} is on a read-only disk and cannot be deleted."
            )));
        }
        entry.disk.delete_object(&dk).await
    }

    /// List objects. At the workspace root the disks themselves are the
    /// top-level directories (a non-recursive root listing returns just
    /// those). Fans out resiliently: a disk that errors is skipped and the
    /// result flagged `is_truncated`.
    pub async fn list_objects(
        &self,
        prefix: Option<&str>,
        opts: ListObjectsOptions,
    ) -> Result<ListObjectsResult> {
        let prefix = prefix.unwrap_or("");
        if to_segments(prefix, None).is_empty() && !opts.recursive {
            let mut prefixes: Vec<String> =
                self.mounts.iter().map(|(n, _)| format!("{n}/")).collect();
            prefixes.sort();
            return Ok(ListObjectsResult {
                common_prefixes: prefixes,
                ..Default::default()
            });
        }
        let covered = self.covered(prefix)?;
        let listings = join_all(covered.iter().map(|(_, entry, rel)| {
            let p = if rel.is_empty() {
                None
            } else if rel.ends_with('/') {
                Some(rel.clone())
            } else {
                Some(format!("{rel}/"))
            };
            let recursive = opts.recursive;
            async move {
                entry
                    .disk
                    .list_objects(
                        p.as_deref(),
                        ListObjectsOptions {
                            recursive,
                            ..Default::default()
                        },
                    )
                    .await
            }
        }))
        .await;
        let mut objects: Vec<S3Object> = Vec::new();
        let mut common_prefixes = Vec::new();
        let mut is_truncated = false;
        let mut key_count = 0;
        for ((name, entry, _), listing) in covered.iter().zip(listings) {
            let Ok(listing) = listing else {
                is_truncated = true; // a disk failed, so the merged listing may be incomplete
                continue;
            };
            for cp in &listing.common_prefixes {
                common_prefixes.push(format!("{}/", abs(name, entry, cp.trim_end_matches('/'))));
            }
            for obj in listing.objects {
                objects.push(S3Object {
                    key: abs(name, entry, &obj.key),
                    ..obj
                });
            }
            is_truncated = is_truncated || listing.is_truncated;
            key_count += listing.key_count;
        }
        objects.sort_by(|a, b| a.key.cmp(&b.key));
        common_prefixes.sort();
        Ok(ListObjectsResult {
            objects,
            common_prefixes,
            is_truncated,
            next_continuation_token: None,
            key_count,
            prefix: None,
        })
    }

    /// Grep across the covered disks. A disk that errors contributes
    /// `list_failed` instead of failing the whole grep.
    pub async fn grep(&self, opts: GrepOptions) -> Result<GrepResult> {
        let covered = self.covered(&opts.directory)?;
        let results = join_all(covered.iter().map(|(_, entry, rel)| {
            let mut o = opts.clone();
            o.directory = rel.clone();
            async move { entry.disk.grep(o).await }
        }))
        .await;
        let mut matches = Vec::new();
        let (mut files_scanned, mut containers) = (0u64, 0u64);
        let (mut compute, mut duration, mut listing, mut grep_ms) = (0f64, 0f64, 0f64, 0f64);
        let mut reasons: Vec<GrepStoppedReason> = Vec::new();
        for ((name, entry, _), result) in covered.iter().zip(results) {
            let Ok(r) = result else {
                reasons.push(GrepStoppedReason::ListFailed);
                continue;
            };
            files_scanned += r.files_scanned;
            containers += r.containers_dispatched;
            compute += r.compute_seconds_used;
            duration = duration.max(r.duration_ms);
            listing = listing.max(r.listing_ms);
            grep_ms = grep_ms.max(r.grep_ms);
            reasons.push(r.stopped_reason);
            for m in r.matches {
                matches.push(GrepMatch {
                    file: abs(name, entry, &m.file),
                    line: m.line,
                    text: m.text,
                });
            }
        }
        // Merging across disks can exceed the cap even when every disk
        // completed, so flag the truncation before slicing.
        let cap = opts.max_results as usize;
        if matches.len() > cap {
            reasons.push(GrepStoppedReason::MaxResults);
        }
        // Report the most serious reason so a failure on one disk isn't masked
        // by a truncation reason on another.
        let priority = [
            GrepStoppedReason::ListFailed,
            GrepStoppedReason::Incomplete,
            GrepStoppedReason::Deadline,
            GrepStoppedReason::MaxResults,
        ];
        let stopped_reason = priority
            .into_iter()
            .find(|p| reasons.contains(p))
            .unwrap_or(GrepStoppedReason::Completed);
        matches.truncate(cap);
        Ok(GrepResult {
            matches,
            stopped_reason,
            files_scanned,
            containers_dispatched: containers,
            compute_seconds_used: compute,
            duration_ms: duration,
            listing_ms: listing,
            grep_ms,
        })
    }

    /// Run a command with every mounted disk under `/mnt/archil/<name>`.
    pub async fn exec(&self, command: &str) -> Result<ExecResult> {
        let mut options = ExecOptions::new(command);
        for (name, entry) in &self.mounts {
            let has_options = entry.read_only
                || entry.subdirectory.as_deref().is_some_and(|s| !s.is_empty())
                || entry.conditional
                || entry.queue_ms.is_some()
                || entry.checkout_paths.is_some();
            let mount = if has_options {
                ExecMount::Spec(ExecMountSpec {
                    disk: (&entry.disk).into(),
                    subdirectory: entry.subdirectory.clone(),
                    read_only: entry.read_only,
                    conditional: entry.conditional,
                    queue_ms: entry.queue_ms,
                    checkout_paths: entry.checkout_paths.clone(),
                })
            } else {
                ExecMount::Disk((&entry.disk).into())
            };
            options.disks.insert(name.clone(), mount);
        }
        self.client.exec(options).await
    }
}

impl FileSystem for Workspace {
    async fn get_object(&self, key: &str) -> Result<Bytes> {
        Workspace::get_object(self, key).await
    }
    async fn put_object(
        &self,
        key: &str,
        body: Bytes,
        options: PutObjectOptions,
    ) -> Result<PutObjectResult> {
        self.put_object_with(key, body, options).await
    }
    async fn delete_object(&self, key: &str) -> Result<()> {
        Workspace::delete_object(self, key).await
    }
    async fn list_objects(
        &self,
        prefix: Option<&str>,
        opts: ListObjectsOptions,
    ) -> Result<ListObjectsResult> {
        Workspace::list_objects(self, prefix, opts).await
    }
    async fn grep(&self, opts: GrepOptions) -> Result<GrepResult> {
        Workspace::grep(self, opts).await
    }
    async fn exec(&self, command: &str) -> Result<ExecResult> {
        Workspace::exec(self, command).await
    }
}

impl FileSystem for Disk {
    async fn get_object(&self, key: &str) -> Result<Bytes> {
        Disk::get_object(self, key).await
    }
    async fn put_object(
        &self,
        key: &str,
        body: Bytes,
        options: PutObjectOptions,
    ) -> Result<PutObjectResult> {
        self.put_object_with(key, body, options).await
    }
    async fn delete_object(&self, key: &str) -> Result<()> {
        Disk::delete_object(self, key).await
    }
    async fn list_objects(
        &self,
        prefix: Option<&str>,
        opts: ListObjectsOptions,
    ) -> Result<ListObjectsResult> {
        Disk::list_objects(self, prefix, opts).await
    }
    async fn grep(&self, opts: GrepOptions) -> Result<GrepResult> {
        Disk::grep(self, opts).await
    }
    async fn exec(&self, command: &str) -> Result<ExecResult> {
        Disk::exec(self, command).await
    }
}
