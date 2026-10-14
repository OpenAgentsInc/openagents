//! Checked SmolVLA checkpoint inputs; files never execute import code.
use crate::{
    Result,
    artifacts::{Binding, Normalization, Source, digest, file_name, read},
    require,
    smolvla::{Config, SmolVla},
};
use robot_data::dataset::FilePin;
use robot_lab::document::Build;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub schema: String,
    pub source: Source,
    pub architecture: Config,
    pub binding: Binding,
    pub weights: String,
    pub normalization: String,
    pub tokenizer: String,
    pub language_tokens: Vec<u32>,
    pub language_valid: Vec<bool>,
    pub empty_cameras: usize,
    pub files: BTreeMap<String, FilePin>,
}
impl Pin {
    pub fn validate(&self) -> Result<()> {
        self.source.validate()?;
        self.architecture.validate()?;
        let state = self.binding.state.joints.len();
        let actions = self.binding.actions.joints.len();
        self.binding.validate(
            state,
            actions,
            0,
            self.binding.cameras.len(),
            self.architecture.chunk_size,
        )?;
        require(
            self.schema == "robot-smolvla.v1"
                && (1..=self.architecture.max_state_dim).contains(&state)
                && (1..=self.architecture.max_action_dim).contains(&actions)
                && !self.binding.cameras.is_empty()
                && self.empty_cameras <= 4
                && self.empty_cameras + self.binding.cameras.len() <= 4
                && self
                    .binding
                    .cameras
                    .iter()
                    .all(|c| c.target == [self.architecture.vision.image_size as u32; 2])
                && self.binding.environment.is_empty(),
            "Map SmolVLA to its exact robot state, actions, and square policy cameras",
        )?;
        require(
            !self.language_tokens.is_empty()
                && self.language_tokens.len() <= 128
                && self.language_tokens.len() == self.language_valid.len()
                && self.language_valid.iter().any(|v| *v)
                && self
                    .language_tokens
                    .iter()
                    .all(|t| (*t as usize) < self.architecture.vocabulary)
                && self.files.len() <= 16
                && [&self.weights, &self.normalization, &self.tokenizer]
                    .iter()
                    .all(|n| self.files.contains_key(*n)),
            "Pin SmolVLA's prompt, tokenizer, normalization, and weights",
        )?;
        let mut total = 0_u64;
        for (name, pin) in &self.files {
            file_name(name)?;
            total = total.saturating_add(pin.bytes);
            require(
                pin.bytes > 0
                    && pin.bytes <= 2 << 30
                    && total <= 3 << 30
                    && pin.sha256.len() == 64
                    && pin.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "Use bounded SmolVLA files with saved hashes",
            )?;
        }
        Ok(())
    }
}
pub struct Artifact {
    pub pin: Pin,
    pub normalization: Normalization,
    pub build: Build,
    weights: Vec<u8>,
}
impl Artifact {
    pub fn open(root: &Path, expected: &str) -> Result<Self> {
        require(
            root.is_absolute(),
            "Choose an absolute folder for private policy weights",
        )?;
        let root = root.canonicalize()?;
        require(
            !root.ancestors().any(|p| p.join(".git").exists()),
            "Keep policy weights outside the Git checkout",
        )?;
        let bytes = read(&root, "policy.json", 2 << 20)?;
        require(
            digest(&bytes) == expected,
            "This policy changed. Review its new checkpoint before running it",
        )?;
        let pin: Pin = serde_json::from_slice(&bytes)?;
        pin.validate()?;
        let mut weights = None;
        let mut normalization = None;
        let mut tokenizer = None;
        for (name, file) in &pin.files {
            let data = read(&root, name, file.bytes as usize)?;
            require(
                data.len() as u64 == file.bytes && digest(&data) == file.sha256,
                "A policy file changed. Restore the saved checkpoint",
            )?;
            if name == &pin.weights {
                weights = Some(data);
            } else if name == &pin.normalization {
                require(data.len() <= 1 << 20, "Use bounded policy normalization")?;
                normalization = Some(serde_json::from_slice::<Normalization>(&data)?);
            } else if name == &pin.tokenizer {
                require(data.len() <= 32 << 20, "Use a bounded policy tokenizer")?;
                tokenizer = Some(data);
            }
        }
        let normalization = normalization
            .ok_or_else(|| crate::Error::Invalid("Save the policy normalization".into()))?;
        normalization.validate_binding(
            &pin.binding,
            pin.binding.state.joints.len(),
            pin.binding.actions.joints.len(),
            0,
        )?;
        require(
            normalization
                .cameras
                .values()
                .all(|n| matches!(n, crate::preprocess::Normalize::Identity)),
            "Use SmolVLA's identity camera normalization before SigLIP preprocessing",
        )?;
        let tokenizer = tokenizers::Tokenizer::from_bytes(
            tokenizer.ok_or_else(|| crate::Error::Invalid("Save the policy tokenizer".into()))?,
        )
        .map_err(|_| {
            crate::Error::Invalid("The saved policy tokenizer couldn't be opened".into())
        })?;
        let encoded = tokenizer
            .encode(format!("{}\n", pin.binding.instruction), true)
            .map_err(|_| {
                crate::Error::Invalid("The fixed policy instruction couldn't be read".into())
            })?;
        require(
            encoded.get_ids() == pin.language_tokens
                && encoded
                    .get_attention_mask()
                    .iter()
                    .map(|v| *v != 0)
                    .collect::<Vec<_>>()
                    == pin.language_valid,
            "Use the saved tokenizer and exact tokenized policy instruction",
        )?;
        let build = Build {
            name: "robot-smolvla-candle-v1".into(),
            sha256: digest(&serde_json::to_vec(&(
                expected,
                digest(include_bytes!("smolvla.rs")),
                digest(include_bytes!("smol_artifacts.rs")),
                digest(include_bytes!("preprocess.rs")),
                digest(include_bytes!("artifacts.rs")),
                digest(include_bytes!("controller.rs")),
                "candle-0.11-f32-cpu/tokenizers-0.22.2/box-muller-splitmix64-v1",
            ))?),
        };
        Ok(Self {
            pin,
            normalization,
            build,
            weights: weights
                .ok_or_else(|| crate::Error::Invalid("Save the policy weights".into()))?,
        })
    }
    pub fn model(self) -> Result<(SmolVla, Pin, Normalization, Build)> {
        let vb = candle_nn::VarBuilder::from_buffered_safetensors(
            self.weights,
            candle_core::DType::F32,
            &candle_core::Device::Cpu,
        )?;
        let model = SmolVla::load(self.pin.architecture.clone(), vb.pp("model"))?;
        Ok((model, self.pin, self.normalization, self.build))
    }
}
/// Stateless noise binds a draw to its experiment seed and observation step;
/// it does not claim to reproduce PyTorch's random-number stream.
pub fn noise(seed: u64, step: u32, count: usize) -> Vec<f32> {
    let mut state = seed ^ (u64::from(step) << 32) ^ 0x6a09e667f3bcc909;
    let mut uniform = || {
        state = state.wrapping_add(0x9e3779b97f4a7c15);
        let mut x = state;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        (((x ^ (x >> 31)) >> 11) as f64 + 0.5) / (1_u64 << 53) as f64
    };
    let mut output = Vec::with_capacity(count);
    while output.len() < count {
        let radius = (-2. * uniform().ln()).sqrt();
        let angle = std::f64::consts::TAU * uniform();
        output.push((radius * angle.cos()) as f32);
        if output.len() < count {
            output.push((radius * angle.sin()) as f32);
        }
    }
    output
}
