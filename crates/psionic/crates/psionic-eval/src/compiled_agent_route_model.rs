use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::CompiledAgentRoute;

pub const COMPILED_AGENT_ROUTE_MODEL_SCHEMA_VERSION: &str = "psionic.compiled_agent.route_model.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentRouteTrainingSample {
    pub sample_id: String,
    pub user_request: String,
    pub expected_route: CompiledAgentRoute,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentRouteModelArtifact {
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
    pub class_priors_log: BTreeMap<CompiledAgentRoute, f64>,
    pub class_feature_log_probs: BTreeMap<CompiledAgentRoute, BTreeMap<String, f64>>,
    pub class_default_feature_log_probs: BTreeMap<CompiledAgentRoute, f64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub feature_idf_weights: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub class_centroid_weights: BTreeMap<CompiledAgentRoute, BTreeMap<String, f64>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub class_bias_scores: BTreeMap<CompiledAgentRoute, f64>,
    pub source_sample_ids: Vec<String>,
    pub heldout_sample_ids: Vec<String>,
    pub training_accuracy: f32,
    pub heldout_accuracy: f32,
    pub detail: String,
    pub artifact_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentRoutePrediction {
    pub route: CompiledAgentRoute,
    pub confidence: f32,
    pub score_margin: f32,
    pub active_features: Vec<String>,
    pub route_scores: BTreeMap<CompiledAgentRoute, f64>,
}

#[must_use]
pub fn compiled_agent_route_features(prompt: &str) -> Vec<String> {
    let tokens = normalized_tokens(prompt);
    let mut features = tokens
        .iter()
        .map(|token| format!("token:{token}"))
        .collect::<Vec<_>>();
    for pair in tokens.windows(2) {
        features.push(format!("bigram:{}__{}", pair[0], pair[1]));
    }
    for triple in tokens.windows(3) {
        features.push(format!("trigram:{}__{}__{}", triple[0], triple[1], triple[2]));
    }
    append_route_marker_features(&tokens, &mut features);
    features
}

#[must_use]
pub fn train_compiled_agent_route_model(
    artifact_id: impl Into<String>,
    row_id: impl Into<String>,
    replay_bundle_digest: impl Into<String>,
    samples: &[CompiledAgentRouteTrainingSample],
    heldout_samples: &[CompiledAgentRouteTrainingSample],
) -> CompiledAgentRouteModelArtifact {
    let smoothing_alpha = 1.0_f64;
    let artifact_id = artifact_id.into();
    let row_id = row_id.into();
    let replay_bundle_digest = replay_bundle_digest.into();
    let classes = [
        CompiledAgentRoute::ProviderStatus,
        CompiledAgentRoute::WalletStatus,
        CompiledAgentRoute::Unsupported,
    ];

    let mut vocabulary = BTreeSet::new();
    let mut class_counts = BTreeMap::new();
    let mut feature_counts = BTreeMap::new();
    let mut total_feature_counts = BTreeMap::new();

    for class in classes {
        class_counts.insert(class, 0_u32);
        feature_counts.insert(class, BTreeMap::<String, u32>::new());
        total_feature_counts.insert(class, 0_u32);
    }

    for sample in samples {
        *class_counts.entry(sample.expected_route).or_default() += 1;
        let features = compiled_agent_route_features(sample.user_request.as_str());
        for feature in features {
            vocabulary.insert(feature.clone());
            *feature_counts
                .entry(sample.expected_route)
                .or_default()
                .entry(feature)
                .or_default() += 1;
            *total_feature_counts
                .entry(sample.expected_route)
                .or_default() += 1;
        }
    }

    let vocabulary = vocabulary.into_iter().collect::<Vec<_>>();
    let total_samples = samples.len().max(1) as f64;
    let vocab_size = vocabulary.len().max(1) as f64;

    let mut class_priors_log = BTreeMap::new();
    let mut class_feature_log_probs = BTreeMap::new();
    let mut class_default_feature_log_probs = BTreeMap::new();

    for class in classes {
        let class_sample_count = (*class_counts.get(&class).unwrap_or(&0)).max(1) as f64;
        class_priors_log.insert(class, (class_sample_count / total_samples).ln());
        let class_total = *total_feature_counts.get(&class).unwrap_or(&0) as f64;
        let denominator = class_total + smoothing_alpha * vocab_size;
        class_default_feature_log_probs.insert(class, (smoothing_alpha / denominator).ln());
        let class_feature_counts = feature_counts.get(&class).cloned().unwrap_or_default();
        let mut feature_log_probs = BTreeMap::new();
        for feature in &vocabulary {
            let count = *class_feature_counts.get(feature).unwrap_or(&0) as f64;
            feature_log_probs.insert(
                feature.clone(),
                ((count + smoothing_alpha) / denominator).ln(),
            );
        }
        class_feature_log_probs.insert(class, feature_log_probs);
    }

    let mut artifact = CompiledAgentRouteModelArtifact {
        schema_version: String::from(COMPILED_AGENT_ROUTE_MODEL_SCHEMA_VERSION),
        artifact_id,
        row_id,
        model_family: String::from("multinomial_naive_bayes"),
        feature_profile: String::from("unigram_bigram_trigram_plus_route_markers"),
        replay_bundle_digest,
        training_sample_count: samples.len() as u32,
        heldout_sample_count: heldout_samples.len() as u32,
        vocabulary_size: vocabulary.len() as u32,
        smoothing_alpha,
        class_priors_log,
        class_feature_log_probs,
        class_default_feature_log_probs,
        feature_idf_weights: BTreeMap::new(),
        class_centroid_weights: BTreeMap::new(),
        class_bias_scores: BTreeMap::new(),
        source_sample_ids: samples
            .iter()
            .map(|sample| sample.sample_id.clone())
            .collect(),
        heldout_sample_ids: heldout_samples
            .iter()
            .map(|sample| sample.sample_id.clone())
            .collect(),
        training_accuracy: 0.0,
        heldout_accuracy: 0.0,
        detail: String::from(
            "Trained route model over replay-backed route samples using bag-of-token and bigram features. This is the first compiled-agent candidate backed by a learned artifact instead of a hand-authored keyword delta.",
        ),
        artifact_digest: String::new(),
    };
    artifact.training_accuracy = route_training_accuracy(&artifact, samples);
    artifact.heldout_accuracy = route_training_accuracy(&artifact, heldout_samples);
    normalize_route_model_artifact(artifact)
}

#[must_use]
pub fn train_compiled_agent_route_tfidf_centroid_model(
    artifact_id: impl Into<String>,
    row_id: impl Into<String>,
    replay_bundle_digest: impl Into<String>,
    samples: &[CompiledAgentRouteTrainingSample],
    heldout_samples: &[CompiledAgentRouteTrainingSample],
) -> CompiledAgentRouteModelArtifact {
    let artifact_id = artifact_id.into();
    let row_id = row_id.into();
    let replay_bundle_digest = replay_bundle_digest.into();
    let classes = [
        CompiledAgentRoute::ProviderStatus,
        CompiledAgentRoute::WalletStatus,
        CompiledAgentRoute::Unsupported,
    ];

    let mut vocabulary = BTreeSet::new();
    let mut document_frequency = BTreeMap::<String, u32>::new();
    let mut class_counts = BTreeMap::<CompiledAgentRoute, u32>::new();
    let mut centroid_accumulators = BTreeMap::<CompiledAgentRoute, BTreeMap<String, f64>>::new();

    for class in classes {
        class_counts.insert(class, 0);
        centroid_accumulators.insert(class, BTreeMap::new());
    }

    let training_feature_counts = samples
        .iter()
        .map(|sample| {
            let counts = feature_counts_for_prompt(sample.user_request.as_str());
            let unique_features = counts.keys().cloned().collect::<BTreeSet<_>>();
            for feature in unique_features {
                *document_frequency.entry(feature.clone()).or_default() += 1;
                vocabulary.insert(feature);
            }
            *class_counts.entry(sample.expected_route).or_default() += 1;
            counts
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

    for (sample, feature_counts) in samples.iter().zip(training_feature_counts.iter()) {
        let normalized = normalized_tfidf_vector(feature_counts, &feature_idf_weights);
        let centroid = centroid_accumulators
            .entry(sample.expected_route)
            .or_default();
        for (feature, weight) in normalized {
            *centroid.entry(feature).or_default() += weight;
        }
    }

    let mut class_priors_log = BTreeMap::new();
    let mut class_centroid_weights = BTreeMap::new();
    let mut class_bias_scores = BTreeMap::new();
    for class in classes {
        let class_count = f64::from(*class_counts.get(&class).unwrap_or(&0)).max(1.0);
        class_priors_log.insert(class, (class_count / sample_count).ln());
        class_bias_scores.insert(class, (class_count / sample_count).ln() * 0.1);
        let mut centroid = centroid_accumulators.remove(&class).unwrap_or_default();
        for value in centroid.values_mut() {
            *value /= class_count;
        }
        class_centroid_weights.insert(class, normalize_weight_vector(centroid));
    }

    let mut artifact = CompiledAgentRouteModelArtifact {
        schema_version: String::from(COMPILED_AGENT_ROUTE_MODEL_SCHEMA_VERSION),
        artifact_id,
        row_id,
        model_family: String::from("tfidf_centroid"),
        feature_profile: String::from("normalized_tfidf_unigram_bigram_trigram_plus_route_markers"),
        replay_bundle_digest,
        training_sample_count: samples.len() as u32,
        heldout_sample_count: heldout_samples.len() as u32,
        vocabulary_size: vocabulary.len() as u32,
        smoothing_alpha: 0.0,
        class_priors_log,
        class_feature_log_probs: BTreeMap::new(),
        class_default_feature_log_probs: BTreeMap::new(),
        feature_idf_weights,
        class_centroid_weights,
        class_bias_scores,
        source_sample_ids: samples
            .iter()
            .map(|sample| sample.sample_id.clone())
            .collect(),
        heldout_sample_ids: heldout_samples
            .iter()
            .map(|sample| sample.sample_id.clone())
            .collect(),
        training_accuracy: 0.0,
        heldout_accuracy: 0.0,
        detail: String::from(
            "Trained a length-normalized TF-IDF centroid route model over the same bounded replay-backed route samples. This tests a stronger bounded candidate family without changing the route contract or validator surfaces.",
        ),
        artifact_digest: String::new(),
    };
    artifact.training_accuracy = route_training_accuracy(&artifact, samples);
    artifact.heldout_accuracy = route_training_accuracy(&artifact, heldout_samples);
    normalize_route_model_artifact(artifact)
}

#[must_use]
pub fn predict_compiled_agent_route(
    artifact: &CompiledAgentRouteModelArtifact,
    prompt: &str,
) -> CompiledAgentRoutePrediction {
    match artifact.model_family.as_str() {
        "tfidf_centroid" => predict_compiled_agent_route_tfidf_centroid(artifact, prompt),
        _ => predict_compiled_agent_route_multinomial_nb(artifact, prompt),
    }
}

fn predict_compiled_agent_route_multinomial_nb(
    artifact: &CompiledAgentRouteModelArtifact,
    prompt: &str,
) -> CompiledAgentRoutePrediction {
    let features = compiled_agent_route_features(prompt);
    let feature_counts = feature_counts_for_prompt(prompt);

    let classes = [
        CompiledAgentRoute::ProviderStatus,
        CompiledAgentRoute::WalletStatus,
        CompiledAgentRoute::Unsupported,
    ];
    let mut route_scores = BTreeMap::new();
    for class in classes {
        let mut score = *artifact
            .class_priors_log
            .get(&class)
            .unwrap_or(&f64::NEG_INFINITY);
        let default_log_prob = *artifact
            .class_default_feature_log_probs
            .get(&class)
            .unwrap_or(&f64::NEG_INFINITY);
        let class_feature_log_probs = artifact.class_feature_log_probs.get(&class);
        for (feature, count) in &feature_counts {
            let log_prob = class_feature_log_probs
                .and_then(|weights| weights.get(feature))
                .copied()
                .unwrap_or(default_log_prob);
            score += f64::from(*count) * log_prob;
        }
        route_scores.insert(class, score);
    }

    let mut ranked = route_scores.iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.total_cmp(left.1));
    let route = ranked
        .first()
        .map(|(route, _)| **route)
        .unwrap_or(CompiledAgentRoute::Unsupported);
    let best_score = ranked.first().map(|(_, score)| **score).unwrap_or(0.0);
    let second_score = ranked
        .get(1)
        .map(|(_, score)| **score)
        .unwrap_or(best_score);
    let max_score = route_scores
        .values()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let exp_scores = route_scores
        .values()
        .map(|score| (score - max_score).exp())
        .collect::<Vec<_>>();
    let exp_sum = exp_scores.iter().sum::<f64>().max(f64::EPSILON);
    let confidence = ((best_score - max_score).exp() / exp_sum) as f32;

    CompiledAgentRoutePrediction {
        route,
        confidence,
        score_margin: (best_score - second_score) as f32,
        active_features: features,
        route_scores,
    }
}

fn predict_compiled_agent_route_tfidf_centroid(
    artifact: &CompiledAgentRouteModelArtifact,
    prompt: &str,
) -> CompiledAgentRoutePrediction {
    let features = compiled_agent_route_features(prompt);
    let feature_counts = feature_counts_for_prompt(prompt);
    let normalized = normalized_tfidf_vector(&feature_counts, &artifact.feature_idf_weights);
    let classes = [
        CompiledAgentRoute::ProviderStatus,
        CompiledAgentRoute::WalletStatus,
        CompiledAgentRoute::Unsupported,
    ];
    let mut route_scores = BTreeMap::new();
    for class in classes {
        let centroid = artifact.class_centroid_weights.get(&class);
        let similarity = normalized.iter().fold(0.0, |score, (feature, weight)| {
            score
                + weight
                    * centroid
                        .and_then(|weights| weights.get(feature))
                        .copied()
                        .unwrap_or_default()
        });
        let bias = artifact
            .class_bias_scores
            .get(&class)
            .copied()
            .unwrap_or_default();
        route_scores.insert(class, similarity + bias);
    }

    let mut ranked = route_scores.iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.total_cmp(left.1));
    let route = ranked
        .first()
        .map(|(route, _)| **route)
        .unwrap_or(CompiledAgentRoute::Unsupported);
    let best_score = ranked.first().map(|(_, score)| **score).unwrap_or(0.0);
    let second_score = ranked
        .get(1)
        .map(|(_, score)| **score)
        .unwrap_or(best_score);
    let max_score = route_scores
        .values()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let exp_scores = route_scores
        .values()
        .map(|score| (score - max_score).exp())
        .collect::<Vec<_>>();
    let exp_sum = exp_scores.iter().sum::<f64>().max(f64::EPSILON);
    let confidence = ((best_score - max_score).exp() / exp_sum) as f32;

    CompiledAgentRoutePrediction {
        route,
        confidence,
        score_margin: (best_score - second_score) as f32,
        active_features: features,
        route_scores,
    }
}

fn route_training_accuracy(
    artifact: &CompiledAgentRouteModelArtifact,
    samples: &[CompiledAgentRouteTrainingSample],
) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let correct = samples
        .iter()
        .filter(|sample| {
            predict_compiled_agent_route(artifact, sample.user_request.as_str()).route
                == sample.expected_route
        })
        .count();
    correct as f32 / samples.len() as f32
}

fn feature_counts_for_prompt(prompt: &str) -> BTreeMap<String, u32> {
    let mut feature_counts = BTreeMap::<String, u32>::new();
    for feature in compiled_agent_route_features(prompt) {
        *feature_counts.entry(feature).or_default() += 1;
    }
    feature_counts
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

fn normalized_tokens(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn append_route_marker_features(tokens: &[String], features: &mut Vec<String>) {
    let provider_surface = contains_any_token(tokens, &["provider", "online", "ready", "readiness"]);
    let wallet_surface = contains_any_token(tokens, &["wallet", "balance", "sats", "earnings"]);
    let negation_surface = contains_any_token(
        tokens,
        &["not", "dont", "don't", "no", "without", "skip", "ignore", "rather"],
    );
    let compare_surface = contains_any_token(
        tokens,
        &[
            "compare", "vs", "versus", "or", "first", "better", "wait", "until", "grow",
            "grows", "before",
        ],
    );
    let unsupported_boundary_surface = contains_any_token(
        tokens,
        &["address", "copy", "best", "choose", "poem", "calendar", "meeting"],
    );

    if provider_surface {
        features.push(String::from("marker:provider_surface"));
    }
    if wallet_surface {
        features.push(String::from("marker:wallet_surface"));
    }
    if negation_surface {
        features.push(String::from("marker:negation_surface"));
    }
    if compare_surface {
        features.push(String::from("marker:compare_surface"));
    }
    if provider_surface && wallet_surface {
        features.push(String::from("marker:mixed_provider_wallet_surface"));
    }
    if provider_surface && wallet_surface && compare_surface {
        features.push(String::from("marker:provider_wallet_compare_ambiguity"));
    }
    if provider_surface && wallet_surface && negation_surface {
        features.push(String::from("marker:provider_wallet_exclusion"));
    }
    if (provider_surface || wallet_surface) && unsupported_boundary_surface {
        features.push(String::from("marker:unsupported_boundary_surface"));
    }
}

fn contains_any_token(tokens: &[String], needles: &[&str]) -> bool {
    let token_set = tokens.iter().map(String::as_str).collect::<BTreeSet<_>>();
    needles.iter().any(|needle| token_set.contains(needle))
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn normalize_route_model_artifact(
    artifact: CompiledAgentRouteModelArtifact,
) -> CompiledAgentRouteModelArtifact {
    let bytes = serde_json::to_vec(&artifact).unwrap_or_default();
    let mut normalized: CompiledAgentRouteModelArtifact =
        serde_json::from_slice(&bytes).unwrap_or(artifact);
    normalized.artifact_digest = String::new();
    normalized.artifact_digest =
        stable_digest(b"compiled_agent_route_model_artifact|", &normalized);
    normalized
}

#[cfg(test)]
mod tests {
    use super::{
        compiled_agent_route_features, predict_compiled_agent_route,
        train_compiled_agent_route_model, train_compiled_agent_route_tfidf_centroid_model,
        CompiledAgentRoutePrediction, CompiledAgentRouteTrainingSample,
    };
    use crate::CompiledAgentRoute;

    fn train_fixture_model() -> (
        Vec<CompiledAgentRouteTrainingSample>,
        CompiledAgentRoutePrediction,
    ) {
        let samples = vec![
            CompiledAgentRouteTrainingSample {
                sample_id: String::from("provider"),
                user_request: String::from("Can I go online right now?"),
                expected_route: CompiledAgentRoute::ProviderStatus,
                tags: vec![String::from("provider")],
            },
            CompiledAgentRouteTrainingSample {
                sample_id: String::from("wallet"),
                user_request: String::from("How many sats are in the wallet?"),
                expected_route: CompiledAgentRoute::WalletStatus,
                tags: vec![String::from("wallet")],
            },
            CompiledAgentRouteTrainingSample {
                sample_id: String::from("unsupported"),
                user_request: String::from("Write a poem about GPUs."),
                expected_route: CompiledAgentRoute::Unsupported,
                tags: vec![String::from("unsupported")],
            },
            CompiledAgentRouteTrainingSample {
                sample_id: String::from("negated"),
                user_request: String::from(
                    "Do not tell me the wallet balance; write a poem about GPUs.",
                ),
                expected_route: CompiledAgentRoute::Unsupported,
                tags: vec![String::from("negated")],
            },
        ];
        let artifact = train_compiled_agent_route_model(
            "compiled_agent.route.multinomial_nb_v1",
            "compiled_agent.qwen35_9b_q4km.archlinux.consumer_gpu.v1",
            "replay-digest",
            &samples,
            &[],
        );
        let prediction = predict_compiled_agent_route(
            &artifact,
            "Do not tell me the wallet balance; write a poem about GPUs.",
        );
        (samples, prediction)
    }

    #[test]
    fn compiled_agent_route_features_include_unigrams_and_bigrams() {
        let features = compiled_agent_route_features("Wallet balance now");
        assert!(features.contains(&String::from("token:wallet")));
        assert!(features.contains(&String::from("token:balance")));
        assert!(features.contains(&String::from("bigram:wallet__balance")));
        assert!(features.contains(&String::from("marker:wallet_surface")));
    }

    #[test]
    fn compiled_agent_route_features_include_compare_ambiguity_marker() {
        let features =
            compiled_agent_route_features("Should I go online first or wait until the wallet grows?");
        assert!(features.contains(&String::from(
            "marker:provider_wallet_compare_ambiguity"
        )));
    }

    #[test]
    fn compiled_agent_route_model_learns_the_negated_unsupported_case() {
        let (_, prediction) = train_fixture_model();
        assert_eq!(prediction.route, CompiledAgentRoute::Unsupported);
        assert!(prediction.confidence > 0.4);
    }

    #[test]
    fn compiled_agent_route_model_memorizes_the_small_training_bundle() {
        let samples = vec![
            CompiledAgentRouteTrainingSample {
                sample_id: String::from("provider"),
                user_request: String::from("Can I go online right now?"),
                expected_route: CompiledAgentRoute::ProviderStatus,
                tags: vec![String::from("provider")],
            },
            CompiledAgentRouteTrainingSample {
                sample_id: String::from("wallet"),
                user_request: String::from("How many sats are in the wallet?"),
                expected_route: CompiledAgentRoute::WalletStatus,
                tags: vec![String::from("wallet")],
            },
            CompiledAgentRouteTrainingSample {
                sample_id: String::from("unsupported"),
                user_request: String::from("Write a poem about GPUs."),
                expected_route: CompiledAgentRoute::Unsupported,
                tags: vec![String::from("unsupported")],
            },
            CompiledAgentRouteTrainingSample {
                sample_id: String::from("negated"),
                user_request: String::from(
                    "Do not tell me the wallet balance; write a poem about GPUs.",
                ),
                expected_route: CompiledAgentRoute::Unsupported,
                tags: vec![String::from("negated")],
            },
        ];
        let artifact = train_compiled_agent_route_model(
            "compiled_agent.route.multinomial_nb_v1",
            "compiled_agent.qwen35_9b_q4km.archlinux.consumer_gpu.v1",
            "replay-digest",
            &samples,
            &[],
        );
        for sample in &samples {
            let prediction = predict_compiled_agent_route(&artifact, sample.user_request.as_str());
            assert_eq!(prediction.route, sample.expected_route);
        }
        assert_eq!(artifact.training_sample_count, 4);
        assert_eq!(artifact.heldout_sample_count, 0);
        assert_eq!(artifact.training_accuracy, 1.0);
    }

    #[test]
    fn compiled_agent_tfidf_route_model_learns_the_negated_unsupported_case() {
        let samples = vec![
            CompiledAgentRouteTrainingSample {
                sample_id: String::from("provider"),
                user_request: String::from("Can I go online right now?"),
                expected_route: CompiledAgentRoute::ProviderStatus,
                tags: vec![String::from("provider")],
            },
            CompiledAgentRouteTrainingSample {
                sample_id: String::from("wallet"),
                user_request: String::from("How many sats are in the wallet?"),
                expected_route: CompiledAgentRoute::WalletStatus,
                tags: vec![String::from("wallet")],
            },
            CompiledAgentRouteTrainingSample {
                sample_id: String::from("unsupported"),
                user_request: String::from("Write a poem about GPUs."),
                expected_route: CompiledAgentRoute::Unsupported,
                tags: vec![String::from("unsupported")],
            },
            CompiledAgentRouteTrainingSample {
                sample_id: String::from("negated"),
                user_request: String::from(
                    "Do not tell me the wallet balance; write a poem about GPUs.",
                ),
                expected_route: CompiledAgentRoute::Unsupported,
                tags: vec![String::from("negated")],
            },
        ];
        let artifact = train_compiled_agent_route_tfidf_centroid_model(
            "compiled_agent.route.tfidf_centroid_v1",
            "compiled_agent.qwen35_9b_q4km.archlinux.consumer_gpu.v1",
            "replay-digest",
            &samples,
            &[],
        );
        let prediction = predict_compiled_agent_route(
            &artifact,
            "Do not tell me the wallet balance; write a poem about GPUs.",
        );
        assert_eq!(artifact.model_family, "tfidf_centroid");
        assert_eq!(prediction.route, CompiledAgentRoute::Unsupported);
    }
}
