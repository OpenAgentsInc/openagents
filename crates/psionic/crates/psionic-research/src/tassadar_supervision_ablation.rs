use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    TassadarExecutorStructuralSupervisionMetric, TassadarExecutorStructuralSupervisionReport,
};
use psionic_models::TassadarExecutorTrainableSurface;
use psionic_train::{
    TassadarExecutorStructuralSupervisionConfig, TassadarExecutorTrainingConfig,
    TassadarExecutorTrainingReport, TassadarSequenceTrainingManifest,
    execute_tassadar_training_run_without_benchmark,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;
const TRAINING_MANIFEST_FILE: &str = "training_manifest.json";
const TRAINING_REPORT_FILE: &str = "training_report.json";
const RUN_BUNDLE_FILE: &str = "run_bundle.json";

/// Canonical output root for the PTAS-401 supervision ablation.
pub const TASSADAR_SUPERVISION_ABLATION_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/sudoku_v0_supervision_ablation_v1";
/// Canonical report filename for the PTAS-401 supervision ablation.
pub const TASSADAR_SUPERVISION_ABLATION_REPORT_FILE: &str = "supervision_ablation_report.json";
/// Stable comparison reference for the PTAS-401 supervision ablation.
pub const TASSADAR_SUPERVISION_ABLATION_REF: &str =
    "research://openagents/psionic/tassadar/supervision_ablation/v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSupervisionAblationFamilyDelta {
    pub family: String,
    pub baseline_exactness_bps: u32,
    pub candidate_exactness_bps: u32,
    pub delta_bps: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarSupervisionAblationVariantReport {
    pub variant_id: String,
    pub description: String,
    pub structural_supervision_profile_id: String,
    pub run_bundle_ref: String,
    pub training_manifest_ref: String,
    pub training_report_ref: String,
    pub structural_supervision_report_ref: String,
    pub training_manifest_digest: String,
    pub training_report_digest: String,
    pub structural_supervision_report_digest: String,
    pub aggregate_target_token_exactness_bps: u32,
    pub first_target_exactness_bps: u32,
    pub first_32_token_exactness_bps: u32,
    pub structural_metrics: Vec<TassadarExecutorStructuralSupervisionMetric>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarSupervisionAblationReport {
    pub schema_version: u16,
    pub comparison_ref: String,
    pub workload: String,
    pub variants: Vec<TassadarSupervisionAblationVariantReport>,
    pub family_deltas_vs_baseline: Vec<TassadarSupervisionAblationFamilyDelta>,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarSupervisionAblationReport {
    fn new(
        variants: Vec<TassadarSupervisionAblationVariantReport>,
        family_deltas_vs_baseline: Vec<TassadarSupervisionAblationFamilyDelta>,
        summary: String,
    ) -> Self {
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            comparison_ref: String::from(TASSADAR_SUPERVISION_ABLATION_REF),
            workload: String::from("sudoku_v0"),
            variants,
            family_deltas_vs_baseline,
            summary,
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(b"psionic_tassadar_supervision_ablation_report|", &report);
        report
    }
}

#[derive(Clone)]
struct VariantSpec {
    variant_id: &'static str,
    description: &'static str,
    config: TassadarExecutorTrainingConfig,
}

#[derive(Debug, Error)]
pub enum TassadarSupervisionAblationError {
    #[error(transparent)]
    Training(#[from] psionic_train::TassadarExecutorRunError),
    #[error("failed to create output directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        artifact_kind: String,
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to serialize `{artifact_kind}`: {error}")]
    Serialize {
        artifact_kind: String,
        error: serde_json::Error,
    },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

#[must_use]
pub fn tassadar_supervision_ablation_output_path() -> PathBuf {
    repo_root().join(TASSADAR_SUPERVISION_ABLATION_OUTPUT_DIR)
}

pub fn run_tassadar_supervision_ablation(
    output_dir: &Path,
) -> Result<TassadarSupervisionAblationReport, TassadarSupervisionAblationError> {
    fs::create_dir_all(output_dir).map_err(|error| TassadarSupervisionAblationError::CreateDir {
        path: output_dir.display().to_string(),
        error,
    })?;

    let variants = supervision_ablation_variants();
    let mut variant_reports = Vec::new();
    for variant in variants {
        let variant_dir = output_dir.join(variant.variant_id);
        fs::create_dir_all(&variant_dir).map_err(|error| {
            TassadarSupervisionAblationError::CreateDir {
                path: variant_dir.display().to_string(),
                error,
            }
        })?;
        execute_tassadar_training_run_without_benchmark(&variant_dir, &variant.config)?;
        let run_bundle_ref = relative_ref(&variant_dir.join(RUN_BUNDLE_FILE));
        let manifest_ref = relative_ref(&variant_dir.join(TRAINING_MANIFEST_FILE));
        let training_report_ref = relative_ref(&variant_dir.join(TRAINING_REPORT_FILE));
        let structural_report_ref = relative_ref(
            &variant_dir.join(psionic_train::TASSADAR_EXECUTOR_STRUCTURAL_SUPERVISION_REPORT_FILE),
        );
        let manifest: TassadarSequenceTrainingManifest =
            read_json(variant_dir.join(TRAINING_MANIFEST_FILE), "tassadar_training_manifest")?;
        let training_report: TassadarExecutorTrainingReport =
            read_json(variant_dir.join(TRAINING_REPORT_FILE), "tassadar_training_report")?;
        let structural_report: TassadarExecutorStructuralSupervisionReport = read_json(
            variant_dir.join(psionic_train::TASSADAR_EXECUTOR_STRUCTURAL_SUPERVISION_REPORT_FILE),
            "tassadar_structural_supervision_report",
        )?;
        variant_reports.push(TassadarSupervisionAblationVariantReport {
            variant_id: String::from(variant.variant_id),
            description: String::from(variant.description),
            structural_supervision_profile_id: training_report
                .config
                .structural_supervision
                .profile_id
                .clone(),
            run_bundle_ref,
            training_manifest_ref: manifest_ref,
            training_report_ref,
            structural_supervision_report_ref: structural_report_ref,
            training_manifest_digest: manifest.manifest_digest,
            training_report_digest: training_report.report_digest.clone(),
            structural_supervision_report_digest: structural_report.report_digest.clone(),
            aggregate_target_token_exactness_bps: training_report
                .evaluation
                .aggregate_target_token_exactness_bps,
            first_target_exactness_bps: training_report.evaluation.first_target_exactness_bps,
            first_32_token_exactness_bps: training_report.evaluation.first_32_token_exactness_bps,
            structural_metrics: structural_report.aggregate_metrics,
        });
    }

    let family_deltas_vs_baseline = build_family_deltas(variant_reports.as_slice());
    let summary = build_summary(variant_reports.as_slice(), family_deltas_vs_baseline.as_slice());
    let report =
        TassadarSupervisionAblationReport::new(variant_reports, family_deltas_vs_baseline, summary);
    write_json(
        output_dir.join(TASSADAR_SUPERVISION_ABLATION_REPORT_FILE),
        "tassadar_supervision_ablation_report",
        &report,
    )?;
    Ok(report)
}

fn supervision_ablation_variants() -> Vec<VariantSpec> {
    let mut baseline = TassadarExecutorTrainingConfig::reference().with_trainable_surface(
        TassadarExecutorTrainableSurface::OutputHeadEmbeddingsAndSmallLearnedMixer,
    );
    baseline.run_id = String::from("tassadar-executor-sudoku-v0-supervision-baseline-v1");
    baseline.max_train_target_tokens_per_example = Some(128);
    baseline.max_eval_target_tokens_per_example = Some(128);
    baseline.curriculum_stages = vec![
        psionic_train::TassadarExecutorCurriculumStage::new("prompt_to_first_token", Some(1), 1),
        psionic_train::TassadarExecutorCurriculumStage::new("prompt_to_first_8_tokens", Some(8), 1),
        psionic_train::TassadarExecutorCurriculumStage::new(
            "prompt_to_first_32_tokens",
            Some(32),
            1,
        ),
    ];
    baseline.structural_supervision = TassadarExecutorStructuralSupervisionConfig::next_token_only();

    let mut structured = baseline.clone();
    structured.run_id = String::from("tassadar-executor-sudoku-v0-supervision-structured-v1");
    structured.structural_supervision =
        TassadarExecutorStructuralSupervisionConfig::structural_state_reference();

    vec![
        VariantSpec {
            variant_id: "next_token_only",
            description: "Bounded early-curriculum baseline with next-token-only loss weighting",
            config: baseline,
        },
        VariantSpec {
            variant_id: "structural_state_weighted",
            description:
                "Matched bounded early-curriculum run with extra instruction-pointer, branch, stack, and memory weighting",
            config: structured,
        },
    ]
}

fn build_family_deltas(
    variants: &[TassadarSupervisionAblationVariantReport],
) -> Vec<TassadarSupervisionAblationFamilyDelta> {
    let Some(baseline) = variants.iter().find(|variant| variant.variant_id == "next_token_only")
    else {
        return Vec::new();
    };
    let Some(candidate) = variants
        .iter()
        .find(|variant| variant.variant_id == "structural_state_weighted")
    else {
        return Vec::new();
    };
    let baseline_metrics = metric_map(baseline.structural_metrics.as_slice());
    let candidate_metrics = metric_map(candidate.structural_metrics.as_slice());
    baseline_metrics
        .into_iter()
        .map(|(family, baseline_metric)| {
            let candidate_metric = candidate_metrics
                .get(family.as_str())
                .cloned()
                .unwrap_or_else(|| baseline_metric.clone());
            TassadarSupervisionAblationFamilyDelta {
                family,
                baseline_exactness_bps: baseline_metric.exactness_bps,
                candidate_exactness_bps: candidate_metric.exactness_bps,
                delta_bps: candidate_metric.exactness_bps as i32
                    - baseline_metric.exactness_bps as i32,
            }
        })
        .collect()
}

fn build_summary(
    variants: &[TassadarSupervisionAblationVariantReport],
    deltas: &[TassadarSupervisionAblationFamilyDelta],
) -> String {
    let Some(candidate) = variants
        .iter()
        .find(|variant| variant.variant_id == "structural_state_weighted")
    else {
        return String::from("Structural supervision ablation did not materialize the candidate variant.");
    };
    let improvements = deltas
        .iter()
        .filter(|delta| delta.delta_bps > 0)
        .map(|delta| format!("{}:+{}", delta.family, delta.delta_bps))
        .collect::<Vec<_>>();
    if improvements.is_empty() {
        return format!(
            "Structured supervision changed the bounded learned lane without widening claims: the candidate variant stayed bounded with aggregate_bps={}, first_target_bps={}, first_32_bps={}, and no structural family improved over baseline.",
            candidate.aggregate_target_token_exactness_bps,
            candidate.first_target_exactness_bps,
            candidate.first_32_token_exactness_bps,
        );
    }
    format!(
        "Structured supervision changed the bounded learned lane while keeping claims bounded: aggregate_bps={}, first_target_bps={}, first_32_bps={}, family_improvements={}.",
        candidate.aggregate_target_token_exactness_bps,
        candidate.first_target_exactness_bps,
        candidate.first_32_token_exactness_bps,
        improvements.join(","),
    )
}

fn metric_map(
    metrics: &[TassadarExecutorStructuralSupervisionMetric],
) -> BTreeMap<String, TassadarExecutorStructuralSupervisionMetric> {
    metrics
        .iter()
        .cloned()
        .map(|metric| (metric.family.label().to_string(), metric))
        .collect()
}

fn relative_ref(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn read_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
) -> Result<T, TassadarSupervisionAblationError>
where
    T: DeserializeOwned,
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarSupervisionAblationError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSupervisionAblationError::Deserialize {
            artifact_kind: artifact_kind.to_string(),
            path: path.display().to_string(),
            error,
        }
    })
}

fn write_json<T>(
    path: PathBuf,
    artifact_kind: &str,
    value: &T,
) -> Result<(), TassadarSupervisionAblationError>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarSupervisionAblationError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(&path, bytes).map_err(|error| TassadarSupervisionAblationError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("tassadar supervision ablation value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}
