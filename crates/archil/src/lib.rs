//! Rust port of Archil's official TypeScript SDK (the `disk` npm package).
//!
//! ```no_run
//! # async fn demo() -> archil::Result<()> {
//! use archil::{Archil, CreateSandboxRequest, ProcessStartOptions};
//!
//! // Reads ARCHIL_API_KEY and ARCHIL_REGION.
//! let client = Archil::from_env()?;
//! let mut sandbox = client.sandboxes().create(CreateSandboxRequest::new()).await?;
//! let result = sandbox.exec("uname -a", ProcessStartOptions::new()).await?;
//! println!("{}", result.stdout);
//! sandbox.stop().await?;
//! sandbox.delete().await?;
//! # Ok(()) }
//! ```
//!
//! Entry points: [`Archil`] (disks, images, sandboxes, API keys, multi-disk
//! exec), [`Disk`] (users, branches, object/S3 API, grep, exec), [`Sandbox`]
//! (lifecycle, ports, network policy, processes, files) and [`Workspace`]
//! (multi-disk key routing). Errors are the typed [`Error`].

mod client;
mod disk;
mod disks;
mod error;
mod files;
mod filesystem;
mod global;
mod http;
mod images;
mod paths;
mod process;
mod regions;
mod retry;
mod s3;
mod sandbox;
mod sandboxes;
mod tokens;
mod types;
mod workspace;

pub use client::{
    Archil, ClientOptions, DiskRef, ExecMount, ExecMountSpec, ExecOptions, USER_AGENT, VERSION,
};
pub use disk::{
    CreateBranchOptions, Disk, DiskMultipart, GrepOptions, ListObjectsPages, ShareUrlOptions,
    ShareUrlResult, UserType,
};
pub use disks::{CreateDiskResult, DiskListPage, Disks, ListDisksOptions};
pub use error::{
    ApiError, Error, ImageBuildError, Result, S3Error, SandboxFileTransferError, SandboxPauseError,
    TransferOperation,
};
pub use files::{FileDownload, FileSource, SandboxFileUploadOptions, SandboxFiles};
pub use filesystem::FileSystem;
pub use global::{
    client, configure, create_api_key, create_disk, create_sandbox, delete_api_key, exec, get_disk,
    get_sandbox, list_api_keys, list_disks, list_sandboxes, workspace,
};
pub use images::{
    BuildImageOptions, BuildImageRequest, Image, ImageFailureReason, ImageStatus, Images,
    RegistryAuth,
};
pub use paths::to_segments;
pub use process::{
    OutputHandler, ProcessConnectOptions, ProcessOutput, ProcessResult, ProcessStartOptions,
    ProcessStatus, ProcessStream, SandboxProcess, Terminal,
};
pub use regions::{REGION_URLS, derive_s3_base_url, resolve_base_url};
pub use s3::{
    AppendObjectOptions, CompletedMultipartUpload, DeleteObjectsError, DeleteObjectsOptions,
    DeleteObjectsResult, ListMultipartUploadsOptions, ListObjectsOptions, ListObjectsResult,
    ListPartsOptions, MultipartUpload, MultipartUploadListing, MultipartUploadSummary,
    ObjectMetadata, PartInfo, PartListing, PosixCreateAttrs, PutObjectOptions, PutObjectResult,
    S3Object, UploadPart, effective_upload_part_size,
};
pub use sandbox::{
    CreateSandboxPortTokenOptions, CreatedSandboxPortToken, ListSandboxPortTokensOptions, Sandbox,
    SandboxEgressAllow, SandboxEgressPolicy, SandboxEgressRule, SandboxEgressTransform,
    SandboxEndpoint, SandboxForkOptions, SandboxMount, SandboxMountSpec, SandboxNetwork,
    SandboxNetworkAction, SandboxPortToken, SandboxPortTokenPage, SandboxResponse,
    SandboxStartOptions, SandboxStatus, SandboxTimeoutOptions, WaitOptions,
};
pub use sandboxes::{CreateSandboxRequest, ListSandboxesOptions, Sandboxes};
pub use tokens::{ListTokensOptions, Tokens};
pub use types::*;
pub use workspace::{Workspace, WorkspaceMount};
