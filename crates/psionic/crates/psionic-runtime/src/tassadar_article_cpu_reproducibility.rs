use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ExecutionProofRuntimeIdentity, TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_ROOT_REF,
    TassadarArticleRuntimeCloseoutError, TassadarArticleRuntimeFloorStatus,
    build_tassadar_article_runtime_closeout_bundle,
};

const TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleCpuFamily {
    X86_64,
    Aarch64,
    Other,
}

impl TassadarArticleCpuFamily {
    #[must_use]
    pub const fn machine_class_id(self) -> &'static str {
        match self {
            Self::X86_64 => "host_cpu_x86_64",
            Self::Aarch64 => "host_cpu_aarch64",
            Self::Other => "other_host_cpu",
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleCpuMachineClassStatus {
    SupportedMeasuredCurrentHost,
    SupportedDeclaredClass,
    UnsupportedMeasuredCurrentHost,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleCpuExactnessPosture {
    ExactCurrentHost,
    ExactRequiredDeclaredClass,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleCpuThroughputFloorPosture {
    PassedCurrentHost,
    RequiredDeclaredClass,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleCpuReproducibilityRow {
    pub machine_class_id: String,
    pub cpu_family: TassadarArticleCpuFamily,
    pub os_scope: String,
    pub status: TassadarArticleCpuMachineClassStatus,
    pub runtime_backend: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_identity: Option<ExecutionProofRuntimeIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_identity_digest: Option<String>,
    pub exactness_posture: TassadarArticleCpuExactnessPosture,
    pub exact_horizon_count: u32,
    pub workload_family_ids: Vec<String>,
    pub floor_posture: TassadarArticleCpuThroughputFloorPosture,
    pub throughput_floor_steps_per_second: f64,
    pub floor_pass_count: u32,
    pub floor_refusal_count: u32,
    pub slowest_workload_horizon_id: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleCpuReproducibilityMatrix {
    pub schema_version: u16,
    pub matrix_id: String,
    pub generated_from_refs: Vec<String>,
    pub current_host_machine_class_id: String,
    pub current_host_os: String,
    pub current_host_arch: String,
    pub rows: Vec<TassadarArticleCpuReproducibilityRow>,
    pub supported_machine_class_ids: Vec<String>,
    pub unsupported_machine_class_ids: Vec<String>,
    pub current_host_measured_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub matrix_digest: String,
}

impl TassadarArticleCpuReproducibilityMatrix {
    fn new(rows: Vec<TassadarArticleCpuReproducibilityRow>) -> Self {
        let current_host_cpu_family = current_host_cpu_family();
        let current_host_machine_class_id = String::from(current_host_cpu_family.machine_class_id());
        let supported_machine_class_ids = vec![
            String::from(TassadarArticleCpuFamily::Aarch64.machine_class_id()),
            String::from(TassadarArticleCpuFamily::X86_64.machine_class_id()),
        ];
        let unsupported_machine_class_ids = vec![String::from(
            TassadarArticleCpuFamily::Other.machine_class_id(),
        )];
        let current_host_measured_green = rows.iter().any(|row| {
            row.machine_class_id == current_host_machine_class_id
                && matches!(
                    row.status,
                    TassadarArticleCpuMachineClassStatus::SupportedMeasuredCurrentHost
                )
                && row.exactness_posture == TassadarArticleCpuExactnessPosture::ExactCurrentHost
                && row.floor_posture
                    == TassadarArticleCpuThroughputFloorPosture::PassedCurrentHost
        });
        let mut matrix = Self {
            schema_version: TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_SCHEMA_VERSION,
            matrix_id: String::from("tassadar.article_cpu_reproducibility_matrix.v1"),
            generated_from_refs: vec![format!(
                "{}/{}",
                TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_ROOT_REF,
                crate::TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_BUNDLE_FILE
            )],
            current_host_machine_class_id,
            current_host_os: std::env::consts::OS.to_string(),
            current_host_arch: std::env::consts::ARCH.to_string(),
            rows,
            supported_machine_class_ids,
            unsupported_machine_class_ids,
            current_host_measured_green,
            claim_boundary: String::from(
                "this matrix closes cross-machine CPU reproducibility only for the Rust-only article runtime path on the canonical host CPU families `x86_64` and `aarch64`. It records current-host exactness and floor posture plus explicit unsupported classes, and it does not imply universal portability, non-CPU backend closure, or success on unmeasured toolchain or machine combinations",
            ),
            summary: String::new(),
            matrix_digest: String::new(),
        };
        matrix.summary = format!(
            "Rust-only article CPU reproducibility now names current_host=`{}` with green_current_host={}, supported_classes={}, unsupported_classes={}.",
            matrix.current_host_machine_class_id,
            matrix.current_host_measured_green,
            matrix.supported_machine_class_ids.len(),
            matrix.unsupported_machine_class_ids.len(),
        );
        matrix.matrix_digest =
            stable_digest(b"psionic_tassadar_article_cpu_reproducibility_matrix|", &matrix);
        matrix
    }
}

pub fn build_tassadar_article_cpu_reproducibility_matrix()
-> Result<TassadarArticleCpuReproducibilityMatrix, TassadarArticleRuntimeCloseoutError> {
    let bundle = build_tassadar_article_runtime_closeout_bundle()?;
    let current_host_cpu_family = current_host_cpu_family();
    let current_host_supported = matches!(
        current_host_cpu_family,
        TassadarArticleCpuFamily::Aarch64 | TassadarArticleCpuFamily::X86_64
    );
    let slowest_receipt = bundle
        .horizon_receipts
        .iter()
        .max_by(|left, right| {
            left.exact_step_count
                .cmp(&right.exact_step_count)
                .then_with(|| left.horizon_id.cmp(&right.horizon_id))
        })
        .expect("article runtime closeout bundle should contain at least one receipt");
    let runtime_identity = slowest_receipt
        .evidence_bundle
        .runtime_manifest
        .runtime_identity
        .clone();
    let runtime_identity_digest = stable_digest(
        b"psionic_tassadar_article_cpu_reproducibility_runtime_identity|",
        &runtime_identity,
    );
    let exact_current_host = bundle.exact_horizon_count == bundle.horizon_receipts.len() as u32;
    let floor_current_host = bundle.floor_refusal_count == 0
        && bundle.floor_pass_count
            == bundle
                .horizon_receipts
                .iter()
                .filter(|receipt| receipt.floor_status == TassadarArticleRuntimeFloorStatus::Passed)
                .count() as u32;

    let rows = [
        TassadarArticleCpuFamily::Aarch64,
        TassadarArticleCpuFamily::X86_64,
        TassadarArticleCpuFamily::Other,
    ]
    .into_iter()
    .map(|cpu_family| {
        build_row(
            cpu_family,
            current_host_cpu_family,
            current_host_supported,
            &runtime_identity,
            &runtime_identity_digest,
            bundle.exact_horizon_count,
            bundle.floor_pass_count,
            bundle.floor_refusal_count,
            bundle.workload_family_ids.clone(),
            slowest_receipt.horizon_id.clone(),
            slowest_receipt.throughput_floor_steps_per_second,
            exact_current_host,
            floor_current_host,
        )
    })
    .collect::<Vec<_>>();

    Ok(TassadarArticleCpuReproducibilityMatrix::new(rows))
}

fn build_row(
    cpu_family: TassadarArticleCpuFamily,
    current_host_cpu_family: TassadarArticleCpuFamily,
    current_host_supported: bool,
    runtime_identity: &ExecutionProofRuntimeIdentity,
    runtime_identity_digest: &str,
    exact_horizon_count: u32,
    floor_pass_count: u32,
    floor_refusal_count: u32,
    workload_family_ids: Vec<String>,
    slowest_workload_horizon_id: String,
    throughput_floor_steps_per_second: f64,
    exact_current_host: bool,
    floor_current_host: bool,
) -> TassadarArticleCpuReproducibilityRow {
    let is_current_host = cpu_family == current_host_cpu_family;
    let is_supported_class = matches!(
        cpu_family,
        TassadarArticleCpuFamily::Aarch64 | TassadarArticleCpuFamily::X86_64
    );
    let status = if is_current_host && is_supported_class {
        TassadarArticleCpuMachineClassStatus::SupportedMeasuredCurrentHost
    } else if is_current_host {
        TassadarArticleCpuMachineClassStatus::UnsupportedMeasuredCurrentHost
    } else if is_supported_class {
        TassadarArticleCpuMachineClassStatus::SupportedDeclaredClass
    } else {
        TassadarArticleCpuMachineClassStatus::Unsupported
    };
    let exactness_posture = if is_current_host && current_host_supported && exact_current_host {
        TassadarArticleCpuExactnessPosture::ExactCurrentHost
    } else if !is_current_host && is_supported_class {
        TassadarArticleCpuExactnessPosture::ExactRequiredDeclaredClass
    } else {
        TassadarArticleCpuExactnessPosture::Refused
    };
    let floor_posture = if is_current_host && current_host_supported && floor_current_host {
        TassadarArticleCpuThroughputFloorPosture::PassedCurrentHost
    } else if !is_current_host && is_supported_class {
        TassadarArticleCpuThroughputFloorPosture::RequiredDeclaredClass
    } else {
        TassadarArticleCpuThroughputFloorPosture::Refused
    };
    let note = match status {
        TassadarArticleCpuMachineClassStatus::SupportedMeasuredCurrentHost => format!(
            "measured current host `{}` reproduced the Rust-only article runtime closeout on the direct CPU lane with exact_horizons={} and floor_passes={}.",
            std::env::consts::ARCH, exact_horizon_count, floor_pass_count,
        ),
        TassadarArticleCpuMachineClassStatus::SupportedDeclaredClass => format!(
            "declared supported CPU family `{}` inherits the same direct CPU runtime contract and required throughput floor, but it remains an operator portability requirement rather than a measured fact in this checkout.",
            cpu_family.as_str(),
        ),
        TassadarArticleCpuMachineClassStatus::UnsupportedMeasuredCurrentHost => format!(
            "measured current host `{}` falls outside the canonical supported CPU families; portability publication must refuse even if local runtime execution is otherwise possible.",
            std::env::consts::ARCH,
        ),
        TassadarArticleCpuMachineClassStatus::Unsupported => String::from(
            "this machine class is outside the canonical x86_64/aarch64 CPU families and must remain refused until a later issue widens the portability envelope with matching evidence.",
        ),
    };
    TassadarArticleCpuReproducibilityRow {
        machine_class_id: String::from(cpu_family.machine_class_id()),
        cpu_family,
        os_scope: if is_current_host {
            std::env::consts::OS.to_string()
        } else {
            String::from("any")
        },
        status,
        runtime_backend: runtime_identity.runtime_backend.clone(),
        runtime_identity: is_current_host.then(|| runtime_identity.clone()),
        runtime_identity_digest: is_current_host.then(|| runtime_identity_digest.to_string()),
        exactness_posture,
        exact_horizon_count,
        workload_family_ids,
        floor_posture,
        throughput_floor_steps_per_second,
        floor_pass_count,
        floor_refusal_count,
        slowest_workload_horizon_id,
        note,
    }
}

#[must_use]
pub fn current_host_cpu_family() -> TassadarArticleCpuFamily {
    match std::env::consts::ARCH {
        "x86_64" => TassadarArticleCpuFamily::X86_64,
        "aarch64" => TassadarArticleCpuFamily::Aarch64,
        _ => TassadarArticleCpuFamily::Other,
    }
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
        TassadarArticleCpuExactnessPosture, TassadarArticleCpuFamily,
        TassadarArticleCpuMachineClassStatus, TassadarArticleCpuThroughputFloorPosture,
        build_tassadar_article_cpu_reproducibility_matrix, current_host_cpu_family,
    };

    #[test]
    fn article_cpu_reproducibility_matrix_names_supported_and_unsupported_machine_classes() {
        let matrix = build_tassadar_article_cpu_reproducibility_matrix().expect("matrix");

        assert_eq!(matrix.rows.len(), 3);
        assert_eq!(
            matrix.supported_machine_class_ids,
            vec![
                String::from("host_cpu_aarch64"),
                String::from("host_cpu_x86_64"),
            ]
        );
        assert_eq!(
            matrix.unsupported_machine_class_ids,
            vec![String::from("other_host_cpu")]
        );
    }

    #[test]
    fn article_cpu_reproducibility_matrix_marks_current_host_explicitly() {
        let matrix = build_tassadar_article_cpu_reproducibility_matrix().expect("matrix");
        let current_host = current_host_cpu_family();
        let row = matrix
            .rows
            .iter()
            .find(|row| row.machine_class_id == current_host.machine_class_id())
            .expect("current host row");

        match current_host {
            TassadarArticleCpuFamily::Aarch64 | TassadarArticleCpuFamily::X86_64 => {
                assert_eq!(
                    row.status,
                    TassadarArticleCpuMachineClassStatus::SupportedMeasuredCurrentHost
                );
                assert_eq!(
                    row.exactness_posture,
                    TassadarArticleCpuExactnessPosture::ExactCurrentHost
                );
                assert_eq!(
                    row.floor_posture,
                    TassadarArticleCpuThroughputFloorPosture::PassedCurrentHost
                );
                assert!(row.runtime_identity.is_some());
                assert!(row.runtime_identity_digest.is_some());
            }
            TassadarArticleCpuFamily::Other => {
                assert_eq!(
                    row.status,
                    TassadarArticleCpuMachineClassStatus::UnsupportedMeasuredCurrentHost
                );
                assert_eq!(row.exactness_posture, TassadarArticleCpuExactnessPosture::Refused);
                assert_eq!(
                    row.floor_posture,
                    TassadarArticleCpuThroughputFloorPosture::Refused
                );
            }
        }
    }
}
