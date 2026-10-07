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

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_existing_substrate_inventory_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarExistingSubstrateInventoryReport,
    TassadarExistingSubstrateInventoryReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_EXISTING_SUBSTRATE_INVENTORY_REPORT_REF,
};

pub const TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_canonical_transformer_stack_boundary_report.json";

const BOUNDARY_DOC_REF: &str = "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const TIED_REQUIREMENT_ID: &str = "TAS-160";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarTransformerBoundaryDecision {
    ExplicitBoundarySpanningExistingCrates,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarTransformerBoundaryInterfaceKind {
    TensorArrayOps,
    LayerAndParameterState,
    ModelArtifactFormat,
    ForwardPassTraceHooks,
    ReplayAndReceiptHooks,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTransformerBoundaryInterfaceRow {
    pub interface_kind: TassadarTransformerBoundaryInterfaceKind,
    pub owner_modules: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTransformerBoundaryOwnershipRow {
    pub crate_id: String,
    pub module_ref: String,
    pub ownership_role: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTransformerBoundaryDependencyCheckRow {
    pub check_id: String,
    pub cargo_toml_ref: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTransformerBoundaryAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub existing_substrate_inventory_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCanonicalTransformerStackBoundaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub boundary_doc_ref: String,
    pub boundary_decision: TassadarTransformerBoundaryDecision,
    pub acceptance_gate_tie: TassadarTransformerBoundaryAcceptanceGateTie,
    pub existing_substrate_inventory_report_ref: String,
    pub existing_substrate_inventory_report: TassadarExistingSubstrateInventoryReport,
    pub interface_rows: Vec<TassadarTransformerBoundaryInterfaceRow>,
    pub ownership_rows: Vec<TassadarTransformerBoundaryOwnershipRow>,
    pub ownership_diagram_lines: Vec<String>,
    pub dependency_checks: Vec<TassadarTransformerBoundaryDependencyCheckRow>,
    pub all_interfaces_covered: bool,
    pub all_dependency_checks_pass: bool,
    pub all_ownership_rows_have_real_module_refs: bool,
    pub canonical_route_required_boundary: bool,
    pub boundary_contract_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarCanonicalTransformerStackBoundaryReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Inventory(#[from] TassadarExistingSubstrateInventoryReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
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

pub fn build_tassadar_canonical_transformer_stack_boundary_report() -> Result<
    TassadarCanonicalTransformerStackBoundaryReport,
    TassadarCanonicalTransformerStackBoundaryReportError,
> {
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let inventory_report = build_tassadar_existing_substrate_inventory_report()?;
    Ok(build_report_from_inputs(
        acceptance_gate_report,
        inventory_report,
        interface_rows(),
        ownership_rows(),
        dependency_checks()?,
    ))
}

fn build_report_from_inputs(
    acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    inventory_report: TassadarExistingSubstrateInventoryReport,
    interface_rows: Vec<TassadarTransformerBoundaryInterfaceRow>,
    ownership_rows: Vec<TassadarTransformerBoundaryOwnershipRow>,
    dependency_checks: Vec<TassadarTransformerBoundaryDependencyCheckRow>,
) -> TassadarCanonicalTransformerStackBoundaryReport {
    let acceptance_gate_tie = TassadarTransformerBoundaryAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        existing_substrate_inventory_report_ref: String::from(
            TASSADAR_EXISTING_SUBSTRATE_INVENTORY_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate_report
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate_report.acceptance_status,
        blocked_issue_ids: acceptance_gate_report.blocked_issue_ids.clone(),
    };
    let all_interfaces_covered = interface_rows
        .iter()
        .map(|row| row.interface_kind)
        .collect::<BTreeSet<_>>()
        == required_interfaces();
    let all_dependency_checks_pass = dependency_checks.iter().all(|row| row.passed);
    let all_ownership_rows_have_real_module_refs = ownership_rows.iter().all(|row| {
        !row.crate_id.trim().is_empty()
            && !row.module_ref.trim().is_empty()
            && !row.ownership_role.trim().is_empty()
    });
    let canonical_route_required_boundary = true;
    let boundary_contract_green = acceptance_gate_tie.tied_requirement_satisfied
        && inventory_report.inventory_contract_green
        && all_interfaces_covered
        && all_dependency_checks_pass
        && all_ownership_rows_have_real_module_refs
        && canonical_route_required_boundary;
    let article_equivalence_green =
        boundary_contract_green && acceptance_gate_report.article_equivalence_green;
    let mut report = TassadarCanonicalTransformerStackBoundaryReport {
        schema_version: 1,
        report_id: String::from("tassadar.canonical_transformer_stack_boundary.report.v1"),
        boundary_doc_ref: String::from(BOUNDARY_DOC_REF),
        boundary_decision: TassadarTransformerBoundaryDecision::ExplicitBoundarySpanningExistingCrates,
        acceptance_gate_tie,
        existing_substrate_inventory_report_ref: String::from(
            TASSADAR_EXISTING_SUBSTRATE_INVENTORY_REPORT_REF,
        ),
        existing_substrate_inventory_report: inventory_report,
        interface_rows,
        ownership_rows,
        ownership_diagram_lines: ownership_diagram_lines(),
        dependency_checks,
        all_interfaces_covered,
        all_dependency_checks_pass,
        all_ownership_rows_have_real_module_refs,
        canonical_route_required_boundary,
        boundary_contract_green,
        article_equivalence_green,
        claim_boundary: String::from(
            "this report freezes one explicit canonical Transformer stack boundary for the article-equivalence closure wave. The route is not one monolithic crate; it is one required boundary spanning existing crates with `psionic-transformer` as the architecture anchor, `psionic-models` as the article-model and artifact owner, and `psionic-runtime` as the replay/receipt owner. This does not claim the final article-equivalence stack is complete or green.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Canonical Transformer stack boundary now records interface_rows={}, dependency_checks={}, tied_requirement_satisfied={}, boundary_contract_green={}, and article_equivalence_green={}.",
        report.interface_rows.len(),
        report.dependency_checks.len(),
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.boundary_contract_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_canonical_transformer_stack_boundary_report|",
        &report,
    );
    report
}

fn required_interfaces() -> BTreeSet<TassadarTransformerBoundaryInterfaceKind> {
    BTreeSet::from([
        TassadarTransformerBoundaryInterfaceKind::TensorArrayOps,
        TassadarTransformerBoundaryInterfaceKind::LayerAndParameterState,
        TassadarTransformerBoundaryInterfaceKind::ModelArtifactFormat,
        TassadarTransformerBoundaryInterfaceKind::ForwardPassTraceHooks,
        TassadarTransformerBoundaryInterfaceKind::ReplayAndReceiptHooks,
    ])
}

fn interface_rows() -> Vec<TassadarTransformerBoundaryInterfaceRow> {
    vec![
        interface_row(
            TassadarTransformerBoundaryInterfaceKind::TensorArrayOps,
            &["crates/psionic-core/src/lib.rs", "crates/psionic-array/src/lib.rs"],
            "tensor metadata, device/layout truth, and bounded array materialization stay below the canonical article route and are reused through `psionic-core` plus `psionic-array`",
        ),
        interface_row(
            TassadarTransformerBoundaryInterfaceKind::LayerAndParameterState,
            &[
                "crates/psionic-nn/src/lib.rs",
                "crates/psionic-nn/src/layers.rs",
                "crates/psionic-transformer/src/lib.rs",
                "crates/psionic-transformer/src/attention.rs",
                "crates/psionic-transformer/src/blocks.rs",
                "crates/psionic-transformer/src/encoder_decoder.rs",
            ],
            "module state, primitive layers, and reusable transformer attention plus encoder-decoder composition stay split between `psionic-nn` and `psionic-transformer`, with `psionic-transformer` as the architecture anchor",
        ),
        interface_row(
            TassadarTransformerBoundaryInterfaceKind::ModelArtifactFormat,
            &[
                "crates/psionic-models/src/lib.rs",
                "crates/psionic-models/src/tassadar_article_transformer.rs",
            ],
            "canonical article-model descriptors, canonical route selection, and later artifact identity stay owned by `psionic-models` and must consume the `psionic-transformer` boundary rather than bypass it",
        ),
        interface_row(
            TassadarTransformerBoundaryInterfaceKind::ForwardPassTraceHooks,
            &[
                "crates/psionic-models/src/tassadar_article_transformer.rs",
                "crates/psionic-runtime/src/tassadar.rs",
            ],
            "forward-pass trace hooks stay model-owned at the article-model boundary and serialize into runtime-owned trace ABI surfaces instead of inventing a second trace layer",
        ),
        interface_row(
            TassadarTransformerBoundaryInterfaceKind::ReplayAndReceiptHooks,
            &["crates/psionic-runtime/src/proof.rs", "crates/psionic-runtime/src/tassadar.rs"],
            "replay, proof, trace receipt, and runtime identity hooks stay owned by `psionic-runtime` as the canonical receipt boundary for the article route",
        ),
    ]
}

fn interface_row(
    interface_kind: TassadarTransformerBoundaryInterfaceKind,
    owner_modules: &[&str],
    detail: &str,
) -> TassadarTransformerBoundaryInterfaceRow {
    TassadarTransformerBoundaryInterfaceRow {
        interface_kind,
        owner_modules: owner_modules
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}

fn ownership_rows() -> Vec<TassadarTransformerBoundaryOwnershipRow> {
    vec![
        ownership_row(
            "psionic-core",
            "crates/psionic-core/src/lib.rs",
            "tensor ids, dtypes, shapes, devices, layouts, and typed refusals",
        ),
        ownership_row(
            "psionic-array",
            "crates/psionic-array/src/lib.rs",
            "bounded array materialization, graph-backed arithmetic, and public array runtime surface",
        ),
        ownership_row(
            "psionic-nn",
            "crates/psionic-nn/src/layers.rs",
            "module state plus reusable primitive layer implementations",
        ),
        ownership_row(
            "psionic-transformer",
            "crates/psionic-transformer/src/encoder_decoder.rs",
            "canonical reusable transformer architecture boundary plus owned attention, embeddings, encoder-decoder stack composition, feed-forward, residual, and norm logic above primitive layers and below model artifacts",
        ),
        ownership_row(
            "psionic-models",
            "crates/psionic-models/src/tassadar_article_transformer.rs",
            "canonical article-route model wrapper, config, route descriptor, and forward-pass surface owner",
        ),
        ownership_row(
            "psionic-runtime",
            "crates/psionic-runtime/src/proof.rs",
            "runtime proof identity, replay, trace receipt, and execution challenge boundary",
        ),
    ]
}

fn ownership_row(
    crate_id: &str,
    module_ref: &str,
    ownership_role: &str,
) -> TassadarTransformerBoundaryOwnershipRow {
    TassadarTransformerBoundaryOwnershipRow {
        crate_id: String::from(crate_id),
        module_ref: String::from(module_ref),
        ownership_role: String::from(ownership_role),
    }
}

fn ownership_diagram_lines() -> Vec<String> {
    vec![
        String::from("psionic-core + psionic-array -> tensor/array ops"),
        String::from("psionic-nn + psionic-transformer -> layer/state + architecture boundary"),
        String::from("psionic-models -> canonical article model artifact + forward-pass hooks"),
        String::from("psionic-runtime -> replay, trace, and receipt hooks"),
    ]
}

fn dependency_checks() -> Result<
    Vec<TassadarTransformerBoundaryDependencyCheckRow>,
    TassadarCanonicalTransformerStackBoundaryReportError,
> {
    let models_cargo = read_repo_file("crates/psionic-models/Cargo.toml")?;
    let transformer_cargo = read_repo_file("crates/psionic-transformer/Cargo.toml")?;
    let nn_cargo = read_repo_file("crates/psionic-nn/Cargo.toml")?;
    let runtime_cargo = read_repo_file("crates/psionic-runtime/Cargo.toml")?;
    Ok(vec![
        dependency_check_row(
            "psionic_models_depends_on_psionic_transformer",
            "crates/psionic-models/Cargo.toml",
            cargo_toml_has_dependency(models_cargo.as_str(), "psionic-transformer"),
            "`psionic-models` must depend on `psionic-transformer` so the canonical article model artifact cannot bypass the architecture boundary",
        ),
        dependency_check_row(
            "psionic_transformer_depends_on_psionic_nn",
            "crates/psionic-transformer/Cargo.toml",
            cargo_toml_has_dependency(transformer_cargo.as_str(), "psionic-nn"),
            "`psionic-transformer` must depend on `psionic-nn` so reusable block composition stays above the layer substrate instead of rebuilding private layer logic",
        ),
        dependency_check_row(
            "psionic_transformer_avoids_models_and_runtime",
            "crates/psionic-transformer/Cargo.toml",
            !cargo_toml_has_dependency(transformer_cargo.as_str(), "psionic-models")
                && !cargo_toml_has_dependency(transformer_cargo.as_str(), "psionic-runtime"),
            "`psionic-transformer` must stay below `psionic-models` and `psionic-runtime` so the architecture boundary remains reusable and not product-coupled",
        ),
        dependency_check_row(
            "psionic_nn_avoids_train_models_and_runtime",
            "crates/psionic-nn/Cargo.toml",
            !cargo_toml_has_dependency(nn_cargo.as_str(), "psionic-train")
                && !cargo_toml_has_dependency(nn_cargo.as_str(), "psionic-models")
                && !cargo_toml_has_dependency(nn_cargo.as_str(), "psionic-runtime"),
            "`psionic-nn` must stay below training, model, and runtime crates so it can act as the lower-level layer and module substrate for the canonical transformer route",
        ),
        dependency_check_row(
            "psionic_runtime_avoids_models_dependency",
            "crates/psionic-runtime/Cargo.toml",
            !cargo_toml_has_dependency(runtime_cargo.as_str(), "psionic-models"),
            "`psionic-runtime` must not depend on `psionic-models`; replay and receipt hooks stay runtime-owned and model-agnostic",
        ),
        dependency_check_row(
            "psionic_models_avoids_eval_and_research_dependency",
            "crates/psionic-models/Cargo.toml",
            !cargo_toml_has_dependency(models_cargo.as_str(), "psionic-eval")
                && !cargo_toml_has_dependency(models_cargo.as_str(), "psionic-research"),
            "`psionic-models` must stay below eval and research crates so the canonical article route uses real runtime/model modules rather than review-only layers",
        ),
    ])
}

fn dependency_check_row(
    check_id: &str,
    cargo_toml_ref: &str,
    passed: bool,
    detail: &str,
) -> TassadarTransformerBoundaryDependencyCheckRow {
    TassadarTransformerBoundaryDependencyCheckRow {
        check_id: String::from(check_id),
        cargo_toml_ref: String::from(cargo_toml_ref),
        passed,
        detail: String::from(detail),
    }
}

fn cargo_toml_has_dependency(contents: &str, dependency: &str) -> bool {
    contents
        .lines()
        .any(|line| line.trim_start().starts_with(&format!("{dependency} =")))
}

fn read_repo_file(
    relative_path: &str,
) -> Result<String, TassadarCanonicalTransformerStackBoundaryReportError> {
    let path = repo_root().join(relative_path);
    fs::read_to_string(&path).map_err(|error| {
        TassadarCanonicalTransformerStackBoundaryReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })
}

#[must_use]
pub fn tassadar_canonical_transformer_stack_boundary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF)
}

pub fn write_tassadar_canonical_transformer_stack_boundary_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarCanonicalTransformerStackBoundaryReport,
    TassadarCanonicalTransformerStackBoundaryReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarCanonicalTransformerStackBoundaryReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_canonical_transformer_stack_boundary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarCanonicalTransformerStackBoundaryReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
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
) -> Result<T, TassadarCanonicalTransformerStackBoundaryReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarCanonicalTransformerStackBoundaryReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarCanonicalTransformerStackBoundaryReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_report_from_inputs, build_tassadar_canonical_transformer_stack_boundary_report,
        dependency_checks, interface_rows, ownership_rows, read_json,
        tassadar_canonical_transformer_stack_boundary_report_path,
        write_tassadar_canonical_transformer_stack_boundary_report,
        TassadarCanonicalTransformerStackBoundaryReport,
    };
    use crate::{
        build_tassadar_article_equivalence_acceptance_gate_report,
        build_tassadar_existing_substrate_inventory_report,
    };

    #[test]
    fn canonical_transformer_stack_boundary_is_tied_and_blocked_until_later_work() {
        let report =
            build_tassadar_canonical_transformer_stack_boundary_report().expect("boundary");

        assert!(report.boundary_contract_green);
        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.article_equivalence_green);
        assert_eq!(report.interface_rows.len(), 5);
        assert_eq!(report.dependency_checks.len(), 6);
        assert!(report.all_dependency_checks_pass);
        assert!(report.interface_rows.iter().any(|row| {
            row.owner_modules
                .contains(&String::from("crates/psionic-transformer/src/blocks.rs"))
        }));
    }

    #[test]
    fn missing_interface_coverage_keeps_boundary_contract_red() {
        let acceptance_gate_report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("gate");
        let inventory_report =
            build_tassadar_existing_substrate_inventory_report().expect("inventory");
        let mut interfaces = interface_rows();
        interfaces.pop();

        let report = build_report_from_inputs(
            acceptance_gate_report,
            inventory_report,
            interfaces,
            ownership_rows(),
            dependency_checks().expect("checks"),
        );

        assert!(!report.boundary_contract_green);
        assert!(!report.all_interfaces_covered);
    }

    #[test]
    fn failing_dependency_check_keeps_boundary_contract_red() {
        let acceptance_gate_report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("gate");
        let inventory_report =
            build_tassadar_existing_substrate_inventory_report().expect("inventory");
        let mut checks = dependency_checks().expect("checks");
        checks[0].passed = false;

        let report = build_report_from_inputs(
            acceptance_gate_report,
            inventory_report,
            interface_rows(),
            ownership_rows(),
            checks,
        );

        assert!(!report.boundary_contract_green);
        assert!(!report.all_dependency_checks_pass);
    }

    #[test]
    fn canonical_transformer_stack_boundary_matches_committed_truth() {
        let generated =
            build_tassadar_canonical_transformer_stack_boundary_report().expect("boundary");
        let committed: TassadarCanonicalTransformerStackBoundaryReport =
            read_json(tassadar_canonical_transformer_stack_boundary_report_path())
                .expect("committed boundary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_canonical_transformer_stack_boundary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_canonical_transformer_stack_boundary_report.json");
        let written = write_tassadar_canonical_transformer_stack_boundary_report(&output_path)
            .expect("write boundary");
        let persisted: TassadarCanonicalTransformerStackBoundaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_canonical_transformer_stack_boundary_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_canonical_transformer_stack_boundary_report.json")
        );
    }
}
