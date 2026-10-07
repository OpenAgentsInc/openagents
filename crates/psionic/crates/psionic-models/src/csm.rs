//! Rust-native CSM prompt, tokenizer, and artifact descriptors.

use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
    time::Instant,
};

use candle_transformers::{
    generation::{LogitsProcessor, Sampling},
    models::csm as candle_csm,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokenizers::{Tokenizer, processors::template::TemplateProcessing};

use crate::{
    CsmAudioMetadata, CsmDeterministicGenerationCase, CsmMimiCodebookPrefix, CsmParityPrompt,
    CsmPythonParityFixture, validate_csm_python_parity_fixture,
};

/// Psionic-owned CSM model id.
pub const CSM_MODEL_ID: &str = "sesame/csm-1b";
/// CSM uses one 32-codebook audio lane group and one text lane.
pub const CSM_FRAME_LANES: usize = 33;
/// CSM audio codebook lane count.
pub const CSM_AUDIO_CODEBOOK_LANES: usize = 32;
/// CSM text lane index inside each 33-lane frame.
pub const CSM_TEXT_LANE_INDEX: usize = 32;
/// Python reference max sequence length.
pub const CSM_MAX_SEQ_LEN: usize = 2048;
/// One generated Mimi frame covers 80 ms at the reference frame rate.
pub const CSM_GENERATION_FRAME_MS: u64 = 80;
/// Native CSM/Mimi sample rate.
pub const CSM_SAMPLE_RATE_HZ: u32 = 24_000;
/// Source prompt WAV sample rate from Sesame's prompt assets.
pub const CSM_PROMPT_SOURCE_SAMPLE_RATE_HZ: u32 = 44_100;
/// CSM codebook EOS token id.
pub const CSM_CODEBOOK_EOS_TOKEN_ID: u32 = 0;
/// CSM codebook pad token id.
pub const CSM_CODEBOOK_PAD_TOKEN_ID: u32 = 2050;
/// Llama 3.2 BOS token used by the CSM text encoder.
pub const CSM_LLAMA_BOS_TOKEN_ID: u32 = 128_000;
/// Llama 3.2 EOS token used by the CSM text encoder.
pub const CSM_LLAMA_EOS_TOKEN_ID: u32 = 128_001;
/// Llama BOS token string in Hugging Face tokenizer JSON.
pub const CSM_LLAMA_BOS_TOKEN: &str = "<|begin_of_text|>";
/// Llama EOS token string in Hugging Face tokenizer JSON.
pub const CSM_LLAMA_EOS_TOKEN: &str = "<|end_of_text|>";
/// Llama tokenizer repo used by CSM.
pub const CSM_LLAMA_TOKENIZER_REPO: &str = "meta-llama/Llama-3.2-1B";
/// Mimi repo used by the CSM codec path.
pub const CSM_MIMI_REPO: &str = "kyutai/moshiko-pytorch-bf16";
/// Mimi safetensors file used by the CSM codec path.
pub const CSM_MIMI_WEIGHT: &str = "tokenizer-e351c8d8-checkpoint125.safetensors";
/// Explicit refusal code for runtime reference-audio encoding.
pub const CSM_REFERENCE_AUDIO_ENCODING_UNSUPPORTED_CODE: &str = "rust_mimi_encode_not_implemented";
/// First Rust CPU CSM generation engine label.
pub const CSM_CPU_EXECUTION_ENGINE: &str = "rust_candle_csm_cpu";
/// First Rust CUDA CSM generation engine label.
pub const CSM_CUDA_EXECUTION_ENGINE: &str = "rust_candle_csm_cuda";
/// First Rust Metal CSM generation engine label.
pub const CSM_METAL_EXECUTION_ENGINE: &str = "rust_candle_csm_metal";
/// Rust CPU Mimi decode engine label.
pub const CSM_MIMI_CPU_EXECUTION_ENGINE: &str = "rust_moshi_mimi_cpu";
/// Rust CUDA Mimi decode engine label.
pub const CSM_MIMI_CUDA_EXECUTION_ENGINE: &str = "rust_moshi_mimi_cuda";
/// Rust Metal Mimi decode engine label.
pub const CSM_MIMI_METAL_EXECUTION_ENGINE: &str = "rust_moshi_mimi_metal";
/// Machine-readable CSM voice-profile governance schema.
pub const CSM_VOICE_PROFILE_GOVERNANCE_SCHEMA_VERSION: &str = "psionic.csm.voice_profiles.v1";
/// First governed OpenAgents CSM voice profile.
pub const CSM_OPENAGENTS_DEFAULT_FEMALE_PROFILE_ID: &str = "openagents/default_female_v1";
/// Internal development admission for governed CSM voice profiles.
pub const CSM_RUNTIME_ADMISSION_INTERNAL_DEVELOPMENT: &str = "admitted_internal_development";
/// Operator-approved production dogfood admission for governed CSM voice profiles.
pub const CSM_RUNTIME_ADMISSION_OPERATOR_AUTOPILOT_DOGFOOD: &str =
    "admitted_openagents_operated_autopilot_production_dogfood";
/// Watermark status used when output is admitted for bounded operator dogfood only.
pub const CSM_WATERMARK_OPERATOR_DOGFOOD: &str = "unsupported_operator_accepted_limited_dogfood";
const CSM_VOICE_PROFILE_GOVERNANCE_JSON: &str =
    include_str!("../../../fixtures/csm/voice_profiles/lyra_voice_profiles.v1.json");

/// CSM frontend, descriptor, and prompt-building errors.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum CsmFrontendError {
    /// Artifact digest did not match the expected fixture digest.
    #[error("CSM artifact digest mismatch for {artifact}: expected {expected}, got {actual}")]
    ArtifactDigestMismatch {
        /// Artifact label.
        artifact: String,
        /// Expected sha256 digest.
        expected: String,
        /// Actual sha256 digest.
        actual: String,
    },
    /// Artifact bytes could not be read.
    #[error("failed to read CSM artifact {artifact}: {message}")]
    ArtifactRead {
        /// Artifact label.
        artifact: String,
        /// Error detail.
        message: String,
    },
    /// Tokenizer JSON could not be loaded or configured.
    #[error("failed to load CSM tokenizer: {message}")]
    TokenizerLoad {
        /// Error detail.
        message: String,
    },
    /// Tokenizer encode failed.
    #[error("failed to encode CSM text: {message}")]
    TokenizerEncode {
        /// Error detail.
        message: String,
    },
    /// CSM config JSON could not be parsed.
    #[error("failed to parse CSM config: {message}")]
    ConfigParse {
        /// Error detail.
        message: String,
    },
    /// CSM config values are not compatible with the known CSM frame contract.
    #[error("invalid CSM config: {message}")]
    ConfigContract {
        /// Error detail.
        message: String,
    },
    /// Fixture or descriptor contract is invalid.
    #[error("invalid CSM descriptor fixture: {message}")]
    DescriptorContract {
        /// Error detail.
        message: String,
    },
    /// Audio codebooks are malformed.
    #[error("invalid CSM audio codebooks: {message}")]
    AudioCodebooks {
        /// Error detail.
        message: String,
    },
    /// Mimi model loading failed.
    #[error("failed to load Rust Mimi decoder: {message}")]
    MimiLoad {
        /// Error detail.
        message: String,
    },
    /// Mimi decode failed.
    #[error("failed to decode Mimi codebooks in Rust: {message}")]
    MimiDecode {
        /// Error detail.
        message: String,
    },
    /// WAV encoding failed.
    #[error("failed to encode CSM WAV: {message}")]
    WavEncode {
        /// Error detail.
        message: String,
    },
    /// Runtime reference-audio encode is not implemented.
    #[error("runtime reference-audio encoding is unsupported: {message}")]
    ReferenceAudioEncodingUnsupported {
        /// Error detail.
        message: String,
    },
    /// CSM model loading failed.
    #[error("failed to load Rust CSM model: {message}")]
    CsmModelLoad {
        /// Error detail.
        message: String,
    },
    /// CSM generation failed.
    #[error("failed to generate CSM codebooks in Rust: {message}")]
    CsmGeneration {
        /// Error detail.
        message: String,
    },
    /// Sampling mode is unsupported by the current Rust path.
    #[error("unsupported CSM sampling mode: {message}")]
    UnsupportedSampling {
        /// Error detail.
        message: String,
    },
    /// The committed fixture lacks enough prompt data for exact replay.
    #[error("CSM fixture replay unavailable: {message}")]
    FixtureReplayUnavailable {
        /// Error detail.
        message: String,
    },
    /// Prompt plus requested generation exceeds the admitted context window.
    #[error(
        "CSM prompt has {prompt_frames} frames, but must be below max_context_len={max_context_len}"
    )]
    PromptContextOverflow {
        /// Prompt frame count.
        prompt_frames: usize,
        /// Allowed context length.
        max_context_len: usize,
        /// Requested generated frame budget.
        max_generation_len: usize,
    },
    /// Generation duration cannot be converted into a valid generation window.
    #[error("invalid CSM generation window: {message}")]
    GenerationWindow {
        /// Error detail.
        message: String,
    },
}

/// Admitted Rust-native execution backend for CSM speech generation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "backend", rename_all = "snake_case")]
pub enum CsmRuntimeBackend {
    /// CPU execution through Candle and Moshi.
    Cpu,
    /// CUDA execution through Candle and Moshi.
    Cuda {
        /// CUDA device ordinal.
        device_ordinal: usize,
    },
    /// Metal execution through Candle and Moshi.
    Metal {
        /// Metal device ordinal.
        device_ordinal: usize,
    },
}

impl CsmRuntimeBackend {
    /// Parses backend labels accepted by the CSM speech worker.
    pub fn parse(value: &str) -> Result<Self, String> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "" | "cpu" => Ok(Self::Cpu),
            "cuda" => Ok(Self::Cuda { device_ordinal: 0 }),
            "metal" => Ok(Self::Metal { device_ordinal: 0 }),
            _ if normalized.starts_with("cuda:") => {
                let ordinal = normalized
                    .trim_start_matches("cuda:")
                    .parse::<usize>()
                    .map_err(|error| format!("invalid CUDA device ordinal: {error}"))?;
                Ok(Self::Cuda {
                    device_ordinal: ordinal,
                })
            }
            _ if normalized.starts_with("metal:") => {
                let ordinal = normalized
                    .trim_start_matches("metal:")
                    .parse::<usize>()
                    .map_err(|error| format!("invalid Metal device ordinal: {error}"))?;
                Ok(Self::Metal {
                    device_ordinal: ordinal,
                })
            }
            _ => Err(format!("unsupported CSM backend `{value}`")),
        }
    }

    /// Stable backend label for headers, health checks, and evidence.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Cuda { .. } => "cuda",
            Self::Metal { .. } => "metal",
        }
    }

    /// Stable execution engine label for CSM generation.
    #[must_use]
    pub fn execution_engine(&self) -> &'static str {
        match self {
            Self::Cpu => CSM_CPU_EXECUTION_ENGINE,
            Self::Cuda { .. } => CSM_CUDA_EXECUTION_ENGINE,
            Self::Metal { .. } => CSM_METAL_EXECUTION_ENGINE,
        }
    }

    /// Stable execution engine label for Mimi decode.
    #[must_use]
    pub fn mimi_execution_engine(&self) -> &'static str {
        match self {
            Self::Cpu => CSM_MIMI_CPU_EXECUTION_ENGINE,
            Self::Cuda { .. } => CSM_MIMI_CUDA_EXECUTION_ENGINE,
            Self::Metal { .. } => CSM_MIMI_METAL_EXECUTION_ENGINE,
        }
    }

    /// Residency label for served runtime status.
    #[must_use]
    pub fn residency_mode(&self) -> &'static str {
        match self {
            Self::Cpu => "warm_cpu",
            Self::Cuda { .. } => "warm_cuda",
            Self::Metal { .. } => "warm_metal",
        }
    }

    /// Accelerator publication label.
    #[must_use]
    pub fn accelerated_backend(&self) -> &'static str {
        match self {
            Self::Cpu => "unavailable_fail_closed",
            Self::Cuda { .. } => "cuda",
            Self::Metal { .. } => "metal",
        }
    }

    /// True when the backend is not CPU.
    #[must_use]
    pub fn is_accelerated(&self) -> bool {
        !matches!(self, Self::Cpu)
    }

    /// Builds the Candle device for this backend.
    pub fn device(&self) -> Result<moshi::candle::Device, CsmFrontendError> {
        match self {
            Self::Cpu => Ok(moshi::candle::Device::Cpu),
            Self::Cuda { device_ordinal } => moshi::candle::Device::new_cuda(*device_ordinal)
                .map_err(|error| CsmFrontendError::CsmModelLoad {
                    message: format!("failed to initialize CUDA device {device_ordinal}: {error}"),
                }),
            Self::Metal { device_ordinal } => moshi::candle::Device::new_metal(*device_ordinal)
                .map_err(|error| CsmFrontendError::CsmModelLoad {
                    message: format!("failed to initialize Metal device {device_ordinal}: {error}"),
                }),
        }
    }
}

/// Complete artifact identity needed before loading CSM weights.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmArtifactDigestSet {
    /// CSM config digest.
    pub csm_config_digest: String,
    /// CSM safetensors digest.
    pub csm_model_digest: String,
    /// Llama tokenizer JSON digest.
    pub llama_tokenizer_digest: String,
    /// Mimi safetensors digest.
    pub mimi_weight_digest: String,
}

/// Stable 33-lane frame contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmFrameContract {
    /// Total frame width.
    pub frame_lanes: usize,
    /// Number of audio codebook lanes.
    pub audio_codebook_lanes: usize,
    /// Text token lane.
    pub text_lane_index: usize,
    /// Maximum sequence length admitted by CSM.
    pub max_seq_len: usize,
    /// Generated-frame duration.
    pub generation_frame_ms: u64,
    /// CSM/Mimi runtime sample rate.
    pub sample_rate_hz: u32,
    /// Codebook EOS token.
    pub codebook_eos_token_id: u32,
    /// Codebook pad token.
    pub codebook_pad_token_id: u32,
}

impl Default for CsmFrameContract {
    fn default() -> Self {
        Self {
            frame_lanes: CSM_FRAME_LANES,
            audio_codebook_lanes: CSM_AUDIO_CODEBOOK_LANES,
            text_lane_index: CSM_TEXT_LANE_INDEX,
            max_seq_len: CSM_MAX_SEQ_LEN,
            generation_frame_ms: CSM_GENERATION_FRAME_MS,
            sample_rate_hz: CSM_SAMPLE_RATE_HZ,
            codebook_eos_token_id: CSM_CODEBOOK_EOS_TOKEN_ID,
            codebook_pad_token_id: CSM_CODEBOOK_PAD_TOKEN_ID,
        }
    }
}

/// Prompt voice-profile descriptor from the committed CSM fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmVoiceProfileDescriptor {
    /// Stable profile id.
    pub profile_id: String,
    /// Numeric CSM speaker id.
    pub speaker: u32,
    /// Prompt transcript.
    pub text: String,
    /// Prompt WAV digest.
    pub prompt_audio_sha256: String,
    /// Prompt WAV metadata.
    pub audio: CsmAudioMetadata,
    /// Optional compact Mimi codebook digest for this prompt.
    pub mimi_tokens_sha256: Option<String>,
    /// Optional prompt-codebook descriptor.
    pub prompt_codebooks: Option<CsmVoiceProfileCodebookDescriptor>,
}

/// Approved voice-profile prompt codebook descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmVoiceProfileCodebookDescriptor {
    /// Descriptor source.
    pub source: String,
    /// Mimi sample rate.
    pub sample_rate_hz: u32,
    /// Codebook count.
    pub codebook_count: usize,
    /// Full prompt codebook frame count.
    pub frame_count: usize,
    /// Retained prefix frame count in the committed fixture.
    pub prefix_frame_count: usize,
    /// Retained prefix codebook count in the committed fixture.
    pub prefix_codebook_count: usize,
    /// Full codebook tensor digest.
    pub tokens_sha256: String,
}

/// Machine-readable CSM voice governance manifest for served OpenAgents profiles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmVoiceProfileGovernanceManifest {
    /// Manifest schema version.
    pub schema_version: String,
    /// Manifest capture timestamp.
    pub generated_at: String,
    /// Psionic model id governed by this manifest.
    pub model_id: String,
    /// Global watermark posture.
    pub watermark_policy: CsmWatermarkGovernancePolicy,
    /// Governed profiles.
    pub profiles: Vec<CsmVoiceProfileGovernanceDescriptor>,
}

impl CsmVoiceProfileGovernanceManifest {
    /// Parses the committed governance manifest.
    pub fn from_committed_manifest() -> Result<Self, CsmFrontendError> {
        let manifest: Self =
            serde_json::from_str(CSM_VOICE_PROFILE_GOVERNANCE_JSON).map_err(|error| {
                CsmFrontendError::DescriptorContract {
                    message: format!(
                        "failed to parse CSM voice-profile governance manifest: {error}"
                    ),
                }
            })?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validates manifest-level invariants that do not require model fixtures.
    pub fn validate(&self) -> Result<(), CsmFrontendError> {
        if self.schema_version != CSM_VOICE_PROFILE_GOVERNANCE_SCHEMA_VERSION {
            return Err(CsmFrontendError::DescriptorContract {
                message: format!(
                    "unsupported CSM voice governance schema {}",
                    self.schema_version
                ),
            });
        }
        if self.model_id != CSM_MODEL_ID {
            return Err(CsmFrontendError::DescriptorContract {
                message: format!(
                    "voice governance model id {} does not match {}",
                    self.model_id, CSM_MODEL_ID
                ),
            });
        }
        if self.profiles.is_empty() {
            return Err(CsmFrontendError::DescriptorContract {
                message: "voice governance manifest has no profiles".to_string(),
            });
        }
        for profile in &self.profiles {
            profile.validate()?;
        }
        Ok(())
    }

    /// Validates that governed profile references match the model descriptor.
    pub fn validate_against_descriptor(
        &self,
        descriptor: &CsmModelArtifactDescriptor,
    ) -> Result<(), CsmFrontendError> {
        self.validate()?;
        for profile in &self.profiles {
            let Some(source) = descriptor
                .voice_profiles
                .iter()
                .find(|candidate| candidate.profile_id == profile.source_prompt_profile_id)
            else {
                return Err(CsmFrontendError::DescriptorContract {
                    message: format!(
                        "governed voice profile {} references missing source prompt {}",
                        profile.profile_id, profile.source_prompt_profile_id
                    ),
                });
            };
            if profile.speaker != source.speaker {
                return Err(CsmFrontendError::DescriptorContract {
                    message: format!(
                        "governed voice profile {} speaker {} does not match source speaker {}",
                        profile.profile_id, profile.speaker, source.speaker
                    ),
                });
            }
            if profile.prompt_audio_sha256 != source.prompt_audio_sha256 {
                return Err(CsmFrontendError::DescriptorContract {
                    message: format!(
                        "governed voice profile {} prompt audio digest does not match source prompt",
                        profile.profile_id
                    ),
                });
            }
            let transcript_digest = sha256_digest(source.text.as_bytes());
            if profile.prompt_transcript_sha256 != transcript_digest {
                return Err(CsmFrontendError::DescriptorContract {
                    message: format!(
                        "governed voice profile {} prompt transcript digest does not match source prompt",
                        profile.profile_id
                    ),
                });
            }
            if profile.prompt_codebook_tokens_sha256 != source.mimi_tokens_sha256 {
                return Err(CsmFrontendError::DescriptorContract {
                    message: format!(
                        "governed voice profile {} prompt codebook digest does not match source prompt",
                        profile.profile_id
                    ),
                });
            }
            if profile.csm_config_digest != descriptor.digests.csm_config_digest
                || profile.csm_model_digest != descriptor.digests.csm_model_digest
                || profile.llama_tokenizer_digest != descriptor.digests.llama_tokenizer_digest
                || profile.mimi_weight_digest != descriptor.digests.mimi_weight_digest
            {
                return Err(CsmFrontendError::DescriptorContract {
                    message: format!(
                        "governed voice profile {} artifact digests do not match descriptor",
                        profile.profile_id
                    ),
                });
            }
        }
        Ok(())
    }

    /// Finds a governed profile by public OpenAgents profile id.
    #[must_use]
    pub fn find_profile(&self, profile_id: &str) -> Option<&CsmVoiceProfileGovernanceDescriptor> {
        self.profiles
            .iter()
            .find(|profile| profile.profile_id == profile_id)
    }
}

/// Global CSM watermark posture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmWatermarkGovernancePolicy {
    /// Current implementation status.
    pub status: String,
    /// Stable refusal code when watermarking is required.
    pub refusal_code: String,
    /// Required future phase.
    pub required_phase: String,
    /// Whether any public demo key is production safety.
    pub public_demo_key_status: String,
    /// Production cutover posture.
    pub production_cutover: String,
}

/// Governed OpenAgents-facing voice profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmVoiceProfileGovernanceDescriptor {
    /// Public OpenAgents voice profile id.
    pub profile_id: String,
    /// Human-readable name.
    pub display_name: String,
    /// Approval status for this placeholder.
    pub approval_status: String,
    /// Runtime admission status.
    pub runtime_admission: String,
    /// Source prompt profile in the parity fixture.
    pub source_prompt_profile_id: String,
    /// Numeric CSM speaker id.
    pub speaker: u32,
    /// Prompt transcript digest.
    pub prompt_transcript_sha256: String,
    /// Prompt audio digest.
    pub prompt_audio_sha256: String,
    /// Prompt codebook tensor digest when available.
    pub prompt_codebook_tokens_sha256: Option<String>,
    /// CSM config digest.
    pub csm_config_digest: String,
    /// CSM model weight digest.
    pub csm_model_digest: String,
    /// Llama tokenizer digest.
    pub llama_tokenizer_digest: String,
    /// Mimi weight digest.
    pub mimi_weight_digest: String,
    /// Model/codec compatibility contract.
    pub compatibility: CsmVoiceProfileCompatibility,
    /// Source provenance posture.
    pub source_provenance: String,
    /// Consent posture.
    pub consent_posture: String,
    /// Product surfaces where this profile is admitted.
    pub allowed_product_surfaces: Vec<String>,
    /// Explicitly disallowed surfaces.
    pub disallowed_product_surfaces: Vec<String>,
    /// Prompt asset retention policy.
    pub retention_policy: String,
    /// Prompt asset redaction policy.
    pub redaction_policy: String,
    /// Per-profile watermark posture.
    pub watermark_policy: CsmVoiceProfileWatermarkPolicy,
}

impl CsmVoiceProfileGovernanceDescriptor {
    fn validate(&self) -> Result<(), CsmFrontendError> {
        if self.profile_id.trim().is_empty() {
            return Err(CsmFrontendError::DescriptorContract {
                message: "governed CSM voice profile id is empty".to_string(),
            });
        }
        if self.source_prompt_profile_id.trim().is_empty() {
            return Err(CsmFrontendError::DescriptorContract {
                message: format!(
                    "governed voice profile {} has no source prompt",
                    self.profile_id
                ),
            });
        }
        if !matches!(
            self.runtime_admission.as_str(),
            CSM_RUNTIME_ADMISSION_INTERNAL_DEVELOPMENT
                | CSM_RUNTIME_ADMISSION_OPERATOR_AUTOPILOT_DOGFOOD
        ) {
            return Err(CsmFrontendError::DescriptorContract {
                message: format!(
                    "governed voice profile {} is not admitted for this runtime",
                    self.profile_id
                ),
            });
        }
        if self.allowed_product_surfaces.is_empty() {
            return Err(CsmFrontendError::DescriptorContract {
                message: format!(
                    "governed voice profile {} has no allowed product surfaces",
                    self.profile_id
                ),
            });
        }
        if !matches!(
            self.watermark_policy.status.as_str(),
            "unsupported_fail_closed" | CSM_WATERMARK_OPERATOR_DOGFOOD
        ) {
            return Err(CsmFrontendError::DescriptorContract {
                message: format!(
                    "governed voice profile {} must publish bounded watermark posture",
                    self.profile_id
                ),
            });
        }
        Ok(())
    }

    /// True when the profile is admitted for the Rust CSM server.
    #[must_use]
    pub fn is_runtime_admitted(&self) -> bool {
        matches!(
            self.runtime_admission.as_str(),
            CSM_RUNTIME_ADMISSION_INTERNAL_DEVELOPMENT
                | CSM_RUNTIME_ADMISSION_OPERATOR_AUTOPILOT_DOGFOOD
        )
    }
}

/// Model/codec compatibility contract for a governed profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmVoiceProfileCompatibility {
    /// CSM model id.
    pub csm_model_id: String,
    /// CSM frame contract version.
    pub frame_contract: String,
    /// Mimi compatibility version.
    pub mimi: String,
    /// Prompt-codebook support status.
    pub prompt_codebooks: String,
}

/// Per-profile watermark posture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmVoiceProfileWatermarkPolicy {
    /// Current implementation status.
    pub status: String,
    /// Stable refusal code when watermarking is required.
    pub refusal_code: String,
    /// Required future phase.
    pub required_phase: String,
    /// Public-demo-key posture.
    pub public_demo_key_status: String,
}

/// Loads and validates the committed CSM voice governance manifest.
pub fn csm_voice_profile_governance_manifest()
-> Result<CsmVoiceProfileGovernanceManifest, CsmFrontendError> {
    CsmVoiceProfileGovernanceManifest::from_committed_manifest()
}

/// CSM model descriptor exported without loading neural weights.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmModelArtifactDescriptor {
    /// Psionic model id.
    pub model_id: String,
    /// CSM HF repo.
    pub csm_repo: String,
    /// Llama tokenizer repo.
    pub llama_tokenizer_repo: String,
    /// Mimi HF repo.
    pub mimi_repo: String,
    /// Mimi weight file.
    pub mimi_weight: String,
    /// Artifact digests.
    pub digests: CsmArtifactDigestSet,
    /// 33-lane framing contract.
    pub frame_contract: CsmFrameContract,
    /// Prompt voice profiles admitted by this descriptor.
    pub voice_profiles: Vec<CsmVoiceProfileDescriptor>,
}

impl CsmModelArtifactDescriptor {
    /// Builds the descriptor from the committed CSM fixture.
    pub fn from_fixture(fixture: &CsmPythonParityFixture) -> Result<Self, CsmFrontendError> {
        validate_csm_python_parity_fixture(fixture).map_err(|message| {
            CsmFrontendError::DescriptorContract {
                message: message.to_string(),
            }
        })?;

        let voice_profiles = fixture
            .prompts
            .iter()
            .map(|prompt| voice_profile_descriptor(prompt, &fixture.mimi_codebook_prefixes))
            .collect();

        Ok(Self {
            model_id: CSM_MODEL_ID.to_string(),
            csm_repo: fixture.model.csm_repo.clone(),
            llama_tokenizer_repo: fixture.model.llama_tokenizer_repo.clone(),
            mimi_repo: fixture.model.mimi_repo.clone(),
            mimi_weight: fixture.model.mimi_weight.clone(),
            digests: CsmArtifactDigestSet {
                csm_config_digest: fixture.model.csm_config_digest.clone(),
                csm_model_digest: fixture.model.csm_model_digest.clone(),
                llama_tokenizer_digest: fixture.model.llama_tokenizer_digest.clone(),
                mimi_weight_digest: fixture.model.mimi_weight_digest.clone(),
            },
            frame_contract: CsmFrameContract::default(),
            voice_profiles,
        })
    }
}

fn voice_profile_descriptor(
    prompt: &CsmParityPrompt,
    prefixes: &[CsmMimiCodebookPrefix],
) -> CsmVoiceProfileDescriptor {
    let prefix = prefixes
        .iter()
        .find(|prefix| prefix.profile_id == prompt.profile_id);
    CsmVoiceProfileDescriptor {
        profile_id: prompt.profile_id.clone(),
        speaker: prompt.speaker,
        text: prompt.text.clone(),
        prompt_audio_sha256: prompt.audio_sha256.clone(),
        audio: prompt.audio.clone(),
        mimi_tokens_sha256: prefix.map(|prefix| prefix.tokens_sha256.clone()),
        prompt_codebooks: prefix.map(|prefix| CsmVoiceProfileCodebookDescriptor {
            source: "committed_parity_fixture_digest_and_prefix".to_string(),
            sample_rate_hz: prefix.sample_rate_hz,
            codebook_count: prefix.codebook_count,
            frame_count: prefix.frame_count,
            prefix_frame_count: prefix.prefix_frame_count,
            prefix_codebook_count: prefix.prefix_codebook_count,
            tokens_sha256: prefix.tokens_sha256.clone(),
        }),
    }
}

/// CSM config root parsed from `config.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsmModelConfig {
    /// HF architecture labels.
    #[serde(default)]
    pub architectures: Vec<String>,
    /// Model type.
    pub model_type: String,
    /// Maximum position embeddings.
    pub max_position_embeddings: usize,
    /// Text vocabulary size.
    pub text_vocab_size: usize,
    /// Audio vocabulary size.
    pub audio_vocab_size: usize,
    /// Main vocab size.
    pub vocab_size: usize,
    /// Audio codebook count.
    pub num_codebooks: usize,
    /// Audio codebook count duplicate from Transformers config.
    pub audio_num_codebooks: usize,
    /// BOS token.
    pub bos_token_id: u32,
    /// Audio token.
    pub audio_token_id: u32,
    /// Audio EOS token.
    pub audio_eos_token_id: u32,
    /// Pad token.
    pub pad_token_id: u32,
    /// Codebook EOS token.
    pub codebook_eos_token_id: u32,
    /// Codebook pad token.
    pub codebook_pad_token_id: u32,
    /// Codec/Mimi nested config.
    pub codec_config: CsmCodecConfig,
    /// Depth decoder nested config.
    pub depth_decoder_config: CsmDepthDecoderConfig,
}

impl CsmModelConfig {
    /// Parses and validates CSM config JSON.
    pub fn from_json_str(json: &str) -> Result<Self, CsmFrontendError> {
        let config: Self =
            serde_json::from_str(json).map_err(|error| CsmFrontendError::ConfigParse {
                message: error.to_string(),
            })?;
        config.validate()?;
        Ok(config)
    }

    /// Parses and validates a CSM config JSON file.
    pub fn from_json_file(path: impl AsRef<Path>) -> Result<Self, CsmFrontendError> {
        Ok(Self::from_json_file_with_digest(path, None)?.0)
    }

    /// Parses a CSM config JSON file, validates its digest, and validates the
    /// config against the CSM contract.
    pub fn from_json_file_with_digest(
        path: impl AsRef<Path>,
        expected_sha256: Option<&str>,
    ) -> Result<(Self, String), CsmFrontendError> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|error| CsmFrontendError::ArtifactRead {
            artifact: path.display().to_string(),
            message: error.to_string(),
        })?;
        let actual = sha256_digest(&bytes);
        if let Some(expected) = expected_sha256
            && actual != expected
        {
            return Err(CsmFrontendError::ArtifactDigestMismatch {
                artifact: "csm_config".to_string(),
                expected: expected.to_string(),
                actual,
            });
        }
        let config: Self =
            serde_json::from_slice(&bytes).map_err(|error| CsmFrontendError::ConfigParse {
                message: error.to_string(),
            })?;
        config.validate()?;
        Ok((config, actual))
    }

    /// Validates the config against the CSM 33-lane contract.
    pub fn validate(&self) -> Result<(), CsmFrontendError> {
        if self.model_type != "csm" {
            return Err(CsmFrontendError::ConfigContract {
                message: format!("expected model_type=csm, got {}", self.model_type),
            });
        }
        if self.max_position_embeddings != CSM_MAX_SEQ_LEN {
            return Err(CsmFrontendError::ConfigContract {
                message: format!(
                    "expected max_position_embeddings={}, got {}",
                    CSM_MAX_SEQ_LEN, self.max_position_embeddings
                ),
            });
        }
        if self.num_codebooks != CSM_AUDIO_CODEBOOK_LANES
            || self.audio_num_codebooks != CSM_AUDIO_CODEBOOK_LANES
            || self.depth_decoder_config.num_codebooks != CSM_AUDIO_CODEBOOK_LANES
            || self.codec_config.num_quantizers != CSM_AUDIO_CODEBOOK_LANES
        {
            return Err(CsmFrontendError::ConfigContract {
                message: "config does not declare 32 audio codebooks".to_string(),
            });
        }
        if self.codec_config.sampling_rate != CSM_SAMPLE_RATE_HZ {
            return Err(CsmFrontendError::ConfigContract {
                message: format!(
                    "expected codec sampling_rate={}, got {}",
                    CSM_SAMPLE_RATE_HZ, self.codec_config.sampling_rate
                ),
            });
        }
        if self.codebook_eos_token_id != CSM_CODEBOOK_EOS_TOKEN_ID {
            return Err(CsmFrontendError::ConfigContract {
                message: format!(
                    "expected codebook_eos_token_id={}, got {}",
                    CSM_CODEBOOK_EOS_TOKEN_ID, self.codebook_eos_token_id
                ),
            });
        }
        if self.codebook_pad_token_id != CSM_CODEBOOK_PAD_TOKEN_ID {
            return Err(CsmFrontendError::ConfigContract {
                message: format!(
                    "expected codebook_pad_token_id={}, got {}",
                    CSM_CODEBOOK_PAD_TOKEN_ID, self.codebook_pad_token_id
                ),
            });
        }
        Ok(())
    }

    /// Builds the Candle CSM config used by the first Rust CPU generation path.
    #[must_use]
    pub fn to_candle_config(&self) -> candle_csm::Config {
        candle_csm::Config {
            audio_num_codebooks: self.audio_num_codebooks,
            audio_vocab_size: self.audio_vocab_size,
            backbone_flavor: candle_csm::Flavor::Llama1B,
            decoder_flavor: candle_csm::Flavor::Llama100M,
            text_vocab_size: self.text_vocab_size,
        }
    }
}

/// CSM nested Mimi config fields used by the Rust contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsmCodecConfig {
    /// Codec model type.
    pub model_type: String,
    /// Audio channel count.
    pub audio_channels: usize,
    /// Codebook size.
    pub codebook_size: usize,
    /// Quantizer count.
    pub num_quantizers: usize,
    /// Runtime sample rate.
    pub sampling_rate: u32,
    /// Codec frame rate.
    pub frame_rate: f32,
}

/// CSM depth decoder config fields used by the Rust contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsmDepthDecoderConfig {
    /// Decoder model type.
    pub model_type: String,
    /// Maximum decoder positions.
    pub max_position_embeddings: usize,
    /// Decoder codebook count.
    pub num_codebooks: usize,
    /// Decoder vocab size.
    pub vocab_size: usize,
}

/// Rust wrapper around Hugging Face's `tokenizer.json` for CSM text.
#[derive(Clone)]
pub struct CsmLlamaTextTokenizer {
    tokenizer: Tokenizer,
    tokenizer_sha256: String,
}

impl fmt::Debug for CsmLlamaTextTokenizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CsmLlamaTextTokenizer")
            .field("tokenizer_sha256", &self.tokenizer_sha256)
            .finish_non_exhaustive()
    }
}

impl CsmLlamaTextTokenizer {
    /// Loads a tokenizer JSON file, validates its digest when supplied, and
    /// installs the BOS/EOS template used by the Python CSM reference.
    pub fn from_tokenizer_json_file(
        path: impl AsRef<Path>,
        expected_sha256: Option<&str>,
    ) -> Result<Self, CsmFrontendError> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|error| CsmFrontendError::ArtifactRead {
            artifact: path.display().to_string(),
            message: error.to_string(),
        })?;
        Self::from_tokenizer_json_bytes(&bytes, expected_sha256)
    }

    /// Loads a tokenizer JSON byte slice.
    pub fn from_tokenizer_json_bytes(
        bytes: &[u8],
        expected_sha256: Option<&str>,
    ) -> Result<Self, CsmFrontendError> {
        let tokenizer_sha256 = sha256_digest(bytes);
        if let Some(expected) = expected_sha256
            && tokenizer_sha256 != expected
        {
            return Err(CsmFrontendError::ArtifactDigestMismatch {
                artifact: "llama_tokenizer_json".to_string(),
                expected: expected.to_string(),
                actual: tokenizer_sha256,
            });
        }

        let mut tokenizer =
            Tokenizer::from_bytes(bytes).map_err(|error| CsmFrontendError::TokenizerLoad {
                message: error.to_string(),
            })?;
        install_csm_llama_template(&mut tokenizer)?;
        Ok(Self {
            tokenizer,
            tokenizer_sha256,
        })
    }

    /// Loads the first matching tokenizer JSON from the local Hugging Face cache.
    pub fn from_default_hf_cache(expected_sha256: Option<&str>) -> Result<Self, CsmFrontendError> {
        let candidates = csm_default_llama_tokenizer_json_candidates();
        let mut errors = Vec::new();
        for candidate in candidates {
            match Self::from_tokenizer_json_file(&candidate, expected_sha256) {
                Ok(tokenizer) => return Ok(tokenizer),
                Err(error) => errors.push(format!("{}: {error}", candidate.display())),
            }
        }
        Err(CsmFrontendError::TokenizerLoad {
            message: format!(
                "no matching {CSM_LLAMA_TOKENIZER_REPO} tokenizer.json found in local HF cache ({})",
                errors.join("; ")
            ),
        })
    }

    /// Returns the tokenizer JSON digest.
    #[must_use]
    pub fn tokenizer_sha256(&self) -> &str {
        &self.tokenizer_sha256
    }

    /// Encodes raw text using the installed CSM BOS/EOS template.
    pub fn encode_raw(&self, text: &str) -> Result<Vec<u32>, CsmFrontendError> {
        self.tokenizer
            .encode(text, true)
            .map(|encoding| encoding.get_ids().to_vec())
            .map_err(|error| CsmFrontendError::TokenizerEncode {
                message: error.to_string(),
            })
    }

    /// Encodes one CSM speaker-tagged text segment as `[{speaker}]{text}`.
    pub fn encode_segment_text(
        &self,
        speaker: u32,
        text: &str,
    ) -> Result<Vec<u32>, CsmFrontendError> {
        self.encode_raw(&csm_format_segment_text(speaker, text))
    }
}

fn install_csm_llama_template(tokenizer: &mut Tokenizer) -> Result<(), CsmFrontendError> {
    let bos_id = tokenizer.token_to_id(CSM_LLAMA_BOS_TOKEN).ok_or_else(|| {
        CsmFrontendError::TokenizerLoad {
            message: format!("missing BOS token {CSM_LLAMA_BOS_TOKEN}"),
        }
    })?;
    let eos_id = tokenizer.token_to_id(CSM_LLAMA_EOS_TOKEN).ok_or_else(|| {
        CsmFrontendError::TokenizerLoad {
            message: format!("missing EOS token {CSM_LLAMA_EOS_TOKEN}"),
        }
    })?;
    if bos_id != CSM_LLAMA_BOS_TOKEN_ID || eos_id != CSM_LLAMA_EOS_TOKEN_ID {
        return Err(CsmFrontendError::TokenizerLoad {
            message: format!(
                "unexpected BOS/EOS ids: bos={bos_id}, eos={eos_id}; expected {CSM_LLAMA_BOS_TOKEN_ID}/{CSM_LLAMA_EOS_TOKEN_ID}"
            ),
        });
    }

    tokenizer.with_post_processor(Some(
        TemplateProcessing::builder()
            .try_single(format!(
                "{CSM_LLAMA_BOS_TOKEN}:0 $A:0 {CSM_LLAMA_EOS_TOKEN}:0"
            ))
            .map_err(|error| CsmFrontendError::TokenizerLoad {
                message: error.to_string(),
            })?
            .try_pair(format!(
                "{CSM_LLAMA_BOS_TOKEN}:0 $A:0 {CSM_LLAMA_EOS_TOKEN}:0 {CSM_LLAMA_BOS_TOKEN}:1 $B:1 {CSM_LLAMA_EOS_TOKEN}:1"
            ))
            .map_err(|error| CsmFrontendError::TokenizerLoad {
                message: error.to_string(),
            })?
            .special_tokens(vec![
                (CSM_LLAMA_BOS_TOKEN, CSM_LLAMA_BOS_TOKEN_ID),
                (CSM_LLAMA_EOS_TOKEN, CSM_LLAMA_EOS_TOKEN_ID),
            ])
            .build()
            .map_err(|error| CsmFrontendError::TokenizerLoad {
                message: error.to_string(),
            })?,
    ));
    Ok(())
}

/// Returns local HF cache tokenizer candidates for Llama 3.2 1B.
#[must_use]
pub fn csm_default_llama_tokenizer_json_candidates() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(hf_home) = env::var("HF_HOME") {
        roots.push(PathBuf::from(hf_home).join("hub"));
    }
    if let Ok(home) = env::var("HOME") {
        roots.push(
            PathBuf::from(home)
                .join(".cache")
                .join("huggingface")
                .join("hub"),
        );
    }

    let mut candidates = Vec::new();
    for root in roots {
        let snapshots = root
            .join("models--meta-llama--Llama-3.2-1B")
            .join("snapshots");
        let Ok(entries) = fs::read_dir(snapshots) else {
            continue;
        };
        for entry in entries.flatten() {
            let candidate = entry.path().join("tokenizer.json");
            if candidate.is_file() {
                candidates.push(candidate);
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

/// Formats one CSM text segment exactly as the reference generator does.
#[must_use]
pub fn csm_format_segment_text(speaker: u32, text: &str) -> String {
    format!("[{speaker}]{text}")
}

/// One 33-lane CSM frame.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmTokenFrame {
    /// Token ids for each lane.
    pub tokens: Vec<u32>,
    /// Active-lane mask.
    pub mask: Vec<bool>,
}

impl CsmTokenFrame {
    /// Creates an empty frame.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            tokens: vec![0; CSM_FRAME_LANES],
            mask: vec![false; CSM_FRAME_LANES],
        }
    }

    /// Creates a text-lane frame.
    #[must_use]
    pub fn text(token_id: u32) -> Self {
        let mut frame = Self::empty();
        frame.tokens[CSM_TEXT_LANE_INDEX] = token_id;
        frame.mask[CSM_TEXT_LANE_INDEX] = true;
        frame
    }

    /// Creates an audio frame from 32 codebook tokens.
    #[must_use]
    pub fn audio(codebook_tokens: [u32; CSM_AUDIO_CODEBOOK_LANES]) -> Self {
        let mut frame = Self::empty();
        frame.tokens[..CSM_AUDIO_CODEBOOK_LANES].copy_from_slice(&codebook_tokens);
        frame.mask[..CSM_AUDIO_CODEBOOK_LANES].fill(true);
        frame
    }
}

/// Contiguous CSM frame block.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmFrameBlock {
    /// Ordered frames.
    pub frames: Vec<CsmTokenFrame>,
}

impl CsmFrameBlock {
    /// Creates a frame block.
    #[must_use]
    pub fn new(frames: Vec<CsmTokenFrame>) -> Self {
        Self { frames }
    }

    /// Returns frame count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Returns whether the block is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Returns `[frames, lanes]`.
    #[must_use]
    pub fn shape(&self) -> [usize; 2] {
        [self.frames.len(), CSM_FRAME_LANES]
    }

    /// Appends another block.
    pub fn extend(&mut self, other: Self) {
        self.frames.extend(other.frames);
    }
}

/// Builds CSM text frames from token ids.
#[must_use]
pub fn csm_text_frame_block(token_ids: &[u32]) -> CsmFrameBlock {
    CsmFrameBlock::new(token_ids.iter().copied().map(CsmTokenFrame::text).collect())
}

/// Builds CSM audio frames and appends the reference all-zero codebook EOS frame.
pub fn csm_audio_frame_block(
    frames: &[[u32; CSM_AUDIO_CODEBOOK_LANES]],
) -> Result<CsmFrameBlock, CsmFrontendError> {
    let mut output = Vec::with_capacity(frames.len() + 1);
    for frame in frames {
        if let Some(token) = frame
            .iter()
            .find(|token| **token > CSM_CODEBOOK_PAD_TOKEN_ID)
        {
            return Err(CsmFrontendError::AudioCodebooks {
                message: format!("audio token {token} exceeds CSM codebook vocab"),
            });
        }
        output.push(CsmTokenFrame::audio(*frame));
    }
    output.push(CsmTokenFrame::audio(
        [CSM_CODEBOOK_EOS_TOKEN_ID; CSM_AUDIO_CODEBOOK_LANES],
    ));
    Ok(CsmFrameBlock::new(output))
}

/// Native mono PCM clip for CSM/Mimi output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsmAudioClip {
    /// Sample rate in Hz.
    pub sample_rate_hz: u32,
    /// Channel count.
    pub channels: u16,
    /// Interleaved PCM samples in `[-1.0, 1.0]`.
    pub samples: Vec<f32>,
}

impl CsmAudioClip {
    /// Creates one PCM clip.
    #[must_use]
    pub fn new(sample_rate_hz: u32, channels: u16, samples: Vec<f32>) -> Self {
        Self {
            sample_rate_hz,
            channels: channels.max(1),
            samples,
        }
    }

    /// Returns clip length in sample frames.
    #[must_use]
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels)
    }

    /// Returns clip duration in milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> u64 {
        let frames = self.frames() as f64;
        let sample_rate = f64::from(self.sample_rate_hz.max(1));
        ((frames / sample_rate) * 1000.0).round() as u64
    }

    /// Returns a stable digest over clip metadata and f32 sample bytes.
    #[must_use]
    pub fn digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.sample_rate_hz.to_le_bytes());
        hasher.update(self.channels.to_le_bytes());
        for sample in &self.samples {
            hasher.update(sample.to_le_bytes());
        }
        format!("sha256:{:x}", hasher.finalize())
    }

    /// Encodes this clip as browser-playable PCM16 WAV bytes.
    pub fn to_wav_pcm16(&self) -> Result<Vec<u8>, CsmFrontendError> {
        csm_encode_wav_pcm16(self)
    }
}

/// WAV/PCM16 encoder for browser-playable CSM output.
pub fn csm_encode_wav_pcm16(clip: &CsmAudioClip) -> Result<Vec<u8>, CsmFrontendError> {
    let channels = usize::from(clip.channels);
    if channels == 0 {
        return Err(CsmFrontendError::WavEncode {
            message: "channel count must be positive".to_string(),
        });
    }
    if clip.samples.len() % channels != 0 {
        return Err(CsmFrontendError::WavEncode {
            message: "interleaved sample length is not divisible by channels".to_string(),
        });
    }

    let bytes_per_sample = 2usize;
    let data_len = clip
        .samples
        .len()
        .checked_mul(bytes_per_sample)
        .ok_or_else(|| CsmFrontendError::WavEncode {
            message: "wav data too large".to_string(),
        })?;
    let data_len_u32 = u32::try_from(data_len).map_err(|error| CsmFrontendError::WavEncode {
        message: error.to_string(),
    })?;
    let riff_len = 36u32
        .checked_add(data_len_u32)
        .ok_or_else(|| CsmFrontendError::WavEncode {
            message: "wav riff length overflow".to_string(),
        })?;
    let byte_rate = clip
        .sample_rate_hz
        .checked_mul(u32::from(clip.channels))
        .and_then(|value| value.checked_mul(bytes_per_sample as u32))
        .ok_or_else(|| CsmFrontendError::WavEncode {
            message: "wav byte rate overflow".to_string(),
        })?;
    let block_align = clip
        .channels
        .checked_mul(bytes_per_sample as u16)
        .ok_or_else(|| CsmFrontendError::WavEncode {
            message: "wav block align overflow".to_string(),
        })?;

    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&clip.channels.to_le_bytes());
    out.extend_from_slice(&clip.sample_rate_hz.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len_u32.to_le_bytes());

    for sample in &clip.samples {
        let pcm = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        out.extend_from_slice(&pcm.to_le_bytes());
    }
    Ok(out)
}

/// Hashes encoded PCM16 WAV bytes for one clip.
pub fn csm_wav_pcm16_digest(clip: &CsmAudioClip) -> Result<String, CsmFrontendError> {
    Ok(sha256_digest(&clip.to_wav_pcm16()?))
}

/// Rust CPU CSM sampling mode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum CsmSamplingStrategy {
    /// Deterministic argmax at every codebook head.
    Greedy,
    /// Fixed-seed top-k sampling.
    TopK {
        /// Top-k candidate count.
        top_k: usize,
        /// Sampling temperature.
        temperature: f64,
        /// RNG seed.
        seed: u64,
    },
}

impl CsmSamplingStrategy {
    fn logits_processor(&self) -> Result<LogitsProcessor, CsmFrontendError> {
        match *self {
            Self::Greedy => Ok(LogitsProcessor::from_sampling(0, Sampling::ArgMax)),
            Self::TopK {
                top_k,
                temperature,
                seed,
            } => {
                if top_k == 0 {
                    return Err(CsmFrontendError::UnsupportedSampling {
                        message: "top_k must be greater than zero".to_string(),
                    });
                }
                if !temperature.is_finite() || temperature <= 0.0 {
                    return Err(CsmFrontendError::UnsupportedSampling {
                        message: "temperature must be finite and positive".to_string(),
                    });
                }
                Ok(LogitsProcessor::from_sampling(
                    seed,
                    Sampling::TopK {
                        k: top_k,
                        temperature,
                    },
                ))
            }
        }
    }
}

/// Rust CPU CSM generation request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsmCpuGenerationRequest {
    /// Prompt frames produced by the Rust CSM frontend.
    pub prompt: CsmPromptFramePlan,
    /// Sampling strategy.
    pub sampling: CsmSamplingStrategy,
}

/// Rust CPU CSM generation evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsmCpuGenerationReport {
    /// Generated codebook frames.
    pub codebook_frames: Vec<[u32; CSM_AUDIO_CODEBOOK_LANES]>,
    /// Stable digest over generated codebook frames.
    pub frames_sha256: String,
    /// Number of generated non-EOS frames.
    pub generated_frame_count: usize,
    /// Whether generation stopped on all-zero EOS.
    pub hit_eos: bool,
    /// Prompt frame count.
    pub prompt_frame_count: usize,
    /// Max generation frame budget.
    pub max_generation_len: usize,
    /// Backend label.
    pub backend: String,
    /// Execution engine label.
    pub execution_engine: String,
    /// CSM config digest.
    pub csm_config_digest: String,
    /// CSM model weight digest.
    pub csm_model_digest: String,
    /// Wall-clock generation latency in milliseconds.
    pub latency_ms: u128,
}

/// One generated CSM codebook window emitted before full turn completion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsmCpuGeneratedWindow {
    /// Zero-based index of the first generated frame in this window.
    pub start_frame_index: usize,
    /// Generated 32-codebook audio frames in this window.
    pub codebook_frames: Vec<[u32; CSM_AUDIO_CODEBOOK_LANES]>,
    /// True when this is the final emitted window for this request.
    pub final_window: bool,
}

/// Rust CPU CSM one-shot generation plus Mimi decode evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsmCpuSpeechSynthesisReport {
    /// Generated codebooks.
    pub generation: CsmCpuGenerationReport,
    /// Mimi decode report.
    pub decode: CsmMimiDecodeReport,
}

/// Rust CPU CSM generator backed by Candle's Rust CSM module.
pub struct CsmCpuGenerator {
    model: candle_csm::Model,
    config_digest: String,
    model_digest: String,
    backend: CsmRuntimeBackend,
    device: moshi::candle::Device,
}

impl fmt::Debug for CsmCpuGenerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CsmCpuGenerator")
            .field("config_digest", &self.config_digest)
            .field("model_digest", &self.model_digest)
            .field("backend", &self.backend.label())
            .field("execution_engine", &self.backend.execution_engine())
            .finish_non_exhaustive()
    }
}

impl CsmCpuGenerator {
    /// Loads the CSM safetensors file into the Rust CPU generator.
    pub fn from_safetensors_file(
        config: &CsmModelConfig,
        config_digest: impl Into<String>,
        path: impl AsRef<Path>,
        expected_model_sha256: Option<&str>,
    ) -> Result<Self, CsmFrontendError> {
        Self::from_safetensors_file_with_backend(
            config,
            config_digest,
            path,
            expected_model_sha256,
            CsmRuntimeBackend::Cpu,
        )
    }

    /// Loads the CSM safetensors file into the requested Rust backend.
    pub fn from_safetensors_file_with_backend(
        config: &CsmModelConfig,
        config_digest: impl Into<String>,
        path: impl AsRef<Path>,
        expected_model_sha256: Option<&str>,
        backend: CsmRuntimeBackend,
    ) -> Result<Self, CsmFrontendError> {
        config.validate()?;
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|error| CsmFrontendError::ArtifactRead {
            artifact: path.display().to_string(),
            message: error.to_string(),
        })?;
        let model_digest = sha256_digest(&bytes);
        if let Some(expected) = expected_model_sha256
            && model_digest != expected
        {
            return Err(CsmFrontendError::ArtifactDigestMismatch {
                artifact: "csm_model".to_string(),
                expected: expected.to_string(),
                actual: model_digest,
            });
        }
        let device = backend.device()?;
        let weights = [path];
        let vb = unsafe {
            moshi::candle_nn::VarBuilder::from_mmaped_safetensors(
                &weights,
                moshi::candle::DType::F32,
                &device,
            )
        }
        .map_err(|error| CsmFrontendError::CsmModelLoad {
            message: error.to_string(),
        })?;
        let model = candle_csm::Model::new(&config.to_candle_config(), vb).map_err(|error| {
            CsmFrontendError::CsmModelLoad {
                message: error.to_string(),
            }
        })?;
        Ok(Self {
            model,
            config_digest: config_digest.into(),
            model_digest,
            backend,
            device,
        })
    }

    /// Returns the CSM model weight digest.
    #[must_use]
    pub fn model_digest(&self) -> &str {
        &self.model_digest
    }

    /// Generates CSM codebook frames from a prepared prompt plan.
    pub fn generate_codebook_frames(
        &mut self,
        request: &CsmCpuGenerationRequest,
    ) -> Result<CsmCpuGenerationReport, CsmFrontendError> {
        self.generate_codebook_frames_with_window_callback(
            request,
            request.prompt.max_generation_len.max(1),
            |_| Ok(()),
        )
    }

    /// Generates CSM codebook frames and calls `on_window` as soon as each
    /// generated codebook window is available. This lets serving layers decode
    /// and flush audio before full response generation completes.
    pub fn generate_codebook_frames_with_window_callback<F>(
        &mut self,
        request: &CsmCpuGenerationRequest,
        window_frame_count: usize,
        mut on_window: F,
    ) -> Result<CsmCpuGenerationReport, CsmFrontendError>
    where
        F: FnMut(CsmCpuGeneratedWindow) -> Result<(), CsmFrontendError>,
    {
        let started = Instant::now();
        self.model.clear_kv_cache();
        let mut lp = request.sampling.logits_processor()?;
        let prompt_frame_count = request.prompt.frames.len();
        if prompt_frame_count == 0 {
            return Err(CsmFrontendError::CsmGeneration {
                message: "prompt must contain at least one frame".to_string(),
            });
        }
        let (mut tokens, mut tokens_mask) =
            csm_frame_block_to_candle_tensors(&request.prompt.frames, &self.device)?;
        let mut input_pos = 0usize;
        let mut generated = Vec::with_capacity(request.prompt.max_generation_len);
        let mut pending_window = Vec::with_capacity(window_frame_count.max(1));
        let window_frame_count = window_frame_count.max(1);
        let mut hit_eos = false;

        for _ in 0..request.prompt.max_generation_len {
            let sample = self
                .model
                .generate_frame(&tokens, &tokens_mask, input_pos, &mut lp)
                .map_err(|error| CsmFrontendError::CsmGeneration {
                    message: error.to_string(),
                })?;
            let frame = csm_vec_to_codebook_frame(sample)?;
            input_pos += tokens
                .dim(1)
                .map_err(|error| CsmFrontendError::CsmGeneration {
                    message: error.to_string(),
                })?;
            if frame
                .iter()
                .all(|token| *token == CSM_CODEBOOK_EOS_TOKEN_ID)
            {
                hit_eos = true;
                break;
            }
            generated.push(frame);
            pending_window.push(frame);
            if pending_window.len() >= window_frame_count {
                let start_frame_index = generated.len().saturating_sub(pending_window.len());
                let final_window = generated.len() >= request.prompt.max_generation_len;
                on_window(CsmCpuGeneratedWindow {
                    start_frame_index,
                    codebook_frames: std::mem::take(&mut pending_window),
                    final_window,
                })?;
            }
            let (next_tokens, next_mask) = self
                .model
                .audio_tokens_and_mask(frame.to_vec())
                .map_err(|error| CsmFrontendError::CsmGeneration {
                    message: error.to_string(),
                })?;
            tokens = next_tokens;
            tokens_mask = next_mask;
        }

        if !pending_window.is_empty() {
            let start_frame_index = generated.len().saturating_sub(pending_window.len());
            on_window(CsmCpuGeneratedWindow {
                start_frame_index,
                codebook_frames: pending_window,
                final_window: true,
            })?;
        }

        let frames_sha256 = csm_codebook_frames_digest(&generated);
        let generated_frame_count = generated.len();
        Ok(CsmCpuGenerationReport {
            codebook_frames: generated,
            frames_sha256,
            generated_frame_count,
            hit_eos,
            prompt_frame_count,
            max_generation_len: request.prompt.max_generation_len,
            backend: self.backend.label().to_string(),
            execution_engine: self.backend.execution_engine().to_string(),
            csm_config_digest: self.config_digest.clone(),
            csm_model_digest: self.model_digest.clone(),
            latency_ms: started.elapsed().as_millis(),
        })
    }

    /// Generates codebooks and decodes them through Rust Mimi into WAV-ready PCM.
    pub fn generate_and_decode(
        &mut self,
        request: &CsmCpuGenerationRequest,
        mimi: &mut CsmMimiDecoder,
    ) -> Result<CsmCpuSpeechSynthesisReport, CsmFrontendError> {
        let generation = self.generate_codebook_frames(request)?;
        let decode = mimi.decode_codebook_frames(&generation.codebook_frames)?;
        Ok(CsmCpuSpeechSynthesisReport { generation, decode })
    }
}

fn csm_frame_block_to_candle_tensors(
    block: &CsmFrameBlock,
    device: &moshi::candle::Device,
) -> Result<(moshi::candle::Tensor, moshi::candle::Tensor), CsmFrontendError> {
    let frame_count = block.len();
    if frame_count == 0 {
        return Err(CsmFrontendError::CsmGeneration {
            message: "cannot build model tensors for an empty frame block".to_string(),
        });
    }
    let mut tokens = Vec::with_capacity(frame_count * CSM_FRAME_LANES);
    let mut mask = Vec::with_capacity(frame_count * CSM_FRAME_LANES);
    for frame in &block.frames {
        if frame.tokens.len() != CSM_FRAME_LANES || frame.mask.len() != CSM_FRAME_LANES {
            return Err(CsmFrontendError::DescriptorContract {
                message: "CSM frame has invalid lane width".to_string(),
            });
        }
        tokens.extend_from_slice(&frame.tokens);
        mask.extend(frame.mask.iter().map(|enabled| u8::from(*enabled)));
    }
    let tokens = moshi::candle::Tensor::from_vec(tokens, (1, frame_count, CSM_FRAME_LANES), device)
        .map_err(|error| CsmFrontendError::CsmGeneration {
            message: error.to_string(),
        })?;
    let mask = moshi::candle::Tensor::from_vec(mask, (1, frame_count, CSM_FRAME_LANES), device)
        .map_err(|error| CsmFrontendError::CsmGeneration {
            message: error.to_string(),
        })?;
    Ok((tokens, mask))
}

fn csm_vec_to_codebook_frame(
    sample: Vec<u32>,
) -> Result<[u32; CSM_AUDIO_CODEBOOK_LANES], CsmFrontendError> {
    let boxed: Box<[u32]> = sample.into_boxed_slice();
    let boxed: Box<[u32; CSM_AUDIO_CODEBOOK_LANES]> =
        boxed
            .try_into()
            .map_err(|boxed: Box<[u32]>| CsmFrontendError::CsmGeneration {
                message: format!(
                    "generated frame has {} codebooks, expected {}",
                    boxed.len(),
                    CSM_AUDIO_CODEBOOK_LANES
                ),
            })?;
    Ok(*boxed)
}

/// Returns a stable digest over frame-major codebook tokens.
#[must_use]
pub fn csm_codebook_frames_digest(frames: &[[u32; CSM_AUDIO_CODEBOOK_LANES]]) -> String {
    let mut hasher = Sha256::new();
    for frame in frames {
        for token in frame {
            hasher.update(token.to_le_bytes());
        }
    }
    format!("sha256:{:x}", hasher.finalize())
}

/// Decode report for one Rust Mimi run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsmMimiDecodeReport {
    /// Output clip.
    pub clip: CsmAudioClip,
    /// Clip digest over f32 samples.
    pub clip_digest: String,
    /// Digest of browser-playable PCM16 WAV bytes.
    pub wav_pcm16_digest: String,
    /// Decoded input frame count after trailing EOS removal.
    pub decoded_codebook_frames: usize,
    /// Mimi weight digest.
    pub mimi_weight_digest: String,
    /// Execution engine label.
    pub execution_engine: String,
}

/// Rust Mimi decoder backed by Kyutai's Rust `moshi` crate.
pub struct CsmMimiDecoder {
    model: moshi::mimi::Mimi,
    mimi_weight_digest: String,
    backend: CsmRuntimeBackend,
    device: moshi::candle::Device,
}

impl fmt::Debug for CsmMimiDecoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CsmMimiDecoder")
            .field("mimi_weight_digest", &self.mimi_weight_digest)
            .field("backend", &self.backend.label())
            .field("execution_engine", &self.backend.mimi_execution_engine())
            .finish_non_exhaustive()
    }
}

impl CsmMimiDecoder {
    /// Loads the Mimi safetensors file into the Rust CPU decoder.
    pub fn from_safetensors_file(
        path: impl AsRef<Path>,
        expected_sha256: Option<&str>,
    ) -> Result<Self, CsmFrontendError> {
        Self::from_safetensors_file_with_backend(path, expected_sha256, CsmRuntimeBackend::Cpu)
    }

    /// Loads the Mimi safetensors file into the requested Rust backend.
    pub fn from_safetensors_file_with_backend(
        path: impl AsRef<Path>,
        expected_sha256: Option<&str>,
        backend: CsmRuntimeBackend,
    ) -> Result<Self, CsmFrontendError> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|error| CsmFrontendError::ArtifactRead {
            artifact: path.display().to_string(),
            message: error.to_string(),
        })?;
        let mimi_weight_digest = sha256_digest(&bytes);
        if let Some(expected) = expected_sha256
            && mimi_weight_digest != expected
        {
            return Err(CsmFrontendError::ArtifactDigestMismatch {
                artifact: "mimi_weight".to_string(),
                expected: expected.to_string(),
                actual: mimi_weight_digest,
            });
        }
        let path = path.to_str().ok_or_else(|| CsmFrontendError::MimiLoad {
            message: "Mimi weight path is not valid UTF-8".to_string(),
        })?;
        let device = backend
            .device()
            .map_err(|error| CsmFrontendError::MimiLoad {
                message: error.to_string(),
            })?;
        let model =
            moshi::mimi::load(path, Some(CSM_AUDIO_CODEBOOK_LANES), &device).map_err(|error| {
                CsmFrontendError::MimiLoad {
                    message: error.to_string(),
                }
            })?;
        if model.config().sample_rate.round() as u32 != CSM_SAMPLE_RATE_HZ {
            return Err(CsmFrontendError::MimiLoad {
                message: format!(
                    "expected Mimi sample_rate={}, got {}",
                    CSM_SAMPLE_RATE_HZ,
                    model.config().sample_rate
                ),
            });
        }
        if model.config().quantizer_n_q != CSM_AUDIO_CODEBOOK_LANES {
            return Err(CsmFrontendError::MimiLoad {
                message: format!(
                    "expected {} codebooks, got {}",
                    CSM_AUDIO_CODEBOOK_LANES,
                    model.config().quantizer_n_q
                ),
            });
        }
        Ok(Self {
            model,
            mimi_weight_digest,
            backend,
            device,
        })
    }

    /// Returns the loaded Mimi weight digest.
    #[must_use]
    pub fn mimi_weight_digest(&self) -> &str {
        &self.mimi_weight_digest
    }

    /// Decodes 32-codebook generated CSM frames into mono 24 kHz PCM.
    pub fn decode_codebook_frames(
        &mut self,
        frames: &[[u32; CSM_AUDIO_CODEBOOK_LANES]],
    ) -> Result<CsmMimiDecodeReport, CsmFrontendError> {
        let frames = strip_trailing_codebook_eos_frames(frames);
        if frames.is_empty() {
            return Err(CsmFrontendError::AudioCodebooks {
                message: "no non-EOS Mimi codebook frames to decode".to_string(),
            });
        }
        let tensor_data = codebook_frames_to_bkt_tensor_data(frames)?;
        let codes = moshi::candle::Tensor::from_vec(
            tensor_data,
            (1, CSM_AUDIO_CODEBOOK_LANES, frames.len()),
            &self.device,
        )
        .map_err(|error| CsmFrontendError::MimiDecode {
            message: error.to_string(),
        })?;
        let decoded = self
            .model
            .decode(&codes)
            .map_err(|error| CsmFrontendError::MimiDecode {
                message: error.to_string(),
            })?;
        let samples = decoded
            .flatten_all()
            .and_then(|tensor| tensor.to_vec1::<f32>())
            .map_err(|error| CsmFrontendError::MimiDecode {
                message: error.to_string(),
            })?;
        let clip = CsmAudioClip::new(CSM_SAMPLE_RATE_HZ, 1, samples);
        let clip_digest = clip.digest();
        let wav_pcm16_digest = csm_wav_pcm16_digest(&clip)?;
        Ok(CsmMimiDecodeReport {
            clip,
            clip_digest,
            wav_pcm16_digest,
            decoded_codebook_frames: frames.len(),
            mimi_weight_digest: self.mimi_weight_digest.clone(),
            execution_engine: self.backend.mimi_execution_engine().to_string(),
        })
    }
}

fn strip_trailing_codebook_eos_frames(
    frames: &[[u32; CSM_AUDIO_CODEBOOK_LANES]],
) -> &[[u32; CSM_AUDIO_CODEBOOK_LANES]] {
    let mut end = frames.len();
    while end > 0
        && frames[end - 1]
            .iter()
            .all(|token| *token == CSM_CODEBOOK_EOS_TOKEN_ID)
    {
        end -= 1;
    }
    &frames[..end]
}

fn codebook_frames_to_bkt_tensor_data(
    frames: &[[u32; CSM_AUDIO_CODEBOOK_LANES]],
) -> Result<Vec<u32>, CsmFrontendError> {
    let mut data = Vec::with_capacity(frames.len() * CSM_AUDIO_CODEBOOK_LANES);
    for codebook in 0..CSM_AUDIO_CODEBOOK_LANES {
        for frame in frames {
            let token = frame[codebook];
            if token > CSM_CODEBOOK_PAD_TOKEN_ID {
                return Err(CsmFrontendError::AudioCodebooks {
                    message: format!("audio token {token} exceeds CSM codebook vocab"),
                });
            }
            data.push(token);
        }
    }
    Ok(data)
}

/// Converts the frozen deterministic generation case into frame-major codebooks.
pub fn csm_generation_case_codebook_frames(
    case: &CsmDeterministicGenerationCase,
) -> Result<Vec<[u32; CSM_AUDIO_CODEBOOK_LANES]>, CsmFrontendError> {
    case.frames
        .iter()
        .map(|frame| {
            let array: [u32; CSM_AUDIO_CODEBOOK_LANES] =
                frame
                    .as_slice()
                    .try_into()
                    .map_err(|_| CsmFrontendError::AudioCodebooks {
                        message: format!(
                            "expected {} codebooks, got {}",
                            CSM_AUDIO_CODEBOOK_LANES,
                            frame.len()
                        ),
                    })?;
            if let Some(token) = array
                .iter()
                .find(|token| **token > CSM_CODEBOOK_PAD_TOKEN_ID)
            {
                return Err(CsmFrontendError::AudioCodebooks {
                    message: format!("audio token {token} exceeds CSM codebook vocab"),
                });
            }
            Ok(array)
        })
        .collect()
}

/// Verifies whether the committed deterministic generation fixture has enough
/// prompt codebooks for exact replay through the Rust generator.
pub fn csm_deterministic_fixture_replay_ready(
    fixture: &CsmPythonParityFixture,
) -> Result<(), CsmFrontendError> {
    let case = &fixture.deterministic_generation_case;
    if case.status != "available" {
        return Err(CsmFrontendError::FixtureReplayUnavailable {
            message: "deterministic generation case is not available".to_string(),
        });
    }
    let Some(prefix) = fixture.mimi_codebook_prefixes.first() else {
        return Err(CsmFrontendError::FixtureReplayUnavailable {
            message: "fixture has no prompt-codebook descriptor".to_string(),
        });
    };
    if prefix.prefix_frame_count < prefix.frame_count {
        return Err(CsmFrontendError::FixtureReplayUnavailable {
            message: format!(
                "fixture keeps only {} of {} prompt codebook frames; exact replay requires full prompt codebooks or Rust Mimi encode",
                prefix.prefix_frame_count, prefix.frame_count
            ),
        });
    }
    Ok(())
}

fn csm_hf_cache_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(hf_home) = env::var("HF_HOME") {
        roots.push(PathBuf::from(hf_home).join("hub"));
    }
    if let Ok(home) = env::var("HOME") {
        roots.push(
            PathBuf::from(home)
                .join(".cache")
                .join("huggingface")
                .join("hub"),
        );
    }
    roots
}

/// Returns local HF cache candidates for the CSM model safetensors weight.
#[must_use]
pub fn csm_default_model_weight_candidates(expected_sha256: Option<&str>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for root in csm_hf_cache_roots() {
        let repo = root.join("models--sesame--csm-1b");
        if let Some(hex) = expected_sha256.and_then(|digest| digest.strip_prefix("sha256:")) {
            let blob = repo.join("blobs").join(hex);
            if blob.is_file() {
                candidates.push(blob);
            }
        }
        let snapshots = repo.join("snapshots");
        let Ok(entries) = fs::read_dir(snapshots) else {
            continue;
        };
        for entry in entries.flatten() {
            let candidate = entry.path().join("model.safetensors");
            if candidate.is_file() {
                candidates.push(candidate);
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

/// Returns local HF cache candidates for the CSM config JSON.
#[must_use]
pub fn csm_default_config_candidates(expected_sha256: Option<&str>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for root in csm_hf_cache_roots() {
        let repo = root.join("models--sesame--csm-1b");
        if let Some(hex) = expected_sha256.and_then(|digest| digest.strip_prefix("sha256:")) {
            let blob = repo.join("blobs").join(hex);
            if blob.is_file() {
                candidates.push(blob);
            }
        }
        let snapshots = repo.join("snapshots");
        let Ok(entries) = fs::read_dir(snapshots) else {
            continue;
        };
        for entry in entries.flatten() {
            let candidate = entry.path().join("config.json");
            if candidate.is_file() {
                candidates.push(candidate);
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

/// Returns local HF cache candidates for the Mimi safetensors weight.
#[must_use]
pub fn csm_default_mimi_weight_candidates(expected_sha256: Option<&str>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for root in csm_hf_cache_roots() {
        let repo = root.join("models--kyutai--moshiko-pytorch-bf16");
        if let Some(hex) = expected_sha256.and_then(|digest| digest.strip_prefix("sha256:")) {
            let blob = repo.join("blobs").join(hex);
            if blob.is_file() {
                candidates.push(blob);
            }
        }
        let snapshots = repo.join("snapshots");
        let Ok(entries) = fs::read_dir(snapshots) else {
            continue;
        };
        for entry in entries.flatten() {
            let candidate = entry.path().join(CSM_MIMI_WEIGHT);
            if candidate.is_file() {
                candidates.push(candidate);
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

/// Explicit capability/refusal publication for CSM runtime reference-audio encoding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmCapabilityRefusal {
    /// Stable refusal code.
    pub code: String,
    /// Human-readable reason.
    pub reason: String,
    /// Required future phase.
    pub required_phase: String,
}

/// Returns the current Rust Mimi encode refusal.
#[must_use]
pub fn csm_reference_audio_encoding_refusal() -> CsmCapabilityRefusal {
    CsmCapabilityRefusal {
        code: CSM_REFERENCE_AUDIO_ENCODING_UNSUPPORTED_CODE.to_string(),
        reason: "Runtime reference-audio encoding is not implemented in the Rust CSM path; approved voice profiles must use precomputed prompt-codebook descriptors"
            .to_string(),
        required_phase: "rust_mimi_encode_or_prompt_codebook_refresh".to_string(),
    }
}

/// One prepared context or generation segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmPromptSegment {
    /// CSM speaker id.
    pub speaker: u32,
    /// Original text.
    pub text: String,
    /// Llama token ids for `[{speaker}]{text}`.
    pub text_token_ids: Vec<u32>,
    /// Optional 32-codebook Mimi frames. Context segments have this; target
    /// generation segments do not.
    pub audio_codebook_frames: Option<Vec<[u32; CSM_AUDIO_CODEBOOK_LANES]>>,
}

impl CsmPromptSegment {
    /// Creates a text-only segment from already encoded token ids.
    #[must_use]
    pub fn text_only(speaker: u32, text: impl Into<String>, text_token_ids: Vec<u32>) -> Self {
        Self {
            speaker,
            text: text.into(),
            text_token_ids,
            audio_codebook_frames: None,
        }
    }

    /// Creates a context segment with audio codebooks.
    #[must_use]
    pub fn with_audio_codebooks(
        speaker: u32,
        text: impl Into<String>,
        text_token_ids: Vec<u32>,
        audio_codebook_frames: Vec<[u32; CSM_AUDIO_CODEBOOK_LANES]>,
    ) -> Self {
        Self {
            speaker,
            text: text.into(),
            text_token_ids,
            audio_codebook_frames: Some(audio_codebook_frames),
        }
    }

    /// Encodes a text-only segment with a Rust CSM tokenizer.
    pub fn encode_text_only(
        tokenizer: &CsmLlamaTextTokenizer,
        speaker: u32,
        text: impl Into<String>,
    ) -> Result<Self, CsmFrontendError> {
        let text = text.into();
        let text_token_ids = tokenizer.encode_segment_text(speaker, &text)?;
        Ok(Self::text_only(speaker, text, text_token_ids))
    }

    /// Builds the segment's text and optional audio frame block.
    pub fn frame_block(&self) -> Result<CsmFrameBlock, CsmFrontendError> {
        let mut block = csm_text_frame_block(&self.text_token_ids);
        if let Some(audio_codebook_frames) = &self.audio_codebook_frames {
            block.extend(csm_audio_frame_block(audio_codebook_frames)?);
        }
        Ok(block)
    }
}

/// Prompt overflow policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CsmContextWindowPolicy {
    /// Refuse prompts that exceed the window.
    Reject,
    /// Drop oldest context segments until the target text fits or refusal is unavoidable.
    DropOldestSegments,
}

/// Prompt window settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmGenerationWindow {
    /// Max sequence length.
    pub max_seq_len: usize,
    /// Requested generation duration.
    pub max_audio_length_ms: u64,
    /// Frame duration.
    pub generation_frame_ms: u64,
}

impl CsmGenerationWindow {
    /// Creates the standard CSM generation window.
    #[must_use]
    pub const fn new(max_audio_length_ms: u64) -> Self {
        Self {
            max_seq_len: CSM_MAX_SEQ_LEN,
            max_audio_length_ms,
            generation_frame_ms: CSM_GENERATION_FRAME_MS,
        }
    }

    /// Returns the generated frame budget.
    pub fn max_generation_len(self) -> Result<usize, CsmFrontendError> {
        if self.generation_frame_ms == 0 {
            return Err(CsmFrontendError::GenerationWindow {
                message: "generation_frame_ms must be positive".to_string(),
            });
        }
        let frames = self.max_audio_length_ms / self.generation_frame_ms;
        if frames == 0 {
            return Err(CsmFrontendError::GenerationWindow {
                message: "max_audio_length_ms is below one CSM frame".to_string(),
            });
        }
        usize::try_from(frames).map_err(|error| CsmFrontendError::GenerationWindow {
            message: error.to_string(),
        })
    }

    /// Returns the prompt context limit.
    pub fn max_context_len(self) -> Result<usize, CsmFrontendError> {
        let max_generation_len = self.max_generation_len()?;
        if max_generation_len >= self.max_seq_len {
            return Err(CsmFrontendError::GenerationWindow {
                message: format!(
                    "max_generation_len={max_generation_len} leaves no prompt context"
                ),
            });
        }
        Ok(self.max_seq_len - max_generation_len)
    }
}

/// Prompt-frame plan after optional segment-boundary truncation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmPromptFramePlan {
    /// Final frames sent to the model prompt.
    pub frames: CsmFrameBlock,
    /// Number of original context segments retained.
    pub retained_context_segments: usize,
    /// Number of original context segments dropped.
    pub dropped_context_segments: usize,
    /// Generated frame budget.
    pub max_generation_len: usize,
    /// Prompt context limit.
    pub max_context_len: usize,
}

/// Builds a CSM prompt from context and target text.
pub fn csm_build_prompt_frame_plan(
    context: &[CsmPromptSegment],
    target_text: &CsmPromptSegment,
    window: CsmGenerationWindow,
    policy: CsmContextWindowPolicy,
) -> Result<CsmPromptFramePlan, CsmFrontendError> {
    let max_generation_len = window.max_generation_len()?;
    let max_context_len = window.max_context_len()?;
    let mut first_context_index = 0;

    loop {
        let mut frames = CsmFrameBlock::default();
        for segment in &context[first_context_index..] {
            frames.extend(segment.frame_block()?);
        }
        frames.extend(csm_text_frame_block(&target_text.text_token_ids));

        if frames.len() < max_context_len {
            return Ok(CsmPromptFramePlan {
                frames,
                retained_context_segments: context.len() - first_context_index,
                dropped_context_segments: first_context_index,
                max_generation_len,
                max_context_len,
            });
        }

        if policy == CsmContextWindowPolicy::Reject || first_context_index >= context.len() {
            return Err(CsmFrontendError::PromptContextOverflow {
                prompt_frames: frames.len(),
                max_context_len,
                max_generation_len,
            });
        }
        first_context_index += 1;
    }
}

/// Computes a `sha256:<hex>` digest.
#[must_use]
pub fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csm_python_parity_fixture;

    const CONFIG_JSON: &str = r#"{
      "architectures": ["CsmForConditionalGeneration"],
      "audio_eos_token_id": 128003,
      "audio_token_id": 128002,
      "bos_token_id": 128000,
      "codebook_eos_token_id": 0,
      "codebook_pad_token_id": 2050,
      "codec_config": {
        "audio_channels": 1,
        "codebook_size": 2048,
        "frame_rate": 12.5,
        "model_type": "mimi",
        "num_quantizers": 32,
        "sampling_rate": 24000
      },
      "depth_decoder_config": {
        "max_position_embeddings": 33,
        "model_type": "csm_depth_decoder_model",
        "num_codebooks": 32,
        "vocab_size": 2051
      },
      "max_position_embeddings": 2048,
      "model_type": "csm",
      "num_codebooks": 32,
      "pad_token_id": 128002,
      "text_vocab_size": 128256,
      "vocab_size": 2051,
      "audio_num_codebooks": 32,
      "audio_vocab_size": 2051
    }"#;

    #[test]
    fn csm_descriptor_from_fixture_exports_artifacts_and_voice_profiles() {
        let fixture = csm_python_parity_fixture().expect("fixture should parse");
        let descriptor =
            CsmModelArtifactDescriptor::from_fixture(&fixture).expect("descriptor should build");

        assert_eq!(descriptor.model_id, CSM_MODEL_ID);
        assert_eq!(
            descriptor.frame_contract.text_lane_index,
            CSM_TEXT_LANE_INDEX
        );
        assert_eq!(descriptor.voice_profiles.len(), 2);
        assert!(
            descriptor
                .voice_profiles
                .iter()
                .any(|profile| profile.profile_id == "conversational_a"
                    && profile.mimi_tokens_sha256.is_some()
                    && profile.prompt_codebooks.is_some())
        );
        assert_eq!(
            descriptor.digests.csm_config_digest,
            "sha256:b203c014cb5a2f7b4f98d2e945f091182aceb17fa530ce968e8c3437e01a9b70"
        );
        assert_eq!(
            descriptor.digests.csm_model_digest,
            "sha256:2e7721144afe38b906d4f1048671da639fe142423f4a26283606ecebe894f4bf"
        );
    }

    #[test]
    fn csm_voice_profile_governance_manifest_admits_openagents_default_female() {
        let fixture = csm_python_parity_fixture().expect("fixture should parse");
        let descriptor =
            CsmModelArtifactDescriptor::from_fixture(&fixture).expect("descriptor should build");
        let manifest =
            csm_voice_profile_governance_manifest().expect("governance manifest should parse");
        manifest
            .validate_against_descriptor(&descriptor)
            .expect("governance should match descriptor");

        assert_eq!(
            manifest.schema_version,
            CSM_VOICE_PROFILE_GOVERNANCE_SCHEMA_VERSION
        );
        assert_eq!(
            manifest.watermark_policy.public_demo_key_status,
            "not_a_production_safety_control"
        );
        let profile = manifest
            .find_profile(CSM_OPENAGENTS_DEFAULT_FEMALE_PROFILE_ID)
            .expect("default OpenAgents voice profile");
        assert!(profile.is_runtime_admitted());
        assert_eq!(profile.source_prompt_profile_id, "conversational_b");
        assert_eq!(
            profile.approval_status,
            "approved_openagents_operated_dogfood"
        );
        assert_eq!(
            profile.watermark_policy.status,
            CSM_WATERMARK_OPERATOR_DOGFOOD
        );
        assert!(
            profile
                .allowed_product_surfaces
                .iter()
                .any(|surface| surface == "autopilot_production_dogfood")
        );
        assert!(
            profile
                .disallowed_product_surfaces
                .iter()
                .any(|surface| surface == "arbitrary_reference_audio_upload")
        );
    }

    #[test]
    fn csm_voice_profile_schema_is_machine_readable() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/csm/voice_profiles/lyra_voice_profiles.schema.json"
        ))
        .expect("voice profile schema should parse");
        assert_eq!(
            schema["properties"]["schema_version"]["const"],
            CSM_VOICE_PROFILE_GOVERNANCE_SCHEMA_VERSION
        );
        assert_eq!(
            schema["$defs"]["profile"]["properties"]["runtime_admission"]["enum"][0],
            "admitted_internal_development"
        );
    }

    #[test]
    fn csm_config_parser_validates_frame_contract() {
        let config = CsmModelConfig::from_json_str(CONFIG_JSON).expect("config should parse");

        assert_eq!(config.model_type, "csm");
        assert_eq!(config.max_position_embeddings, CSM_MAX_SEQ_LEN);
        assert_eq!(config.codec_config.sampling_rate, CSM_SAMPLE_RATE_HZ);
    }

    #[test]
    fn csm_config_file_digest_validation_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.json");
        fs::write(&path, CONFIG_JSON).expect("write config");
        let digest = sha256_digest(CONFIG_JSON.as_bytes());

        let (config, actual) =
            CsmModelConfig::from_json_file_with_digest(&path, Some(&digest)).expect("config");
        let mismatch =
            CsmModelConfig::from_json_file_with_digest(&path, Some("sha256:bad")).unwrap_err();

        assert_eq!(config.model_type, "csm");
        assert_eq!(actual, digest);
        assert!(matches!(
            mismatch,
            CsmFrontendError::ArtifactDigestMismatch {
                artifact,
                expected,
                ..
            } if artifact == "csm_config" && expected == "sha256:bad"
        ));
    }

    #[test]
    fn csm_speaker_tag_format_matches_reference() {
        assert_eq!(
            csm_format_segment_text(0, "Hey how are you doing?"),
            "[0]Hey how are you doing?"
        );
        assert_eq!(csm_format_segment_text(1, ""), "[1]");
    }

    #[test]
    fn csm_text_frame_block_matches_fixture_example() {
        let fixture = csm_python_parity_fixture().expect("fixture should parse");
        let example = &fixture.tokenizer_examples[0];

        let block = csm_text_frame_block(&example.encoded_token_ids);

        assert_eq!(block.shape(), example.frame_shape);
        assert_eq!(block.frames[0].tokens, example.first_frame_tokens);
        assert_eq!(block.frames[0].mask, example.first_frame_mask);
    }

    #[test]
    fn csm_empty_text_segment_builds_text_frames() {
        let segment = CsmPromptSegment::text_only(
            1,
            "",
            vec![CSM_LLAMA_BOS_TOKEN_ID, 58, 16, 60, CSM_LLAMA_EOS_TOKEN_ID],
        );

        let block = segment.frame_block().expect("empty text frame block");

        assert_eq!(segment.text, "");
        assert_eq!(block.shape(), [5, CSM_FRAME_LANES]);
        assert_eq!(
            block.frames[0].tokens[CSM_TEXT_LANE_INDEX],
            CSM_LLAMA_BOS_TOKEN_ID
        );
        assert_eq!(
            block.frames[4].tokens[CSM_TEXT_LANE_INDEX],
            CSM_LLAMA_EOS_TOKEN_ID
        );
    }

    #[test]
    fn csm_audio_frame_block_adds_codebook_eos_frame() {
        let frames = vec![[7; CSM_AUDIO_CODEBOOK_LANES], [8; CSM_AUDIO_CODEBOOK_LANES]];

        let block = csm_audio_frame_block(&frames).expect("audio block");

        assert_eq!(block.shape(), [3, CSM_FRAME_LANES]);
        assert_eq!(block.frames[0].tokens[0], 7);
        assert_eq!(
            &block.frames[2].tokens[..CSM_AUDIO_CODEBOOK_LANES],
            &[CSM_CODEBOOK_EOS_TOKEN_ID; CSM_AUDIO_CODEBOOK_LANES]
        );
        assert!(
            block.frames[2].mask[..CSM_AUDIO_CODEBOOK_LANES]
                .iter()
                .all(|value| *value)
        );
        assert!(!block.frames[2].mask[CSM_TEXT_LANE_INDEX]);
    }

    #[test]
    fn csm_generation_case_codebook_frames_have_32_lanes() {
        let fixture = csm_python_parity_fixture().expect("fixture should parse");

        let frames = csm_generation_case_codebook_frames(&fixture.deterministic_generation_case)
            .expect("generation case frames");

        assert_eq!(
            frames.len(),
            fixture
                .deterministic_generation_case
                .generated_prefix_frame_count
        );
        assert_eq!(frames[0][0], 420);
        assert_eq!(frames[0][CSM_AUDIO_CODEBOOK_LANES - 1], 434);
    }

    #[test]
    fn csm_deterministic_fixture_replay_gap_is_explicit() {
        let fixture = csm_python_parity_fixture().expect("fixture should parse");

        let result = csm_deterministic_fixture_replay_ready(&fixture);

        assert!(matches!(
            result,
            Err(CsmFrontendError::FixtureReplayUnavailable { .. })
        ));
    }

    #[test]
    fn csm_runtime_backend_parse_and_labels_are_stable() {
        assert_eq!(
            CsmRuntimeBackend::parse("cpu")
                .expect("cpu")
                .execution_engine(),
            CSM_CPU_EXECUTION_ENGINE
        );
        assert_eq!(
            CsmRuntimeBackend::parse("cuda").expect("cuda").label(),
            "cuda"
        );
        assert_eq!(
            CsmRuntimeBackend::parse("cuda:2")
                .expect("cuda ordinal")
                .residency_mode(),
            "warm_cuda"
        );
        assert!(CsmRuntimeBackend::parse("bogus").is_err());
    }

    #[test]
    fn csm_codebook_frame_digest_is_stable() {
        let fixture = csm_python_parity_fixture().expect("fixture should parse");
        let frames = csm_generation_case_codebook_frames(&fixture.deterministic_generation_case)
            .expect("generation case frames");

        assert_eq!(
            csm_codebook_frames_digest(&frames),
            "sha256:9231cf36b3e869ca4a025ef0db3e5cddae67b5441d99925b6a2b9af3a1bc683e"
        );
    }

    #[test]
    fn csm_wav_pcm16_is_browser_playable() {
        let clip = CsmAudioClip::new(CSM_SAMPLE_RATE_HZ, 1, vec![-1.0, -0.5, 0.0, 0.5, 1.0]);

        let wav = clip.to_wav_pcm16().expect("wav bytes");

        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(clip.frames(), 5);
        assert_eq!(clip.sample_rate_hz, CSM_SAMPLE_RATE_HZ);
        assert_eq!(
            csm_wav_pcm16_digest(&clip).expect("wav digest"),
            sha256_digest(&wav)
        );
    }

    #[test]
    fn csm_reference_audio_encoding_refusal_is_specific() {
        let refusal = csm_reference_audio_encoding_refusal();

        assert_eq!(refusal.code, CSM_REFERENCE_AUDIO_ENCODING_UNSUPPORTED_CODE);
        assert!(refusal.reason.contains("Runtime reference-audio encoding"));
    }

    #[test]
    fn csm_prompt_plan_supports_multi_segment_context() {
        let segment_a = CsmPromptSegment::with_audio_codebooks(
            0,
            "a",
            vec![CSM_LLAMA_BOS_TOKEN_ID, 11, CSM_LLAMA_EOS_TOKEN_ID],
            vec![[1; CSM_AUDIO_CODEBOOK_LANES]; 2],
        );
        let segment_b = CsmPromptSegment::with_audio_codebooks(
            1,
            "b",
            vec![CSM_LLAMA_BOS_TOKEN_ID, 12, CSM_LLAMA_EOS_TOKEN_ID],
            vec![[2; CSM_AUDIO_CODEBOOK_LANES]; 3],
        );
        let target = CsmPromptSegment::text_only(
            0,
            "target",
            vec![CSM_LLAMA_BOS_TOKEN_ID, 13, CSM_LLAMA_EOS_TOKEN_ID],
        );

        let plan = csm_build_prompt_frame_plan(
            &[segment_a, segment_b],
            &target,
            CsmGenerationWindow::new(1_000),
            CsmContextWindowPolicy::Reject,
        )
        .expect("prompt plan");

        assert_eq!(plan.retained_context_segments, 2);
        assert_eq!(plan.dropped_context_segments, 0);
        assert_eq!(plan.frames.len(), 3 + 3 + 3 + 4 + 3);
    }

    #[test]
    fn csm_prompt_overflow_refuses_like_reference() {
        let long_context = CsmPromptSegment::with_audio_codebooks(
            0,
            "long",
            vec![CSM_LLAMA_BOS_TOKEN_ID],
            vec![[1; CSM_AUDIO_CODEBOOK_LANES]; 2042],
        );
        let target = CsmPromptSegment::text_only(
            0,
            "target",
            vec![CSM_LLAMA_BOS_TOKEN_ID, 13, CSM_LLAMA_EOS_TOKEN_ID],
        );

        let result = csm_build_prompt_frame_plan(
            &[long_context],
            &target,
            CsmGenerationWindow::new(160),
            CsmContextWindowPolicy::Reject,
        );

        assert!(matches!(
            result,
            Err(CsmFrontendError::PromptContextOverflow {
                max_context_len: 2046,
                max_generation_len: 2,
                ..
            })
        ));
    }

    #[test]
    fn csm_prompt_policy_drops_oldest_segments_at_boundaries() {
        let too_large = CsmPromptSegment::with_audio_codebooks(
            0,
            "too large",
            vec![CSM_LLAMA_BOS_TOKEN_ID],
            vec![[1; CSM_AUDIO_CODEBOOK_LANES]; 2042],
        );
        let retained = CsmPromptSegment::with_audio_codebooks(
            1,
            "retained",
            vec![CSM_LLAMA_BOS_TOKEN_ID, 12, CSM_LLAMA_EOS_TOKEN_ID],
            vec![[2; CSM_AUDIO_CODEBOOK_LANES]; 2],
        );
        let target = CsmPromptSegment::text_only(
            1,
            "target",
            vec![CSM_LLAMA_BOS_TOKEN_ID, 13, CSM_LLAMA_EOS_TOKEN_ID],
        );

        let plan = csm_build_prompt_frame_plan(
            &[too_large, retained],
            &target,
            CsmGenerationWindow::new(160),
            CsmContextWindowPolicy::DropOldestSegments,
        )
        .expect("prompt should fit after dropping oldest segment");

        assert_eq!(plan.dropped_context_segments, 1);
        assert_eq!(plan.retained_context_segments, 1);
    }

    #[test]
    fn csm_llama_tokenizer_matches_python_fixture_when_cache_is_present() {
        let fixture = csm_python_parity_fixture().expect("fixture should parse");
        let tokenizer = match CsmLlamaTextTokenizer::from_default_hf_cache(Some(
            &fixture.model.llama_tokenizer_digest,
        )) {
            Ok(tokenizer) => tokenizer,
            Err(error) => {
                eprintln!("skipping local tokenizer parity test: {error}");
                return;
            }
        };

        for example in &fixture.tokenizer_examples {
            let actual = tokenizer
                .encode_segment_text(example.speaker, &example.text)
                .expect("encode segment");
            assert_eq!(actual, example.encoded_token_ids);
        }
    }

    #[test]
    fn csm_mimi_decode_generated_prefix_when_weight_cache_is_present() {
        let fixture = csm_python_parity_fixture().expect("fixture should parse");
        let weight =
            match csm_default_mimi_weight_candidates(Some(&fixture.model.mimi_weight_digest))
                .into_iter()
                .next()
            {
                Some(weight) => weight,
                None => {
                    eprintln!(
                        "skipping local Mimi decode parity test: matching weight not present"
                    );
                    return;
                }
            };
        let mut decoder = match CsmMimiDecoder::from_safetensors_file(
            &weight,
            Some(&fixture.model.mimi_weight_digest),
        ) {
            Ok(decoder) => decoder,
            Err(error) => {
                eprintln!("skipping local Mimi decode parity test: {error}");
                return;
            }
        };
        let frames = csm_generation_case_codebook_frames(&fixture.deterministic_generation_case)
            .expect("generation case frames");

        let report = decoder
            .decode_codebook_frames(&frames)
            .expect("Rust Mimi decode");

        assert_eq!(report.decoded_codebook_frames, frames.len());
        assert_eq!(report.clip.sample_rate_hz, CSM_SAMPLE_RATE_HZ);
        assert_eq!(report.clip.channels, 1);
        assert!(!report.clip.samples.is_empty());
        assert!(report.clip.duration_ms() > 0);
        assert_eq!(report.mimi_weight_digest, fixture.model.mimi_weight_digest);
        assert_eq!(report.execution_engine, CSM_MIMI_CPU_EXECUTION_ENGINE);
        assert_eq!(
            report.clip_digest,
            "sha256:30350d2c6648102458e2eedb3c2388894b162452de6fbce931f1058f95d9c509"
        );
        assert_eq!(
            report.wav_pcm16_digest,
            "sha256:8a23a6965b90c0faf627f3eb203c45c8fafc4200c7d8e96231660c4cd931e0cd"
        );
    }

    #[test]
    fn csm_cpu_generates_and_decodes_text_only_when_cache_is_present() {
        let fixture = csm_python_parity_fixture().expect("fixture should parse");
        let tokenizer = match CsmLlamaTextTokenizer::from_default_hf_cache(Some(
            &fixture.model.llama_tokenizer_digest,
        )) {
            Ok(tokenizer) => tokenizer,
            Err(error) => {
                eprintln!("skipping local CSM CPU generation test: tokenizer unavailable: {error}");
                return;
            }
        };
        let config_path =
            match csm_default_config_candidates(Some(&fixture.model.csm_config_digest))
                .into_iter()
                .next()
            {
                Some(path) => path,
                None => {
                    eprintln!("skipping local CSM CPU generation test: config unavailable");
                    return;
                }
            };
        let model_path =
            match csm_default_model_weight_candidates(Some(&fixture.model.csm_model_digest))
                .into_iter()
                .next()
            {
                Some(path) => path,
                None => {
                    eprintln!("skipping local CSM CPU generation test: model weights unavailable");
                    return;
                }
            };
        let mimi_path =
            match csm_default_mimi_weight_candidates(Some(&fixture.model.mimi_weight_digest))
                .into_iter()
                .next()
            {
                Some(path) => path,
                None => {
                    eprintln!("skipping local CSM CPU generation test: Mimi weights unavailable");
                    return;
                }
            };
        let (config, config_digest) = CsmModelConfig::from_json_file_with_digest(
            &config_path,
            Some(&fixture.model.csm_config_digest),
        )
        .expect("CSM config");
        let mut generator = CsmCpuGenerator::from_safetensors_file(
            &config,
            config_digest,
            &model_path,
            Some(&fixture.model.csm_model_digest),
        )
        .expect("Rust CSM model");
        let mut mimi = CsmMimiDecoder::from_safetensors_file(
            &mimi_path,
            Some(&fixture.model.mimi_weight_digest),
        )
        .expect("Rust Mimi model");
        let target = CsmPromptSegment::encode_text_only(&tokenizer, 1, "Hello from Autopilot.")
            .expect("target");
        let plan = csm_build_prompt_frame_plan(
            &[],
            &target,
            CsmGenerationWindow::new(160),
            CsmContextWindowPolicy::Reject,
        )
        .expect("prompt plan");

        let report = generator
            .generate_and_decode(
                &CsmCpuGenerationRequest {
                    prompt: plan,
                    sampling: CsmSamplingStrategy::Greedy,
                },
                &mut mimi,
            )
            .expect("Rust CPU CSM generation and Mimi decode");

        assert_eq!(report.generation.backend, "cpu");
        assert_eq!(report.generation.execution_engine, CSM_CPU_EXECUTION_ENGINE);
        assert!(!report.generation.codebook_frames.is_empty());
        assert!(report.generation.generated_frame_count <= 2);
        assert_eq!(report.decode.clip.sample_rate_hz, CSM_SAMPLE_RATE_HZ);
        assert_eq!(report.decode.clip.channels, 1);
        assert!(!report.decode.clip.samples.is_empty());
        assert!(report.decode.wav_pcm16_digest.starts_with("sha256:"));
    }
}
