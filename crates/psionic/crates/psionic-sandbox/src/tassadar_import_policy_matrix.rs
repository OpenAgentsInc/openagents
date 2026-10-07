use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    TassadarDeterministicImportSideEffectPolicy, TassadarHostImportStubKind,
    TassadarModuleExecutionRefusalKind, tassadar_module_execution_capability_report,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_import_policy_matrix_report.json";

/// Typed import class published by the sandbox policy matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarImportClass {
    DeterministicStub,
    ExternalSandboxDelegation,
    UnsafeSideEffect,
}

/// Mount-time execution posture for one import entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarImportExecutionBoundary {
    InternalOnly,
    SandboxDelegationOnly,
    Refused,
}

/// Evidence requirement attached to one import entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarImportEvidenceRequirement {
    CapabilityPublicationOnly,
    SandboxDescriptorAndChallengeReceipt,
    Refused,
}

/// One import entry in the host-call policy matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarImportPolicyEntry {
    pub import_ref: String,
    pub import_class: TassadarImportClass,
    pub execution_boundary: TassadarImportExecutionBoundary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_stub_kind: Option<TassadarHostImportStubKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side_effect_policy: Option<TassadarDeterministicImportSideEffectPolicy>,
    pub evidence_requirement: TassadarImportEvidenceRequirement,
    pub refusal_kind: TassadarModuleExecutionRefusalKind,
    pub note: String,
}

/// Sandbox-owned host-call policy matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarImportPolicyMatrix {
    pub matrix_id: String,
    pub supported_internal_stub_kinds: Vec<TassadarHostImportStubKind>,
    pub entries: Vec<TassadarImportPolicyEntry>,
    pub kernel_policy_dependency_marker: String,
    pub world_mount_dependency_marker: String,
    pub claim_boundary: String,
    pub matrix_digest: String,
}

/// Negotiation request evaluated against the import policy matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarImportPolicyRequest {
    pub request_id: String,
    pub import_ref: String,
    pub allow_external_delegation: bool,
    pub sandbox_descriptor_present: bool,
    pub challenge_receipt_present: bool,
}

/// Successful policy decision for one import request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarImportPolicyDecision {
    AllowedInternal,
    DelegatedSandbox,
}

/// Typed refusal reason for one import request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarImportPolicyRefusalReason {
    UnknownImport,
    ExternalDelegationDisallowed,
    SandboxDescriptorMissing,
    ChallengeReceiptMissing,
    UnsafeImportClass,
}

/// Successful resolution for one import request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarImportPolicySelection {
    pub request_id: String,
    pub import_ref: String,
    pub decision: TassadarImportPolicyDecision,
    pub note: String,
}

/// One evaluated host-call policy case in the committed report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarImportPolicyCaseReport {
    pub case_id: String,
    pub request: TassadarImportPolicyRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<TassadarImportPolicySelection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarImportPolicyRefusalReason>,
    pub note: String,
}

/// Committed report over the seeded host-call policy matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarImportPolicyMatrixReport {
    pub schema_version: u16,
    pub report_id: String,
    pub policy_matrix: TassadarImportPolicyMatrix,
    pub allowed_internal_case_count: u32,
    pub delegated_case_count: u32,
    pub refused_case_count: u32,
    pub case_reports: Vec<TassadarImportPolicyCaseReport>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarImportPolicyMatrixReportError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Returns the seeded host-call policy matrix for bounded module imports.
#[must_use]
pub fn tassadar_import_policy_matrix() -> TassadarImportPolicyMatrix {
    let capability = tassadar_module_execution_capability_report();
    let entries = vec![
        TassadarImportPolicyEntry {
            import_ref: String::from("env.clock_stub"),
            import_class: TassadarImportClass::DeterministicStub,
            execution_boundary: TassadarImportExecutionBoundary::InternalOnly,
            runtime_stub_kind: Some(TassadarHostImportStubKind::DeterministicI32Const),
            side_effect_policy: Some(TassadarDeterministicImportSideEffectPolicy::NoSideEffects),
            evidence_requirement: TassadarImportEvidenceRequirement::CapabilityPublicationOnly,
            refusal_kind: capability
                .host_import_boundary
                .unsupported_host_call_refusal,
            note: String::from(
                "deterministic zero-side-effect import stub stays inside the bounded internal execution lane",
            ),
        },
        TassadarImportPolicyEntry {
            import_ref: String::from("sandbox.math_eval"),
            import_class: TassadarImportClass::ExternalSandboxDelegation,
            execution_boundary: TassadarImportExecutionBoundary::SandboxDelegationOnly,
            runtime_stub_kind: None,
            side_effect_policy: None,
            evidence_requirement:
                TassadarImportEvidenceRequirement::SandboxDescriptorAndChallengeReceipt,
            refusal_kind: capability
                .host_import_boundary
                .unsupported_host_call_refusal,
            note: String::from(
                "external math delegation is allowed only through an explicit sandbox descriptor and challengeable receipt path",
            ),
        },
        TassadarImportPolicyEntry {
            import_ref: String::from("host.fs_write"),
            import_class: TassadarImportClass::UnsafeSideEffect,
            execution_boundary: TassadarImportExecutionBoundary::Refused,
            runtime_stub_kind: Some(TassadarHostImportStubKind::UnsupportedHostCall),
            side_effect_policy: None,
            evidence_requirement: TassadarImportEvidenceRequirement::Refused,
            refusal_kind: capability
                .host_import_boundary
                .unsupported_host_call_refusal,
            note: String::from(
                "filesystem writes remain disallowed because they collapse bounded internal execution into side-effectful host behavior",
            ),
        },
    ];
    let mut matrix = TassadarImportPolicyMatrix {
        matrix_id: String::from("tassadar.import_policy_matrix.v1"),
        supported_internal_stub_kinds: capability.host_import_boundary.supported_stub_kinds,
        entries,
        kernel_policy_dependency_marker: String::from(
            "kernel-policy remains the authority owner for settlement-grade import admission outside standalone psionic",
        ),
        world_mount_dependency_marker: String::from(
            "world-mounts remain the authority owner for task-scoped import admission outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this matrix keeps bounded internal deterministic stubs separate from sandbox delegation and refused side effects. It does not collapse external delegation into internal exact compute",
        ),
        matrix_digest: String::new(),
    };
    matrix.matrix_digest = stable_digest(b"psionic_tassadar_import_policy_matrix|", &matrix);
    matrix
}

/// Negotiates one import request against the seeded policy matrix.
pub fn negotiate_tassadar_import_policy(
    request: &TassadarImportPolicyRequest,
    matrix: &TassadarImportPolicyMatrix,
) -> Result<TassadarImportPolicySelection, TassadarImportPolicyRefusalReason> {
    let entry = matrix
        .entries
        .iter()
        .find(|entry| entry.import_ref == request.import_ref)
        .ok_or(TassadarImportPolicyRefusalReason::UnknownImport)?;
    match entry.execution_boundary {
        TassadarImportExecutionBoundary::InternalOnly => Ok(TassadarImportPolicySelection {
            request_id: request.request_id.clone(),
            import_ref: request.import_ref.clone(),
            decision: TassadarImportPolicyDecision::AllowedInternal,
            note: entry.note.clone(),
        }),
        TassadarImportExecutionBoundary::SandboxDelegationOnly => {
            if !request.allow_external_delegation {
                return Err(TassadarImportPolicyRefusalReason::ExternalDelegationDisallowed);
            }
            if !request.sandbox_descriptor_present {
                return Err(TassadarImportPolicyRefusalReason::SandboxDescriptorMissing);
            }
            if !request.challenge_receipt_present {
                return Err(TassadarImportPolicyRefusalReason::ChallengeReceiptMissing);
            }
            Ok(TassadarImportPolicySelection {
                request_id: request.request_id.clone(),
                import_ref: request.import_ref.clone(),
                decision: TassadarImportPolicyDecision::DelegatedSandbox,
                note: entry.note.clone(),
            })
        }
        TassadarImportExecutionBoundary::Refused => {
            Err(TassadarImportPolicyRefusalReason::UnsafeImportClass)
        }
    }
}

/// Builds the committed host-call policy matrix report.
#[must_use]
pub fn build_tassadar_import_policy_matrix_report() -> TassadarImportPolicyMatrixReport {
    let policy_matrix = tassadar_import_policy_matrix();
    let case_reports = vec![
        case_report(
            "import_policy.internal_allowed.v1",
            TassadarImportPolicyRequest {
                request_id: String::from("request.internal_allowed"),
                import_ref: String::from("env.clock_stub"),
                allow_external_delegation: false,
                sandbox_descriptor_present: false,
                challenge_receipt_present: false,
            },
            &policy_matrix,
            "deterministic clock stub stays internal and is admissible without sandbox delegation",
        ),
        case_report(
            "import_policy.sandbox_delegated.v1",
            TassadarImportPolicyRequest {
                request_id: String::from("request.sandbox_delegated"),
                import_ref: String::from("sandbox.math_eval"),
                allow_external_delegation: true,
                sandbox_descriptor_present: true,
                challenge_receipt_present: true,
            },
            &policy_matrix,
            "sandbox math delegation stays explicit as an external boundary with descriptor and challenge receipt attached",
        ),
        case_report(
            "import_policy.delegation_denied.v1",
            TassadarImportPolicyRequest {
                request_id: String::from("request.delegation_denied"),
                import_ref: String::from("sandbox.math_eval"),
                allow_external_delegation: false,
                sandbox_descriptor_present: true,
                challenge_receipt_present: true,
            },
            &policy_matrix,
            "mount-time policy denial remains explicit instead of silently treating external delegation as an internal import",
        ),
        case_report(
            "import_policy.unsafe_refused.v1",
            TassadarImportPolicyRequest {
                request_id: String::from("request.unsafe_refused"),
                import_ref: String::from("host.fs_write"),
                allow_external_delegation: true,
                sandbox_descriptor_present: true,
                challenge_receipt_present: true,
            },
            &policy_matrix,
            "unsafe side effects remain blocked even when the caller offers delegation and receipts",
        ),
    ];
    let mut report = TassadarImportPolicyMatrixReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.import_policy_matrix.report.v1"),
        policy_matrix,
        allowed_internal_case_count: case_reports
            .iter()
            .filter(|case| {
                case.selection.as_ref().map(|selection| selection.decision)
                    == Some(TassadarImportPolicyDecision::AllowedInternal)
            })
            .count() as u32,
        delegated_case_count: case_reports
            .iter()
            .filter(|case| {
                case.selection.as_ref().map(|selection| selection.decision)
                    == Some(TassadarImportPolicyDecision::DelegatedSandbox)
            })
            .count() as u32,
        refused_case_count: case_reports
            .iter()
            .filter(|case| case.refusal_reason.is_some())
            .count() as u32,
        case_reports,
        claim_boundary: String::from(
            "this sandbox report keeps deterministic internal stubs, sandbox delegation, and refused side effects explicit. It does not claim kernel-policy or world-mount settlement closure inside standalone psionic",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Import-policy matrix report now freezes {} internal cases, {} delegated sandbox cases, and {} refused cases.",
        report.allowed_internal_case_count, report.delegated_case_count, report.refused_case_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_import_policy_matrix_report|", &report);
    report
}

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_import_policy_matrix_report_path() -> PathBuf {
    repo_root().join(TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF)
}

/// Writes the committed report.
pub fn write_tassadar_import_policy_matrix_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarImportPolicyMatrixReport, TassadarImportPolicyMatrixReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarImportPolicyMatrixReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_import_policy_matrix_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarImportPolicyMatrixReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_import_policy_matrix_report(
    path: impl AsRef<Path>,
) -> Result<TassadarImportPolicyMatrixReport, TassadarImportPolicyMatrixReportError> {
    read_json(path)
}

fn case_report(
    case_id: &str,
    request: TassadarImportPolicyRequest,
    matrix: &TassadarImportPolicyMatrix,
    note: &str,
) -> TassadarImportPolicyCaseReport {
    match negotiate_tassadar_import_policy(&request, matrix) {
        Ok(selection) => TassadarImportPolicyCaseReport {
            case_id: String::from(case_id),
            request,
            selection: Some(selection),
            refusal_reason: None,
            note: String::from(note),
        },
        Err(refusal_reason) => TassadarImportPolicyCaseReport {
            case_id: String::from(case_id),
            request,
            selection: None,
            refusal_reason: Some(refusal_reason),
            note: String::from(note),
        },
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarImportPolicyMatrixReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarImportPolicyMatrixReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarImportPolicyMatrixReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarImportClass, TassadarImportPolicyDecision, TassadarImportPolicyRefusalReason,
        build_tassadar_import_policy_matrix_report, load_tassadar_import_policy_matrix_report,
        negotiate_tassadar_import_policy, tassadar_import_policy_matrix,
        tassadar_import_policy_matrix_report_path,
    };

    #[test]
    fn import_policy_matrix_is_machine_legible() {
        let matrix = tassadar_import_policy_matrix();

        assert_eq!(matrix.entries.len(), 3);
        assert!(matrix.entries.iter().any(|entry| {
            entry.import_ref == "env.clock_stub"
                && entry.import_class == TassadarImportClass::DeterministicStub
        }));
        assert!(matrix.entries.iter().any(|entry| {
            entry.import_ref == "sandbox.math_eval"
                && entry.import_class == TassadarImportClass::ExternalSandboxDelegation
        }));
    }

    #[test]
    fn import_policy_negotiation_keeps_delegation_and_refusal_explicit() {
        let matrix = tassadar_import_policy_matrix();
        let delegated = super::TassadarImportPolicyRequest {
            request_id: String::from("delegated"),
            import_ref: String::from("sandbox.math_eval"),
            allow_external_delegation: true,
            sandbox_descriptor_present: true,
            challenge_receipt_present: true,
        };
        let selected = negotiate_tassadar_import_policy(&delegated, &matrix).expect("selection");
        assert_eq!(
            selected.decision,
            TassadarImportPolicyDecision::DelegatedSandbox
        );

        let refused = super::TassadarImportPolicyRequest {
            request_id: String::from("refused"),
            import_ref: String::from("host.fs_write"),
            allow_external_delegation: true,
            sandbox_descriptor_present: true,
            challenge_receipt_present: true,
        };
        assert_eq!(
            negotiate_tassadar_import_policy(&refused, &matrix),
            Err(TassadarImportPolicyRefusalReason::UnsafeImportClass)
        );
    }

    #[test]
    fn import_policy_matrix_report_matches_committed_truth() {
        let expected = build_tassadar_import_policy_matrix_report();
        let committed =
            load_tassadar_import_policy_matrix_report(tassadar_import_policy_matrix_report_path())
                .expect("committed report");

        assert_eq!(committed, expected);
    }
}
