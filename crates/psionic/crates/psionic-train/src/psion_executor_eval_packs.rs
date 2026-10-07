use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable schema version for the executor eval-pack catalog.
pub const PSION_EXECUTOR_EVAL_PACK_CATALOG_SCHEMA_VERSION: &str =
    "psion.executor_eval_pack_catalog.v1";
/// Canonical fixture path for the executor eval-pack catalog.
pub const PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_eval_packs_v1.json";
/// Canonical doc path for the executor eval packs.
pub const PSION_EXECUTOR_EVAL_PACK_DOC_PATH: &str = "docs/PSION_EXECUTOR_EVAL_PACKS.md";

const PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_ACCEPTANCE_PROFILE.md";
const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const PSION_EXCLUSION_MANIFEST_FIXTURE_PATH: &str =
    "fixtures/psion/isolation/psion_exclusion_manifest_v1.json";
const TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json";
const TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_PATH: &str =
    "fixtures/tassadar/reports/tassadar_benchmark_package_set_summary.json";
const TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_evaluation_independence_audit_report.json";
const TASSADAR_STACK_BOUNDARY_DOC_PATH: &str =
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";

/// Typed executor eval-pack identifiers frozen in EPIC 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionExecutorEvalPackKind {
    Frequent,
    Promotion,
}

/// Typed suite classes used by the executor eval packs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionExecutorEvalSuiteClass {
    ExactnessCases,
    HeldOutExclusions,
    OperatorReviewCases,
    ThroughputBlockers,
    HeldOutSuite,
    AdversarialSuite,
    RuntimeBlockers,
    ServingBlockers,
    ReferenceLinearAnchorChecks,
    HullCacheFastRouteChecks,
}

/// Comparison direction for one executor throughput threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionExecutorEvalThresholdDirection {
    Minimum,
    Maximum,
}

/// Repo-owned authority artifact for one executor eval pack.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorEvalAuthorityArtifact {
    /// Repo-local path.
    pub path: String,
    /// Stable SHA256 over the artifact bytes.
    pub sha256: String,
    /// Why the artifact matters to the pack.
    pub detail: String,
}

/// One suite reference inside a frozen executor eval pack.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorEvalSuiteRef {
    /// Stable suite identifier.
    pub suite_id: String,
    /// Stable suite class.
    pub suite_class: PsionExecutorEvalSuiteClass,
    /// Repo-local source reference.
    pub source_ref: String,
    /// Stable case ids frozen into the suite when applicable.
    pub case_ids: Vec<String>,
    /// Stable metric ids frozen into the suite when applicable.
    pub metric_ids: Vec<String>,
    /// Whether a red status on this suite blocks the pack.
    pub required_for_green: bool,
    /// Short operator-facing detail.
    pub detail: String,
}

/// One throughput or latency threshold frozen for promotion review.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorEvalThreshold {
    /// Stable threshold id.
    pub threshold_id: String,
    /// Stable metric id.
    pub metric_id: String,
    /// Threshold direction.
    pub direction: PsionExecutorEvalThresholdDirection,
    /// Frozen threshold value.
    pub value: f64,
    /// Short explanation of the threshold.
    pub detail: String,
}

/// One typed executor eval pack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorEvalPack {
    /// Stable pack id such as `tassadar.eval.frequent.v0`.
    pub pack_id: String,
    /// Pack kind.
    pub pack_kind: PsionExecutorEvalPackKind,
    /// Admitted profile ids allowed to cite this pack.
    pub admitted_profile_ids: Vec<String>,
    /// Authority artifacts proving the pack boundary.
    pub authority_artifacts: Vec<PsionExecutorEvalAuthorityArtifact>,
    /// Frozen suite references.
    pub suite_refs: Vec<PsionExecutorEvalSuiteRef>,
    /// Frozen thresholds.
    pub thresholds: Vec<PsionExecutorEvalThreshold>,
    /// Acceptance profile reference when the pack is promotion-facing.
    pub acceptance_profile_ref: Option<String>,
    /// Explicit claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the pack.
    pub pack_digest: String,
}

/// Catalog of frozen executor eval packs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorEvalPackCatalog {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable catalog id.
    pub catalog_id: String,
    /// Ordered eval packs.
    pub packs: Vec<PsionExecutorEvalPack>,
    /// Short summary of the catalog.
    pub summary: String,
    /// Stable digest over the catalog.
    pub catalog_digest: String,
}

impl PsionExecutorEvalPackCatalog {
    /// Validate catalog structure and digests.
    pub fn validate(&self) -> Result<(), PsionExecutorEvalPackError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_eval_pack_catalog.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_EVAL_PACK_CATALOG_SCHEMA_VERSION {
            return Err(PsionExecutorEvalPackError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_EVAL_PACK_CATALOG_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.catalog_id.as_str(),
            "psion_executor_eval_pack_catalog.catalog_id",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_executor_eval_pack_catalog.summary",
        )?;
        if self.packs.is_empty() {
            return Err(PsionExecutorEvalPackError::MissingField {
                field: String::from("psion_executor_eval_pack_catalog.packs"),
            });
        }
        let mut seen_ids = BTreeSet::new();
        let mut seen_kinds = BTreeSet::new();
        for pack in &self.packs {
            pack.validate()?;
            if !seen_ids.insert(pack.pack_id.as_str()) {
                return Err(PsionExecutorEvalPackError::DuplicatePack {
                    pack_id: pack.pack_id.clone(),
                });
            }
            if !seen_kinds.insert(pack.pack_kind) {
                return Err(PsionExecutorEvalPackError::DuplicatePackKind {
                    pack_kind: format!("{:?}", pack.pack_kind),
                });
            }
        }
        if self.catalog_digest != stable_executor_eval_pack_catalog_digest(self) {
            return Err(PsionExecutorEvalPackError::DigestMismatch {
                kind: String::from("psion_executor_eval_pack_catalog"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorEvalPack {
    fn validate(&self) -> Result<(), PsionExecutorEvalPackError> {
        ensure_nonempty(self.pack_id.as_str(), "psion_executor_eval_pack.pack_id")?;
        if self.admitted_profile_ids.is_empty() {
            return Err(PsionExecutorEvalPackError::MissingField {
                field: format!(
                    "psion_executor_eval_pack[{}].admitted_profile_ids",
                    self.pack_id
                ),
            });
        }
        for profile_id in &self.admitted_profile_ids {
            ensure_nonempty(
                profile_id.as_str(),
                "psion_executor_eval_pack.admitted_profile_ids[]",
            )?;
        }
        if self.authority_artifacts.is_empty() {
            return Err(PsionExecutorEvalPackError::MissingField {
                field: format!(
                    "psion_executor_eval_pack[{}].authority_artifacts",
                    self.pack_id
                ),
            });
        }
        for artifact in &self.authority_artifacts {
            ensure_nonempty(
                artifact.path.as_str(),
                "psion_executor_eval_pack.authority_artifacts[].path",
            )?;
            ensure_nonempty(
                artifact.sha256.as_str(),
                "psion_executor_eval_pack.authority_artifacts[].sha256",
            )?;
            ensure_nonempty(
                artifact.detail.as_str(),
                "psion_executor_eval_pack.authority_artifacts[].detail",
            )?;
        }
        if self.suite_refs.is_empty() {
            return Err(PsionExecutorEvalPackError::MissingField {
                field: format!("psion_executor_eval_pack[{}].suite_refs", self.pack_id),
            });
        }
        let mut seen_suites = BTreeSet::new();
        let mut seen_classes = BTreeSet::new();
        for suite in &self.suite_refs {
            ensure_nonempty(
                suite.suite_id.as_str(),
                "psion_executor_eval_pack.suite_refs[].suite_id",
            )?;
            ensure_nonempty(
                suite.source_ref.as_str(),
                "psion_executor_eval_pack.suite_refs[].source_ref",
            )?;
            ensure_nonempty(
                suite.detail.as_str(),
                "psion_executor_eval_pack.suite_refs[].detail",
            )?;
            if !seen_suites.insert(suite.suite_id.as_str()) {
                return Err(PsionExecutorEvalPackError::DuplicateSuite {
                    pack_id: self.pack_id.clone(),
                    suite_id: suite.suite_id.clone(),
                });
            }
            seen_classes.insert(suite.suite_class);
        }
        for class in required_suite_classes(self.pack_kind) {
            if !seen_classes.contains(&class) {
                return Err(PsionExecutorEvalPackError::MissingRequiredSuiteClass {
                    pack_id: self.pack_id.clone(),
                    suite_class: format!("{:?}", class),
                });
            }
        }
        for threshold in &self.thresholds {
            ensure_nonempty(
                threshold.threshold_id.as_str(),
                "psion_executor_eval_pack.thresholds[].threshold_id",
            )?;
            ensure_nonempty(
                threshold.metric_id.as_str(),
                "psion_executor_eval_pack.thresholds[].metric_id",
            )?;
            ensure_nonempty(
                threshold.detail.as_str(),
                "psion_executor_eval_pack.thresholds[].detail",
            )?;
            if !(threshold.value.is_finite()) {
                return Err(PsionExecutorEvalPackError::InvalidThreshold {
                    pack_id: self.pack_id.clone(),
                    threshold_id: threshold.threshold_id.clone(),
                });
            }
        }
        match self.pack_kind {
            PsionExecutorEvalPackKind::Frequent => {
                if !self.thresholds.is_empty() {
                    return Err(PsionExecutorEvalPackError::UnexpectedThresholds {
                        pack_id: self.pack_id.clone(),
                    });
                }
                if self.acceptance_profile_ref.is_some() {
                    return Err(PsionExecutorEvalPackError::UnexpectedAcceptanceProfile {
                        pack_id: self.pack_id.clone(),
                    });
                }
            }
            PsionExecutorEvalPackKind::Promotion => {
                if self.thresholds.is_empty() {
                    return Err(PsionExecutorEvalPackError::MissingField {
                        field: format!("psion_executor_eval_pack[{}].thresholds", self.pack_id),
                    });
                }
                let acceptance_profile_ref =
                    self.acceptance_profile_ref.as_ref().ok_or_else(|| {
                        PsionExecutorEvalPackError::MissingField {
                            field: format!(
                                "psion_executor_eval_pack[{}].acceptance_profile_ref",
                                self.pack_id
                            ),
                        }
                    })?;
                ensure_nonempty(
                    acceptance_profile_ref.as_str(),
                    "psion_executor_eval_pack.acceptance_profile_ref",
                )?;
                if acceptance_profile_ref != PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH {
                    return Err(
                        PsionExecutorEvalPackError::UnexpectedAcceptanceProfilePath {
                            pack_id: self.pack_id.clone(),
                            actual: acceptance_profile_ref.clone(),
                        },
                    );
                }
            }
        }
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "psion_executor_eval_pack.claim_boundary",
        )?;
        if self.pack_digest != stable_executor_eval_pack_digest(self) {
            return Err(PsionExecutorEvalPackError::DigestMismatch {
                kind: format!("psion_executor_eval_pack.{}", self.pack_id),
            });
        }
        Ok(())
    }
}

/// Errors surfaced while building or validating executor eval packs.
#[derive(Debug, Error)]
pub enum PsionExecutorEvalPackError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("schema version mismatch: expected `{expected}`, got `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("duplicate pack `{pack_id}`")]
    DuplicatePack { pack_id: String },
    #[error("duplicate pack kind `{pack_kind}`")]
    DuplicatePackKind { pack_kind: String },
    #[error("duplicate suite `{suite_id}` in pack `{pack_id}`")]
    DuplicateSuite { pack_id: String, suite_id: String },
    #[error("pack `{pack_id}` is missing required suite class `{suite_class}`")]
    MissingRequiredSuiteClass {
        pack_id: String,
        suite_class: String,
    },
    #[error("pack `{pack_id}` has invalid threshold `{threshold_id}`")]
    InvalidThreshold {
        pack_id: String,
        threshold_id: String,
    },
    #[error("frequent pack `{pack_id}` unexpectedly carried thresholds")]
    UnexpectedThresholds { pack_id: String },
    #[error("frequent pack `{pack_id}` unexpectedly carried an acceptance profile ref")]
    UnexpectedAcceptanceProfile { pack_id: String },
    #[error("promotion pack `{pack_id}` referenced unexpected acceptance profile path `{actual}`")]
    UnexpectedAcceptanceProfilePath { pack_id: String, actual: String },
    #[error("digest mismatch for `{kind}`")]
    DigestMismatch { kind: String },
}

/// Build the current canonical executor eval-pack catalog.
pub fn builtin_executor_eval_pack_catalog(
    workspace_root: &Path,
) -> Result<PsionExecutorEvalPackCatalog, PsionExecutorEvalPackError> {
    let packs = vec![
        builtin_executor_frequent_eval_pack(workspace_root)?,
        builtin_executor_promotion_eval_pack(workspace_root)?,
    ];
    let mut catalog = PsionExecutorEvalPackCatalog {
        schema_version: String::from(PSION_EXECUTOR_EVAL_PACK_CATALOG_SCHEMA_VERSION),
        catalog_id: String::from("psion_executor_eval_packs_v1"),
        packs,
        summary: String::from(
            "Phase-one executor eval-pack catalog freezing the checkpoint-time frequent pack and the promotion pack on top of the retained article benchmark, isolation, and acceptance surfaces.",
        ),
        catalog_digest: String::new(),
    };
    catalog.catalog_digest = stable_executor_eval_pack_catalog_digest(&catalog);
    catalog.validate()?;
    Ok(catalog)
}

/// Write the current executor eval-pack catalog fixture.
pub fn write_builtin_executor_eval_pack_catalog(
    workspace_root: &Path,
) -> Result<PsionExecutorEvalPackCatalog, PsionExecutorEvalPackError> {
    let catalog = builtin_executor_eval_pack_catalog(workspace_root)?;
    let fixture_path = workspace_root.join(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH);
    if let Some(parent) = fixture_path.parent() {
        fs::create_dir_all(parent).map_err(|error| PsionExecutorEvalPackError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    fs::write(&fixture_path, serde_json::to_vec_pretty(&catalog)?).map_err(|error| {
        PsionExecutorEvalPackError::Write {
            path: fixture_path.display().to_string(),
            error,
        }
    })?;
    Ok(catalog)
}

fn builtin_executor_frequent_eval_pack(
    workspace_root: &Path,
) -> Result<PsionExecutorEvalPack, PsionExecutorEvalPackError> {
    let authority_artifacts = vec![
        authority_artifact(
            workspace_root,
            TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH,
            "Retained article benchmark report carrying the admitted workload targets, exactness hooks, reference_linear anchor posture, and future hull_cache throughput metrics.",
        )?,
        authority_artifact(
            workspace_root,
            TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_PATH,
            "Retained benchmark-package summary carrying the public case-family vocabulary used by the executor workload family.",
        )?,
        authority_artifact(
            workspace_root,
            PSION_EXCLUSION_MANIFEST_FIXTURE_PATH,
            "Retained held-out isolation manifest proving later executor decisions stay tied to the same exclusion and near-duplicate review boundary.",
        )?,
        authority_artifact(
            workspace_root,
            PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH,
            "Retained local-profile reference proving the operator review packet still depends on admitted machine and control-plane profiles instead of a second launcher story.",
        )?,
    ];
    let mut pack = PsionExecutorEvalPack {
        pack_id: String::from("tassadar.eval.frequent.v0"),
        pack_kind: PsionExecutorEvalPackKind::Frequent,
        admitted_profile_ids: vec![
            String::from("local_mac_mlx_aarch64"),
            String::from("local_4080_cuda_tailnet_x86_64"),
            String::from("local_tailnet_cluster_control_plane"),
        ],
        authority_artifacts,
        suite_refs: vec![
            PsionExecutorEvalSuiteRef {
                suite_id: String::from("frequent_exactness_cases_v0"),
                suite_class: PsionExecutorEvalSuiteClass::ExactnessCases,
                source_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH),
                case_ids: vec![
                    String::from("micro_wasm_kernel"),
                    String::from("branch_heavy_kernel"),
                    String::from("memory_heavy_kernel"),
                    String::from("long_loop_kernel"),
                    String::from("sudoku_v0_test_a"),
                    String::from("hungarian_matching"),
                ],
                metric_ids: vec![
                    String::from("final_output_exactness_bps"),
                    String::from("step_exactness_bps"),
                    String::from("halt_exactness_bps"),
                ],
                required_for_green: true,
                detail: String::from(
                    "Checkpoint-time exactness pack covering the currently admitted executor workloads before a longer promotion review is even considered.",
                ),
            },
            PsionExecutorEvalSuiteRef {
                suite_id: String::from("frequent_held_out_exclusions_v0"),
                suite_class: PsionExecutorEvalSuiteClass::HeldOutExclusions,
                source_ref: String::from(PSION_EXCLUSION_MANIFEST_FIXTURE_PATH),
                case_ids: vec![
                    String::from("spec_quiz_eval_pack_v1"),
                    String::from("vendor_manual_private_scan_v1"),
                    String::from("forum_scrape_misc_001"),
                ],
                metric_ids: vec![],
                required_for_green: true,
                detail: String::from(
                    "Frequent decisions stay tied to the same held-out and training-excluded source boundary so checkpoint-time review cannot silently widen the corpus.",
                ),
            },
            PsionExecutorEvalSuiteRef {
                suite_id: String::from("frequent_operator_review_cases_v0"),
                suite_class: PsionExecutorEvalSuiteClass::OperatorReviewCases,
                source_ref: String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
                case_ids: vec![
                    String::from("artifact_packet_complete"),
                    String::from("checkpoint_restore_rehearsal_green"),
                    String::from("export_smoke_green"),
                    String::from("local_cluster_roundtrip_green"),
                ],
                metric_ids: vec![],
                required_for_green: true,
                detail: String::from(
                    "Checkpoint-time review remains explicit about artifact packet completeness, restore rehearsal, export smoke, and the local-cluster roundtrip instead of reducing the pack to pure task accuracy.",
                ),
            },
            PsionExecutorEvalSuiteRef {
                suite_id: String::from("frequent_throughput_blockers_v0"),
                suite_class: PsionExecutorEvalSuiteClass::ThroughputBlockers,
                source_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH),
                case_ids: vec![
                    String::from("micro_wasm_kernel"),
                    String::from("branch_heavy_kernel"),
                    String::from("memory_heavy_kernel"),
                    String::from("hungarian_matching"),
                ],
                metric_ids: vec![
                    String::from("tassadar.reference_linear_steps_per_second"),
                    String::from("tassadar.hull_cache_steps_per_second"),
                    String::from("tassadar.hull_cache_speedup_over_reference_linear"),
                    String::from("tassadar.hull_cache_remaining_gap_vs_cpu_reference"),
                ],
                required_for_green: true,
                detail: String::from(
                    "Frequent review treats throughput regressions as blockers when the fast route or anchor path falls far enough that later promotion claims would become misleading.",
                ),
            },
        ],
        thresholds: vec![],
        acceptance_profile_ref: None,
        claim_boundary: String::from(
            "This pack freezes the checkpoint-time executor review surface only: admitted exactness cases, the held-out exclusion boundary, explicit operator review cases, and throughput blockers. It does not by itself decide promotion.",
        ),
        pack_digest: String::new(),
    };
    pack.pack_digest = stable_executor_eval_pack_digest(&pack);
    pack.validate()?;
    Ok(pack)
}

fn builtin_executor_promotion_eval_pack(
    workspace_root: &Path,
) -> Result<PsionExecutorEvalPack, PsionExecutorEvalPackError> {
    let authority_artifacts = vec![
        authority_artifact(
            workspace_root,
            TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH,
            "Retained article benchmark report carrying exactness, reference_linear, hull_cache, and throughput evidence on the admitted workload family.",
        )?,
        authority_artifact(
            workspace_root,
            TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH,
            "Retained evaluation-independence audit carrying the frozen held-out and adversarial executor rows.",
        )?,
        authority_artifact(
            workspace_root,
            PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH,
            "Retained acceptance profile defining the executor promotion gates that this pack must satisfy.",
        )?,
        authority_artifact(
            workspace_root,
            TASSADAR_STACK_BOUNDARY_DOC_PATH,
            "Retained stack-boundary doc proving runtime, export, and article-route ownership stay explicit during promotion review.",
        )?,
    ];
    let mut pack = PsionExecutorEvalPack {
        pack_id: String::from("tassadar.eval.promotion.v0"),
        pack_kind: PsionExecutorEvalPackKind::Promotion,
        admitted_profile_ids: vec![
            String::from("local_mac_mlx_aarch64"),
            String::from("local_4080_cuda_tailnet_x86_64"),
            String::from("local_tailnet_cluster_control_plane"),
        ],
        authority_artifacts,
        suite_refs: vec![
            PsionExecutorEvalSuiteRef {
                suite_id: String::from("promotion_exactness_suite_v0"),
                suite_class: PsionExecutorEvalSuiteClass::ExactnessCases,
                source_ref: String::from(TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_PATH),
                case_ids: vec![
                    String::from("micro_wasm_kernel"),
                    String::from("branch_heavy_kernel"),
                    String::from("memory_heavy_kernel"),
                    String::from("long_loop_kernel"),
                    String::from("sudoku_v0_test_a"),
                    String::from("sudoku_v0_test_b"),
                    String::from("hungarian_matching"),
                ],
                metric_ids: vec![
                    String::from("final_output_exactness_bps"),
                    String::from("step_exactness_bps"),
                    String::from("halt_exactness_bps"),
                ],
                required_for_green: true,
                detail: String::from(
                    "Promotion exactness suite spans the admitted executor workload family and keeps exactness green as the first non-negotiable gate.",
                ),
            },
            PsionExecutorEvalSuiteRef {
                suite_id: String::from("promotion_held_out_suite_v0"),
                suite_class: PsionExecutorEvalSuiteClass::HeldOutSuite,
                source_ref: String::from(TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH),
                case_ids: vec![
                    String::from("micro_wasm_kernel"),
                    String::from("branch_heavy_kernel"),
                    String::from("memory_heavy_kernel"),
                    String::from("randomized_sudoku_v0_holdout_a"),
                    String::from("randomized_hungarian_v0_holdout_a"),
                ],
                metric_ids: vec![
                    String::from("held_out_exactness_bps"),
                    String::from("held_out_regression_count"),
                ],
                required_for_green: true,
                detail: String::from(
                    "Promotion held-out suite keeps the learned lane honest on non-train rows before any executor candidate can move forward.",
                ),
            },
            PsionExecutorEvalSuiteRef {
                suite_id: String::from("promotion_adversarial_suite_v0"),
                suite_class: PsionExecutorEvalSuiteClass::AdversarialSuite,
                source_ref: String::from(TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH),
                case_ids: vec![
                    String::from("adversarial_sudoku_9x9_clustered_a"),
                    String::from("adversarial_hungarian_10x10_permuted_a"),
                ],
                metric_ids: vec![String::from("adversarial_regression_count")],
                required_for_green: true,
                detail: String::from(
                    "Promotion adversarial suite keeps article-scale hostile variants explicit instead of letting smaller exactness wins override them.",
                ),
            },
            PsionExecutorEvalSuiteRef {
                suite_id: String::from("promotion_runtime_blockers_v0"),
                suite_class: PsionExecutorEvalSuiteClass::RuntimeBlockers,
                source_ref: String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
                case_ids: vec![
                    String::from("cpu_matrix_green"),
                    String::from("checkpoint_restore_green"),
                    String::from("local_cluster_roundtrip_green"),
                ],
                metric_ids: vec![],
                required_for_green: true,
                detail: String::from(
                    "Promotion runtime blockers keep CPU validation, restore rehearsal, and the Mac -> 4080 -> Mac roundtrip green before promotion is even reviewable.",
                ),
            },
            PsionExecutorEvalSuiteRef {
                suite_id: String::from("promotion_serving_blockers_v0"),
                suite_class: PsionExecutorEvalSuiteClass::ServingBlockers,
                source_ref: String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
                case_ids: vec![
                    String::from("export_packet_green"),
                    String::from("replacement_packet_green"),
                    String::from("promoted_artifact_compatible"),
                    String::from("shadow_rollback_safe"),
                ],
                metric_ids: vec![],
                required_for_green: true,
                detail: String::from(
                    "Promotion serving blockers keep export, replacement, promoted-artifact compatibility, and rollback safety explicit instead of treating runtime quality as enough by itself.",
                ),
            },
            PsionExecutorEvalSuiteRef {
                suite_id: String::from("promotion_reference_linear_anchor_checks_v0"),
                suite_class: PsionExecutorEvalSuiteClass::ReferenceLinearAnchorChecks,
                source_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH),
                case_ids: vec![
                    String::from("micro_wasm_kernel"),
                    String::from("branch_heavy_kernel"),
                    String::from("memory_heavy_kernel"),
                    String::from("hungarian_matching"),
                ],
                metric_ids: vec![String::from("tassadar.reference_linear_steps_per_second")],
                required_for_green: true,
                detail: String::from(
                    "Promotion still requires `reference_linear` as the measured baseline truth anchor instead of letting fast-route wins erase the floor.",
                ),
            },
            PsionExecutorEvalSuiteRef {
                suite_id: String::from("promotion_hull_cache_fast_route_checks_v0"),
                suite_class: PsionExecutorEvalSuiteClass::HullCacheFastRouteChecks,
                source_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH),
                case_ids: vec![
                    String::from("micro_wasm_kernel"),
                    String::from("branch_heavy_kernel"),
                    String::from("memory_heavy_kernel"),
                    String::from("hungarian_matching"),
                ],
                metric_ids: vec![
                    String::from("tassadar.hull_cache_steps_per_second"),
                    String::from("tassadar.hull_cache_speedup_over_reference_linear"),
                    String::from("tassadar.hull_cache_remaining_gap_vs_cpu_reference"),
                ],
                required_for_green: true,
                detail: String::from(
                    "Promotion fast-route checks keep admitted-workload `hull_cache` explicit as the fast-route target while preserving the `reference_linear` anchor.",
                ),
            },
        ],
        thresholds: vec![
            PsionExecutorEvalThreshold {
                threshold_id: String::from("promotion_hull_cache_speedup_floor_v0"),
                metric_id: String::from("tassadar.hull_cache_speedup_over_reference_linear"),
                direction: PsionExecutorEvalThresholdDirection::Minimum,
                value: 1.5,
                detail: String::from(
                    "Promotion requires the admitted fast route to stay materially faster than `reference_linear`; this floor stays below the weakest retained current workload result instead of overfitting to one case.",
                ),
            },
            PsionExecutorEvalThreshold {
                threshold_id: String::from("promotion_hull_cache_cpu_gap_ceiling_v0"),
                metric_id: String::from("tassadar.hull_cache_remaining_gap_vs_cpu_reference"),
                direction: PsionExecutorEvalThresholdDirection::Maximum,
                value: 3.0,
                detail: String::from(
                    "Promotion keeps the admitted fast route within one frozen remaining-gap ceiling against the CPU reference so throughput wins do not hide a degraded execution surface.",
                ),
            },
        ],
        acceptance_profile_ref: Some(String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH)),
        claim_boundary: String::from(
            "This pack freezes the first executor promotion review surface: exactness, held-out and adversarial suites, runtime and serving blockers, `reference_linear` anchor truth, admitted-workload `hull_cache` fast-route checks, and the first frozen throughput thresholds. It does not widen the workload family beyond the retained executor lane.",
        ),
        pack_digest: String::new(),
    };
    pack.pack_digest = stable_executor_eval_pack_digest(&pack);
    pack.validate()?;
    Ok(pack)
}

fn authority_artifact(
    workspace_root: &Path,
    rel_path: &str,
    detail: &str,
) -> Result<PsionExecutorEvalAuthorityArtifact, PsionExecutorEvalPackError> {
    Ok(PsionExecutorEvalAuthorityArtifact {
        path: String::from(rel_path),
        sha256: sha256_for_path(workspace_root.join(rel_path))?,
        detail: String::from(detail),
    })
}

fn required_suite_classes(
    pack_kind: PsionExecutorEvalPackKind,
) -> &'static [PsionExecutorEvalSuiteClass] {
    match pack_kind {
        PsionExecutorEvalPackKind::Frequent => &[
            PsionExecutorEvalSuiteClass::ExactnessCases,
            PsionExecutorEvalSuiteClass::HeldOutExclusions,
            PsionExecutorEvalSuiteClass::OperatorReviewCases,
            PsionExecutorEvalSuiteClass::ThroughputBlockers,
        ],
        PsionExecutorEvalPackKind::Promotion => &[
            PsionExecutorEvalSuiteClass::ExactnessCases,
            PsionExecutorEvalSuiteClass::HeldOutSuite,
            PsionExecutorEvalSuiteClass::AdversarialSuite,
            PsionExecutorEvalSuiteClass::RuntimeBlockers,
            PsionExecutorEvalSuiteClass::ServingBlockers,
            PsionExecutorEvalSuiteClass::ReferenceLinearAnchorChecks,
            PsionExecutorEvalSuiteClass::HullCacheFastRouteChecks,
        ],
    }
}

fn sha256_for_path(path: PathBuf) -> Result<String, PsionExecutorEvalPackError> {
    let bytes = fs::read(&path).map_err(|error| PsionExecutorEvalPackError::Read {
        path: path.display().to_string(),
        error,
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorEvalPackError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorEvalPackError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_executor_eval_pack_digest(pack: &PsionExecutorEvalPack) -> String {
    let mut clone = pack.clone();
    clone.pack_digest.clear();
    stable_json_digest(&clone)
}

fn stable_executor_eval_pack_catalog_digest(catalog: &PsionExecutorEvalPackCatalog) -> String {
    let mut clone = catalog.clone();
    clone.catalog_digest.clear();
    stable_json_digest(&clone)
}

fn stable_json_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("executor eval-pack digest serialization");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .map(PathBuf::from)
            .expect("workspace root")
    }

    #[test]
    fn builtin_executor_eval_pack_catalog_matches_committed_fixture() {
        let root = workspace_root();
        let built = builtin_executor_eval_pack_catalog(&root).expect("built catalog");
        let fixture: PsionExecutorEvalPackCatalog = serde_json::from_slice(
            &fs::read(root.join(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH))
                .expect("fixture bytes"),
        )
        .expect("fixture json");
        assert_eq!(built, fixture);
    }

    #[test]
    fn builtin_executor_eval_pack_catalog_is_valid() {
        let root = workspace_root();
        let catalog = builtin_executor_eval_pack_catalog(&root).expect("catalog");
        catalog.validate().expect("catalog should validate");
        assert_eq!(catalog.packs.len(), 2);
        assert_eq!(catalog.packs[0].pack_id, "tassadar.eval.frequent.v0");
        assert_eq!(catalog.packs[1].pack_id, "tassadar.eval.promotion.v0");
    }
}
