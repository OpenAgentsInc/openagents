use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum ShadowControlKhalaError {
    #[error("shadow control/khala file I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("shadow control/khala parse error: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShadowControlKhalaManifest {
    pub scenario_id: String,
    pub control_status_path: PathBuf,
    pub control_route_split_status_path: PathBuf,
    pub khala_poll_path: PathBuf,
    pub khala_metrics_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShadowControlKhalaGatePolicy {
    pub max_warning_count: u64,
    pub block_on_critical: bool,
}

impl Default for ShadowControlKhalaGatePolicy {
    fn default() -> Self {
        Self {
            max_warning_count: 0,
            block_on_critical: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShadowControlKhalaParityReport {
    pub schema: String,
    pub generated_at: String,
    pub scenario_id: String,
    pub comparisons: Vec<ShadowControlKhalaComparison>,
    pub diffs: Vec<ShadowControlKhalaDiff>,
    pub totals: ShadowControlKhalaDiffTotals,
    pub gate: ShadowControlKhalaGateResult,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShadowControlKhalaComparison {
    pub component: String,
    pub severity: ShadowControlKhalaSeverity,
    pub matched: bool,
    pub legacy_hash: String,
    pub rust_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShadowControlKhalaDiff {
    pub severity: ShadowControlKhalaSeverity,
    pub component: String,
    pub field: String,
    pub legacy: String,
    pub rust: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShadowControlKhalaSeverity {
    Critical,
    Warning,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShadowControlKhalaDiffTotals {
    pub critical: u64,
    pub warning: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShadowControlKhalaGateResult {
    pub decision: ShadowControlKhalaGateDecision,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShadowControlKhalaGateDecision {
    Allow,
    Block,
}

struct ComponentSpec {
    component: &'static str,
    severity: ShadowControlKhalaSeverity,
}

const COMPONENT_SPECS: [ComponentSpec; 4] = [
    ComponentSpec {
        component: "control_status",
        severity: ShadowControlKhalaSeverity::Critical,
    },
    ComponentSpec {
        component: "control_route_split_status",
        severity: ShadowControlKhalaSeverity::Warning,
    },
    ComponentSpec {
        component: "khala_poll",
        severity: ShadowControlKhalaSeverity::Critical,
    },
    ComponentSpec {
        component: "khala_metrics",
        severity: ShadowControlKhalaSeverity::Warning,
    },
];

pub fn load_control_khala_manifest(
    path: impl AsRef<Path>,
) -> Result<ShadowControlKhalaManifest, ShadowControlKhalaError> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn write_control_khala_report(
    path: impl AsRef<Path>,
    report: &ShadowControlKhalaParityReport,
) -> Result<(), ShadowControlKhalaError> {
    let bytes = serde_json::to_vec_pretty(report)?;
    fs::write(path, bytes)?;
    Ok(())
}

pub fn generate_control_khala_parity_report(
    legacy_manifest: &ShadowControlKhalaManifest,
    rust_manifest: &ShadowControlKhalaManifest,
    policy: &ShadowControlKhalaGatePolicy,
    legacy_manifest_path: impl AsRef<Path>,
    rust_manifest_path: impl AsRef<Path>,
) -> Result<ShadowControlKhalaParityReport, ShadowControlKhalaError> {
    let legacy_components = load_component_snapshots(legacy_manifest, legacy_manifest_path)?;
    let rust_components = load_component_snapshots(rust_manifest, rust_manifest_path)?;
    let mut comparisons = Vec::with_capacity(COMPONENT_SPECS.len());
    let mut diffs = Vec::new();

    for spec in COMPONENT_SPECS {
        let legacy_value = legacy_components
            .get(spec.component)
            .cloned()
            .unwrap_or(Value::Null);
        let rust_value = rust_components
            .get(spec.component)
            .cloned()
            .unwrap_or(Value::Null);
        let legacy_hash = value_hash(&legacy_value);
        let rust_hash = value_hash(&rust_value);
        let matched = legacy_value == rust_value;

        comparisons.push(ShadowControlKhalaComparison {
            component: spec.component.to_string(),
            severity: spec.severity.clone(),
            matched,
            legacy_hash: legacy_hash.clone(),
            rust_hash: rust_hash.clone(),
        });

        if !matched {
            diffs.push(ShadowControlKhalaDiff {
                severity: spec.severity.clone(),
                component: spec.component.to_string(),
                field: "$".to_string(),
                legacy: legacy_hash,
                rust: rust_hash,
                message: format!("{} snapshot diverged", spec.component),
            });
        }
    }

    let totals = summarize_diffs(&diffs);
    let gate = evaluate_gate(&totals, policy);

    Ok(ShadowControlKhalaParityReport {
        schema: "openagents.shadow.control_khala_parity.v1".to_string(),
        generated_at: Utc::now().to_rfc3339(),
        scenario_id: rust_manifest.scenario_id.clone(),
        comparisons,
        diffs,
        totals,
        gate,
    })
}

fn load_component_snapshots(
    manifest: &ShadowControlKhalaManifest,
    manifest_path: impl AsRef<Path>,
) -> Result<BTreeMap<&'static str, Value>, ShadowControlKhalaError> {
    let base = manifest_path
        .as_ref()
        .parent()
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let control_status = load_json(resolve_relative(&base, &manifest.control_status_path))?;
    let route_split_status = load_json(resolve_relative(
        &base,
        &manifest.control_route_split_status_path,
    ))?;
    let khala_poll = load_json(resolve_relative(&base, &manifest.khala_poll_path))?;
    let khala_metrics = load_json(resolve_relative(&base, &manifest.khala_metrics_path))?;

    let mut components = BTreeMap::new();
    components.insert(
        "control_status",
        normalize_value(strip_control_status_volatiles(control_status)),
    );
    components.insert(
        "control_route_split_status",
        normalize_value(route_split_status),
    );
    components.insert(
        "khala_poll",
        normalize_value(strip_khala_poll_volatiles(khala_poll)),
    );
    components.insert(
        "khala_metrics",
        normalize_value(extract_khala_metrics_contract(khala_metrics)),
    );
    Ok(components)
}

fn strip_control_status_volatiles(mut value: Value) -> Value {
    if let Some(data) = value.pointer_mut("/data")
        && let Some(data_obj) = data.as_object_mut()
    {
        data_obj.remove("memberships");
    }
    value
}

fn strip_khala_poll_volatiles(mut value: Value) -> Value {
    if let Some(messages) = value.pointer_mut("/messages").and_then(Value::as_array_mut) {
        for message in messages {
            if let Some(obj) = message.as_object_mut() {
                obj.remove("published_at");
            }
        }
    }
    value
}

fn extract_khala_metrics_contract(value: Value) -> Value {
    let driver = value
        .get("driver")
        .cloned()
        .unwrap_or_else(|| Value::String("unknown".to_string()));
    let topic_windows = value
        .get("topic_windows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut topic_contracts = topic_windows
        .iter()
        .map(|window| {
            serde_json::json!({
                "topic": window.get("topic").cloned().unwrap_or(Value::Null),
                "topic_class": window.get("topic_class").cloned().unwrap_or(Value::Null),
                "oldest_sequence": window.get("oldest_sequence").cloned().unwrap_or(Value::Null),
                "head_sequence": window.get("head_sequence").cloned().unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    topic_contracts.sort_by_key(|item| {
        item.get("topic")
            .and_then(Value::as_str)
            .map_or_else(String::new, ToString::to_string)
    });
    serde_json::json!({
        "driver": driver,
        "topic_windows": topic_contracts,
    })
}

fn load_json(path: PathBuf) -> Result<Value, ShadowControlKhalaError> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn resolve_relative(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn normalize_value(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut normalized = Map::new();
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if let Some(entry) = object.get(&key) {
                    normalized.insert(key, normalize_value(entry.clone()));
                }
            }
            Value::Object(normalized)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(normalize_value).collect()),
        other => other,
    }
}

fn value_hash(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn summarize_diffs(diffs: &[ShadowControlKhalaDiff]) -> ShadowControlKhalaDiffTotals {
    let mut critical = 0_u64;
    let mut warning = 0_u64;
    for diff in diffs {
        match diff.severity {
            ShadowControlKhalaSeverity::Critical => critical = critical.saturating_add(1),
            ShadowControlKhalaSeverity::Warning => warning = warning.saturating_add(1),
        }
    }
    ShadowControlKhalaDiffTotals { critical, warning }
}

fn evaluate_gate(
    totals: &ShadowControlKhalaDiffTotals,
    policy: &ShadowControlKhalaGatePolicy,
) -> ShadowControlKhalaGateResult {
    if policy.block_on_critical && totals.critical > 0 {
        return ShadowControlKhalaGateResult {
            decision: ShadowControlKhalaGateDecision::Block,
            reason: format!("{} critical parity diffs detected", totals.critical),
        };
    }
    if totals.warning > policy.max_warning_count {
        return ShadowControlKhalaGateResult {
            decision: ShadowControlKhalaGateDecision::Block,
            reason: format!(
                "{} warning diffs exceed policy max {}",
                totals.warning, policy.max_warning_count
            ),
        };
    }
    ShadowControlKhalaGateResult {
        decision: ShadowControlKhalaGateDecision::Allow,
        reason: "parity report is within configured thresholds".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use tempfile::tempdir;

    use super::{
        ShadowControlKhalaGateDecision, ShadowControlKhalaGatePolicy, ShadowControlKhalaManifest,
        generate_control_khala_parity_report,
    };

    fn write_json(path: &std::path::Path, value: serde_json::Value) -> Result<()> {
        std::fs::write(path, serde_json::to_vec_pretty(&value)?)?;
        Ok(())
    }

    #[test]
    fn gate_blocks_on_critical_divergence() -> Result<()> {
        let dir = tempdir()?;
        let legacy_dir = dir.path().join("legacy");
        let rust_dir = dir.path().join("rust");
        std::fs::create_dir_all(&legacy_dir)?;
        std::fs::create_dir_all(&rust_dir)?;

        write_json(
            &legacy_dir.join("control_status.json"),
            serde_json::json!({"data": {"service":"openagents-control-service","authProvider":"workos","activeOrgId":"user:1"}}),
        )?;
        write_json(
            &rust_dir.join("control_status.json"),
            serde_json::json!({"data": {"service":"openagents-control-service","authProvider":"mock","activeOrgId":"user:1"}}),
        )?;
        write_json(
            &legacy_dir.join("route_split_status.json"),
            serde_json::json!({"data": {}}),
        )?;
        write_json(
            &rust_dir.join("route_split_status.json"),
            serde_json::json!({"data": {}}),
        )?;
        write_json(
            &legacy_dir.join("khala_poll.json"),
            serde_json::json!({"messages":[{"sequence":1,"kind":"run.started","published_at":"ignore"}]}),
        )?;
        write_json(
            &rust_dir.join("khala_poll.json"),
            serde_json::json!({"messages":[{"sequence":1,"kind":"run.started","published_at":"ignore"}]}),
        )?;
        write_json(
            &legacy_dir.join("khala_metrics.json"),
            serde_json::json!({"driver":"memory","topic_windows":[]}),
        )?;
        write_json(
            &rust_dir.join("khala_metrics.json"),
            serde_json::json!({"driver":"memory","topic_windows":[]}),
        )?;

        let legacy_manifest = ShadowControlKhalaManifest {
            scenario_id: "critical-divergence".to_string(),
            control_status_path: "control_status.json".into(),
            control_route_split_status_path: "route_split_status.json".into(),
            khala_poll_path: "khala_poll.json".into(),
            khala_metrics_path: "khala_metrics.json".into(),
        };
        let rust_manifest = legacy_manifest.clone();
        std::fs::write(
            legacy_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&legacy_manifest)?,
        )?;
        std::fs::write(
            rust_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&rust_manifest)?,
        )?;

        let report = generate_control_khala_parity_report(
            &legacy_manifest,
            &rust_manifest,
            &ShadowControlKhalaGatePolicy::default(),
            legacy_dir.join("manifest.json"),
            rust_dir.join("manifest.json"),
        )?;
        assert_eq!(report.totals.critical, 1);
        assert_eq!(report.gate.decision, ShadowControlKhalaGateDecision::Block);
        Ok(())
    }

    #[test]
    fn warning_threshold_policy_allows_or_blocks() -> Result<()> {
        let dir = tempdir()?;
        let legacy_dir = dir.path().join("legacy");
        let rust_dir = dir.path().join("rust");
        std::fs::create_dir_all(&legacy_dir)?;
        std::fs::create_dir_all(&rust_dir)?;

        write_json(
            &legacy_dir.join("control_status.json"),
            serde_json::json!({"data": {"service":"openagents-control-service","authProvider":"workos","activeOrgId":"user:1"}}),
        )?;
        write_json(
            &rust_dir.join("control_status.json"),
            serde_json::json!({"data": {"service":"openagents-control-service","authProvider":"workos","activeOrgId":"user:1"}}),
        )?;
        write_json(
            &legacy_dir.join("route_split_status.json"),
            serde_json::json!({"data": {"mode":"rust"}}),
        )?;
        write_json(
            &rust_dir.join("route_split_status.json"),
            serde_json::json!({"data": {"mode":"legacy"}}),
        )?;
        write_json(
            &legacy_dir.join("khala_poll.json"),
            serde_json::json!({"messages":[]}),
        )?;
        write_json(
            &rust_dir.join("khala_poll.json"),
            serde_json::json!({"messages":[]}),
        )?;
        write_json(
            &legacy_dir.join("khala_metrics.json"),
            serde_json::json!({"driver":"memory","topic_windows":[]}),
        )?;
        write_json(
            &rust_dir.join("khala_metrics.json"),
            serde_json::json!({"driver":"memory","topic_windows":[]}),
        )?;

        let manifest = ShadowControlKhalaManifest {
            scenario_id: "warning-threshold".to_string(),
            control_status_path: "control_status.json".into(),
            control_route_split_status_path: "route_split_status.json".into(),
            khala_poll_path: "khala_poll.json".into(),
            khala_metrics_path: "khala_metrics.json".into(),
        };
        std::fs::write(
            legacy_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest)?,
        )?;
        std::fs::write(
            rust_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest)?,
        )?;

        let allow_report = generate_control_khala_parity_report(
            &manifest,
            &manifest,
            &ShadowControlKhalaGatePolicy {
                max_warning_count: 1,
                block_on_critical: true,
            },
            legacy_dir.join("manifest.json"),
            rust_dir.join("manifest.json"),
        )?;
        assert_eq!(allow_report.totals.warning, 1);
        assert_eq!(
            allow_report.gate.decision,
            ShadowControlKhalaGateDecision::Allow
        );

        let block_report = generate_control_khala_parity_report(
            &manifest,
            &manifest,
            &ShadowControlKhalaGatePolicy {
                max_warning_count: 0,
                block_on_critical: true,
            },
            legacy_dir.join("manifest.json"),
            rust_dir.join("manifest.json"),
        )?;
        assert_eq!(block_report.totals.warning, 1);
        assert_eq!(
            block_report.gate.decision,
            ShadowControlKhalaGateDecision::Block
        );
        Ok(())
    }
}
