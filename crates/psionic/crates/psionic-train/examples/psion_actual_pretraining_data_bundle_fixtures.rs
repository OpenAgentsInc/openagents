use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_train::{
    PSION_ACTUAL_PRETRAINING_DATA_BUNDLE_ID, PSION_ACTUAL_PRETRAINING_DATA_BUNDLE_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_LANE_ID, PsionActualPretrainingArtifactRef,
    PsionActualPretrainingContentClassShare, PsionActualPretrainingDataBundle,
    PsionActualPretrainingDataDedupAuthority, PsionActualPretrainingDataFilterAuthority,
    PsionActualPretrainingDataTransformationStage, PsionActualPretrainingEvalBinding,
    PsionActualPretrainingFamilyMixtureWeight, PsionActualPretrainingMixtureAuthority,
    PsionActualPretrainingRecipeChangeEvalPackage, PsionActualPretrainingRegressionGate,
    PsionActualPretrainingRepetitiveRegionControl, PsionActualPretrainingReplayAuthority,
    PsionActualPretrainingSourceContributionCap,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&fixtures_dir)?;

    let lane_spec_path =
        root.join("fixtures/psion/pretrain/psion_actual_pretraining_lane_spec_v1.json");
    let recipe_bundle_path =
        root.join("fixtures/psion/pretrain/psion_actual_pretraining_recipe_bundle_v1.json");
    let admission_policy_path =
        root.join("fixtures/psion/corpus_admission/psion_corpus_admission_policy_v1.json");
    let source_admission_manifest_path =
        root.join("fixtures/psion/corpus_admission/psion_source_admission_manifest_v1.json");
    let source_lifecycle_manifest_path =
        root.join("fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json");
    let raw_source_manifest_path =
        root.join("fixtures/psion/ingestion/psion_raw_source_manifest_v1.json");
    let benchmark_isolation_manifest_path =
        root.join("fixtures/psion/isolation/psion_exclusion_manifest_v1.json");
    let tokenizer_training_manifest_path =
        root.join("fixtures/psion/tokenizer/psion_tokenizer_training_manifest_v1.json");
    let tokenized_corpus_manifest_path =
        root.join("fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json");
    let sampling_policy_manifest_path =
        root.join("fixtures/psion/sampling/psion_sampling_policy_manifest_v1.json");
    let sampling_policy_comparison_path =
        root.join("fixtures/psion/sampling/psion_sampling_policy_comparison_receipt_v1.json");
    let benchmark_catalog_path =
        root.join("fixtures/psion/benchmarks/psion_benchmark_catalog_v1.json");
    let benchmark_receipt_set_path =
        root.join("fixtures/psion/benchmarks/psion_benchmark_receipt_set_v1.json");

    let recipe_bundle = load_json(&recipe_bundle_path)?;
    let source_admission_manifest = load_json(&source_admission_manifest_path)?;
    let benchmark_isolation_manifest = load_json(&benchmark_isolation_manifest_path)?;
    let raw_source_manifest = load_json(&raw_source_manifest_path)?;
    let tokenizer_training_manifest = load_json(&tokenizer_training_manifest_path)?;
    let tokenized_corpus_manifest = load_json(&tokenized_corpus_manifest_path)?;
    let sampling_policy_manifest = load_json(&sampling_policy_manifest_path)?;
    let sampling_policy_comparison = load_json(&sampling_policy_comparison_path)?;
    let benchmark_catalog = load_json(&benchmark_catalog_path)?;
    let benchmark_receipt_set = load_json(&benchmark_receipt_set_path)?;

    let dataset_identity = string_at(&recipe_bundle, &["dataset_identity"])?;
    let sampling_policy_id = string_at(&recipe_bundle, &["sampling_policy_id"])?;
    let sampling_policy_version = string_at(&recipe_bundle, &["sampling_policy_version"])?;
    let preprocessing_version = string_at(
        &raw_source_manifest,
        &["normalization_profile", "preprocessing_version"],
    )?;

    let transformation_stages = vec![
        PsionActualPretrainingDataTransformationStage {
            stage_id: String::from("psion_actual_pretraining_admission_review"),
            stage_kind: String::from("admission_review"),
            source_artifact: artifact_ref(&root, &source_admission_manifest_path)?,
            output_identity: String::from("psion_source_admission_manifest_v1"),
            detail: String::from(
                "Reviewed source-admission truth freezes which sources are admitted, restricted, evaluation-only, or rejected before any actual-lane build step starts.",
            ),
        },
        PsionActualPretrainingDataTransformationStage {
            stage_id: String::from("psion_actual_pretraining_benchmark_isolation"),
            stage_kind: String::from("benchmark_isolation"),
            source_artifact: artifact_ref(&root, &benchmark_isolation_manifest_path)?,
            output_identity: String::from("psion_exclusion_manifest_v1"),
            detail: String::from(
                "Benchmark and held-out isolation freezes the exclusion surface that keeps evaluation-only, tokenizer-only, and rejected material out of the train loader.",
            ),
        },
        PsionActualPretrainingDataTransformationStage {
            stage_id: String::from("psion_actual_pretraining_raw_source_ingestion"),
            stage_kind: String::from("raw_source_ingestion"),
            source_artifact: artifact_ref(&root, &raw_source_manifest_path)?,
            output_identity: preprocessing_version.clone(),
            detail: String::from(
                "Raw-source ingestion normalizes admitted material into one stable section-anchored source family before tokenizer or tokenized-corpus stages consume it.",
            ),
        },
        PsionActualPretrainingDataTransformationStage {
            stage_id: String::from("psion_actual_pretraining_tokenizer_training"),
            stage_kind: String::from("tokenizer_training"),
            source_artifact: artifact_ref(&root, &tokenizer_training_manifest_path)?,
            output_identity: format!(
                "{}@{}",
                string_at(&tokenizer_training_manifest, &["tokenizer_id"])?,
                string_at(&tokenizer_training_manifest, &["tokenizer_version"])?
            ),
            detail: String::from(
                "Tokenizer training freezes the tokenizer-visible source set separately from the model-training-visible source set so restricted and evaluation-only sources cannot drift silently into training.",
            ),
        },
        PsionActualPretrainingDataTransformationStage {
            stage_id: String::from("psion_actual_pretraining_tokenized_corpus_build"),
            stage_kind: String::from("tokenized_corpus_build"),
            source_artifact: artifact_ref(&root, &tokenized_corpus_manifest_path)?,
            output_identity: dataset_identity.clone(),
            detail: String::from(
                "Tokenized-corpus build freezes the replay-safe dataset identity, shard layout, packing policy, and train/validation/held-out split boundaries used by the actual lane.",
            ),
        },
        PsionActualPretrainingDataTransformationStage {
            stage_id: String::from("psion_actual_pretraining_sampling_policy_freeze"),
            stage_kind: String::from("sampling_policy_freeze"),
            source_artifact: artifact_ref(&root, &sampling_policy_manifest_path)?,
            output_identity: format!("{sampling_policy_id}@{sampling_policy_version}"),
            detail: String::from(
                "Sampling-policy freeze binds one production mixture, repetitive-region controls, and explicit regression thresholds into the actual recipe instead of leaving mixture choice implicit.",
            ),
        },
    ];

    let filter_authority = PsionActualPretrainingDataFilterAuthority {
        admission_policy: artifact_ref(&root, &admission_policy_path)?,
        source_admission_manifest: artifact_ref(&root, &source_admission_manifest_path)?,
        source_lifecycle_manifest: artifact_ref(&root, &source_lifecycle_manifest_path)?,
        benchmark_isolation_manifest: artifact_ref(&root, &benchmark_isolation_manifest_path)?,
        admitted_training_source_ids: training_source_ids(&tokenized_corpus_manifest)?,
        tokenizer_only_source_ids: tokenizer_only_source_ids(&tokenizer_training_manifest)?,
        held_out_source_ids: string_list(&benchmark_isolation_manifest, &["held_out_source_ids"])?,
        rejected_source_ids: rejected_source_ids(&source_admission_manifest)?,
        detail: String::from(
            "Filter authority freezes the exact train-visible, tokenizer-only, held-out, and rejected source ids consumed by the actual recipe.",
        ),
    };

    let dedup_authority = PsionActualPretrainingDataDedupAuthority {
        near_duplicate_review_required_before_training: bool_at(
            &benchmark_isolation_manifest,
            &[
                "near_duplicate_review_policy",
                "review_required_before_training",
            ],
        )?,
        near_duplicate_review_required_before_benchmark_publication: bool_at(
            &benchmark_isolation_manifest,
            &[
                "near_duplicate_review_policy",
                "review_required_before_benchmark_publication",
            ],
        )?,
        near_duplicate_review_ref: near_duplicate_review_ref(&benchmark_catalog)?,
        training_excluded_source_ids: string_list(
            &benchmark_isolation_manifest,
            &["training_excluded_source_ids"],
        )?,
        benchmark_excluded_source_ids: string_list(
            &benchmark_isolation_manifest,
            &["benchmark_excluded_source_ids"],
        )?,
        repetitive_region_controls: repetitive_region_controls(&sampling_policy_manifest)?,
        detail: String::from(
            "Dedup authority keeps near-duplicate review mandatory, preserves the mechanically excluded source ids, and freezes repetitive-region downweighting so the actual lane cannot silently train on duplicated boilerplate.",
        ),
    };

    let mixture_authority = PsionActualPretrainingMixtureAuthority {
        dataset_identity: dataset_identity.clone(),
        sampling_policy_id: sampling_policy_id.clone(),
        sampling_policy_version: sampling_policy_version.clone(),
        sampling_policy_manifest: artifact_ref(&root, &sampling_policy_manifest_path)?,
        maximum_code_token_ratio_bps: u32_at(
            &sampling_policy_manifest,
            &["maximum_code_token_ratio_bps"],
        )?,
        source_family_weights: source_family_weights(&sampling_policy_manifest)?,
        source_contribution_caps: source_contribution_caps(&sampling_policy_manifest)?,
        content_class_token_share_report: content_class_token_share_report(
            &sampling_policy_manifest,
        )?,
        comparison_receipt: artifact_ref(&root, &sampling_policy_comparison_path)?,
        lm_loss_delta_bps: i32_at(&sampling_policy_comparison, &["lm_loss_delta_bps"])?,
        regression_gates: regression_gates(&sampling_policy_manifest, &sampling_policy_comparison)?,
        detail: String::from(
            "Mixture authority freezes one production pretrain mix, one regression budget surface, and one candidate-vs-baseline comparison receipt that recipe changes must clear before promotion.",
        ),
    };

    let replay_authority = PsionActualPretrainingReplayAuthority {
        tokenizer_training_manifest: artifact_ref(&root, &tokenizer_training_manifest_path)?,
        raw_source_manifest: artifact_ref(&root, &raw_source_manifest_path)?,
        tokenized_corpus_manifest: artifact_ref(&root, &tokenized_corpus_manifest_path)?,
        dataset_identity,
        replay_iteration_mode: string_at(
            &tokenized_corpus_manifest,
            &["replay_contract", "iteration_mode"],
        )?,
        shard_ordering: string_at(
            &tokenized_corpus_manifest,
            &["replay_contract", "shard_ordering"],
        )?,
        deterministic_shuffle_seed: u64_at(
            &tokenized_corpus_manifest,
            &["replay_contract", "deterministic_shuffle_seed"],
        )?,
        packing_policy_id: string_at(&tokenized_corpus_manifest, &["packing_policy", "policy_id"])?,
        packing_policy_version: string_at(
            &tokenized_corpus_manifest,
            &["packing_policy", "policy_version"],
        )?,
        max_sequence_tokens: u64_at(
            &tokenized_corpus_manifest,
            &["packing_policy", "max_sequence_tokens"],
        )?,
        train_shard_ids: split_shard_ids(&tokenized_corpus_manifest, "train")?,
        validation_shard_ids: split_shard_ids(&tokenized_corpus_manifest, "validation")?,
        held_out_shard_ids: split_shard_ids(&tokenized_corpus_manifest, "held_out")?,
        detail: String::from(
            "Replay authority freezes the exact repeat-plus-deterministic-shuffle corpus contract, the context packing policy, and the split-specific shard identities consumed by the actual pretraining lane.",
        ),
    };

    let required_eval_families = vec![
        String::from("architecture_reasoning"),
        String::from("normative_spec_reading"),
        String::from("engineering_spec_interpretation"),
        String::from("memorization_versus_reasoning"),
    ];
    let recipe_change_eval_package = PsionActualPretrainingRecipeChangeEvalPackage {
        benchmark_catalog: artifact_ref(&root, &benchmark_catalog_path)?,
        benchmark_receipt_set: artifact_ref(&root, &benchmark_receipt_set_path)?,
        required_package_families: required_eval_families.clone(),
        required_acceptance_families: required_eval_families.clone(),
        eval_bindings: eval_bindings(&benchmark_receipt_set, &required_eval_families)?,
        detail: String::from(
            "Recipe-change eval package keeps the mixture attached to the existing benchmark catalog and retained receipt set so data changes remain measurable on the real lane instead of in a detached data-research loop.",
        ),
    };

    let mut bundle = PsionActualPretrainingDataBundle {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_DATA_BUNDLE_SCHEMA_VERSION),
        data_bundle_id: String::from(PSION_ACTUAL_PRETRAINING_DATA_BUNDLE_ID),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        lane_spec: artifact_ref(&root, &lane_spec_path)?,
        recipe_bundle: artifact_ref(&root, &recipe_bundle_path)?,
        transformation_stages,
        filter_authority,
        dedup_authority,
        mixture_authority,
        replay_authority,
        recipe_change_eval_package,
        support_refs: vec![
            String::from("docs/PSION_ACTUAL_PRETRAINING_DATA_BUNDLE.md"),
            String::from("docs/PSION_TOKENIZED_CORPUS.md"),
            String::from("docs/TRAIN_SYSTEM.md"),
            String::from("fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json"),
            String::from("fixtures/psion/sampling/psion_sampling_policy_manifest_v1.json"),
            String::from("fixtures/psion/benchmarks/psion_benchmark_receipt_set_v1.json"),
        ],
        claim_boundary: String::from(
            "The actual-lane data bundle freezes one admission-filtering path, one benchmark-isolation and near-duplicate review posture, one tokenized replay-safe corpus, one production mixture, and one bounded recipe-change eval package. It does not claim Common Crawl-scale ingestion, open-ended mixture search, or automated data research outside the frozen lane.",
        ),
        summary: String::from(
            "The canonical actual-pretraining data bundle binds admission, isolation, raw-source ingestion, tokenizer training, tokenized replay truth, mixture authority, and bounded recipe-change eval receipts into one actual-lane data contract.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_data_bundle_digest(&bundle)?;
    bundle.validate()?;

    write_json(
        &fixtures_dir.join("psion_actual_pretraining_data_bundle_v1.json"),
        &bundle,
    )?;
    Ok(())
}

fn training_source_ids(tokenized_corpus: &Value) -> Result<Vec<String>, Box<dyn Error>> {
    let mut source_ids = string_set();
    for shard in array_at(tokenized_corpus, &["shards"])? {
        let split_kind = string_field(shard, "split_kind")?;
        if split_kind != "train" && split_kind != "validation" {
            continue;
        }
        for lineage in array_field(shard, "source_lineage")? {
            source_ids.insert(string_field(lineage, "source_id")?);
        }
    }
    Ok(source_ids.into_iter().collect())
}

fn tokenizer_only_source_ids(tokenizer_training: &Value) -> Result<Vec<String>, Box<dyn Error>> {
    let mut source_ids = string_set();
    for row in array_at(tokenizer_training, &["exposure_report"])? {
        if bool_field(row, "tokenizer_only_exposure")? {
            source_ids.insert(string_field(row, "source_id")?);
        }
    }
    Ok(source_ids.into_iter().collect())
}

fn rejected_source_ids(source_admission: &Value) -> Result<Vec<String>, Box<dyn Error>> {
    let mut source_ids = string_set();
    for row in array_at(source_admission, &["sources"])? {
        if string_field(row, "review_decision")? == "rejected" {
            source_ids.insert(string_field(row, "source_id")?);
        }
    }
    Ok(source_ids.into_iter().collect())
}

fn repetitive_region_controls(
    sampling_policy: &Value,
) -> Result<Vec<PsionActualPretrainingRepetitiveRegionControl>, Box<dyn Error>> {
    array_at(sampling_policy, &["repetitive_region_controls"])?
        .iter()
        .map(|row| {
            Ok(PsionActualPretrainingRepetitiveRegionControl {
                source_id: string_field(row, "source_id")?,
                document_id: string_field(row, "document_id")?,
                section_id: string_field(row, "section_id")?,
                downweight_multiplier_bps: u32_field(row, "downweight_multiplier_bps")?,
                maximum_region_token_share_bps: u32_field(row, "maximum_region_token_share_bps")?,
            })
        })
        .collect()
}

fn source_family_weights(
    sampling_policy: &Value,
) -> Result<Vec<PsionActualPretrainingFamilyMixtureWeight>, Box<dyn Error>> {
    array_at(sampling_policy, &["source_family_weights"])?
        .iter()
        .map(|row| {
            Ok(PsionActualPretrainingFamilyMixtureWeight {
                source_family_id: string_field(row, "source_family_id")?,
                content_class: string_field(row, "content_class")?,
                sampling_weight_bps: u32_field(row, "sampling_weight_bps")?,
                maximum_family_token_share_bps: u32_field(row, "maximum_family_token_share_bps")?,
            })
        })
        .collect()
}

fn source_contribution_caps(
    sampling_policy: &Value,
) -> Result<Vec<PsionActualPretrainingSourceContributionCap>, Box<dyn Error>> {
    array_at(sampling_policy, &["source_contribution_caps"])?
        .iter()
        .map(|row| {
            Ok(PsionActualPretrainingSourceContributionCap {
                source_id: string_field(row, "source_id")?,
                maximum_source_token_share_bps: u32_field(row, "maximum_source_token_share_bps")?,
            })
        })
        .collect()
}

fn content_class_token_share_report(
    sampling_policy: &Value,
) -> Result<Vec<PsionActualPretrainingContentClassShare>, Box<dyn Error>> {
    array_at(sampling_policy, &["content_class_token_share_report"])?
        .iter()
        .map(|row| {
            Ok(PsionActualPretrainingContentClassShare {
                content_class: string_field(row, "content_class")?,
                observed_token_share_bps: u32_field(row, "observed_token_share_bps")?,
            })
        })
        .collect()
}

fn regression_gates(
    sampling_policy: &Value,
    comparison_receipt: &Value,
) -> Result<Vec<PsionActualPretrainingRegressionGate>, Box<dyn Error>> {
    let mut thresholds = std::collections::BTreeMap::new();
    for row in array_at(sampling_policy, &["regression_thresholds"])? {
        thresholds.insert(
            string_field(row, "regression_kind")?,
            u32_field(row, "maximum_regression_bps")?,
        );
    }
    array_at(comparison_receipt, &["regression_metrics"])?
        .iter()
        .map(|row| {
            let regression_kind = string_field(row, "regression_kind")?;
            let Some(maximum_regression_bps) = thresholds.get(regression_kind.as_str()) else {
                return Err(boxed_error(format!(
                    "missing regression threshold for `{regression_kind}`"
                )));
            };
            Ok(PsionActualPretrainingRegressionGate {
                regression_kind,
                maximum_regression_bps: *maximum_regression_bps,
                observed_regression_bps: u32_field(row, "regression_bps")?,
            })
        })
        .collect()
}

fn split_shard_ids(
    tokenized_corpus: &Value,
    split_name: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    for split in array_at(tokenized_corpus, &["splits"])? {
        if string_field(split, "split_name")? == split_name {
            return string_list_from_value(field(split, "shard_ids")?);
        }
    }
    Err(format!("missing split `{split_name}`").into())
}

fn near_duplicate_review_ref(benchmark_catalog: &Value) -> Result<String, Box<dyn Error>> {
    let first_package = array_at(benchmark_catalog, &["packages"])?
        .first()
        .ok_or_else(|| io_error("benchmark catalog missing packages"))?;
    string_at(
        first_package,
        &["contamination_inputs", "near_duplicate_review_ref"],
    )
}

fn eval_bindings(
    benchmark_receipt_set: &Value,
    required_families: &[String],
) -> Result<Vec<PsionActualPretrainingEvalBinding>, Box<dyn Error>> {
    let required = required_families
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let mut rows = Vec::new();
    for receipt in array_at(benchmark_receipt_set, &["receipts"])? {
        let package_family = string_field(receipt, "package_family")?;
        if !required.contains(package_family.as_str()) {
            continue;
        }
        let observed_metric = array_field(receipt, "observed_metrics")?
            .first()
            .ok_or_else(|| io_error("benchmark receipt missing observed metrics"))?;
        rows.push(PsionActualPretrainingEvalBinding {
            package_family,
            acceptance_family: string_field(receipt, "acceptance_family")?,
            receipt_id: string_field(receipt, "receipt_id")?,
            contamination_input_digest: string_field(receipt, "contamination_input_digest")?,
            metric_kind: string_field(observed_metric, "metric_kind")?,
            observed_bps: u32_field(observed_metric, "observed_bps")?,
            detail: String::from(
                "Bounded recipe-change eval keeps this benchmark family attached to the actual-lane data review surface before mixture changes can be admitted.",
            ),
        });
    }
    if rows.len() != required.len() {
        return Err(boxed_error(format!(
            "expected {} required recipe-change eval bindings, found {}",
            required.len(),
            rows.len()
        )));
    }
    rows.sort_by(|left, right| left.package_family.cmp(&right.package_family));
    Ok(rows)
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), Box<dyn Error>> {
    let encoded = serde_json::to_vec_pretty(value)?;
    fs::write(path, encoded)?;
    Ok(())
}

fn artifact_ref(
    root: &Path,
    path: &Path,
) -> Result<PsionActualPretrainingArtifactRef, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let relative = path
        .strip_prefix(root)?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(PsionActualPretrainingArtifactRef {
        path: relative,
        sha256: sha256_hex(&bytes),
    })
}

fn stable_data_bundle_digest(
    bundle: &PsionActualPretrainingDataBundle,
) -> Result<String, Box<dyn Error>> {
    let mut clone = bundle.clone();
    clone.bundle_digest.clear();
    let bytes = serde_json::to_vec(&clone)?;
    let mut hasher = Sha256::new();
    hasher.update(b"psion_actual_pretraining_data_bundle|");
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn load_json(path: &Path) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn string_at(value: &Value, path: &[&str]) -> Result<String, Box<dyn Error>> {
    Ok(navigate(value, path)?
        .as_str()
        .ok_or_else(|| io_error(format!("expected string at {}", path.join("."))))?
        .to_owned())
}

fn bool_at(value: &Value, path: &[&str]) -> Result<bool, Box<dyn Error>> {
    navigate(value, path)?
        .as_bool()
        .ok_or_else(|| boxed_error(format!("expected bool at {}", path.join("."))))
}

fn u64_at(value: &Value, path: &[&str]) -> Result<u64, Box<dyn Error>> {
    navigate(value, path)?
        .as_u64()
        .ok_or_else(|| boxed_error(format!("expected u64 at {}", path.join("."))))
}

fn u32_at(value: &Value, path: &[&str]) -> Result<u32, Box<dyn Error>> {
    Ok(u64_at(value, path)?.try_into()?)
}

fn i32_at(value: &Value, path: &[&str]) -> Result<i32, Box<dyn Error>> {
    Ok(navigate(value, path)?
        .as_i64()
        .ok_or_else(|| format!("expected i64 at {}", path.join(".")))?
        .try_into()?)
}

fn array_at<'a>(value: &'a Value, path: &[&str]) -> Result<&'a [Value], Box<dyn Error>> {
    Ok(navigate(value, path)?
        .as_array()
        .ok_or_else(|| io_error(format!("expected array at {}", path.join("."))))?)
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value, Box<dyn Error>> {
    value
        .get(key)
        .ok_or_else(|| boxed_error(format!("missing field `{key}`")))
}

fn array_field<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], Box<dyn Error>> {
    Ok(field(value, key)?
        .as_array()
        .ok_or_else(|| io_error(format!("expected array field `{key}`")))?)
}

fn string_field(value: &Value, key: &str) -> Result<String, Box<dyn Error>> {
    Ok(field(value, key)?
        .as_str()
        .ok_or_else(|| io_error(format!("expected string field `{key}`")))?
        .to_owned())
}

fn bool_field(value: &Value, key: &str) -> Result<bool, Box<dyn Error>> {
    field(value, key)?
        .as_bool()
        .ok_or_else(|| boxed_error(format!("expected bool field `{key}`")))
}

fn u32_field(value: &Value, key: &str) -> Result<u32, Box<dyn Error>> {
    Ok(field(value, key)?
        .as_u64()
        .ok_or_else(|| io_error(format!("expected u64 field `{key}`")))?
        .try_into()?)
}

fn string_list(value: &Value, path: &[&str]) -> Result<Vec<String>, Box<dyn Error>> {
    string_list_from_value(navigate(value, path)?)
}

fn string_list_from_value(value: &Value) -> Result<Vec<String>, Box<dyn Error>> {
    let mut rows = string_set();
    for item in value
        .as_array()
        .ok_or_else(|| io_error("expected string array"))?
    {
        rows.insert(
            item.as_str()
                .ok_or_else(|| io_error("expected string array entry"))?
                .to_owned(),
        );
    }
    Ok(rows.into_iter().collect())
}

fn navigate<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Value, Box<dyn Error>> {
    let mut current = value;
    for key in path {
        current = current
            .get(*key)
            .ok_or_else(|| io_error(format!("missing field `{}`", path.join("."))))?;
    }
    Ok(current)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn string_set() -> std::collections::BTreeSet<String> {
    std::collections::BTreeSet::new()
}

fn io_error(message: impl Into<String>) -> std::io::Error {
    std::io::Error::other(message.into())
}

fn boxed_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io_error(message))
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("Cargo.toml").exists() && dir.join("fixtures").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            return Err("failed to locate workspace root".into());
        }
    }
}
