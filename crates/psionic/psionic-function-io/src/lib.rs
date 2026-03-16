//! Public function artifact IO above `psionic-ir` and `psionic-compiler`.
//!
//! This crate keeps Psionic's graph-first function export story separate from
//! model-family checkpoint portability, while still exposing bounded
//! compatibility shells such as `.mlxfn` on top of the native artifact model.

mod mlxfn;

use std::{fs, path::Path};

use psionic_compiler::{
    CompileTraceFamilyError, CompileTraceFamilyIdentity, CompilerArtifacts,
    DeploymentArtifactContract, DeploymentArtifactContractError, compile_trace_family_identity,
};
use psionic_core::{PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope};
use psionic_ir::{ExportableGraphContract, Graph, GraphExportContractError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use mlxfn::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "public function artifact IO above psionic-ir and psionic-compiler";

/// Public native function artifact format supported by Psionic today.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunctionArtifactFormat {
    /// One Psionic-native `.psifn` artifact.
    Psifn,
}

impl FunctionArtifactFormat {
    /// Returns the stable artifact label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Psifn => "psifn",
        }
    }
}

/// Direction of one function-artifact boundary crossing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionArtifactDirection {
    /// Psionic function state was exported into one artifact.
    Export,
    /// Psionic function state was imported from one artifact.
    Import,
}

/// Optional compiler-visible bundle embedded inside one native function artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionCompileBundle {
    /// Lowered compiler artifacts bound to the export-safe graph.
    pub artifacts: CompilerArtifacts,
    /// Optional trace-family identity for replay-safe shapeless or concrete compile posture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_family_identity: Option<CompileTraceFamilyIdentity>,
    /// Optional deployment bundle contract bound to the graph export contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_artifact_contract: Option<DeploymentArtifactContract>,
}

impl FunctionCompileBundle {
    /// Builds one optional compile bundle and validates any embedded trace-family
    /// identity against the compiler artifacts.
    pub fn new(
        artifacts: CompilerArtifacts,
        trace_family_identity: Option<CompileTraceFamilyIdentity>,
        deployment_artifact_contract: Option<DeploymentArtifactContract>,
    ) -> Result<Self, FunctionIoError> {
        let bundle = Self {
            artifacts,
            trace_family_identity,
            deployment_artifact_contract,
        };
        bundle.validate_trace_family()?;
        Ok(bundle)
    }

    /// Returns stable signature lines suitable for fixtures and audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!(
                "compiler_artifacts_digest={}",
                self.artifacts.stable_digest()
            ),
            format!(
                "cache_identity_digest={}",
                self.artifacts.cache_identity.stable_digest()
            ),
        ];
        if let Some(trace_family_identity) = &self.trace_family_identity {
            lines.push(format!(
                "trace_family_digest={}",
                trace_family_identity.stable_digest()
            ));
        }
        if let Some(deployment_artifact_contract) = &self.deployment_artifact_contract {
            lines.push(format!(
                "deployment_artifact_digest={}",
                deployment_artifact_contract.artifact_digest
            ));
        }
        lines
    }

    fn validate_trace_family(&self) -> Result<(), FunctionIoError> {
        let Some(trace_family_identity) = &self.trace_family_identity else {
            return Ok(());
        };
        let expected =
            compile_trace_family_identity(&self.artifacts, trace_family_identity.shape_mode)?;
        if expected != *trace_family_identity {
            return Err(FunctionIoError::TraceFamilyIdentityMismatch {
                expected: expected.trace_family_digest,
                actual: trace_family_identity.trace_family_digest.clone(),
            });
        }
        Ok(())
    }

    fn validate_against_export_contract(
        &self,
        export_contract: &ExportableGraphContract,
    ) -> Result<(), FunctionIoError> {
        if self.artifacts.compiled.plan.graph_digest != export_contract.source_graph_digest {
            return Err(FunctionIoError::CompilerGraphDigestMismatch {
                expected: export_contract.source_graph_digest.clone(),
                actual: self.artifacts.compiled.plan.graph_digest.clone(),
            });
        }
        self.validate_trace_family()?;
        if let Some(deployment_artifact_contract) = &self.deployment_artifact_contract {
            let expected = self.artifacts.deployment_artifact_contract(
                export_contract,
                deployment_artifact_contract.artifact_label.clone(),
                deployment_artifact_contract.artifact_format,
            )?;
            if expected != *deployment_artifact_contract {
                return Err(FunctionIoError::DeploymentArtifactMismatch {
                    expected: expected.artifact_digest,
                    actual: deployment_artifact_contract.artifact_digest.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Stable digest-bound Psionic-native function artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionArtifact {
    /// Stable artifact format.
    pub format: FunctionArtifactFormat,
    /// Stable schema version.
    pub schema_version: u32,
    /// Canonical graph carried by the artifact.
    pub graph: Graph,
    /// Export-safe graph contract bound to the graph.
    pub export_contract: ExportableGraphContract,
    /// Optional compiler-visible replay bundle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_bundle: Option<FunctionCompileBundle>,
    /// Stable digest over the whole artifact contract.
    pub artifact_digest: String,
}

impl FunctionArtifact {
    /// Builds one native function artifact from an export-safe graph.
    pub fn from_graph(
        graph: &Graph,
        entrypoint: impl Into<String>,
    ) -> Result<Self, FunctionIoError> {
        let export_contract = graph.exportable_graph_contract(entrypoint.into())?;
        Self::from_parts(graph.clone(), export_contract, None)
    }

    /// Builds one native function artifact from an export-safe graph plus an
    /// optional compiler-visible replay bundle.
    pub fn from_graph_with_compile_bundle(
        graph: &Graph,
        entrypoint: impl Into<String>,
        compile_bundle: FunctionCompileBundle,
    ) -> Result<Self, FunctionIoError> {
        let export_contract = graph.exportable_graph_contract(entrypoint.into())?;
        Self::from_parts(graph.clone(), export_contract, Some(compile_bundle))
    }

    /// Attaches one compile bundle to an existing function artifact.
    pub fn with_compile_bundle(
        mut self,
        compile_bundle: FunctionCompileBundle,
    ) -> Result<Self, FunctionIoError> {
        compile_bundle.validate_against_export_contract(&self.export_contract)?;
        self.compile_bundle = Some(compile_bundle);
        self.artifact_digest = self.compute_artifact_digest();
        Ok(self)
    }

    /// Returns the stable artifact digest.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        self.artifact_digest.clone()
    }

    /// Returns the exported entrypoint name.
    #[must_use]
    pub fn entrypoint(&self) -> &str {
        self.export_contract.entrypoint.as_str()
    }

    /// Returns stable signature lines suitable for fixtures and audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("format={}", self.format.label()),
            format!("entrypoint={}", self.entrypoint()),
            format!("graph_digest={}", self.graph.stable_digest()),
            format!(
                "export_contract_digest={}",
                self.export_contract.contract_digest
            ),
        ];
        if let Some(compile_bundle) = &self.compile_bundle {
            lines.extend(compile_bundle.stable_signature_lines());
        }
        lines.push(format!("artifact_digest={}", self.artifact_digest));
        lines
    }

    /// Validates the carried graph, export contract, optional compile bundle,
    /// and artifact digest.
    pub fn validate(&self) -> Result<(), FunctionIoError> {
        self.validate_payload()?;
        let expected = self.compute_artifact_digest();
        if expected != self.artifact_digest {
            return Err(FunctionIoError::ArtifactDigestMismatch {
                expected,
                actual: self.artifact_digest.clone(),
            });
        }
        Ok(())
    }

    fn from_parts(
        graph: Graph,
        export_contract: ExportableGraphContract,
        compile_bundle: Option<FunctionCompileBundle>,
    ) -> Result<Self, FunctionIoError> {
        let mut artifact = Self {
            format: FunctionArtifactFormat::Psifn,
            schema_version: 1,
            graph,
            export_contract,
            compile_bundle,
            artifact_digest: String::new(),
        };
        artifact.validate_payload()?;
        artifact.artifact_digest = artifact.compute_artifact_digest();
        Ok(artifact)
    }

    fn validate_payload(&self) -> Result<(), FunctionIoError> {
        self.export_contract.validate_against_graph(&self.graph)?;
        let expected_contract = self
            .graph
            .exportable_graph_contract(self.export_contract.entrypoint.clone())?;
        if expected_contract != self.export_contract {
            return Err(FunctionIoError::ExportContractMismatch {
                expected: expected_contract.contract_digest,
                actual: self.export_contract.contract_digest.clone(),
            });
        }
        if let Some(compile_bundle) = &self.compile_bundle {
            compile_bundle.validate_against_export_contract(&self.export_contract)?;
        }
        Ok(())
    }

    fn compute_artifact_digest(&self) -> String {
        stable_function_artifact_digest(
            self.format,
            self.schema_version,
            self.graph.stable_digest().as_str(),
            self.export_contract.contract_digest.as_str(),
            self.compile_bundle.as_ref(),
        )
    }
}

/// Stable receipt emitted for one native function artifact import or export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionArtifactReceipt {
    /// Artifact family that crossed the boundary.
    pub format: FunctionArtifactFormat,
    /// Whether the boundary was import or export.
    pub direction: FunctionArtifactDirection,
    /// Stable SHA-256 digest over the encoded bytes.
    pub artifact_sha256: String,
    /// Exact artifact length in bytes.
    pub artifact_bytes: usize,
    /// Stable exported entrypoint.
    pub entrypoint: String,
    /// Stable source graph digest.
    pub graph_digest: String,
    /// Stable export-contract digest.
    pub export_contract_digest: String,
    /// Stable function-artifact digest.
    pub function_artifact_digest: String,
    /// Stable plan-cache identity digest when compile artifacts are attached.
    pub cache_identity_digest: Option<String>,
    /// Stable trace-family digest when one is attached.
    pub trace_family_digest: Option<String>,
    /// Stable deployment-artifact digest when one is attached.
    pub deployment_artifact_digest: Option<String>,
}

impl FunctionArtifactReceipt {
    fn new(
        direction: FunctionArtifactDirection,
        bytes: &[u8],
        artifact: &FunctionArtifact,
    ) -> Self {
        Self {
            format: artifact.format,
            direction,
            artifact_sha256: hex::encode(Sha256::digest(bytes)),
            artifact_bytes: bytes.len(),
            entrypoint: artifact.entrypoint().to_string(),
            graph_digest: artifact.graph.stable_digest(),
            export_contract_digest: artifact.export_contract.contract_digest.clone(),
            function_artifact_digest: artifact.stable_digest(),
            cache_identity_digest: artifact
                .compile_bundle
                .as_ref()
                .map(|bundle| bundle.artifacts.cache_identity.stable_digest()),
            trace_family_digest: artifact.compile_bundle.as_ref().and_then(|bundle| {
                bundle
                    .trace_family_identity
                    .as_ref()
                    .map(CompileTraceFamilyIdentity::stable_digest)
            }),
            deployment_artifact_digest: artifact.compile_bundle.as_ref().and_then(|bundle| {
                bundle
                    .deployment_artifact_contract
                    .as_ref()
                    .map(|contract| contract.artifact_digest.clone())
            }),
        }
    }
}

/// Error returned by the public function-artifact IO layer.
#[derive(Debug, Error)]
pub enum FunctionIoError {
    /// One exportable-graph contract operation failed.
    #[error(transparent)]
    GraphExport(#[from] GraphExportContractError),
    /// One compile trace-family operation failed.
    #[error(transparent)]
    TraceFamily(#[from] CompileTraceFamilyError),
    /// One deployment-artifact contract operation failed.
    #[error(transparent)]
    DeploymentArtifact(#[from] DeploymentArtifactContractError),
    /// The graph and export contract no longer describe the same stable contract.
    #[error(
        "function artifact export contract mismatch: expected `{expected}` but found `{actual}`"
    )]
    ExportContractMismatch {
        /// Expected export-contract digest.
        expected: String,
        /// Actual export-contract digest.
        actual: String,
    },
    /// The compile bundle was built from a different graph.
    #[error("function artifact compile bundle expected graph `{expected}` but found `{actual}`")]
    CompilerGraphDigestMismatch {
        /// Expected graph digest.
        expected: String,
        /// Actual graph digest.
        actual: String,
    },
    /// The embedded trace-family identity drifted from the compiler artifacts.
    #[error("function artifact trace-family mismatch: expected `{expected}` but found `{actual}`")]
    TraceFamilyIdentityMismatch {
        /// Expected trace-family digest.
        expected: String,
        /// Actual trace-family digest.
        actual: String,
    },
    /// The embedded deployment-artifact contract drifted from the compiler artifacts.
    #[error(
        "function artifact deployment bundle mismatch: expected `{expected}` but found `{actual}`"
    )]
    DeploymentArtifactMismatch {
        /// Expected deployment-artifact digest.
        expected: String,
        /// Actual deployment-artifact digest.
        actual: String,
    },
    /// The aggregate artifact digest drifted from the serialized contract contents.
    #[error("function artifact digest mismatch: expected `{expected}` but found `{actual}`")]
    ArtifactDigestMismatch {
        /// Expected artifact digest.
        expected: String,
        /// Actual artifact digest.
        actual: String,
    },
    /// JSON serialization or deserialization failed.
    #[error("function artifact {operation} failed: {message}")]
    Serialization {
        /// Operation label.
        operation: &'static str,
        /// Plain-language error.
        message: String,
    },
    /// One filesystem operation failed.
    #[error("path `{path}` failed during {operation}: {message}")]
    Io {
        /// Path that failed.
        path: String,
        /// Operation label.
        operation: &'static str,
        /// Plain-language error.
        message: String,
    },
}

impl FunctionIoError {
    /// Returns the canonical refusal when the failure belongs to one explicit
    /// compatibility or serialization boundary.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::GraphExport(error) => error.refusal(),
            Self::TraceFamily(error) => Some(error.refusal()),
            Self::DeploymentArtifact(error) => error.refusal(),
            Self::ExportContractMismatch { .. }
            | Self::CompilerGraphDigestMismatch { .. }
            | Self::TraceFamilyIdentityMismatch { .. }
            | Self::DeploymentArtifactMismatch { .. }
            | Self::ArtifactDigestMismatch { .. }
            | Self::Serialization { .. } => Some(PsionicRefusal::new(
                PsionicRefusalCode::SerializationIncompatibility,
                PsionicRefusalScope::Graph,
                self.to_string(),
            )),
            Self::Io { .. } => None,
        }
    }
}

/// Encodes one native function artifact into bytes plus a stable receipt.
pub fn encode_function_artifact(
    artifact: &FunctionArtifact,
) -> Result<(Vec<u8>, FunctionArtifactReceipt), FunctionIoError> {
    artifact.validate()?;
    let bytes =
        serde_json::to_vec_pretty(artifact).map_err(|error| FunctionIoError::Serialization {
            operation: "encode",
            message: error.to_string(),
        })?;
    let receipt = FunctionArtifactReceipt::new(FunctionArtifactDirection::Export, &bytes, artifact);
    Ok((bytes, receipt))
}

/// Decodes one native function artifact from bytes plus a stable receipt.
pub fn decode_function_artifact(
    bytes: &[u8],
) -> Result<(FunctionArtifact, FunctionArtifactReceipt), FunctionIoError> {
    let artifact = serde_json::from_slice::<FunctionArtifact>(bytes).map_err(|error| {
        FunctionIoError::Serialization {
            operation: "decode",
            message: error.to_string(),
        }
    })?;
    artifact.validate()?;
    let receipt = FunctionArtifactReceipt::new(FunctionArtifactDirection::Import, bytes, &artifact);
    Ok((artifact, receipt))
}

/// Saves one native function artifact to the provided path and returns the
/// stable export receipt.
pub fn save_function_artifact_path(
    artifact: &FunctionArtifact,
    path: impl AsRef<Path>,
) -> Result<FunctionArtifactReceipt, FunctionIoError> {
    let path = path.as_ref();
    let (bytes, receipt) = encode_function_artifact(artifact)?;
    fs::write(path, bytes).map_err(|error| FunctionIoError::Io {
        path: path.display().to_string(),
        operation: "write",
        message: error.to_string(),
    })?;
    Ok(receipt)
}

/// Loads one native function artifact from the provided path and returns the
/// artifact plus a stable import receipt.
pub fn load_function_artifact_path(
    path: impl AsRef<Path>,
) -> Result<(FunctionArtifact, FunctionArtifactReceipt), FunctionIoError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| FunctionIoError::Io {
        path: path.display().to_string(),
        operation: "read",
        message: error.to_string(),
    })?;
    decode_function_artifact(&bytes)
}

fn stable_function_artifact_digest(
    format: FunctionArtifactFormat,
    schema_version: u32,
    graph_digest: &str,
    export_contract_digest: &str,
    compile_bundle: Option<&FunctionCompileBundle>,
) -> String {
    let mut lines = vec![
        format!("schema_version={schema_version}"),
        format!("format={}", format.label()),
        format!("graph_digest={graph_digest}"),
        format!("export_contract_digest={export_contract_digest}"),
    ];
    if let Some(compile_bundle) = compile_bundle {
        lines.extend(compile_bundle.stable_signature_lines());
    }
    digest_lines(lines)
}

fn digest_lines(lines: Vec<String>) -> String {
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use super::{
        FunctionArtifact, FunctionArtifactDirection, FunctionCompileBundle, FunctionIoError,
        decode_function_artifact, encode_function_artifact, load_function_artifact_path,
        save_function_artifact_path,
    };
    use psionic_compiler::{
        CompileShapeMode, DeploymentArtifactFormat, compile_graph_artifacts,
        compile_trace_family_identity,
    };
    use psionic_core::{DType, Device, PsionicRefusalCode, Shape};
    use psionic_ir::{Graph, GraphBuilder};
    use tempfile::NamedTempFile;

    fn ensure(condition: bool, message: impl Into<String>) -> Result<(), FunctionIoError> {
        if condition {
            Ok(())
        } else {
            Err(FunctionIoError::Serialization {
                operation: "test assertion",
                message: message.into(),
            })
        }
    }

    fn ensure_eq<T>(actual: &T, expected: &T, label: &str) -> Result<(), FunctionIoError>
    where
        T: Debug + PartialEq,
    {
        if actual == expected {
            Ok(())
        } else {
            Err(FunctionIoError::Serialization {
                operation: "test assertion",
                message: format!("{label} mismatch: actual={actual:?} expected={expected:?}"),
            })
        }
    }

    fn seeded_export_safe_graph() -> Result<Graph, FunctionIoError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let left = builder.input("left", Shape::new(vec![2, 3]), DType::F32);
        let right = builder.input("right", Shape::new(vec![2, 3]), DType::F32);
        let shifted =
            builder
                .add(&left, &right)
                .map_err(|error| FunctionIoError::Serialization {
                    operation: "build seeded export-safe graph",
                    message: error.to_string(),
                })?;
        let reduced = builder.reduce_sum_axis(&shifted, 1).map_err(|error| {
            FunctionIoError::Serialization {
                operation: "build seeded export-safe graph",
                message: error.to_string(),
            }
        })?;
        Ok(builder.finish(vec![reduced]))
    }

    fn seeded_opaque_graph() -> Result<Graph, FunctionIoError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![1, 4]), DType::F32);
        let weight = builder
            .constant_f32(Shape::new(vec![4]), vec![1.0, 1.0, 1.0, 1.0])
            .map_err(|error| FunctionIoError::Serialization {
                operation: "build seeded opaque graph",
                message: error.to_string(),
            })?;
        let normed = builder.rms_norm(&input, &weight, 1e-5).map_err(|error| {
            FunctionIoError::Serialization {
                operation: "build seeded opaque graph",
                message: error.to_string(),
            }
        })?;
        Ok(builder.finish(vec![normed]))
    }

    #[test]
    fn native_function_artifact_roundtrips_graph_only() -> Result<(), FunctionIoError> {
        let graph = seeded_export_safe_graph()?;
        let artifact = FunctionArtifact::from_graph(&graph, "main")?;
        let (bytes, export_receipt) = encode_function_artifact(&artifact)?;
        let (decoded, import_receipt) = decode_function_artifact(&bytes)?;

        ensure_eq(&decoded, &artifact, "graph-only artifact roundtrip")?;
        ensure_eq(
            &export_receipt.direction,
            &FunctionArtifactDirection::Export,
            "graph-only export direction",
        )?;
        ensure_eq(
            &import_receipt.direction,
            &FunctionArtifactDirection::Import,
            "graph-only import direction",
        )?;
        ensure_eq(
            &export_receipt.function_artifact_digest,
            &import_receipt.function_artifact_digest,
            "graph-only digest",
        )?;
        ensure_eq(
            &export_receipt.graph_digest,
            &graph.stable_digest(),
            "graph-only source graph digest",
        )?;
        ensure(
            export_receipt.cache_identity_digest.is_none(),
            "graph-only receipt unexpectedly carried a compile cache digest",
        )?;

        Ok(())
    }

    #[test]
    fn native_function_artifact_roundtrips_compiled_bundle_and_path_io()
    -> Result<(), FunctionIoError> {
        let graph = seeded_export_safe_graph()?;
        let export_contract = graph.exportable_graph_contract("main")?;
        let artifacts =
            compile_graph_artifacts(&graph).map_err(|error| FunctionIoError::Serialization {
                operation: "compile seeded export-safe graph",
                message: error.to_string(),
            })?;
        let trace_family_identity =
            compile_trace_family_identity(&artifacts, CompileShapeMode::ShapelessTraceFamily)?;
        let deployment_artifact_contract = artifacts.deployment_artifact_contract(
            &export_contract,
            "seeded_bundle",
            DeploymentArtifactFormat::ExecutionPlanBundle,
        )?;
        let compile_bundle = FunctionCompileBundle::new(
            artifacts,
            Some(trace_family_identity.clone()),
            Some(deployment_artifact_contract.clone()),
        )?;
        let artifact =
            FunctionArtifact::from_graph_with_compile_bundle(&graph, "main", compile_bundle)?;
        let file = NamedTempFile::new().map_err(|error| FunctionIoError::Io {
            path: String::from("<tempfile>"),
            operation: "create",
            message: error.to_string(),
        })?;
        let export_receipt = save_function_artifact_path(&artifact, file.path())?;
        let (loaded, import_receipt) = load_function_artifact_path(file.path())?;

        ensure_eq(&loaded, &artifact, "compiled artifact roundtrip")?;
        ensure_eq(
            &export_receipt.trace_family_digest,
            &Some(trace_family_identity.stable_digest()),
            "compiled artifact trace-family digest",
        )?;
        ensure_eq(
            &import_receipt.deployment_artifact_digest,
            &Some(deployment_artifact_contract.artifact_digest),
            "compiled artifact deployment digest",
        )?;
        ensure(
            import_receipt.cache_identity_digest.is_some(),
            "compiled artifact receipt unexpectedly omitted the cache-identity digest",
        )?;

        Ok(())
    }

    #[test]
    fn native_function_artifact_refuses_opaque_graphs() -> Result<(), FunctionIoError> {
        let graph = seeded_opaque_graph()?;
        let error = match FunctionArtifact::from_graph(&graph, "main") {
            Ok(_) => {
                return Err(FunctionIoError::Serialization {
                    operation: "opaque graph refusal test",
                    message: String::from(
                        "opaque backend-extension graphs unexpectedly exported as native artifacts",
                    ),
                });
            }
            Err(error) => error,
        };
        let refusal = error
            .refusal()
            .ok_or_else(|| FunctionIoError::Serialization {
                operation: "opaque graph refusal test",
                message: String::from("opaque export failure did not expose one canonical refusal"),
            })?;
        ensure_eq(
            &refusal.code,
            &PsionicRefusalCode::UnsupportedOp,
            "opaque graph refusal code",
        )?;
        Ok(())
    }

    #[test]
    fn native_function_artifact_detects_tampered_trace_family_identity()
    -> Result<(), FunctionIoError> {
        let graph = seeded_export_safe_graph()?;
        let export_contract = graph.exportable_graph_contract("main")?;
        let artifacts =
            compile_graph_artifacts(&graph).map_err(|error| FunctionIoError::Serialization {
                operation: "compile seeded export-safe graph",
                message: error.to_string(),
            })?;
        let mut trace_family_identity =
            compile_trace_family_identity(&artifacts, CompileShapeMode::ShapelessTraceFamily)?;
        let deployment_artifact_contract = artifacts.deployment_artifact_contract(
            &export_contract,
            "seeded_bundle",
            DeploymentArtifactFormat::ExecutionPlanBundle,
        )?;
        let compile_bundle = FunctionCompileBundle::new(
            artifacts,
            Some(trace_family_identity.clone()),
            Some(deployment_artifact_contract),
        )?;
        let mut artifact =
            FunctionArtifact::from_graph_with_compile_bundle(&graph, "main", compile_bundle)?;
        trace_family_identity.trace_family_digest = String::from("tampered");
        if let Some(compile_bundle) = artifact.compile_bundle.as_mut() {
            compile_bundle.trace_family_identity = Some(trace_family_identity);
        }

        let error = match encode_function_artifact(&artifact) {
            Ok(_) => {
                return Err(FunctionIoError::Serialization {
                    operation: "tampered trace-family test",
                    message: String::from(
                        "tampered trace-family identity unexpectedly encoded successfully",
                    ),
                });
            }
            Err(error) => error,
        };
        let refusal = error
            .refusal()
            .ok_or_else(|| FunctionIoError::Serialization {
                operation: "tampered trace-family test",
                message: String::from(
                    "trace-family drift did not map to serialization incompatibility",
                ),
            })?;
        ensure(
            matches!(error, FunctionIoError::TraceFamilyIdentityMismatch { .. }),
            "tampered trace-family drift did not return TraceFamilyIdentityMismatch",
        )?;
        ensure_eq(
            &refusal.code,
            &PsionicRefusalCode::SerializationIncompatibility,
            "tampered trace-family refusal code",
        )?;

        Ok(())
    }
}
