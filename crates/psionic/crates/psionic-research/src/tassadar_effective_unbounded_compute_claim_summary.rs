use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    build_tassadar_effective_unbounded_compute_claim_report,
    TassadarEffectiveUnboundedComputeClaimReport,
    TassadarEffectiveUnboundedComputeClaimReportError,
};

pub const TASSADAR_EFFECTIVE_UNBOUNDED_COMPUTE_CLAIM_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_effective_unbounded_compute_claim_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectiveUnboundedComputeClaimSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub eval_report: TassadarEffectiveUnboundedComputeClaimReport,
    pub allowed_statement: String,
    pub blocked_by: Vec<String>,
    pub explicit_non_implications: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarEffectiveUnboundedComputeClaimSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarEffectiveUnboundedComputeClaimReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_effective_unbounded_compute_claim_summary() -> Result<
    TassadarEffectiveUnboundedComputeClaimSummary,
    TassadarEffectiveUnboundedComputeClaimSummaryError,
> {
    let eval_report = build_tassadar_effective_unbounded_compute_claim_report()?;
    let allowed_statement = String::from(
        "Psionic/Tassadar has bounded execution slices plus resumable continuation and state objects under explicit envelopes, but the repo may not yet claim `effective unbounded computation` as a public broad-compute posture.",
    );
    let blocked_by = eval_report.missing_prerequisite_ids.clone();
    let explicit_non_implications = eval_report.out_of_scope_claims.clone();
    let mut summary = TassadarEffectiveUnboundedComputeClaimSummary {
        schema_version: 1,
        report_id: String::from("tassadar.effective_unbounded_compute_claim.summary.v1"),
        eval_report,
        allowed_statement,
        blocked_by,
        explicit_non_implications,
        claim_boundary: String::from(
            "this summary is a disclosure-safe claim audit. It states what the repo can say today about resumable bounded computation and what it still cannot say about effective unbounded computation or universality",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    summary.summary = format!(
        "Effective-unbounded claim summary keeps blocked_by={}, explicit_non_implications={}, claim_status={:?}.",
        summary.blocked_by.len(),
        summary.explicit_non_implications.len(),
        summary.eval_report.claim_status,
    );
    summary.report_digest = stable_digest(
        b"psionic_tassadar_effective_unbounded_compute_claim_summary|",
        &summary,
    );
    Ok(summary)
}

#[must_use]
pub fn tassadar_effective_unbounded_compute_claim_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_EFFECTIVE_UNBOUNDED_COMPUTE_CLAIM_SUMMARY_REF)
}

pub fn write_tassadar_effective_unbounded_compute_claim_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarEffectiveUnboundedComputeClaimSummary,
    TassadarEffectiveUnboundedComputeClaimSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarEffectiveUnboundedComputeClaimSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_effective_unbounded_compute_claim_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarEffectiveUnboundedComputeClaimSummaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(summary)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-research crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarEffectiveUnboundedComputeClaimSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarEffectiveUnboundedComputeClaimSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarEffectiveUnboundedComputeClaimSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_effective_unbounded_compute_claim_summary, read_repo_json,
        tassadar_effective_unbounded_compute_claim_summary_path,
        TassadarEffectiveUnboundedComputeClaimSummary,
        TASSADAR_EFFECTIVE_UNBOUNDED_COMPUTE_CLAIM_SUMMARY_REF,
    };

    #[test]
    fn effective_unbounded_claim_summary_keeps_blockers_and_non_implications_explicit() {
        let summary = build_tassadar_effective_unbounded_compute_claim_summary().expect("summary");

        assert!(summary
            .blocked_by
            .contains(&String::from("broad_publication_gate")));
        assert!(summary
            .explicit_non_implications
            .contains(&String::from("Turing-complete support")));
    }

    #[test]
    fn effective_unbounded_claim_summary_matches_committed_truth() {
        let generated =
            build_tassadar_effective_unbounded_compute_claim_summary().expect("summary");
        let committed: TassadarEffectiveUnboundedComputeClaimSummary =
            read_repo_json(TASSADAR_EFFECTIVE_UNBOUNDED_COMPUTE_CLAIM_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn effective_unbounded_claim_summary_path_is_stable() {
        assert_eq!(
            tassadar_effective_unbounded_compute_claim_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_effective_unbounded_compute_claim_summary.json")
        );
    }
}
