use std::{collections::BTreeSet, fmt, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PYLON_RELEASE_MANIFEST_SCHEMA: &str = "openagents.psionic.release_manifest.v0.3";
pub const PYLON_MODEL_ARTIFACT_MANIFEST_SCHEMA: &str =
    "openagents.psionic.model_artifact_manifest.v0.3";
pub const PYLON_RELEASE_CONTRACT_VERSION: &str = "psionic.release_manifest.v1";
pub const PYLON_MODEL_CONTRACT_VERSION: &str = "psionic.model_artifact_manifest.v1";

const REQUIRED_ENDPOINTS: &[&str] = &["/health", "/v1/models", "/v1/chat/completions"];
const SUPPORTED_PLATFORMS: &[&str] = &["darwin-arm64", "linux-x64", "linux-arm64"];
const SUPPORTED_BACKENDS: &[&str] = &["cpu", "cuda", "metal"];
const QWEN35_0_8B_MODEL_REF: &str = "model.psionic.qwen35.0_8b.q8_0";
const QWEN35_2B_MODEL_REF: &str = "model.psionic.qwen35.2b.q8_0";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PylonPsionicReleaseManifest {
    pub schema: String,
    pub contract_version: String,
    pub channel: String,
    pub version: String,
    pub platform: String,
    pub binary: PylonPsionicBinaryArtifact,
    #[serde(default)]
    pub platforms: Vec<PylonPsionicPlatformArtifact>,
    #[serde(default)]
    pub supported_endpoints: Vec<String>,
    #[serde(default)]
    pub backend_support: Vec<String>,
    pub inference_only: bool,
    pub training_claim: String,
    pub paid_inference_claim: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PylonPsionicBinaryArtifact {
    pub url: String,
    pub sha256: String,
    pub artifact_ref: String,
    pub binary_ref: String,
    #[serde(default)]
    pub signature_ref: Option<String>,
    #[serde(default)]
    pub signature_sha256: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PylonPsionicPlatformArtifact {
    pub platform: String,
    pub binary: PylonPsionicBinaryArtifact,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PylonPsionicModelArtifactManifest {
    pub schema: String,
    pub contract_version: String,
    pub model_key: String,
    pub model_ref: String,
    pub url: String,
    pub sha256: String,
    pub artifact_ref: String,
    pub model_family: String,
    pub parameter_class: String,
    pub quantization: String,
    pub role: PylonPsionicModelRole,
    pub size_bytes: u64,
    #[serde(default)]
    pub chat_template_sha256: Option<String>,
    pub license_boundary: String,
    #[serde(default)]
    pub supported_backend_families: Vec<String>,
    #[serde(default)]
    pub admitted_endpoints: Vec<String>,
    pub tool_calling: PylonPsionicToolCallingClaim,
    pub inference_only: bool,
    pub training_claim: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonPsionicModelRole {
    LowFootprintSmokeFallback,
    CodingAgentToolLoop,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PylonPsionicToolCallingClaim {
    pub admitted: bool,
    pub modes: Vec<String>,
    pub smoke_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PylonManifestValidation {
    pub schema: String,
    pub contract_version: String,
    pub subject_ref: String,
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PylonManifestError {
    Io(String),
    Decode(String),
    Invalid { field: String, message: String },
}

impl fmt::Display for PylonManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => write!(f, "manifest read failed: {message}"),
            Self::Decode(message) => write!(f, "manifest decode failed: {message}"),
            Self::Invalid { field, message } => {
                write!(f, "manifest field `{field}` is invalid: {message}")
            }
        }
    }
}

impl std::error::Error for PylonManifestError {}

impl PylonPsionicReleaseManifest {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, PylonManifestError> {
        let bytes =
            fs::read(path.as_ref()).map_err(|error| PylonManifestError::Io(error.to_string()))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| PylonManifestError::Decode(error.to_string()))
    }

    pub fn validate(&self) -> Result<PylonManifestValidation, PylonManifestError> {
        require_eq(
            "schema",
            self.schema.as_str(),
            PYLON_RELEASE_MANIFEST_SCHEMA,
        )?;
        require_eq(
            "contractVersion",
            self.contract_version.as_str(),
            PYLON_RELEASE_CONTRACT_VERSION,
        )?;
        require_supported_platform(self.platform.as_str())?;
        require_nonempty_ref("channel", self.channel.as_str())?;
        require_nonempty_ref("version", self.version.as_str())?;
        require_binary("binary", &self.binary)?;
        require_endpoint_set("supportedEndpoints", &self.supported_endpoints)?;
        require_backend_set("backendSupport", &self.backend_support)?;
        if !self.inference_only {
            return invalid(
                "inferenceOnly",
                "Pylon sidecar releases must be inference-only",
            );
        }
        require_eq("trainingClaim", self.training_claim.as_str(), "blocked")?;
        require_eq(
            "paidInferenceClaim",
            self.paid_inference_claim.as_str(),
            "blocked",
        )?;

        let mut seen = BTreeSet::new();
        for row in &self.platforms {
            require_supported_platform(row.platform.as_str())?;
            if !seen.insert(row.platform.as_str()) {
                return invalid("platforms", "duplicate platform row");
            }
            require_binary("platforms.binary", &row.binary)?;
        }
        for platform in SUPPORTED_PLATFORMS {
            if !seen.contains(platform) {
                return invalid("platforms", "missing first-pass Pylon platform row");
            }
        }
        let selected = self
            .platforms
            .iter()
            .find(|row| row.platform == self.platform)
            .ok_or_else(|| PylonManifestError::Invalid {
                field: String::from("platforms"),
                message: String::from("selected platform is absent from platform rows"),
            })?;
        if selected.binary != self.binary {
            return invalid(
                "binary",
                "selected platform binary must match the legacy Pylon binary field",
            );
        }

        Ok(PylonManifestValidation {
            schema: self.schema.clone(),
            contract_version: self.contract_version.clone(),
            subject_ref: self.binary.binary_ref.clone(),
            digest: stable_manifest_digest(self),
        })
    }
}

impl PylonPsionicModelArtifactManifest {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, PylonManifestError> {
        let bytes =
            fs::read(path.as_ref()).map_err(|error| PylonManifestError::Io(error.to_string()))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| PylonManifestError::Decode(error.to_string()))
    }

    pub fn validate(&self) -> Result<PylonManifestValidation, PylonManifestError> {
        require_eq(
            "schema",
            self.schema.as_str(),
            PYLON_MODEL_ARTIFACT_MANIFEST_SCHEMA,
        )?;
        require_eq(
            "contractVersion",
            self.contract_version.as_str(),
            PYLON_MODEL_CONTRACT_VERSION,
        )?;
        require_nonempty_ref("modelKey", self.model_key.as_str())?;
        require_model_ref(self.model_ref.as_str())?;
        require_url("url", self.url.as_str())?;
        require_sha256("sha256", self.sha256.as_str())?;
        require_nonempty_ref("artifactRef", self.artifact_ref.as_str())?;
        require_eq("modelFamily", self.model_family.as_str(), "qwen35")?;
        require_eq("quantization", self.quantization.as_str(), "q8_0")?;
        if self.size_bytes == 0 {
            return invalid("sizeBytes", "model artifact size must be non-zero");
        }
        if let Some(chat_template_sha256) = &self.chat_template_sha256 {
            require_sha256("chatTemplateSha256", chat_template_sha256)?;
        }
        require_endpoint_set("admittedEndpoints", &self.admitted_endpoints)?;
        require_backend_set("supportedBackendFamilies", &self.supported_backend_families)?;
        require_nonempty_ref("toolCalling.smokeRef", self.tool_calling.smoke_ref.as_str())?;
        if !self.tool_calling.admitted {
            return invalid(
                "toolCalling.admitted",
                "Pylon model rows must admit tool calls",
            );
        }
        if !self
            .tool_calling
            .modes
            .iter()
            .any(|mode| mode == "required")
        {
            return invalid("toolCalling.modes", "required tool-call mode is missing");
        }
        if !self.inference_only {
            return invalid(
                "inferenceOnly",
                "Pylon model artifacts must be inference-only",
            );
        }
        require_eq("trainingClaim", self.training_claim.as_str(), "blocked")?;
        match self.model_ref.as_str() {
            QWEN35_0_8B_MODEL_REF
                if self.role == PylonPsionicModelRole::LowFootprintSmokeFallback => {}
            QWEN35_2B_MODEL_REF if self.role == PylonPsionicModelRole::CodingAgentToolLoop => {}
            QWEN35_0_8B_MODEL_REF | QWEN35_2B_MODEL_REF => {
                return invalid("role", "model role does not match model ref");
            }
            _ => return invalid("modelRef", "unsupported Pylon Qwen3.5 model ref"),
        }

        Ok(PylonManifestValidation {
            schema: self.schema.clone(),
            contract_version: self.contract_version.clone(),
            subject_ref: self.model_ref.clone(),
            digest: stable_manifest_digest(self),
        })
    }
}

fn require_binary(
    field: &str,
    binary: &PylonPsionicBinaryArtifact,
) -> Result<(), PylonManifestError> {
    require_url(&format!("{field}.url"), binary.url.as_str())?;
    require_sha256(&format!("{field}.sha256"), binary.sha256.as_str())?;
    require_nonempty_ref(
        &format!("{field}.artifactRef"),
        binary.artifact_ref.as_str(),
    )?;
    require_nonempty_ref(&format!("{field}.binaryRef"), binary.binary_ref.as_str())?;
    if let Some(signature_ref) = &binary.signature_ref {
        require_nonempty_ref(&format!("{field}.signatureRef"), signature_ref)?;
    }
    if let Some(signature_sha256) = &binary.signature_sha256 {
        require_sha256(&format!("{field}.signatureSha256"), signature_sha256)?;
    }
    Ok(())
}

fn require_supported_platform(platform: &str) -> Result<(), PylonManifestError> {
    if SUPPORTED_PLATFORMS.contains(&platform) {
        Ok(())
    } else {
        invalid("platform", "unsupported first-pass Pylon platform")
    }
}

fn require_model_ref(value: &str) -> Result<(), PylonManifestError> {
    if value == QWEN35_0_8B_MODEL_REF || value == QWEN35_2B_MODEL_REF {
        Ok(())
    } else {
        invalid("modelRef", "unsupported Pylon Qwen3.5 model ref")
    }
}

fn require_endpoint_set(field: &str, values: &[String]) -> Result<(), PylonManifestError> {
    for endpoint in REQUIRED_ENDPOINTS {
        if !values.iter().any(|value| value == endpoint) {
            return invalid(field, "required OpenAI-compatible endpoint is missing");
        }
    }
    Ok(())
}

fn require_backend_set(field: &str, values: &[String]) -> Result<(), PylonManifestError> {
    for backend in SUPPORTED_BACKENDS {
        if !values.iter().any(|value| value == backend) {
            return invalid(field, "required backend family is missing");
        }
    }
    Ok(())
}

fn require_eq(field: &str, actual: &str, expected: &str) -> Result<(), PylonManifestError> {
    if actual == expected {
        Ok(())
    } else {
        invalid(field, format!("expected `{expected}`, got `{actual}`"))
    }
}

fn require_nonempty_ref(field: &str, value: &str) -> Result<(), PylonManifestError> {
    if value.is_empty() || value.contains('/') || value.contains('\\') || value.contains(' ') {
        invalid(field, "expected a public ref, not a path or free text")
    } else if contains_secret_shape(value) {
        invalid(field, "secret-shaped value is not allowed")
    } else {
        Ok(())
    }
}

fn require_url(field: &str, value: &str) -> Result<(), PylonManifestError> {
    if value.starts_with("https://") && !contains_secret_shape(value) {
        Ok(())
    } else {
        invalid(field, "expected public https artifact URL")
    }
}

fn require_sha256(field: &str, value: &str) -> Result<(), PylonManifestError> {
    if value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Ok(())
    } else {
        invalid(field, "expected lowercase SHA-256 hex")
    }
}

fn contains_secret_shape(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("bearer ")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("secret")
        || lower.contains("token=")
        || lower.contains("authorization")
}

fn invalid<T>(
    field: impl Into<String>,
    message: impl Into<String>,
) -> Result<T, PylonManifestError> {
    Err(PylonManifestError::Invalid {
        field: field.into(),
        message: message.into(),
    })
}

fn stable_manifest_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        PylonPsionicModelArtifactManifest, PylonPsionicModelRole, PylonPsionicReleaseManifest,
    };

    const RELEASE_FIXTURES: &[&str] = &[
        "fixtures/pylon/psionic/release_manifest_darwin_arm64_v0_3.json",
        "fixtures/pylon/psionic/release_manifest_linux_x64_v0_3.json",
        "fixtures/pylon/psionic/release_manifest_linux_arm64_v0_3.json",
    ];

    const MODEL_FIXTURES: &[&str] = &[
        "fixtures/pylon/psionic/model_artifact_manifest_qwen35_0_8b_q8_0_v0_3.json",
        "fixtures/pylon/psionic/model_artifact_manifest_qwen35_2b_q8_0_v0_3.json",
    ];

    #[test]
    fn pylon_manifest_release_manifests_validate_for_first_pass_platforms()
    -> Result<(), Box<dyn std::error::Error>> {
        for fixture in RELEASE_FIXTURES {
            let manifest = PylonPsionicReleaseManifest::from_path(repo_fixture(fixture))?;
            let validation = manifest.validate()?;
            assert_eq!(validation.contract_version, "psionic.release_manifest.v1");
            assert!(
                validation
                    .subject_ref
                    .starts_with("binary.psionic.openai_server.")
            );
            assert_eq!(manifest.platforms.len(), 3);
            assert!(
                manifest
                    .supported_endpoints
                    .contains(&String::from("/v1/chat/completions"))
            );
            assert_eq!(manifest.training_claim, "blocked");
            assert_eq!(manifest.paid_inference_claim, "blocked");
        }
        Ok(())
    }

    #[test]
    fn pylon_manifest_model_artifact_manifests_validate_roles_and_tool_claims()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut roles = Vec::new();
        for fixture in MODEL_FIXTURES {
            let manifest = PylonPsionicModelArtifactManifest::from_path(repo_fixture(fixture))?;
            let validation = manifest.validate()?;
            assert_eq!(
                validation.contract_version,
                "psionic.model_artifact_manifest.v1"
            );
            assert!(manifest.tool_calling.admitted);
            assert!(
                manifest
                    .tool_calling
                    .modes
                    .contains(&String::from("required"))
            );
            assert!(manifest.inference_only);
            assert_eq!(manifest.training_claim, "blocked");
            roles.push(manifest.role);
        }
        assert!(roles.contains(&PylonPsionicModelRole::LowFootprintSmokeFallback));
        assert!(roles.contains(&PylonPsionicModelRole::CodingAgentToolLoop));
        Ok(())
    }

    #[test]
    fn pylon_manifest_validation_refuses_private_or_overclaiming_rows() {
        let mut manifest = PylonPsionicModelArtifactManifest::from_path(repo_fixture(
            "fixtures/pylon/psionic/model_artifact_manifest_qwen35_0_8b_q8_0_v0_3.json",
        ))
        .unwrap_or_else(|error| panic!("fixture should decode: {error}"));
        manifest.url = String::from("file:///Users/christopherdavid/model.gguf");
        assert!(manifest.validate().is_err());

        manifest.url =
            String::from("https://artifacts.openagents.com/psionic/qwen35-0_8b-q8_0.gguf");
        manifest.training_claim = String::from("live");
        assert!(manifest.validate().is_err());
    }

    fn repo_fixture(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    }
}
