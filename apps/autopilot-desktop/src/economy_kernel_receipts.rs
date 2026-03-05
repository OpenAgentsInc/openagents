use bitcoin::hashes::{Hash, sha256};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum Asset {
    AssetUnspecified,
    Btc,
    UsdCents,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackLatencyClass {
    FeedbackLatencyClassUnspecified,
    Instant,
    Short,
    Medium,
    Long,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum SeverityClass {
    SeverityClassUnspecified,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum VerificationTier {
    VerificationTierUnspecified,
    TierOObjective,
    Tier1Correlated,
    Tier2Heterogeneous,
    Tier3Adjudication,
    Tier4Human,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceGrade {
    ProvenanceGradeUnspecified,
    P0Minimal,
    P1Toolchain,
    P2Lineage,
    P3Attested,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(tag = "unit", content = "amount", rename_all = "snake_case")]
pub enum MoneyAmount {
    AmountMsats(u64),
    AmountSats(u64),
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct Money {
    pub asset: Asset,
    pub amount: MoneyAmount,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct TraceContext {
    pub session_id: Option<String>,
    pub trajectory_hash: Option<String>,
    pub job_hash: Option<String>,
    pub run_id: Option<String>,
    pub work_unit_id: Option<String>,
    pub contract_id: Option<String>,
    pub claim_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct PolicyContext {
    pub policy_bundle_id: String,
    pub policy_version: String,
    pub approved_by: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct EvidenceRef {
    pub kind: String,
    pub uri: String,
    pub digest: String,
    pub meta: BTreeMap<String, Value>,
}

impl EvidenceRef {
    pub fn new(kind: impl Into<String>, uri: impl Into<String>, digest: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            uri: uri.into(),
            digest: digest.into(),
            meta: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct ReceiptHints {
    pub category: Option<String>,
    pub tfb_class: Option<FeedbackLatencyClass>,
    pub severity: Option<SeverityClass>,
    pub achieved_verification_tier: Option<VerificationTier>,
    pub verification_correlated: Option<bool>,
    pub provenance_grade: Option<ProvenanceGrade>,
    pub reason_code: Option<String>,
    pub notional: Option<Money>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ReceiptRef {
    pub receipt_id: String,
    pub receipt_type: String,
    pub canonical_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Receipt {
    pub receipt_id: String,
    pub receipt_type: String,
    pub created_at_ms: i64,
    pub canonical_hash: String,
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub inputs_hash: String,
    pub outputs_hash: String,
    pub evidence: Vec<EvidenceRef>,
    pub hints: ReceiptHints,
    pub tags: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct ReceiptBuilder {
    receipt_id: String,
    receipt_type: String,
    created_at_ms: i64,
    idempotency_key: String,
    trace: TraceContext,
    policy: PolicyContext,
    inputs_payload: Value,
    outputs_payload: Value,
    evidence: Vec<EvidenceRef>,
    hints: ReceiptHints,
    tags: BTreeMap<String, String>,
}

impl ReceiptBuilder {
    pub fn new(
        receipt_id: impl Into<String>,
        receipt_type: impl Into<String>,
        created_at_ms: i64,
        idempotency_key: impl Into<String>,
        trace: TraceContext,
        policy: PolicyContext,
    ) -> Self {
        Self {
            receipt_id: receipt_id.into(),
            receipt_type: receipt_type.into(),
            created_at_ms,
            idempotency_key: idempotency_key.into(),
            trace,
            policy,
            inputs_payload: Value::Null,
            outputs_payload: Value::Null,
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
            tags: BTreeMap::new(),
        }
    }

    pub fn with_inputs_payload(mut self, payload: Value) -> Self {
        self.inputs_payload = payload;
        self
    }

    pub fn with_outputs_payload(mut self, payload: Value) -> Self {
        self.outputs_payload = payload;
        self
    }

    pub fn with_evidence(mut self, evidence: Vec<EvidenceRef>) -> Self {
        self.evidence = evidence;
        self
    }

    pub fn with_hints(mut self, hints: ReceiptHints) -> Self {
        self.hints = hints;
        self
    }

    pub fn with_tags(mut self, tags: BTreeMap<String, String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn build(mut self) -> Result<Receipt, String> {
        if self.receipt_id.trim().is_empty() {
            return Err("receipt_id cannot be empty".to_string());
        }
        if self.receipt_type.trim().is_empty() {
            return Err("receipt_type cannot be empty".to_string());
        }
        if self.idempotency_key.trim().is_empty() {
            return Err("idempotency_key cannot be empty".to_string());
        }
        if self.policy.policy_bundle_id.trim().is_empty() {
            return Err("policy_bundle_id cannot be empty".to_string());
        }
        if self.policy.policy_version.trim().is_empty() {
            return Err("policy_version cannot be empty".to_string());
        }

        self.evidence.sort_by(|lhs, rhs| {
            lhs.kind
                .cmp(&rhs.kind)
                .then_with(|| lhs.digest.cmp(&rhs.digest))
                .then_with(|| lhs.uri.cmp(&rhs.uri))
        });

        let normalized_inputs = canonicalize_value(self.inputs_payload);
        let normalized_outputs = canonicalize_value(self.outputs_payload);
        let inputs_hash = hash_value(&normalized_inputs)?;
        let outputs_hash = hash_value(&normalized_outputs)?;

        let canonical_payload = CanonicalReceiptPayload {
            receipt_id: self.receipt_id.clone(),
            receipt_type: self.receipt_type.clone(),
            created_at_ms: self.created_at_ms,
            idempotency_key: self.idempotency_key.clone(),
            trace: self.trace.clone(),
            policy: self.policy.clone(),
            inputs_hash: inputs_hash.clone(),
            outputs_hash: outputs_hash.clone(),
            evidence: self.evidence.clone(),
            hints: self.hints.clone(),
        };
        let canonical_hash =
            hash_value(&serde_json::to_value(canonical_payload).map_err(|error| {
                format!("failed to encode canonical receipt payload: {error}")
            })?)?;

        Ok(Receipt {
            receipt_id: self.receipt_id,
            receipt_type: self.receipt_type,
            created_at_ms: self.created_at_ms,
            canonical_hash,
            idempotency_key: self.idempotency_key,
            trace: self.trace,
            policy: self.policy,
            inputs_hash,
            outputs_hash,
            evidence: self.evidence,
            hints: self.hints,
            tags: self.tags,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
struct CanonicalReceiptPayload {
    receipt_id: String,
    receipt_type: String,
    created_at_ms: i64,
    idempotency_key: String,
    trace: TraceContext,
    policy: PolicyContext,
    inputs_hash: String,
    outputs_hash: String,
    evidence: Vec<EvidenceRef>,
    hints: ReceiptHints,
}

fn hash_value(value: &Value) -> Result<String, String> {
    let payload =
        serde_json::to_vec(value).map_err(|error| format!("failed to encode value: {error}"))?;
    let digest = sha256::Hash::hash(payload.as_slice());
    Ok(format!("sha256:{digest}"))
}

fn canonicalize_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (key, entry) in map {
                let normalized = canonicalize_value(entry);
                if should_drop_value(&normalized) {
                    continue;
                }
                sorted.insert(key, normalized);
            }
            let mut canonical = serde_json::Map::new();
            for (key, entry) in sorted {
                canonical.insert(key, entry);
            }
            Value::Object(canonical)
        }
        Value::Array(values) => {
            let normalized: Vec<Value> = values.into_iter().map(canonicalize_value).collect();
            Value::Array(normalized)
        }
        scalar => scalar,
    }
}

fn should_drop_value(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(value) => value.is_empty(),
        Value::Array(values) => values.is_empty(),
        Value::Object(values) => values.is_empty(),
        Value::Bool(_) | Value::Number(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_builder() -> ReceiptBuilder {
        ReceiptBuilder::new(
            "receipt-1",
            "earn.job.settlement_observed.v1",
            1_762_000_000_000,
            "idemp-1",
            TraceContext {
                session_id: Some("session-1".to_string()),
                trajectory_hash: Some("sha256:trajectory".to_string()),
                job_hash: Some("sha256:job".to_string()),
                run_id: Some("run-1".to_string()),
                work_unit_id: Some("work-job-1".to_string()),
                contract_id: None,
                claim_id: None,
            },
            PolicyContext {
                policy_bundle_id: "policy.earn.default".to_string(),
                policy_version: "1".to_string(),
                approved_by: "desktop.autopilot".to_string(),
            },
        )
        .with_evidence(vec![EvidenceRef::new(
            "wallet_receive",
            "oa://wallet/payments/123",
            "sha256:wallet123",
        )])
        .with_hints(ReceiptHints {
            category: Some("compute".to_string()),
            tfb_class: Some(FeedbackLatencyClass::Short),
            severity: Some(SeverityClass::Low),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            reason_code: None,
            notional: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(42),
            }),
        })
    }

    #[test]
    fn canonical_hash_is_stable_when_object_key_order_changes() {
        let receipt_a = sample_builder()
            .with_inputs_payload(json!({
                "b": 2,
                "a": 1,
                "nested": {"y": true, "x": false}
            }))
            .with_outputs_payload(json!({
                "result": "paid",
                "proof": "sha256:wallet123"
            }))
            .build()
            .expect("receipt should build");

        let receipt_b = sample_builder()
            .with_inputs_payload(json!({
                "nested": {"x": false, "y": true},
                "a": 1,
                "b": 2
            }))
            .with_outputs_payload(json!({
                "proof": "sha256:wallet123",
                "result": "paid"
            }))
            .build()
            .expect("receipt should build");

        assert_eq!(receipt_a.inputs_hash, receipt_b.inputs_hash);
        assert_eq!(receipt_a.outputs_hash, receipt_b.outputs_hash);
        assert_eq!(receipt_a.canonical_hash, receipt_b.canonical_hash);
    }

    #[test]
    fn canonical_hash_normalizes_optional_null_and_empty_values() {
        let receipt_with_nulls = sample_builder()
            .with_inputs_payload(json!({
                "job_id": "job-1",
                "optional": null,
                "empty_array": [],
                "empty_string": ""
            }))
            .with_outputs_payload(json!({
                "status": "paid",
                "notes": null
            }))
            .build()
            .expect("receipt should build");

        let receipt_without_optional = sample_builder()
            .with_inputs_payload(json!({
                "job_id": "job-1"
            }))
            .with_outputs_payload(json!({
                "status": "paid"
            }))
            .build()
            .expect("receipt should build");

        assert_eq!(
            receipt_with_nulls.inputs_hash,
            receipt_without_optional.inputs_hash
        );
        assert_eq!(
            receipt_with_nulls.outputs_hash,
            receipt_without_optional.outputs_hash
        );
        assert_eq!(
            receipt_with_nulls.canonical_hash,
            receipt_without_optional.canonical_hash
        );
    }

    #[test]
    fn canonical_hash_excludes_tags() {
        let mut tags_a = BTreeMap::new();
        tags_a.insert("debug".to_string(), "first".to_string());
        let mut tags_b = BTreeMap::new();
        tags_b.insert("debug".to_string(), "second".to_string());

        let receipt_a = sample_builder()
            .with_inputs_payload(json!({"job_id": "job-1"}))
            .with_outputs_payload(json!({"status": "paid"}))
            .with_tags(tags_a)
            .build()
            .expect("receipt should build");

        let receipt_b = sample_builder()
            .with_inputs_payload(json!({"job_id": "job-1"}))
            .with_outputs_payload(json!({"status": "paid"}))
            .with_tags(tags_b)
            .build()
            .expect("receipt should build");

        assert_eq!(receipt_a.inputs_hash, receipt_b.inputs_hash);
        assert_eq!(receipt_a.outputs_hash, receipt_b.outputs_hash);
        assert_eq!(receipt_a.canonical_hash, receipt_b.canonical_hash);
        assert_ne!(receipt_a.tags, receipt_b.tags);
    }

    #[test]
    fn hashes_use_sha256_prefix() {
        let receipt = sample_builder()
            .with_inputs_payload(json!({"job_id": "job-1"}))
            .with_outputs_payload(json!({"status": "paid"}))
            .build()
            .expect("receipt should build");

        assert!(receipt.inputs_hash.starts_with("sha256:"));
        assert!(receipt.outputs_hash.starts_with("sha256:"));
        assert!(receipt.canonical_hash.starts_with("sha256:"));
    }
}
