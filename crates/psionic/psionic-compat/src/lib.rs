//! Machine-readable semantics-claim vocabulary and evidence aggregation.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "compatibility claim vocabulary and semantics evidence aggregation";

/// Frozen upstream MLX release window used for Psionic MLX parity claims.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MlxUpstreamVersionWindow {
    /// Canonical upstream repository name for the bounded oracle window.
    pub upstream_repository: String,
    /// Lowest upstream release tag included in the current window.
    pub minimum_inclusive_tag: String,
    /// Highest upstream release tag included in the current window.
    pub maximum_inclusive_tag: String,
    /// Informational local checkout commit used during the initial roadmap review.
    pub review_checkout_commit: String,
    /// Human-readable `git describe` string for that review checkout.
    pub review_checkout_describe: String,
    /// Date the informative review checkout was inspected.
    pub review_checkout_date: String,
}

/// One machine-readable MLX claim-language term and its required boundaries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MlxCompatibilityTerm {
    /// Stable claim-language identifier.
    pub term_id: String,
    /// Plain-language summary of what the term means inside Psionic.
    pub summary: String,
    /// Properties the term requires before the language is allowed in docs or review.
    pub required_properties: Vec<String>,
    /// Shortcuts the term explicitly forbids.
    pub forbidden_shortcuts: Vec<String>,
}

/// Aggregate posture for the current MLX acceptance matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MlxAcceptanceMatrixPosture {
    /// The matrix exists as the canonical closure contract, but the categories are
    /// still mainly tracking future implementation work.
    TrackingOnly,
}

/// Status vocabulary for one MLX acceptance category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MlxAcceptanceCategoryStatus {
    /// The category has full declared coverage for the current MLX lane.
    Implemented,
    /// The category is runnable and real, but still intentionally bounded.
    ImplementedEarly,
    /// Some substantial part of the category exists, but central work remains open.
    Partial,
    /// A meaningful part exists outside the intended owner split and must move inward.
    PartialOutsidePsionic,
    /// The category is tracked explicitly but not implemented yet.
    Planned,
}

/// One category in the MLX-lane acceptance matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MlxAcceptanceCategory {
    /// Stable matrix category identifier.
    pub category_id: String,
    /// Current matrix status for the category.
    pub matrix_status: MlxAcceptanceCategoryStatus,
    /// Owning roadmap epic identifier.
    pub epic_id: String,
    /// Roadmap or GitHub issue references that govern the category.
    pub issue_refs: Vec<String>,
    /// What a green category would honestly mean.
    pub green_definition: String,
    /// Current repo truth for the category.
    pub current_repo_truth: String,
    /// Boundary note that keeps current claims honest.
    pub boundary_note: String,
}

/// Outcome for one seeded MLX parity-harness family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MlxParityHarnessOutcome {
    /// The current Psionic hook can legitimately stand in as a seeded pass for the family.
    Pass,
    /// The current Psionic hook proves the family still refuses explicitly.
    Refusal,
    /// No current Psionic hook can honestly claim the family yet.
    Unsupported,
}

/// One seeded upstream MLX family carried by the parity harness manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MlxParityHarnessFamily {
    /// Stable family identifier.
    pub family_id: String,
    /// MLX acceptance category this family informs.
    pub acceptance_category: String,
    /// Upstream MLX source files mirrored by this family.
    pub upstream_sources: Vec<String>,
    /// Current seeded outcome for the family.
    pub current_outcome: MlxParityHarnessOutcome,
    /// Repo-owned hook commands that justify the current seeded outcome.
    pub psionic_hook_commands: Vec<String>,
    /// Plain-language summary of the current seeded claim.
    pub summary: String,
    /// Boundary note for what this seeded family does not prove yet.
    pub boundary_note: String,
}

/// Compatibility posture for one MLX-facing surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MlxCompatibilityMatrixStatus {
    /// The bounded surface is available and truthfully supported today.
    Supported,
    /// The surface is not native MLX support yet, but there is a bounded Psionic-native bridge.
    Convertible,
    /// The surface remains intentionally unsupported today.
    Unsupported,
}

/// One row in the bounded MLX compatibility matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MlxCompatibilityMatrixEntry {
    /// Stable surface identifier.
    pub surface_id: String,
    /// Current compatibility posture for the surface.
    pub matrix_status: MlxCompatibilityMatrixStatus,
    /// Plain-language summary of the current posture.
    pub summary: String,
    /// Repo-owned reports, hooks, or contracts that justify the current posture.
    pub evidence_refs: Vec<String>,
    /// Open issue references that would move the posture forward.
    pub blocking_issue_refs: Vec<String>,
    /// Boundary note that keeps current claims honest.
    pub boundary_note: String,
}

/// Aggregate machine-readable contract for Psionic's bounded MLX claim language.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MlxCompatibilityScopeReport {
    /// Stable schema version for the report.
    pub schema_version: u32,
    /// Versioned MLX claim vocabulary window.
    pub claim_vocabulary_version: String,
    /// Frozen upstream MLX release window for current parity claims.
    pub upstream_version_window: MlxUpstreamVersionWindow,
    /// Allowed MLX claim-language terms and their required boundaries.
    pub compatibility_terms: Vec<MlxCompatibilityTerm>,
    /// Global rules that keep later parity or compatibility work version-bounded.
    pub explicit_rules: Vec<String>,
    /// Stable digest over the report contents.
    pub report_digest: String,
}

/// Aggregate machine-readable acceptance report for the MLX roadmap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MlxAcceptanceMatrixReport {
    /// Stable schema version for the report.
    pub schema_version: u32,
    /// Canonical JSON schema path for this report.
    pub schema_path: String,
    /// Canonical runner path for this report.
    pub runner: String,
    /// Whether this report is a tracking contract or a green acceptance proof.
    pub report_posture: MlxAcceptanceMatrixPosture,
    /// Category ids included in this report instance.
    pub selected_categories: Vec<String>,
    /// Acceptance categories carried by the report.
    pub categories: Vec<MlxAcceptanceCategory>,
    /// Stable digest over the report contents.
    pub report_digest: String,
}

/// Aggregate machine-readable seeded parity-harness report for the MLX roadmap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MlxParityHarnessReport {
    /// Stable schema version for the report.
    pub schema_version: u32,
    /// Canonical JSON schema path for this report.
    pub schema_path: String,
    /// Canonical runner path for this report.
    pub runner: String,
    /// Frozen upstream oracle window used by the seeded families.
    pub oracle_window: String,
    /// Family ids included in this report instance.
    pub selected_families: Vec<String>,
    /// Seeded upstream MLX test families carried by the report.
    pub families: Vec<MlxParityHarnessFamily>,
    /// Stable digest over the report contents.
    pub report_digest: String,
}

/// Aggregate machine-readable compatibility matrix for the MLX roadmap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MlxCompatibilityMatrixReport {
    /// Stable schema version for the report.
    pub schema_version: u32,
    /// Canonical JSON schema path for this report.
    pub schema_path: String,
    /// Canonical runner path for this report.
    pub runner: String,
    /// Frozen upstream oracle window used by the matrix.
    pub oracle_window: String,
    /// Surface ids included in this report instance.
    pub selected_surfaces: Vec<String>,
    /// Compatibility rows carried by the report.
    pub surfaces: Vec<MlxCompatibilityMatrixEntry>,
    /// Stable digest over the report contents.
    pub report_digest: String,
}

impl MlxAcceptanceMatrixReport {
    fn new(categories: Vec<MlxAcceptanceCategory>) -> Self {
        let selected_categories = categories
            .iter()
            .map(|category| category.category_id.clone())
            .collect::<Vec<_>>();
        Self::from_selected_categories(categories, selected_categories)
    }

    fn from_selected_categories(
        categories: Vec<MlxAcceptanceCategory>,
        selected_categories: Vec<String>,
    ) -> Self {
        let schema_path =
            String::from("crates/psionic/docs/mlx_acceptance_matrix_report.schema.json");
        let runner = String::from("scripts/release/check-psionic-mlx-acceptance-matrix.sh");
        let report_posture = MlxAcceptanceMatrixPosture::TrackingOnly;
        let report_digest = stable_mlx_acceptance_matrix_report_digest(
            schema_path.as_str(),
            runner.as_str(),
            report_posture,
            &selected_categories,
            &categories,
        );
        Self {
            schema_version: 1,
            schema_path,
            runner,
            report_posture,
            selected_categories,
            categories,
            report_digest,
        }
    }

    /// Returns a filtered report with only the named categories.
    pub fn filter_to_categories(
        &self,
        category_ids: &[String],
    ) -> Result<Self, MlxAcceptanceMatrixError> {
        if category_ids.is_empty() {
            return Ok(self.clone());
        }

        let mut filtered_categories = Vec::with_capacity(category_ids.len());
        for category_id in category_ids {
            let category = self
                .categories
                .iter()
                .find(|category| category.category_id == *category_id)
                .cloned()
                .ok_or_else(|| MlxAcceptanceMatrixError::UnknownCategory(category_id.clone()))?;
            filtered_categories.push(category);
        }

        Ok(Self::from_selected_categories(
            filtered_categories,
            category_ids.to_vec(),
        ))
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("schema_path={}", self.schema_path),
            format!("runner={}", self.runner),
            format!("report_posture={:?}", self.report_posture),
            format!("report_digest={}", self.report_digest),
        ];
        for category in &self.categories {
            lines.push(format!(
                "{}|{:?}",
                category.category_id, category.matrix_status
            ));
        }
        lines
    }
}

impl MlxParityHarnessReport {
    fn new(families: Vec<MlxParityHarnessFamily>) -> Self {
        let selected_families = families
            .iter()
            .map(|family| family.family_id.clone())
            .collect::<Vec<_>>();
        Self::from_selected_families(families, selected_families)
    }

    fn from_selected_families(
        families: Vec<MlxParityHarnessFamily>,
        selected_families: Vec<String>,
    ) -> Self {
        let schema_path = String::from("crates/psionic/docs/mlx_parity_harness_report.schema.json");
        let runner = String::from("scripts/release/check-psionic-mlx-parity-harness.sh");
        let oracle_window = String::from("ml-explore/mlx:v0.31.0..v0.31.1:seed_v0");
        let report_digest = stable_mlx_parity_harness_report_digest(
            schema_path.as_str(),
            runner.as_str(),
            oracle_window.as_str(),
            &selected_families,
            &families,
        );
        Self {
            schema_version: 1,
            schema_path,
            runner,
            oracle_window,
            selected_families,
            families,
            report_digest,
        }
    }

    /// Returns a filtered report with only the named families.
    pub fn filter_to_families(&self, family_ids: &[String]) -> Result<Self, MlxParityHarnessError> {
        if family_ids.is_empty() {
            return Ok(self.clone());
        }

        let mut filtered_families = Vec::with_capacity(family_ids.len());
        for family_id in family_ids {
            let family = self
                .families
                .iter()
                .find(|family| family.family_id == *family_id)
                .cloned()
                .ok_or_else(|| MlxParityHarnessError::UnknownFamily(family_id.clone()))?;
            filtered_families.push(family);
        }

        Ok(Self::from_selected_families(
            filtered_families,
            family_ids.to_vec(),
        ))
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("schema_path={}", self.schema_path),
            format!("runner={}", self.runner),
            format!("oracle_window={}", self.oracle_window),
            format!("report_digest={}", self.report_digest),
        ];
        for family in &self.families {
            lines.push(format!("{}|{:?}", family.family_id, family.current_outcome));
        }
        lines
    }
}

impl MlxCompatibilityMatrixReport {
    fn new(surfaces: Vec<MlxCompatibilityMatrixEntry>) -> Self {
        let selected_surfaces = surfaces
            .iter()
            .map(|surface| surface.surface_id.clone())
            .collect::<Vec<_>>();
        Self::from_selected_surfaces(surfaces, selected_surfaces)
    }

    fn from_selected_surfaces(
        surfaces: Vec<MlxCompatibilityMatrixEntry>,
        selected_surfaces: Vec<String>,
    ) -> Self {
        let schema_path =
            String::from("crates/psionic/docs/mlx_compatibility_matrix_report.schema.json");
        let runner = String::from("scripts/release/check-psionic-mlx-compatibility-matrix.sh");
        let oracle_window = String::from("ml-explore/mlx:v0.31.0..v0.31.1:matrix_v0");
        let report_digest = stable_mlx_compatibility_matrix_report_digest(
            schema_path.as_str(),
            runner.as_str(),
            oracle_window.as_str(),
            &selected_surfaces,
            &surfaces,
        );
        Self {
            schema_version: 1,
            schema_path,
            runner,
            oracle_window,
            selected_surfaces,
            surfaces,
            report_digest,
        }
    }

    /// Returns a filtered report with only the named surfaces.
    pub fn filter_to_surfaces(
        &self,
        surface_ids: &[String],
    ) -> Result<Self, MlxCompatibilityMatrixError> {
        if surface_ids.is_empty() {
            return Ok(self.clone());
        }

        let mut filtered_surfaces = Vec::with_capacity(surface_ids.len());
        for surface_id in surface_ids {
            let surface = self
                .surfaces
                .iter()
                .find(|surface| surface.surface_id == *surface_id)
                .cloned()
                .ok_or_else(|| MlxCompatibilityMatrixError::UnknownSurface(surface_id.clone()))?;
            filtered_surfaces.push(surface);
        }

        Ok(Self::from_selected_surfaces(
            filtered_surfaces,
            surface_ids.to_vec(),
        ))
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("schema_path={}", self.schema_path),
            format!("runner={}", self.runner),
            format!("oracle_window={}", self.oracle_window),
            format!("report_digest={}", self.report_digest),
        ];
        for surface in &self.surfaces {
            lines.push(format!(
                "{}|{:?}",
                surface.surface_id, surface.matrix_status
            ));
        }
        lines
    }
}

impl MlxCompatibilityScopeReport {
    fn new(
        upstream_version_window: MlxUpstreamVersionWindow,
        compatibility_terms: Vec<MlxCompatibilityTerm>,
        explicit_rules: Vec<String>,
    ) -> Self {
        let claim_vocabulary_version = String::from("mlx_claim_v1");
        let report_digest = stable_mlx_compatibility_scope_report_digest(
            claim_vocabulary_version.as_str(),
            &upstream_version_window,
            &compatibility_terms,
            &explicit_rules,
        );
        Self {
            schema_version: 1,
            claim_vocabulary_version,
            upstream_version_window,
            compatibility_terms,
            explicit_rules,
            report_digest,
        }
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("claim_vocabulary_version={}", self.claim_vocabulary_version),
            format!(
                "upstream_repository={}",
                self.upstream_version_window.upstream_repository
            ),
            format!(
                "minimum_inclusive_tag={}",
                self.upstream_version_window.minimum_inclusive_tag
            ),
            format!(
                "maximum_inclusive_tag={}",
                self.upstream_version_window.maximum_inclusive_tag
            ),
            format!(
                "review_checkout_commit={}",
                self.upstream_version_window.review_checkout_commit
            ),
            format!("report_digest={}", self.report_digest),
        ];
        for term in &self.compatibility_terms {
            lines.push(format!("term={}", term.term_id));
        }
        lines
    }
}

/// Stable claim posture for one PyTorch-facing semantics area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticsClaimPosture {
    /// The area has enough breadth and evidence to make a bounded PyTorch-credible claim.
    PyTorchCredible,
    /// The area has seeded machine-legible evidence, but not enough breadth to claim credibility.
    SeededEvidenceOnly,
    /// The area remains an explicit future compatibility target rather than a current credibility claim.
    PyTorchCompatibleLater,
}

/// Failure returned while filtering the MLX acceptance matrix.
#[derive(Debug, Error)]
pub enum MlxAcceptanceMatrixError {
    /// The requested category id is not declared in the builtin matrix.
    #[error("unknown MLX acceptance category: {0}")]
    UnknownCategory(String),
}

/// Failure returned while filtering the MLX parity harness.
#[derive(Debug, Error)]
pub enum MlxParityHarnessError {
    /// The requested family id is not declared in the builtin harness.
    #[error("unknown MLX parity family: {0}")]
    UnknownFamily(String),
}

/// Failure returned while filtering the MLX compatibility matrix.
#[derive(Debug, Error)]
pub enum MlxCompatibilityMatrixError {
    /// The requested surface id is not declared in the builtin matrix.
    #[error("unknown MLX compatibility surface: {0}")]
    UnknownSurface(String),
}

/// One evidence reference that supports the current posture for a claim area.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticsEvidenceRef {
    /// Stable evidence artifact identifier.
    pub artifact_id: String,
    /// Stable digest of the evidence artifact.
    pub artifact_digest: String,
}

impl SemanticsEvidenceRef {
    fn new(artifact_id: impl Into<String>, artifact_digest: impl Into<String>) -> Self {
        Self {
            artifact_id: artifact_id.into(),
            artifact_digest: artifact_digest.into(),
        }
    }
}

/// One machine-readable semantics claim area in the PyTorch-facing vocabulary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticsClaimArea {
    /// Stable claim-area identifier.
    pub area_id: String,
    /// Current posture for the claim area.
    pub current_posture: SemanticsClaimPosture,
    /// Plain-language scope boundary for the current posture.
    pub bounded_scope: String,
    /// Evidence artifacts that justify the current posture.
    pub evidence_refs: Vec<SemanticsEvidenceRef>,
    /// Concrete blockers that prevent a stronger claim today.
    pub blockers: Vec<String>,
    /// Open issue references that track those blockers when one exists.
    pub blocking_issue_refs: Vec<String>,
}

/// Aggregate machine-readable claim report for Psionic semantics posture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticsClaimReport {
    /// Stable schema version for the claim report.
    pub schema_version: u32,
    /// Versioned claim vocabulary window.
    pub claim_vocabulary_version: String,
    /// Overall semantics posture for the current report.
    pub overall_posture: SemanticsClaimPosture,
    /// Claim areas carried by the report.
    pub areas: Vec<SemanticsClaimArea>,
    /// Stable digest over the report contents.
    pub report_digest: String,
}

impl SemanticsClaimReport {
    fn new(areas: Vec<SemanticsClaimArea>) -> Self {
        let claim_vocabulary_version = String::from("pytorch_claim_v1");
        let report_digest =
            stable_semantics_claim_report_digest(claim_vocabulary_version.as_str(), &areas);
        Self {
            schema_version: 1,
            claim_vocabulary_version,
            overall_posture: SemanticsClaimPosture::SeededEvidenceOnly,
            areas,
            report_digest,
        }
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("claim_vocabulary_version={}", self.claim_vocabulary_version),
            format!("overall_posture={:?}", self.overall_posture),
            format!("report_digest={}", self.report_digest),
        ];
        for area in &self.areas {
            lines.push(format!("{}|{:?}", area.area_id, area.current_posture));
        }
        lines
    }
}

/// Failure returned while aggregating semantics-claim evidence.
#[derive(Debug, Error)]
pub enum SemanticsClaimError {
    /// One operator-parity artifact failed to build.
    #[error(transparent)]
    Operator(#[from] psionic_ir::GraphError),
    /// One module-parity artifact failed to build.
    #[error(transparent)]
    Module(#[from] psionic_nn::ModuleStateError),
    /// One optimizer-parity artifact failed to build.
    #[error(transparent)]
    Optimizer(#[from] psionic_train::TrainingOptimizerError),
    /// One compiler-hygiene artifact failed to build.
    #[error(transparent)]
    Compiler(#[from] psionic_compiler::CompilerHygieneParityError),
    /// One export/deployment artifact semantics report failed to build.
    #[error(transparent)]
    ExportDeployment(#[from] psionic_compiler::ExportDeploymentArtifactSemanticsError),
}

/// Builds the canonical semantics-claim report for the current Psionic
/// PyTorch-facing posture.
pub fn builtin_semantics_claim_report() -> Result<SemanticsClaimReport, SemanticsClaimError> {
    let operator = psionic_ir::builtin_operator_parity_matrix_report()?;
    let advanced_operator_programs = psionic_ir::builtin_advanced_operator_program_matrix_report()?;
    let extensions = psionic_ir::builtin_extension_contract_semantics_report();
    let program_transforms = psionic_ir::builtin_program_transform_capability_matrix_report();
    let data_ingress = psionic_data::builtin_data_ingress_semantics_report();
    let distributed_data_feed = psionic_data::builtin_distributed_data_feed_semantics_report();
    let tensor_families = psionic_ir::builtin_tensor_family_capability_matrix_report();
    let advanced_dtypes = psionic_core::builtin_advanced_dtype_semantics_report();
    let autocast = psionic_core::builtin_autocast_policy_matrix_report();
    let quantization = psionic_core::builtin_quantization_capability_semantics_report();
    let module = psionic_nn::builtin_module_parity_matrix_report()?;
    let optimizer = psionic_train::builtin_optimizer_parity_matrix_report()?;
    let gradient_scaling = psionic_train::builtin_gradient_scaling_semantics_report();
    let reproducibility = psionic_train::builtin_reproducibility_semantics_report();
    let compiler = psionic_compiler::builtin_compiler_hygiene_parity_matrix_report()?;
    let export_deployment =
        psionic_compiler::builtin_export_deployment_artifact_semantics_report()?;

    Ok(SemanticsClaimReport::new(vec![
        seeded_area(
            "operator_semantics",
            "Current claim is bounded to the seeded OpInfo-style matrix over add, mul, matmul, reshape, permute, concat, scaled_dot_product_attention, and one explicit backend-capability refusal.",
            vec![SemanticsEvidenceRef::new(
                "operator_parity_matrix",
                operator.matrix_digest,
            )],
            vec![
                String::from("broaden operator-family breadth beyond the current seeded cases"),
                String::from("add wider dtype and device cartesian coverage"),
                String::from(
                    "prove runtime numerical parity beyond meta-execution shape contracts",
                ),
            ],
            vec![String::from("#3726")],
        ),
        seeded_area(
            "advanced_operator_family_semantics",
            "Current claim is bounded to reusable linalg gram-matrix, signal Fourier-projection, and rotary-attention residual programs above the compact core plus explicit refusal for distribution and special-function families that still require broader normalization, sampling, or special-function primitives.",
            vec![SemanticsEvidenceRef::new(
                "advanced_operator_program_matrix",
                advanced_operator_programs.matrix_digest,
            )],
            vec![
                String::from(
                    "broaden advanced operator-family coverage beyond the current linalg, signal, and attention seed programs",
                ),
                String::from(
                    "land concrete distribution and special-function primitives instead of refusal-only family placeholders",
                ),
            ],
            Vec::new(),
        ),
        seeded_area(
            "program_transform_semantics",
            "Current claim is bounded to functionalization, symbolic-rewrite readiness, and export-safe graph handoff over graphs with explicit alias-root and barrier reporting, plus explicit future refusal for `vmap`, `jvp`, and `jacobian`.",
            vec![SemanticsEvidenceRef::new(
                "program_transform_capability_matrix",
                program_transforms.matrix_digest,
            )],
            vec![
                String::from(
                    "broaden transform capability beyond functionalization and export-safe graph readiness",
                ),
                String::from(
                    "connect export-safe graph capability to deployment artifact contracts",
                ),
            ],
            Vec::new(),
        ),
        seeded_area(
            "export_and_deployment_semantics",
            "Current claim is bounded to export-safe functionalized graph contracts with named entry signatures plus deployment artifact contracts over execution-plan and topology-aware bundles, including explicit refusal for opaque backend-extension graphs and graph-digest mismatches.",
            vec![SemanticsEvidenceRef::new(
                "export_deployment_artifact_semantics",
                export_deployment.report_digest,
            )],
            vec![
                String::from(
                    "broaden export and deployment semantics beyond the current execution-plan and topology-aware bundle window",
                ),
                String::from(
                    "connect checkpoint migration and broader plugin distribution to the graph-first deployment contracts",
                ),
            ],
            Vec::new(),
        ),
        seeded_area(
            "extension_contract_semantics",
            "Current claim is bounded to typed custom-op, custom-kernel, custom-autograd, backend-plugin, and quantizer-plugin contracts above the existing extensible registry, plus explicit refusal for contracts that bypass declared-output or non-dense quantization-mode requirements.",
            vec![SemanticsEvidenceRef::new(
                "extension_contract_semantics",
                extensions.report_digest,
            )],
            vec![
                String::from(
                    "broaden extension semantics beyond the current typed contract bundles",
                ),
                String::from("connect extension contracts to deployment and distribution surfaces"),
            ],
            Vec::new(),
        ),
        seeded_area(
            "data_ingress_semantics",
            "Current claim is bounded to reusable local data-ingress contracts covering map-style and iterable-streaming dataset access, sequential and deterministic-shuffle sampling, batch-sampler policy above packing contracts, and direct-host or pinned-prefetch staging into one target device lane.",
            vec![SemanticsEvidenceRef::new(
                "data_ingress_semantics",
                data_ingress.report_digest,
            )],
            vec![String::from(
                "broaden data ingress beyond the current local source, sampler, and staging window",
            )],
            Vec::new(),
        ),
        seeded_area(
            "distributed_data_feed_semantics",
            "Current claim is bounded to fixed-world-size distributed sampler partitioning over shard-ordered local ingress contracts, with contiguous-block and rank-strided shard assignment, explicit epoch-barrier or fixed-cadence step-barrier worker coordination, runtime-derived per-rank replay generators, and explicit refusal for elastic membership.",
            vec![SemanticsEvidenceRef::new(
                "distributed_data_feed_semantics",
                distributed_data_feed.report_digest,
            )],
            vec![
                String::from(
                    "broaden distributed data-feed semantics beyond the current fixed-world-size shard partitioning window",
                ),
                String::from(
                    "connect elastic membership and topology revision to replay-safe worker ordering",
                ),
            ],
            Vec::new(),
        ),
        seeded_area(
            "tensor_family_semantics",
            "Current claim is bounded to first-class sparse, nested, masked, and storage-aware tensor-family contracts, their meta-execution and declared-output capability matrix, and explicit refusal proofs for non-dense runtime materialization.",
            vec![SemanticsEvidenceRef::new(
                "tensor_family_capability_matrix",
                tensor_families.matrix_digest,
            )],
            vec![
                String::from("land runtime materialization for non-dense tensor families"),
                String::from(
                    "broaden tensor-family behavior beyond meta, alias, and declared-output contracts",
                ),
                String::from(
                    "connect broader operator-family and export semantics to these non-dense families",
                ),
            ],
            Vec::new(),
        ),
        seeded_area(
            "advanced_dtype_semantics",
            "Current claim is bounded to seeded advanced-dtype promotion, cast, and backend-capability rules over complex, float8, wider integer, and higher-precision real dtypes, plus an explicit bridge back down to the compact runtime-core `DType` subset.",
            vec![SemanticsEvidenceRef::new(
                "advanced_dtype_semantics",
                advanced_dtypes.report_digest,
            )],
            vec![
                String::from(
                    "broaden promotion and cast coverage beyond the current seeded matrix",
                ),
                String::from(
                    "connect broader operator-family semantics to the richer dtype vocabulary",
                ),
                String::from(
                    "materialize additional dtypes beyond the compact runtime-core subset",
                ),
            ],
            Vec::new(),
        ),
        seeded_area(
            "reproducibility_semantics",
            "Current claim is bounded to framework-wide replay seed discipline, strict trainer and eval runtime contracts, stable local-device and distributed-rank generator derivation, and checkpoint-stable RNG restore across the current runtime and training replay substrate.",
            vec![SemanticsEvidenceRef::new(
                "reproducibility_semantics",
                reproducibility.report_digest,
            )],
            vec![
                String::from(
                    "broaden reproducibility coverage beyond the current seeded runtime and replay cases",
                ),
                String::from(
                    "connect mixed-precision and distributed data-feed semantics to the replayable RNG contract",
                ),
                String::from(
                    "extend checkpointed RNG restore deeper into later train-loop and export surfaces",
                ),
            ],
            Vec::new(),
        ),
        seeded_area(
            "precision_policy_semantics",
            "Current claim is bounded to seeded autocast-style precision-policy rules over backend family, preferred low-precision dtype, operator family, and numerics diagnostics plus train-class gradient scaling, overflow, and underflow handling over the bounded fp16 dynamic-scaling and bf16 no-scaling window.",
            vec![
                SemanticsEvidenceRef::new("autocast_policy_matrix", autocast.report_digest),
                SemanticsEvidenceRef::new(
                    "gradient_scaling_semantics",
                    gradient_scaling.report_digest,
                ),
            ],
            vec![
                String::from(
                    "broaden autocast and train-class mixed-precision coverage beyond the current seeded fp16 and bf16 cases",
                ),
                String::from(
                    "connect distributed data-feed semantics and wider operator-family coverage to the mixed-precision surface",
                ),
                String::from(
                    "extend backend capability truth beyond the current bounded runtime-vs-meta split",
                ),
            ],
            Vec::new(),
        ),
        seeded_area(
            "quantization_semantics",
            "Current claim is bounded to seeded PTQ, QAT, runtime-execution, compiler-lowering, and export-aware quantization capability semantics above raw file-format decode, plus explicit refusal for unsupported block-quant QAT and broader runtime activation-dtype closure.",
            vec![SemanticsEvidenceRef::new(
                "quantization_capability_semantics",
                quantization.report_digest,
            )],
            vec![
                String::from(
                    "broaden quantization coverage beyond the current int8 and ggml_q4_0 seeded cases",
                ),
                String::from("connect quantization capability to extension and plugin contracts"),
                String::from(
                    "land deployment-facing export artifacts on top of the export-aware quantization surface",
                ),
            ],
            Vec::new(),
        ),
        seeded_area(
            "module_and_state_semantics",
            "Current claim is bounded to normalized module-tree and state_dict semantics for linear, batch_norm1d, and one nested transformer-style fixture plus one explicit registration-order refusal proof.",
            vec![SemanticsEvidenceRef::new(
                "module_parity_matrix",
                module.matrix_digest,
            )],
            vec![
                String::from("prove forward numerics parity for standard modules"),
                String::from("support registration-order-preserving serialization where required"),
                String::from("widen module-family breadth beyond the current seeded fixtures"),
            ],
            Vec::new(),
        ),
        seeded_area(
            "optimizer_semantics",
            "Current claim is bounded to single-step optimizer semantics for SGD, Adam, AdamW, LARS, and LAMB plus one explicit state-kind refusal proof.",
            vec![SemanticsEvidenceRef::new(
                "optimizer_parity_matrix",
                optimizer.matrix_digest,
            )],
            vec![
                String::from("broaden parameter-group and multi-step optimizer parity coverage"),
                String::from(
                    "add broader mixed-precision and distributed optimizer claim surfaces",
                ),
                String::from("add wider scheduler parity coverage"),
            ],
            vec![String::from("#3734")],
        ),
        seeded_area(
            "compiler_hygiene_and_fake_tensor",
            "Current claim is bounded to seeded fake-tensor graph-vs-plan parity, non-dense meta-tensor contracts, cache-temperature hygiene, alias-aware memory planning, and one explicit symbolic-shape refusal proof.",
            vec![SemanticsEvidenceRef::new(
                "compiler_hygiene_parity_matrix",
                compiler.matrix_digest,
            )],
            vec![
                String::from("land symbolic-shape environments and guard simplification"),
                String::from("land export breadth above the current fake/meta slice"),
            ],
            Vec::new(),
        ),
        future_area(
            "checkpoint_and_model_io_interop",
            "Portable model-IO and compatibility contracts exist, but PyTorch-credible checkpoint migration breadth is not claimed yet.",
            vec![
                String::from(
                    "broaden state-dict and checkpoint migration beyond the current bounded formats",
                ),
                String::from(
                    "prove practical migration behavior over dense and sharded checkpoint families",
                ),
            ],
            Vec::new(),
        ),
        future_area(
            "advanced_tensor_dtype_and_precision",
            "Broader mixed-precision behavior and train-class precision control remain explicit future compatibility targets after landing bounded tensor-family, dtype, reproducibility, autocast, and gradient-scaling seed coverage.",
            vec![String::from(
                "broaden mixed-precision and train-class precision control beyond the current seeded fp16/bf16 window",
            )],
            Vec::new(),
        ),
        future_area(
            "extensions_and_plugins",
            "Broader extension and plugin distribution, loading, and deployment behavior remain future compatibility targets after landing bounded typed contracts.",
            vec![String::from(
                "connect extension contracts to deployment and distribution surfaces",
            )],
            Vec::new(),
        ),
    ]))
}

/// Builds the canonical MLX compatibility scope report for the current Psionic
/// MLX-facing program.
#[must_use]
pub fn builtin_mlx_compatibility_scope_report() -> MlxCompatibilityScopeReport {
    MlxCompatibilityScopeReport::new(
        MlxUpstreamVersionWindow {
            upstream_repository: String::from("ml-explore/mlx"),
            minimum_inclusive_tag: String::from("v0.31.0"),
            maximum_inclusive_tag: String::from("v0.31.1"),
            review_checkout_commit: String::from("ea91bd02cf0671f3fe6ddaf746812c27bf05154e"),
            review_checkout_describe: String::from("v0.31.1-7-gea91bd02"),
            review_checkout_date: String::from("2026-03-16"),
        },
        vec![
            MlxCompatibilityTerm {
                term_id: String::from("mlx_class"),
                summary: String::from(
                    "A Rust-native Psionic framework surface that targets the bounded MLX release window semantically while keeping runtime, artifact, receipt, and train truth Psionic-owned end to end.",
                ),
                required_properties: vec![
                    String::from(
                        "implemented natively inside crates/psionic/* without delegating runtime behavior to upstream MLX, Python, or FFI proxy layers",
                    ),
                    String::from(
                        "cites the frozen inclusive upstream MLX release window when making parity, acceptance, or adoption claims",
                    ),
                    String::from(
                        "reuses Psionic runtime, compiler, train, and artifact truth instead of bypassing them with lane-local compatibility code",
                    ),
                ],
                forbidden_shortcuts: vec![
                    String::from(
                        "calling a thin wrapper over upstream MLX itself a Psionic MLX-class implementation",
                    ),
                    String::from(
                        "using compatibility or import shells as a substitute for missing native framework semantics",
                    ),
                    String::from(
                        "making unversioned claims such as MLX-like, MLX equivalent, or MLX compatible without naming the bounded window",
                    ),
                ],
            },
            MlxCompatibilityTerm {
                term_id: String::from("mlx_compatible"),
                summary: String::from(
                    "A later bounded facade, import/export path, naming shim, or migration surface built on top of the native Psionic MLX-class substrate.",
                ),
                required_properties: vec![
                    String::from(
                        "ships only after a native Psionic-owned MLX-class substrate exists for the claimed area",
                    ),
                    String::from(
                        "publishes explicit supported, convertible, and unsupported behavior for the same frozen upstream MLX release window",
                    ),
                    String::from(
                        "keeps refusal behavior and lossy conversions machine-legible rather than implying full upstream closure",
                    ),
                ],
                forbidden_shortcuts: vec![
                    String::from(
                        "describing a compatibility facade as proof that the underlying native semantics are complete",
                    ),
                    String::from(
                        "using compatibility language to widen the bounded release window or to imply tip-of-tree MLX coverage",
                    ),
                    String::from(
                        "claiming upstream test or notebook coverage without routing that evidence through the repo-owned parity harness",
                    ),
                ],
            },
        ],
        vec![
            String::from(
                "The frozen upstream MLX release window is inclusive v0.31.0 through v0.31.1 from ml-explore/mlx.",
            ),
            String::from(
                "The local audit checkout commit ea91bd02cf0671f3fe6ddaf746812c27bf05154e (v0.31.1-7-gea91bd02, observed 2026-03-16) informed roadmap review but does not widen the supported release window.",
            ),
            String::from(
                "Every later MLX parity artifact, acceptance matrix, or compatibility shell must cite this frozen window or update this report first.",
            ),
            String::from(
                "Psionic must not claim MLX-identical or unversioned MLX-compatible behavior from roadmap text, demos, or wrapper code alone.",
            ),
        ],
    )
}

/// Builds the canonical MLX acceptance matrix report for the current Psionic
/// MLX roadmap.
#[must_use]
pub fn builtin_mlx_acceptance_matrix_report() -> MlxAcceptanceMatrixReport {
    MlxAcceptanceMatrixReport::new(vec![
        MlxAcceptanceCategory {
            category_id: String::from("array-runtime-surface"),
            matrix_status: MlxAcceptanceCategoryStatus::ImplementedEarly,
            epic_id: String::from("PMLX-E1"),
            issue_refs: vec![
                String::from("PMLX-101 (#3834)"),
                String::from("PMLX-102 (#3835)"),
                String::from("PMLX-103 (#3836)"),
                String::from("PMLX-104 (#3837)"),
                String::from("PMLX-105 (#3838)"),
                String::from("PMLX-106 (#3839)"),
            ],
            green_definition: String::from(
                "A public lazy array type exists with explicit eval and async_eval posture, public device and stream handles, creation and view families, deterministic random and cast behavior, and safe host materialization boundaries.",
            ),
            current_repo_truth: String::from(
                "Psionic now publishes a first user-facing lazy-array facade in psionic-array with runtime-backed device handles, honest unified-memory flags, explicit stream-dependency policy, graph-backed arithmetic, explicit eval and deferred async_eval, replay-stable eval receipts, scalar and filled-array creation helpers, reshape/permute/transpose/slice/select/concat/broadcast_to view families, explicit seeded or best-effort random-uniform and random-normal creation, logical dtype casts, arange/linspace/eye helpers, explicit host-owned typed buffer export, singleton item extraction, and deterministic tree flatten/map/unflatten utilities.",
            ),
            boundary_note: String::from(
                "Treat this as supported early array/runtime closure only; it does not imply transform, nn, export, or distributed MLX closure.",
            ),
        },
        MlxAcceptanceCategory {
            category_id: String::from("transform-compile"),
            matrix_status: MlxAcceptanceCategoryStatus::ImplementedEarly,
            epic_id: String::from("PMLX-E2"),
            issue_refs: vec![
                String::from("PMLX-201 (#3840)"),
                String::from("PMLX-202 (#3841)"),
                String::from("PMLX-203 (#3842)"),
                String::from("PMLX-204 (#3843)"),
                String::from("PMLX-205 (#3844)"),
                String::from("PMLX-206 (#3845)"),
            ],
            green_definition: String::from(
                "The public transform layer exposes grad, value_and_grad, vjp, jvp, vmap, checkpoint, compile-as-transform, and explicit symbolic or shapeless compile boundaries with typed refusals.",
            ),
            current_repo_truth: String::from(
                "Psionic now exposes a first public transform layer in psionic-ir with stable grad, value_and_grad, vjp, jvp, vmap, checkpoint, and graph-registry-backed custom_vjp objects above AutodiffGraph, plus typed target validation, zero-cotangent materialization for disconnected reverse-mode targets, dense f32 tangent propagation for primitive forward-mode graphs, per-lane reference vectorization over selected graph inputs, checkpoint replay that retains only the requested output then replays backward-plan primal bindings, graph-digest plus reverse-signature keyed transform-hook registration, and explicit cast plus backend-extension refusal for the current vmap and checkpoint coverage; psionic-compiler now adds a first public compile transform with explicit enable/disable posture, purity declaration, cache reuse versus bypass versus explicit invalidation control, cache-identity versus trace-family trace capture, plan-debug output, a bounded shapeless trace-family identity over same-rank primitive graphs, and explicit reshape/expand plus opaque-op refusal for the current symbolic boundary.",
            ),
            boundary_note: String::from(
                "Do not infer full MLX transform closure from the current reverse-plus-forward plus bounded-vmap plus checkpoint plus compile-transform slice; jacobian remains outside the current bounded surface, shapeless compile is still intentionally narrower than a full symbolic-shape environment, and custom_vjp is still graph-scoped rather than a broad plugin-distribution story.",
            ),
        },
        MlxAcceptanceCategory {
            category_id: String::from("nn-optimizer"),
            matrix_status: MlxAcceptanceCategoryStatus::ImplementedEarly,
            epic_id: String::from("PMLX-E3"),
            issue_refs: vec![
                String::from("PMLX-301 (#3846)"),
                String::from("PMLX-302 (#3847)"),
                String::from("PMLX-303 (#3848)"),
                String::from("PMLX-304 (#3849)"),
                String::from("PMLX-305 (#3850)"),
                String::from("PMLX-306 (#3851)"),
                String::from("PMLX-307 (#3852)"),
            ],
            green_definition: String::from(
                "The MLX lane exposes a public Module tree, state save and load behavior, core layers, losses, initializers, optimizers, schedulers, and quantized-module semantics above Psionic-native training primitives.",
            ),
            current_repo_truth: String::from(
                "Psionic now exposes a first public module tree in psionic-nn, including explicit parameter versus buffer registration, trainable versus frozen posture, recursive parameter discovery with filtered trainable or frozen views, deterministic state-tree/state-dict behavior, bounded public save_weights/load_weights wrappers with strict-by-default plus explicit non-strict load posture, a bounded CPU-reference core layer surface spanning linear, embedding, layer_norm, rms_norm, activation, dropout, conv1d, conv2d, pool1d, and pool2d families, bounded CPU-reference loss, initializer, and helper functions including mse_loss, l1_loss, binary_cross_entropy_loss, cross_entropy_loss, softmax_last_dim, log_softmax_last_dim, sigmoid, one_hot, init_tensor, and init_parameter, plus a bounded public optimizer shell with module-path keyed state, scheduler bindings, parameter-group scaling semantics, and multi-optimizer composition built above psionic-train optimizer and scheduler primitives; psionic-nn now also exposes an eval-oriented quantized module shell with Module::quantize, explicit quantize reports, and QuantizedLinear plus QuantizedEmbedding wrappers over int8_symmetric block storage and dequantize-to-f32 forward semantics.",
            ),
            boundary_note: String::from(
                "Treat this as supported early nn closure only: the public surface now covers registration, freeze posture, module-state save/load semantics, core layer numerics, reusable CPU-reference losses and initializers, a path-keyed optimizer plus scheduler shell with parameter-group composition, and a first eval-oriented quantized wrapper slice for linear and embedding families; it does not yet imply broad quantized training closure, conv/norm quantized wrappers, or export-format quantization parity.",
            ),
        },
        MlxAcceptanceCategory {
            category_id: String::from("export-serialization-tooling"),
            matrix_status: MlxAcceptanceCategoryStatus::Planned,
            epic_id: String::from("PMLX-E4"),
            issue_refs: vec![
                String::from("PMLX-401 (#3853)"),
                String::from("PMLX-402 (#3854)"),
                String::from("PMLX-403 (#3855)"),
                String::from("PMLX-404 (#3856)"),
                String::from("PMLX-405 (#3857)"),
                String::from("PMLX-406 (#3858)"),
            ],
            green_definition: String::from(
                "The MLX lane supports general array IO, a Psionic-native function export/import artifact, bounded .mlxfn compatibility, memory and cache controls, backend debug hooks, and extension-facing tooling.",
            ),
            current_repo_truth: String::from(
                "Psionic now has a first public array-IO slice in psionic-array-io with stable receipts plus bounded npy, npz, safetensors, and dense GGUF save/load above psionic-array, and a first native .psifn function-artifact slice in psionic-function-io with export-safe graph contracts, optional compiler artifacts, trace-family identity, deployment bundle binding, and stable import/export receipts, while .mlxfn compatibility, memory controls, debug hooks, and extension tooling remain open.",
            ),
            boundary_note: String::from(
                "Do not collapse model loaders, deployment artifacts, or internal runtime diagnostics into MLX export or tooling closure; general array IO and the native .psifn function artifact now exist, but the rest of the public export/tooling shell is still missing.",
            ),
        },
        MlxAcceptanceCategory {
            category_id: String::from("distributed-semantics"),
            matrix_status: MlxAcceptanceCategoryStatus::Planned,
            epic_id: String::from("PMLX-E5"),
            issue_refs: vec![
                String::from("PMLX-501 (#3859)"),
                String::from("PMLX-502 (#3860)"),
                String::from("PMLX-503 (#3861)"),
                String::from("PMLX-504 (#3862)"),
                String::from("PMLX-505 (#3863)"),
                String::from("PMLX-506 (#3864)"),
                String::from("PMLX-507 (#3865)"),
            ],
            green_definition: String::from(
                "The MLX lane publishes distributed group, collective, gradient-reduction, tensor-parallel, FSDP-style update, and launch/topology helpers above Psionic collectives and cluster truth.",
            ),
            current_repo_truth: String::from(
                "Psionic owns collectives, cluster, and distributed optimizer substrate, but it does not yet expose framework-level MLX distributed helpers or launch contracts.",
            ),
            boundary_note: String::from(
                "Do not infer MLX distributed closure from lower-level collectives or cluster internals until the framework-visible group and helper APIs are real.",
            ),
        },
        MlxAcceptanceCategory {
            category_id: String::from("backend-closure"),
            matrix_status: MlxAcceptanceCategoryStatus::Planned,
            epic_id: String::from("PMLX-E6"),
            issue_refs: vec![
                String::from("PMLX-601 (#3866)"),
                String::from("PMLX-602 (#3867)"),
                String::from("PMLX-603 (#3868)"),
                String::from("PMLX-604 (#3869)"),
                String::from("PMLX-605 (#3870)"),
                String::from("PMLX-606 (#3871)"),
                String::from("PMLX-607 (#3872)"),
                String::from("PMLX-608 (#3873)"),
            ],
            green_definition: String::from(
                "CPU reference, Metal, and CUDA backends honestly cover the declared MLX surface, the parity harness carries the upstream MLX test families, and any compatibility or binding shells stay explicitly bounded.",
            ),
            current_repo_truth: String::from(
                "Psionic has real backend-specific substrate and the first MLX version window contract, but it does not yet have MLX-class backend closure, parity-harness evidence, or compatibility-shell boundaries above that substrate.",
            ),
            boundary_note: String::from(
                "Do not claim bounded MLX backend or compatibility closure from one backend lane, one demo, or one local checkout until the declared categories have parity evidence and explicit shell boundaries.",
            ),
        },
    ])
}

/// Builds the canonical seeded MLX parity-harness report for the current
/// Psionic MLX roadmap.
#[must_use]
pub fn builtin_mlx_parity_harness_report() -> MlxParityHarnessReport {
    MlxParityHarnessReport::new(vec![
        MlxParityHarnessFamily {
            family_id: String::from("array_core"),
            acceptance_category: String::from("array-runtime-surface"),
            upstream_sources: vec![
                String::from("tests/array_tests.cpp"),
                String::from("python/tests/test_array.py"),
                String::from("python/tests/test_constants.py"),
                String::from("python/tests/test_bf16.py"),
                String::from("python/tests/test_double.py"),
            ],
            current_outcome: MlxParityHarnessOutcome::Unsupported,
            psionic_hook_commands: vec![
                String::from(
                    "cargo test -p psionic-array tests::public_lazy_array_surface_builds_graph_backed_arithmetic -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-array tests::public_lazy_array_creation_and_view_families_materialize -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-array tests::public_lazy_array_random_cast_and_common_creation_families_stay_seeded -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-array tests::public_lazy_array_host_interop_and_item_access_stay_explicit -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-array tests::public_lazy_array_tree_utilities_preserve_structure_and_refuse_bad_unflatten -- --exact --nocapture",
                ),
            ],
            summary: String::from(
                "The upstream array-core family is still tracked as unsupported, but psionic-array now provides the first public lazy-array entrypoint plus standard creation, deterministic random, cast, host-interop, item, and tree families for later parity work.",
            ),
            boundary_note: String::from(
                "A first public array facade with host/item/tree coverage exists, but that is still not enough to call the upstream array-core family ported.",
            ),
        },
        MlxParityHarnessFamily {
            family_id: String::from("ops_numeric"),
            acceptance_category: String::from("array-runtime-surface"),
            upstream_sources: vec![
                String::from("tests/ops_tests.cpp"),
                String::from("tests/creations_tests.cpp"),
                String::from("tests/arg_reduce_tests.cpp"),
                String::from("tests/einsum_tests.cpp"),
                String::from("tests/random_tests.cpp"),
                String::from("python/tests/test_ops.py"),
                String::from("python/tests/test_reduce.py"),
                String::from("python/tests/test_einsum.py"),
                String::from("python/tests/test_random.py"),
            ],
            current_outcome: MlxParityHarnessOutcome::Unsupported,
            psionic_hook_commands: vec![
                String::from(
                    "cargo test -p psionic-array tests::public_lazy_array_surface_builds_graph_backed_arithmetic -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-array tests::public_lazy_array_creation_and_view_families_materialize -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-array tests::public_lazy_array_random_cast_and_common_creation_families_stay_seeded -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-array tests::public_lazy_array_host_interop_and_item_access_stay_explicit -- --exact --nocapture",
                ),
            ],
            summary: String::from(
                "The numeric-op and creation families remain tracked but unsupported even though psionic-array now exposes graph-backed arithmetic plus common creation, deterministic random, logical dtype-cast coverage, and explicit host or singleton export boundaries.",
            ),
            boundary_note: String::from(
                "The first public numeric ops plus host/item coverage are still not the same thing as a seeded upstream MLX numeric parity family.",
            ),
        },
        MlxParityHarnessFamily {
            family_id: String::from("device_eval_memory"),
            acceptance_category: String::from("backend-closure"),
            upstream_sources: vec![
                String::from("tests/device_tests.cpp"),
                String::from("tests/eval_tests.cpp"),
                String::from("tests/allocator_tests.cpp"),
                String::from("tests/gpu_tests.cpp"),
                String::from("tests/scheduler_tests.cpp"),
                String::from("python/tests/test_device.py"),
                String::from("python/tests/test_eval.py"),
                String::from("python/tests/test_memory.py"),
            ],
            current_outcome: MlxParityHarnessOutcome::Unsupported,
            psionic_hook_commands: vec![
                String::from(
                    "cargo test -p psionic-array tests::public_lazy_array_device_handles_preserve_unified_memory_truth -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-array tests::public_lazy_array_streams_report_dependency_policy_honestly -- --exact --nocapture",
                ),
            ],
            summary: String::from(
                "Device, eval, scheduler, and memory families remain named and unsupported even though psionic-array now exposes public device and stream handles with honest unified-memory and dependency-policy truth.",
            ),
            boundary_note: String::from(
                "Public device and stream handles are still not the same thing as MLX-class allocator, scheduler, or runtime-memory parity.",
            ),
        },
        MlxParityHarnessFamily {
            family_id: String::from("autograd"),
            acceptance_category: String::from("transform-compile"),
            upstream_sources: vec![
                String::from("tests/autograd_tests.cpp"),
                String::from("python/tests/test_autograd.py"),
            ],
            current_outcome: MlxParityHarnessOutcome::Pass,
            psionic_hook_commands: vec![
                String::from(
                    "cargo test -p psionic-ir autodiff::tests::public_reverse_mode_transforms_expose_grad_value_and_grad_and_vjp -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-ir autodiff::tests::public_forward_mode_jvp_exposes_value_and_tangent -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-ir autodiff::tests::reverse_mode_autodiff_materializes_matmul_chain_gradients -- --exact --nocapture",
                ),
            ],
            summary: String::from(
                "The seeded autograd family can now point at a public transform layer with grad, value_and_grad, vjp, and jvp plus the existing reference tests as a bounded parity anchor.",
            ),
            boundary_note: String::from(
                "This is only a seeded reverse-plus-forward pass, not a claim that the full public MLX transform API is complete.",
            ),
        },
        MlxParityHarnessFamily {
            family_id: String::from("vmap"),
            acceptance_category: String::from("transform-compile"),
            upstream_sources: vec![
                String::from("tests/vmap_tests.cpp"),
                String::from("python/tests/test_vmap.py"),
            ],
            current_outcome: MlxParityHarnessOutcome::Pass,
            psionic_hook_commands: vec![
                String::from(
                    "cargo test -p psionic-ir autodiff::tests::public_vmap_transform_batches_reference_graph_outputs -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-ir tests::program_transform_capability_matrix_tracks_seeded_transform_and_future_cases -- --exact --nocapture",
                ),
            ],
            summary: String::from(
                "The seeded vmap family can now point at a public single-lane-to-batched transform with explicit cast and backend-extension refusals in the capability matrix.",
            ),
            boundary_note: String::from(
                "This is a bounded public vmap pass, not proof that jacobian or compile-as-transform are complete.",
            ),
        },
        MlxParityHarnessFamily {
            family_id: String::from("custom_vjp"),
            acceptance_category: String::from("transform-compile"),
            upstream_sources: vec![String::from("tests/custom_vjp_tests.cpp")],
            current_outcome: MlxParityHarnessOutcome::Pass,
            psionic_hook_commands: vec![
                String::from(
                    "cargo test -p psionic-ir autodiff::tests::public_custom_vjp_transform_uses_registered_rule -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-ir autodiff::tests::custom_vjp_registry_and_transform_refuse_missing_and_duplicate_rules -- --exact --nocapture",
                ),
            ],
            summary: String::from(
                "The upstream custom_vjp family can now point at a public graph-scoped transform-hook registry plus custom_vjp transform with explicit registration and cotangent validation.",
            ),
            boundary_note: String::from(
                "This is a bounded graph-scoped custom_vjp pass, not proof that jacobian or compile-as-transform are complete.",
            ),
        },
        MlxParityHarnessFamily {
            family_id: String::from("compile"),
            acceptance_category: String::from("transform-compile"),
            upstream_sources: vec![
                String::from("tests/compile_tests.cpp"),
                String::from("python/tests/test_compile.py"),
                String::from("python/tests/test_graph.py"),
            ],
            current_outcome: MlxParityHarnessOutcome::Pass,
            psionic_hook_commands: vec![
                String::from(
                    "cargo test -p psionic-compiler tests::compiler_hygiene_parity_matrix_tracks_seeded_supported_and_refusal_cases -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-compiler tests::compile_transform_emits_cold_then_warm_cache_hits_with_trace_and_debug -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-compiler tests::compile_transform_cache_controls_make_bypass_and_invalidation_explicit -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-compiler tests::compile_transform_shapeless_trace_family_identity_groups_same_rank_graphs -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-compiler tests::compile_transform_shapeless_trace_family_refuses_reshape_without_formula -- --exact --nocapture",
                ),
            ],
            summary: String::from(
                "The seeded compile family can now point at both the compiler-hygiene parity matrix and a first public compile-transform surface with explicit purity, cache, concrete-plan identity, and bounded shapeless trace-family identity controls.",
            ),
            boundary_note: String::from(
                "This bounded compile-transform pass now includes a narrow shapeless trace-family identity, but it is still not a full symbolic-shape, dynamic-guard, or broad shape-polymorphic compile claim.",
            ),
        },
        MlxParityHarnessFamily {
            family_id: String::from("export_import"),
            acceptance_category: String::from("export-serialization-tooling"),
            upstream_sources: vec![
                String::from("tests/export_import_tests.cpp"),
                String::from("tests/load_tests.cpp"),
                String::from("python/tests/test_export_import.py"),
                String::from("python/tests/test_load.py"),
            ],
            current_outcome: MlxParityHarnessOutcome::Pass,
            psionic_hook_commands: vec![
                String::from(
                    "cargo test -p psionic-ir tests::exportable_graph_contract_tracks_entry_signature_and_refuses_opaque_graphs -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-train model_io::tests::portable_model_bundle_roundtrips_through_safetensors_manifest -- --exact --nocapture",
                ),
                String::from("cargo test -p psionic-array-io -- --nocapture"),
                String::from("cargo test -p psionic-function-io -- --nocapture"),
            ],
            summary: String::from(
                "The seeded export/import family can now point at exportable-graph, native .psifn function artifacts, portable-model-IO, and public array-IO hooks as bounded parity anchors.",
            ),
            boundary_note: String::from(
                "This seed pass does not imply the full public MLX export or tooling shell already exists.",
            ),
        },
        MlxParityHarnessFamily {
            family_id: String::from("nn_optimizers_quantized"),
            acceptance_category: String::from("nn-optimizer"),
            upstream_sources: vec![
                String::from("python/tests/test_nn.py"),
                String::from("python/tests/test_losses.py"),
                String::from("python/tests/test_init.py"),
                String::from("python/tests/test_optimizers.py"),
                String::from("python/tests/test_quantized.py"),
                String::from("python/tests/test_tree.py"),
            ],
            current_outcome: MlxParityHarnessOutcome::Pass,
            psionic_hook_commands: vec![
                String::from(
                    "cargo test -p psionic-nn layers::tests::linear_forward_applies_affine_projection -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-nn layers::tests::embedding_lookup_preserves_index_shape_and_bounds -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-nn layers::tests::conv2d_and_pool2d_match_reference_windows -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-nn training::tests::classification_losses_and_helpers_match_reference -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-nn optimizers::tests::module_optimizer_updates_trainable_parameters_and_ignores_frozen_gradients -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-nn optimizers::tests::module_optimizer_scheduler_and_parameter_semantics_scale_effective_rates -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-nn optimizers::tests::multi_optimizer_composes_disjoint_groups_and_refuses_overlap_or_unassigned_paths -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-nn quantized::tests::module_quantize_reports_quantized_and_dense_paths_and_freezes_eval_copy -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-nn quantized::tests::quantized_linear_forward_tracks_dense_reference -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-nn quantized::tests::quantized_embedding_lookup_tracks_dense_reference -- --exact --nocapture",
                ),
                String::from(
                    "cargo test -p psionic-nn quantized::tests::quantized_linear_roundtrips_through_module_state_load -- --exact --nocapture",
                ),
            ],
            summary: String::from(
                "psionic-nn now exposes a first bounded quantized nn slice on the MLX public surface, including Module::quantize with explicit keep-dense versus strict posture, eval-only frozen quantized modules, and QuantizedLinear plus QuantizedEmbedding wrappers backed by int8_symmetric block storage and CPU-reference dequantize-to-f32 forward checks.",
            ),
            boundary_note: String::from(
                "This seeded pass proves only a first eval-oriented quantized layer family for linear and embedding modules; it does not imply broad quantized training closure, conv or norm quantized wrappers, or external artifact-format parity.",
            ),
        },
        MlxParityHarnessFamily {
            family_id: String::from("distributed"),
            acceptance_category: String::from("distributed-semantics"),
            upstream_sources: vec![
                String::from("python/tests/ring_test_distributed.py"),
                String::from("python/tests/mpi_test_distributed.py"),
                String::from("python/tests/nccl_test_distributed.py"),
                String::from("python/tests/mlx_distributed_tests.py"),
            ],
            current_outcome: MlxParityHarnessOutcome::Unsupported,
            psionic_hook_commands: Vec::new(),
            summary: String::from(
                "The upstream distributed families are seeded into the harness, but the MLX framework-distributed API does not exist in Psionic yet.",
            ),
            boundary_note: String::from(
                "Current collectives and cluster substrate are not enough to claim an upstream distributed family port.",
            ),
        },
    ])
}

/// Builds the canonical supported/convertible/unsupported MLX compatibility
/// matrix for the current Psionic roadmap.
#[must_use]
pub fn builtin_mlx_compatibility_matrix_report() -> MlxCompatibilityMatrixReport {
    MlxCompatibilityMatrixReport::new(vec![
        MlxCompatibilityMatrixEntry {
            surface_id: String::from("governance_contracts"),
            matrix_status: MlxCompatibilityMatrixStatus::Supported,
            summary: String::from(
                "The bounded MLX version window, acceptance matrix, parity harness, and claim-language contracts are all repo-owned and runnable today.",
            ),
            evidence_refs: vec![
                String::from("MlxCompatibilityScopeReport"),
                String::from("MlxAcceptanceMatrixReport"),
                String::from("MlxParityHarnessReport"),
            ],
            blocking_issue_refs: Vec::new(),
            boundary_note: String::from(
                "This is governance support, not a claim that the public MLX framework surface is already complete.",
            ),
        },
        MlxCompatibilityMatrixEntry {
            surface_id: String::from("seeded_transform_compile_export_parity_anchors"),
            matrix_status: MlxCompatibilityMatrixStatus::Supported,
            summary: String::from(
                "Seeded parity anchors now exist for autograd, vmap, custom_vjp, compile, and export/import.",
            ),
            evidence_refs: vec![
                String::from("MLX parity family `autograd` = pass"),
                String::from("MLX parity family `vmap` = pass"),
                String::from("MLX parity family `custom_vjp` = pass"),
                String::from("MLX parity family `compile` = pass"),
                String::from("MLX parity family `export_import` = pass"),
            ],
            blocking_issue_refs: Vec::new(),
            boundary_note: String::from(
                "Seeded family anchors are evidence, not blanket MLX-class API closure.",
            ),
        },
        MlxCompatibilityMatrixEntry {
            surface_id: String::from("graph_first_function_export_bridge"),
            matrix_status: MlxCompatibilityMatrixStatus::Supported,
            summary: String::from(
                "Psionic now exposes one supported native graph-first function export bridge through psionic-function-io, with .psifn artifacts binding export-safe graphs to optional compiler artifacts, trace-family identity, deployment contracts, and stable import/export receipts.",
            ),
            evidence_refs: vec![
                String::from("ExportableGraphContract"),
                String::from("DeploymentArtifactContract"),
                String::from("FunctionArtifact"),
                String::from("FunctionArtifactReceipt"),
            ],
            blocking_issue_refs: vec![String::from("PMLX-403 (#3855)")],
            boundary_note: String::from(
                "This supported bridge is Psionic-native `.psifn`, not current `.mlxfn` support.",
            ),
        },
        MlxCompatibilityMatrixEntry {
            surface_id: String::from("portable_model_io_bridge"),
            matrix_status: MlxCompatibilityMatrixStatus::Convertible,
            summary: String::from(
                "Portable model IO through safetensors manifests, GGUF import, and related receipts can support later bounded MLX migration paths.",
            ),
            evidence_refs: vec![
                String::from("PortableModelBundle::export_safetensors"),
                String::from("PortableModelBundle::import_safetensors"),
                String::from("GGUF import inventory"),
            ],
            blocking_issue_refs: Vec::new(),
            boundary_note: String::from(
                "Portable model IO is not the same thing as native MLX weight or module-state compatibility.",
            ),
        },
        MlxCompatibilityMatrixEntry {
            surface_id: String::from("module_state_tree_bridge"),
            matrix_status: MlxCompatibilityMatrixStatus::Supported,
            summary: String::from(
                "Psionic now exposes a public module tree with explicit trainable versus frozen posture, deterministic module state-tree and state-dict contracts, and bounded public save_weights/load_weights behavior with strict-by-default plus explicit non-strict loading.",
            ),
            evidence_refs: vec![
                String::from("Module"),
                String::from("ModuleParameterView"),
                String::from("Module::save_weights"),
                String::from("Module::load_weights"),
                String::from("Module::load_weights_with_mode"),
                String::from("ModuleStateDict"),
                String::from("ModuleStateLoadReport"),
            ],
            blocking_issue_refs: Vec::new(),
            boundary_note: String::from(
                "This supported module-state bridge is still bounded to Psionic-native module naming and receipts; it does not imply general array file IO or broad external MLX artifact compatibility.",
            ),
        },
        MlxCompatibilityMatrixEntry {
            surface_id: String::from("public_mlx_array_api"),
            matrix_status: MlxCompatibilityMatrixStatus::Supported,
            summary: String::from(
                "psionic-array now exposes a first public lazy-array facade with runtime-backed device handles, honest unified-memory flags, explicit stream-dependency policy, graph-backed arithmetic, explicit eval and deferred async_eval, replay-stable eval receipts, explicit-only materialization boundaries, scalar and filled-array creation helpers, bounded reshape/permute/transpose/slice/select/concat/broadcast_to families, explicit seeded or best-effort random creation, logical dtype casts, arange/linspace/eye helpers, explicit host-owned typed buffer export, singleton item extraction, and deterministic tree flatten/map/unflatten utilities, while the companion psionic-array-io crate now adds stable npy, npz, safetensors, and bounded dense GGUF save/load with explicit receipt inventory and GGUF quantization-to-dense import disclosure.",
            ),
            evidence_refs: vec![
                String::from("ArrayDevice"),
                String::from("ArrayStream"),
                String::from("ArrayContext"),
                String::from("Array"),
                String::from("EvaluatedArray"),
                String::from("HostArrayData"),
                String::from("ArrayScalar"),
                String::from("Tree<Array>"),
                String::from("PendingAsyncEval"),
                String::from("ArrayArtifactReceipt"),
                String::from("encode_npy"),
                String::from("decode_npz"),
                String::from("encode_safetensors"),
                String::from("decode_gguf"),
                String::from(
                    "MlxAcceptanceMatrixReport::array-runtime-surface = implemented_early",
                ),
            ],
            blocking_issue_refs: Vec::new(),
            boundary_note: String::from(
                "This is a bounded supported early array surface; it does not imply MLX transform, nn, native function export, or distributed support.",
            ),
        },
        MlxCompatibilityMatrixEntry {
            surface_id: String::from("public_mlx_transform_api"),
            matrix_status: MlxCompatibilityMatrixStatus::Supported,
            summary: String::from(
                "psionic-ir now exposes a bounded public transform layer with grad, value_and_grad, vjp, jvp, vmap, checkpoint replay, and graph-scoped custom_vjp hooks above AutodiffGraph, while psionic-compiler now exposes compile-as-transform with explicit purity, cache, concrete-plan identity, trace-family identity, and debug controls plus honest reshape/expand refusal on the current shapeless boundary.",
            ),
            evidence_refs: vec![
                String::from("grad"),
                String::from("value_and_grad"),
                String::from("vjp"),
                String::from("jvp"),
                String::from("vmap"),
                String::from("checkpoint"),
                String::from("custom_vjp"),
                String::from("compile_transform"),
                String::from("MlxParityHarnessReport"),
                String::from("CompileTraceFamilyIdentity"),
                String::from("MlxAcceptanceMatrixReport::transform-compile = implemented_early"),
                String::from("ProgramTransformCapabilityMatrixReport"),
            ],
            blocking_issue_refs: Vec::new(),
            boundary_note: String::from(
                "This is a bounded supported public transform surface, not a claim of jacobian support, full symbolic-shape closure, or broad higher-order transform completeness.",
            ),
        },
        MlxCompatibilityMatrixEntry {
            surface_id: String::from("public_mlx_nn_optimizer_api"),
            matrix_status: MlxCompatibilityMatrixStatus::Supported,
            summary: String::from(
                "psionic-nn now exposes a bounded supported public nn surface with a Module tree, save_weights/load_weights behavior, a CPU-reference core layer surface, CPU-reference losses and initializers, a public optimizer plus scheduler shell, and a first eval-oriented quantized module API through Module::quantize plus QuantizedLinear and QuantizedEmbedding.",
            ),
            evidence_refs: vec![
                String::from("Module"),
                String::from("ModuleParameterView"),
                String::from("Module::save_weights"),
                String::from("Module::load_weights"),
                String::from("Module::quantize"),
                String::from("ModuleQuantizeConfig"),
                String::from("QuantizedModule"),
                String::from("NnTensor"),
                String::from("Linear"),
                String::from("Embedding"),
                String::from("QuantizedLinear"),
                String::from("QuantizedEmbedding"),
                String::from("LayerNorm"),
                String::from("RmsNorm"),
                String::from("Activation"),
                String::from("Dropout"),
                String::from("Conv1d"),
                String::from("Conv2d"),
                String::from("Pool1d"),
                String::from("Pool2d"),
                String::from("LossReduction"),
                String::from("mse_loss"),
                String::from("cross_entropy_loss"),
                String::from("softmax_last_dim"),
                String::from("one_hot"),
                String::from("InitKind"),
                String::from("init_tensor"),
                String::from("init_parameter"),
                String::from("Optimizer"),
                String::from("OptimizerConfig"),
                String::from("Optimizer::step_module"),
                String::from("OptimizerStateSnapshot"),
                String::from("SchedulerConfig"),
                String::from("SchedulerBinding"),
                String::from("ParameterGroupSemantics"),
                String::from("OptimizerGroup"),
                String::from("MultiOptimizer"),
                String::from("MlxAcceptanceMatrixReport::nn-optimizer = implemented_early"),
                String::from("MLX parity family `nn_optimizers_quantized` = pass"),
            ],
            blocking_issue_refs: Vec::new(),
            boundary_note: String::from(
                "This is a bounded supported early public nn surface, not a claim of broad quantized training closure, quantized conv/norm wrapper breadth, or export-format quantization parity.",
            ),
        },
        MlxCompatibilityMatrixEntry {
            surface_id: String::from("mlxfn_interop"),
            matrix_status: MlxCompatibilityMatrixStatus::Unsupported,
            summary: String::from(
                "There is no `.mlxfn` import or export support in Psionic today.",
            ),
            evidence_refs: vec![String::from(
                "ROADMAP_MLX Phase 9 compatibility work remains open",
            )],
            blocking_issue_refs: vec![
                String::from("PMLX-402 (#3854)"),
                String::from("PMLX-403 (#3855)"),
            ],
            boundary_note: String::from(
                "Native graph-first export substrate does not imply `.mlxfn` compatibility.",
            ),
        },
        MlxCompatibilityMatrixEntry {
            surface_id: String::from("mlx_naming_facade_and_bindings"),
            matrix_status: MlxCompatibilityMatrixStatus::Unsupported,
            summary: String::from(
                "There is no MLX naming facade or Python/C/Swift binding layer in Psionic today.",
            ),
            evidence_refs: vec![String::from(
                "ROADMAP_MLX Epic 6 late-surface compatibility work remains open",
            )],
            blocking_issue_refs: vec![
                String::from("PMLX-606 (#3871)"),
                String::from("PMLX-607 (#3872)"),
                String::from("PMLX-608 (#3873)"),
            ],
            boundary_note: String::from(
                "Adoption-facing names and bindings are explicitly late work and must not be implied early.",
            ),
        },
        MlxCompatibilityMatrixEntry {
            surface_id: String::from("public_mlx_distributed_api"),
            matrix_status: MlxCompatibilityMatrixStatus::Unsupported,
            summary: String::from(
                "There is no public MLX-class distributed group and helper API in Psionic today.",
            ),
            evidence_refs: vec![
                String::from("MlxAcceptanceMatrixReport::distributed-semantics = planned"),
                String::from("MLX parity family `distributed` = unsupported"),
            ],
            blocking_issue_refs: vec![
                String::from("PMLX-501 (#3859)"),
                String::from("PMLX-502 (#3860)"),
                String::from("PMLX-503 (#3861)"),
                String::from("PMLX-504 (#3862)"),
                String::from("PMLX-505 (#3863)"),
                String::from("PMLX-506 (#3864)"),
                String::from("PMLX-507 (#3865)"),
            ],
            boundary_note: String::from(
                "Current collectives and cluster internals are not themselves a supported MLX public distributed surface.",
            ),
        },
        MlxCompatibilityMatrixEntry {
            surface_id: String::from("mlx_package_ecosystem"),
            matrix_status: MlxCompatibilityMatrixStatus::Unsupported,
            summary: String::from(
                "There is no supported MLX-lm, multimodal, audio, serving, recipe, or benchmark ecosystem layer in Psionic today.",
            ),
            evidence_refs: vec![String::from(
                "ROADMAP_MLX Epic 7 remains entirely future work",
            )],
            blocking_issue_refs: vec![
                String::from("PMLX-701 (#3874)"),
                String::from("PMLX-702 (#3875)"),
                String::from("PMLX-703 (#3876)"),
                String::from("PMLX-704 (#3877)"),
                String::from("PMLX-705 (#3878)"),
                String::from("PMLX-706 (#3879)"),
                String::from("PMLX-707 (#3880)"),
                String::from("PMLX-708 (#3881)"),
                String::from("PMLX-709 (#3882)"),
            ],
            boundary_note: String::from(
                "Ecosystem workflows are intentionally later and must not be implied by the current governance slice.",
            ),
        },
    ])
}

fn seeded_area(
    area_id: &str,
    bounded_scope: &str,
    evidence_refs: Vec<SemanticsEvidenceRef>,
    blockers: Vec<String>,
    blocking_issue_refs: Vec<String>,
) -> SemanticsClaimArea {
    SemanticsClaimArea {
        area_id: String::from(area_id),
        current_posture: SemanticsClaimPosture::SeededEvidenceOnly,
        bounded_scope: String::from(bounded_scope),
        evidence_refs,
        blockers,
        blocking_issue_refs,
    }
}

fn future_area(
    area_id: &str,
    bounded_scope: &str,
    blockers: Vec<String>,
    blocking_issue_refs: Vec<String>,
) -> SemanticsClaimArea {
    SemanticsClaimArea {
        area_id: String::from(area_id),
        current_posture: SemanticsClaimPosture::PyTorchCompatibleLater,
        bounded_scope: String::from(bounded_scope),
        evidence_refs: Vec::new(),
        blockers,
        blocking_issue_refs,
    }
}

fn stable_semantics_claim_report_digest(
    claim_vocabulary_version: &str,
    areas: &[SemanticsClaimArea],
) -> String {
    let mut lines = vec![format!(
        "claim_vocabulary_version={claim_vocabulary_version}"
    )];
    for area in areas {
        lines.push(format!("{}|{:?}", area.area_id, area.current_posture));
        lines.push(format!("scope={}", area.bounded_scope));
        for evidence in &area.evidence_refs {
            lines.push(format!(
                "evidence={}|{}",
                evidence.artifact_id, evidence.artifact_digest
            ));
        }
        for blocker in &area.blockers {
            lines.push(format!("blocker={blocker}"));
        }
        for issue in &area.blocking_issue_refs {
            lines.push(format!("issue={issue}"));
        }
    }
    lines.sort();
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

fn stable_mlx_compatibility_scope_report_digest(
    claim_vocabulary_version: &str,
    upstream_version_window: &MlxUpstreamVersionWindow,
    compatibility_terms: &[MlxCompatibilityTerm],
    explicit_rules: &[String],
) -> String {
    let mut lines = vec![
        format!("claim_vocabulary_version={claim_vocabulary_version}"),
        format!(
            "upstream_repository={}",
            upstream_version_window.upstream_repository
        ),
        format!(
            "minimum_inclusive_tag={}",
            upstream_version_window.minimum_inclusive_tag
        ),
        format!(
            "maximum_inclusive_tag={}",
            upstream_version_window.maximum_inclusive_tag
        ),
        format!(
            "review_checkout_commit={}",
            upstream_version_window.review_checkout_commit
        ),
        format!(
            "review_checkout_describe={}",
            upstream_version_window.review_checkout_describe
        ),
        format!(
            "review_checkout_date={}",
            upstream_version_window.review_checkout_date
        ),
    ];
    for term in compatibility_terms {
        lines.push(format!("term={}", term.term_id));
        lines.push(format!("summary={}", term.summary));
        for property in &term.required_properties {
            lines.push(format!("required={property}"));
        }
        for shortcut in &term.forbidden_shortcuts {
            lines.push(format!("forbidden={shortcut}"));
        }
    }
    for rule in explicit_rules {
        lines.push(format!("rule={rule}"));
    }
    lines.sort();
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

fn stable_mlx_acceptance_matrix_report_digest(
    schema_path: &str,
    runner: &str,
    report_posture: MlxAcceptanceMatrixPosture,
    selected_categories: &[String],
    categories: &[MlxAcceptanceCategory],
) -> String {
    let mut lines = vec![
        format!("schema_path={schema_path}"),
        format!("runner={runner}"),
        format!("report_posture={report_posture:?}"),
    ];
    for category_id in selected_categories {
        lines.push(format!("selected={category_id}"));
    }
    for category in categories {
        lines.push(format!(
            "category={}|{:?}|{}",
            category.category_id, category.matrix_status, category.epic_id
        ));
        for issue_ref in &category.issue_refs {
            lines.push(format!("issue={issue_ref}"));
        }
        lines.push(format!("green={}", category.green_definition));
        lines.push(format!("truth={}", category.current_repo_truth));
        lines.push(format!("boundary={}", category.boundary_note));
    }
    lines.sort();
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

fn stable_mlx_parity_harness_report_digest(
    schema_path: &str,
    runner: &str,
    oracle_window: &str,
    selected_families: &[String],
    families: &[MlxParityHarnessFamily],
) -> String {
    let mut lines = vec![
        format!("schema_path={schema_path}"),
        format!("runner={runner}"),
        format!("oracle_window={oracle_window}"),
    ];
    for family_id in selected_families {
        lines.push(format!("selected={family_id}"));
    }
    for family in families {
        lines.push(format!(
            "family={}|{:?}|{}",
            family.family_id, family.current_outcome, family.acceptance_category
        ));
        for source in &family.upstream_sources {
            lines.push(format!("source={source}"));
        }
        for command in &family.psionic_hook_commands {
            lines.push(format!("hook={command}"));
        }
        lines.push(format!("summary={}", family.summary));
        lines.push(format!("boundary={}", family.boundary_note));
    }
    lines.sort();
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

fn stable_mlx_compatibility_matrix_report_digest(
    schema_path: &str,
    runner: &str,
    oracle_window: &str,
    selected_surfaces: &[String],
    surfaces: &[MlxCompatibilityMatrixEntry],
) -> String {
    let mut lines = vec![
        format!("schema_path={schema_path}"),
        format!("runner={runner}"),
        format!("oracle_window={oracle_window}"),
    ];
    for surface_id in selected_surfaces {
        lines.push(format!("selected={surface_id}"));
    }
    for surface in surfaces {
        lines.push(format!(
            "surface={}|{:?}",
            surface.surface_id, surface.matrix_status
        ));
        for evidence_ref in &surface.evidence_refs {
            lines.push(format!("evidence={evidence_ref}"));
        }
        for issue_ref in &surface.blocking_issue_refs {
            lines.push(format!("issue={issue_ref}"));
        }
        lines.push(format!("summary={}", surface.summary));
        lines.push(format!("boundary={}", surface.boundary_note));
    }
    lines.sort();
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        MlxAcceptanceCategoryStatus, MlxAcceptanceMatrixPosture, MlxCompatibilityMatrixStatus,
        MlxParityHarnessOutcome, SemanticsClaimPosture, builtin_mlx_acceptance_matrix_report,
        builtin_mlx_compatibility_matrix_report, builtin_mlx_compatibility_scope_report,
        builtin_mlx_parity_harness_report, builtin_semantics_claim_report,
    };

    #[test]
    fn mlx_compatibility_scope_report_freezes_upstream_version_window_and_claim_vocabulary() {
        let report = builtin_mlx_compatibility_scope_report();
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.claim_vocabulary_version, "mlx_claim_v1");
        assert_eq!(
            report.upstream_version_window.upstream_repository,
            "ml-explore/mlx"
        );
        assert_eq!(
            report.upstream_version_window.minimum_inclusive_tag,
            "v0.31.0"
        );
        assert_eq!(
            report.upstream_version_window.maximum_inclusive_tag,
            "v0.31.1"
        );
        assert_eq!(
            report.upstream_version_window.review_checkout_commit,
            "ea91bd02cf0671f3fe6ddaf746812c27bf05154e"
        );
        assert_eq!(
            report.upstream_version_window.review_checkout_describe,
            "v0.31.1-7-gea91bd02"
        );
        assert_eq!(
            report.upstream_version_window.review_checkout_date,
            "2026-03-16"
        );
        assert!(
            report
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("report_digest="))
        );

        let mlx_class = report
            .compatibility_terms
            .iter()
            .find(|term| term.term_id == "mlx_class")
            .expect("missing mlx_class term");
        assert!(
            mlx_class
                .required_properties
                .iter()
                .any(|property| property.contains("implemented natively"))
        );
        assert!(
            mlx_class
                .forbidden_shortcuts
                .iter()
                .any(|shortcut| shortcut.contains("unversioned claims"))
        );

        let mlx_compatible = report
            .compatibility_terms
            .iter()
            .find(|term| term.term_id == "mlx_compatible")
            .expect("missing mlx_compatible term");
        assert!(
            mlx_compatible
                .required_properties
                .iter()
                .any(|property| property.contains("supported, convertible, and unsupported"))
        );
        assert!(
            mlx_compatible
                .forbidden_shortcuts
                .iter()
                .any(|shortcut| shortcut.contains("tip-of-tree MLX coverage"))
        );

        assert!(
            report
                .explicit_rules
                .iter()
                .any(|rule| rule.contains("inclusive v0.31.0 through v0.31.1"))
        );
        assert!(
            report
                .explicit_rules
                .iter()
                .any(|rule| rule.contains("does not widen the supported release window"))
        );
    }

    #[test]
    fn mlx_acceptance_matrix_report_declares_all_named_closure_categories_and_filtering()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = builtin_mlx_acceptance_matrix_report();
        assert_eq!(report.schema_version, 1);
        assert_eq!(
            report.schema_path,
            "crates/psionic/docs/mlx_acceptance_matrix_report.schema.json"
        );
        assert_eq!(
            report.runner,
            "scripts/release/check-psionic-mlx-acceptance-matrix.sh"
        );
        assert_eq!(
            report.report_posture,
            MlxAcceptanceMatrixPosture::TrackingOnly
        );
        assert!(
            report
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("report_digest="))
        );

        for category_id in [
            "array-runtime-surface",
            "transform-compile",
            "nn-optimizer",
            "export-serialization-tooling",
            "distributed-semantics",
            "backend-closure",
        ] {
            let category = report
                .categories
                .iter()
                .find(|category| category.category_id == category_id)
                .expect("missing MLX acceptance category");
            let expected_status = match category_id {
                "array-runtime-surface" => MlxAcceptanceCategoryStatus::ImplementedEarly,
                "transform-compile" => MlxAcceptanceCategoryStatus::ImplementedEarly,
                "nn-optimizer" => MlxAcceptanceCategoryStatus::ImplementedEarly,
                _ => MlxAcceptanceCategoryStatus::Planned,
            };
            assert_eq!(category.matrix_status, expected_status);
            assert!(!category.issue_refs.is_empty());
            assert!(!category.green_definition.is_empty());
            assert!(!category.current_repo_truth.is_empty());
            assert!(!category.boundary_note.is_empty());
        }

        let filtered = report.filter_to_categories(&[
            String::from("array-runtime-surface"),
            String::from("backend-closure"),
        ])?;
        assert_eq!(
            filtered.selected_categories,
            vec![
                String::from("array-runtime-surface"),
                String::from("backend-closure"),
            ]
        );
        assert_eq!(filtered.categories.len(), 2);

        let error = report
            .filter_to_categories(&[String::from("not-a-real-category")])
            .expect_err("unknown category should refuse");
        assert_eq!(
            error.to_string(),
            "unknown MLX acceptance category: not-a-real-category"
        );

        Ok(())
    }

    #[test]
    fn mlx_parity_harness_report_tracks_seeded_pass_refusal_and_unsupported_families()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = builtin_mlx_parity_harness_report();
        assert_eq!(report.schema_version, 1);
        assert_eq!(
            report.schema_path,
            "crates/psionic/docs/mlx_parity_harness_report.schema.json"
        );
        assert_eq!(
            report.runner,
            "scripts/release/check-psionic-mlx-parity-harness.sh"
        );
        assert_eq!(
            report.oracle_window,
            "ml-explore/mlx:v0.31.0..v0.31.1:seed_v0"
        );
        assert!(
            report
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("report_digest="))
        );

        let autograd = report
            .families
            .iter()
            .find(|family| family.family_id == "autograd")
            .expect("missing autograd family");
        assert_eq!(autograd.current_outcome, MlxParityHarnessOutcome::Pass);
        assert!(!autograd.psionic_hook_commands.is_empty());

        let vmap = report
            .families
            .iter()
            .find(|family| family.family_id == "vmap")
            .expect("missing vmap family");
        assert_eq!(vmap.current_outcome, MlxParityHarnessOutcome::Pass);
        assert!(!vmap.psionic_hook_commands.is_empty());

        let custom_vjp = report
            .families
            .iter()
            .find(|family| family.family_id == "custom_vjp")
            .expect("missing custom_vjp family");
        assert_eq!(custom_vjp.current_outcome, MlxParityHarnessOutcome::Pass);
        assert!(!custom_vjp.psionic_hook_commands.is_empty());

        let distributed = report
            .families
            .iter()
            .find(|family| family.family_id == "distributed")
            .expect("missing distributed family");
        assert_eq!(
            distributed.current_outcome,
            MlxParityHarnessOutcome::Unsupported
        );
        assert!(distributed.psionic_hook_commands.is_empty());

        let eval = report
            .families
            .iter()
            .find(|family| family.family_id == "device_eval_memory")
            .expect("missing device/eval/memory family");
        assert_eq!(eval.current_outcome, MlxParityHarnessOutcome::Unsupported);
        assert!(!eval.psionic_hook_commands.is_empty());

        let nn = report
            .families
            .iter()
            .find(|family| family.family_id == "nn_optimizers_quantized")
            .expect("missing nn family");
        assert_eq!(nn.current_outcome, MlxParityHarnessOutcome::Pass);
        assert!(!nn.psionic_hook_commands.is_empty());
        assert!(nn.psionic_hook_commands.iter().any(|hook| {
            hook.contains("training::tests::classification_losses_and_helpers_match_reference")
        }));
        assert!(nn.psionic_hook_commands.iter().any(|hook| {
            hook.contains(
                "optimizers::tests::module_optimizer_updates_trainable_parameters_and_ignores_frozen_gradients",
            )
        }));
        assert!(nn.psionic_hook_commands.iter().any(|hook| {
            hook.contains(
                "optimizers::tests::module_optimizer_scheduler_and_parameter_semantics_scale_effective_rates",
            )
        }));
        assert!(nn.psionic_hook_commands.iter().any(|hook| {
            hook.contains(
                "optimizers::tests::multi_optimizer_composes_disjoint_groups_and_refuses_overlap_or_unassigned_paths",
            )
        }));
        assert!(nn.psionic_hook_commands.iter().any(|hook| {
            hook.contains(
                "quantized::tests::module_quantize_reports_quantized_and_dense_paths_and_freezes_eval_copy",
            )
        }));
        assert!(nn.psionic_hook_commands.iter().any(|hook| {
            hook.contains("quantized::tests::quantized_linear_forward_tracks_dense_reference")
        }));

        let filtered =
            report.filter_to_families(&[String::from("autograd"), String::from("distributed")])?;
        assert_eq!(
            filtered.selected_families,
            vec![String::from("autograd"), String::from("distributed")]
        );
        assert_eq!(filtered.families.len(), 2);

        let error = report
            .filter_to_families(&[String::from("not-a-real-family")])
            .expect_err("unknown family should refuse");
        assert_eq!(
            error.to_string(),
            "unknown MLX parity family: not-a-real-family"
        );

        Ok(())
    }

    #[test]
    fn mlx_compatibility_matrix_report_tracks_supported_convertible_and_unsupported_rows()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = builtin_mlx_compatibility_matrix_report();
        assert_eq!(report.schema_version, 1);
        assert_eq!(
            report.schema_path,
            "crates/psionic/docs/mlx_compatibility_matrix_report.schema.json"
        );
        assert_eq!(
            report.runner,
            "scripts/release/check-psionic-mlx-compatibility-matrix.sh"
        );
        assert_eq!(
            report.oracle_window,
            "ml-explore/mlx:v0.31.0..v0.31.1:matrix_v0"
        );
        assert!(
            report
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("report_digest="))
        );

        let governance = report
            .surfaces
            .iter()
            .find(|surface| surface.surface_id == "governance_contracts")
            .expect("missing governance row");
        assert_eq!(
            governance.matrix_status,
            MlxCompatibilityMatrixStatus::Supported
        );

        let bridge = report
            .surfaces
            .iter()
            .find(|surface| surface.surface_id == "graph_first_function_export_bridge")
            .expect("missing export bridge row");
        assert_eq!(
            bridge.matrix_status,
            MlxCompatibilityMatrixStatus::Supported
        );

        let array = report
            .surfaces
            .iter()
            .find(|surface| surface.surface_id == "public_mlx_array_api")
            .expect("missing public array row");
        assert_eq!(array.matrix_status, MlxCompatibilityMatrixStatus::Supported);

        let transform = report
            .surfaces
            .iter()
            .find(|surface| surface.surface_id == "public_mlx_transform_api")
            .expect("missing public transform row");
        assert_eq!(
            transform.matrix_status,
            MlxCompatibilityMatrixStatus::Supported
        );

        let module_state = report
            .surfaces
            .iter()
            .find(|surface| surface.surface_id == "module_state_tree_bridge")
            .expect("missing module-state bridge row");
        assert_eq!(
            module_state.matrix_status,
            MlxCompatibilityMatrixStatus::Supported
        );
        assert!(module_state.blocking_issue_refs.is_empty());

        let nn = report
            .surfaces
            .iter()
            .find(|surface| surface.surface_id == "public_mlx_nn_optimizer_api")
            .expect("missing public nn row");
        assert_eq!(nn.matrix_status, MlxCompatibilityMatrixStatus::Supported);
        assert!(
            nn.blocking_issue_refs
                .iter()
                .all(|issue| !issue.contains("PMLX-303"))
        );
        assert!(
            nn.blocking_issue_refs
                .iter()
                .all(|issue| !issue.contains("PMLX-304"))
        );
        assert!(
            nn.blocking_issue_refs
                .iter()
                .all(|issue| !issue.contains("PMLX-305"))
        );
        assert!(nn.blocking_issue_refs.is_empty());
        assert!(
            nn.blocking_issue_refs
                .iter()
                .all(|issue| !issue.contains("PMLX-306"))
        );

        let filtered = report.filter_to_surfaces(&[
            String::from("governance_contracts"),
            String::from("public_mlx_array_api"),
        ])?;
        assert_eq!(
            filtered.selected_surfaces,
            vec![
                String::from("governance_contracts"),
                String::from("public_mlx_array_api"),
            ]
        );
        assert_eq!(filtered.surfaces.len(), 2);

        let error = report
            .filter_to_surfaces(&[String::from("not-a-real-surface")])
            .expect_err("unknown surface should refuse");
        assert_eq!(
            error.to_string(),
            "unknown MLX compatibility surface: not-a-real-surface"
        );

        Ok(())
    }

    #[test]
    fn semantics_claim_report_marks_seeded_evidence_and_future_compatibility_targets()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = builtin_semantics_claim_report()?;
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.claim_vocabulary_version, "pytorch_claim_v1");
        assert_eq!(
            report.overall_posture,
            SemanticsClaimPosture::SeededEvidenceOnly
        );
        assert!(
            report
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("report_digest="))
        );

        for area_id in [
            "operator_semantics",
            "advanced_operator_family_semantics",
            "program_transform_semantics",
            "export_and_deployment_semantics",
            "extension_contract_semantics",
            "data_ingress_semantics",
            "distributed_data_feed_semantics",
            "tensor_family_semantics",
            "advanced_dtype_semantics",
            "reproducibility_semantics",
            "precision_policy_semantics",
            "quantization_semantics",
            "module_and_state_semantics",
            "optimizer_semantics",
            "compiler_hygiene_and_fake_tensor",
        ] {
            let area = report
                .areas
                .iter()
                .find(|area| area.area_id == area_id)
                .expect("missing seeded-evidence area");
            assert_eq!(
                area.current_posture,
                SemanticsClaimPosture::SeededEvidenceOnly
            );
            assert!(!area.evidence_refs.is_empty());
        }

        for area_id in [
            "checkpoint_and_model_io_interop",
            "advanced_tensor_dtype_and_precision",
            "extensions_and_plugins",
        ] {
            let area = report
                .areas
                .iter()
                .find(|area| area.area_id == area_id)
                .expect("missing future compatibility area");
            assert_eq!(
                area.current_posture,
                SemanticsClaimPosture::PyTorchCompatibleLater
            );
            assert!(area.evidence_refs.is_empty());
            assert!(!area.blockers.is_empty());
        }

        Ok(())
    }
}
