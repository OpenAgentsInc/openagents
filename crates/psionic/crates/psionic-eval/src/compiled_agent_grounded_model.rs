use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{CompiledAgentPublicOutcomeKind, CompiledAgentRoute, CompiledAgentToolResult};

pub const COMPILED_AGENT_GROUNDED_ANSWER_MODEL_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.grounded_answer_model.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentGroundedAnswerProgram {
    ProviderReady,
    ProviderBlocked,
    WalletBalanceOnly,
    WalletBalanceWithRecentEarnings,
    UnsupportedRefusal,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentGroundedAnswerTrainingSample {
    pub sample_id: String,
    pub route: CompiledAgentRoute,
    pub tool_results: Vec<CompiledAgentToolResult>,
    pub expected_kind: CompiledAgentPublicOutcomeKind,
    pub expected_response: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentGroundedAnswerModelArtifact {
    pub schema_version: String,
    pub artifact_id: String,
    pub row_id: String,
    pub model_family: String,
    pub feature_profile: String,
    pub replay_bundle_digest: String,
    pub training_sample_count: u32,
    pub heldout_sample_count: u32,
    pub vocabulary_size: u32,
    pub smoothing_alpha: f64,
    pub class_priors_log: BTreeMap<CompiledAgentGroundedAnswerProgram, f64>,
    pub class_feature_log_probs:
        BTreeMap<CompiledAgentGroundedAnswerProgram, BTreeMap<String, f64>>,
    pub class_default_feature_log_probs: BTreeMap<CompiledAgentGroundedAnswerProgram, f64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub feature_idf_weights: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub program_centroid_weights:
        BTreeMap<CompiledAgentGroundedAnswerProgram, BTreeMap<String, f64>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub program_bias_scores: BTreeMap<CompiledAgentGroundedAnswerProgram, f64>,
    pub program_templates: BTreeMap<CompiledAgentGroundedAnswerProgram, String>,
    pub missing_facts_template: String,
    pub conflicting_facts_template: String,
    pub source_sample_ids: Vec<String>,
    pub heldout_sample_ids: Vec<String>,
    pub training_accuracy: f32,
    pub heldout_accuracy: f32,
    pub detail: String,
    pub artifact_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentGroundedAnswerPrediction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<CompiledAgentGroundedAnswerProgram>,
    pub outcome_kind: CompiledAgentPublicOutcomeKind,
    pub response: String,
    pub confidence: f32,
    pub score_margin: f32,
    pub active_features: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    pub program_scores: BTreeMap<CompiledAgentGroundedAnswerProgram, f64>,
}

#[derive(Clone, Debug, PartialEq)]
enum GroundedFactSignature {
    Provider {
        ready: bool,
        blockers: Vec<String>,
    },
    Wallet {
        balance_sats: u64,
        recent_earnings_sats: u64,
    },
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GroundedFactErrorKind {
    MissingFacts,
    ConflictingFacts,
}

#[must_use]
pub fn train_compiled_agent_grounded_answer_model(
    artifact_id: impl Into<String>,
    row_id: impl Into<String>,
    replay_bundle_digest: impl Into<String>,
    samples: &[CompiledAgentGroundedAnswerTrainingSample],
    heldout_samples: &[CompiledAgentGroundedAnswerTrainingSample],
) -> CompiledAgentGroundedAnswerModelArtifact {
    let artifact_id = artifact_id.into();
    let row_id = row_id.into();
    let replay_bundle_digest = replay_bundle_digest.into();
    let smoothing_alpha = 1.0_f64;

    let mut vocabulary = BTreeSet::new();
    let mut class_counts = BTreeMap::<CompiledAgentGroundedAnswerProgram, u32>::new();
    let mut feature_counts =
        BTreeMap::<CompiledAgentGroundedAnswerProgram, BTreeMap<String, u32>>::new();
    let mut total_feature_counts = BTreeMap::<CompiledAgentGroundedAnswerProgram, u32>::new();
    let mut template_votes =
        BTreeMap::<CompiledAgentGroundedAnswerProgram, BTreeMap<String, u32>>::new();

    for sample in samples {
        let program = program_for_training_sample(sample);
        *class_counts.entry(program).or_default() += 1;
        let features = grounded_features(sample.route, &sample.tool_results).unwrap_or_default();
        for feature in features {
            vocabulary.insert(feature.clone());
            *feature_counts
                .entry(program)
                .or_default()
                .entry(feature)
                .or_default() += 1;
            *total_feature_counts.entry(program).or_default() += 1;
        }
        let template = canonicalize_template(
            sample.route,
            &sample.tool_results,
            sample.expected_response.as_str(),
        );
        *template_votes
            .entry(program)
            .or_default()
            .entry(template)
            .or_default() += 1;
    }

    let total_samples = samples.len().max(1) as f64;
    let vocab_size = vocabulary.len().max(1) as f64;
    let mut class_priors_log = BTreeMap::new();
    let mut class_feature_log_probs = BTreeMap::new();
    let mut class_default_feature_log_probs = BTreeMap::new();
    let mut program_templates = BTreeMap::new();

    for program in class_counts.keys().copied().collect::<Vec<_>>() {
        let class_sample_count = (*class_counts.get(&program).unwrap_or(&0)).max(1) as f64;
        class_priors_log.insert(program, (class_sample_count / total_samples).ln());
        let class_total = *total_feature_counts.get(&program).unwrap_or(&0) as f64;
        let denominator = class_total + smoothing_alpha * vocab_size;
        class_default_feature_log_probs.insert(program, (smoothing_alpha / denominator).ln());

        let class_feature_counts = feature_counts.get(&program).cloned().unwrap_or_default();
        let mut feature_log_probs = BTreeMap::new();
        for feature in &vocabulary {
            let count = *class_feature_counts.get(feature).unwrap_or(&0) as f64;
            feature_log_probs.insert(
                feature.clone(),
                ((count + smoothing_alpha) / denominator).ln(),
            );
        }
        class_feature_log_probs.insert(program, feature_log_probs);

        let learned_template = template_votes
            .get(&program)
            .and_then(|votes| {
                votes.iter().max_by(|left, right| {
                    left.1
                        .cmp(right.1)
                        .then_with(|| left.0.cmp(right.0).reverse())
                })
            })
            .map(|(template, _)| template.clone())
            .unwrap_or_else(|| default_template(program));
        program_templates.insert(program, learned_template);
    }

    let mut artifact = CompiledAgentGroundedAnswerModelArtifact {
        schema_version: String::from(COMPILED_AGENT_GROUNDED_ANSWER_MODEL_SCHEMA_VERSION),
        artifact_id,
        row_id,
        model_family: String::from("multinomial_naive_bayes"),
        feature_profile: String::from("route_plus_fact_signature"),
        replay_bundle_digest,
        training_sample_count: samples.len() as u32,
        heldout_sample_count: heldout_samples.len() as u32,
        vocabulary_size: vocabulary.len() as u32,
        smoothing_alpha,
        class_priors_log,
        class_feature_log_probs,
        class_default_feature_log_probs,
        feature_idf_weights: BTreeMap::new(),
        program_centroid_weights: BTreeMap::new(),
        program_bias_scores: BTreeMap::new(),
        program_templates,
        missing_facts_template: String::from("Grounded facts were unavailable."),
        conflicting_facts_template: String::from("Grounded facts were conflicting."),
        source_sample_ids: samples.iter().map(|sample| sample.sample_id.clone()).collect(),
        heldout_sample_ids: heldout_samples
            .iter()
            .map(|sample| sample.sample_id.clone())
            .collect(),
        training_accuracy: 0.0,
        heldout_accuracy: 0.0,
        detail: String::from(
            "Trained grounded-answer model over explicit fact signatures only. The model predicts a narrow grounded-answer program from supplied tool facts and falls back on missing or conflicting facts instead of inventing unsupported synthesis.",
        ),
        artifact_digest: String::new(),
    };
    artifact.training_accuracy = grounded_training_accuracy(&artifact, samples);
    artifact.heldout_accuracy = grounded_training_accuracy(&artifact, heldout_samples);
    normalize_grounded_answer_model_artifact(artifact)
}

#[must_use]
pub fn train_compiled_agent_grounded_answer_tfidf_centroid_model(
    artifact_id: impl Into<String>,
    row_id: impl Into<String>,
    replay_bundle_digest: impl Into<String>,
    samples: &[CompiledAgentGroundedAnswerTrainingSample],
    heldout_samples: &[CompiledAgentGroundedAnswerTrainingSample],
) -> CompiledAgentGroundedAnswerModelArtifact {
    let artifact_id = artifact_id.into();
    let row_id = row_id.into();
    let replay_bundle_digest = replay_bundle_digest.into();

    let mut vocabulary = BTreeSet::new();
    let mut document_frequency = BTreeMap::<String, u32>::new();
    let mut class_counts = BTreeMap::<CompiledAgentGroundedAnswerProgram, u32>::new();
    let mut centroid_accumulators =
        BTreeMap::<CompiledAgentGroundedAnswerProgram, BTreeMap<String, f64>>::new();
    let mut template_votes =
        BTreeMap::<CompiledAgentGroundedAnswerProgram, BTreeMap<String, u32>>::new();

    let training_feature_counts = samples
        .iter()
        .map(|sample| {
            let program = program_for_training_sample(sample);
            *class_counts.entry(program).or_default() += 1;
            let counts = feature_count_map(
                &grounded_features(sample.route, &sample.tool_results).unwrap_or_default(),
            );
            let unique_features = counts.keys().cloned().collect::<BTreeSet<_>>();
            for feature in unique_features {
                *document_frequency.entry(feature.clone()).or_default() += 1;
                vocabulary.insert(feature);
            }
            let template = canonicalize_template(
                sample.route,
                &sample.tool_results,
                sample.expected_response.as_str(),
            );
            *template_votes
                .entry(program)
                .or_default()
                .entry(template)
                .or_default() += 1;
            (program, counts)
        })
        .collect::<Vec<_>>();

    let sample_count = samples.len().max(1) as f64;
    let feature_idf_weights = vocabulary
        .iter()
        .map(|feature| {
            let document_count = f64::from(*document_frequency.get(feature).unwrap_or(&0));
            (
                feature.clone(),
                ((1.0 + sample_count) / (1.0 + document_count)).ln() + 1.0,
            )
        })
        .collect::<BTreeMap<_, _>>();

    for (program, feature_counts) in &training_feature_counts {
        let normalized = normalized_tfidf_vector(feature_counts, &feature_idf_weights);
        let centroid = centroid_accumulators.entry(*program).or_default();
        for (feature, weight) in normalized {
            *centroid.entry(feature).or_default() += weight;
        }
    }

    let mut class_priors_log = BTreeMap::new();
    let mut program_centroid_weights = BTreeMap::new();
    let mut program_bias_scores = BTreeMap::new();
    let mut program_templates = BTreeMap::new();
    for program in class_counts.keys().copied().collect::<Vec<_>>() {
        let class_count = f64::from(*class_counts.get(&program).unwrap_or(&0)).max(1.0);
        class_priors_log.insert(program, (class_count / sample_count).ln());
        program_bias_scores.insert(program, (class_count / sample_count).ln() * 0.1);

        let mut centroid = centroid_accumulators.remove(&program).unwrap_or_default();
        for value in centroid.values_mut() {
            *value /= class_count;
        }
        program_centroid_weights.insert(program, normalize_weight_vector(centroid));

        let learned_template = template_votes
            .get(&program)
            .and_then(|votes| {
                votes.iter().max_by(|left, right| {
                    left.1
                        .cmp(right.1)
                        .then_with(|| left.0.cmp(right.0).reverse())
                })
            })
            .map(|(template, _)| template.clone())
            .unwrap_or_else(|| default_template(program));
        program_templates.insert(program, learned_template);
    }

    let mut artifact = CompiledAgentGroundedAnswerModelArtifact {
        schema_version: String::from(COMPILED_AGENT_GROUNDED_ANSWER_MODEL_SCHEMA_VERSION),
        artifact_id,
        row_id,
        model_family: String::from("tfidf_centroid"),
        feature_profile: String::from("normalized_tfidf_route_plus_fact_signature"),
        replay_bundle_digest,
        training_sample_count: samples.len() as u32,
        heldout_sample_count: heldout_samples.len() as u32,
        vocabulary_size: vocabulary.len() as u32,
        smoothing_alpha: 0.0,
        class_priors_log,
        class_feature_log_probs: BTreeMap::new(),
        class_default_feature_log_probs: BTreeMap::new(),
        feature_idf_weights,
        program_centroid_weights,
        program_bias_scores,
        program_templates,
        missing_facts_template: String::from("Grounded facts were unavailable."),
        conflicting_facts_template: String::from("Grounded facts were conflicting."),
        source_sample_ids: samples.iter().map(|sample| sample.sample_id.clone()).collect(),
        heldout_sample_ids: heldout_samples
            .iter()
            .map(|sample| sample.sample_id.clone())
            .collect(),
        training_accuracy: 0.0,
        heldout_accuracy: 0.0,
        detail: String::from(
            "Trained a length-normalized TF-IDF centroid grounded-answer model over the same strict fact-signature inputs. This tests a stronger bounded candidate family without widening the grounded-answer contract.",
        ),
        artifact_digest: String::new(),
    };
    artifact.training_accuracy = grounded_training_accuracy(&artifact, samples);
    artifact.heldout_accuracy = grounded_training_accuracy(&artifact, heldout_samples);
    normalize_grounded_answer_model_artifact(artifact)
}

#[must_use]
pub fn predict_compiled_agent_grounded_answer(
    artifact: &CompiledAgentGroundedAnswerModelArtifact,
    route: CompiledAgentRoute,
    tool_results: &[CompiledAgentToolResult],
) -> CompiledAgentGroundedAnswerPrediction {
    let signature = match grounded_fact_signature(route, tool_results) {
        Ok(signature) => signature,
        Err(GroundedFactErrorKind::MissingFacts) => {
            return CompiledAgentGroundedAnswerPrediction {
                program: None,
                outcome_kind: CompiledAgentPublicOutcomeKind::ConfidenceFallback,
                response: artifact.missing_facts_template.clone(),
                confidence: 0.0,
                score_margin: 0.0,
                active_features: Vec::new(),
                fallback_reason: Some(String::from("missing_facts")),
                program_scores: BTreeMap::new(),
            };
        }
        Err(GroundedFactErrorKind::ConflictingFacts) => {
            return CompiledAgentGroundedAnswerPrediction {
                program: None,
                outcome_kind: CompiledAgentPublicOutcomeKind::ConfidenceFallback,
                response: artifact.conflicting_facts_template.clone(),
                confidence: 0.0,
                score_margin: 0.0,
                active_features: Vec::new(),
                fallback_reason: Some(String::from("conflicting_facts")),
                program_scores: BTreeMap::new(),
            };
        }
    };

    let features = grounded_features(route, tool_results).unwrap_or_default();
    let feature_counts = feature_count_map(&features);
    let program_scores = match artifact.model_family.as_str() {
        "tfidf_centroid" => grounded_tfidf_scores(artifact, &feature_counts),
        _ => grounded_multinomial_scores(artifact, &feature_counts),
    };

    let mut ranked = program_scores.iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.total_cmp(left.1));
    let program = ranked.first().map(|(program, _)| **program);
    let best_score = ranked.first().map(|(_, score)| **score).unwrap_or(0.0);
    let second_score = ranked
        .get(1)
        .map(|(_, score)| **score)
        .unwrap_or(best_score);
    let max_score = program_scores
        .values()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let exp_scores = program_scores
        .values()
        .map(|score| (score - max_score).exp())
        .collect::<Vec<_>>();
    let exp_sum = exp_scores.iter().sum::<f64>().max(f64::EPSILON);
    let confidence = ((best_score - max_score).exp() / exp_sum) as f32;

    let (outcome_kind, response) = match program {
        Some(program) => render_prediction(artifact, program, &signature),
        None => (
            CompiledAgentPublicOutcomeKind::ConfidenceFallback,
            artifact.missing_facts_template.clone(),
        ),
    };

    CompiledAgentGroundedAnswerPrediction {
        program,
        outcome_kind,
        response,
        confidence,
        score_margin: (best_score - second_score) as f32,
        active_features: features,
        fallback_reason: None,
        program_scores,
    }
}

fn render_prediction(
    artifact: &CompiledAgentGroundedAnswerModelArtifact,
    program: CompiledAgentGroundedAnswerProgram,
    signature: &GroundedFactSignature,
) -> (CompiledAgentPublicOutcomeKind, String) {
    match (program, signature) {
        (
            CompiledAgentGroundedAnswerProgram::ProviderReady,
            GroundedFactSignature::Provider { .. },
        )
        | (
            CompiledAgentGroundedAnswerProgram::ProviderBlocked,
            GroundedFactSignature::Provider { .. },
        )
        | (
            CompiledAgentGroundedAnswerProgram::UnsupportedRefusal,
            GroundedFactSignature::Unsupported,
        ) => (
            match program {
                CompiledAgentGroundedAnswerProgram::UnsupportedRefusal => {
                    CompiledAgentPublicOutcomeKind::UnsupportedRefusal
                }
                _ => CompiledAgentPublicOutcomeKind::GroundedAnswer,
            },
            artifact
                .program_templates
                .get(&program)
                .cloned()
                .unwrap_or_else(|| default_template(program)),
        ),
        (
            CompiledAgentGroundedAnswerProgram::WalletBalanceOnly,
            GroundedFactSignature::Wallet {
                balance_sats,
                recent_earnings_sats: _,
            },
        ) => (
            CompiledAgentPublicOutcomeKind::GroundedAnswer,
            artifact
                .program_templates
                .get(&program)
                .cloned()
                .unwrap_or_else(|| default_template(program))
                .replace("{balance_sats}", &balance_sats.to_string()),
        ),
        (
            CompiledAgentGroundedAnswerProgram::WalletBalanceWithRecentEarnings,
            GroundedFactSignature::Wallet {
                balance_sats,
                recent_earnings_sats,
            },
        ) => (
            CompiledAgentPublicOutcomeKind::GroundedAnswer,
            artifact
                .program_templates
                .get(&program)
                .cloned()
                .unwrap_or_else(|| default_template(program))
                .replace("{balance_sats}", &balance_sats.to_string())
                .replace("{recent_earnings_sats}", &recent_earnings_sats.to_string()),
        ),
        _ => (
            CompiledAgentPublicOutcomeKind::ConfidenceFallback,
            artifact.conflicting_facts_template.clone(),
        ),
    }
}

fn grounded_training_accuracy(
    artifact: &CompiledAgentGroundedAnswerModelArtifact,
    samples: &[CompiledAgentGroundedAnswerTrainingSample],
) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let correct = samples
        .iter()
        .filter(|sample| {
            let prediction = predict_compiled_agent_grounded_answer(
                artifact,
                sample.route,
                &sample.tool_results,
            );
            prediction.outcome_kind == sample.expected_kind
                && prediction.response == sample.expected_response
        })
        .count();
    correct as f32 / samples.len() as f32
}

fn feature_count_map(features: &[String]) -> BTreeMap<String, u32> {
    let mut feature_counts = BTreeMap::<String, u32>::new();
    for feature in features {
        *feature_counts.entry(feature.clone()).or_default() += 1;
    }
    feature_counts
}

fn grounded_multinomial_scores(
    artifact: &CompiledAgentGroundedAnswerModelArtifact,
    feature_counts: &BTreeMap<String, u32>,
) -> BTreeMap<CompiledAgentGroundedAnswerProgram, f64> {
    let mut program_scores = BTreeMap::new();
    for program in artifact.class_priors_log.keys().copied() {
        let mut score = *artifact
            .class_priors_log
            .get(&program)
            .unwrap_or(&f64::NEG_INFINITY);
        let default_log_prob = *artifact
            .class_default_feature_log_probs
            .get(&program)
            .unwrap_or(&f64::NEG_INFINITY);
        let class_feature_log_probs = artifact.class_feature_log_probs.get(&program);
        for (feature, count) in feature_counts {
            let log_prob = class_feature_log_probs
                .and_then(|weights| weights.get(feature))
                .copied()
                .unwrap_or(default_log_prob);
            score += f64::from(*count) * log_prob;
        }
        program_scores.insert(program, score);
    }
    program_scores
}

fn grounded_tfidf_scores(
    artifact: &CompiledAgentGroundedAnswerModelArtifact,
    feature_counts: &BTreeMap<String, u32>,
) -> BTreeMap<CompiledAgentGroundedAnswerProgram, f64> {
    let normalized = normalized_tfidf_vector(feature_counts, &artifact.feature_idf_weights);
    let mut program_scores = BTreeMap::new();
    for program in artifact.class_priors_log.keys().copied() {
        let centroid = artifact.program_centroid_weights.get(&program);
        let similarity = normalized.iter().fold(0.0, |score, (feature, weight)| {
            score
                + weight
                    * centroid
                        .and_then(|weights| weights.get(feature))
                        .copied()
                        .unwrap_or_default()
        });
        let bias = artifact
            .program_bias_scores
            .get(&program)
            .copied()
            .unwrap_or_default();
        program_scores.insert(program, similarity + bias);
    }
    program_scores
}

fn normalized_tfidf_vector(
    feature_counts: &BTreeMap<String, u32>,
    idf_weights: &BTreeMap<String, f64>,
) -> BTreeMap<String, f64> {
    let mut weighted = BTreeMap::new();
    for (feature, count) in feature_counts {
        let idf = idf_weights.get(feature).copied().unwrap_or(1.0);
        weighted.insert(feature.clone(), f64::from(*count) * idf);
    }
    normalize_weight_vector(weighted)
}

fn normalize_weight_vector(weights: BTreeMap<String, f64>) -> BTreeMap<String, f64> {
    let norm = weights
        .values()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt()
        .max(f64::EPSILON);
    weights
        .into_iter()
        .map(|(feature, value)| (feature, value / norm))
        .collect()
}

fn grounded_features(
    route: CompiledAgentRoute,
    tool_results: &[CompiledAgentToolResult],
) -> Result<Vec<String>, GroundedFactErrorKind> {
    let signature = grounded_fact_signature(route, tool_results)?;
    Ok(match signature {
        GroundedFactSignature::Provider {
            ready,
            ref blockers,
        } => vec![
            String::from("route:provider_status"),
            format!("provider_ready:{ready}"),
            format!("provider_blocker_count:{}", blockers.len()),
            format!("provider_blockers_present:{}", !blockers.is_empty()),
        ],
        GroundedFactSignature::Wallet {
            balance_sats,
            recent_earnings_sats,
        } => vec![
            String::from("route:wallet_status"),
            String::from("wallet_balance_present:true"),
            String::from("wallet_recent_earnings_present:true"),
            format!("wallet_balance_zero:{}", balance_sats == 0),
            format!("wallet_recent_earnings_zero:{}", recent_earnings_sats == 0),
        ],
        GroundedFactSignature::Unsupported => vec![String::from("route:unsupported")],
    })
}

fn grounded_fact_signature(
    route: CompiledAgentRoute,
    tool_results: &[CompiledAgentToolResult],
) -> Result<GroundedFactSignature, GroundedFactErrorKind> {
    match route {
        CompiledAgentRoute::ProviderStatus => {
            let provider_tools = tool_results
                .iter()
                .filter(|tool| tool.tool_name == "provider_status")
                .collect::<Vec<_>>();
            if provider_tools.is_empty() {
                return Err(GroundedFactErrorKind::MissingFacts);
            }
            let ready_values = provider_tools
                .iter()
                .map(|tool| tool.payload.get("ready").and_then(Value::as_bool))
                .collect::<Vec<_>>();
            if ready_values.iter().any(Option::is_none) {
                return Err(GroundedFactErrorKind::MissingFacts);
            }
            let ready = ready_values[0].unwrap_or(false);
            if ready_values
                .iter()
                .any(|value| value.unwrap_or(ready) != ready)
            {
                return Err(GroundedFactErrorKind::ConflictingFacts);
            }
            let blocker_sets = provider_tools
                .iter()
                .map(|tool| blockers_from_payload(&tool.payload))
                .collect::<Vec<_>>();
            let blockers = blocker_sets[0].clone();
            if blocker_sets.iter().any(|candidate| candidate != &blockers) {
                return Err(GroundedFactErrorKind::ConflictingFacts);
            }
            Ok(GroundedFactSignature::Provider { ready, blockers })
        }
        CompiledAgentRoute::WalletStatus => {
            let wallet_tools = tool_results
                .iter()
                .filter(|tool| tool.tool_name == "wallet_status")
                .collect::<Vec<_>>();
            if wallet_tools.is_empty() {
                return Err(GroundedFactErrorKind::MissingFacts);
            }
            let balances = wallet_tools
                .iter()
                .map(|tool| tool.payload.get("balance_sats").and_then(Value::as_u64))
                .collect::<Vec<_>>();
            let earnings = wallet_tools
                .iter()
                .map(|tool| {
                    tool.payload
                        .get("recent_earnings_sats")
                        .and_then(Value::as_u64)
                })
                .collect::<Vec<_>>();
            if balances.iter().any(Option::is_none) || earnings.iter().any(Option::is_none) {
                return Err(GroundedFactErrorKind::MissingFacts);
            }
            let balance_sats = balances[0].unwrap_or(0);
            let recent_earnings_sats = earnings[0].unwrap_or(0);
            if balances
                .iter()
                .any(|value| value.unwrap_or(balance_sats) != balance_sats)
                || earnings
                    .iter()
                    .any(|value| value.unwrap_or(recent_earnings_sats) != recent_earnings_sats)
            {
                return Err(GroundedFactErrorKind::ConflictingFacts);
            }
            Ok(GroundedFactSignature::Wallet {
                balance_sats,
                recent_earnings_sats,
            })
        }
        CompiledAgentRoute::Unsupported => Ok(GroundedFactSignature::Unsupported),
    }
}

fn blockers_from_payload(payload: &Value) -> Vec<String> {
    payload
        .get("blockers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(ToOwned::to_owned))
        .collect()
}

fn program_for_training_sample(
    sample: &CompiledAgentGroundedAnswerTrainingSample,
) -> CompiledAgentGroundedAnswerProgram {
    match sample.route {
        CompiledAgentRoute::Unsupported => CompiledAgentGroundedAnswerProgram::UnsupportedRefusal,
        CompiledAgentRoute::ProviderStatus => {
            if sample
                .expected_response
                .to_ascii_lowercase()
                .contains("not ready")
            {
                CompiledAgentGroundedAnswerProgram::ProviderBlocked
            } else {
                CompiledAgentGroundedAnswerProgram::ProviderReady
            }
        }
        CompiledAgentRoute::WalletStatus => {
            if sample
                .expected_response
                .to_ascii_lowercase()
                .contains("recent earnings")
            {
                CompiledAgentGroundedAnswerProgram::WalletBalanceWithRecentEarnings
            } else {
                CompiledAgentGroundedAnswerProgram::WalletBalanceOnly
            }
        }
    }
}

fn canonicalize_template(
    route: CompiledAgentRoute,
    tool_results: &[CompiledAgentToolResult],
    response: &str,
) -> String {
    match grounded_fact_signature(route, tool_results) {
        Ok(GroundedFactSignature::Wallet {
            balance_sats,
            recent_earnings_sats,
        }) => response
            .replace(&balance_sats.to_string(), "{balance_sats}")
            .replace(&recent_earnings_sats.to_string(), "{recent_earnings_sats}"),
        _ => response.to_string(),
    }
}

fn default_template(program: CompiledAgentGroundedAnswerProgram) -> String {
    match program {
        CompiledAgentGroundedAnswerProgram::ProviderReady => {
            String::from("Provider is ready to go online.")
        }
        CompiledAgentGroundedAnswerProgram::ProviderBlocked => {
            String::from("Provider is not ready to go online.")
        }
        CompiledAgentGroundedAnswerProgram::WalletBalanceOnly => {
            String::from("Wallet balance is {balance_sats} sats.")
        }
        CompiledAgentGroundedAnswerProgram::WalletBalanceWithRecentEarnings => String::from(
            "Wallet balance is {balance_sats} sats, with {recent_earnings_sats} sats of recent earnings.",
        ),
        CompiledAgentGroundedAnswerProgram::UnsupportedRefusal => String::from(
            "I can currently answer only provider readiness and wallet balance questions.",
        ),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn normalize_grounded_answer_model_artifact(
    artifact: CompiledAgentGroundedAnswerModelArtifact,
) -> CompiledAgentGroundedAnswerModelArtifact {
    let bytes = serde_json::to_vec(&artifact).unwrap_or_default();
    let mut normalized: CompiledAgentGroundedAnswerModelArtifact =
        serde_json::from_slice(&bytes).unwrap_or(artifact);
    normalized.artifact_digest = String::new();
    normalized.artifact_digest = stable_digest(
        b"compiled_agent_grounded_answer_model_artifact|",
        &normalized,
    );
    normalized
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        predict_compiled_agent_grounded_answer, train_compiled_agent_grounded_answer_model,
        train_compiled_agent_grounded_answer_tfidf_centroid_model,
        CompiledAgentGroundedAnswerTrainingSample,
    };
    use crate::{CompiledAgentPublicOutcomeKind, CompiledAgentRoute, CompiledAgentToolResult};

    fn sample(
        sample_id: &str,
        route: CompiledAgentRoute,
        tool_results: Vec<CompiledAgentToolResult>,
        kind: CompiledAgentPublicOutcomeKind,
        response: &str,
    ) -> CompiledAgentGroundedAnswerTrainingSample {
        CompiledAgentGroundedAnswerTrainingSample {
            sample_id: sample_id.to_string(),
            route,
            tool_results,
            expected_kind: kind,
            expected_response: response.to_string(),
            tags: Vec::new(),
        }
    }

    #[test]
    fn compiled_agent_grounded_model_memorizes_the_small_training_bundle() {
        let samples = vec![
            sample(
                "provider_ready",
                CompiledAgentRoute::ProviderStatus,
                vec![CompiledAgentToolResult {
                    tool_name: String::from("provider_status"),
                    payload: json!({"ready": true, "blockers": []}),
                }],
                CompiledAgentPublicOutcomeKind::GroundedAnswer,
                "Provider is ready to go online.",
            ),
            sample(
                "wallet_recent_earnings",
                CompiledAgentRoute::WalletStatus,
                vec![CompiledAgentToolResult {
                    tool_name: String::from("wallet_status"),
                    payload: json!({"balance_sats": 1200, "recent_earnings_sats": 240}),
                }],
                CompiledAgentPublicOutcomeKind::GroundedAnswer,
                "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
            ),
            sample(
                "unsupported",
                CompiledAgentRoute::Unsupported,
                Vec::new(),
                CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                "I can currently answer only provider readiness and wallet balance questions.",
            ),
        ];
        let artifact = train_compiled_agent_grounded_answer_model(
            "compiled_agent.grounded_answer.multinomial_nb_v1",
            "compiled_agent.test_row.v1",
            "bundle",
            &samples,
            &[],
        );
        assert_eq!(artifact.training_accuracy, 1.0);
        assert_eq!(artifact.heldout_sample_count, 0);
    }

    #[test]
    fn compiled_agent_grounded_model_falls_back_on_missing_wallet_facts() {
        let samples = vec![sample(
            "wallet_recent_earnings",
            CompiledAgentRoute::WalletStatus,
            vec![CompiledAgentToolResult {
                tool_name: String::from("wallet_status"),
                payload: json!({"balance_sats": 1200, "recent_earnings_sats": 240}),
            }],
            CompiledAgentPublicOutcomeKind::GroundedAnswer,
            "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
        )];
        let artifact = train_compiled_agent_grounded_answer_model(
            "compiled_agent.grounded_answer.multinomial_nb_v1",
            "compiled_agent.test_row.v1",
            "bundle",
            &samples,
            &[],
        );
        let prediction = predict_compiled_agent_grounded_answer(
            &artifact,
            CompiledAgentRoute::WalletStatus,
            &[CompiledAgentToolResult {
                tool_name: String::from("wallet_status"),
                payload: json!({"balance_sats": 1200}),
            }],
        );
        assert_eq!(
            prediction.outcome_kind,
            CompiledAgentPublicOutcomeKind::ConfidenceFallback
        );
        assert!(prediction.response.contains("unavailable"));
    }

    #[test]
    fn compiled_agent_grounded_model_falls_back_on_conflicting_wallet_facts() {
        let samples = vec![sample(
            "wallet_recent_earnings",
            CompiledAgentRoute::WalletStatus,
            vec![CompiledAgentToolResult {
                tool_name: String::from("wallet_status"),
                payload: json!({"balance_sats": 1200, "recent_earnings_sats": 240}),
            }],
            CompiledAgentPublicOutcomeKind::GroundedAnswer,
            "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
        )];
        let artifact = train_compiled_agent_grounded_answer_model(
            "compiled_agent.grounded_answer.multinomial_nb_v1",
            "compiled_agent.test_row.v1",
            "bundle",
            &samples,
            &[],
        );
        let prediction = predict_compiled_agent_grounded_answer(
            &artifact,
            CompiledAgentRoute::WalletStatus,
            &[
                CompiledAgentToolResult {
                    tool_name: String::from("wallet_status"),
                    payload: json!({"balance_sats": 1200, "recent_earnings_sats": 240}),
                },
                CompiledAgentToolResult {
                    tool_name: String::from("wallet_status"),
                    payload: json!({"balance_sats": 1500, "recent_earnings_sats": 240}),
                },
            ],
        );
        assert_eq!(
            prediction.outcome_kind,
            CompiledAgentPublicOutcomeKind::ConfidenceFallback
        );
        assert!(prediction.response.contains("conflicting"));
    }

    #[test]
    fn compiled_agent_tfidf_grounded_model_retains_supported_wallet_answer() {
        let samples = vec![sample(
            "wallet_recent_earnings",
            CompiledAgentRoute::WalletStatus,
            vec![CompiledAgentToolResult {
                tool_name: String::from("wallet_status"),
                payload: json!({"balance_sats": 1200, "recent_earnings_sats": 240}),
            }],
            CompiledAgentPublicOutcomeKind::GroundedAnswer,
            "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
        )];
        let artifact = train_compiled_agent_grounded_answer_tfidf_centroid_model(
            "compiled_agent.grounded_answer.tfidf_centroid_v1",
            "compiled_agent.test_row.v1",
            "bundle",
            &samples,
            &[],
        );
        let prediction = predict_compiled_agent_grounded_answer(
            &artifact,
            CompiledAgentRoute::WalletStatus,
            &[CompiledAgentToolResult {
                tool_name: String::from("wallet_status"),
                payload: json!({"balance_sats": 1200, "recent_earnings_sats": 240}),
            }],
        );
        assert_eq!(artifact.model_family, "tfidf_centroid");
        assert_eq!(
            prediction.response,
            "Wallet balance is 1200 sats, with 240 sats of recent earnings."
        );
    }
}
