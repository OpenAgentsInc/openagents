use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::hash::stable_hex_digest;
use crate::text_to_cad::{TextToCadModelProfile, TextToCadOutcome, TextToCadRequest, text_to_cad};
use crate::{CadError, CadResult};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextToCadDatasetConfig {
    pub seed: u64,
    pub samples_per_family: usize,
    pub include_mini_profile: bool,
}

impl Default for TextToCadDatasetConfig {
    fn default() -> Self {
        Self {
            seed: 1,
            samples_per_family: 2,
            include_mini_profile: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextToCadAnnotation {
    pub part_family: String,
    pub operation_count: usize,
    pub root_count: usize,
    pub prompt_token_count: usize,
    pub numeric_token_count: usize,
    pub compact_ir_hash: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextToCadDatasetSample {
    pub sample_id: String,
    pub prompt: String,
    pub model: TextToCadModelProfile,
    pub compact_ir: String,
    pub annotation: TextToCadAnnotation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextToCadDataset {
    pub manifest_version: u64,
    pub seed: u64,
    pub samples_per_family: usize,
    pub include_mini_profile: bool,
    pub samples: Vec<TextToCadDatasetSample>,
    pub dataset_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextToCadAnnotationSummary {
    pub sample_count: usize,
    pub by_family: BTreeMap<String, usize>,
    pub by_model: BTreeMap<String, usize>,
    pub max_operation_count: usize,
    pub min_operation_count: usize,
}

pub fn generate_text_to_cad_dataset(config: TextToCadDatasetConfig) -> CadResult<TextToCadDataset> {
    if config.samples_per_family == 0 {
        return Err(CadError::InvalidParameter {
            name: "samples_per_family".to_string(),
            reason: "samples_per_family must be greater than zero".to_string(),
        });
    }

    let profiles = if config.include_mini_profile {
        vec![TextToCadModelProfile::Cad0, TextToCadModelProfile::Cad0Mini]
    } else {
        vec![TextToCadModelProfile::Cad0]
    };

    let families = ["bracket", "stand", "enclosure"];
    let mut samples = Vec::new();
    for (family_index, family) in families.iter().enumerate() {
        for sample_index in 0..config.samples_per_family {
            let prompt = prompt_for_family(config.seed, family, family_index, sample_index);
            for model in &profiles {
                let generated = match text_to_cad(TextToCadRequest {
                    prompt: prompt.clone(),
                    model: *model,
                })? {
                    TextToCadOutcome::Generated(generated) => generated,
                    TextToCadOutcome::Clarification(clarification) => {
                        return Err(CadError::ParseFailed {
                            reason: format!(
                                "dataset template prompt produced clarification {}: {}",
                                clarification.code, clarification.message
                            ),
                        });
                    }
                };
                let sample_id = format!("dataset.{family}.{}.{}", sample_index + 1, model.as_str());
                let annotation = TextToCadAnnotation {
                    part_family: (*family).to_string(),
                    operation_count: generated.operation_count,
                    root_count: generated.ir.roots.len(),
                    prompt_token_count: token_count(&prompt),
                    numeric_token_count: numeric_token_count(&prompt),
                    compact_ir_hash: stable_hex_digest(generated.compact_ir.as_bytes()),
                    tags: vec![
                        format!("family:{family}"),
                        format!("model:{}", model.as_str()),
                        "source:text_to_cad_adapter".to_string(),
                    ],
                };
                samples.push(TextToCadDatasetSample {
                    sample_id,
                    prompt: prompt.clone(),
                    model: *model,
                    compact_ir: generated.compact_ir,
                    annotation,
                });
            }
        }
    }

    samples.sort_by(|left, right| left.sample_id.cmp(&right.sample_id));
    let dataset_hash = dataset_hash(&samples)?;
    Ok(TextToCadDataset {
        manifest_version: 1,
        seed: config.seed,
        samples_per_family: config.samples_per_family,
        include_mini_profile: config.include_mini_profile,
        samples,
        dataset_hash,
    })
}

pub fn dataset_to_ndjson(dataset: &TextToCadDataset) -> CadResult<String> {
    let mut lines = Vec::with_capacity(dataset.samples.len());
    for sample in &dataset.samples {
        lines.push(
            serde_json::to_string(sample).map_err(|error| CadError::ParseFailed {
                reason: format!(
                    "failed to serialize dataset sample {}: {error}",
                    sample.sample_id
                ),
            })?,
        );
    }
    Ok(lines.join("\n"))
}

pub fn summarize_annotations(dataset: &TextToCadDataset) -> TextToCadAnnotationSummary {
    let mut by_family = BTreeMap::new();
    let mut by_model = BTreeMap::new();
    let mut min_operation_count = usize::MAX;
    let mut max_operation_count = 0usize;

    for sample in &dataset.samples {
        *by_family
            .entry(sample.annotation.part_family.clone())
            .or_insert(0) += 1;
        *by_model
            .entry(sample.model.as_str().to_string())
            .or_insert(0) += 1;
        min_operation_count = min_operation_count.min(sample.annotation.operation_count);
        max_operation_count = max_operation_count.max(sample.annotation.operation_count);
    }

    if dataset.samples.is_empty() {
        min_operation_count = 0;
    }

    TextToCadAnnotationSummary {
        sample_count: dataset.samples.len(),
        by_family,
        by_model,
        max_operation_count,
        min_operation_count,
    }
}

fn prompt_for_family(seed: u64, family: &str, family_index: usize, sample_index: usize) -> String {
    let family_seed = mix_seed(seed, (family_index as u64) + 1, (sample_index as u64) + 1);
    let a = ranged_value(family_seed, 60.0, 140.0);
    let b = ranged_value(family_seed.rotate_left(13), 30.0, 90.0);
    let c = ranged_value(family_seed.rotate_left(27), 3.0, 14.0);
    let d = ranged_value(family_seed.rotate_left(39), 4.0, 12.0);
    match family {
        "bracket" => {
            format!(
                "Design a bracket {:.0} {:.0} {:.0} with {:.0}mm holes",
                a, b, c, d
            )
        }
        "stand" => {
            let back = ranged_value(family_seed.rotate_left(7), 50.0, 120.0);
            format!(
                "Design a phone stand {:.0} {:.0} {:.0} {:.0}",
                a, b, c, back
            )
        }
        "enclosure" => {
            let z = ranged_value(family_seed.rotate_left(11), 30.0, 80.0);
            format!("Design an enclosure {:.0} {:.0} {:.0} {:.0}", a, b, z, c)
        }
        _ => format!("Design a part {:.0} {:.0} {:.0}", a, b, c),
    }
}

fn mix_seed(seed: u64, family: u64, sample: u64) -> u64 {
    let mut x = seed
        ^ family.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ sample.wrapping_mul(0xD2B7_44A1_C5D4_3B29);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn ranged_value(seed: u64, min: f64, max: f64) -> f64 {
    let ratio = (seed as f64) / (u64::MAX as f64);
    min + (max - min) * ratio
}

fn dataset_hash(samples: &[TextToCadDatasetSample]) -> CadResult<String> {
    let json = serde_json::to_string(samples).map_err(|error| CadError::ParseFailed {
        reason: format!("failed to serialize dataset samples for hash: {error}"),
    })?;
    Ok(stable_hex_digest(json.as_bytes()))
}

fn token_count(prompt: &str) -> usize {
    prompt
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .count()
}

fn numeric_token_count(prompt: &str) -> usize {
    let mut count = 0usize;
    let mut current = String::new();
    for ch in prompt.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            current.push(ch);
        } else if !current.is_empty() {
            if current.parse::<f64>().is_ok() {
                count += 1;
            }
            current.clear();
        }
    }
    if !current.is_empty() && current.parse::<f64>().is_ok() {
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::{
        TextToCadDatasetConfig, dataset_to_ndjson, generate_text_to_cad_dataset,
        summarize_annotations,
    };

    #[test]
    fn dataset_generation_is_deterministic() {
        let config = TextToCadDatasetConfig {
            seed: 42,
            samples_per_family: 2,
            include_mini_profile: true,
        };
        let first = generate_text_to_cad_dataset(config.clone()).expect("first dataset generation");
        let second = generate_text_to_cad_dataset(config).expect("second dataset generation");
        assert_eq!(first, second);
        assert!(!first.dataset_hash.is_empty());
    }

    #[test]
    fn dataset_sample_count_matches_configuration() {
        let config = TextToCadDatasetConfig {
            seed: 7,
            samples_per_family: 3,
            include_mini_profile: false,
        };
        let dataset = generate_text_to_cad_dataset(config).expect("dataset generation");
        assert_eq!(dataset.samples.len(), 3 * 3);
        assert!(
            dataset
                .samples
                .iter()
                .all(|sample| sample.annotation.operation_count > 0)
        );
    }

    #[test]
    fn dataset_annotations_can_be_summarized() {
        let dataset = generate_text_to_cad_dataset(TextToCadDatasetConfig::default())
            .expect("dataset generation");
        let summary = summarize_annotations(&dataset);
        assert_eq!(summary.by_family.len(), 3);
        assert_eq!(summary.by_model.len(), 2);
        assert!(summary.max_operation_count >= summary.min_operation_count);
    }

    #[test]
    fn dataset_can_be_exported_to_ndjson() {
        let dataset = generate_text_to_cad_dataset(TextToCadDatasetConfig {
            seed: 9,
            samples_per_family: 1,
            include_mini_profile: true,
        })
        .expect("dataset generation");
        let ndjson = dataset_to_ndjson(&dataset).expect("ndjson export");
        assert_eq!(ndjson.lines().count(), dataset.samples.len());
    }
}
