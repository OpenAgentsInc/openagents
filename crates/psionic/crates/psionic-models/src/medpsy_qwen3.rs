use std::{fs, fs::File, path::Path};

use candle::{DType, Device, Tensor};
use candle::quantized::gguf_file;
use candle_nn::VarBuilder;
use candle_transformers::models::{
    qwen3::{Config as Qwen3Config, ModelForCausalLM},
    quantized_qwen3::ModelWeights as QuantizedQwen3ModelWeights,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    medpsy_quantization_admission, MedPsyModelSize, MedPsyQuantizationAdmission,
    MedPsyQuantizationTier, TokenId,
};

/// Rust-native MedPsy / Qwen3 safetensors load or generation failure.
#[derive(Debug, Error)]
pub enum MedPsyQwen3Error {
    /// The model artifact could not be read.
    #[error("failed to read MedPsy artifact `{artifact}`: {message}")]
    ArtifactRead {
        /// Artifact path or logical ref.
        artifact: String,
        /// Read failure detail.
        message: String,
    },
    /// The model artifact digest did not match the admitted digest.
    #[error("MedPsy artifact digest mismatch: expected {expected}, got {actual}")]
    ArtifactDigestMismatch {
        /// Expected sha256 digest.
        expected: String,
        /// Actual sha256 digest.
        actual: String,
    },
    /// The requested prompt/generation parameters are outside the bounded lane.
    #[error("invalid MedPsy generation request: {message}")]
    InvalidGenerationRequest {
        /// Refusal detail.
        message: String,
    },
    /// The requested medical-domain quantization tier is blocked by policy.
    #[error("MedPsy quantization tier `{tier}` is blocked by medical-domain policy")]
    QuantizationBlocked {
        /// Published tier label.
        tier: String,
    },
    /// Candle failed during model load.
    #[error("failed to load Rust-native MedPsy Qwen3 model: {message}")]
    ModelLoad {
        /// Lower-level failure detail.
        message: String,
    },
    /// Candle failed during generation.
    #[error("failed to run Rust-native MedPsy Qwen3 generation: {message}")]
    Generation {
        /// Lower-level failure detail.
        message: String,
    },
}

/// Runtime backend admitted by the first MedPsy safetensors lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MedPsyQwen3RuntimeBackend {
    /// CPU reference execution through Candle.
    Cpu,
    /// CUDA execution through Candle on one NVIDIA device.
    #[cfg(feature = "medpsy-cuda")]
    Cuda {
        /// CUDA device ordinal.
        device_ordinal: usize,
    },
}

impl MedPsyQwen3RuntimeBackend {
    fn device(self) -> Result<Device, MedPsyQwen3Error> {
        match self {
            Self::Cpu => Ok(Device::Cpu),
            #[cfg(feature = "medpsy-cuda")]
            Self::Cuda { device_ordinal } => {
                Device::new_cuda(device_ordinal).map_err(|error| MedPsyQwen3Error::ModelLoad {
                    message: format!("failed to initialize CUDA device {device_ordinal}: {error}"),
                })
            }
        }
    }

    /// Stable execution-engine label.
    #[must_use]
    pub const fn execution_engine(self) -> &'static str {
        match self {
            Self::Cpu => "rust_candle_qwen3_cpu",
            #[cfg(feature = "medpsy-cuda")]
            Self::Cuda { .. } => "rust_candle_qwen3_cuda",
        }
    }

    /// Stable backend label.
    #[must_use]
    pub const fn backend_label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            #[cfg(feature = "medpsy-cuda")]
            Self::Cuda { .. } => "cuda",
        }
    }
}

/// Config facts required by the Candle Qwen3 implementation.
#[derive(Clone, Debug, PartialEq)]
pub struct MedPsyQwen3CandleConfig {
    /// Published MedPsy size row.
    pub size: MedPsyModelSize,
    /// Candle Qwen3 config.
    pub qwen3: Qwen3Config,
}

impl MedPsyQwen3CandleConfig {
    /// Returns the published QVAC MedPsy config for one size row.
    #[must_use]
    pub fn from_size(size: MedPsyModelSize) -> Self {
        let qwen3 = match size {
            MedPsyModelSize::OnePointSevenB => Qwen3Config {
                vocab_size: 151_936,
                hidden_size: 2048,
                intermediate_size: 6144,
                num_hidden_layers: 28,
                num_attention_heads: 16,
                head_dim: 128,
                attention_bias: false,
                num_key_value_heads: 8,
                max_position_embeddings: 40_960,
                sliding_window: None,
                max_window_layers: 28,
                tie_word_embeddings: true,
                rope_theta: 1_000_000.0,
                rms_norm_eps: 1e-6,
                use_sliding_window: false,
                hidden_act: candle_nn::Activation::Silu,
            },
            MedPsyModelSize::FourB => Qwen3Config {
                vocab_size: 151_936,
                hidden_size: 2560,
                intermediate_size: 9728,
                num_hidden_layers: 36,
                num_attention_heads: 32,
                head_dim: 128,
                attention_bias: false,
                num_key_value_heads: 8,
                max_position_embeddings: 262_144,
                sliding_window: None,
                max_window_layers: 36,
                tie_word_embeddings: true,
                rope_theta: 5_000_000.0,
                rms_norm_eps: 1e-6,
                use_sliding_window: false,
                hidden_act: candle_nn::Activation::Silu,
            },
        };
        Self { size, qwen3 }
    }
}

/// Output from one bounded greedy MedPsy generation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MedPsyQwen3GenerationReport {
    /// Prompt tokens supplied by the caller.
    pub prompt_tokens: Vec<TokenId>,
    /// Newly generated token ids.
    pub generated_tokens: Vec<TokenId>,
    /// Generation stopped because an EOS token was selected.
    pub stopped_on_eos: bool,
    /// Model artifact digest used by this generator.
    pub model_artifact_sha256: String,
    /// Execution backend label.
    pub backend: MedPsyQwen3RuntimeBackend,
    /// Execution engine label.
    pub execution_engine: String,
}

/// Rust-native MedPsy / Qwen3 safetensors generator.
pub struct MedPsyQwen3CandleGenerator {
    config: MedPsyQwen3CandleConfig,
    backend: MedPsyQwen3RuntimeBackend,
    device: Device,
    model: ModelForCausalLM,
    model_artifact_sha256: String,
}

/// Rust-native MedPsy / Qwen3 GGUF generator backed by Candle quantized Qwen3.
pub struct MedPsyQwen3GgufGenerator {
    backend: MedPsyQwen3RuntimeBackend,
    device: Device,
    model: QuantizedQwen3ModelWeights,
    model_artifact_sha256: String,
    model_family: String,
}

impl MedPsyQwen3CandleGenerator {
    /// Loads a published MedPsy safetensors artifact through Candle Qwen3.
    pub fn from_safetensors_file(
        config: MedPsyQwen3CandleConfig,
        path: impl AsRef<Path>,
        expected_sha256: Option<&str>,
    ) -> Result<Self, MedPsyQwen3Error> {
        Self::from_safetensors_file_with_backend(
            config,
            path,
            expected_sha256,
            MedPsyQwen3RuntimeBackend::Cpu,
        )
    }

    /// Loads a published MedPsy safetensors artifact through the requested Rust backend.
    pub fn from_safetensors_file_with_backend(
        config: MedPsyQwen3CandleConfig,
        path: impl AsRef<Path>,
        expected_sha256: Option<&str>,
        backend: MedPsyQwen3RuntimeBackend,
    ) -> Result<Self, MedPsyQwen3Error> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|error| MedPsyQwen3Error::ArtifactRead {
            artifact: path.display().to_string(),
            message: error.to_string(),
        })?;
        let model_artifact_sha256 = sha256_digest(bytes.as_slice());
        if let Some(expected) = expected_sha256 {
            if model_artifact_sha256 != expected {
                return Err(MedPsyQwen3Error::ArtifactDigestMismatch {
                    expected: expected.to_string(),
                    actual: model_artifact_sha256,
                });
            }
        }
        let device = backend.device()?;
        let weights = [path];
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&weights, DType::BF16, &device) }
            .map_err(|error| MedPsyQwen3Error::ModelLoad {
                message: error.to_string(),
            })?;
        let model = ModelForCausalLM::new(&config.qwen3, vb).map_err(|error| {
            MedPsyQwen3Error::ModelLoad {
                message: error.to_string(),
            }
        })?;
        Ok(Self {
            config,
            backend,
            device,
            model,
            model_artifact_sha256,
        })
    }

    /// Returns the published MedPsy size row.
    #[must_use]
    pub const fn size(&self) -> MedPsyModelSize {
        self.config.size
    }

    /// Returns the stable model artifact digest.
    #[must_use]
    pub fn model_artifact_sha256(&self) -> &str {
        self.model_artifact_sha256.as_str()
    }

    /// Greedy argmax generation over already-tokenized Qwen3 prompt tokens.
    pub fn generate_greedy_token_ids(
        &mut self,
        prompt_tokens: &[TokenId],
        max_new_tokens: usize,
        eos_token_ids: &[TokenId],
    ) -> Result<MedPsyQwen3GenerationReport, MedPsyQwen3Error> {
        if prompt_tokens.is_empty() {
            return Err(MedPsyQwen3Error::InvalidGenerationRequest {
                message: String::from("prompt token list must not be empty"),
            });
        }
        if max_new_tokens == 0 {
            return Err(MedPsyQwen3Error::InvalidGenerationRequest {
                message: String::from("max_new_tokens must be greater than zero"),
            });
        }
        self.model.clear_kv_cache();
        let mut generated = Vec::new();
        let mut offset = 0usize;
        let mut input_tokens = prompt_tokens.to_vec();
        for step in 0..max_new_tokens {
            let input = tensor_from_tokens(input_tokens.as_slice(), &self.device)?;
            let logits = self.model.forward(&input, offset).map_err(|error| {
                MedPsyQwen3Error::Generation {
                    message: error.to_string(),
                }
            })?;
            let next = argmax_token(&logits)?;
            generated.push(next);
            if eos_token_ids.contains(&next) {
                return Ok(self.report(prompt_tokens, generated, true));
            }
            offset = if step == 0 {
                prompt_tokens.len()
            } else {
                offset.saturating_add(1)
            };
            input_tokens.clear();
            input_tokens.push(next);
        }
        Ok(self.report(prompt_tokens, generated, false))
    }

    fn report(
        &self,
        prompt_tokens: &[TokenId],
        generated_tokens: Vec<TokenId>,
        stopped_on_eos: bool,
    ) -> MedPsyQwen3GenerationReport {
        MedPsyQwen3GenerationReport {
            prompt_tokens: prompt_tokens.to_vec(),
            generated_tokens,
            stopped_on_eos,
            model_artifact_sha256: self.model_artifact_sha256.clone(),
            backend: self.backend,
            execution_engine: self.backend.execution_engine().to_string(),
        }
    }
}

impl MedPsyQwen3GgufGenerator {
    /// Loads an admitted MedPsy Qwen3 GGUF artifact after applying medical quantization policy.
    pub fn from_admitted_gguf_file(
        size: MedPsyModelSize,
        tier: MedPsyQuantizationTier,
        path: impl AsRef<Path>,
        expected_sha256: Option<&str>,
        allow_blocked_medical_quantization: bool,
    ) -> Result<Self, MedPsyQwen3Error> {
        if medpsy_quantization_admission(size, tier) == MedPsyQuantizationAdmission::BlockedByDefault
            && !allow_blocked_medical_quantization
        {
            return Err(MedPsyQwen3Error::QuantizationBlocked {
                tier: tier.as_str().to_string(),
            });
        }
        Self::from_gguf_file(path, expected_sha256)
    }

    /// Loads a MedPsy Qwen3 GGUF artifact through Candle's Rust quantized Qwen3 path.
    pub fn from_gguf_file(
        path: impl AsRef<Path>,
        expected_sha256: Option<&str>,
    ) -> Result<Self, MedPsyQwen3Error> {
        Self::from_gguf_file_with_backend(path, expected_sha256, MedPsyQwen3RuntimeBackend::Cpu)
    }

    /// Loads a MedPsy Qwen3 GGUF artifact through the requested Rust backend.
    pub fn from_gguf_file_with_backend(
        path: impl AsRef<Path>,
        expected_sha256: Option<&str>,
        backend: MedPsyQwen3RuntimeBackend,
    ) -> Result<Self, MedPsyQwen3Error> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|error| MedPsyQwen3Error::ArtifactRead {
            artifact: path.display().to_string(),
            message: error.to_string(),
        })?;
        let model_artifact_sha256 = sha256_digest(bytes.as_slice());
        if let Some(expected) = expected_sha256 {
            if model_artifact_sha256 != expected {
                return Err(MedPsyQwen3Error::ArtifactDigestMismatch {
                    expected: expected.to_string(),
                    actual: model_artifact_sha256,
                });
            }
        }
        let device = backend.device()?;
        let mut file = File::open(path).map_err(|error| MedPsyQwen3Error::ArtifactRead {
            artifact: path.display().to_string(),
            message: error.to_string(),
        })?;
        let content = gguf_file::Content::read(&mut file).map_err(|error| {
            MedPsyQwen3Error::ModelLoad {
                message: error.to_string(),
            }
        })?;
        let architecture = content
            .metadata
            .get("general.architecture")
            .and_then(|value| value.to_string().ok().cloned())
            .unwrap_or_default();
        if architecture != "qwen3" {
            return Err(MedPsyQwen3Error::ModelLoad {
                message: format!(
                    "MedPsy GGUF expects general.architecture=qwen3, got `{architecture}`"
                ),
            });
        }
        let model = QuantizedQwen3ModelWeights::from_gguf(content, &mut file, &device).map_err(
            |error| MedPsyQwen3Error::ModelLoad {
                message: error.to_string(),
            },
        )?;
        Ok(Self {
            backend,
            device,
            model,
            model_artifact_sha256,
            model_family: String::from("medpsy_qwen3"),
        })
    }

    /// Returns the stable model artifact digest.
    #[must_use]
    pub fn model_artifact_sha256(&self) -> &str {
        self.model_artifact_sha256.as_str()
    }

    /// Returns the stable family label.
    #[must_use]
    pub fn model_family(&self) -> &str {
        self.model_family.as_str()
    }

    /// Greedy argmax generation over already-tokenized Qwen3 prompt tokens.
    pub fn generate_greedy_token_ids(
        &mut self,
        prompt_tokens: &[TokenId],
        max_new_tokens: usize,
        eos_token_ids: &[TokenId],
    ) -> Result<MedPsyQwen3GenerationReport, MedPsyQwen3Error> {
        if prompt_tokens.is_empty() {
            return Err(MedPsyQwen3Error::InvalidGenerationRequest {
                message: String::from("prompt token list must not be empty"),
            });
        }
        if max_new_tokens == 0 {
            return Err(MedPsyQwen3Error::InvalidGenerationRequest {
                message: String::from("max_new_tokens must be greater than zero"),
            });
        }
        self.model.clear_kv_cache();
        let mut generated = Vec::new();
        let mut offset = 0usize;
        let mut input_tokens = prompt_tokens.to_vec();
        for step in 0..max_new_tokens {
            let input = tensor_from_tokens(input_tokens.as_slice(), &self.device)?;
            let logits = self.model.forward(&input, offset).map_err(|error| {
                MedPsyQwen3Error::Generation {
                    message: error.to_string(),
                }
            })?;
            let next = argmax_token(&logits)?;
            generated.push(next);
            if eos_token_ids.contains(&next) {
                return Ok(self.report(prompt_tokens, generated, true));
            }
            offset = if step == 0 {
                prompt_tokens.len()
            } else {
                offset.saturating_add(1)
            };
            input_tokens.clear();
            input_tokens.push(next);
        }
        Ok(self.report(prompt_tokens, generated, false))
    }

    fn report(
        &self,
        prompt_tokens: &[TokenId],
        generated_tokens: Vec<TokenId>,
        stopped_on_eos: bool,
    ) -> MedPsyQwen3GenerationReport {
        MedPsyQwen3GenerationReport {
            prompt_tokens: prompt_tokens.to_vec(),
            generated_tokens,
            stopped_on_eos,
            model_artifact_sha256: self.model_artifact_sha256.clone(),
            backend: self.backend,
            execution_engine: self.backend.execution_engine().to_string(),
        }
    }
}

fn tensor_from_tokens(tokens: &[TokenId], device: &Device) -> Result<Tensor, MedPsyQwen3Error> {
    let values = tokens
        .iter()
        .map(|token| token.as_u32())
        .collect::<Vec<_>>();
    Tensor::from_vec(values, (1, tokens.len()), device).map_err(|error| {
        MedPsyQwen3Error::Generation {
            message: error.to_string(),
        }
    })
}

fn argmax_token(logits: &Tensor) -> Result<TokenId, MedPsyQwen3Error> {
    let values = logits
        .argmax(candle::D::Minus1)
        .and_then(|tensor| tensor.flatten_all())
        .and_then(|tensor| tensor.to_vec1::<u32>())
        .map_err(|error| MedPsyQwen3Error::Generation {
            message: error.to_string(),
        })?;
    let Some(token_id) = values.last().copied() else {
        return Err(MedPsyQwen3Error::Generation {
            message: String::from("model returned empty logits"),
        });
    };
    Ok(TokenId(token_id))
}

fn sha256_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medpsy_qwen3_candle_configs_match_published_shapes() {
        let one_point_seven = MedPsyQwen3CandleConfig::from_size(MedPsyModelSize::OnePointSevenB);
        assert_eq!(one_point_seven.qwen3.hidden_size, 2048);
        assert_eq!(one_point_seven.qwen3.num_hidden_layers, 28);
        assert_eq!(one_point_seven.qwen3.num_key_value_heads, 8);
        assert_eq!(one_point_seven.qwen3.max_position_embeddings, 40_960);
        assert_eq!(one_point_seven.qwen3.rope_theta, 1_000_000.0);

        let four = MedPsyQwen3CandleConfig::from_size(MedPsyModelSize::FourB);
        assert_eq!(four.qwen3.hidden_size, 2560);
        assert_eq!(four.qwen3.num_hidden_layers, 36);
        assert_eq!(four.qwen3.num_attention_heads, 32);
        assert_eq!(four.qwen3.max_position_embeddings, 262_144);
        assert_eq!(four.qwen3.rope_theta, 5_000_000.0);
    }

    #[test]
    fn medpsy_qwen3_missing_safetensors_fails_closed() {
        let config = MedPsyQwen3CandleConfig::from_size(MedPsyModelSize::OnePointSevenB);
        let result = MedPsyQwen3CandleGenerator::from_safetensors_file(
            config,
            "/definitely/missing/medpsy-model.safetensors",
            None,
        );
        assert!(matches!(result, Err(MedPsyQwen3Error::ArtifactRead { .. })));
    }

    #[test]
    fn medpsy_qwen3_missing_gguf_fails_closed() {
        let result = MedPsyQwen3GgufGenerator::from_gguf_file(
            "/definitely/missing/medpsy-model.gguf",
            None,
        );
        assert!(matches!(result, Err(MedPsyQwen3Error::ArtifactRead { .. })));
    }

    #[test]
    fn medpsy_qwen3_blocks_disallowed_medical_quantization_by_default() {
        let result = MedPsyQwen3GgufGenerator::from_admitted_gguf_file(
            MedPsyModelSize::OnePointSevenB,
            MedPsyQuantizationTier::IQ3M,
            "/definitely/missing/medpsy-model.gguf",
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(MedPsyQwen3Error::QuantizationBlocked { .. })
        ));
    }

    #[test]
    fn medpsy_qwen3_real_17b_safetensors_smoke_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let Ok(path) = std::env::var("PSIONIC_MEDPSY_17B_SAFETENSORS_PATH") else {
            return Ok(());
        };
        if path.trim().is_empty() || !Path::new(path.as_str()).exists() {
            return Ok(());
        }
        let config = MedPsyQwen3CandleConfig::from_size(MedPsyModelSize::OnePointSevenB);
        let mut generator = MedPsyQwen3CandleGenerator::from_safetensors_file(config, path, None)?;
        let report =
            generator.generate_greedy_token_ids(&[TokenId(151644)], 1, &[TokenId(151645)])?;
        assert_eq!(report.prompt_tokens, vec![TokenId(151644)]);
        assert_eq!(report.generated_tokens.len(), 1);
        assert_eq!(report.execution_engine, "rust_candle_qwen3_cpu");
        Ok(())
    }

    #[test]
    fn medpsy_qwen3_real_17b_q4_gguf_smoke_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let Ok(path) = std::env::var("PSIONIC_MEDPSY_17B_Q4_K_M_GGUF_PATH") else {
            return Ok(());
        };
        if path.trim().is_empty() || !Path::new(path.as_str()).exists() {
            return Ok(());
        }
        let mut generator = MedPsyQwen3GgufGenerator::from_gguf_file(path, None)?;
        let report =
            generator.generate_greedy_token_ids(&[TokenId(151644)], 1, &[TokenId(151645)])?;
        assert_eq!(report.prompt_tokens, vec![TokenId(151644)]);
        assert_eq!(report.generated_tokens.len(), 1);
        assert_eq!(report.execution_engine, "rust_candle_qwen3_cpu");
        assert_eq!(generator.model_family(), "medpsy_qwen3");
        Ok(())
    }
}
