use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_catalog::{
    build_tassadar_post_article_starter_plugin_catalog_report,
    TassadarPostArticleStarterPluginCatalogReportError,
    TassadarPostArticleStarterPluginCatalogStatus,
};

pub const TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_EVAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_starter_plugin_catalog_eval_report.json";
pub const TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-starter-plugin-catalog.sh";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleStarterPluginCatalogEvalStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleStarterPluginCatalogEvalDependencyClass {
    CatalogPrecedent,
    SupportingPrecedent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCatalogEvalMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub closure_bundle_digest: String,
    pub catalog_report_id: String,
    pub catalog_report_digest: String,
    pub runtime_bundle_id: String,
    pub runtime_bundle_digest: String,
    pub runtime_bundle_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCatalogEvalDependencyRow {
    pub dependency_id: String,
    pub dependency_class: TassadarPostArticleStarterPluginCatalogEvalDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCatalogEvalValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCatalogEvalReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub catalog_report_ref: String,
    pub machine_identity_binding: TassadarPostArticleStarterPluginCatalogEvalMachineIdentityBinding,
    pub dependency_rows: Vec<TassadarPostArticleStarterPluginCatalogEvalDependencyRow>,
    pub validation_rows: Vec<TassadarPostArticleStarterPluginCatalogEvalValidationRow>,
    pub catalog_status: TassadarPostArticleStarterPluginCatalogStatus,
    pub eval_status: TassadarPostArticleStarterPluginCatalogEvalStatus,
    pub eval_green: bool,
    pub starter_plugin_count: u32,
    pub local_deterministic_plugin_count: u32,
    pub read_only_network_plugin_count: u32,
    pub bounded_flow_count: u32,
    pub operator_internal_only_posture: bool,
    pub local_network_distinction_explicit: bool,
    pub composition_harness_green: bool,
    pub runtime_builtins_separate: bool,
    pub public_marketplace_language_suppressed: bool,
    pub closure_bundle_bound_by_digest: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub next_issue_id: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleStarterPluginCatalogEvalReportError {
    #[error(transparent)]
    Catalog(#[from] TassadarPostArticleStarterPluginCatalogReportError),
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

pub fn build_tassadar_post_article_starter_plugin_catalog_eval_report() -> Result<
    TassadarPostArticleStarterPluginCatalogEvalReport,
    TassadarPostArticleStarterPluginCatalogEvalReportError,
> {
    let catalog = build_tassadar_post_article_starter_plugin_catalog_report()?;
    let starter_plugin_count = catalog.entry_rows.len() as u32;
    let local_deterministic_plugin_count = catalog
        .entry_rows
        .iter()
        .filter(|row| row.local_deterministic)
        .count() as u32;
    let read_only_network_plugin_count = catalog
        .entry_rows
        .iter()
        .filter(|row| row.read_only_network)
        .count() as u32;
    let bounded_flow_count = catalog.composition_rows.len() as u32;

    let machine_identity_binding = TassadarPostArticleStarterPluginCatalogEvalMachineIdentityBinding {
        machine_identity_id: catalog
            .machine_identity_binding
            .machine_identity_id
            .clone(),
        canonical_model_id: catalog.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: catalog.machine_identity_binding.canonical_route_id.clone(),
        canonical_route_descriptor_digest: catalog
            .machine_identity_binding
            .canonical_route_descriptor_digest
            .clone(),
        canonical_weight_bundle_digest: catalog
            .machine_identity_binding
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: catalog
            .machine_identity_binding
            .canonical_weight_primary_artifact_sha256
            .clone(),
        continuation_contract_id: catalog
            .machine_identity_binding
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: catalog
            .machine_identity_binding
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: catalog
            .machine_identity_binding
            .computational_model_statement_id
            .clone(),
        closure_bundle_digest: catalog
            .machine_identity_binding
            .closure_bundle_digest
            .clone(),
        catalog_report_id: catalog.report_id.clone(),
        catalog_report_digest: catalog.report_digest.clone(),
        runtime_bundle_id: catalog.machine_identity_binding.runtime_bundle_id.clone(),
        runtime_bundle_digest: catalog
            .machine_identity_binding
            .runtime_bundle_digest
            .clone(),
        runtime_bundle_ref: catalog.machine_identity_binding.runtime_bundle_ref.clone(),
        detail: format!(
            "starter catalog eval keeps catalog_report_id=`{}`, runtime_bundle_id=`{}`, machine_identity_id=`{}`, and closure_bundle_digest=`{}` aligned.",
            catalog.report_id,
            catalog.machine_identity_binding.runtime_bundle_id,
            catalog.machine_identity_binding.machine_identity_id,
            catalog.machine_identity_binding.closure_bundle_digest,
        ),
    };

    let dependency_rows = vec![
        dependency_row(
            "starter_catalog_report_green",
            TassadarPostArticleStarterPluginCatalogEvalDependencyClass::CatalogPrecedent,
            catalog.contract_green,
            &catalog.report_id,
            Some(catalog.report_id.clone()),
            Some(catalog.report_digest.clone()),
            "the catalog-owned starter plugin report is green and keeps the catalog digest-bound and operator-only.",
        ),
        dependency_row(
            "starter_catalog_closure_bundle_bound",
            TassadarPostArticleStarterPluginCatalogEvalDependencyClass::SupportingPrecedent,
            catalog.closure_bundle_bound_by_digest,
            &catalog.closure_bundle_report_ref,
            Some(catalog.machine_identity_binding.closure_bundle_report_id.clone()),
            Some(catalog.machine_identity_binding.closure_bundle_report_digest.clone()),
            "the eval posture inherits the same closure-bundle digest binding as the catalog report.",
        ),
    ];

    let validation_rows = vec![
        validation_row(
            "starter_plugin_count_exact",
            starter_plugin_count == 6,
            &[&catalog.runtime_bundle_ref],
            "the eval report keeps the starter catalog fixed to six operator-curated entries, including the digest-bound guest-artifact row plus the earlier capability-free and manual networked_read_only plugins.",
        ),
        validation_row(
            "local_network_distinction_explicit",
            catalog.local_network_distinction_explicit
                && local_deterministic_plugin_count == 5
                && read_only_network_plugin_count == 1,
            &[&catalog.runtime_bundle_ref, &catalog.report_id],
            "five local deterministic plugins, including one digest-bound guest-artifact row, remain distinct from one read-only network plugin.",
        ),
        validation_row(
            "composition_harness_green",
            catalog.composition_harness_green && bounded_flow_count == 2,
            &[&catalog.runtime_bundle_ref, &catalog.report_id],
            "the starter catalog still carries the two bounded composition flows required by the issue.",
        ),
        validation_row(
            "operator_only_marketplace_suppressed",
            catalog.operator_internal_only_posture && catalog.public_marketplace_language_suppressed,
            &[&catalog.report_id],
            "operator-only posture and marketplace suppression remain explicit in the eval projection.",
        ),
    ];

    let eval_green = dependency_rows.iter().all(|row| row.satisfied)
        && validation_rows.iter().all(|row| row.green)
        && catalog.runtime_builtins_separate;

    let mut report = TassadarPostArticleStarterPluginCatalogEvalReport {
        schema_version: 1,
        report_id: String::from("tassadar.post_article.starter_plugin_catalog.eval_report.v1"),
        checker_script_ref: String::from(TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_CHECKER_REF),
        catalog_report_ref: String::from(
            "fixtures/tassadar/reports/tassadar_post_article_starter_plugin_catalog_report.json",
        ),
        machine_identity_binding,
        dependency_rows,
        validation_rows,
        catalog_status: catalog.contract_status,
        eval_status: if eval_green {
            TassadarPostArticleStarterPluginCatalogEvalStatus::Green
        } else {
            TassadarPostArticleStarterPluginCatalogEvalStatus::Incomplete
        },
        eval_green,
        starter_plugin_count,
        local_deterministic_plugin_count,
        read_only_network_plugin_count,
        bounded_flow_count,
        operator_internal_only_posture: catalog.operator_internal_only_posture,
        local_network_distinction_explicit: catalog.local_network_distinction_explicit,
        composition_harness_green: catalog.composition_harness_green,
        runtime_builtins_separate: catalog.runtime_builtins_separate,
        public_marketplace_language_suppressed: catalog.public_marketplace_language_suppressed,
        closure_bundle_bound_by_digest: catalog.closure_bundle_bound_by_digest,
        rebase_claim_allowed: catalog.rebase_claim_allowed,
        plugin_capability_claim_allowed: catalog.plugin_capability_claim_allowed,
        weighted_plugin_control_allowed: catalog.weighted_plugin_control_allowed,
        plugin_publication_allowed: catalog.plugin_publication_allowed,
        served_public_universality_allowed: catalog.served_public_universality_allowed,
        arbitrary_software_capability_allowed: catalog.arbitrary_software_capability_allowed,
        next_issue_id: catalog.next_issue_id.clone(),
        claim_boundary: String::from(
            "this eval report keeps the starter catalog within the same operator-only, digest-bound, no-public-marketplace boundary as the catalog report and does not widen to served or public plugin rights.",
        ),
        summary: format!(
            "starter catalog eval binds {} starter plugins and {} bounded flows to the catalog report with operator-only posture, marketplace suppression, and closure-bundle binding still explicit.",
            starter_plugin_count, bounded_flow_count,
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_starter_plugin_catalog_eval_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_post_article_starter_plugin_catalog_eval_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_EVAL_REPORT_REF)
}

pub fn write_tassadar_post_article_starter_plugin_catalog_eval_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleStarterPluginCatalogEvalReport,
    TassadarPostArticleStarterPluginCatalogEvalReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleStarterPluginCatalogEvalReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_starter_plugin_catalog_eval_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleStarterPluginCatalogEvalReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn dependency_row(
    dependency_id: &str,
    dependency_class: TassadarPostArticleStarterPluginCatalogEvalDependencyClass,
    satisfied: bool,
    source_ref: &str,
    bound_report_id: Option<String>,
    bound_report_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleStarterPluginCatalogEvalDependencyRow {
    TassadarPostArticleStarterPluginCatalogEvalDependencyRow {
        dependency_id: String::from(dependency_id),
        dependency_class,
        satisfied,
        source_ref: String::from(source_ref),
        bound_report_id,
        bound_report_digest,
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticleStarterPluginCatalogEvalValidationRow {
    TassadarPostArticleStarterPluginCatalogEvalValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
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
) -> Result<T, TassadarPostArticleStarterPluginCatalogEvalReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleStarterPluginCatalogEvalReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleStarterPluginCatalogEvalReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_starter_plugin_catalog_eval_report, read_json,
        tassadar_post_article_starter_plugin_catalog_eval_report_path,
        write_tassadar_post_article_starter_plugin_catalog_eval_report,
        TassadarPostArticleStarterPluginCatalogEvalReport,
        TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_EVAL_REPORT_REF,
    };

    #[test]
    fn starter_plugin_catalog_eval_keeps_operator_only_counts_explicit() {
        let report =
            build_tassadar_post_article_starter_plugin_catalog_eval_report().expect("eval report");

        assert_eq!(
            report.report_id,
            "tassadar.post_article.starter_plugin_catalog.eval_report.v1"
        );
        assert!(report.eval_green);
        assert_eq!(report.starter_plugin_count, 6);
        assert_eq!(report.local_deterministic_plugin_count, 5);
        assert_eq!(report.read_only_network_plugin_count, 1);
        assert_eq!(report.bounded_flow_count, 2);
        assert!(report.operator_internal_only_posture);
        assert!(report.public_marketplace_language_suppressed);
        assert!(report.closure_bundle_bound_by_digest);
        assert_eq!(report.next_issue_id, "TAS-217");
    }

    #[test]
    fn starter_plugin_catalog_eval_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_starter_plugin_catalog_eval_report().expect("eval report");
        let committed: TassadarPostArticleStarterPluginCatalogEvalReport =
            read_json(tassadar_post_article_starter_plugin_catalog_eval_report_path())
                .expect("committed eval report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_starter_plugin_catalog_eval_persists_current_truth() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_post_article_starter_plugin_catalog_eval_report.json");
        let written = write_tassadar_post_article_starter_plugin_catalog_eval_report(&output_path)
            .expect("write");
        let roundtrip: TassadarPostArticleStarterPluginCatalogEvalReport =
            read_json(&output_path).expect("roundtrip");

        assert_eq!(written, roundtrip);
        assert_eq!(
            tassadar_post_article_starter_plugin_catalog_eval_report_path()
                .strip_prefix(super::repo_root())
                .expect("starter catalog eval report should live under repo root")
                .to_string_lossy(),
            TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_EVAL_REPORT_REF
        );
    }
}
