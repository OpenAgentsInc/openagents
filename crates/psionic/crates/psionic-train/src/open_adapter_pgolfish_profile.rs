use psionic_data::{TokenizerDigest, TokenizerFamily};

use crate::{
    first_swarm_open_adapter_training_config, OpenAdapterExecutionConfig,
    OpenAdapterHiddenStateSample, OpenAdapterTrainingExecutionError, TrainingLoopBudget,
};

pub const OPEN_ADAPTER_PGOLFISH_HIDDEN_SIZE: usize = 512;
pub const OPEN_ADAPTER_PGOLFISH_VOCAB_SIZE: usize = 1_024;
pub const OPEN_ADAPTER_PGOLFISH_LORA_RANK: usize = 32;
pub const OPEN_ADAPTER_PGOLFISH_BATCH_SIZE: usize = 8;
pub const OPEN_ADAPTER_PGOLFISH_TRAIN_SAMPLE_COUNT: usize = 128;
pub const OPEN_ADAPTER_PGOLFISH_HOLDOUT_SAMPLE_COUNT: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenAdapterPgolfishSampleSplit {
    Training,
    Holdout,
}

impl OpenAdapterPgolfishSampleSplit {
    fn sample_count(self) -> usize {
        match self {
            Self::Training => OPEN_ADAPTER_PGOLFISH_TRAIN_SAMPLE_COUNT,
            Self::Holdout => OPEN_ADAPTER_PGOLFISH_HOLDOUT_SAMPLE_COUNT,
        }
    }

    fn sample_index_offset(self) -> usize {
        match self {
            Self::Training => 0,
            Self::Holdout => 10_000,
        }
    }

    fn target_offset(self) -> usize {
        match self {
            Self::Training => 0,
            Self::Holdout => 211,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Training => "training",
            Self::Holdout => "holdout",
        }
    }
}

pub fn open_adapter_pgolfish_config(
    backend_label: &str,
    run_id: String,
    checkpoint_family: String,
    max_steps: u64,
) -> Result<OpenAdapterExecutionConfig, OpenAdapterTrainingExecutionError> {
    open_adapter_pgolfish_config_with_batch_size(
        backend_label,
        run_id,
        checkpoint_family,
        max_steps,
        OPEN_ADAPTER_PGOLFISH_BATCH_SIZE,
    )
}

pub fn open_adapter_pgolfish_config_with_batch_size(
    backend_label: &str,
    run_id: String,
    checkpoint_family: String,
    max_steps: u64,
    batch_size: usize,
) -> Result<OpenAdapterExecutionConfig, OpenAdapterTrainingExecutionError> {
    let mut config =
        first_swarm_open_adapter_training_config(run_id, checkpoint_family, backend_label);
    config.budget = TrainingLoopBudget::new(max_steps, 1, 1)?;
    config.batch_size = batch_size;
    config.model.hidden_size = OPEN_ADAPTER_PGOLFISH_HIDDEN_SIZE;
    config.model.vocab_size = OPEN_ADAPTER_PGOLFISH_VOCAB_SIZE;
    config.model.target.lora_rank = OPEN_ADAPTER_PGOLFISH_LORA_RANK;
    config.model.target.lora_alpha = OPEN_ADAPTER_PGOLFISH_LORA_RANK as f32;
    config.model.tokenizer = TokenizerDigest::new(
        TokenizerFamily::SentencePiece,
        "psionic.synthetic.pgolfish.sp1024.v1",
        OPEN_ADAPTER_PGOLFISH_VOCAB_SIZE as u32,
    )
    .with_template_digest("psionic.synthetic.pgolfish.prompt_template.v1");
    Ok(config)
}

pub fn open_adapter_pgolfish_samples(
    sample_prefix: &str,
    split: OpenAdapterPgolfishSampleSplit,
) -> Result<Vec<OpenAdapterHiddenStateSample>, OpenAdapterTrainingExecutionError> {
    let mut samples = Vec::with_capacity(split.sample_count());
    for sample_index in 0..split.sample_count() {
        let canonical_index = sample_index + split.sample_index_offset();
        let target_token_id = ((canonical_index * 37 + split.target_offset())
            % OPEN_ADAPTER_PGOLFISH_VOCAB_SIZE) as u32;
        let mut hidden_state = Vec::with_capacity(OPEN_ADAPTER_PGOLFISH_HIDDEN_SIZE);
        for dim in 0..OPEN_ADAPTER_PGOLFISH_HIDDEN_SIZE {
            hidden_state.push(synthetic_hidden_value(
                canonical_index,
                dim,
                target_token_id as usize,
            ));
        }
        samples.push(OpenAdapterHiddenStateSample::new(
            format!("{sample_prefix}-{sample_index:04}-{}", split.label()),
            hidden_state,
            target_token_id,
            192 + (canonical_index % 64) as u32,
        )?);
    }
    Ok(samples)
}

fn synthetic_hidden_value(sample_index: usize, dim: usize, target_token_id: usize) -> f32 {
    let seed = ((sample_index as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        ^ ((dim as u64 + 1).wrapping_mul(0xBF58_476D_1CE4_E5B9))
        ^ ((target_token_id as u64 + 1).wrapping_mul(0x94D0_49BB_1331_11EB));
    let hashed = seed ^ (seed >> 30);
    let base = ((hashed & 0xffff) as f32 / 32768.0) - 1.0;
    let bucket = if dim % 64 == target_token_id % 64 {
        0.75
    } else if dim % 17 == sample_index % 17 {
        0.25
    } else {
        0.0
    };
    (base * 0.35) + bucket
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holdout_split_is_distinct_from_training_split() {
        let training = open_adapter_pgolfish_samples(
            "pgolfish-test",
            OpenAdapterPgolfishSampleSplit::Training,
        )
        .expect("training split should build");
        let holdout =
            open_adapter_pgolfish_samples("pgolfish-test", OpenAdapterPgolfishSampleSplit::Holdout)
                .expect("holdout split should build");
        assert_eq!(training.len(), OPEN_ADAPTER_PGOLFISH_TRAIN_SAMPLE_COUNT);
        assert_eq!(holdout.len(), OPEN_ADAPTER_PGOLFISH_HOLDOUT_SAMPLE_COUNT);
        assert_ne!(training[0].sample_digest, holdout[0].sample_digest);
        assert_ne!(training[0].target_token_id, holdout[0].target_token_id);
    }
}
