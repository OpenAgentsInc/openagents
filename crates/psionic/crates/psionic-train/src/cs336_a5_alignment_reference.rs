use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable lane identifier for the bounded CS336 A5 alignment reference lane.
pub const CS336_A5_REFERENCE_LANE_ID: &str = "psion_cs336_a5_alignment_reference_v1";
/// Claim boundary for the bounded CS336 A5 alignment reference lane.
pub const CS336_A5_REFERENCE_CLAIM_BOUNDARY: &str = "bounded deterministic f64 reference math \
     for the portable Stanford CS336 A5 adapter surface only; no model execution, no RL training \
     run, no tokenizer coupling, and no claim of full A5 parity against Stanford fixtures";

/// Failure for one bounded A5 reference computation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Cs336A5ReferenceError {
    /// Batch inputs disagree on length.
    #[error("batch arity mismatch: {left} vs {right}")]
    BatchArityMismatch {
        /// First length.
        left: usize,
        /// Second length.
        right: usize,
    },
    /// The reward batch is not divisible into equal groups.
    #[error("rollout batch of {batch} is not divisible by group size {group_size}")]
    GroupSizeMismatch {
        /// Rollout batch size.
        batch: usize,
        /// Requested group size.
        group_size: usize,
    },
    /// A required argument for the selected method is missing.
    #[error("method `{method}` requires `{argument}`")]
    MissingArgument {
        /// Selected method name.
        method: &'static str,
        /// Missing argument name.
        argument: &'static str,
    },
    /// A sequence batch is empty where data is required.
    #[error("empty batch")]
    EmptyBatch,
}

/// One tokenized prompt/output batch over pre-tokenized id sequences.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A5TokenizedBatch {
    /// `(batch, max_len - 1)` concatenated ids with the final token sliced.
    pub input_ids: Vec<Vec<u32>>,
    /// `(batch, max_len - 1)` shifted ids (concatenation without the first).
    pub labels: Vec<Vec<u32>>,
    /// `(batch, max_len - 1)` mask aligned with labels: 1 where the label
    /// token belongs to the response, 0 for prompt or padding.
    pub response_mask: Vec<Vec<u8>>,
}

/// Tokenizes prompt/output id pairs into the A5 next-token training layout.
///
/// The Stanford adapter runs a HuggingFace tokenizer over strings first;
/// this bounded lane takes pre-tokenized id sequences and owns only the
/// concatenation, shifting, padding, and response-mask construction.
pub fn cs336_a5_tokenize_prompt_and_output(
    prompts: &[Vec<u32>],
    outputs: &[Vec<u32>],
    pad_token: u32,
) -> Result<Cs336A5TokenizedBatch, Cs336A5ReferenceError> {
    if prompts.len() != outputs.len() {
        return Err(Cs336A5ReferenceError::BatchArityMismatch {
            left: prompts.len(),
            right: outputs.len(),
        });
    }
    if prompts.is_empty() {
        return Err(Cs336A5ReferenceError::EmptyBatch);
    }
    let max_len = prompts
        .iter()
        .zip(outputs)
        .map(|(prompt, output)| prompt.len() + output.len())
        .max()
        .unwrap_or(0);
    let width = max_len.saturating_sub(1);
    let mut input_ids = Vec::with_capacity(prompts.len());
    let mut labels = Vec::with_capacity(prompts.len());
    let mut response_mask = Vec::with_capacity(prompts.len());
    for (prompt, output) in prompts.iter().zip(outputs) {
        let mut joined: Vec<u32> = Vec::with_capacity(max_len);
        joined.extend_from_slice(prompt);
        joined.extend_from_slice(output);
        let joined_len = joined.len();
        joined.resize(max_len, pad_token);
        let row_inputs: Vec<u32> = joined[..width].to_vec();
        let row_labels: Vec<u32> = joined[1..].to_vec();
        let mut row_mask: Vec<u8> = vec![0; width];
        // Label index i carries joined[i + 1]; that token is part of the
        // response when prompt_len <= i + 1 < joined_len.
        for (index, mask) in row_mask.iter_mut().enumerate() {
            let label_position = index + 1;
            if label_position >= prompt.len() && label_position < joined_len {
                *mask = 1;
            }
        }
        input_ids.push(row_inputs);
        labels.push(row_labels);
        response_mask.push(row_mask);
    }
    Ok(Cs336A5TokenizedBatch {
        input_ids,
        labels,
        response_mask,
    })
}

/// Baseline applied within each rollout group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cs336A5Baseline {
    /// Subtract the per-group mean reward.
    Mean,
    /// No baseline subtraction.
    None,
}

/// Normalizer applied within each rollout group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cs336A5AdvantageNormalizer {
    /// Divide by the per-group sample standard deviation plus epsilon.
    Std,
    /// No normalization.
    None,
    /// Divide by the per-group mean reward plus epsilon.
    Mean,
}

/// Metadata logged with group-normalized rewards.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A5RewardMetadata {
    /// Mean reward over the rollout batch.
    pub reward_mean: f64,
    /// Maximum reward over the rollout batch.
    pub reward_max: f64,
    /// Minimum reward over the rollout batch.
    pub reward_min: f64,
}

/// Computes group-normalized advantages from raw rollout rewards.
///
/// Standard deviations are sample standard deviations (n - 1 denominator),
/// matching the torch default the Stanford adapter inherits.
pub fn cs336_a5_group_normalized_rewards(
    raw_rewards: &[f64],
    group_size: usize,
    baseline: Cs336A5Baseline,
    advantage_eps: f64,
    normalizer: Cs336A5AdvantageNormalizer,
) -> Result<(Vec<f64>, Cs336A5RewardMetadata), Cs336A5ReferenceError> {
    if raw_rewards.is_empty() {
        return Err(Cs336A5ReferenceError::EmptyBatch);
    }
    if group_size == 0 || !raw_rewards.len().is_multiple_of(group_size) {
        return Err(Cs336A5ReferenceError::GroupSizeMismatch {
            batch: raw_rewards.len(),
            group_size,
        });
    }
    let mut advantages = Vec::with_capacity(raw_rewards.len());
    for group in raw_rewards.chunks(group_size) {
        let mean = group.iter().sum::<f64>() / group.len() as f64;
        let centered: Vec<f64> = match baseline {
            Cs336A5Baseline::Mean => group.iter().map(|reward| reward - mean).collect(),
            Cs336A5Baseline::None => group.to_vec(),
        };
        let denominator = match normalizer {
            Cs336A5AdvantageNormalizer::None => 1.0,
            Cs336A5AdvantageNormalizer::Mean => mean + advantage_eps,
            Cs336A5AdvantageNormalizer::Std => {
                let variance = if group.len() > 1 {
                    group
                        .iter()
                        .map(|reward| {
                            let deviation = reward - mean;
                            deviation * deviation
                        })
                        .sum::<f64>()
                        / (group.len() - 1) as f64
                } else {
                    0.0
                };
                variance.sqrt() + advantage_eps
            }
        };
        for value in centered {
            advantages.push(value / denominator);
        }
    }
    let reward_mean = raw_rewards.iter().sum::<f64>() / raw_rewards.len() as f64;
    let reward_max = raw_rewards.iter().copied().fold(f64::MIN, f64::max);
    let reward_min = raw_rewards.iter().copied().fold(f64::MAX, f64::min);
    Ok((
        advantages,
        Cs336A5RewardMetadata {
            reward_mean,
            reward_max,
            reward_min,
        },
    ))
}

/// Importance-reweighting method for the policy-gradient loss.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cs336A5ImportanceReweighting {
    /// No importance reweighting: `-A * log_prob`.
    None,
    /// Token-level importance ratio without clipping.
    NoClip,
    /// PPO/GRPO-style token-level clipped objective.
    Grpo,
    /// GSPO-style sequence-level ratio with clipping.
    Gspo,
}

/// Metadata logged with the policy-gradient loss.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A5PolicyGradientMetadata {
    /// Fraction of contributing positions where the clipped branch was
    /// active (zero for unclipped methods).
    pub clip_fraction: f64,
}

/// Computes the per-token policy-gradient loss.
pub fn cs336_a5_policy_gradient_loss(
    rewards_or_advantages: &[f64],
    policy_log_probs: &[Vec<f64>],
    method: Cs336A5ImportanceReweighting,
    old_log_probs: Option<&[Vec<f64>]>,
    cliprange: Option<f64>,
    response_mask: Option<&[Vec<u8>]>,
) -> Result<(Vec<Vec<f64>>, Cs336A5PolicyGradientMetadata), Cs336A5ReferenceError> {
    if rewards_or_advantages.len() != policy_log_probs.len() {
        return Err(Cs336A5ReferenceError::BatchArityMismatch {
            left: rewards_or_advantages.len(),
            right: policy_log_probs.len(),
        });
    }
    if policy_log_probs.is_empty() {
        return Err(Cs336A5ReferenceError::EmptyBatch);
    }
    let needs_old = !matches!(method, Cs336A5ImportanceReweighting::None);
    let old = match (needs_old, old_log_probs) {
        (true, None) => {
            return Err(Cs336A5ReferenceError::MissingArgument {
                method: method_name(method),
                argument: "old_log_probs",
            });
        }
        (_, old) => old,
    };
    let needs_clip = matches!(
        method,
        Cs336A5ImportanceReweighting::Grpo | Cs336A5ImportanceReweighting::Gspo
    );
    let clip = match (needs_clip, cliprange) {
        (true, None) => {
            return Err(Cs336A5ReferenceError::MissingArgument {
                method: method_name(method),
                argument: "cliprange",
            });
        }
        (_, clip) => clip,
    };
    let mut per_token: Vec<Vec<f64>> = Vec::with_capacity(policy_log_probs.len());
    let mut clipped_positions = 0_usize;
    let mut total_positions = 0_usize;
    for (row_index, log_probs) in policy_log_probs.iter().enumerate() {
        let advantage = rewards_or_advantages[row_index];
        let mut row = Vec::with_capacity(log_probs.len());
        match method {
            Cs336A5ImportanceReweighting::None => {
                for log_prob in log_probs {
                    row.push(-advantage * log_prob);
                }
            }
            Cs336A5ImportanceReweighting::NoClip => {
                let old_row = &old.unwrap_or_default()[row_index];
                for (log_prob, old_log_prob) in log_probs.iter().zip(old_row) {
                    let ratio = (log_prob - old_log_prob).exp();
                    row.push(-advantage * ratio);
                }
            }
            Cs336A5ImportanceReweighting::Grpo => {
                let old_row = &old.unwrap_or_default()[row_index];
                let epsilon = clip.unwrap_or_default();
                for (log_prob, old_log_prob) in log_probs.iter().zip(old_row) {
                    let ratio = (log_prob - old_log_prob).exp();
                    let clipped = ratio.clamp(1.0 - epsilon, 1.0 + epsilon);
                    let unclipped_term = ratio * advantage;
                    let clipped_term = clipped * advantage;
                    total_positions += 1;
                    if clipped_term < unclipped_term {
                        clipped_positions += 1;
                    }
                    row.push(-unclipped_term.min(clipped_term));
                }
            }
            Cs336A5ImportanceReweighting::Gspo => {
                let old_row = &old.unwrap_or_default()[row_index];
                let mask_row: Option<&Vec<u8>> = response_mask.map(|mask| &mask[row_index]);
                let mut ratio_sum = 0.0_f64;
                let mut ratio_count = 0_usize;
                for (index, (log_prob, old_log_prob)) in log_probs.iter().zip(old_row).enumerate() {
                    let included = mask_row.is_none_or(|mask| mask[index] != 0);
                    if included {
                        ratio_sum += log_prob - old_log_prob;
                        ratio_count += 1;
                    }
                }
                if ratio_count == 0 {
                    return Err(Cs336A5ReferenceError::MissingArgument {
                        method: "gspo",
                        argument: "response_mask with at least one included token",
                    });
                }
                let sequence_ratio = (ratio_sum / ratio_count as f64).exp();
                let epsilon = clip.unwrap_or_default();
                let clipped = sequence_ratio.clamp(1.0 - epsilon, 1.0 + epsilon);
                let unclipped_term = sequence_ratio * advantage;
                let clipped_term = clipped * advantage;
                total_positions += 1;
                if clipped_term < unclipped_term {
                    clipped_positions += 1;
                }
                let loss = -unclipped_term.min(clipped_term);
                row = vec![loss; log_probs.len()];
            }
        }
        per_token.push(row);
    }
    let clip_fraction = if total_positions == 0 {
        0.0
    } else {
        clipped_positions as f64 / total_positions as f64
    };
    Ok((per_token, Cs336A5PolicyGradientMetadata { clip_fraction }))
}

fn method_name(method: Cs336A5ImportanceReweighting) -> &'static str {
    match method {
        Cs336A5ImportanceReweighting::None => "none",
        Cs336A5ImportanceReweighting::NoClip => "noclip",
        Cs336A5ImportanceReweighting::Grpo => "grpo",
        Cs336A5ImportanceReweighting::Gspo => "gspo",
    }
}

/// Loss-normalization strategy for microbatch aggregation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cs336A5LossNormalization {
    /// Average masked loss per sequence, then average over sequences.
    Sequence,
    /// Divide the total masked loss by a constant.
    Constant,
}

/// Aggregates the per-token policy-gradient loss across one microbatch.
pub fn cs336_a5_aggregate_loss_across_microbatch(
    per_token_loss: &[Vec<f64>],
    mask: &[Vec<u8>],
    normalization: Cs336A5LossNormalization,
    normalization_constant: Option<i64>,
) -> Result<f64, Cs336A5ReferenceError> {
    if per_token_loss.len() != mask.len() {
        return Err(Cs336A5ReferenceError::BatchArityMismatch {
            left: per_token_loss.len(),
            right: mask.len(),
        });
    }
    if per_token_loss.is_empty() {
        return Err(Cs336A5ReferenceError::EmptyBatch);
    }
    match normalization {
        Cs336A5LossNormalization::Sequence => {
            let mut sequence_means = Vec::with_capacity(per_token_loss.len());
            for (row, mask_row) in per_token_loss.iter().zip(mask) {
                let mut total = 0.0_f64;
                let mut count = 0_usize;
                for (value, included) in row.iter().zip(mask_row) {
                    if *included != 0 {
                        total += value;
                        count += 1;
                    }
                }
                sequence_means.push(if count == 0 {
                    0.0
                } else {
                    total / count as f64
                });
            }
            Ok(sequence_means.iter().sum::<f64>() / sequence_means.len() as f64)
        }
        Cs336A5LossNormalization::Constant => {
            let constant =
                normalization_constant.ok_or(Cs336A5ReferenceError::MissingArgument {
                    method: "constant",
                    argument: "normalization_constant",
                })?;
            let mut total = 0.0_f64;
            for (row, mask_row) in per_token_loss.iter().zip(mask) {
                for (value, included) in row.iter().zip(mask_row) {
                    if *included != 0 {
                        total += value;
                    }
                }
            }
            Ok(total / constant as f64)
        }
    }
}

/// Computes the per-instance DPO loss from summed sequence log-probs:
/// `-log sigmoid(beta * ((pi_w - ref_w) - (pi_l - ref_l)))`.
///
/// The Stanford adapter runs two models and a tokenizer to obtain these
/// log-probs; this bounded lane owns the loss math over supplied values.
#[must_use]
pub fn cs336_a5_per_instance_dpo_loss(
    policy_chosen_log_prob: f64,
    policy_rejected_log_prob: f64,
    reference_chosen_log_prob: f64,
    reference_rejected_log_prob: f64,
    beta: f64,
) -> f64 {
    let margin = (policy_chosen_log_prob - reference_chosen_log_prob)
        - (policy_rejected_log_prob - reference_rejected_log_prob);
    let scaled = beta * margin;
    // -log(sigmoid(x)) computed stably as softplus(-x).
    let negative = -scaled;
    if negative > 0.0 {
        negative + (-negative).exp().ln_1p()
    } else {
        negative.exp().ln_1p()
    }
}

/// One scored rollout's reward components, as the Stanford reward callables
/// produce them.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A5RewardComponents {
    /// Total reward.
    pub reward: f64,
    /// Format-compliance reward component.
    pub format_reward: f64,
    /// Answer-correctness reward component.
    pub answer_reward: f64,
}

/// Aggregated rollout-reward statistics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A5RolloutRewardSummary {
    /// Raw total rewards per rollout, in input order.
    pub raw_rewards: Vec<f64>,
    /// Mean total reward over the rollout batch.
    pub reward_mean: f64,
    /// Mean format reward over the rollout batch.
    pub format_reward_mean: f64,
    /// Mean answer reward over the rollout batch.
    pub answer_reward_mean: f64,
}

/// Aggregates precomputed rollout reward components.
///
/// The Stanford adapter invokes the reward callable itself; this bounded
/// lane owns the aggregation over already-scored components.
pub fn cs336_a5_rollout_rewards(
    components: &[Cs336A5RewardComponents],
) -> Result<Cs336A5RolloutRewardSummary, Cs336A5ReferenceError> {
    if components.is_empty() {
        return Err(Cs336A5ReferenceError::EmptyBatch);
    }
    let count = components.len() as f64;
    Ok(Cs336A5RolloutRewardSummary {
        raw_rewards: components.iter().map(|c| c.reward).collect(),
        reward_mean: components.iter().map(|c| c.reward).sum::<f64>() / count,
        format_reward_mean: components.iter().map(|c| c.format_reward).sum::<f64>() / count,
        answer_reward_mean: components.iter().map(|c| c.answer_reward).sum::<f64>() / count,
    })
}

/// Parses one MMLU response into an answer letter.
///
/// Bounded heuristic: returns the first standalone `A`-`D` letter token,
/// preferring an explicit `The correct answer is X` pattern.
#[must_use]
pub fn cs336_a5_parse_mmlu_response(output: &str) -> Option<char> {
    let explicit = output
        .split("the correct answer is")
        .nth(1)
        .or_else(|| output.split("The correct answer is").nth(1));
    let candidates: Vec<&str> = match explicit {
        Some(rest) => vec![rest, output],
        None => vec![output],
    };
    for candidate in candidates {
        for token in candidate.split(|c: char| !c.is_ascii_alphanumeric()) {
            if token.len() == 1 {
                let letter = token.chars().next()?;
                if ('A'..='D').contains(&letter) {
                    return Some(letter);
                }
            }
        }
    }
    None
}

/// Parses one GSM8K response into its final numeric answer.
///
/// Bounded heuristic: returns the last numeric token (commas and dollar
/// signs stripped, optional sign and decimal point).
#[must_use]
pub fn cs336_a5_parse_gsm8k_response(output: &str) -> Option<String> {
    let mut last: Option<String> = None;
    let cleaned = output.replace([',', '$'], "");
    let mut current = String::new();
    let mut chars = cleaned.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            current.push(c);
            continue;
        }
        if c == '.' && !current.is_empty() && chars.peek().is_some_and(char::is_ascii_digit) {
            current.push(c);
            continue;
        }
        if c == '-' && current.is_empty() && chars.peek().is_some_and(char::is_ascii_digit) {
            current.push(c);
            continue;
        }
        if !current.is_empty() && current.chars().any(|d| d.is_ascii_digit()) {
            last = Some(std::mem::take(&mut current));
        } else {
            current.clear();
        }
    }
    if !current.is_empty() && current.chars().any(|d| d.is_ascii_digit()) {
        last = Some(current);
    }
    last
}

/// Packs tokenized documents into fixed-length training blocks by
/// concatenation and chunking, dropping the trailing remainder.
///
/// Status `partial`: the packing layout is the standard concat-and-chunk
/// construction, but exact conformance against the Stanford packed-SFT
/// fixtures has not been verified.
pub fn cs336_a5_pack_sft_sequences(
    sequences: &[Vec<u32>],
    block_length: usize,
) -> Result<Vec<Vec<u32>>, Cs336A5ReferenceError> {
    if block_length == 0 || sequences.is_empty() {
        return Err(Cs336A5ReferenceError::EmptyBatch);
    }
    let mut stream: Vec<u32> = Vec::new();
    for sequence in sequences {
        stream.extend_from_slice(sequence);
    }
    Ok(stream
        .chunks_exact(block_length)
        .map(<[u32]>::to_vec)
        .collect())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn tokenize_builds_shifted_labels_and_response_mask() {
        let prompts = vec![vec![10, 11], vec![20]];
        let outputs = vec![vec![12, 13], vec![21, 22, 23]];
        let batch = cs336_a5_tokenize_prompt_and_output(&prompts, &outputs, 0).expect("tokenizes");
        // max joint length 4, width 3.
        assert_eq!(batch.input_ids[0], vec![10, 11, 12]);
        assert_eq!(batch.labels[0], vec![11, 12, 13]);
        assert_eq!(batch.response_mask[0], vec![0, 1, 1]);
        assert_eq!(batch.input_ids[1], vec![20, 21, 22]);
        assert_eq!(batch.labels[1], vec![21, 22, 23]);
        assert_eq!(batch.response_mask[1], vec![1, 1, 1]);
    }

    #[test]
    fn group_normalization_matches_hand_computation() {
        // Two groups of two: [1, 3] and [2, 2].
        let (advantages, metadata) = cs336_a5_group_normalized_rewards(
            &[1.0, 3.0, 2.0, 2.0],
            2,
            Cs336A5Baseline::Mean,
            1e-6,
            Cs336A5AdvantageNormalizer::Std,
        )
        .expect("normalizes");
        // Group one: mean 2, sample std sqrt(2).
        let expected = 1.0 / (2.0_f64.sqrt() + 1e-6);
        assert!((advantages[0] + expected).abs() < 1e-9);
        assert!((advantages[1] - expected).abs() < 1e-9);
        // Group two: zero deviations.
        assert!(advantages[2].abs() < 1e-9);
        assert!(advantages[3].abs() < 1e-9);
        assert!((metadata.reward_mean - 2.0).abs() < 1e-12);
    }

    #[test]
    fn group_size_mismatch_refuses() {
        assert_eq!(
            cs336_a5_group_normalized_rewards(
                &[1.0, 2.0, 3.0],
                2,
                Cs336A5Baseline::Mean,
                1e-6,
                Cs336A5AdvantageNormalizer::Std,
            )
            .expect_err("refuses"),
            Cs336A5ReferenceError::GroupSizeMismatch {
                batch: 3,
                group_size: 2
            }
        );
    }

    #[test]
    fn policy_gradient_none_is_negative_advantage_times_log_prob() {
        let (loss, metadata) = cs336_a5_policy_gradient_loss(
            &[2.0],
            &[vec![-0.5, -1.0]],
            Cs336A5ImportanceReweighting::None,
            None,
            None,
            None,
        )
        .expect("computes");
        assert_eq!(loss, vec![vec![1.0, 2.0]]);
        assert!((metadata.clip_fraction).abs() < 1e-12);
    }

    #[test]
    fn grpo_clips_large_ratios_and_reports_clip_fraction() {
        // ratio = e^(0.5) ~ 1.6487 against cliprange 0.2 -> clipped to 1.2.
        let (loss, metadata) = cs336_a5_policy_gradient_loss(
            &[1.0],
            &[vec![-0.5, -1.0]],
            Cs336A5ImportanceReweighting::Grpo,
            Some(&[vec![-1.0, -1.0]]),
            Some(0.2),
            None,
        )
        .expect("computes");
        let clipped = -(1.2_f64);
        assert!((loss[0][0] - clipped).abs() < 1e-9);
        // Second token: ratio 1.0, unclipped.
        assert!((loss[0][1] + 1.0).abs() < 1e-9);
        assert!((metadata.clip_fraction - 0.5).abs() < 1e-12);
    }

    #[test]
    fn gspo_uses_sequence_level_masked_ratio() {
        // Masked tokens: ratios from (logp - old) = [0.4, ignored, 0.0];
        // mean over included = 0.2; sequence ratio = e^0.2 ~ 1.2214; clip 0.1
        // -> clipped to 1.1.
        let (loss, metadata) = cs336_a5_policy_gradient_loss(
            &[1.0],
            &[vec![-0.6, -9.0, -1.0]],
            Cs336A5ImportanceReweighting::Gspo,
            Some(&[vec![-1.0, -1.0, -1.0]]),
            Some(0.1),
            Some(&[vec![1, 0, 1]]),
        )
        .expect("computes");
        for value in &loss[0] {
            assert!((value + 1.1).abs() < 1e-9);
        }
        assert!((metadata.clip_fraction - 1.0).abs() < 1e-12);
    }

    #[test]
    fn missing_arguments_refuse_with_typed_errors() {
        assert_eq!(
            cs336_a5_policy_gradient_loss(
                &[1.0],
                &[vec![-0.5]],
                Cs336A5ImportanceReweighting::Grpo,
                None,
                Some(0.2),
                None,
            )
            .expect_err("refuses"),
            Cs336A5ReferenceError::MissingArgument {
                method: "grpo",
                argument: "old_log_probs"
            }
        );
        assert_eq!(
            cs336_a5_policy_gradient_loss(
                &[1.0],
                &[vec![-0.5]],
                Cs336A5ImportanceReweighting::Grpo,
                Some(&[vec![-0.5]]),
                None,
                None,
            )
            .expect_err("refuses"),
            Cs336A5ReferenceError::MissingArgument {
                method: "grpo",
                argument: "cliprange"
            }
        );
    }

    #[test]
    fn aggregation_supports_sequence_and_constant_normalization() {
        let per_token = vec![vec![1.0, 2.0, 3.0], vec![4.0, 0.0, 0.0]];
        let mask = vec![vec![1, 1, 0], vec![1, 0, 0]];
        let sequence = cs336_a5_aggregate_loss_across_microbatch(
            &per_token,
            &mask,
            Cs336A5LossNormalization::Sequence,
            None,
        )
        .expect("aggregates");
        // Sequence means: 1.5 and 4.0 -> 2.75.
        assert!((sequence - 2.75).abs() < 1e-12);
        let constant = cs336_a5_aggregate_loss_across_microbatch(
            &per_token,
            &mask,
            Cs336A5LossNormalization::Constant,
            Some(7),
        )
        .expect("aggregates");
        assert!((constant - 1.0).abs() < 1e-12);
    }

    #[test]
    fn dpo_loss_matches_closed_form() {
        // Equal margins -> sigmoid(0) -> ln 2.
        let neutral = cs336_a5_per_instance_dpo_loss(-1.0, -1.0, -1.0, -1.0, 0.5);
        assert!((neutral - 2.0_f64.ln()).abs() < 1e-12);
        // Strong preference for chosen lowers the loss below ln 2.
        let preferred = cs336_a5_per_instance_dpo_loss(-0.5, -2.0, -1.0, -1.0, 1.0);
        assert!(preferred < neutral);
        // Symmetric dispreference raises it.
        let dispreferred = cs336_a5_per_instance_dpo_loss(-2.0, -0.5, -1.0, -1.0, 1.0);
        assert!(dispreferred > neutral);
    }

    #[test]
    fn rollout_reward_summary_averages_components() {
        let summary = cs336_a5_rollout_rewards(&[
            Cs336A5RewardComponents {
                reward: 1.0,
                format_reward: 1.0,
                answer_reward: 0.0,
            },
            Cs336A5RewardComponents {
                reward: 0.0,
                format_reward: 1.0,
                answer_reward: 0.0,
            },
        ])
        .expect("summarizes");
        assert_eq!(summary.raw_rewards, vec![1.0, 0.0]);
        assert!((summary.reward_mean - 0.5).abs() < 1e-12);
        assert!((summary.format_reward_mean - 1.0).abs() < 1e-12);
        assert!((summary.answer_reward_mean).abs() < 1e-12);
    }

    #[test]
    fn mmlu_parsing_prefers_the_explicit_pattern() {
        assert_eq!(
            cs336_a5_parse_mmlu_response("Reasoning... The correct answer is C."),
            Some('C')
        );
        assert_eq!(cs336_a5_parse_mmlu_response("Answer: B"), Some('B'));
        assert_eq!(cs336_a5_parse_mmlu_response("no letter here"), None);
    }

    #[test]
    fn gsm8k_parsing_returns_the_last_number() {
        assert_eq!(
            cs336_a5_parse_gsm8k_response("3 + 5 = 8. The answer is 8."),
            Some("8".to_string())
        );
        assert_eq!(
            cs336_a5_parse_gsm8k_response("Total cost: $1,234.50"),
            Some("1234.50".to_string())
        );
        assert_eq!(
            cs336_a5_parse_gsm8k_response("It is -42 degrees"),
            Some("-42".to_string())
        );
        assert_eq!(cs336_a5_parse_gsm8k_response("no numbers"), None);
    }

    #[test]
    fn sft_packing_chunks_the_concatenated_stream() {
        let blocks = cs336_a5_pack_sft_sequences(&[vec![1, 2, 3], vec![4, 5], vec![6, 7, 8, 9]], 4)
            .expect("packs");
        assert_eq!(blocks, vec![vec![1, 2, 3, 4], vec![5, 6, 7, 8]]);
    }
}
