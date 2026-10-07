//! Local registry and promotion gates for Qwen legal adapters.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const QWEN_LEGAL_ADAPTER_REGISTRY_SCHEMA_VERSION: u16 = 1;
pub const DEFAULT_QWEN_LEGAL_ADAPTER_REGISTRY_PATH: &str =
    "target/legal/qwen_adapter_registry/registry.json";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalRegistryDigest {
    pub algorithm: String,
    pub value: String,
}

impl QwenLegalRegistryDigest {
    pub fn sha256(value: impl Into<String>) -> Self {
        Self {
            algorithm: String::from("sha256"),
            value: value.into(),
        }
    }

    fn is_complete(&self) -> bool {
        self.algorithm == "sha256" && self.value.len() == 64 && is_hex(self.value.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalAdapterPromotionStatus {
    Candidate,
    Champion,
    Superseded,
    Held,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalAdapterEvalSummary {
    pub legal_score_bps: u32,
    pub answer_file_success_rate_bps: u32,
    pub required_workflow_success_rate_bps: u32,
    pub integrity_failure_count: u64,
    pub tool_failure_count: u64,
    pub timeout_failure_count: u64,
    pub harness_modified_answer_text: bool,
    pub hidden_benchmark_leakage: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalAdapterRegistryEntry {
    pub schema_version: u16,
    pub adapter_id: String,
    pub base_model_id: String,
    pub base_model_hash: QwenLegalRegistryDigest,
    pub adapter_artifact_hash: QwenLegalRegistryDigest,
    pub training_dataset_id: String,
    pub training_dataset_hash: QwenLegalRegistryDigest,
    pub training_config_id: String,
    pub training_config_hash: QwenLegalRegistryDigest,
    pub psionic_version: String,
    pub git_commit: String,
    pub training_worker_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_receipt_hash: Option<QwenLegalRegistryDigest>,
    pub eval_suite_id: String,
    pub eval_suite_hash: QwenLegalRegistryDigest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_result_hash: Option<QwenLegalRegistryDigest>,
    pub promotion_status: QwenLegalAdapterPromotionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_adapter_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_adapter_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub score_report_hashes: BTreeMap<String, QwenLegalRegistryDigest>,
    pub training_data_allowed: bool,
    pub excluded_training_data: bool,
    pub produced_by_allowed_psionic_path: bool,
    pub eval_summary: QwenLegalAdapterEvalSummary,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalAdapterRegistry {
    pub schema_version: u16,
    pub registry_id: String,
    pub entries: BTreeMap<String, QwenLegalAdapterRegistryEntry>,
    pub champion_adapter_by_suite: BTreeMap<String, String>,
    pub promotion_receipts: Vec<QwenLegalAdapterPromotionReceipt>,
}

impl Default for QwenLegalAdapterRegistry {
    fn default() -> Self {
        Self {
            schema_version: QWEN_LEGAL_ADAPTER_REGISTRY_SCHEMA_VERSION,
            registry_id: String::from("qwen_legal_adapter_registry.local"),
            entries: BTreeMap::new(),
            champion_adapter_by_suite: BTreeMap::new(),
            promotion_receipts: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalPromotionDecision {
    Promote,
    Hold,
    Reject,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalAdapterPromotionReceipt {
    pub schema_version: u16,
    pub receipt_id: String,
    pub suite_id: String,
    pub candidate_adapter_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_champion_adapter_id: Option<String>,
    pub decision: QwenLegalPromotionDecision,
    pub candidate_score_bps: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub champion_score_bps: Option<u32>,
    pub score_delta_bps: i32,
    pub reasons: Vec<String>,
    pub receipt_hash: QwenLegalRegistryDigest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalAdapterRegistrationReceipt {
    pub schema_version: u16,
    pub adapter_id: String,
    pub registry_path: String,
    pub entry_hash: QwenLegalRegistryDigest,
    pub accepted: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Error)]
pub enum QwenLegalAdapterRegistryError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("registry rejected adapter: {0}")]
    Rejected(String),
    #[error("adapter `{0}` not found")]
    AdapterNotFound(String),
    #[error("suite `{0}` has no champion")]
    ChampionNotFound(String),
}

pub fn default_qwen_legal_adapter_registry_path() -> PathBuf {
    env::var("PSIONIC_LEGAL_ADAPTER_REGISTRY")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_QWEN_LEGAL_ADAPTER_REGISTRY_PATH))
}

pub fn register_qwen_legal_adapter_manifest(
    manifest_path: impl AsRef<Path>,
    registry_path: impl AsRef<Path>,
) -> Result<QwenLegalAdapterRegistrationReceipt, QwenLegalAdapterRegistryError> {
    let manifest_path = manifest_path.as_ref();
    let registry_path = registry_path.as_ref();
    let entry = read_json::<QwenLegalAdapterRegistryEntry>(manifest_path)?;
    let mut registry = load_qwen_legal_adapter_registry(registry_path)?;
    register_qwen_legal_adapter_entry(&mut registry, entry, registry_path)
}

pub fn register_qwen_legal_adapter_entry(
    registry: &mut QwenLegalAdapterRegistry,
    entry: QwenLegalAdapterRegistryEntry,
    registry_path: impl AsRef<Path>,
) -> Result<QwenLegalAdapterRegistrationReceipt, QwenLegalAdapterRegistryError> {
    let registry_path = registry_path.as_ref();
    let reasons = registration_rejection_reasons(&entry);
    if !reasons.is_empty() {
        return Err(QwenLegalAdapterRegistryError::Rejected(reasons.join("; ")));
    }
    let entry_hash = registry_entry_hash(&entry)?;
    if entry.promotion_status == QwenLegalAdapterPromotionStatus::Champion {
        registry
            .champion_adapter_by_suite
            .insert(entry.eval_suite_id.clone(), entry.adapter_id.clone());
    }
    let adapter_id = entry.adapter_id.clone();
    registry.entries.insert(adapter_id.clone(), entry);
    save_qwen_legal_adapter_registry(registry_path, registry)?;
    Ok(QwenLegalAdapterRegistrationReceipt {
        schema_version: QWEN_LEGAL_ADAPTER_REGISTRY_SCHEMA_VERSION,
        adapter_id,
        registry_path: registry_path.to_string_lossy().to_string(),
        entry_hash: QwenLegalRegistryDigest::sha256(entry_hash),
        accepted: true,
        reasons: Vec::new(),
    })
}

pub fn promote_qwen_legal_adapter(
    registry_path: impl AsRef<Path>,
    candidate_adapter_id: &str,
    suite_id: &str,
) -> Result<QwenLegalAdapterPromotionReceipt, QwenLegalAdapterRegistryError> {
    let registry_path = registry_path.as_ref();
    let mut registry = load_qwen_legal_adapter_registry(registry_path)?;
    let candidate = registry
        .entries
        .get(candidate_adapter_id)
        .cloned()
        .ok_or_else(|| {
            QwenLegalAdapterRegistryError::AdapterNotFound(candidate_adapter_id.to_owned())
        })?;
    let previous_champion_adapter_id = registry.champion_adapter_by_suite.get(suite_id).cloned();
    if previous_champion_adapter_id.as_deref() == Some(candidate_adapter_id) {
        let receipt = already_champion_receipt(&candidate, suite_id)?;
        registry.promotion_receipts.push(receipt.clone());
        save_qwen_legal_adapter_registry(registry_path, &registry)?;
        write_promotion_receipt(registry_path, &receipt)?;
        return Ok(receipt);
    }
    let previous_champion = previous_champion_adapter_id
        .as_ref()
        .and_then(|adapter_id| registry.entries.get(adapter_id))
        .cloned();
    let receipt = promotion_receipt(&candidate, previous_champion.as_ref(), suite_id)?;
    match receipt.decision {
        QwenLegalPromotionDecision::Promote => {
            if let Some(previous_id) = &previous_champion_adapter_id {
                if previous_id != candidate_adapter_id {
                    if let Some(previous) = registry.entries.get_mut(previous_id) {
                        previous.promotion_status = QwenLegalAdapterPromotionStatus::Superseded;
                    }
                }
            }
            if let Some(candidate_entry) = registry.entries.get_mut(candidate_adapter_id) {
                candidate_entry.promotion_status = QwenLegalAdapterPromotionStatus::Champion;
            }
            registry
                .champion_adapter_by_suite
                .insert(suite_id.to_owned(), candidate_adapter_id.to_owned());
        }
        QwenLegalPromotionDecision::Hold => {
            if let Some(candidate_entry) = registry.entries.get_mut(candidate_adapter_id) {
                candidate_entry.promotion_status = QwenLegalAdapterPromotionStatus::Held;
            }
        }
        QwenLegalPromotionDecision::Reject => {
            if let Some(candidate_entry) = registry.entries.get_mut(candidate_adapter_id) {
                candidate_entry.promotion_status = QwenLegalAdapterPromotionStatus::Rejected;
            }
        }
    }
    registry.promotion_receipts.push(receipt.clone());
    save_qwen_legal_adapter_registry(registry_path, &registry)?;
    write_promotion_receipt(registry_path, &receipt)?;
    Ok(receipt)
}

pub fn load_qwen_legal_adapter_registry(
    registry_path: impl AsRef<Path>,
) -> Result<QwenLegalAdapterRegistry, QwenLegalAdapterRegistryError> {
    let registry_path = registry_path.as_ref();
    if !registry_path.exists() {
        return Ok(QwenLegalAdapterRegistry::default());
    }
    read_json(registry_path)
}

pub fn save_qwen_legal_adapter_registry(
    registry_path: impl AsRef<Path>,
    registry: &QwenLegalAdapterRegistry,
) -> Result<(), QwenLegalAdapterRegistryError> {
    let registry_path = registry_path.as_ref();
    if let Some(parent) = registry_path.parent() {
        fs::create_dir_all(parent).map_err(|source| QwenLegalAdapterRegistryError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(registry_path, serde_json::to_vec_pretty(registry)?).map_err(|source| {
        QwenLegalAdapterRegistryError::Io {
            path: registry_path.to_path_buf(),
            source,
        }
    })
}

pub fn registry_entry_hash(
    entry: &QwenLegalAdapterRegistryEntry,
) -> Result<String, QwenLegalAdapterRegistryError> {
    stable_json_digest("psionic.qwen_legal_adapter_registry.entry.v1", entry)
}

fn promotion_receipt(
    candidate: &QwenLegalAdapterRegistryEntry,
    champion: Option<&QwenLegalAdapterRegistryEntry>,
    suite_id: &str,
) -> Result<QwenLegalAdapterPromotionReceipt, QwenLegalAdapterRegistryError> {
    let mut reasons = registration_rejection_reasons(candidate);
    if candidate.eval_suite_id != suite_id {
        reasons.push(format!(
            "candidate eval suite `{}` does not match requested suite `{suite_id}`",
            candidate.eval_suite_id
        ));
    }
    let champion_score = champion.map(|entry| entry.eval_summary.legal_score_bps);
    let mut score_delta_bps = 0_i32;
    if let Some(champion) = champion {
        if champion.eval_suite_id != candidate.eval_suite_id {
            reasons.push(String::from(
                "candidate and champion were not evaluated on the same suite",
            ));
        }
        if champion.eval_suite_hash != candidate.eval_suite_hash {
            reasons.push(String::from(
                "candidate and champion eval suite hashes differ",
            ));
        }
        score_delta_bps = i32::try_from(candidate.eval_summary.legal_score_bps).unwrap_or(i32::MAX)
            - i32::try_from(champion.eval_summary.legal_score_bps).unwrap_or(i32::MAX);
        if candidate.eval_summary.legal_score_bps <= champion.eval_summary.legal_score_bps {
            reasons.push(String::from(
                "candidate score does not beat current champion",
            ));
        }
        if candidate.eval_summary.answer_file_success_rate_bps
            < champion.eval_summary.answer_file_success_rate_bps
        {
            reasons.push(String::from("candidate regresses answer-file write rate"));
        }
        if candidate.eval_summary.required_workflow_success_rate_bps
            < champion.eval_summary.required_workflow_success_rate_bps
        {
            reasons.push(String::from(
                "candidate regresses required workflow behavior",
            ));
        }
    } else {
        reasons.push(String::from("no current champion exists for this suite"));
    }
    let decision = if reasons.iter().any(|reason| {
        reason.contains("missing")
            || reason.contains("excluded")
            || reason.contains("hidden benchmark leakage")
            || reason.contains("harness modified")
            || reason.contains("not produced")
            || reason.contains("does not beat")
            || reason.contains("regresses")
            || reason.contains("differs")
            || reason.contains("does not match")
            || reason.contains("no current champion")
    }) {
        QwenLegalPromotionDecision::Reject
    } else {
        QwenLegalPromotionDecision::Promote
    };
    let mut receipt = QwenLegalAdapterPromotionReceipt {
        schema_version: QWEN_LEGAL_ADAPTER_REGISTRY_SCHEMA_VERSION,
        receipt_id: format!("qwen.legal.promotion.{suite_id}.{}", candidate.adapter_id),
        suite_id: suite_id.to_owned(),
        candidate_adapter_id: candidate.adapter_id.clone(),
        previous_champion_adapter_id: champion.map(|entry| entry.adapter_id.clone()),
        decision,
        candidate_score_bps: candidate.eval_summary.legal_score_bps,
        champion_score_bps: champion_score,
        score_delta_bps,
        reasons: if reasons.is_empty() {
            vec![String::from(
                "candidate beats champion and passes all hard gates",
            )]
        } else {
            reasons
        },
        receipt_hash: QwenLegalRegistryDigest::sha256(
            "0000000000000000000000000000000000000000000000000000000000000000",
        ),
    };
    let receipt_hash = stable_json_digest(
        "psionic.qwen_legal_adapter_registry.promotion_receipt.v1",
        &receipt,
    )?;
    receipt.receipt_hash = QwenLegalRegistryDigest::sha256(receipt_hash);
    Ok(receipt)
}

fn already_champion_receipt(
    candidate: &QwenLegalAdapterRegistryEntry,
    suite_id: &str,
) -> Result<QwenLegalAdapterPromotionReceipt, QwenLegalAdapterRegistryError> {
    let mut receipt = QwenLegalAdapterPromotionReceipt {
        schema_version: QWEN_LEGAL_ADAPTER_REGISTRY_SCHEMA_VERSION,
        receipt_id: format!("qwen.legal.promotion.{suite_id}.{}", candidate.adapter_id),
        suite_id: suite_id.to_owned(),
        candidate_adapter_id: candidate.adapter_id.clone(),
        previous_champion_adapter_id: Some(candidate.adapter_id.clone()),
        decision: QwenLegalPromotionDecision::Hold,
        candidate_score_bps: candidate.eval_summary.legal_score_bps,
        champion_score_bps: Some(candidate.eval_summary.legal_score_bps),
        score_delta_bps: 0,
        reasons: vec![String::from("candidate is already champion for this suite")],
        receipt_hash: QwenLegalRegistryDigest::sha256(
            "0000000000000000000000000000000000000000000000000000000000000000",
        ),
    };
    let receipt_hash = stable_json_digest(
        "psionic.qwen_legal_adapter_registry.promotion_receipt.v1",
        &receipt,
    )?;
    receipt.receipt_hash = QwenLegalRegistryDigest::sha256(receipt_hash);
    Ok(receipt)
}

fn registration_rejection_reasons(entry: &QwenLegalAdapterRegistryEntry) -> Vec<String> {
    let mut reasons = Vec::new();
    if entry.schema_version != QWEN_LEGAL_ADAPTER_REGISTRY_SCHEMA_VERSION {
        reasons.push(format!(
            "schema version must be {QWEN_LEGAL_ADAPTER_REGISTRY_SCHEMA_VERSION}"
        ));
    }
    for (field, value) in [
        ("adapter_id", entry.adapter_id.as_str()),
        ("base_model_id", entry.base_model_id.as_str()),
        ("training_dataset_id", entry.training_dataset_id.as_str()),
        ("training_config_id", entry.training_config_id.as_str()),
        ("psionic_version", entry.psionic_version.as_str()),
        ("git_commit", entry.git_commit.as_str()),
        ("eval_suite_id", entry.eval_suite_id.as_str()),
    ] {
        if value.trim().is_empty() {
            reasons.push(format!("missing {field}"));
        }
    }
    for (field, digest) in [
        ("base_model_hash", &entry.base_model_hash),
        ("adapter_artifact_hash", &entry.adapter_artifact_hash),
        ("training_dataset_hash", &entry.training_dataset_hash),
        ("training_config_hash", &entry.training_config_hash),
        ("eval_suite_hash", &entry.eval_suite_hash),
    ] {
        if !digest.is_complete() {
            reasons.push(format!("missing or invalid {field}"));
        }
    }
    if entry.score_report_hashes.is_empty() {
        reasons.push(String::from("missing score report hashes"));
    }
    for (score_report_id, digest) in &entry.score_report_hashes {
        if score_report_id.trim().is_empty() || !digest.is_complete() {
            reasons.push(String::from("missing or invalid score report hash"));
        }
    }
    match &entry.training_receipt_hash {
        Some(hash) if hash.is_complete() => {}
        _ => reasons.push(String::from("missing training receipt")),
    }
    match &entry.eval_result_hash {
        Some(hash) if hash.is_complete() => {}
        _ => reasons.push(String::from("missing eval receipt")),
    }
    if entry.training_worker_ids.is_empty() {
        reasons.push(String::from("missing training worker ids"));
    }
    if !entry.training_data_allowed || entry.excluded_training_data {
        reasons.push(String::from("adapter trained on excluded data"));
    }
    if !entry.produced_by_allowed_psionic_path {
        reasons.push(String::from(
            "adapter was not produced by an allowed Psionic/Pylon path",
        ));
    }
    if entry.eval_summary.integrity_failure_count > 0 {
        reasons.push(String::from(
            "answer integrity is not valid on all counted tasks",
        ));
    }
    if entry.eval_summary.harness_modified_answer_text {
        reasons.push(String::from("harness modified answer text"));
    }
    if entry.eval_summary.hidden_benchmark_leakage {
        reasons.push(String::from("hidden benchmark leakage"));
    }
    if entry.eval_summary.legal_score_bps > 10_000
        || entry.eval_summary.answer_file_success_rate_bps > 10_000
        || entry.eval_summary.required_workflow_success_rate_bps > 10_000
    {
        reasons.push(String::from("basis-point fields must be <= 10000"));
    }
    reasons
}

fn write_promotion_receipt(
    registry_path: &Path,
    receipt: &QwenLegalAdapterPromotionReceipt,
) -> Result<(), QwenLegalAdapterRegistryError> {
    let parent = registry_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let receipt_path = parent.join(format!(
        "promotion_{}_{}.json",
        sanitize_path(receipt.suite_id.as_str()),
        sanitize_path(receipt.candidate_adapter_id.as_str())
    ));
    fs::write(&receipt_path, serde_json::to_vec_pretty(receipt)?).map_err(|source| {
        QwenLegalAdapterRegistryError::Io {
            path: receipt_path,
            source,
        }
    })
}

fn read_json<T>(path: &Path) -> Result<T, QwenLegalAdapterRegistryError>
where
    T: for<'de> Deserialize<'de>,
{
    let bytes = fs::read(path).map_err(|source| QwenLegalAdapterRegistryError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn stable_json_digest<T>(
    namespace: &str,
    value: &T,
) -> Result<String, QwenLegalAdapterRegistryError>
where
    T: Serialize,
{
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    hasher.update(b"|");
    hasher.update(serde_json::to_vec(value)?);
    Ok(hex::encode(hasher.finalize()))
}

fn sanitize_path(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => character,
            _ => '_',
        })
        .collect()
}

fn is_hex(value: &str) -> bool {
    value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: &str) -> QwenLegalRegistryDigest {
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        QwenLegalRegistryDigest::sha256(hex::encode(hasher.finalize()))
    }

    fn entry(
        adapter_id: &str,
        score: u32,
        status: QwenLegalAdapterPromotionStatus,
    ) -> QwenLegalAdapterRegistryEntry {
        QwenLegalAdapterRegistryEntry {
            schema_version: QWEN_LEGAL_ADAPTER_REGISTRY_SCHEMA_VERSION,
            adapter_id: String::from(adapter_id),
            base_model_id: String::from("Qwen/Qwen3.6-27B"),
            base_model_hash: digest("base"),
            adapter_artifact_hash: digest("adapter"),
            training_dataset_id: String::from("legal-sft-v1"),
            training_dataset_hash: digest("dataset"),
            training_config_id: String::from("qwen36-legal-sft-smoke"),
            training_config_hash: digest("config"),
            psionic_version: String::from(env!("CARGO_PKG_VERSION")),
            git_commit: String::from("abcdef123"),
            training_worker_ids: vec![String::from("pylon.local.1")],
            training_receipt_hash: Some(digest("training")),
            eval_suite_id: String::from("harvey_public_three_deterministic_replay_v1"),
            eval_suite_hash: digest("suite"),
            eval_result_hash: Some(digest("eval")),
            promotion_status: status,
            parent_adapter_id: None,
            rollback_adapter_id: Some(String::from("adapter.rollback")),
            score_report_hashes: BTreeMap::from([(
                String::from("harvey_public_three_deterministic_replay_v1"),
                digest("score-report"),
            )]),
            training_data_allowed: true,
            excluded_training_data: false,
            produced_by_allowed_psionic_path: true,
            eval_summary: QwenLegalAdapterEvalSummary {
                legal_score_bps: score,
                answer_file_success_rate_bps: 10_000,
                required_workflow_success_rate_bps: 10_000,
                integrity_failure_count: 0,
                tool_failure_count: 0,
                timeout_failure_count: 0,
                harness_modified_answer_text: false,
                hidden_benchmark_leakage: false,
            },
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn registry_rejects_missing_training_receipt() {
        let mut candidate = entry(
            "adapter.missing.training",
            9_000,
            QwenLegalAdapterPromotionStatus::Candidate,
        );
        candidate.training_receipt_hash = None;
        let mut registry = QwenLegalAdapterRegistry::default();
        let temp = tempfile::tempdir().expect("tempdir");
        let error = register_qwen_legal_adapter_entry(
            &mut registry,
            candidate,
            temp.path().join("registry.json"),
        )
        .expect_err("missing training receipt should fail");
        assert!(error.to_string().contains("missing training receipt"));
    }

    #[test]
    fn registry_rejects_missing_eval_receipt() {
        let mut candidate = entry(
            "adapter.missing.eval",
            9_000,
            QwenLegalAdapterPromotionStatus::Candidate,
        );
        candidate.eval_result_hash = None;
        let mut registry = QwenLegalAdapterRegistry::default();
        let temp = tempfile::tempdir().expect("tempdir");
        let error = register_qwen_legal_adapter_entry(
            &mut registry,
            candidate,
            temp.path().join("registry.json"),
        )
        .expect_err("missing eval receipt should fail");
        assert!(error.to_string().contains("missing eval receipt"));
    }

    #[test]
    fn registry_rejects_missing_adapter_identity_fields() {
        let mut candidate = entry(
            "adapter.missing.identity",
            9_000,
            QwenLegalAdapterPromotionStatus::Candidate,
        );
        candidate.adapter_artifact_hash = QwenLegalRegistryDigest::sha256(String::new());
        candidate.score_report_hashes.clear();
        let mut registry = QwenLegalAdapterRegistry::default();
        let temp = tempfile::tempdir().expect("tempdir");
        let error = register_qwen_legal_adapter_entry(
            &mut registry,
            candidate,
            temp.path().join("registry.json"),
        )
        .expect_err("missing adapter identity should fail");
        let message = error.to_string();
        assert!(message.contains("adapter_artifact_hash"));
        assert!(message.contains("missing score report hashes"));
    }

    #[test]
    fn registry_rejects_excluded_training_data() {
        let mut candidate = entry(
            "adapter.excluded",
            9_000,
            QwenLegalAdapterPromotionStatus::Candidate,
        );
        candidate.excluded_training_data = true;
        let mut registry = QwenLegalAdapterRegistry::default();
        let temp = tempfile::tempdir().expect("tempdir");
        let error = register_qwen_legal_adapter_entry(
            &mut registry,
            candidate,
            temp.path().join("registry.json"),
        )
        .expect_err("excluded training data should fail");
        assert!(error.to_string().contains("excluded data"));
    }

    #[test]
    fn promotion_rejects_lower_score() {
        let temp = tempfile::tempdir().expect("tempdir");
        let registry_path = temp.path().join("registry.json");
        let mut registry = QwenLegalAdapterRegistry::default();
        register_qwen_legal_adapter_entry(
            &mut registry,
            entry(
                "adapter.champion",
                8_000,
                QwenLegalAdapterPromotionStatus::Champion,
            ),
            &registry_path,
        )
        .expect("champion");
        register_qwen_legal_adapter_entry(
            &mut registry,
            entry(
                "adapter.lower",
                7_000,
                QwenLegalAdapterPromotionStatus::Candidate,
            ),
            &registry_path,
        )
        .expect("candidate");
        let receipt = promote_qwen_legal_adapter(
            &registry_path,
            "adapter.lower",
            "harvey_public_three_deterministic_replay_v1",
        )
        .expect("promotion receipt");
        assert_eq!(receipt.decision, QwenLegalPromotionDecision::Reject);
        assert!(receipt
            .reasons
            .iter()
            .any(|reason| reason.contains("does not beat")));
    }

    #[test]
    fn promotion_accepts_valid_improving_candidate() {
        let temp = tempfile::tempdir().expect("tempdir");
        let registry_path = temp.path().join("registry.json");
        let mut registry = QwenLegalAdapterRegistry::default();
        register_qwen_legal_adapter_entry(
            &mut registry,
            entry(
                "adapter.champion",
                8_000,
                QwenLegalAdapterPromotionStatus::Champion,
            ),
            &registry_path,
        )
        .expect("champion");
        register_qwen_legal_adapter_entry(
            &mut registry,
            entry(
                "adapter.better",
                9_000,
                QwenLegalAdapterPromotionStatus::Candidate,
            ),
            &registry_path,
        )
        .expect("candidate");
        let receipt = promote_qwen_legal_adapter(
            &registry_path,
            "adapter.better",
            "harvey_public_three_deterministic_replay_v1",
        )
        .expect("promotion receipt");
        assert_eq!(receipt.decision, QwenLegalPromotionDecision::Promote);
        let registry = load_qwen_legal_adapter_registry(&registry_path).expect("registry");
        assert_eq!(
            registry
                .champion_adapter_by_suite
                .get("harvey_public_three_deterministic_replay_v1")
                .map(String::as_str),
            Some("adapter.better")
        );
    }
}
