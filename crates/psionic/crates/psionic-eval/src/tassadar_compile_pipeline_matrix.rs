use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_compiler::{
    TassadarCSourceArtifactBundlePipelineError, TassadarWasmModuleArtifactBundle,
    TassadarWasmTextArtifactBundlePipelineError, TassadarWasmTextCompileConfig,
    TassadarWasmTextCompileReceipt, compile_tassadar_c_source_to_artifact_bundle,
    compile_tassadar_wasm_text_to_artifact_bundle,
};
use psionic_ir::parse_tassadar_normalized_wasm_module;
use psionic_runtime::{
    TASSADAR_CANONICAL_C_SOURCE_REF, TassadarCToWasmCompileConfig, TassadarCompileRefusal,
    TassadarCompilerToolchainIdentity, TassadarCpuReferenceRunner, TassadarExecutionRefusal,
    TassadarProgramSourceKind, TassadarTraceAbi, TassadarWasmBinarySummary, TassadarWasmProfile,
    tassadar_canonical_c_source_path,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_COMPILE_PIPELINE_MATRIX_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_compile_pipeline_matrix_report.json";

/// Repo-facing status for one real source-to-Wasm-to-Tassadar case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCompilePipelineMatrixCaseStatus {
    /// The source compiled and lowered exactly through the current bounded lane.
    LoweredExact,
    /// The source compiled to Wasm, but the current lowering boundary refused.
    LoweringRefused,
    /// The source never reached Wasm because the compile step refused first.
    CompileRefused,
}

/// One repo-facing real compile-pipeline case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompilePipelineMatrixCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable source-family identifier.
    pub source_family_id: String,
    /// Stable source-language family.
    pub source_kind: TassadarProgramSourceKind,
    /// Repo-relative source ref.
    pub source_ref: String,
    /// Stable source digest.
    pub source_digest: String,
    /// Current runtime target Wasm profile.
    pub target_wasm_profile_id: String,
    /// Current runtime target trace ABI.
    pub target_trace_abi_id: String,
    /// Current runtime target trace ABI version.
    pub target_trace_abi_version: u16,
    /// Stable compile surface identifier.
    pub compile_surface_id: String,
    /// Stable digest over the compile configuration.
    pub compile_config_digest: String,
    /// Stable feature list declared by the compile configuration.
    pub compile_pipeline_features: Vec<String>,
    /// Compiler/toolchain identity observed for the case.
    pub toolchain_identity: TassadarCompilerToolchainIdentity,
    /// Stable digest over the toolchain identity.
    pub toolchain_digest: String,
    /// Stable digest over the full compile receipt.
    pub compile_receipt_digest: String,
    /// Stable case status.
    pub status: TassadarCompilePipelineMatrixCaseStatus,
    /// Repo-relative Wasm output ref when the compile succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_binary_ref: Option<String>,
    /// Stable digest over the Wasm output when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_binary_digest: Option<String>,
    /// Structural summary over the compiled Wasm output when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_binary_summary: Option<TassadarWasmBinarySummary>,
    /// Stable digest over the normalized module when the Wasm output exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_module_digest: Option<String>,
    /// Stable lowered bundle digest when lowering succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_bundle_digest: Option<String>,
    /// Export names lowered exactly when lowering succeeded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lowered_export_names: Vec<String>,
    /// Lowered artifact digests when lowering succeeded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lowered_artifact_digests: Vec<String>,
    /// Exact outputs by export when lowering succeeded.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub exact_outputs_by_export: BTreeMap<String, Vec<i32>>,
    /// Compile refusal kind when the compile refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_refusal_kind: Option<String>,
    /// Compile refusal detail when the compile refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_refusal_detail: Option<String>,
    /// Lowering refusal kind when the lowering refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lowering_refusal_kind: Option<String>,
    /// Lowering refusal detail when the lowering refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lowering_refusal_detail: Option<String>,
}

/// Committed report over the real source-to-Wasm-to-Tassadar matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompilePipelineMatrixReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Ordered source-family identifiers covered by the report.
    pub source_family_ids: Vec<String>,
    /// Stable refs used to generate the report.
    pub generated_from_refs: Vec<String>,
    /// Ordered compile-pipeline cases.
    pub cases: Vec<TassadarCompilePipelineMatrixCase>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarCompilePipelineMatrixReport {
    fn new(cases: Vec<TassadarCompilePipelineMatrixCase>) -> Self {
        let mut report = Self {
            schema_version: TASSADAR_COMPILE_PIPELINE_MATRIX_SCHEMA_VERSION,
            report_id: String::from("tassadar.compile_pipeline_matrix_report.v1"),
            source_family_ids: vec![
                String::from("wasm_text.multi_export_arithmetic"),
                String::from("wasm_text.memory_lookup"),
                String::from("wasm_text.param_abi"),
                String::from("c_source.toolchain_unavailable"),
            ],
            generated_from_refs: vec![
                String::from("fixtures/tassadar/sources/tassadar_multi_export_kernel.wat"),
                String::from("fixtures/tassadar/sources/tassadar_memory_lookup_kernel.wat"),
                String::from("fixtures/tassadar/sources/tassadar_param_abi_kernel.wat"),
                String::from(TASSADAR_CANONICAL_C_SOURCE_REF),
            ],
            cases,
            claim_boundary: String::from(
                "this report records a small real compile-pipeline matrix through the current bounded Tassadar Wasm-module lane: exact Wasm-text multi-export arithmetic and memory-lookup fixtures, one explicit Wasm-text parameter-ABI lowering refusal, and one typed missing-toolchain refusal on the existing C-source path; it does not imply arbitrary frontend closure, arbitrary Wasm lowering, or general module execution completeness",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_compile_pipeline_matrix_report|", &report);
        report
    }
}

/// Report build failures for the compile-pipeline matrix.
#[derive(Debug, Error)]
pub enum TassadarCompilePipelineMatrixReportError {
    /// Failed to read one committed source fixture.
    #[error("failed to read source fixture `{path}`: {error}")]
    ReadSource {
        /// Source path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to read one compiled Wasm output after a successful compile.
    #[error("failed to read compiled Wasm for case `{case_id}` at `{path}`: {error}")]
    ReadCompiledWasm {
        /// Case identifier.
        case_id: String,
        /// Wasm path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Parsing the compiled Wasm into normalized form failed unexpectedly.
    #[error("failed to normalize compiled Wasm for case `{case_id}`: {error}")]
    NormalizeCompiledWasm {
        /// Case identifier.
        case_id: String,
        /// Parser error.
        error: psionic_ir::TassadarNormalizedWasmModuleError,
    },
    /// Replaying one lowered export failed unexpectedly.
    #[error("failed to replay lowered export `{export_name}` for case `{case_id}`: {error}")]
    Replay {
        /// Case identifier.
        case_id: String,
        /// Export name.
        export_name: String,
        /// Runtime refusal.
        error: TassadarExecutionRefusal,
    },
    /// One lowered export diverged from its committed execution manifest.
    #[error(
        "lowered export `{export_name}` for case `{case_id}` diverged from its execution manifest"
    )]
    ExactnessMismatch {
        /// Case identifier.
        case_id: String,
        /// Export name.
        export_name: String,
    },
    /// Failed to create the report output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to write the report.
    #[error("failed to write compile-pipeline matrix report `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
}

/// Builds the committed real compile-pipeline matrix report.
pub fn build_tassadar_compile_pipeline_matrix_report()
-> Result<TassadarCompilePipelineMatrixReport, TassadarCompilePipelineMatrixReportError> {
    Ok(TassadarCompilePipelineMatrixReport::new(vec![
        build_wasm_text_case(
            "wasm_text_multi_export_arithmetic_exact",
            "wasm_text.multi_export_arithmetic",
            "fixtures/tassadar/sources/tassadar_multi_export_kernel.wat",
            fixture_source_path("tassadar_multi_export_kernel.wat"),
            repo_root().join("fixtures/tassadar/wasm/tassadar_multi_export_kernel.wasm"),
            TassadarWasmTextCompileConfig::canonical_multi_export_kernel(),
        )?,
        build_wasm_text_case(
            "wasm_text_memory_lookup_exact",
            "wasm_text.memory_lookup",
            "fixtures/tassadar/sources/tassadar_memory_lookup_kernel.wat",
            fixture_source_path("tassadar_memory_lookup_kernel.wat"),
            repo_root().join("fixtures/tassadar/wasm/tassadar_memory_lookup_kernel.wasm"),
            TassadarWasmTextCompileConfig::canonical_memory_lookup_kernel(),
        )?,
        build_wasm_text_case(
            "wasm_text_param_abi_lowering_refusal",
            "wasm_text.param_abi",
            "fixtures/tassadar/sources/tassadar_param_abi_kernel.wat",
            fixture_source_path("tassadar_param_abi_kernel.wat"),
            repo_root().join("fixtures/tassadar/wasm/tassadar_param_abi_kernel.wasm"),
            TassadarWasmTextCompileConfig::canonical_param_abi_kernel(),
        )?,
        build_c_toolchain_refusal_case()?,
    ]))
}

/// Returns the canonical absolute path for the committed compile-pipeline matrix report.
pub fn tassadar_compile_pipeline_matrix_report_path() -> PathBuf {
    repo_root().join(TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF)
}

/// Writes the committed compile-pipeline matrix report.
pub fn write_tassadar_compile_pipeline_matrix_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarCompilePipelineMatrixReport, TassadarCompilePipelineMatrixReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarCompilePipelineMatrixReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_compile_pipeline_matrix_report()?;
    let bytes = serde_json::to_vec_pretty(&report)
        .expect("compile-pipeline matrix report should serialize");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarCompilePipelineMatrixReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_wasm_text_case(
    case_id: &str,
    source_family_id: &str,
    source_ref: &str,
    source_path: PathBuf,
    output_wasm_path: PathBuf,
    compile_config: TassadarWasmTextCompileConfig,
) -> Result<TassadarCompilePipelineMatrixCase, TassadarCompilePipelineMatrixReportError> {
    let source_bytes = fs::read(&source_path).map_err(|error| {
        TassadarCompilePipelineMatrixReportError::ReadSource {
            path: source_path.display().to_string(),
            error,
        }
    })?;
    let source_text =
        std::str::from_utf8(&source_bytes).expect("Wasm-text fixture should be valid UTF-8");
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    let trace_abi = TassadarTraceAbi::article_i32_compute_v1();

    match compile_tassadar_wasm_text_to_artifact_bundle(
        source_ref,
        source_text,
        &output_wasm_path,
        &compile_config,
        &profile,
    ) {
        Ok(pipeline) => Ok(TassadarCompilePipelineMatrixCase {
            case_id: String::from(case_id),
            source_family_id: String::from(source_family_id),
            source_kind: pipeline.compile_receipt.source_identity.source_kind,
            source_ref: String::from(source_ref),
            source_digest: pipeline
                .compile_receipt
                .source_identity
                .source_digest
                .clone(),
            target_wasm_profile_id: profile.profile_id.clone(),
            target_trace_abi_id: trace_abi.abi_id.clone(),
            target_trace_abi_version: trace_abi.schema_version,
            compile_surface_id: String::from("wasm_text"),
            compile_config_digest: compile_config.stable_digest(),
            compile_pipeline_features: compile_config.pipeline_features(),
            toolchain_digest: pipeline.compile_receipt.toolchain_identity.stable_digest(),
            toolchain_identity: pipeline.compile_receipt.toolchain_identity.clone(),
            compile_receipt_digest: pipeline.compile_receipt.receipt_digest.clone(),
            status: TassadarCompilePipelineMatrixCaseStatus::LoweredExact,
            wasm_binary_ref: pipeline
                .compile_receipt
                .wasm_binary_ref()
                .map(ToOwned::to_owned),
            wasm_binary_digest: pipeline
                .compile_receipt
                .wasm_binary_digest()
                .map(ToOwned::to_owned),
            wasm_binary_summary: pipeline.compile_receipt.wasm_binary_summary().cloned(),
            normalized_module_digest: Some(
                pipeline
                    .artifact_bundle
                    .normalized_module
                    .module_digest
                    .clone(),
            ),
            artifact_bundle_digest: Some(pipeline.artifact_bundle.bundle_digest.clone()),
            lowered_export_names: pipeline
                .artifact_bundle
                .lowered_exports
                .iter()
                .map(|artifact| artifact.export_name.clone())
                .collect(),
            lowered_artifact_digests: pipeline
                .artifact_bundle
                .lowered_exports
                .iter()
                .map(|artifact| artifact.program_artifact.artifact_digest.clone())
                .collect(),
            exact_outputs_by_export: exact_outputs_by_export(case_id, &pipeline.artifact_bundle)?,
            compile_refusal_kind: None,
            compile_refusal_detail: None,
            lowering_refusal_kind: None,
            lowering_refusal_detail: None,
        }),
        Err(TassadarWasmTextArtifactBundlePipelineError::LoweringRefused {
            compile_receipt,
            error,
            ..
        }) => Ok(TassadarCompilePipelineMatrixCase {
            case_id: String::from(case_id),
            source_family_id: String::from(source_family_id),
            source_kind: compile_receipt.source_identity.source_kind,
            source_ref: String::from(source_ref),
            source_digest: compile_receipt.source_identity.source_digest.clone(),
            target_wasm_profile_id: profile.profile_id.clone(),
            target_trace_abi_id: trace_abi.abi_id.clone(),
            target_trace_abi_version: trace_abi.schema_version,
            compile_surface_id: String::from("wasm_text"),
            compile_config_digest: compile_config.stable_digest(),
            compile_pipeline_features: compile_config.pipeline_features(),
            toolchain_digest: compile_receipt.toolchain_identity.stable_digest(),
            toolchain_identity: compile_receipt.toolchain_identity.clone(),
            compile_receipt_digest: compile_receipt.receipt_digest.clone(),
            status: TassadarCompilePipelineMatrixCaseStatus::LoweringRefused,
            wasm_binary_ref: compile_receipt.wasm_binary_ref().map(ToOwned::to_owned),
            wasm_binary_digest: compile_receipt.wasm_binary_digest().map(ToOwned::to_owned),
            wasm_binary_summary: compile_receipt.wasm_binary_summary().cloned(),
            normalized_module_digest: Some(read_normalized_module_digest(
                case_id,
                &output_wasm_path,
            )?),
            artifact_bundle_digest: None,
            lowered_export_names: Vec::new(),
            lowered_artifact_digests: Vec::new(),
            exact_outputs_by_export: BTreeMap::new(),
            compile_refusal_kind: None,
            compile_refusal_detail: None,
            lowering_refusal_kind: Some(String::from(error.kind_slug())),
            lowering_refusal_detail: Some(error.to_string()),
        }),
        Err(TassadarWasmTextArtifactBundlePipelineError::CompileRefused {
            compile_receipt,
            refusal,
        }) => compile_refused_from_wasm_text(
            case_id,
            source_family_id,
            source_ref,
            &profile,
            &trace_abi,
            compile_config,
            compile_receipt,
            &refusal,
        ),
        Err(TassadarWasmTextArtifactBundlePipelineError::WriteCompiledWasm {
            path,
            message,
            ..
        }) => Err(TassadarCompilePipelineMatrixReportError::ReadCompiledWasm {
            case_id: String::from(case_id),
            path,
            error: std::io::Error::other(message),
        }),
    }
}

fn build_c_toolchain_refusal_case()
-> Result<TassadarCompilePipelineMatrixCase, TassadarCompilePipelineMatrixReportError> {
    let source_path = tassadar_canonical_c_source_path();
    let source_bytes = fs::read(&source_path).map_err(|error| {
        TassadarCompilePipelineMatrixReportError::ReadSource {
            path: source_path.display().to_string(),
            error,
        }
    })?;
    let mut compile_config = TassadarCToWasmCompileConfig::canonical_micro_wasm_kernel();
    compile_config.compiler_binary = String::from("clang-not-installed-for-tassadar");
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    let trace_abi = TassadarTraceAbi::article_i32_compute_v1();
    let temp_output = std::env::temp_dir().join("psionic-tassadar-missing-toolchain.wasm");

    match compile_tassadar_c_source_to_artifact_bundle(
        TASSADAR_CANONICAL_C_SOURCE_REF,
        &source_bytes,
        &temp_output,
        &compile_config,
        &profile,
    ) {
        Err(TassadarCSourceArtifactBundlePipelineError::CompileRefused {
            compile_receipt,
            refusal,
        }) => Ok(TassadarCompilePipelineMatrixCase {
            case_id: String::from("c_missing_toolchain_refusal"),
            source_family_id: String::from("c_source.toolchain_unavailable"),
            source_kind: compile_receipt.source_identity.source_kind,
            source_ref: String::from(TASSADAR_CANONICAL_C_SOURCE_REF),
            source_digest: compile_receipt.source_identity.source_digest.clone(),
            target_wasm_profile_id: profile.profile_id.clone(),
            target_trace_abi_id: trace_abi.abi_id.clone(),
            target_trace_abi_version: trace_abi.schema_version,
            compile_surface_id: String::from("c_source"),
            compile_config_digest: compile_config.stable_digest(),
            compile_pipeline_features: compile_config.pipeline_features(),
            toolchain_digest: compile_receipt.toolchain_identity.stable_digest(),
            toolchain_identity: compile_receipt.toolchain_identity.clone(),
            compile_receipt_digest: compile_receipt.receipt_digest.clone(),
            status: TassadarCompilePipelineMatrixCaseStatus::CompileRefused,
            wasm_binary_ref: None,
            wasm_binary_digest: None,
            wasm_binary_summary: None,
            normalized_module_digest: None,
            artifact_bundle_digest: None,
            lowered_export_names: Vec::new(),
            lowered_artifact_digests: Vec::new(),
            exact_outputs_by_export: BTreeMap::new(),
            compile_refusal_kind: Some(String::from(refusal.kind_slug())),
            compile_refusal_detail: Some(refusal.to_string()),
            lowering_refusal_kind: None,
            lowering_refusal_detail: None,
        }),
        Err(other) => Err(TassadarCompilePipelineMatrixReportError::ReadCompiledWasm {
            case_id: String::from("c_missing_toolchain_refusal"),
            path: temp_output.display().to_string(),
            error: std::io::Error::other(format!("unexpected C pipeline outcome: {other}")),
        }),
        Ok(_) => Err(TassadarCompilePipelineMatrixReportError::ReadCompiledWasm {
            case_id: String::from("c_missing_toolchain_refusal"),
            path: temp_output.display().to_string(),
            error: std::io::Error::other("expected missing-toolchain refusal, got exact lowering"),
        }),
    }
}

fn compile_refused_from_wasm_text(
    case_id: &str,
    source_family_id: &str,
    source_ref: &str,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
    compile_config: TassadarWasmTextCompileConfig,
    compile_receipt: TassadarWasmTextCompileReceipt,
    refusal: &TassadarCompileRefusal,
) -> Result<TassadarCompilePipelineMatrixCase, TassadarCompilePipelineMatrixReportError> {
    Ok(TassadarCompilePipelineMatrixCase {
        case_id: String::from(case_id),
        source_family_id: String::from(source_family_id),
        source_kind: compile_receipt.source_identity.source_kind,
        source_ref: String::from(source_ref),
        source_digest: compile_receipt.source_identity.source_digest.clone(),
        target_wasm_profile_id: profile.profile_id.clone(),
        target_trace_abi_id: trace_abi.abi_id.clone(),
        target_trace_abi_version: trace_abi.schema_version,
        compile_surface_id: String::from("wasm_text"),
        compile_config_digest: compile_config.stable_digest(),
        compile_pipeline_features: compile_config.pipeline_features(),
        toolchain_digest: compile_receipt.toolchain_identity.stable_digest(),
        toolchain_identity: compile_receipt.toolchain_identity.clone(),
        compile_receipt_digest: compile_receipt.receipt_digest.clone(),
        status: TassadarCompilePipelineMatrixCaseStatus::CompileRefused,
        wasm_binary_ref: None,
        wasm_binary_digest: None,
        wasm_binary_summary: None,
        normalized_module_digest: None,
        artifact_bundle_digest: None,
        lowered_export_names: Vec::new(),
        lowered_artifact_digests: Vec::new(),
        exact_outputs_by_export: BTreeMap::new(),
        compile_refusal_kind: Some(String::from(refusal.kind_slug())),
        compile_refusal_detail: Some(refusal.to_string()),
        lowering_refusal_kind: None,
        lowering_refusal_detail: None,
    })
}

fn exact_outputs_by_export(
    case_id: &str,
    bundle: &TassadarWasmModuleArtifactBundle,
) -> Result<BTreeMap<String, Vec<i32>>, TassadarCompilePipelineMatrixReportError> {
    let mut outputs = BTreeMap::new();
    for artifact in &bundle.lowered_exports {
        let execution =
            TassadarCpuReferenceRunner::for_program(&artifact.program_artifact.validated_program)
                .expect("lowered program should select a runner")
                .execute(&artifact.program_artifact.validated_program)
                .map_err(|error| TassadarCompilePipelineMatrixReportError::Replay {
                    case_id: String::from(case_id),
                    export_name: artifact.export_name.clone(),
                    error,
                })?;
        if execution.outputs != artifact.execution_manifest.expected_outputs {
            return Err(
                TassadarCompilePipelineMatrixReportError::ExactnessMismatch {
                    case_id: String::from(case_id),
                    export_name: artifact.export_name.clone(),
                },
            );
        }
        outputs.insert(artifact.export_name.clone(), execution.outputs);
    }
    Ok(outputs)
}

fn read_normalized_module_digest(
    case_id: &str,
    wasm_path: &Path,
) -> Result<String, TassadarCompilePipelineMatrixReportError> {
    let wasm_bytes = fs::read(wasm_path).map_err(|error| {
        TassadarCompilePipelineMatrixReportError::ReadCompiledWasm {
            case_id: String::from(case_id),
            path: wasm_path.display().to_string(),
            error,
        }
    })?;
    let normalized = parse_tassadar_normalized_wasm_module(&wasm_bytes).map_err(|error| {
        TassadarCompilePipelineMatrixReportError::NormalizeCompiledWasm {
            case_id: String::from(case_id),
            error,
        }
    })?;
    Ok(normalized.module_digest)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn fixture_source_path(file_name: &str) -> PathBuf {
    repo_root()
        .join("fixtures")
        .join("tassadar")
        .join("sources")
        .join(file_name)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF, build_tassadar_compile_pipeline_matrix_report,
        tassadar_compile_pipeline_matrix_report_path,
        write_tassadar_compile_pipeline_matrix_report,
    };

    #[test]
    fn compile_pipeline_matrix_report_captures_exact_and_refusal_cases() {
        let report = build_tassadar_compile_pipeline_matrix_report()
            .expect("compile-pipeline matrix report should build");
        assert_eq!(report.cases.len(), 4);
        assert!(report.cases.iter().any(|case| case.case_id
            == "wasm_text_multi_export_arithmetic_exact"
            && case.status == super::TassadarCompilePipelineMatrixCaseStatus::LoweredExact));
        assert!(
            report
                .cases
                .iter()
                .any(|case| case.case_id == "wasm_text_memory_lookup_exact"
                    && case.status == super::TassadarCompilePipelineMatrixCaseStatus::LoweredExact)
        );
        assert!(report.cases.iter().any(|case| case.case_id
            == "wasm_text_param_abi_lowering_refusal"
            && case.lowering_refusal_kind.as_deref() == Some("unsupported_param_count")));
        assert!(
            report
                .cases
                .iter()
                .any(|case| case.case_id == "c_missing_toolchain_refusal"
                    && case.compile_refusal_kind.as_deref() == Some("toolchain_unavailable"))
        );
    }

    #[test]
    fn compile_pipeline_matrix_report_matches_committed_truth() {
        let report = build_tassadar_compile_pipeline_matrix_report()
            .expect("compile-pipeline matrix report should build");
        let path = tassadar_compile_pipeline_matrix_report_path();
        let bytes = std::fs::read(&path).expect("committed report should exist");
        let persisted: super::TassadarCompilePipelineMatrixReport =
            serde_json::from_slice(&bytes).expect("committed report should decode");
        assert_eq!(
            persisted,
            report,
            "run the example to refresh `{}`",
            path.display()
        );
    }

    #[test]
    fn write_compile_pipeline_matrix_report_persists_current_truth() {
        let output_path = std::env::temp_dir().join("tassadar_compile_pipeline_matrix_report.json");
        let report = write_tassadar_compile_pipeline_matrix_report(&output_path)
            .expect("compile-pipeline matrix report should write");
        let bytes = std::fs::read(&output_path).expect("persisted report should exist");
        let persisted: super::TassadarCompilePipelineMatrixReport =
            serde_json::from_slice(&bytes).expect("persisted report should decode");
        assert_eq!(persisted, report);
        std::fs::remove_file(&output_path).expect("temp report should be removable");
        assert_eq!(
            PathBuf::from(TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF)
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_compile_pipeline_matrix_report.json")
        );
    }
}
