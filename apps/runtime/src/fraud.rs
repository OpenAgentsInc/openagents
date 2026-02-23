use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FraudIncident {
    pub schema: String,
    pub incident_id: String,
    pub incident_type: String,
    pub severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_worker_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub evidence: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl FraudIncident {
    pub fn new(
        incident_type: &str,
        severity: &str,
        provider_id: Option<String>,
        provider_worker_id: Option<String>,
        job_hash: Option<String>,
        run_id: Option<String>,
        quote_sha256: Option<String>,
        actions: Vec<String>,
        evidence: serde_json::Value,
    ) -> Option<Self> {
        let incident_type = incident_type.trim();
        if incident_type.is_empty() {
            return None;
        }
        let severity = severity.trim();
        if severity.is_empty() {
            return None;
        }

        #[derive(Serialize)]
        struct IncidentIdInput<'a> {
            incident_type: &'a str,
            provider_worker_id: &'a Option<String>,
            job_hash: &'a Option<String>,
            quote_sha256: &'a Option<String>,
        }
        let input = IncidentIdInput {
            incident_type,
            provider_worker_id: &provider_worker_id,
            job_hash: &job_hash,
            quote_sha256: &quote_sha256,
        };
        let digest = protocol::hash::canonical_hash(&input).ok()?;
        let incident_id = format!("incident_{}", &digest[..16]);

        Some(Self {
            schema: "openagents.fraud.incident.v1".to_string(),
            incident_id,
            incident_type: incident_type.to_string(),
            severity: severity.to_string(),
            provider_id,
            provider_worker_id,
            job_hash,
            run_id,
            quote_sha256,
            actions,
            evidence,
            created_at: Utc::now(),
        })
    }
}

#[derive(Default)]
pub struct FraudIncidentLog {
    inner: Mutex<VecDeque<FraudIncident>>,
}

const FRAUD_INCIDENT_LOG_CAPACITY: usize = 500;

impl FraudIncidentLog {
    pub async fn record(&self, incident: FraudIncident) -> bool {
        let mut inner = self.inner.lock().await;
        if inner
            .iter()
            .any(|entry| entry.incident_id == incident.incident_id)
        {
            return false;
        }
        if inner.len() >= FRAUD_INCIDENT_LOG_CAPACITY {
            inner.pop_front();
        }
        inner.push_back(incident);
        true
    }

    pub async fn list(&self, limit: usize) -> Vec<FraudIncident> {
        let inner = self.inner.lock().await;
        let limit = limit.max(1).min(FRAUD_INCIDENT_LOG_CAPACITY);
        inner.iter().rev().take(limit).cloned().collect::<Vec<_>>()
    }
}
