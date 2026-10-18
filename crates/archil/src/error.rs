//! Typed errors.
//!
//! [`Error`] mirrors the TypeScript SDK's `ArchilError` hierarchy: every
//! variant that came from the service exposes [`Error::status`] and
//! [`Error::code`].

use crate::images::{Image, ImageFailureReason};
use crate::sandbox::Sandbox;

/// Result alias used across the crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Error from the control-plane REST API (`ArchilApiError`).
#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct ApiError {
    pub message: String,
    /// HTTP status code.
    pub status: u16,
    /// Machine-readable error code, when the server provided one.
    pub code: Option<String>,
}

/// Error from the S3-compatible object API (`ArchilS3Error`).
#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct S3Error {
    pub message: String,
    pub status: u16,
    /// S3 error code such as `NoSuchKey`.
    pub code: Option<String>,
    /// S3 request id, if the gateway returned one.
    pub request_id: Option<String>,
    /// Raw response body (the XML document), for debugging.
    pub raw: String,
}

/// A sandbox left `pausing` without reaching `paused` (`SandboxPauseError`).
#[derive(Debug, Clone, thiserror::Error)]
#[error("Sandbox entered {status} before it paused{reason}", status = latest.status, reason = latest.exit_reason.as_deref().map(|r| format!(": {r}")).unwrap_or_default())]
pub struct SandboxPauseError {
    /// The sandbox as last observed.
    pub latest: Box<Sandbox>,
}

/// An image build failed (`ImageBuildError`).
#[derive(Debug, Clone, thiserror::Error)]
#[error("Image {} build failed: {}", latest.image_id, reason_text(latest.failure_reason))]
pub struct ImageBuildError {
    /// The image as last observed.
    pub latest: Box<Image>,
}

impl ImageBuildError {
    /// Why the build failed.
    pub fn reason(&self) -> Option<ImageFailureReason> {
        self.latest.failure_reason
    }
}

fn reason_text(reason: Option<ImageFailureReason>) -> &'static str {
    match reason {
        Some(ImageFailureReason::InvalidSource) => "invalid_source",
        Some(ImageFailureReason::Interrupted) => "interrupted",
        Some(ImageFailureReason::Unknown) => "unknown",
        None => "undefined",
    }
}

/// Which direction a sandbox file transfer was going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferOperation {
    Upload,
    Download,
}

impl std::fmt::Display for TransferOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Upload => "upload",
            Self::Download => "download",
        })
    }
}

/// A sandbox file upload or download failed (`SandboxFileTransferError`).
#[derive(Debug, thiserror::Error)]
#[error("Sandbox file {operation} failed for {path}: {cause}")]
pub struct SandboxFileTransferError {
    pub operation: TransferOperation,
    pub path: String,
    pub cause: Box<Error>,
}

/// Every error the SDK returns.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error(transparent)]
    S3(#[from] S3Error),
    #[error(transparent)]
    SandboxPause(#[from] SandboxPauseError),
    #[error(transparent)]
    ImageBuild(#[from] ImageBuildError),
    #[error(transparent)]
    FileTransfer(#[from] SandboxFileTransferError),
    /// A generic SDK error with an HTTP-style status and code, e.g. the
    /// `IMAGE_BUILD_TIMEOUT` raised when an image build outlives its budget.
    #[error("{message}")]
    Archil {
        message: String,
        status: u16,
        code: Option<String>,
    },
    /// Missing or invalid client configuration (API key, region, TLS, ...).
    #[error("{0}")]
    Config(String),
    /// A caller-supplied argument was rejected before any request was sent.
    #[error("{0}")]
    InvalidArgument(String),
    /// The HTTP transport failed.
    #[error("request failed: {0}")]
    Transport(#[from] reqwest::Error),
    /// A process connection (WebSocket) failed or was misused.
    #[error("{0}")]
    Process(String),
    /// A response body could not be decoded.
    #[error("invalid response: {0}")]
    Decode(String),
}

impl Error {
    /// HTTP status associated with the failure, for errors that carry one.
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Api(e) => Some(e.status),
            Self::S3(e) => Some(e.status),
            Self::SandboxPause(_) => Some(409),
            Self::ImageBuild(_) => Some(409),
            Self::FileTransfer(_) => Some(500),
            Self::Archil { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Machine-readable error code, for errors that carry one.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Api(e) => e.code.as_deref(),
            Self::S3(e) => e.code.as_deref(),
            Self::SandboxPause(_) => Some("SANDBOX_PAUSE_FAILED"),
            Self::ImageBuild(_) => Some("IMAGE_BUILD_FAILED"),
            Self::FileTransfer(_) => Some("SANDBOX_FILE_TRANSFER_FAILED"),
            Self::Archil { code, .. } => code.as_deref(),
            _ => None,
        }
    }

    pub(crate) fn process(msg: impl Into<String>) -> Self {
        Self::Process(msg.into())
    }
}
