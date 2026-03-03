use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::hash::stable_hex_digest;
use crate::text_to_cad_dataset::{TextToCadDataset, summarize_annotations};
use crate::{CadError, CadResult};

pub const TEXT_TO_CAD_TRAINING_HOOK_GATE_ENV: &str = "OPENAGENTS_CAD_ENABLE_TRAINING_HOOKS";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextToCadTrainingHookConfig {
    pub enable_training_hooks: bool,
    pub eval_ratio_percent: u8,
}

impl Default for TextToCadTrainingHookConfig {
    fn default() -> Self {
        Self {
            enable_training_hooks: false,
            eval_ratio_percent: 20,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextToCadTrainingHookGate {
    pub code: String,
    pub message: String,
    pub gate_env: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextToCadTrainingEvalHooks {
    pub dataset_hash: String,
    pub train_sample_ids: Vec<String>,
    pub eval_sample_ids: Vec<String>,
    pub annotation_summary_hash: String,
    pub train_payload_hash: String,
    pub eval_payload_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TextToCadTrainingEvalOutcome {
    Gated(TextToCadTrainingHookGate),
    Ready(TextToCadTrainingEvalHooks),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextToCadTrainingHookRecord {
    pub sample_id: String,
    pub split: String,
    pub prompt: String,
    pub model: String,
    pub compact_ir_hash: String,
}

pub fn build_text_to_cad_training_eval_hooks(
    dataset: &TextToCadDataset,
    config: TextToCadTrainingHookConfig,
) -> CadResult<TextToCadTrainingEvalOutcome> {
    if !config.enable_training_hooks {
        return Ok(TextToCadTrainingEvalOutcome::Gated(
            TextToCadTrainingHookGate {
                code: "CAD0-TRAINING-HOOKS-GATED".to_string(),
                message: "training/eval hook generation is disabled by gate".to_string(),
                gate_env: TEXT_TO_CAD_TRAINING_HOOK_GATE_ENV.to_string(),
            },
        ));
    }

    if config.eval_ratio_percent == 0 || config.eval_ratio_percent > 50 {
        return Err(CadError::InvalidParameter {
            name: "eval_ratio_percent".to_string(),
            reason: "eval_ratio_percent must be in range [1, 50]".to_string(),
        });
    }

    if dataset.samples.is_empty() {
        return Err(CadError::InvalidParameter {
            name: "dataset.samples".to_string(),
            reason: "dataset must include at least one sample".to_string(),
        });
    }

    let mut train_sample_ids = Vec::new();
    let mut eval_sample_ids = Vec::new();
    for sample in &dataset.samples {
        let score = split_score(&dataset.dataset_hash, &sample.sample_id);
        if score < config.eval_ratio_percent {
            eval_sample_ids.push(sample.sample_id.clone());
        } else {
            train_sample_ids.push(sample.sample_id.clone());
        }
    }

    if eval_sample_ids.is_empty() {
        if let Some(moved) = train_sample_ids.pop() {
            eval_sample_ids.push(moved);
        }
    } else if train_sample_ids.is_empty() {
        if let Some(moved) = eval_sample_ids.pop() {
            train_sample_ids.push(moved);
        }
    }

    train_sample_ids.sort();
    eval_sample_ids.sort();

    let annotation_summary = summarize_annotations(dataset);
    let annotation_summary_hash = stable_hex_digest(
        serde_json::to_string(&annotation_summary)
            .unwrap_or_default()
            .as_bytes(),
    );

    let train_payload_hash = stable_hex_digest(train_sample_ids.join(",").as_bytes());
    let eval_payload_hash = stable_hex_digest(eval_sample_ids.join(",").as_bytes());

    Ok(TextToCadTrainingEvalOutcome::Ready(
        TextToCadTrainingEvalHooks {
            dataset_hash: dataset.dataset_hash.clone(),
            train_sample_ids,
            eval_sample_ids,
            annotation_summary_hash,
            train_payload_hash,
            eval_payload_hash,
        },
    ))
}

pub fn training_hook_records_ndjson(
    dataset: &TextToCadDataset,
    hooks: &TextToCadTrainingEvalHooks,
) -> CadResult<String> {
    let by_id: BTreeMap<&str, _> = dataset
        .samples
        .iter()
        .map(|sample| (sample.sample_id.as_str(), sample))
        .collect();

    let mut records = Vec::new();
    for sample_id in &hooks.train_sample_ids {
        let Some(sample) = by_id.get(sample_id.as_str()) else {
            return Err(CadError::ParseFailed {
                reason: format!("training hooks reference missing train sample {sample_id}"),
            });
        };
        records.push(TextToCadTrainingHookRecord {
            sample_id: sample.sample_id.clone(),
            split: "train".to_string(),
            prompt: sample.prompt.clone(),
            model: sample.model.as_str().to_string(),
            compact_ir_hash: sample.annotation.compact_ir_hash.clone(),
        });
    }
    for sample_id in &hooks.eval_sample_ids {
        let Some(sample) = by_id.get(sample_id.as_str()) else {
            return Err(CadError::ParseFailed {
                reason: format!("training hooks reference missing eval sample {sample_id}"),
            });
        };
        records.push(TextToCadTrainingHookRecord {
            sample_id: sample.sample_id.clone(),
            split: "eval".to_string(),
            prompt: sample.prompt.clone(),
            model: sample.model.as_str().to_string(),
            compact_ir_hash: sample.annotation.compact_ir_hash.clone(),
        });
    }
    records.sort_by(|left, right| left.sample_id.cmp(&right.sample_id));

    let mut lines = Vec::with_capacity(records.len());
    for record in records {
        lines.push(
            serde_json::to_string(&record).map_err(|error| CadError::ParseFailed {
                reason: format!("failed to serialize training hook record: {error}"),
            })?,
        );
    }
    Ok(lines.join("\n"))
}

fn split_score(dataset_hash: &str, sample_id: &str) -> u8 {
    let digest = stable_hex_digest(format!("{dataset_hash}|{sample_id}").as_bytes());
    let prefix = &digest[..8.min(digest.len())];
    let value = u32::from_str_radix(prefix, 16).unwrap_or(0);
    (value % 100) as u8
}

#[cfg(test)]
mod tests {
    use super::{
        TextToCadTrainingEvalOutcome, TextToCadTrainingHookConfig,
        build_text_to_cad_training_eval_hooks, training_hook_records_ndjson,
    };
    use crate::text_to_cad_dataset::{TextToCadDatasetConfig, generate_text_to_cad_dataset};

    #[test]
    fn hooks_are_gated_by_default() {
        let dataset =
            generate_text_to_cad_dataset(TextToCadDatasetConfig::default()).expect("dataset");
        let outcome =
            build_text_to_cad_training_eval_hooks(&dataset, TextToCadTrainingHookConfig::default())
                .expect("hooks");
        match outcome {
            TextToCadTrainingEvalOutcome::Gated(gated) => {
                assert_eq!(gated.code, "CAD0-TRAINING-HOOKS-GATED");
                assert!(
                    gated
                        .gate_env
                        .contains("OPENAGENTS_CAD_ENABLE_TRAINING_HOOKS")
                );
            }
            other => panic!("expected gated outcome, got {other:?}"),
        }
    }

    #[test]
    fn enabled_hooks_produce_deterministic_train_eval_split() {
        let dataset = generate_text_to_cad_dataset(TextToCadDatasetConfig {
            seed: 42,
            samples_per_family: 2,
            include_mini_profile: true,
        })
        .expect("dataset");
        let config = TextToCadTrainingHookConfig {
            enable_training_hooks: true,
            eval_ratio_percent: 20,
        };
        let first = build_text_to_cad_training_eval_hooks(&dataset, config.clone()).expect("hooks");
        let second = build_text_to_cad_training_eval_hooks(&dataset, config).expect("hooks");
        assert_eq!(first, second);
        match first {
            TextToCadTrainingEvalOutcome::Ready(hooks) => {
                assert!(!hooks.train_sample_ids.is_empty());
                assert!(!hooks.eval_sample_ids.is_empty());
                assert_eq!(
                    hooks.train_sample_ids.len() + hooks.eval_sample_ids.len(),
                    dataset.samples.len()
                );
            }
            other => panic!("expected ready outcome, got {other:?}"),
        }
    }

    #[test]
    fn invalid_eval_ratio_is_rejected() {
        let dataset =
            generate_text_to_cad_dataset(TextToCadDatasetConfig::default()).expect("dataset");
        let error = build_text_to_cad_training_eval_hooks(
            &dataset,
            TextToCadTrainingHookConfig {
                enable_training_hooks: true,
                eval_ratio_percent: 0,
            },
        )
        .expect_err("eval ratio 0 must fail");
        assert!(error.to_string().contains("eval_ratio_percent"));
    }

    #[test]
    fn ready_hooks_can_export_ndjson_records() {
        let dataset =
            generate_text_to_cad_dataset(TextToCadDatasetConfig::default()).expect("dataset");
        let outcome = build_text_to_cad_training_eval_hooks(
            &dataset,
            TextToCadTrainingHookConfig {
                enable_training_hooks: true,
                eval_ratio_percent: 20,
            },
        )
        .expect("hooks");
        let hooks = match outcome {
            TextToCadTrainingEvalOutcome::Ready(hooks) => hooks,
            other => panic!("expected ready outcome, got {other:?}"),
        };
        let ndjson = training_hook_records_ndjson(&dataset, &hooks).expect("ndjson");
        assert_eq!(ndjson.lines().count(), dataset.samples.len());
    }
}
