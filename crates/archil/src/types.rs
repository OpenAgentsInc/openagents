//! Wire types for the control-plane API (disks, users, tokens, exec, grep).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Disk lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiskStatus {
    Available,
    Creating,
    Deleting,
    Deleted,
    Failed,
    #[serde(other)]
    Unknown,
}

/// POSIX attributes of a disk's root directory, applied at creation.
///
/// `mode` is a plain integer: write it as an octal literal (`0o755`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootAttrs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gid: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
}

/// Mount configuration returned for a disk's storage mount (secrets omitted).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MountConfigResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// A storage mount attached to a disk.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MountResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// `s3`, `gcs`, `r2`, `s3-compatible` or `azure-blob`.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `rw` or `ro`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<MountConfigResponse>,
    /// `connected` or `disconnected`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_error: Option<String>,
    /// `iam`, `accessKeys` or `oauth`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization_type: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskMetrics {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_transfer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg_response_time: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedClient {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connected_at: Option<String>,
}

/// A user authorized to mount a disk.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedUser {
    /// `token` or `awssts`.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_suffix: Option<String>,
    /// The generated mount token. Shown exactly once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Stable identifier, used to remove the user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub one_use: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
    /// `active`, `expired` or `consumed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// A disk as returned by the API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskResponse {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub organization: String,
    pub status: DiskStatus,
    #[serde(default)]
    pub fs_handler_status: Option<String>,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub sandbox_disk: bool,
    #[serde(default)]
    pub last_accessed: Option<String>,
    #[serde(default)]
    pub active_data_bytes: Option<u64>,
    #[serde(default)]
    pub total_data_bytes: Option<u64>,
    #[serde(default)]
    pub monthly_usage: Option<String>,
    #[serde(default)]
    pub mounts: Option<Vec<MountResponse>>,
    #[serde(default)]
    pub metrics: Option<DiskMetrics>,
    #[serde(default)]
    pub connected_clients: Option<Vec<ConnectedClient>>,
    #[serde(default)]
    pub authorized_users: Option<Vec<AuthorizedUser>>,
    #[serde(default)]
    pub allowed_ips: Option<Vec<String>>,
    #[serde(default)]
    pub capabilities: Option<Vec<String>>,
    #[serde(default)]
    pub root_attrs: Option<RootAttrs>,
}

/// Storage mount to attach when creating a disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MountConfig {
    #[serde(rename = "s3", rename_all = "camelCase")]
    S3 {
        bucket_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        access_key_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secret_access_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_token: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bucket_prefix: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bucket_region: Option<String>,
    },
    #[serde(rename = "gcs", rename_all = "camelCase")]
    Gcs {
        bucket_name: String,
        access_key_id: String,
        secret_access_key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bucket_prefix: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bucket_region: Option<String>,
    },
    #[serde(rename = "r2", rename_all = "camelCase")]
    R2 {
        bucket_name: String,
        bucket_endpoint: String,
        access_key_id: String,
        secret_access_key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bucket_prefix: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bucket_region: Option<String>,
    },
    #[serde(rename = "s3-compatible", rename_all = "camelCase")]
    S3Compatible {
        bucket_name: String,
        bucket_endpoint: String,
        access_key_id: String,
        secret_access_key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bucket_prefix: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bucket_region: Option<String>,
    },
    #[serde(rename = "azure-blob", rename_all = "camelCase")]
    AzureBlob {
        container_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        endpoint: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        storage_account_name: Option<String>,
        tenant_id: String,
        client_id: String,
        client_secret: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bucket_prefix: Option<String>,
    },
}

/// A user to authorize on a disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DiskUser {
    /// A shared mount token. The server generates the secret when `principal`
    /// is omitted.
    #[serde(rename = "token", rename_all = "camelCase")]
    Token(TokenUser),
    /// AWS STS role assumption.
    #[serde(rename = "awssts", rename_all = "camelCase")]
    AwsSts(AwsStsUser),
}

/// Token user fields.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUser {
    pub nickname: String,
    /// Deprecated: client-provided token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
    /// Deprecated: last 4 characters of a client-provided token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_suffix: Option<String>,
    /// Go duration such as `"1h"`; at most `"8760h"`. Exclusive with `expires_at`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
    /// RFC 3339 expiry. Exclusive with `ttl`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub one_use: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
}

/// AWS STS user fields.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwsStsUser {
    /// IAM principal ARN.
    pub principal: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub one_use: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
}

/// Request body for creating a disk.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDiskRequest {
    pub name: String,
    /// Storage mounts to attach. Omit for Archil-managed storage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mounts: Option<Vec<MountConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_ips: Option<Vec<String>>,
    /// Deprecated: suppresses the default generated token user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_methods: Option<Vec<DiskUser>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_attrs: Option<RootAttrs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_client_version: Option<String>,
}

impl CreateDiskRequest {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }
}

/// A delegation: exclusive write access to an inode for one client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Delegation {
    pub client_id: String,
    pub inode_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default)]
    pub is_pending: bool,
    #[serde(default)]
    pub is_orphaned: bool,
}

/// A writable fork of a disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    pub root_filesystem_id: String,
    pub branch_name: String,
    pub filesystem_id: String,
    pub from_checkpoint_name: String,
    pub from_checkpoint_filesystem_id: String,
    pub created_at: String,
}

/// Request body for creating an API key.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateApiTokenRequest {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Defaults to full access (`*`) when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,
}

impl CreateApiTokenRequest {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }
}

/// An API key's metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiTokenResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_suffix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
}

/// A newly created API key: metadata plus the full secret (shown once).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedApiToken {
    #[serde(flatten)]
    pub info: ApiTokenResponse,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// Server-measured timings for an exec request, in milliseconds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecTiming {
    #[serde(default)]
    pub total_ms: f64,
    #[serde(default)]
    pub queue_ms: f64,
    #[serde(default)]
    pub execute_ms: f64,
}

/// Result of a serverless exec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecDiskResult {
    pub exit_code: i64,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    #[serde(default)]
    pub timing: ExecTiming,
}

/// Alias matching the TypeScript SDK's `ExecResult`.
pub type ExecResult = ExecDiskResult;

/// One matching line from a grep.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrepMatch {
    pub file: String,
    pub line: u64,
    pub text: String,
}

/// Why a grep stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrepStoppedReason {
    Completed,
    Incomplete,
    MaxResults,
    Deadline,
    ListFailed,
}

/// Result of a serverless grep.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrepDiskResult {
    pub matches: Vec<GrepMatch>,
    pub stopped_reason: GrepStoppedReason,
    #[serde(default)]
    pub files_scanned: u64,
    #[serde(default)]
    pub containers_dispatched: u64,
    #[serde(default)]
    pub compute_seconds_used: f64,
    #[serde(default)]
    pub duration_ms: f64,
    #[serde(default)]
    pub listing_ms: f64,
    #[serde(default)]
    pub grep_ms: f64,
}

/// Alias matching the TypeScript SDK's `GrepResult`.
pub type GrepResult = GrepDiskResult;

/// Wire form of one disk in a multi-disk exec request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecMountWire {
    pub disk: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subdirectory: Option<String>,
    pub read_only: bool,
    pub conditional: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout_paths: Option<Vec<String>>,
}

/// Wire form of a multi-disk exec request (`ExecRequest`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecRequest {
    pub disks: BTreeMap<String, serde_json::Value>,
    pub command: String,
}

/// Wire request to grep a disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrepDiskRequest {
    pub directory: String,
    pub pattern: String,
    pub recursive: bool,
    pub max_duration_seconds: u64,
    pub concurrency: u64,
    pub max_results: u64,
}
