use std::{collections::BTreeMap, fs, path::Path};

use psionic_core::{Shape, TensorData};
use psionic_data::{
    train_cs336_a1_byte_pair_encoding_from_path, train_cs336_a1_byte_pair_encoding_from_text,
    Cs336A1BytePairEncodingArtifacts,
};
use psionic_models::{
    Cs336A1BytePairTokenizer, Cs336A1ReferenceConfig, Cs336A1TransformerLm, TokenizerBoundary,
};
use psionic_nn::{
    cross_entropy_loss, LayerError, LossReduction, ModuleStateDict, ModuleStateEntry,
    ModuleStateEntryKind, ModuleStateLoadMode, ModuleStateView, NnTensor, NnTrainingError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{TrainingOptimizerConfig, TrainingOptimizerError, TrainingOptimizerState};

pub const CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a1_reference_tiny_corpus.txt";
pub const CS336_A1_REFERENCE_TINY_CHECKPOINT_STEP2_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a1_reference_tiny_checkpoint_step2.json";
pub const CS336_A1_REFERENCE_TINY_CHECKPOINT_STEP4_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a1_reference_tiny_checkpoint_step4.json";
pub const CS336_A1_REFERENCE_TINY_TRAINING_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a1_reference_tiny_training_bundle_v1.json";
pub const CS336_A1_REFERENCE_TRAINING_BUNDLE_SCHEMA_VERSION: &str =
    "psion.cs336_a1.reference_training_bundle.v1";
pub const CS336_A1_REFERENCE_CHECKPOINT_SCHEMA_VERSION: &str =
    "psion.cs336_a1.reference_checkpoint.v1";

#[derive(Debug, Error)]
pub enum Cs336A1ReferenceTrainingError {
    #[error("invalid CS336 A1 reference training configuration: {0}")]
    InvalidConfig(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Bpe(#[from] psionic_data::Cs336A1BytePairEncodingError),
    #[error(transparent)]
    Tokenizer(#[from] psionic_models::Cs336A1BytePairTokenizerError),
    #[error(transparent)]
    Model(#[from] psionic_models::Cs336A1ReferenceError),
    #[error(transparent)]
    Nn(#[from] NnTrainingError),
    #[error(transparent)]
    Layer(#[from] LayerError),
    #[error(transparent)]
    ModuleState(#[from] psionic_nn::ModuleStateError),
    #[error(transparent)]
    ModuleStateLoad(#[from] psionic_nn::ModuleStateLoadError),
    #[error(transparent)]
    Optimizer(#[from] TrainingOptimizerError),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A1ReferenceTrainingConfig {
    pub requested_vocab_size: u32,
    pub batch_size: usize,
    pub context_length: usize,
    pub d_model: usize,
    pub num_layers: usize,
    pub num_heads: usize,
    pub d_ff: usize,
    pub rope_theta: f32,
    pub rms_norm_eps: f32,
    pub max_learning_rate: f32,
    pub min_learning_rate: f32,
    pub warmup_iters: u64,
    pub cosine_cycle_iters: u64,
    pub gradient_clip_norm: f32,
    pub finite_difference_epsilon: f32,
    pub adam_beta1: f32,
    pub adam_beta2: f32,
    pub adam_epsilon: f32,
    pub weight_decay: f32,
}

impl Cs336A1ReferenceTrainingConfig {
    #[must_use]
    pub fn tiny() -> Self {
        Self {
            requested_vocab_size: 256,
            batch_size: 1,
            context_length: 2,
            d_model: 2,
            num_layers: 1,
            num_heads: 1,
            d_ff: 4,
            rope_theta: 10_000.0,
            rms_norm_eps: 1e-5,
            max_learning_rate: 0.15,
            min_learning_rate: 0.02,
            warmup_iters: 1,
            cosine_cycle_iters: 4,
            gradient_clip_norm: 1.0,
            finite_difference_epsilon: 5e-3,
            adam_beta1: 0.9,
            adam_beta2: 0.95,
            adam_epsilon: 1e-8,
            weight_decay: 0.0,
        }
    }

    pub fn validate(&self) -> Result<(), Cs336A1ReferenceTrainingError> {
        if self.requested_vocab_size < 256 {
            return Err(Cs336A1ReferenceTrainingError::InvalidConfig(
                "requested_vocab_size must be at least 256 for byte-level BPE".into(),
            ));
        }
        if self.batch_size == 0 {
            return Err(Cs336A1ReferenceTrainingError::InvalidConfig(
                "batch_size must be positive".into(),
            ));
        }
        if self.context_length == 0 {
            return Err(Cs336A1ReferenceTrainingError::InvalidConfig(
                "context_length must be positive".into(),
            ));
        }
        if self.gradient_clip_norm <= 0.0 {
            return Err(Cs336A1ReferenceTrainingError::InvalidConfig(
                "gradient_clip_norm must be positive".into(),
            ));
        }
        if self.finite_difference_epsilon <= 0.0 {
            return Err(Cs336A1ReferenceTrainingError::InvalidConfig(
                "finite_difference_epsilon must be positive".into(),
            ));
        }
        if self.max_learning_rate <= 0.0 || self.min_learning_rate <= 0.0 {
            return Err(Cs336A1ReferenceTrainingError::InvalidConfig(
                "learning rates must be positive".into(),
            ));
        }
        if self.max_learning_rate < self.min_learning_rate {
            return Err(Cs336A1ReferenceTrainingError::InvalidConfig(
                "max_learning_rate must be at least min_learning_rate".into(),
            ));
        }
        if self.cosine_cycle_iters == 0 {
            return Err(Cs336A1ReferenceTrainingError::InvalidConfig(
                "cosine_cycle_iters must be positive".into(),
            ));
        }
        if self.warmup_iters > self.cosine_cycle_iters {
            return Err(Cs336A1ReferenceTrainingError::InvalidConfig(
                "warmup_iters must not exceed cosine_cycle_iters".into(),
            ));
        }
        self.model_config().validate()?;
        Ok(())
    }

    #[must_use]
    pub fn model_config(&self) -> Cs336A1ReferenceConfig {
        Cs336A1ReferenceConfig {
            vocab_size: self.requested_vocab_size as usize,
            context_length: self.context_length,
            d_model: self.d_model,
            num_layers: self.num_layers,
            num_heads: self.num_heads,
            d_ff: self.d_ff,
        }
    }

    fn optimizer_config(&self, learning_rate: f32) -> TrainingOptimizerConfig {
        TrainingOptimizerConfig::adamw(
            learning_rate,
            self.adam_beta1,
            self.adam_beta2,
            self.adam_epsilon,
        )
        .with_weight_decay(self.weight_decay)
    }
}

impl Default for Cs336A1ReferenceTrainingConfig {
    fn default() -> Self {
        Self::tiny()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A1ReferenceBatch {
    pub iteration: u64,
    pub batch_size: usize,
    pub context_length: usize,
    pub start_positions: Vec<usize>,
    pub inputs: Vec<u32>,
    pub targets: Vec<u32>,
}

impl Cs336A1ReferenceBatch {
    #[must_use]
    pub fn token_shape(&self) -> Shape {
        Shape::new(vec![self.batch_size, self.context_length])
    }

    #[must_use]
    pub fn input_ids(&self) -> Vec<usize> {
        self.inputs.iter().map(|token| *token as usize).collect()
    }

    #[must_use]
    pub fn target_ids(&self) -> Vec<usize> {
        self.targets.iter().map(|token| *token as usize).collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A1GradientClipReport {
    pub gradient_norm_l2_before: f32,
    pub gradient_norm_l2_after: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clipping_ratio: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A1ReferenceTrainingStepReport {
    pub step_number: u64,
    pub learning_rate: f32,
    pub loss_before: f32,
    pub loss_after: f32,
    pub gradient_clip: Cs336A1GradientClipReport,
    pub model_state_digest_before: String,
    pub model_state_digest_after: String,
    pub optimizer_state_digest_after: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A1ReferenceCheckpoint {
    pub schema_version: String,
    pub config: Cs336A1ReferenceTrainingConfig,
    pub tokenizer_artifacts: Cs336A1BytePairEncodingArtifacts,
    pub tokenized_dataset: Vec<u32>,
    pub iteration: u64,
    pub loss_history: Vec<f32>,
    pub model_state: ModuleStateDict,
    pub optimizer_states: BTreeMap<String, TrainingOptimizerState>,
}

impl Cs336A1ReferenceCheckpoint {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_json_digest(b"psion.cs336_a1.reference_checkpoint", self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A1ReferenceCheckpointReceipt {
    pub checkpoint_path: String,
    pub checkpoint_digest: String,
    pub iteration: u64,
    pub model_state_digest: String,
    pub optimizer_state_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A1ReferenceTrainingBundle {
    pub schema_version: String,
    pub config: Cs336A1ReferenceTrainingConfig,
    pub corpus_path: String,
    pub corpus_digest: String,
    pub tokenizer_digest: String,
    pub dataset_digest: String,
    pub initial_loss: f32,
    pub pre_checkpoint_steps: Vec<Cs336A1ReferenceTrainingStepReport>,
    pub resumed_steps: Vec<Cs336A1ReferenceTrainingStepReport>,
    pub uninterrupted_steps: Vec<Cs336A1ReferenceTrainingStepReport>,
    pub checkpoint_step2: Cs336A1ReferenceCheckpointReceipt,
    pub checkpoint_step4: Cs336A1ReferenceCheckpointReceipt,
    pub resumed_final_model_state_digest: String,
    pub uninterrupted_final_model_state_digest: String,
    pub resumed_final_optimizer_state_digest: String,
    pub uninterrupted_final_optimizer_state_digest: String,
    pub resume_matches_uninterrupted: bool,
    pub schedule_preview: Vec<f32>,
    pub claim_boundary: String,
}

#[derive(Clone, Debug)]
pub struct Cs336A1ReferenceTrainer {
    config: Cs336A1ReferenceTrainingConfig,
    tokenizer_artifacts: Cs336A1BytePairEncodingArtifacts,
    tokenized_dataset: Vec<u32>,
    model: Cs336A1TransformerLm,
    optimizer_states: BTreeMap<String, TrainingOptimizerState>,
    iteration: u64,
    loss_history: Vec<f32>,
}

impl Cs336A1ReferenceTrainer {
    pub fn from_corpus_path(
        corpus_path: impl AsRef<Path>,
        config: Cs336A1ReferenceTrainingConfig,
    ) -> Result<Self, Cs336A1ReferenceTrainingError> {
        let tokenizer_artifacts = train_cs336_a1_byte_pair_encoding_from_path(
            corpus_path.as_ref(),
            config.requested_vocab_size,
            &[],
        )?;
        let corpus = fs::read_to_string(corpus_path)?;
        Self::from_artifacts_and_corpus_text(config, tokenizer_artifacts, corpus.as_str())
    }

    pub fn from_corpus_text(
        corpus: &str,
        config: Cs336A1ReferenceTrainingConfig,
    ) -> Result<Self, Cs336A1ReferenceTrainingError> {
        let tokenizer_artifacts =
            train_cs336_a1_byte_pair_encoding_from_text(corpus, config.requested_vocab_size, &[])?;
        Self::from_artifacts_and_corpus_text(config, tokenizer_artifacts, corpus)
    }

    pub fn load_checkpoint(
        checkpoint_path: impl AsRef<Path>,
    ) -> Result<Self, Cs336A1ReferenceTrainingError> {
        let checkpoint = load_cs336_a1_reference_checkpoint(checkpoint_path)?;
        Self::from_checkpoint(checkpoint)
    }

    pub fn from_checkpoint(
        checkpoint: Cs336A1ReferenceCheckpoint,
    ) -> Result<Self, Cs336A1ReferenceTrainingError> {
        checkpoint.config.validate()?;
        if checkpoint.schema_version != CS336_A1_REFERENCE_CHECKPOINT_SCHEMA_VERSION {
            return Err(Cs336A1ReferenceTrainingError::InvalidConfig(format!(
                "unsupported checkpoint schema version `{}`",
                checkpoint.schema_version
            )));
        }
        validate_tokenized_dataset(
            checkpoint.tokenized_dataset.as_slice(),
            checkpoint.config.context_length,
        )?;
        let mut model = Cs336A1TransformerLm::new(
            "cs336_a1_reference_lm",
            checkpoint.config.model_config(),
            checkpoint.config.rope_theta,
            checkpoint.config.rms_norm_eps,
        )?;
        model.load_state_dict(&checkpoint.model_state, ModuleStateLoadMode::Strict)?;
        Ok(Self {
            config: checkpoint.config,
            tokenizer_artifacts: checkpoint.tokenizer_artifacts,
            tokenized_dataset: checkpoint.tokenized_dataset,
            model,
            optimizer_states: checkpoint.optimizer_states,
            iteration: checkpoint.iteration,
            loss_history: checkpoint.loss_history,
        })
    }

    #[must_use]
    pub fn config(&self) -> &Cs336A1ReferenceTrainingConfig {
        &self.config
    }

    #[must_use]
    pub fn tokenized_dataset(&self) -> &[u32] {
        self.tokenized_dataset.as_slice()
    }

    #[must_use]
    pub fn model_state(&self) -> ModuleStateDict {
        self.model.state_dict()
    }

    pub fn load_model_state(
        &mut self,
        state: &ModuleStateDict,
    ) -> Result<(), Cs336A1ReferenceTrainingError> {
        self.model
            .load_state_dict(state, ModuleStateLoadMode::Strict)?;
        Ok(())
    }

    #[must_use]
    pub fn optimizer_states(&self) -> &BTreeMap<String, TrainingOptimizerState> {
        &self.optimizer_states
    }

    pub fn batch_for_iteration(
        &self,
        iteration: u64,
    ) -> Result<Cs336A1ReferenceBatch, Cs336A1ReferenceTrainingError> {
        cs336_a1_get_batch(
            self.tokenized_dataset.as_slice(),
            self.config.batch_size,
            self.config.context_length,
            iteration,
        )
    }

    pub fn current_loss(&self) -> Result<f32, Cs336A1ReferenceTrainingError> {
        let batch = self.batch_for_iteration(self.iteration)?;
        self.loss_for_explicit_batch(&batch)
    }

    pub fn loss_for_explicit_batch(
        &self,
        batch: &Cs336A1ReferenceBatch,
    ) -> Result<f32, Cs336A1ReferenceTrainingError> {
        self.loss_for_batch(&self.model, batch)
    }

    #[must_use]
    pub fn trainable_parameter_paths(&self) -> Vec<String> {
        self.model
            .state_dict()
            .entries
            .iter()
            .filter(|(_, entry)| {
                entry.kind == ModuleStateEntryKind::Parameter && entry.requires_grad
            })
            .map(|(path, _)| path.clone())
            .collect()
    }

    pub fn estimate_parameter_gradient(
        &self,
        batch: &Cs336A1ReferenceBatch,
        base_loss: f32,
        path: &str,
    ) -> Result<ModuleStateEntry, Cs336A1ReferenceTrainingError> {
        let base_state = self.model.state_dict();
        let parameter_entry = base_state.entries.get(path).ok_or_else(|| {
            Cs336A1ReferenceTrainingError::InvalidConfig(format!(
                "missing parameter entry `{path}` during finite-difference gradient estimation"
            ))
        })?;
        if parameter_entry.kind != ModuleStateEntryKind::Parameter || !parameter_entry.requires_grad
        {
            return Err(Cs336A1ReferenceTrainingError::InvalidConfig(format!(
                "parameter entry `{path}` is not a trainable parameter"
            )));
        }
        let parameter_values = dense_tensor_values(&parameter_entry.data)?;
        let mut gradient_values = Vec::with_capacity(parameter_values.len());
        for index in 0..parameter_values.len() {
            let mut perturbed_state = base_state.clone();
            let perturbed_entry = perturbed_state.entries.get_mut(path).ok_or_else(|| {
                Cs336A1ReferenceTrainingError::InvalidConfig(format!(
                    "missing parameter entry `{path}` during finite-difference gradient estimation"
                ))
            })?;
            let perturbed_values = dense_tensor_values_mut(perturbed_entry)?;
            perturbed_values[index] += self.config.finite_difference_epsilon;
            let mut perturbed_model = self.model.clone();
            perturbed_model.load_state_dict(&perturbed_state, ModuleStateLoadMode::Strict)?;
            let perturbed_loss = self.loss_for_batch(&perturbed_model, batch)?;
            gradient_values
                .push((perturbed_loss - base_loss) / self.config.finite_difference_epsilon);
        }
        Ok(ModuleStateEntry {
            path: path.to_string(),
            kind: ModuleStateEntryKind::Parameter,
            spec: parameter_entry.spec.clone(),
            data: TensorData::F32(gradient_values),
            requires_grad: true,
            persistent: true,
        })
    }

    pub fn apply_precomputed_gradients(
        &mut self,
        batch: &Cs336A1ReferenceBatch,
        loss_before: f32,
        mut gradients: ModuleStateDict,
    ) -> Result<Cs336A1ReferenceTrainingStepReport, Cs336A1ReferenceTrainingError> {
        let gradient_clip =
            cs336_a1_gradient_clipping(&mut gradients, self.config.gradient_clip_norm)?;
        let learning_rate = cs336_a1_get_lr_cosine_schedule(
            self.iteration,
            self.config.max_learning_rate,
            self.config.min_learning_rate,
            self.config.warmup_iters,
            self.config.cosine_cycle_iters,
        );
        let model_state_before = self.model.state_dict();
        let model_state_digest_before = model_state_before.state_dict_digest.clone();
        let mut updated_state = model_state_before.clone();
        for (path, gradient_entry) in &gradients.entries {
            let parameter_entry = updated_state.entries.get_mut(path).ok_or_else(|| {
                Cs336A1ReferenceTrainingError::InvalidConfig(format!(
                    "missing parameter entry `{path}` during optimizer step"
                ))
            })?;
            let parameter_values = dense_tensor_values_mut(parameter_entry)?;
            let gradient_values = dense_tensor_values(&gradient_entry.data)?;
            let optimizer_config = self.config.optimizer_config(learning_rate);
            let optimizer_state = self
                .optimizer_states
                .entry(path.clone())
                .or_insert_with(|| optimizer_config.initialize_state(parameter_values.len()));
            optimizer_config.apply_step(
                parameter_values.as_mut_slice(),
                gradient_values,
                optimizer_state,
                self.iteration + 1,
            )?;
        }
        self.model
            .load_state_dict(&updated_state, ModuleStateLoadMode::Strict)?;
        let loss_after = self.loss_for_batch(&self.model, batch)?;
        self.iteration += 1;
        self.loss_history.push(loss_after);
        Ok(Cs336A1ReferenceTrainingStepReport {
            step_number: self.iteration,
            learning_rate,
            loss_before,
            loss_after,
            gradient_clip,
            model_state_digest_before,
            model_state_digest_after: self.model.state_dict().state_dict_digest,
            optimizer_state_digest_after: optimizer_state_digest(&self.optimizer_states)?,
        })
    }

    pub fn step_with_batch(
        &mut self,
        batch: &Cs336A1ReferenceBatch,
    ) -> Result<Cs336A1ReferenceTrainingStepReport, Cs336A1ReferenceTrainingError> {
        let loss_before = self.loss_for_explicit_batch(batch)?;
        let gradients = self.estimate_gradients(batch, loss_before)?;
        self.apply_precomputed_gradients(batch, loss_before, gradients)
    }

    pub fn step(
        &mut self,
    ) -> Result<Cs336A1ReferenceTrainingStepReport, Cs336A1ReferenceTrainingError> {
        let batch = self.batch_for_iteration(self.iteration)?;
        self.step_with_batch(&batch)
    }

    pub fn run_steps(
        &mut self,
        step_count: usize,
    ) -> Result<Vec<Cs336A1ReferenceTrainingStepReport>, Cs336A1ReferenceTrainingError> {
        let mut reports = Vec::with_capacity(step_count);
        for _ in 0..step_count {
            reports.push(self.step()?);
        }
        Ok(reports)
    }

    pub fn save_checkpoint(
        &self,
        checkpoint_path: impl AsRef<Path>,
    ) -> Result<Cs336A1ReferenceCheckpointReceipt, Cs336A1ReferenceTrainingError> {
        let checkpoint = Cs336A1ReferenceCheckpoint {
            schema_version: String::from(CS336_A1_REFERENCE_CHECKPOINT_SCHEMA_VERSION),
            config: self.config.clone(),
            tokenizer_artifacts: self.tokenizer_artifacts.clone(),
            tokenized_dataset: self.tokenized_dataset.clone(),
            iteration: self.iteration,
            loss_history: self.loss_history.clone(),
            model_state: self.model.state_dict(),
            optimizer_states: self.optimizer_states.clone(),
        };
        save_cs336_a1_reference_checkpoint(checkpoint_path, &checkpoint)
    }

    #[must_use]
    pub fn model_state_digest(&self) -> String {
        self.model.state_dict().state_dict_digest
    }

    pub fn optimizer_state_digest(&self) -> Result<String, Cs336A1ReferenceTrainingError> {
        optimizer_state_digest(&self.optimizer_states)
    }

    fn from_artifacts_and_corpus_text(
        config: Cs336A1ReferenceTrainingConfig,
        tokenizer_artifacts: Cs336A1BytePairEncodingArtifacts,
        corpus: &str,
    ) -> Result<Self, Cs336A1ReferenceTrainingError> {
        config.validate()?;
        let tokenizer = tokenizer_from_artifacts(&tokenizer_artifacts)?;
        let tokenized_dataset = tokenizer
            .encode(corpus)
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect::<Vec<_>>();
        validate_tokenized_dataset(tokenized_dataset.as_slice(), config.context_length)?;
        let mut model = Cs336A1TransformerLm::new(
            "cs336_a1_reference_lm",
            config.model_config(),
            config.rope_theta,
            config.rms_norm_eps,
        )?;
        initialize_cs336_a1_reference_model(&mut model)?;
        Ok(Self {
            config,
            tokenizer_artifacts,
            tokenized_dataset,
            model,
            optimizer_states: BTreeMap::new(),
            iteration: 0,
            loss_history: Vec::new(),
        })
    }

    fn estimate_gradients(
        &self,
        batch: &Cs336A1ReferenceBatch,
        base_loss: f32,
    ) -> Result<ModuleStateDict, Cs336A1ReferenceTrainingError> {
        let mut gradient_entries = BTreeMap::new();
        for path in self.trainable_parameter_paths() {
            gradient_entries.insert(
                path.clone(),
                self.estimate_parameter_gradient(batch, base_loss, path.as_str())?,
            );
        }
        let base_state = self.model.state_dict();
        Ok(ModuleStateDict::new(
            base_state.root_module_id.clone(),
            base_state.root_module_kind.clone(),
            ModuleStateView::PersistentOnly,
            gradient_entries,
        )?)
    }

    fn loss_for_batch(
        &self,
        model: &Cs336A1TransformerLm,
        batch: &Cs336A1ReferenceBatch,
    ) -> Result<f32, Cs336A1ReferenceTrainingError> {
        let logits = model.forward_tokens(batch.token_shape(), &batch.input_ids())?;
        let loss = cross_entropy_loss(&logits, &batch.target_ids(), LossReduction::Mean)?;
        scalar_from_nn_tensor(&loss)
    }
}

pub fn cs336_a1_get_batch(
    dataset: &[u32],
    batch_size: usize,
    context_length: usize,
    iteration: u64,
) -> Result<Cs336A1ReferenceBatch, Cs336A1ReferenceTrainingError> {
    if batch_size == 0 {
        return Err(Cs336A1ReferenceTrainingError::InvalidConfig(
            "batch_size must be positive".into(),
        ));
    }
    if context_length == 0 {
        return Err(Cs336A1ReferenceTrainingError::InvalidConfig(
            "context_length must be positive".into(),
        ));
    }
    validate_tokenized_dataset(dataset, context_length)?;
    let window_count = dataset.len() - context_length;
    let mut start_positions = Vec::with_capacity(batch_size);
    let mut inputs = Vec::with_capacity(batch_size * context_length);
    let mut targets = Vec::with_capacity(batch_size * context_length);
    for row in 0..batch_size {
        let start = (((iteration as usize) * batch_size) + row) % window_count;
        start_positions.push(start);
        inputs.extend_from_slice(&dataset[start..start + context_length]);
        targets.extend_from_slice(&dataset[start + 1..start + context_length + 1]);
    }
    Ok(Cs336A1ReferenceBatch {
        iteration,
        batch_size,
        context_length,
        start_positions,
        inputs,
        targets,
    })
}

pub fn cs336_a1_softmax(
    input: &NnTensor,
    dim: isize,
) -> Result<NnTensor, Cs336A1ReferenceTrainingError> {
    let dims = input.dims();
    if dims.is_empty() {
        return Err(Cs336A1ReferenceTrainingError::InvalidConfig(
            "softmax input rank must be at least one".into(),
        ));
    }
    let axis = normalize_dim(dims.len(), dim)?;
    let axis_size = dims[axis];
    if axis_size == 0 {
        return Err(Cs336A1ReferenceTrainingError::InvalidConfig(
            "softmax axis size must be positive".into(),
        ));
    }
    let inner = dims[axis + 1..].iter().product::<usize>().max(1);
    let outer = dims[..axis].iter().product::<usize>().max(1);
    let values = input.as_f32_slice()?;
    let mut output = vec![0.0; values.len()];
    for outer_index in 0..outer {
        for inner_index in 0..inner {
            let mut max_value = f32::NEG_INFINITY;
            for axis_index in 0..axis_size {
                let index = (outer_index * axis_size * inner) + (axis_index * inner) + inner_index;
                max_value = max_value.max(values[index]);
            }
            let mut sum = 0.0;
            for axis_index in 0..axis_size {
                let index = (outer_index * axis_size * inner) + (axis_index * inner) + inner_index;
                let exp = (values[index] - max_value).exp();
                output[index] = exp;
                sum += exp;
            }
            for axis_index in 0..axis_size {
                let index = (outer_index * axis_size * inner) + (axis_index * inner) + inner_index;
                output[index] /= sum;
            }
        }
    }
    Ok(NnTensor::f32(Shape::new(dims.to_vec()), output)?)
}

pub fn cs336_a1_cross_entropy(
    logits: &NnTensor,
    targets: &[usize],
) -> Result<f32, Cs336A1ReferenceTrainingError> {
    let loss = cross_entropy_loss(logits, targets, LossReduction::Mean)?;
    scalar_from_nn_tensor(&loss)
}

pub fn cs336_a1_gradient_clipping(
    gradients: &mut ModuleStateDict,
    max_l2_norm: f32,
) -> Result<Cs336A1GradientClipReport, Cs336A1ReferenceTrainingError> {
    if max_l2_norm <= 0.0 {
        return Err(Cs336A1ReferenceTrainingError::InvalidConfig(
            "max_l2_norm must be positive".into(),
        ));
    }
    let gradient_norm_l2_before = module_state_dict_l2_norm(gradients)?;
    if gradient_norm_l2_before == 0.0 || gradient_norm_l2_before <= max_l2_norm {
        return Ok(Cs336A1GradientClipReport {
            gradient_norm_l2_before,
            gradient_norm_l2_after: gradient_norm_l2_before,
            clipping_ratio: Some(1.0),
        });
    }
    let clipping_ratio = max_l2_norm / gradient_norm_l2_before;
    for entry in gradients.entries.values_mut() {
        for value in dense_tensor_values_mut(entry)? {
            *value *= clipping_ratio;
        }
    }
    Ok(Cs336A1GradientClipReport {
        gradient_norm_l2_before,
        gradient_norm_l2_after: module_state_dict_l2_norm(gradients)?,
        clipping_ratio: Some(clipping_ratio),
    })
}

#[must_use]
pub fn cs336_a1_adamw_config(
    learning_rate: f32,
    beta1: f32,
    beta2: f32,
    epsilon: f32,
    weight_decay: f32,
) -> TrainingOptimizerConfig {
    TrainingOptimizerConfig::adamw(learning_rate, beta1, beta2, epsilon)
        .with_weight_decay(weight_decay)
}

#[must_use]
pub fn cs336_a1_get_lr_cosine_schedule(
    iteration: u64,
    max_learning_rate: f32,
    min_learning_rate: f32,
    warmup_iters: u64,
    cosine_cycle_iters: u64,
) -> f32 {
    if warmup_iters > 0 && iteration < warmup_iters {
        return max_learning_rate * ((iteration + 1) as f32 / warmup_iters as f32);
    }
    if cosine_cycle_iters <= warmup_iters {
        return min_learning_rate;
    }
    if iteration >= cosine_cycle_iters {
        return min_learning_rate;
    }
    let decay_progress = (iteration.saturating_sub(warmup_iters)) as f32
        / (cosine_cycle_iters - warmup_iters) as f32;
    let cosine = (std::f32::consts::PI * decay_progress).cos();
    min_learning_rate + (0.5 * (1.0 + cosine) * (max_learning_rate - min_learning_rate))
}

pub fn save_cs336_a1_reference_checkpoint(
    checkpoint_path: impl AsRef<Path>,
    checkpoint: &Cs336A1ReferenceCheckpoint,
) -> Result<Cs336A1ReferenceCheckpointReceipt, Cs336A1ReferenceTrainingError> {
    let checkpoint_path = checkpoint_path.as_ref();
    if let Some(parent) = checkpoint_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(checkpoint)?;
    fs::write(checkpoint_path, &bytes)?;
    Ok(Cs336A1ReferenceCheckpointReceipt {
        checkpoint_path: checkpoint_path.display().to_string(),
        checkpoint_digest: hex::encode(Sha256::digest(bytes)),
        iteration: checkpoint.iteration,
        model_state_digest: checkpoint.model_state.state_dict_digest.clone(),
        optimizer_state_digest: optimizer_state_digest(&checkpoint.optimizer_states)?,
    })
}

pub fn load_cs336_a1_reference_checkpoint(
    checkpoint_path: impl AsRef<Path>,
) -> Result<Cs336A1ReferenceCheckpoint, Cs336A1ReferenceTrainingError> {
    let bytes = fs::read(checkpoint_path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn write_cs336_a1_reference_tiny_training_bundle(
    workspace_root: impl AsRef<Path>,
) -> Result<Cs336A1ReferenceTrainingBundle, Cs336A1ReferenceTrainingError> {
    let workspace_root = workspace_root.as_ref();
    let corpus_path = workspace_root.join(CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH);
    let checkpoint_step2_path =
        workspace_root.join(CS336_A1_REFERENCE_TINY_CHECKPOINT_STEP2_FIXTURE_PATH);
    let checkpoint_step4_path =
        workspace_root.join(CS336_A1_REFERENCE_TINY_CHECKPOINT_STEP4_FIXTURE_PATH);
    let bundle_path = workspace_root.join(CS336_A1_REFERENCE_TINY_TRAINING_BUNDLE_FIXTURE_PATH);
    let config = Cs336A1ReferenceTrainingConfig::tiny();
    let corpus = fs::read_to_string(&corpus_path)?;

    let mut trainer = Cs336A1ReferenceTrainer::from_corpus_text(&corpus, config.clone())?;
    let initial_loss = trainer.current_loss()?;
    let pre_checkpoint_steps = trainer.run_steps(2)?;
    let checkpoint_step2 = trainer.save_checkpoint(&checkpoint_step2_path)?;

    let mut resumed = Cs336A1ReferenceTrainer::load_checkpoint(&checkpoint_step2_path)?;
    let resumed_steps = resumed.run_steps(2)?;
    let checkpoint_step4 = resumed.save_checkpoint(&checkpoint_step4_path)?;

    let mut uninterrupted = Cs336A1ReferenceTrainer::from_corpus_text(&corpus, config.clone())?;
    let uninterrupted_steps = uninterrupted.run_steps(4)?;

    let bundle = Cs336A1ReferenceTrainingBundle {
        schema_version: String::from(CS336_A1_REFERENCE_TRAINING_BUNDLE_SCHEMA_VERSION),
        config: config.clone(),
        corpus_path: corpus_path.display().to_string(),
        corpus_digest: trainer.tokenizer_artifacts.corpus_digest.clone(),
        tokenizer_digest: trainer.tokenizer_artifacts.tokenizer_digest.stable_digest(),
        dataset_digest: stable_dataset_digest(trainer.tokenized_dataset.as_slice()),
        initial_loss,
        pre_checkpoint_steps,
        resumed_steps,
        uninterrupted_steps,
        checkpoint_step2,
        checkpoint_step4,
        resumed_final_model_state_digest: resumed.model_state_digest(),
        uninterrupted_final_model_state_digest: uninterrupted.model_state_digest(),
        resumed_final_optimizer_state_digest: resumed.optimizer_state_digest()?,
        uninterrupted_final_optimizer_state_digest: uninterrupted.optimizer_state_digest()?,
        resume_matches_uninterrupted: resumed.model_state_digest()
            == uninterrupted.model_state_digest()
            && resumed.optimizer_state_digest()? == uninterrupted.optimizer_state_digest()?,
        schedule_preview: (0..config.cosine_cycle_iters)
            .map(|iteration| {
                cs336_a1_get_lr_cosine_schedule(
                    iteration,
                    config.max_learning_rate,
                    config.min_learning_rate,
                    config.warmup_iters,
                    config.cosine_cycle_iters,
                )
            })
            .collect(),
        claim_boundary: String::from(
            "This bundle proves a bounded CS336 A1 training lane in owned Rust: tokenizer-trained tiny corpus, deterministic batching, softmax and cross-entropy loss, global gradient clipping, AdamW updates, cosine schedule, checkpoint save/load, and resume exactness against an uninterrupted run. The gradient path is finite-difference reference math for a tiny model and does not claim scalable broader-pretraining backward support.",
        ),
    };
    if let Some(parent) = bundle_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&bundle_path, serde_json::to_vec_pretty(&bundle)?)?;
    Ok(bundle)
}

fn tokenizer_from_artifacts(
    tokenizer_artifacts: &Cs336A1BytePairEncodingArtifacts,
) -> Result<Cs336A1BytePairTokenizer, Cs336A1ReferenceTrainingError> {
    let vocab = tokenizer_artifacts.vocabulary_bytes()?;
    let merges = tokenizer_artifacts.merge_pairs_bytes()?;
    Ok(Cs336A1BytePairTokenizer::from_vocab_and_merges(
        vocab.as_slice(),
        merges.as_slice(),
        tokenizer_artifacts.special_tokens.as_slice(),
    )?)
}

pub fn initialize_cs336_a1_reference_model(
    model: &mut Cs336A1TransformerLm,
) -> Result<(), Cs336A1ReferenceTrainingError> {
    let mut state_dict = model.state_dict();
    for (path, entry) in &mut state_dict.entries {
        if entry.kind != ModuleStateEntryKind::Parameter {
            continue;
        }
        let values = dense_tensor_values_mut(entry)?;
        if path.ends_with("ln1.weight")
            || path.ends_with("ln2.weight")
            || path.ends_with("ln_final.weight")
        {
            for value in values {
                *value = 1.0;
            }
            continue;
        }
        for (index, value) in values.iter_mut().enumerate() {
            *value = deterministic_parameter_value(path.as_str(), index);
        }
    }
    model.load_state_dict(&state_dict, ModuleStateLoadMode::Strict)?;
    Ok(())
}

fn deterministic_parameter_value(path: &str, index: usize) -> f32 {
    let mut hasher = Sha256::new();
    hasher.update(b"psion.cs336_a1.reference_parameter");
    hasher.update(path.as_bytes());
    hasher.update(index.to_le_bytes());
    let digest = hasher.finalize();
    let integer = u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]);
    (((integer % 10_000) as f32 / 10_000.0) - 0.5) * 0.08
}

fn dense_tensor_values(data: &TensorData) -> Result<&[f32], Cs336A1ReferenceTrainingError> {
    match data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values.as_slice()),
        other => Err(Cs336A1ReferenceTrainingError::InvalidConfig(format!(
            "expected dense floating tensor, found `{other:?}`"
        ))),
    }
}

fn dense_tensor_values_mut(
    entry: &mut ModuleStateEntry,
) -> Result<&mut Vec<f32>, Cs336A1ReferenceTrainingError> {
    match &mut entry.data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values),
        other => Err(Cs336A1ReferenceTrainingError::InvalidConfig(format!(
            "expected mutable dense floating tensor at `{}`, found `{other:?}`",
            entry.path
        ))),
    }
}

fn scalar_from_nn_tensor(tensor: &NnTensor) -> Result<f32, Cs336A1ReferenceTrainingError> {
    let values = tensor.as_f32_slice()?;
    if values.len() != 1 {
        return Err(Cs336A1ReferenceTrainingError::InvalidConfig(format!(
            "expected scalar tensor, found {} values",
            values.len()
        )));
    }
    Ok(values[0])
}

fn module_state_dict_l2_norm(
    gradients: &ModuleStateDict,
) -> Result<f32, Cs336A1ReferenceTrainingError> {
    let mut sum = 0.0;
    for entry in gradients.entries.values() {
        for value in dense_tensor_values(&entry.data)? {
            sum += value * value;
        }
    }
    Ok(sum.sqrt())
}

fn optimizer_state_digest(
    optimizer_states: &BTreeMap<String, TrainingOptimizerState>,
) -> Result<String, Cs336A1ReferenceTrainingError> {
    Ok(stable_json_digest(
        b"psion.cs336_a1.reference_optimizer_state",
        optimizer_states,
    ))
}

fn stable_dataset_digest(dataset: &[u32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion.cs336_a1.reference_dataset");
    for token in dataset {
        hasher.update(token.to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_json_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    let bytes = serde_json::to_vec(value).expect("serializable digest payload");
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn validate_tokenized_dataset(
    dataset: &[u32],
    context_length: usize,
) -> Result<(), Cs336A1ReferenceTrainingError> {
    if dataset.len() <= context_length {
        return Err(Cs336A1ReferenceTrainingError::InvalidConfig(format!(
            "tokenized dataset length {} must exceed context_length {}",
            dataset.len(),
            context_length
        )));
    }
    Ok(())
}

fn normalize_dim(rank: usize, dim: isize) -> Result<usize, Cs336A1ReferenceTrainingError> {
    let normalized = if dim < 0 { (rank as isize) + dim } else { dim };
    if normalized < 0 || normalized >= rank as isize {
        return Err(Cs336A1ReferenceTrainingError::InvalidConfig(format!(
            "softmax dim {dim} is out of range for rank {rank}"
        )));
    }
    Ok(normalized as usize)
}

#[cfg(test)]
mod tests {
    use super::{
        cs336_a1_adamw_config, cs336_a1_cross_entropy, cs336_a1_get_batch,
        cs336_a1_get_lr_cosine_schedule, cs336_a1_gradient_clipping, cs336_a1_softmax,
        load_cs336_a1_reference_checkpoint, write_cs336_a1_reference_tiny_training_bundle,
        Cs336A1ReferenceTrainer, Cs336A1ReferenceTrainingConfig,
    };
    use psionic_core::{DType, Device, Shape, TensorData, TensorSpec};
    use psionic_nn::{
        ModuleStateDict, ModuleStateEntry, ModuleStateEntryKind, ModuleStateView, NnTensor,
    };
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    #[test]
    fn get_batch_cycles_deterministically() -> Result<(), Box<dyn std::error::Error>> {
        let dataset = vec![10, 11, 12, 13, 14];
        let batch = cs336_a1_get_batch(&dataset, 2, 2, 1)?;
        assert_eq!(batch.start_positions, vec![2, 0]);
        assert_eq!(batch.inputs, vec![12, 13, 10, 11]);
        assert_eq!(batch.targets, vec![13, 14, 11, 12]);
        Ok(())
    }

    #[test]
    fn softmax_normalizes_requested_dimension() -> Result<(), Box<dyn std::error::Error>> {
        let tensor = NnTensor::f32(Shape::new(vec![2, 2]), vec![1.0, 3.0, 2.0, 4.0])?;
        let softmax = cs336_a1_softmax(&tensor, 0)?;
        let values = softmax.as_f32_slice()?;
        let column0 = values[0] + values[2];
        let column1 = values[1] + values[3];
        assert!((column0 - 1.0).abs() < 1e-5);
        assert!((column1 - 1.0).abs() < 1e-5);
        Ok(())
    }

    #[test]
    fn cross_entropy_matches_expected_average() -> Result<(), Box<dyn std::error::Error>> {
        let logits = NnTensor::f32(Shape::new(vec![2, 3]), vec![2.0, 0.0, -2.0, 0.0, 1.0, -1.0])?;
        let loss = cs336_a1_cross_entropy(&logits, &[0, 1])?;
        assert!(loss > 0.0);
        assert!(loss < 1.0);
        Ok(())
    }

    #[test]
    fn gradient_clipping_enforces_global_norm_bound() -> Result<(), Box<dyn std::error::Error>> {
        let spec = TensorSpec::new(Shape::new(vec![2]), DType::F32, Device::cpu());
        let mut gradients = ModuleStateDict::new(
            "cs336_a1_gradients",
            "test_gradients",
            ModuleStateView::PersistentOnly,
            BTreeMap::from([
                (
                    String::from("layer0.weight"),
                    ModuleStateEntry {
                        path: String::from("layer0.weight"),
                        kind: ModuleStateEntryKind::Parameter,
                        spec: spec.clone(),
                        data: TensorData::F32(vec![3.0, 4.0]),
                        requires_grad: true,
                        persistent: true,
                    },
                ),
                (
                    String::from("layer1.weight"),
                    ModuleStateEntry {
                        path: String::from("layer1.weight"),
                        kind: ModuleStateEntryKind::Parameter,
                        spec,
                        data: TensorData::F32(vec![12.0, 0.0]),
                        requires_grad: true,
                        persistent: true,
                    },
                ),
            ]),
        )?;
        let report = cs336_a1_gradient_clipping(&mut gradients, 1.0)?;
        let values = gradients
            .entries
            .values()
            .flat_map(|entry| match &entry.data {
                TensorData::F32(values) => values.clone(),
                _ => Vec::new(),
            })
            .collect::<Vec<_>>();
        let l2_norm = values.iter().map(|value| value.powi(2)).sum::<f32>().sqrt();
        assert!(report.clipping_ratio.is_some());
        assert!(report.gradient_norm_l2_before > 1.0);
        assert!(report.gradient_norm_l2_after <= 1.0 + 1e-5);
        assert!((l2_norm - 1.0).abs() < 1e-5);
        Ok(())
    }

    #[test]
    fn adamw_config_updates_parameters_on_first_step() -> Result<(), Box<dyn std::error::Error>> {
        let optimizer = cs336_a1_adamw_config(0.1, 0.9, 0.95, 1e-8, 0.0);
        let mut parameters = vec![1.0, -2.0];
        let gradients = vec![0.5, -0.25];
        let mut state = optimizer.initialize_state(parameters.len());
        optimizer.apply_step(&mut parameters, gradients.as_slice(), &mut state, 1)?;
        assert!((parameters[0] - 0.9).abs() < 1e-5);
        assert!((parameters[1] + 1.9).abs() < 1e-5);
        Ok(())
    }

    #[test]
    fn cosine_schedule_warms_up_and_decays() {
        assert!((cs336_a1_get_lr_cosine_schedule(0, 0.1, 0.01, 2, 6) - 0.05).abs() < 1e-6);
        assert!((cs336_a1_get_lr_cosine_schedule(1, 0.1, 0.01, 2, 6) - 0.1).abs() < 1e-6);
        assert!(cs336_a1_get_lr_cosine_schedule(5, 0.1, 0.01, 2, 6) >= 0.01);
        assert_eq!(cs336_a1_get_lr_cosine_schedule(7, 0.1, 0.01, 2, 6), 0.01);
    }

    #[test]
    fn tiny_reference_training_descends_and_resume_matches_uninterrupted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let corpus = "the cat sat on the mat.\nthe cat saw the mat.\n";
        let config = Cs336A1ReferenceTrainingConfig::tiny();
        let mut trainer = Cs336A1ReferenceTrainer::from_corpus_text(corpus, config.clone())?;
        let initial_loss = trainer.current_loss()?;
        let first_reports = trainer.run_steps(2)?;
        let tempdir = tempdir()?;
        let checkpoint_path = tempdir.path().join("checkpoint.json");
        trainer.save_checkpoint(&checkpoint_path)?;
        let mut resumed = Cs336A1ReferenceTrainer::load_checkpoint(&checkpoint_path)?;
        let resumed_reports = resumed.run_steps(2)?;
        let mut uninterrupted = Cs336A1ReferenceTrainer::from_corpus_text(corpus, config)?;
        let uninterrupted_reports = uninterrupted.run_steps(4)?;
        assert!(first_reports
            .iter()
            .chain(resumed_reports.iter())
            .any(|report| report.loss_after < initial_loss));
        assert_eq!(
            resumed.model_state_digest(),
            uninterrupted.model_state_digest()
        );
        assert_eq!(
            resumed.optimizer_state_digest()?,
            uninterrupted.optimizer_state_digest()?
        );
        assert_eq!(resumed_reports.len(), 2);
        assert_eq!(uninterrupted_reports.len(), 4);
        Ok(())
    }

    #[test]
    fn checkpoint_round_trip_preserves_iteration_and_state_digests(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let corpus = "the cat sat on the mat.\nthe cat saw the mat.\n";
        let mut trainer = Cs336A1ReferenceTrainer::from_corpus_text(
            corpus,
            Cs336A1ReferenceTrainingConfig::tiny(),
        )?;
        let _ = trainer.run_steps(2)?;
        let expected_model_digest = trainer.model_state_digest();
        let expected_optimizer_digest = trainer.optimizer_state_digest()?;
        let tempdir = tempdir()?;
        let checkpoint_path = tempdir.path().join("checkpoint.json");
        trainer.save_checkpoint(&checkpoint_path)?;
        let checkpoint = load_cs336_a1_reference_checkpoint(&checkpoint_path)?;
        let restored = Cs336A1ReferenceTrainer::load_checkpoint(&checkpoint_path)?;
        assert_eq!(checkpoint.iteration, 2);
        assert_eq!(
            checkpoint.model_state.state_dict_digest,
            expected_model_digest
        );
        assert_eq!(restored.model_state_digest(), expected_model_digest);
        assert_eq!(
            restored.optimizer_state_digest()?,
            expected_optimizer_digest
        );
        Ok(())
    }

    #[test]
    fn fixture_writer_emits_resume_exactness_bundle() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempdir()?;
        std::fs::create_dir_all(root.path().join("fixtures/training"))?;
        std::fs::write(
            root.path()
                .join("fixtures/training/cs336_a1_reference_tiny_corpus.txt"),
            "the cat sat on the mat.\nthe cat saw the mat.\n",
        )?;
        let bundle = write_cs336_a1_reference_tiny_training_bundle(root.path())?;
        assert!(bundle.resume_matches_uninterrupted);
        Ok(())
    }
}
