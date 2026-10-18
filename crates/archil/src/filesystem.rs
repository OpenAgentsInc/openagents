//! The key-addressable filesystem contract shared by [`Disk`](crate::Disk)
//! and [`Workspace`](crate::Workspace).

use std::future::Future;

use bytes::Bytes;

use crate::disk::GrepOptions;
use crate::error::Result;
use crate::s3::{ListObjectsOptions, ListObjectsResult, PutObjectOptions, PutObjectResult};
use crate::types::{ExecResult, GrepResult};

/// A key-addressable filesystem over one or more Archil disks.
///
/// A single [`Disk`](crate::Disk) is the one-disk case: keys are relative to
/// the disk root. A [`Workspace`](crate::Workspace) spans several disks, each
/// mounted under a name that is the first segment of every key. Both
/// implement this trait so the two cannot drift apart.
pub trait FileSystem {
    /// Read an object's full contents. Fails with a 404 [`S3Error`](crate::S3Error) if absent.
    fn get_object(&self, key: &str) -> impl Future<Output = Result<Bytes>> + Send;
    /// Create or overwrite an object.
    fn put_object(
        &self,
        key: &str,
        body: Bytes,
        options: PutObjectOptions,
    ) -> impl Future<Output = Result<PutObjectResult>> + Send;
    /// Delete an object (idempotent).
    fn delete_object(&self, key: &str) -> impl Future<Output = Result<()>> + Send;
    /// List objects and common (directory) prefixes under a key prefix.
    fn list_objects(
        &self,
        prefix: Option<&str>,
        opts: ListObjectsOptions,
    ) -> impl Future<Output = Result<ListObjectsResult>> + Send;
    /// Constant-time parallel grep.
    fn grep(&self, opts: GrepOptions) -> impl Future<Output = Result<GrepResult>> + Send;
    /// Run a command in a container with the filesystem mounted.
    fn exec(&self, command: &str) -> impl Future<Output = Result<ExecResult>> + Send;
}
