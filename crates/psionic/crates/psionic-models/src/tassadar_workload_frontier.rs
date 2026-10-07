use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TASSADAR_WORKLOAD_FRONTIER_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_WORKLOAD_CAPABILITY_FRONTIER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_workload_capability_frontier_report.json";
pub const TASSADAR_WORKLOAD_CAPABILITY_FRONTIER_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_workload_capability_frontier_summary.json";

/// Machine-legible publication status for the workload-hardness taxonomy lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorkloadHardnessTaxonomyStatus {
    /// Landed as a repo-backed public research surface.
    Implemented,
}

/// Stable workload-hardness labels used by the capability frontier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorkloadHardnessClass {
    RetrievalLike,
    Parallelizable,
    SearchHeavy,
    MemoryHeavy,
    ControlHeavy,
}

/// Stable budget-pressure scale used by the frontier report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBudgetPressure {
    Low,
    Medium,
    High,
}

/// Stable lane kinds used when summarizing the capability frontier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarFrontierLaneKind {
    CompiledExact,
    RuntimeRecurrentBaseline,
    SearchSpecificTrace,
    LearnedSharedDepth,
    RefuseInsteadOfDegrade,
}

/// Architecture-budget descriptor carried by each workload family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArchitectureBudgetDescriptor {
    pub depth_budget: TassadarBudgetPressure,
    pub width_budget: TassadarBudgetPressure,
    pub recurrent_budget: TassadarBudgetPressure,
    pub extra_trace_space_budget: TassadarBudgetPressure,
}

/// One workload-family row in the public hardness taxonomy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkloadHardnessTaxonomyRow {
    pub workload_family_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_workload_target: Option<String>,
    pub hardness_classes: Vec<TassadarWorkloadHardnessClass>,
    pub budget_descriptor: TassadarArchitectureBudgetDescriptor,
    pub preferred_lanes: Vec<TassadarFrontierLaneKind>,
    pub refusal_boundary: String,
    pub note: String,
}

/// Public model-facing publication for the workload-hardness taxonomy lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkloadHardnessTaxonomyPublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub status: TassadarWorkloadHardnessTaxonomyStatus,
    pub claim_class: String,
    pub rows: Vec<TassadarWorkloadHardnessTaxonomyRow>,
    pub target_surfaces: Vec<String>,
    pub validation_refs: Vec<String>,
    pub support_boundaries: Vec<String>,
    pub publication_digest: String,
}

impl TassadarWorkloadHardnessTaxonomyPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_WORKLOAD_FRONTIER_SCHEMA_VERSION,
            publication_id: String::from("tassadar.workload_hardness_taxonomy.publication.v1"),
            status: TassadarWorkloadHardnessTaxonomyStatus::Implemented,
            claim_class: String::from("research_only_capability_truth"),
            rows: taxonomy_rows(),
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-research"),
                String::from("crates/psionic-provider"),
            ],
            validation_refs: vec![
                String::from(TASSADAR_WORKLOAD_CAPABILITY_FRONTIER_REPORT_REF),
                String::from(TASSADAR_WORKLOAD_CAPABILITY_FRONTIER_SUMMARY_REPORT_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the taxonomy is a research-only workload-classification and frontier vocabulary over the current benchmark pack; it does not promote served capability or flatten compiled, learned, search, and fast-path work into one generic score",
                ),
                String::from(
                    "budget descriptors are regime labels for the current workload families, not a proof that every workload with the same label is solved or benchmarked",
                ),
                String::from(
                    "preferred lanes remain refusal-first guidance; unsupported or under-mapped regions must stay explicit instead of being smoothed into aggregate capability claims",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_workload_hardness_taxonomy_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public workload-hardness taxonomy publication.
#[must_use]
pub fn tassadar_workload_hardness_taxonomy_publication(
) -> TassadarWorkloadHardnessTaxonomyPublication {
    TassadarWorkloadHardnessTaxonomyPublication::new()
}

fn taxonomy_rows() -> Vec<TassadarWorkloadHardnessTaxonomyRow> {
    vec![
        row(
            "micro_wasm_kernel",
            Some("micro_wasm_kernel"),
            vec![
                TassadarWorkloadHardnessClass::RetrievalLike,
                TassadarWorkloadHardnessClass::MemoryHeavy,
            ],
            budget(TassadarBudgetPressure::Low, TassadarBudgetPressure::Low, TassadarBudgetPressure::Low, TassadarBudgetPressure::Low),
            vec![
                TassadarFrontierLaneKind::CompiledExact,
                TassadarFrontierLaneKind::RuntimeRecurrentBaseline,
            ],
            "micro kernels should refuse only when bounded opcode or memory support is missing rather than silently degrading into unrelated learned behavior",
            "bounded micro-kernel family where compiled exactness and recurrent runtime baselines are both meaningful frontier anchors",
        ),
        row(
            "branch_heavy_kernel",
            Some("branch_heavy_kernel"),
            vec![TassadarWorkloadHardnessClass::ControlHeavy],
            budget(TassadarBudgetPressure::Medium, TassadarBudgetPressure::Low, TassadarBudgetPressure::Medium, TassadarBudgetPressure::Low),
            vec![
                TassadarFrontierLaneKind::CompiledExact,
                TassadarFrontierLaneKind::RuntimeRecurrentBaseline,
            ],
            "branch-heavy kernels should refuse when control-flow support falls outside the explicit bounded runtime family instead of silently flattening into dense replay",
            "control-heavy kernel family used to separate bounded control-flow pressure from open-ended search",
        ),
        row(
            "memory_heavy_kernel",
            Some("memory_heavy_kernel"),
            vec![TassadarWorkloadHardnessClass::MemoryHeavy],
            budget(TassadarBudgetPressure::Low, TassadarBudgetPressure::Medium, TassadarBudgetPressure::Low, TassadarBudgetPressure::Medium),
            vec![
                TassadarFrontierLaneKind::CompiledExact,
                TassadarFrontierLaneKind::RuntimeRecurrentBaseline,
            ],
            "memory-heavy kernels should refuse when bounded state or address support is exceeded rather than hiding that pressure behind aggregate exactness",
            "dense load/store kernel family that exposes memory pressure separately from search pressure",
        ),
        row(
            "long_loop_kernel",
            Some("long_loop_kernel"),
            vec![TassadarWorkloadHardnessClass::ControlHeavy],
            budget(TassadarBudgetPressure::Low, TassadarBudgetPressure::Low, TassadarBudgetPressure::High, TassadarBudgetPressure::Low),
            vec![
                TassadarFrontierLaneKind::RuntimeRecurrentBaseline,
                TassadarFrontierLaneKind::CompiledExact,
                TassadarFrontierLaneKind::LearnedSharedDepth,
            ],
            "long-loop kernels should refuse when recurrent or halting budgets are exhausted instead of silently treating long horizons as small-prefix work",
            "loop-heavy kernel family used to anchor recurrent-budget frontier questions",
        ),
        row(
            "clrs_shortest_path",
            Some("clrs_shortest_path"),
            vec![
                TassadarWorkloadHardnessClass::Parallelizable,
                TassadarWorkloadHardnessClass::MemoryHeavy,
            ],
            budget(TassadarBudgetPressure::Medium, TassadarBudgetPressure::Medium, TassadarBudgetPressure::Medium, TassadarBudgetPressure::Medium),
            vec![
                TassadarFrontierLaneKind::CompiledExact,
                TassadarFrontierLaneKind::RefuseInsteadOfDegrade,
            ],
            "CLRS bridge workloads should refuse broad promotion until the bridge stays explicit about trajectory family, length bucket, and exact bounded export coverage",
            "shared shortest-path bridge family that keeps CLRS-style trajectory work legible without claiming full algorithm-benchmark closure",
        ),
        row(
            "sudoku_class",
            Some("sudoku_class"),
            vec![
                TassadarWorkloadHardnessClass::SearchHeavy,
                TassadarWorkloadHardnessClass::ControlHeavy,
            ],
            budget(TassadarBudgetPressure::High, TassadarBudgetPressure::Low, TassadarBudgetPressure::Medium, TassadarBudgetPressure::High),
            vec![
                TassadarFrontierLaneKind::SearchSpecificTrace,
                TassadarFrontierLaneKind::CompiledExact,
                TassadarFrontierLaneKind::RefuseInsteadOfDegrade,
            ],
            "search-heavy Sudoku workloads should refuse rather than degrade when verifier/search traces or exact compiled support are unavailable",
            "real search family where search-specific traces and exact compiled closure matter more than flat dense replay",
        ),
        row(
            "hungarian_matching",
            Some("hungarian_matching"),
            vec![
                TassadarWorkloadHardnessClass::Parallelizable,
                TassadarWorkloadHardnessClass::SearchHeavy,
            ],
            budget(TassadarBudgetPressure::Medium, TassadarBudgetPressure::Medium, TassadarBudgetPressure::Low, TassadarBudgetPressure::Medium),
            vec![
                TassadarFrontierLaneKind::CompiledExact,
                TassadarFrontierLaneKind::RefuseInsteadOfDegrade,
            ],
            "matching workloads should refuse when bounded exact search or compiled support is missing rather than pretending parallel frontier traces imply full executor closure",
            "matching family that mixes parallelizable frontier structure with exact combinatorial search pressure",
        ),
        row(
            "module_memcpy",
            Some("module_memcpy"),
            vec![
                TassadarWorkloadHardnessClass::RetrievalLike,
                TassadarWorkloadHardnessClass::Parallelizable,
            ],
            budget(TassadarBudgetPressure::Low, TassadarBudgetPressure::Medium, TassadarBudgetPressure::Low, TassadarBudgetPressure::Low),
            vec![
                TassadarFrontierLaneKind::CompiledExact,
                TassadarFrontierLaneKind::LearnedSharedDepth,
            ],
            "module memcpy workloads should refuse when module-scale lowering or export coverage is missing instead of being collapsed into kernel-only exactness",
            "module-scale copy family used to anchor low-depth but non-kernel module execution",
        ),
        row(
            "module_parsing",
            Some("module_parsing"),
            vec![
                TassadarWorkloadHardnessClass::ControlHeavy,
                TassadarWorkloadHardnessClass::MemoryHeavy,
            ],
            budget(TassadarBudgetPressure::Medium, TassadarBudgetPressure::Medium, TassadarBudgetPressure::Medium, TassadarBudgetPressure::Low),
            vec![
                TassadarFrontierLaneKind::CompiledExact,
                TassadarFrontierLaneKind::LearnedSharedDepth,
                TassadarFrontierLaneKind::RefuseInsteadOfDegrade,
            ],
            "module parsing workloads should refuse when bounded control-flow or state channels are missing instead of being over-read from memcpy-style wins",
            "module parsing family used to separate nested control and state pressure from flat copy kernels",
        ),
        row(
            "module_checksum",
            Some("module_checksum"),
            vec![
                TassadarWorkloadHardnessClass::MemoryHeavy,
                TassadarWorkloadHardnessClass::Parallelizable,
            ],
            budget(TassadarBudgetPressure::Low, TassadarBudgetPressure::Low, TassadarBudgetPressure::Low, TassadarBudgetPressure::Low),
            vec![
                TassadarFrontierLaneKind::CompiledExact,
                TassadarFrontierLaneKind::LearnedSharedDepth,
            ],
            "module checksum workloads should refuse once bounded memory-span assumptions break instead of silently widening module exactness",
            "module checksum family that remains low-depth but nontrivial at module scale",
        ),
        row(
            "module_vm_style",
            Some("module_vm_style"),
            vec![
                TassadarWorkloadHardnessClass::ControlHeavy,
                TassadarWorkloadHardnessClass::MemoryHeavy,
            ],
            budget(TassadarBudgetPressure::High, TassadarBudgetPressure::Medium, TassadarBudgetPressure::High, TassadarBudgetPressure::Medium),
            vec![
                TassadarFrontierLaneKind::CompiledExact,
                TassadarFrontierLaneKind::LearnedSharedDepth,
                TassadarFrontierLaneKind::RefuseInsteadOfDegrade,
            ],
            "vm-style module workloads should refuse when dispatch or call-frame support leaves the bounded family instead of being treated like flat parsing or checksum cases",
            "module-scale dispatch family with the highest current module-control pressure in the public suite",
        ),
    ]
}

fn row(
    workload_family_id: &str,
    benchmark_workload_target: Option<&str>,
    hardness_classes: Vec<TassadarWorkloadHardnessClass>,
    budget_descriptor: TassadarArchitectureBudgetDescriptor,
    preferred_lanes: Vec<TassadarFrontierLaneKind>,
    refusal_boundary: &str,
    note: &str,
) -> TassadarWorkloadHardnessTaxonomyRow {
    TassadarWorkloadHardnessTaxonomyRow {
        workload_family_id: String::from(workload_family_id),
        benchmark_workload_target: benchmark_workload_target.map(String::from),
        hardness_classes,
        budget_descriptor,
        preferred_lanes,
        refusal_boundary: String::from(refusal_boundary),
        note: String::from(note),
    }
}

fn budget(
    depth_budget: TassadarBudgetPressure,
    width_budget: TassadarBudgetPressure,
    recurrent_budget: TassadarBudgetPressure,
    extra_trace_space_budget: TassadarBudgetPressure,
) -> TassadarArchitectureBudgetDescriptor {
    TassadarArchitectureBudgetDescriptor {
        depth_budget,
        width_budget,
        recurrent_budget,
        extra_trace_space_budget,
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        tassadar_workload_hardness_taxonomy_publication, TassadarFrontierLaneKind,
        TassadarWorkloadHardnessClass, TassadarWorkloadHardnessTaxonomyStatus,
    };

    #[test]
    fn workload_hardness_taxonomy_publication_is_machine_legible() {
        let publication = tassadar_workload_hardness_taxonomy_publication();

        assert_eq!(
            publication.status,
            TassadarWorkloadHardnessTaxonomyStatus::Implemented
        );
        assert_eq!(publication.rows.len(), 11);
        let sudoku = publication
            .rows
            .iter()
            .find(|row| row.workload_family_id == "sudoku_class")
            .expect("sudoku workload row");
        assert!(sudoku
            .hardness_classes
            .contains(&TassadarWorkloadHardnessClass::SearchHeavy));
        assert!(sudoku
            .preferred_lanes
            .contains(&TassadarFrontierLaneKind::SearchSpecificTrace));
        assert!(!publication.publication_digest.is_empty());
    }
}
