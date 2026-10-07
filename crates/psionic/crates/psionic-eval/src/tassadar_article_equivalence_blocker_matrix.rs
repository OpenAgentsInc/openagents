use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_equivalence_blocker_matrix_report.json";
pub const TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_CHECKER_REF: &str =
    "scripts/check-tassadar-article-equivalence-blocker-matrix.sh";

const ARTICLE_SOURCE_ID: &str = "percepta.can_llms_be_computers.2026_03_11.claim_surface";
const ARTICLE_SOURCE_TITLE: &str = "Can LLMs Be Computers?";
const ARTICLE_SOURCE_DATE: &str = "2026-03-11";
const ARTICLE_SOURCE_REF: &str =
    "Percepta article `can-llms-be-computers.md` as reviewed on 2026-03-19";
const ARTICLE_SOURCE_LINE_NUMBERING_NOTE: &str = "line numbers refer to the 2026-03-19 working article text that includes the implementation-status note preface; they do not reuse the older gap-analysis line numbering";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleEquivalenceBlockerCategory {
    FrontendScope,
    InterpreterBreadth,
    TransformerStackReality,
    FastRouteScope,
    BenchmarkScope,
    SingleRunScope,
    WeightsOwnershipScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRepoStatus {
    Implemented,
    ImplementedEarly,
    Partial,
    PartialOutsidePsionic,
    Planned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleEquivalenceIssueRole {
    Prerequisite,
    Gate,
    Implementation,
    FinalAudit,
    OptionalResearch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleLineProvenance {
    pub claim_ref_id: String,
    pub line_start: u16,
    pub line_end: u16,
    pub claim_paraphrase: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceIssueCoverageRow {
    pub issue_id: String,
    pub github_issue_number: u16,
    pub title: String,
    pub issue_role: TassadarArticleEquivalenceIssueRole,
    pub issue_state: String,
    pub blocker_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceCategoryCoverage {
    pub category: TassadarArticleEquivalenceBlockerCategory,
    pub blocker_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceBlockerRow {
    pub blocker_id: String,
    pub category: TassadarArticleEquivalenceBlockerCategory,
    pub title: String,
    pub repo_status: TassadarRepoStatus,
    pub current_gap_summary: String,
    pub current_public_truth: String,
    pub closure_requirements: Vec<String>,
    pub article_line_refs: Vec<TassadarArticleLineProvenance>,
    pub covered_by_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceBlockerMatrixReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub source_id: String,
    pub source_title: String,
    pub source_date: String,
    pub source_ref: String,
    pub source_line_numbering_note: String,
    pub required_category_count: usize,
    pub category_coverage: Vec<TassadarArticleEquivalenceCategoryCoverage>,
    pub blocker_count: usize,
    pub blocker_ids: Vec<String>,
    pub blockers: Vec<TassadarArticleEquivalenceBlockerRow>,
    pub issue_coverage_rows: Vec<TassadarArticleEquivalenceIssueCoverageRow>,
    pub required_later_issue_count: usize,
    pub prerequisite_transformer_boundary_green: bool,
    pub all_required_categories_present: bool,
    pub all_blocker_ids_unique: bool,
    pub all_blockers_have_article_line_provenance: bool,
    pub all_blockers_covered_by_issue_map: bool,
    pub all_later_issues_covered: bool,
    pub all_issue_refs_point_to_known_blockers: bool,
    pub matrix_contract_green: bool,
    pub article_equivalence_green: bool,
    pub open_blocker_count: usize,
    pub open_blocker_ids: Vec<String>,
    pub current_truth_boundary: String,
    pub non_implications: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleEquivalenceBlockerMatrixReportError {
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MatrixEvaluation {
    prerequisite_transformer_boundary_green: bool,
    all_required_categories_present: bool,
    all_blocker_ids_unique: bool,
    all_blockers_have_article_line_provenance: bool,
    all_blockers_covered_by_issue_map: bool,
    all_later_issues_covered: bool,
    all_issue_refs_point_to_known_blockers: bool,
    matrix_contract_green: bool,
    article_equivalence_green: bool,
    open_blocker_count: usize,
}

pub fn build_tassadar_article_equivalence_blocker_matrix_report() -> Result<
    TassadarArticleEquivalenceBlockerMatrixReport,
    TassadarArticleEquivalenceBlockerMatrixReportError,
> {
    let issue_coverage_rows = issue_coverage_rows();
    let mut blockers = blocker_rows();
    attach_issue_coverage(&mut blockers, issue_coverage_rows.as_slice());
    let category_coverage = category_coverage(blockers.as_slice());
    let evaluation = evaluate_matrix(blockers.as_slice(), issue_coverage_rows.as_slice());
    let blocker_ids = blockers
        .iter()
        .map(|blocker| blocker.blocker_id.clone())
        .collect::<Vec<_>>();
    let open_blocker_ids = blockers
        .iter()
        .filter(|blocker| !blocker.repo_status_closes_blocker())
        .map(|blocker| blocker.blocker_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarArticleEquivalenceBlockerMatrixReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_equivalence.blocker_matrix.v1"),
        checker_script_ref: String::from(TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_CHECKER_REF),
        source_id: String::from(ARTICLE_SOURCE_ID),
        source_title: String::from(ARTICLE_SOURCE_TITLE),
        source_date: String::from(ARTICLE_SOURCE_DATE),
        source_ref: String::from(ARTICLE_SOURCE_REF),
        source_line_numbering_note: String::from(ARTICLE_SOURCE_LINE_NUMBERING_NOTE),
        required_category_count: required_categories().len(),
        category_coverage,
        blocker_count: blockers.len(),
        blocker_ids,
        blockers,
        issue_coverage_rows,
        required_later_issue_count: required_later_issue_ids().len(),
        prerequisite_transformer_boundary_green: evaluation.prerequisite_transformer_boundary_green,
        all_required_categories_present: evaluation.all_required_categories_present,
        all_blocker_ids_unique: evaluation.all_blocker_ids_unique,
        all_blockers_have_article_line_provenance: evaluation
            .all_blockers_have_article_line_provenance,
        all_blockers_covered_by_issue_map: evaluation.all_blockers_covered_by_issue_map,
        all_later_issues_covered: evaluation.all_later_issues_covered,
        all_issue_refs_point_to_known_blockers: evaluation.all_issue_refs_point_to_known_blockers,
        matrix_contract_green: evaluation.matrix_contract_green,
        article_equivalence_green: evaluation.article_equivalence_green,
        open_blocker_count: evaluation.open_blocker_count,
        open_blocker_ids,
        current_truth_boundary: String::from(
            "the public repo now has one bounded article-equivalence closure for the declared Rust-only article envelope: the owned `psionic-transformer` route, the canonical trained trace-bound article model and weight artifact, the direct deterministic HullCache claim route, the declared CPU machine matrix, and the explicit mechanistic, behavioral, and operational closure gates all now land green together",
        ),
        non_implications: vec![
            String::from("not arbitrary C or arbitrary Wasm ingress closure"),
            String::from("not a generic public interpreter-in-weights claim"),
            String::from(
                "not a widening of the claim beyond the declared article frontend, interpreter, benchmark, and same-run operator envelope",
            ),
            String::from(
                "not a planner-mediated, hybrid, stochastic, or resumed execution claim for the canonical public article route",
            ),
            String::from(
                "not the post-article universality or generic Turing-completeness bridge",
            ),
        ],
        claim_boundary: String::from(
            "this report freezes the exact blocker set for bounded article-equivalence closure and now records that every blocker row has closed. It remains a machine-readable contract for what had to become true, and it must not be read as widening the public capability surface beyond the declared article envelope",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article-equivalence blocker matrix now freezes blocker_count={}, later_issue_count={}, matrix_contract_green={}, article_equivalence_green={}, and open_blocker_count={}.",
        report.blocker_count,
        report.required_later_issue_count,
        report.matrix_contract_green,
        report.article_equivalence_green,
        report.open_blocker_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_equivalence_blocker_matrix_report|",
        &report,
    );
    Ok(report)
}

pub fn tassadar_article_equivalence_blocker_matrix_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF)
}

pub fn write_tassadar_article_equivalence_blocker_matrix_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleEquivalenceBlockerMatrixReport,
    TassadarArticleEquivalenceBlockerMatrixReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleEquivalenceBlockerMatrixReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_equivalence_blocker_matrix_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleEquivalenceBlockerMatrixReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn category_coverage(
    blockers: &[TassadarArticleEquivalenceBlockerRow],
) -> Vec<TassadarArticleEquivalenceCategoryCoverage> {
    required_categories()
        .into_iter()
        .map(|category| TassadarArticleEquivalenceCategoryCoverage {
            category,
            blocker_count: blockers
                .iter()
                .filter(|blocker| blocker.category == category)
                .count(),
        })
        .collect()
}

fn evaluate_matrix(
    blockers: &[TassadarArticleEquivalenceBlockerRow],
    issue_coverage_rows: &[TassadarArticleEquivalenceIssueCoverageRow],
) -> MatrixEvaluation {
    let blocker_ids = blockers
        .iter()
        .map(|blocker| blocker.blocker_id.clone())
        .collect::<Vec<_>>();
    let blocker_id_set = blocker_ids.iter().cloned().collect::<BTreeSet<_>>();
    let all_blocker_ids_unique = blocker_ids.len() == blocker_id_set.len();
    let observed_categories = blockers
        .iter()
        .map(|blocker| blocker.category)
        .collect::<BTreeSet<_>>();
    let all_required_categories_present = observed_categories == required_categories();
    let all_blockers_have_article_line_provenance = blockers.iter().all(|blocker| {
        !blocker.article_line_refs.is_empty()
            && blocker.article_line_refs.iter().all(|line_ref| {
                line_ref.line_start <= line_ref.line_end
                    && !line_ref.claim_paraphrase.trim().is_empty()
            })
    });
    let all_blockers_covered_by_issue_map = blockers
        .iter()
        .all(|blocker| !blocker.covered_by_issue_ids.is_empty());
    let issue_rows_without_prerequisite = issue_coverage_rows
        .iter()
        .filter(|row| row.issue_role != TassadarArticleEquivalenceIssueRole::Prerequisite)
        .collect::<Vec<_>>();
    let observed_later_issue_ids = issue_rows_without_prerequisite
        .iter()
        .map(|row| row.issue_id.clone())
        .collect::<BTreeSet<_>>();
    let all_later_issues_covered = observed_later_issue_ids == required_later_issue_ids();
    let all_issue_refs_point_to_known_blockers = issue_coverage_rows.iter().all(|row| {
        !row.blocker_ids.is_empty()
            && row
                .blocker_ids
                .iter()
                .all(|blocker_id| blocker_id_set.contains(blocker_id))
    });
    let prerequisite_transformer_boundary_green = issue_coverage_rows.iter().any(|row| {
        row.issue_id == "TAS-156A"
            && row.issue_role == TassadarArticleEquivalenceIssueRole::Prerequisite
            && row.issue_state == "closed"
    });
    let open_blocker_count = blockers
        .iter()
        .filter(|blocker| !blocker.repo_status_closes_blocker())
        .count();
    let matrix_contract_green = prerequisite_transformer_boundary_green
        && all_required_categories_present
        && all_blocker_ids_unique
        && all_blockers_have_article_line_provenance
        && all_blockers_covered_by_issue_map
        && all_later_issues_covered
        && all_issue_refs_point_to_known_blockers;
    let article_equivalence_green = matrix_contract_green && open_blocker_count == 0;

    MatrixEvaluation {
        prerequisite_transformer_boundary_green,
        all_required_categories_present,
        all_blocker_ids_unique,
        all_blockers_have_article_line_provenance,
        all_blockers_covered_by_issue_map,
        all_later_issues_covered,
        all_issue_refs_point_to_known_blockers,
        matrix_contract_green,
        article_equivalence_green,
        open_blocker_count,
    }
}

fn attach_issue_coverage(
    blockers: &mut [TassadarArticleEquivalenceBlockerRow],
    issue_coverage_rows: &[TassadarArticleEquivalenceIssueCoverageRow],
) {
    for blocker in blockers {
        let mut covered_by_issue_ids = issue_coverage_rows
            .iter()
            .filter(|row| {
                row.blocker_ids
                    .iter()
                    .any(|blocker_id| blocker_id == &blocker.blocker_id)
            })
            .map(|row| row.issue_id.clone())
            .collect::<Vec<_>>();
        covered_by_issue_ids.sort();
        covered_by_issue_ids.dedup();
        blocker.covered_by_issue_ids = covered_by_issue_ids;
    }
}

impl TassadarArticleEquivalenceBlockerRow {
    fn repo_status_closes_blocker(&self) -> bool {
        self.repo_status == TassadarRepoStatus::Implemented
    }
}

fn required_categories() -> BTreeSet<TassadarArticleEquivalenceBlockerCategory> {
    BTreeSet::from([
        TassadarArticleEquivalenceBlockerCategory::FrontendScope,
        TassadarArticleEquivalenceBlockerCategory::InterpreterBreadth,
        TassadarArticleEquivalenceBlockerCategory::TransformerStackReality,
        TassadarArticleEquivalenceBlockerCategory::FastRouteScope,
        TassadarArticleEquivalenceBlockerCategory::BenchmarkScope,
        TassadarArticleEquivalenceBlockerCategory::SingleRunScope,
        TassadarArticleEquivalenceBlockerCategory::WeightsOwnershipScope,
    ])
}

fn required_later_issue_ids() -> BTreeSet<String> {
    BTreeSet::from_iter(
        [
            "TAS-158", "TAS-159", "TAS-160", "TAS-161", "TAS-162", "TAS-163", "TAS-164", "TAS-165",
            "TAS-166", "TAS-167", "TAS-167A", "TAS-168", "TAS-169", "TAS-169A", "TAS-170",
            "TAS-171", "TAS-171A", "TAS-171B", "TAS-171C", "TAS-172", "TAS-173", "TAS-174",
            "TAS-175", "TAS-176", "TAS-177", "TAS-178", "TAS-179", "TAS-179A", "TAS-180",
            "TAS-181", "TAS-182", "TAS-183", "TAS-184", "TAS-184A", "TAS-185", "TAS-185A",
            "TAS-186", "TAS-R1",
        ]
        .into_iter()
        .map(String::from),
    )
}

fn blocker_rows() -> Vec<TassadarArticleEquivalenceBlockerRow> {
    vec![
        TassadarArticleEquivalenceBlockerRow {
            blocker_id: String::from("BEQ-001"),
            category: TassadarArticleEquivalenceBlockerCategory::FrontendScope,
            title: String::from(
                "Declared article-equivalent frontend/compiler ingress remains open",
            ),
            repo_status: TassadarRepoStatus::Implemented,
            current_gap_summary: String::from(
                "the public repo now closes the Hungarian and Sudoku article demo sources through one explicit Rust-source-only frontend/compiler envelope and binds them to the same canonical compiled-executor workload identities used by the bounded reproducers, while keeping broader arbitrary-C public claims explicitly out of scope",
            ),
            current_public_truth: String::from(
                "the repo now has one explicit article frontend/compiler envelope manifest and report over Rust-source-only `rustc` -> `wasm32-unknown-unknown` `#![no_std]` / `#![no_main]` sources, a broader committed frontend corpus and compile matrix over arithmetic, branch-heavy, state-machine, allocator-backed-memory, Hungarian-like, and Sudoku-like support families, and one dedicated article-demo frontend parity report that recompiles the canonical Hungarian and Sudoku demo sources through that envelope with stable source/receipt/Wasm parity and the same canonical compiled-executor case and workload identities as the existing bounded reproducers",
            ),
            closure_requirements: vec![
                String::from(
                    "close the Hungarian and Sudoku demo sources through that declared envelope",
                ),
            ],
            article_line_refs: vec![
                line_ref(
                    "article.frontend.arbitrary_c",
                    142,
                    142,
                    "the article claims arbitrary C code can be turned into tokens and executed by the model",
                ),
                line_ref(
                    "article.frontend.c_cpp_to_wasm",
                    238,
                    238,
                    "the article frames the interpreter as a WebAssembly target that languages such as C and C++ can compile to",
                ),
            ],
            covered_by_issue_ids: Vec::new(),
        },
        TassadarArticleEquivalenceBlockerRow {
            blocker_id: String::from("BEQ-002"),
            category: TassadarArticleEquivalenceBlockerCategory::InterpreterBreadth,
            title: String::from("Declared article interpreter breadth remains open"),
            repo_status: TassadarRepoStatus::Implemented,
            current_gap_summary: String::from(
                "the public repo now closes the article-interpreter breadth blocker inside the declared TAS-179 envelope by proving one green generic family suite over arithmetic, call-heavy, allocator-backed, indirect-call, branch-heavy, loop-heavy, state-machine, and parser-style rows while keeping arbitrary-program language explicitly out of scope",
            ),
            current_public_truth: String::from(
                "the repo now has a machine-readable article interpreter breadth envelope plus one green TAS-179A suite gate over the declared generic article-program families, with linked-program bundles still research-only and broader import-mediated, dynamic-memory, memory64, multi-memory, component-linking, exception, and float-semantics families still explicit outside the article envelope",
            ),
            closure_requirements: vec![
                String::from(
                    "declare one article interpreter breadth envelope rather than relying on bounded Rust-only article fixtures",
                ),
                String::from(
                    "build a broad program-family suite over the declared article interpreter envelope",
                ),
                String::from(
                    "keep refusal and unsupported-program taxonomy machine-legible when breadth remains incomplete",
                ),
            ],
            article_line_refs: vec![
                line_ref(
                    "article.interpreter.arbitrary_programs",
                    218,
                    218,
                    "the article claims arbitrary programs can execute inside a transformer",
                ),
                line_ref(
                    "article.interpreter.same_transformer_execution",
                    236,
                    247,
                    "the article claims the transformer executes the emitted program itself step by step within the same transformer",
                ),
                line_ref(
                    "article.interpreter.wasm_in_weights",
                    238,
                    238,
                    "the article claims a WebAssembly interpreter inside the transformer weights",
                ),
            ],
            covered_by_issue_ids: Vec::new(),
        },
        TassadarArticleEquivalenceBlockerRow {
            blocker_id: String::from("BEQ-003"),
            category: TassadarArticleEquivalenceBlockerCategory::TransformerStackReality,
            title: String::from("Canonical owned Transformer-backed article route is now closed"),
            repo_status: TassadarRepoStatus::Implemented,
            current_gap_summary: String::from(
                "the remaining gap was one canonical owned Transformer stack plus one non-fixture article model artifact carrying the public route end to end; that gap is now closed",
            ),
            current_public_truth: String::from(
                "the repo now has one canonical owned `psionic-transformer` route, one canonical trained article model artifact with weight lineage, and one bounded public article route that stays inside that owned stack end to end",
            ),
            closure_requirements: vec![
                String::from(
                    "freeze one canonical Transformer stack boundary above the existing Psionic substrate",
                ),
                String::from(
                    "implement the owned paper-faithful article Transformer path with training, replay, and receipts",
                ),
                String::from(
                    "replace the current fixture-shaped article model surface with a real artifact-backed Transformer model and weight lineage",
                ),
            ],
            article_line_refs: vec![
                line_ref(
                    "article.execution.transformer_weights",
                    180,
                    180,
                    "the article says execution happens directly via transformer weights rather than an external tool",
                ),
                line_ref(
                    "article.execution.same_transformer_program",
                    236,
                    247,
                    "the article says the same transformer emits and executes the program",
                ),
                line_ref(
                    "article.weights.learned_interpreter",
                    821,
                    821,
                    "the article says the interpreter behavior is encoded in the model weights and could later compile programs directly into weights",
                ),
            ],
            covered_by_issue_ids: Vec::new(),
        },
        TassadarArticleEquivalenceBlockerRow {
            blocker_id: String::from("BEQ-004"),
            category: TassadarArticleEquivalenceBlockerCategory::FastRouteScope,
            title: String::from("Canonical fast decode route remains open"),
            repo_status: TassadarRepoStatus::Implemented,
            current_gap_summary: String::from(
                "the selected HullCache fast path now has canonical route selection, owned-model integration, exactness, no-fallback closure, and explicit throughput-floor evidence on the committed article workloads; later issues still own frontend breadth, final demo parity, and benchmark-wide closure",
            ),
            current_public_truth: String::from(
                "the repo now has one canonical fast-route stack with machine-legible selection, exactness, and throughput-floor artifacts on the bounded committed workloads, while later tranches keep final article-equivalence green status red",
            ),
            closure_requirements: vec![
                String::from(
                    "choose one fast architecture that can become the canonical article route",
                ),
                String::from(
                    "integrate that fast path into the Transformer-backed article model itself",
                ),
                String::from(
                    "close exactness, no-fallback, and throughput floors on the selected fast path",
                ),
            ],
            article_line_refs: vec![
                line_ref(
                    "article.fast_route.log_time_summary",
                    182,
                    182,
                    "the article says the decoding path turns attention lookups into logarithmic-time queries and enables millions of steps in a single run",
                ),
                line_ref(
                    "article.fast_route.technical_unlock",
                    216,
                    216,
                    "the article says the key technical unlock is a log-time decoding path enabled by 2D lookup heads",
                ),
                line_ref(
                    "article.fast_route.exponential_speedup",
                    440,
                    452,
                    "the article claims exponentially faster attention lookups with logarithmic-time work per step",
                ),
                line_ref(
                    "article.fast_route.long_horizon_payoff",
                    582,
                    582,
                    "the article says the hull-based decoder changes long-horizon feasibility by keeping per-step cost logarithmic",
                ),
            ],
            covered_by_issue_ids: Vec::new(),
        },
        TassadarArticleEquivalenceBlockerRow {
            blocker_id: String::from("BEQ-005"),
            category: TassadarArticleEquivalenceBlockerCategory::BenchmarkScope,
            title: String::from("Unified article demo-and-benchmark gate is now closed"),
            repo_status: TassadarRepoStatus::Implemented,
            current_gap_summary: String::from(
                "the repo now closes the article's combined demo-plus-benchmark claim surface inside one canonical joined parity gate over the Hungarian demo, the named Arto row, and the declared hard-Sudoku suite",
            ),
            current_public_truth: String::from(
                "the repo now has one canonical article demo-and-benchmark equivalence gate that binds bounded Hungarian fast-route parity, the named Arto Inkala row, the declared hard-Sudoku suite closure, and the owned-route boundary into one machine-readable benchmark-scope closure surface at the article's stated breadth",
            ),
            closure_requirements: vec![
                String::from(
                    "combine those rows into one article demo-and-benchmark equivalence gate",
                ),
            ],
            article_line_refs: vec![
                line_ref(
                    "article.hungarian.demo_throughput",
                    160,
                    163,
                    "the Hungarian demo publishes concrete token and line throughput plus total-token figures",
                ),
                line_ref(
                    "article.hungarian.cpu_throughput_claim",
                    180,
                    180,
                    "the article states the model streams results at more than 30k tokens per second on CPU",
                ),
                line_ref(
                    "article.sudoku.benchmark_accuracy",
                    292,
                    292,
                    "the article claims 100 percent accuracy on the hard-Sudoku benchmarks it highlights",
                ),
                line_ref(
                    "article.sudoku.arto_runtime",
                    296,
                    296,
                    "the article claims Arto Inkala's Sudoku is solved in under 3 minutes",
                ),
                line_ref(
                    "article.sudoku.demo_throughput",
                    311,
                    314,
                    "the Sudoku demo publishes concrete token, line, and total-token figures",
                ),
            ],
            covered_by_issue_ids: Vec::new(),
        },
        TassadarArticleEquivalenceBlockerRow {
            blocker_id: String::from("BEQ-006"),
            category: TassadarArticleEquivalenceBlockerCategory::SingleRunScope,
            title: String::from("Single-run no-spill article posture is closed"),
            repo_status: TassadarRepoStatus::Implemented,
            current_gap_summary: String::from(
                "the repo now freezes one no-resume single-run operator envelope over the canonical article route boundary, keeps long-horizon exactness and throughput-floor receipts explicit, and treats checkpoint, spill, and continuation lanes as negative controls instead of inheriting them into article closure",
            ),
            current_public_truth: String::from(
                "the repo now has one machine-readable TAS-183 gate over benchmark parity, million-step and multi-million-step horizon receipts, step-to-compute consistency, context-length sensitivity, and explicit checkpoint/spill/resume negative controls while keeping final weight-causality and route-minimality verdicts separate",
            ),
            closure_requirements: vec![
                String::from(
                    "freeze one no-resume article operator envelope over the canonical route",
                ),
                String::from(
                    "prove exact long-horizon execution with no checkpoint restore, spill/tape extension, hidden re-entry, or teacher forcing",
                ),
                String::from(
                    "keep context-length sensitivity and no-cheat boundary perturbations explicit",
                ),
            ],
            article_line_refs: vec![
                line_ref(
                    "article.single_run.million_steps",
                    142,
                    142,
                    "the article says arbitrary C-code programs can execute reliably for millions of steps in seconds",
                ),
                line_ref(
                    "article.single_run.log_time_enables_single_run",
                    182,
                    182,
                    "the article says the fast path enables millions of correct execution steps inside a single transformer run",
                ),
                line_ref(
                    "article.single_run.same_transformer",
                    236,
                    247,
                    "the article says execution stays within the same transformer instead of pausing for an external tool",
                ),
            ],
            covered_by_issue_ids: Vec::new(),
        },
        TassadarArticleEquivalenceBlockerRow {
            blocker_id: String::from("BEQ-007"),
            category: TassadarArticleEquivalenceBlockerCategory::WeightsOwnershipScope,
            title: String::from(
                "Clean-room weight causality and route-minimality verdict are closed",
            ),
            repo_status: TassadarRepoStatus::Implemented,
            current_gap_summary: String::from(
                "the article invites a strong reading where decisive execution behavior lives in transformer weights on a minimal route. The public repo now closes the joined interpreter-ownership verdict, the explicit KV-cache plus activation-state discipline verdict, the cross-machine reproducibility matrix, the direct-route minimality plus publication verdict split, and the final bounded article-equivalence claim checker for that stronger reading",
            ),
            current_public_truth: String::from(
                "the repo now has one canonical TAS-184 interpreter-ownership gate, one TAS-184A KV-cache and activation-state discipline audit, one TAS-185 cross-machine reproducibility matrix, one TAS-185A route-minimality audit plus publication verdict, and one TAS-186 final article-equivalence claim checker plus audit, together making the bounded route's weight sensitivity, mixed same-run state carrier posture, declared machine-class stability, direct-route ownership boundary, and final bounded article-equivalence verdict machine-readable",
            ),
            closure_requirements: vec![
                String::from(
                    "move direct no-tool proof and exactness onto the Transformer-backed route",
                ),
                String::from(
                    "close invariance, anti-memorization, and evaluation-independence evidence so correctness does not look like brittle recall",
                ),
                String::from(
                    "publish clean-room weight causality, KV-cache and activation-state discipline, route-minimality, and explicit operator versus public publication verdicts",
                ),
            ],
            article_line_refs: vec![
                line_ref(
                    "article.weights.no_external_tool",
                    180,
                    180,
                    "the article says the model does not call an external tool and executes directly via transformer weights",
                ),
                line_ref(
                    "article.weights.transparent_execution",
                    289,
                    289,
                    "the article contrasts in-model execution as transparent and fully inside the model loop",
                ),
                line_ref(
                    "article.weights.compiled_solver_inside_transformer",
                    294,
                    296,
                    "the article says the compiled Sudoku solver runs inside the transformer itself with no verifier gap",
                ),
                line_ref(
                    "article.weights.interpreter_in_weights",
                    821,
                    821,
                    "the article says the current prototype learns an interpreter whose behavior is encoded in its weights",
                ),
            ],
            covered_by_issue_ids: Vec::new(),
        },
    ]
}

fn issue_coverage_rows() -> Vec<TassadarArticleEquivalenceIssueCoverageRow> {
    vec![
        issue_row(
            "TAS-156A",
            273,
            "TAS-156A: Create psionic-transformer crate boundary",
            TassadarArticleEquivalenceIssueRole::Prerequisite,
            "closed",
            &["BEQ-003"],
        ),
        issue_row(
            "TAS-158",
            275,
            "TAS-158: Final article-equivalence acceptance gate skeleton",
            TassadarArticleEquivalenceIssueRole::Gate,
            "closed",
            &[
                "BEQ-001", "BEQ-002", "BEQ-003", "BEQ-004", "BEQ-005", "BEQ-006", "BEQ-007",
            ],
        ),
        issue_row(
            "TAS-159",
            276,
            "TAS-159: Existing substrate inventory and reuse boundary",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003"],
        ),
        issue_row(
            "TAS-160",
            277,
            "TAS-160: Canonical Transformer stack boundary",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003"],
        ),
        issue_row(
            "TAS-161",
            278,
            "TAS-161: Attention primitive and mask closure",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003"],
        ),
        issue_row(
            "TAS-162",
            279,
            "TAS-162: Multi-head, feed-forward, residual, and norm block closure",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003"],
        ),
        issue_row(
            "TAS-163",
            280,
            "TAS-163: Paper-faithful article-Transformer model closure",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003"],
        ),
        issue_row(
            "TAS-164",
            281,
            "TAS-164: Training recipe and tiny-task closure",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003"],
        ),
        issue_row(
            "TAS-165",
            282,
            "TAS-165: Transformer forward-pass replay and receipt closure",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003"],
        ),
        issue_row(
            "TAS-166",
            283,
            "TAS-166: Owned Transformer stack audit",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003"],
        ),
        issue_row(
            "TAS-167",
            284,
            "TAS-167: Article trace vocabulary and channel binding for the owned stack",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003", "BEQ-007"],
        ),
        issue_row(
            "TAS-167A",
            285,
            "TAS-167A: Prompt, tokenization, and representation invariance gate",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-007"],
        ),
        issue_row(
            "TAS-168",
            286,
            "TAS-168: Transformer-backed article model descriptor and weight artifact",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003"],
        ),
        issue_row(
            "TAS-169",
            287,
            "TAS-169: Transformer-backed article model weight production run",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003"],
        ),
        issue_row(
            "TAS-169A",
            288,
            "TAS-169A: Transformer-backed article weight lineage and artifact contract",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003", "BEQ-007"],
        ),
        issue_row(
            "TAS-170",
            289,
            "TAS-170: Fixture-to-Transformer parity harness",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003", "BEQ-007"],
        ),
        issue_row(
            "TAS-171",
            290,
            "TAS-171: Reference-linear direct-proof closure on the Transformer-backed route",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003", "BEQ-007"],
        ),
        issue_row(
            "TAS-171A",
            291,
            "TAS-171A: Reference-linear Transformer-backed article exactness gate",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-003", "BEQ-005", "BEQ-007"],
        ),
        issue_row(
            "TAS-171B",
            292,
            "TAS-171B: Generalization and anti-memorization gate",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-002", "BEQ-005", "BEQ-007"],
        ),
        issue_row(
            "TAS-171C",
            293,
            "TAS-171C: Dataset contamination and evaluation independence audit",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-005", "BEQ-007"],
        ),
        issue_row(
            "TAS-172",
            294,
            "TAS-172: Canonical fast-route architecture selection",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-004"],
        ),
        issue_row(
            "TAS-173",
            295,
            "TAS-173: Fast-path implementation inside the Transformer-backed model",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-004"],
        ),
        issue_row(
            "TAS-174",
            296,
            "TAS-174: Fast-route exactness and no-fallback closure",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-004"],
        ),
        issue_row(
            "TAS-175",
            297,
            "TAS-175: Fast-route throughput floor closeout",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-004", "BEQ-005"],
        ),
        issue_row(
            "TAS-176",
            298,
            "TAS-176: Declared article frontend/compiler envelope",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-001"],
        ),
        issue_row(
            "TAS-177",
            299,
            "TAS-177: Frontend corpus and compile-matrix expansion",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-001"],
        ),
        issue_row(
            "TAS-178",
            300,
            "TAS-178: Article-demo frontend parity",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-001", "BEQ-005"],
        ),
        issue_row(
            "TAS-179",
            301,
            "TAS-179: Declared article interpreter breadth envelope",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-002"],
        ),
        issue_row(
            "TAS-179A",
            302,
            "TAS-179A: Article interpreter breadth suite and gate",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-002"],
        ),
        issue_row(
            "TAS-180",
            303,
            "TAS-180: Hungarian article demo parity",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-004", "BEQ-005"],
        ),
        issue_row(
            "TAS-181",
            304,
            "TAS-181: Arto Inkala and hard-Sudoku benchmark closure",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-004", "BEQ-005"],
        ),
        issue_row(
            "TAS-182",
            305,
            "TAS-182: Article demo and benchmark equivalence gate",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-005"],
        ),
        issue_row(
            "TAS-183",
            306,
            "TAS-183: Single-run no-spill million-step closure",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-006"],
        ),
        issue_row(
            "TAS-184",
            307,
            "TAS-184: Clean-room weight causality and interpreter ownership gate",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-007"],
        ),
        issue_row(
            "TAS-184A",
            308,
            "TAS-184A: KV-cache and activation-state discipline audit",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-007"],
        ),
        issue_row(
            "TAS-185",
            309,
            "TAS-185: Cross-machine reproducibility matrix",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-004", "BEQ-005", "BEQ-006", "BEQ-007"],
        ),
        issue_row(
            "TAS-185A",
            310,
            "TAS-185A: Route minimality audit and publication verdict",
            TassadarArticleEquivalenceIssueRole::Implementation,
            "closed",
            &["BEQ-007"],
        ),
        issue_row(
            "TAS-186",
            311,
            "TAS-186: Final article-equivalence claim checker and audit",
            TassadarArticleEquivalenceIssueRole::FinalAudit,
            "closed",
            &[
                "BEQ-001", "BEQ-002", "BEQ-003", "BEQ-004", "BEQ-005", "BEQ-006", "BEQ-007",
            ],
        ),
        issue_row(
            "TAS-R1",
            312,
            "TAS-R1: Minimal Transformer size for article-equivalent behavior",
            TassadarArticleEquivalenceIssueRole::OptionalResearch,
            "open",
            &["BEQ-003", "BEQ-004", "BEQ-007"],
        ),
    ]
}

fn line_ref(
    claim_ref_id: &str,
    line_start: u16,
    line_end: u16,
    claim_paraphrase: &str,
) -> TassadarArticleLineProvenance {
    TassadarArticleLineProvenance {
        claim_ref_id: String::from(claim_ref_id),
        line_start,
        line_end,
        claim_paraphrase: String::from(claim_paraphrase),
    }
}

fn issue_row(
    issue_id: &str,
    github_issue_number: u16,
    title: &str,
    issue_role: TassadarArticleEquivalenceIssueRole,
    issue_state: &str,
    blocker_ids: &[&str],
) -> TassadarArticleEquivalenceIssueCoverageRow {
    TassadarArticleEquivalenceIssueCoverageRow {
        issue_id: String::from(issue_id),
        github_issue_number,
        title: String::from(title),
        issue_role,
        issue_state: String::from(issue_state),
        blocker_ids: blocker_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarArticleEquivalenceBlockerMatrixReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarArticleEquivalenceBlockerMatrixReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleEquivalenceBlockerMatrixReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_equivalence_blocker_matrix_report, evaluate_matrix,
        issue_coverage_rows, read_json, tassadar_article_equivalence_blocker_matrix_report_path,
        write_tassadar_article_equivalence_blocker_matrix_report,
        TassadarArticleEquivalenceBlockerMatrixReport, TassadarArticleEquivalenceIssueRole,
        TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
    };

    #[test]
    fn article_equivalence_blocker_matrix_turns_green_when_all_required_rows_close() {
        let report = build_tassadar_article_equivalence_blocker_matrix_report().expect("report");

        assert!(report.prerequisite_transformer_boundary_green);
        assert!(report.matrix_contract_green);
        assert!(report.article_equivalence_green);
        assert_eq!(report.blocker_count, 7);
        assert_eq!(report.open_blocker_count, 0);
        assert!(report.all_later_issues_covered);
        assert!(report.all_blockers_have_article_line_provenance);
    }

    #[test]
    fn article_equivalence_blocker_matrix_matches_committed_truth() {
        let generated = build_tassadar_article_equivalence_blocker_matrix_report().expect("report");
        let committed: TassadarArticleEquivalenceBlockerMatrixReport =
            read_json(tassadar_article_equivalence_blocker_matrix_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_article_equivalence_blocker_matrix_report.json"
        );
    }

    #[test]
    fn write_article_equivalence_blocker_matrix_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_equivalence_blocker_matrix_report.json");
        let written = write_tassadar_article_equivalence_blocker_matrix_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleEquivalenceBlockerMatrixReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_equivalence_blocker_matrix_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_equivalence_blocker_matrix_report.json")
        );
    }

    #[test]
    fn missing_blocker_provenance_keeps_contract_red() {
        let mut report =
            build_tassadar_article_equivalence_blocker_matrix_report().expect("report");
        report.blockers[0].article_line_refs.clear();
        let evaluation = evaluate_matrix(
            report.blockers.as_slice(),
            report.issue_coverage_rows.as_slice(),
        );
        assert!(!evaluation.matrix_contract_green);
        assert!(!evaluation.article_equivalence_green);
    }

    #[test]
    fn missing_later_issue_coverage_keeps_contract_red() {
        let report = build_tassadar_article_equivalence_blocker_matrix_report().expect("report");
        let mut issue_rows = issue_coverage_rows();
        issue_rows.retain(|row| row.issue_id != "TAS-186");
        assert!(issue_rows.iter().all(|row| row.issue_role
            != TassadarArticleEquivalenceIssueRole::Prerequisite
            || row.issue_state == "closed"));
        let evaluation = evaluate_matrix(report.blockers.as_slice(), issue_rows.as_slice());
        assert!(!evaluation.matrix_contract_green);
        assert!(!evaluation.article_equivalence_green);
    }
}
