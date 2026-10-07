//! Document extraction contracts for legal-agent benchmarks.
//!
//! The day-one implementation keeps the Rust-owned interface, receipts, and
//! native text extraction in this crate. Heavy Office/PDF/email extraction is
//! represented as pinned sandboxed external adapter specs so runner work can
//! attach real command execution without changing downstream receipt schemas.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::{stable_json_digest, ArtifactKind, Metadata, SourceArtifact};

pub const LEGAL_BENCHMARK_EXTRACTION_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionOutputFormat {
    PlainText,
    Markdown,
    JsonText,
    StructuredJson,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArtifactExtractionPolicy {
    pub schema_version: u16,
    pub policy_id: String,
    pub max_input_bytes: u64,
    pub output_format: ExtractionOutputFormat,
    pub external_tools_allowed: bool,
    pub sandbox_required: bool,
    pub fail_on_warning: bool,
    pub extractor_allowlist: Vec<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

impl Default for ArtifactExtractionPolicy {
    fn default() -> Self {
        Self {
            schema_version: LEGAL_BENCHMARK_EXTRACTION_SCHEMA_VERSION,
            policy_id: "legal_benchmark.extraction.default.v1".to_string(),
            max_input_bytes: 64 * 1024 * 1024,
            output_format: ExtractionOutputFormat::PlainText,
            external_tools_allowed: true,
            sandbox_required: true,
            fail_on_warning: false,
            extractor_allowlist: Vec::new(),
            metadata: Metadata::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionFailureKind {
    UnsupportedArtifact,
    InputTooLarge,
    DecodeError,
    JsonParseError,
    PolicyDenied,
    ExternalToolUnavailable,
    ExternalCommandFailed,
    SandboxUnavailable,
    Timeout,
    IoError,
    InternalError,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtractionWarning {
    pub warning_code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtractionCoverage {
    pub input_byte_count: u64,
    pub output_byte_count: u64,
    pub output_char_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages_extracted: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheets_extracted: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_part_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_parts_extracted: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExtractionReceipt {
    pub schema_version: u16,
    pub receipt_id: String,
    pub source_artifact_id: String,
    pub source_artifact_hash: String,
    pub source_artifact_path: String,
    pub input_media_type: String,
    pub input_byte_size: u64,
    pub extractor_name: String,
    pub extractor_version: String,
    pub command_or_crate_version: String,
    pub output_format: ExtractionOutputFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_artifact_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_byte_size: Option<u64>,
    pub coverage: ExtractionCoverage,
    pub warnings: Vec<ExtractionWarning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<ExtractionFailureKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_receipt_ref: Option<String>,
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedArtifact {
    pub artifact: SourceArtifact,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactExtractionResult {
    pub receipt: ExtractionReceipt,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extracted_artifact: Option<ExtractedArtifact>,
}

pub trait ArtifactExtractor {
    fn extractor_name(&self) -> &'static str;
    fn extractor_version(&self) -> &'static str;
    fn command_or_crate_version(&self) -> &'static str;
    fn supports(&self, artifact: &SourceArtifact) -> bool;

    fn extract(
        &self,
        artifact: &SourceArtifact,
        bytes: &[u8],
        policy: &ArtifactExtractionPolicy,
    ) -> ArtifactExtractionResult;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeTextArtifactExtractor;

impl ArtifactExtractor for NativeTextArtifactExtractor {
    fn extractor_name(&self) -> &'static str {
        "psionic.native_text"
    }

    fn extractor_version(&self) -> &'static str {
        "1.0.0"
    }

    fn command_or_crate_version(&self) -> &'static str {
        "std::str+serde_json"
    }

    fn supports(&self, artifact: &SourceArtifact) -> bool {
        is_native_text_artifact(artifact)
    }

    fn extract(
        &self,
        artifact: &SourceArtifact,
        bytes: &[u8],
        policy: &ArtifactExtractionPolicy,
    ) -> ArtifactExtractionResult {
        let started = Instant::now();
        if !extractor_allowed(self.extractor_name(), policy) {
            return failure_result(
                artifact,
                self.extractor_name(),
                self.extractor_version(),
                self.command_or_crate_version(),
                policy.output_format,
                bytes.len(),
                started,
                ExtractionFailureKind::PolicyDenied,
                "extractor is not allowed by extraction policy",
                Vec::new(),
            );
        }
        if bytes.len() > usize::try_from(policy.max_input_bytes).unwrap_or(usize::MAX) {
            return failure_result(
                artifact,
                self.extractor_name(),
                self.extractor_version(),
                self.command_or_crate_version(),
                policy.output_format,
                bytes.len(),
                started,
                ExtractionFailureKind::InputTooLarge,
                "input exceeds extraction policy byte limit",
                Vec::new(),
            );
        }

        let extension = file_extension(artifact.relative_path.as_str());
        let extracted = match extension.as_deref() {
            Some("json") => match serde_json::from_slice::<Value>(bytes) {
                Ok(value) => match serde_json::to_string_pretty(&value) {
                    Ok(mut text) => {
                        text.push('\n');
                        Ok(text)
                    }
                    Err(error) => Err((
                        ExtractionFailureKind::JsonParseError,
                        format!("failed to render JSON text: {error}"),
                    )),
                },
                Err(error) => Err((
                    ExtractionFailureKind::JsonParseError,
                    format!("failed to parse JSON input: {error}"),
                )),
            },
            _ => match std::str::from_utf8(bytes) {
                Ok(text) => Ok(normalize_newlines(text)),
                Err(error) => Err((
                    ExtractionFailureKind::DecodeError,
                    format!("failed to decode UTF-8 text: {error}"),
                )),
            },
        };

        match extracted {
            Ok(text) => success_result(
                artifact,
                self.extractor_name(),
                self.extractor_version(),
                self.command_or_crate_version(),
                policy.output_format,
                bytes.len(),
                started,
                text,
                Vec::new(),
                None,
            ),
            Err((failure_kind, detail)) => failure_result(
                artifact,
                self.extractor_name(),
                self.extractor_version(),
                self.command_or_crate_version(),
                policy.output_format,
                bytes.len(),
                started,
                failure_kind,
                detail,
                Vec::new(),
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExternalExtractorSpec {
    pub extractor_name: String,
    pub extractor_version: String,
    pub command_or_crate_version: String,
    pub container_image: String,
    pub command: Vec<String>,
    pub supported_extensions: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeOfficeXmlArtifactExtractor;

impl ArtifactExtractor for NativeOfficeXmlArtifactExtractor {
    fn extractor_name(&self) -> &'static str {
        "psionic.native_office_xml"
    }

    fn extractor_version(&self) -> &'static str {
        "1.0.0"
    }

    fn command_or_crate_version(&self) -> &'static str {
        "zip+xml_text"
    }

    fn supports(&self, artifact: &SourceArtifact) -> bool {
        file_extension(artifact.relative_path.as_str())
            .as_deref()
            .is_some_and(|extension| matches!(extension, "docx" | "pptx" | "xlsx"))
    }

    fn extract(
        &self,
        artifact: &SourceArtifact,
        bytes: &[u8],
        policy: &ArtifactExtractionPolicy,
    ) -> ArtifactExtractionResult {
        let started = Instant::now();
        if !extractor_allowed(self.extractor_name(), policy) {
            return failure_result(
                artifact,
                self.extractor_name(),
                self.extractor_version(),
                self.command_or_crate_version(),
                policy.output_format,
                bytes.len(),
                started,
                ExtractionFailureKind::PolicyDenied,
                "extractor is not allowed by extraction policy",
                Vec::new(),
            );
        }
        if bytes.len() > usize::try_from(policy.max_input_bytes).unwrap_or(usize::MAX) {
            return failure_result(
                artifact,
                self.extractor_name(),
                self.extractor_version(),
                self.command_or_crate_version(),
                policy.output_format,
                bytes.len(),
                started,
                ExtractionFailureKind::InputTooLarge,
                "input exceeds extraction policy byte limit",
                Vec::new(),
            );
        }

        let extension = file_extension(artifact.relative_path.as_str());
        let extracted = match extension.as_deref() {
            Some("docx") => extract_docx_text(bytes),
            Some("pptx") => extract_pptx_text(bytes),
            Some("xlsx") => extract_xlsx_text(bytes),
            _ => Err(String::from("unsupported Office XML extension")),
        };

        match extracted {
            Ok(text) => success_result(
                artifact,
                self.extractor_name(),
                self.extractor_version(),
                self.command_or_crate_version(),
                policy.output_format,
                bytes.len(),
                started,
                text,
                vec![ExtractionWarning {
                    warning_code: String::from("native_office_xml_lossy"),
                    message: String::from(
                        "native Office XML extraction preserves text only; layout, comments, formulas, and tracked-change semantics are not authoritative",
                    ),
                    metadata: Metadata::new(),
                }],
                None,
            ),
            Err(detail) => failure_result(
                artifact,
                self.extractor_name(),
                self.extractor_version(),
                self.command_or_crate_version(),
                policy.output_format,
                bytes.len(),
                started,
                ExtractionFailureKind::DecodeError,
                detail,
                Vec::new(),
            ),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeEmailArtifactExtractor;

impl ArtifactExtractor for NativeEmailArtifactExtractor {
    fn extractor_name(&self) -> &'static str {
        "psionic.native_email"
    }

    fn extractor_version(&self) -> &'static str {
        "1.0.0"
    }

    fn command_or_crate_version(&self) -> &'static str {
        "std::str+lossy_mime"
    }

    fn supports(&self, artifact: &SourceArtifact) -> bool {
        file_extension(artifact.relative_path.as_str())
            .as_deref()
            .is_some_and(|extension| extension == "eml")
            || artifact.media_type == "message/rfc822"
    }

    fn extract(
        &self,
        artifact: &SourceArtifact,
        bytes: &[u8],
        policy: &ArtifactExtractionPolicy,
    ) -> ArtifactExtractionResult {
        let started = Instant::now();
        if !extractor_allowed(self.extractor_name(), policy) {
            return failure_result(
                artifact,
                self.extractor_name(),
                self.extractor_version(),
                self.command_or_crate_version(),
                policy.output_format,
                bytes.len(),
                started,
                ExtractionFailureKind::PolicyDenied,
                "extractor is not allowed by extraction policy",
                Vec::new(),
            );
        }
        if bytes.len() > usize::try_from(policy.max_input_bytes).unwrap_or(usize::MAX) {
            return failure_result(
                artifact,
                self.extractor_name(),
                self.extractor_version(),
                self.command_or_crate_version(),
                policy.output_format,
                bytes.len(),
                started,
                ExtractionFailureKind::InputTooLarge,
                "input exceeds extraction policy byte limit",
                Vec::new(),
            );
        }

        let text = extract_eml_text(bytes);
        success_result(
            artifact,
            self.extractor_name(),
            self.extractor_version(),
            self.command_or_crate_version(),
            policy.output_format,
            bytes.len(),
            started,
            text,
            vec![ExtractionWarning {
                warning_code: String::from("native_email_lossy"),
                message: String::from(
                    "native EML extraction preserves readable headers and body text only; attachments and MIME encodings require the sandboxed extractor",
                ),
                metadata: Metadata::new(),
            }],
            None,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExternalCommandArtifactExtractor {
    pub spec: ExternalExtractorSpec,
}

impl ArtifactExtractor for ExternalCommandArtifactExtractor {
    fn extractor_name(&self) -> &'static str {
        "psionic.sandboxed_external"
    }

    fn extractor_version(&self) -> &'static str {
        "1.0.0"
    }

    fn command_or_crate_version(&self) -> &'static str {
        "external_command_spec"
    }

    fn supports(&self, artifact: &SourceArtifact) -> bool {
        let extension = file_extension(artifact.relative_path.as_str());
        extension.is_some_and(|extension| {
            self.spec
                .supported_extensions
                .iter()
                .any(|supported| supported == &extension)
        })
    }

    fn extract(
        &self,
        artifact: &SourceArtifact,
        bytes: &[u8],
        policy: &ArtifactExtractionPolicy,
    ) -> ArtifactExtractionResult {
        let started = Instant::now();
        if !extractor_allowed(self.spec.extractor_name.as_str(), policy) {
            return failure_result(
                artifact,
                self.spec.extractor_name.as_str(),
                self.spec.extractor_version.as_str(),
                self.spec.command_or_crate_version.as_str(),
                policy.output_format,
                bytes.len(),
                started,
                ExtractionFailureKind::PolicyDenied,
                "extractor is not allowed by extraction policy",
                Vec::new(),
            );
        }
        if bytes.len() > usize::try_from(policy.max_input_bytes).unwrap_or(usize::MAX) {
            return failure_result(
                artifact,
                self.spec.extractor_name.as_str(),
                self.spec.extractor_version.as_str(),
                self.spec.command_or_crate_version.as_str(),
                policy.output_format,
                bytes.len(),
                started,
                ExtractionFailureKind::InputTooLarge,
                "input exceeds extraction policy byte limit",
                Vec::new(),
            );
        }
        let warning = ExtractionWarning {
            warning_code: "external_adapter_not_executed".to_string(),
            message: format!(
                "external extractor `{}` requires a live sandbox command executor",
                self.spec.extractor_name
            ),
            metadata: BTreeMap::new(),
        };
        if !policy.external_tools_allowed {
            return failure_result(
                artifact,
                self.spec.extractor_name.as_str(),
                self.spec.extractor_version.as_str(),
                self.spec.command_or_crate_version.as_str(),
                policy.output_format,
                bytes.len(),
                started,
                ExtractionFailureKind::PolicyDenied,
                "external extraction is disabled by policy",
                vec![warning],
            );
        }
        failure_result(
            artifact,
            self.spec.extractor_name.as_str(),
            self.spec.extractor_version.as_str(),
            self.spec.command_or_crate_version.as_str(),
            policy.output_format,
            bytes.len(),
            started,
            ExtractionFailureKind::ExternalToolUnavailable,
            "sandboxed external extractor is declared but no command executor was attached",
            vec![warning],
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArtifactExtractorRegistry {
    pub external_extractors: Vec<ExternalCommandArtifactExtractor>,
}

impl Default for ArtifactExtractorRegistry {
    fn default() -> Self {
        Self {
            external_extractors: default_external_extractors(),
        }
    }
}

impl ArtifactExtractorRegistry {
    pub fn extract(
        &self,
        artifact: &SourceArtifact,
        bytes: &[u8],
        policy: &ArtifactExtractionPolicy,
    ) -> ArtifactExtractionResult {
        let native = NativeTextArtifactExtractor;
        if native.supports(artifact) {
            return native.extract(artifact, bytes, policy);
        }
        let office = NativeOfficeXmlArtifactExtractor;
        if office.supports(artifact) {
            return office.extract(artifact, bytes, policy);
        }
        let email = NativeEmailArtifactExtractor;
        if email.supports(artifact) {
            return email.extract(artifact, bytes, policy);
        }
        for extractor in &self.external_extractors {
            if extractor.supports(artifact) {
                return extractor.extract(artifact, bytes, policy);
            }
        }
        let started = Instant::now();
        failure_result(
            artifact,
            "psionic.registry",
            "1.0.0",
            "registry",
            policy.output_format,
            bytes.len(),
            started,
            ExtractionFailureKind::UnsupportedArtifact,
            "no registered extractor supports this artifact",
            Vec::new(),
        )
    }
}

pub fn default_external_extractors() -> Vec<ExternalCommandArtifactExtractor> {
    vec![
        external_spec(
            "psionic.external.docx",
            "1.0.0",
            "pandoc@3.x",
            "ghcr.io/openagents/legal-extractors:pandoc-3",
            &["pandoc", "--from=docx", "--to=plain", "/workspace/inputs"],
            &["docx"],
        ),
        external_spec(
            "psionic.external.pdf",
            "1.0.0",
            "pdftotext@24.x",
            "ghcr.io/openagents/legal-extractors:poppler-24",
            &["pdftotext", "-layout", "/workspace/inputs", "-"],
            &["pdf"],
        ),
        external_spec(
            "psionic.external.pptx",
            "1.0.0",
            "pandoc@3.x",
            "ghcr.io/openagents/legal-extractors:pandoc-3",
            &["pandoc", "--from=pptx", "--to=plain", "/workspace/inputs"],
            &["pptx"],
        ),
        external_spec(
            "psionic.external.xlsx",
            "1.0.0",
            "python-openpyxl@3.x",
            "ghcr.io/openagents/legal-extractors:office-3",
            &[
                "python",
                "-m",
                "openagents_extractors.xlsx",
                "/workspace/inputs",
            ],
            &["xlsx"],
        ),
        external_spec(
            "psionic.external.eml",
            "1.0.0",
            "python-email-policy-default",
            "ghcr.io/openagents/legal-extractors:email-3",
            &[
                "python",
                "-m",
                "openagents_extractors.eml",
                "/workspace/inputs",
            ],
            &["eml"],
        ),
    ]
}

pub fn extraction_receipt_digest(receipt: &ExtractionReceipt) -> Result<String, serde_json::Error> {
    stable_json_digest("psionic.legal_benchmark.extraction_receipt.v1", receipt)
}

fn external_spec(
    extractor_name: &str,
    extractor_version: &str,
    command_or_crate_version: &str,
    container_image: &str,
    command: &[&str],
    supported_extensions: &[&str],
) -> ExternalCommandArtifactExtractor {
    ExternalCommandArtifactExtractor {
        spec: ExternalExtractorSpec {
            extractor_name: extractor_name.to_string(),
            extractor_version: extractor_version.to_string(),
            command_or_crate_version: command_or_crate_version.to_string(),
            container_image: container_image.to_string(),
            command: command.iter().map(|part| (*part).to_string()).collect(),
            supported_extensions: supported_extensions
                .iter()
                .map(|extension| (*extension).to_string())
                .collect(),
        },
    }
}

fn success_result(
    artifact: &SourceArtifact,
    extractor_name: &str,
    extractor_version: &str,
    command_or_crate_version: &str,
    output_format: ExtractionOutputFormat,
    input_len: usize,
    started: Instant,
    text: String,
    warnings: Vec<ExtractionWarning>,
    sandbox_receipt_ref: Option<String>,
) -> ArtifactExtractionResult {
    let output_bytes = text.as_bytes();
    let output_hash = sha256_hex(output_bytes);
    let output_artifact_id = format!("{}.extracted", artifact.artifact_id);
    let receipt_id = receipt_id(
        artifact.artifact_id.as_str(),
        artifact.sha256.as_str(),
        extractor_name,
        output_hash.as_str(),
        None,
    );
    let output_artifact = SourceArtifact {
        artifact_id: output_artifact_id.clone(),
        artifact_kind: ArtifactKind::ExtractedText,
        relative_path: format!("extracted/{output_artifact_id}.txt"),
        original_filename: format!("{}.txt", artifact.original_filename),
        media_type: "text/plain".to_string(),
        byte_size: u64::try_from(output_bytes.len()).unwrap_or(u64::MAX),
        sha256: output_hash.clone(),
        data_classification: artifact.data_classification,
        provenance: Some(receipt_id.clone()),
    };
    let coverage = ExtractionCoverage {
        input_byte_count: u64::try_from(input_len).unwrap_or(u64::MAX),
        output_byte_count: u64::try_from(output_bytes.len()).unwrap_or(u64::MAX),
        output_char_count: u64::try_from(text.chars().count()).unwrap_or(u64::MAX),
        ..ExtractionCoverage::default()
    };
    ArtifactExtractionResult {
        receipt: ExtractionReceipt {
            schema_version: LEGAL_BENCHMARK_EXTRACTION_SCHEMA_VERSION,
            receipt_id,
            source_artifact_id: artifact.artifact_id.clone(),
            source_artifact_hash: artifact.sha256.clone(),
            source_artifact_path: artifact.relative_path.clone(),
            input_media_type: artifact.media_type.clone(),
            input_byte_size: artifact.byte_size,
            extractor_name: extractor_name.to_string(),
            extractor_version: extractor_version.to_string(),
            command_or_crate_version: command_or_crate_version.to_string(),
            output_format,
            output_artifact_id: Some(output_artifact_id),
            output_hash: Some(output_hash),
            output_byte_size: Some(u64::try_from(output_bytes.len()).unwrap_or(u64::MAX)),
            coverage,
            warnings,
            failure_kind: None,
            failure_detail: None,
            sandbox_receipt_ref,
            elapsed_ms: elapsed_ms(started),
            metadata: BTreeMap::new(),
        },
        extracted_artifact: Some(ExtractedArtifact {
            artifact: output_artifact,
            text,
        }),
    }
}

fn failure_result(
    artifact: &SourceArtifact,
    extractor_name: &str,
    extractor_version: &str,
    command_or_crate_version: &str,
    output_format: ExtractionOutputFormat,
    input_len: usize,
    started: Instant,
    failure_kind: ExtractionFailureKind,
    failure_detail: impl Into<String>,
    warnings: Vec<ExtractionWarning>,
) -> ArtifactExtractionResult {
    let failure_detail = failure_detail.into();
    let receipt_id = receipt_id(
        artifact.artifact_id.as_str(),
        artifact.sha256.as_str(),
        extractor_name,
        failure_detail.as_str(),
        Some(failure_kind),
    );
    ArtifactExtractionResult {
        receipt: ExtractionReceipt {
            schema_version: LEGAL_BENCHMARK_EXTRACTION_SCHEMA_VERSION,
            receipt_id,
            source_artifact_id: artifact.artifact_id.clone(),
            source_artifact_hash: artifact.sha256.clone(),
            source_artifact_path: artifact.relative_path.clone(),
            input_media_type: artifact.media_type.clone(),
            input_byte_size: artifact.byte_size,
            extractor_name: extractor_name.to_string(),
            extractor_version: extractor_version.to_string(),
            command_or_crate_version: command_or_crate_version.to_string(),
            output_format,
            output_artifact_id: None,
            output_hash: None,
            output_byte_size: None,
            coverage: ExtractionCoverage {
                input_byte_count: u64::try_from(input_len).unwrap_or(u64::MAX),
                ..ExtractionCoverage::default()
            },
            warnings,
            failure_kind: Some(failure_kind),
            failure_detail: Some(failure_detail),
            sandbox_receipt_ref: None,
            elapsed_ms: elapsed_ms(started),
            metadata: BTreeMap::new(),
        },
        extracted_artifact: None,
    }
}

fn extractor_allowed(extractor_name: &str, policy: &ArtifactExtractionPolicy) -> bool {
    policy.extractor_allowlist.is_empty()
        || policy
            .extractor_allowlist
            .iter()
            .any(|allowed| allowed == extractor_name)
}

fn is_native_text_artifact(artifact: &SourceArtifact) -> bool {
    let extension = file_extension(artifact.relative_path.as_str());
    extension.as_deref().is_some_and(|extension| {
        matches!(
            extension,
            "txt"
                | "text"
                | "md"
                | "markdown"
                | "json"
                | "csv"
                | "tsv"
                | "xml"
                | "html"
                | "htm"
                | "yaml"
                | "yml"
                | "toml"
                | "log"
        )
    }) || artifact.media_type.starts_with("text/")
        || artifact.media_type == "application/json"
}

fn normalize_newlines(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

fn extract_docx_text(bytes: &[u8]) -> Result<String, String> {
    let parts = extract_zip_xml_text(bytes, |name| {
        name == "word/document.xml"
            || name.starts_with("word/header")
            || name.starts_with("word/footer")
    })?;
    let mut output = Vec::new();
    for part in parts {
        output.push(collect_tag_text(&part, "w:t").join(" "));
    }
    nonempty_text(output.join("\n"))
}

fn extract_pptx_text(bytes: &[u8]) -> Result<String, String> {
    let mut parts = extract_zip_xml_text(bytes, |name| {
        name.starts_with("ppt/slides/slide") && name.ends_with(".xml")
    })?;
    parts.sort();
    let output = parts
        .into_iter()
        .map(|part| collect_tag_text(&part, "a:t").join(" "))
        .collect::<Vec<_>>()
        .join("\n");
    nonempty_text(output)
}

fn extract_xlsx_text(bytes: &[u8]) -> Result<String, String> {
    let mut parts = extract_zip_xml_text(bytes, |name| {
        name == "xl/sharedStrings.xml"
            || (name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml"))
    })?;
    parts.sort();
    let output = parts
        .into_iter()
        .flat_map(|part| {
            let mut values = collect_tag_text(&part, "t");
            values.extend(collect_tag_text(&part, "v"));
            values
        })
        .collect::<Vec<_>>()
        .join("\n");
    nonempty_text(output)
}

fn extract_zip_xml_text(
    bytes: &[u8],
    mut include_name: impl FnMut(&str) -> bool,
) -> Result<Vec<String>, String> {
    let cursor = Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|error| format!("failed to open zip archive: {error}"))?;
    let mut parts = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| format!("failed to read zip entry {index}: {error}"))?;
        let name = file.name().to_string();
        if !include_name(&name) {
            continue;
        }
        let mut raw = Vec::new();
        file.read_to_end(&mut raw)
            .map_err(|error| format!("failed to read zip entry `{name}`: {error}"))?;
        parts.push(String::from_utf8_lossy(&raw).into_owned());
    }
    if parts.is_empty() {
        return Err(String::from(
            "Office XML archive did not contain supported text parts",
        ));
    }
    Ok(parts)
}

fn collect_tag_text(xml: &str, tag_name: &str) -> Vec<String> {
    let mut values = Vec::new();
    let open_prefix = format!("<{tag_name}");
    let close = format!("</{tag_name}>");
    let mut offset = 0;
    while let Some(open_start) = xml[offset..].find(&open_prefix) {
        let open_start = offset + open_start;
        let Some(open_end) = xml[open_start..].find('>') else {
            break;
        };
        let value_start = open_start + open_end + 1;
        let Some(close_start) = xml[value_start..].find(&close) else {
            break;
        };
        let close_start = value_start + close_start;
        let value = decode_xml_entities(&xml[value_start..close_start]);
        let value = value.trim();
        if !value.is_empty() {
            values.push(value.to_string());
        }
        offset = close_start + close.len();
    }
    values
}

fn decode_xml_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn extract_eml_text(bytes: &[u8]) -> String {
    let raw = String::from_utf8_lossy(bytes);
    let normalized = normalize_newlines(&raw);
    let (headers, body) = normalized
        .split_once("\n\n")
        .unwrap_or((normalized.as_str(), ""));
    let mut output = Vec::new();
    for header in ["From:", "To:", "Cc:", "Date:", "Subject:"] {
        if let Some(line) = headers.lines().find(|line| {
            line.to_ascii_lowercase()
                .starts_with(&header.to_ascii_lowercase())
        }) {
            output.push(line.trim().to_string());
        }
    }
    let body = body.trim();
    if !body.is_empty() {
        output.push(body.to_string());
    }
    output.join("\n")
}

fn nonempty_text(text: String) -> Result<String, String> {
    let text = normalize_newlines(text.trim());
    if text.is_empty() {
        Err(String::from("extracted text was empty"))
    } else {
        Ok(format!("{text}\n"))
    }
}

fn receipt_id(
    artifact_id: &str,
    artifact_hash: &str,
    extractor_name: &str,
    output_or_failure_hash: &str,
    failure_kind: Option<ExtractionFailureKind>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(artifact_id.as_bytes());
    hasher.update(b"|");
    hasher.update(artifact_hash.as_bytes());
    hasher.update(b"|");
    hasher.update(extractor_name.as_bytes());
    hasher.update(b"|");
    hasher.update(output_or_failure_hash.as_bytes());
    if let Some(kind) = failure_kind {
        hasher.update(b"|");
        hasher.update(format!("{kind:?}").as_bytes());
    }
    let digest = hex::encode(hasher.finalize());
    format!("extraction.receipt.{artifact_id}.{digest}")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn file_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .filter(|extension| !extension.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        artifact_from_file, DataClassification, RunRecord, ScoreReport,
        LEGAL_BENCHMARK_SCHEMA_VERSION,
    };
    use std::io::Write;
    use std::path::PathBuf;
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

    fn samples_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/legal_benchmark/extraction_samples")
    }

    fn sample_artifact(name: &str) -> SourceArtifact {
        let root = samples_root();
        artifact_from_file(
            format!("sample.{name}"),
            ArtifactKind::SourceDocument,
            root.as_path(),
            root.join(name),
            DataClassification::PublicReference,
            Some("fixture".to_string()),
        )
        .expect("sample artifact")
    }

    fn zipped_office_artifact(name: &str, entries: &[(&str, &str)]) -> (SourceArtifact, Vec<u8>) {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let artifact_path = tempdir.path().join(name);
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut archive = ZipWriter::new(&mut cursor);
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            for (entry_name, body) in entries {
                archive
                    .start_file(*entry_name, options)
                    .expect("start zip entry");
                archive.write_all(body.as_bytes()).expect("write zip entry");
            }
            archive.finish().expect("finish zip");
        }
        let bytes = cursor.into_inner();
        std::fs::write(&artifact_path, bytes.as_slice()).expect("write office artifact");
        let artifact = artifact_from_file(
            format!("fixture.{name}"),
            ArtifactKind::SourceDocument,
            tempdir.path(),
            artifact_path,
            DataClassification::PublicReference,
            Some("fixture".to_string()),
        )
        .expect("office artifact");
        (artifact, bytes)
    }

    #[test]
    fn native_text_extractor_extracts_plain_text() {
        let artifact = sample_artifact("sample.txt");
        let bytes = std::fs::read(samples_root().join("sample.txt")).expect("read sample");
        let result = NativeTextArtifactExtractor.extract(
            &artifact,
            bytes.as_slice(),
            &ArtifactExtractionPolicy::default(),
        );

        assert!(result.receipt.failure_kind.is_none());
        assert_eq!(result.receipt.source_artifact_hash, artifact.sha256);
        assert!(result.receipt.output_hash.is_some());
        assert_eq!(result.receipt.warnings.len(), 0);
        let extracted = result.extracted_artifact.expect("extracted text");
        assert!(extracted.text.contains("Settlement agreement"));
        assert_eq!(
            extracted.artifact.artifact_kind,
            ArtifactKind::ExtractedText
        );
    }

    #[test]
    fn native_json_extractor_parses_and_pretty_prints_json() {
        let artifact = sample_artifact("sample.json");
        let bytes = std::fs::read(samples_root().join("sample.json")).expect("read sample");
        let result = ArtifactExtractorRegistry::default().extract(
            &artifact,
            bytes.as_slice(),
            &ArtifactExtractionPolicy {
                output_format: ExtractionOutputFormat::JsonText,
                ..ArtifactExtractionPolicy::default()
            },
        );

        assert!(result.receipt.failure_kind.is_none());
        let extracted = result.extracted_artifact.expect("json text");
        assert!(extracted.text.contains("\"matter\""));
        assert_eq!(result.receipt.coverage.input_byte_count, artifact.byte_size);
    }

    #[test]
    fn native_office_xml_extractor_reads_docx_pptx_and_xlsx_text() {
        let registry = ArtifactExtractorRegistry::default();
        let policy = ArtifactExtractionPolicy::default();
        let cases = [
            (
                "sample.docx",
                vec![(
                    "word/document.xml",
                    "<w:document><w:body><w:t>Trust amendment</w:t><w:t>Client instruction</w:t></w:body></w:document>",
                )],
                "Trust amendment Client instruction",
            ),
            (
                "sample.pptx",
                vec![(
                    "ppt/slides/slide1.xml",
                    "<p:sld><a:t>Board deck</a:t><a:t>Closing checklist</a:t></p:sld>",
                )],
                "Board deck Closing checklist",
            ),
            (
                "sample.xlsx",
                vec![(
                    "xl/sharedStrings.xml",
                    "<sst><si><t>Purchase price</t></si><si><t>Escrow amount</t></si></sst>",
                )],
                "Purchase price",
            ),
        ];

        for (name, entries, expected_text) in cases {
            let (artifact, bytes) = zipped_office_artifact(name, &entries);
            let result = registry.extract(&artifact, bytes.as_slice(), &policy);

            assert!(result.receipt.failure_kind.is_none());
            assert_eq!(result.receipt.extractor_name, "psionic.native_office_xml");
            let extracted = result.extracted_artifact.expect("office text");
            assert!(extracted.text.contains(expected_text));
        }
    }

    #[test]
    fn major_harvey_extensions_are_supported_or_structurally_failed() {
        let registry = ArtifactExtractorRegistry::default();
        let policy = ArtifactExtractionPolicy::default();
        let expected = [
            ("sample.txt", None),
            ("sample.md", None),
            ("sample.json", None),
            ("sample.docx", Some(ExtractionFailureKind::DecodeError)),
            (
                "sample.pdf",
                Some(ExtractionFailureKind::ExternalToolUnavailable),
            ),
            ("sample.pptx", Some(ExtractionFailureKind::DecodeError)),
            ("sample.xlsx", Some(ExtractionFailureKind::DecodeError)),
            ("sample.eml", None),
        ];

        for (name, expected_failure) in expected {
            let artifact = sample_artifact(name);
            let bytes = std::fs::read(samples_root().join(name)).expect("read sample");
            let result = registry.extract(&artifact, bytes.as_slice(), &policy);
            assert_eq!(result.receipt.failure_kind, expected_failure);
            assert_eq!(result.receipt.source_artifact_hash, artifact.sha256);
            assert_eq!(result.receipt.input_byte_size, artifact.byte_size);
        }
    }

    #[test]
    fn external_policy_denial_is_structured() {
        let registry = ArtifactExtractorRegistry::default();
        let artifact = sample_artifact("sample.pdf");
        let bytes = std::fs::read(samples_root().join("sample.pdf")).expect("read sample");
        let result = registry.extract(
            &artifact,
            bytes.as_slice(),
            &ArtifactExtractionPolicy {
                external_tools_allowed: false,
                ..ArtifactExtractionPolicy::default()
            },
        );

        assert_eq!(
            result.receipt.failure_kind,
            Some(ExtractionFailureKind::PolicyDenied)
        );
        assert_eq!(result.receipt.warnings.len(), 1);
    }

    #[test]
    fn extraction_receipt_fixture_parses_for_operator_import() {
        let receipts: Vec<ExtractionReceipt> = serde_json::from_str(include_str!(
            "../../../fixtures/legal_benchmark/extraction_receipts_sample.json"
        ))
        .expect("receipt fixture parses");
        assert_eq!(receipts.len(), 2);
        assert!(receipts[0].failure_kind.is_none());
        assert_eq!(
            receipts[1].failure_kind,
            Some(ExtractionFailureKind::ExternalToolUnavailable)
        );
    }

    #[test]
    fn run_and_score_reports_retain_extraction_receipt_refs() {
        let receipt_ref = "extraction.receipt.sample.txt.abc123".to_string();
        let mut run_record: RunRecord = serde_json::from_value(serde_json::json!({
            "schema_version": LEGAL_BENCHMARK_SCHEMA_VERSION,
            "run_id": "run.legal.sample",
            "task_id": "task.legal.sample",
            "task_version": "v1",
            "input_artifact_manifest_hash": "input",
            "run_config_hash": "config",
            "output_artifact_manifest_hash": "output",
            "terminal_state": "submitted",
            "transcript": [],
            "tool_calls": [],
            "metrics": {
                "model_turns": 1,
                "tool_call_count": 0,
                "input_tokens": 10,
                "output_tokens": 20,
                "wall_time_ms": 30,
                "estimated_cost_micro_usd": 40
            }
        }))
        .expect("run record parses");
        run_record.extraction_receipt_refs.push(receipt_ref.clone());

        let mut score_report: ScoreReport = serde_json::from_value(serde_json::json!({
            "schema_version": LEGAL_BENCHMARK_SCHEMA_VERSION,
            "score_report_id": "score.legal.sample",
            "run_id": "run.legal.sample",
            "task_id": "task.legal.sample",
            "task_version": "v1",
            "run_record_hash": "run",
            "output_artifact_manifest_hash": "output",
            "all_pass": false,
            "criterion_pass_rate_bps": 0,
            "criterion_results": [],
            "metrics": {
                "model_turns": 1,
                "tool_call_count": 0,
                "input_tokens": 10,
                "output_tokens": 20,
                "wall_time_ms": 30,
                "estimated_cost_micro_usd": 40
            }
        }))
        .expect("score report parses");
        score_report
            .extraction_receipt_refs
            .push(receipt_ref.clone());

        assert_eq!(
            run_record.extraction_receipt_refs,
            vec![receipt_ref.clone()]
        );
        assert_eq!(score_report.extraction_receipt_refs, vec![receipt_ref]);
    }
}
