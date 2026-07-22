//! Audience-scoped, read-only preview over the managed guest-I/O channel.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::managed_sandbox_guest_io::{self, ManagedSandboxGuestIoResponse};
use crate::managed_sandbox_runtime;

const SCHEMA_VERSION: &str = "openagents.managed_sandbox_private_preview.v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedSandboxPrivatePreviewRequest {
    pub schema_version: String,
    pub request_ref: String,
    pub capability_ref: String,
    pub audience_ref: String,
    pub path: String,
    pub encoding: String,
    pub capability: Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSandboxPrivatePreviewResponse {
    pub schema_version: &'static str,
    pub capability_ref: String,
    pub audience_ref: String,
    pub sandbox_ref: String,
    pub resource_generation: u64,
    pub preview: ManagedSandboxGuestIoResponse,
}

#[derive(Debug)]
pub enum PrivatePreviewError {
    Invalid(&'static str),
    Runtime(managed_sandbox_runtime::RuntimeError),
    GuestIo(managed_sandbox_guest_io::GuestIoError),
}

impl PrivatePreviewError {
    pub fn status(&self) -> u16 {
        match self {
            Self::Invalid(_) => 400,
            Self::Runtime(error) => error.status(),
            Self::GuestIo(error) => error.status(),
        }
    }

    pub fn response(&self) -> Value {
        match self {
            Self::Invalid(reason_ref) => json!({
                "schemaVersion": "openagents.managed_sandbox_private_preview_error.v1",
                "code": "invalid_request",
                "reasonRef": reason_ref,
                "retryable": false
            }),
            Self::Runtime(error) => error.response(),
            Self::GuestIo(error) => error.response(),
        }
    }
}

pub fn execute(
    state_root: &Path,
    request: ManagedSandboxPrivatePreviewRequest,
) -> Result<ManagedSandboxPrivatePreviewResponse, PrivatePreviewError> {
    if request.schema_version != SCHEMA_VERSION
        || request.capability_ref.len() < 3
        || request.audience_ref.len() < 3
        || request.request_ref.len() < 3
    {
        return Err(PrivatePreviewError::Invalid(
            "private_preview_request_invalid",
        ));
    }
    if request
        .capability
        .get("capabilityRef")
        .and_then(Value::as_str)
        != Some(request.capability_ref.as_str())
    {
        return Err(PrivatePreviewError::Invalid(
            "private_preview_capability_ref_conflict",
        ));
    }
    let guest_request = managed_sandbox_runtime::authorize_private_preview(
        state_root,
        &request.capability,
        &request.audience_ref,
        &request.path,
        &request.encoding,
        &request.request_ref,
    )
    .map_err(PrivatePreviewError::Runtime)?;
    let sandbox_ref = guest_request.sandbox_ref.clone();
    let resource_generation = guest_request.resource_generation;
    let preview =
        managed_sandbox_guest_io::execute(guest_request).map_err(PrivatePreviewError::GuestIo)?;
    Ok(ManagedSandboxPrivatePreviewResponse {
        schema_version: SCHEMA_VERSION,
        capability_ref: request.capability_ref,
        audience_ref: request.audience_ref,
        sandbox_ref,
        resource_generation,
        preview,
    })
}
