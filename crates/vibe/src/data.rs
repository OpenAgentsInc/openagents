use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use super::types::*;

static STORE: OnceLock<RwLock<HashMap<String, VibeSnapshot>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VibeSnapshot {
    pub projects: Vec<Project>,
    pub templates: Vec<ProjectTemplate>,
    pub files: Vec<ProjectFile>,
    pub tables: Vec<DatabaseTable>,
    pub deployments: Vec<Deployment>,
    pub domains: Vec<Domain>,
    pub analytics: Vec<AnalyticsData>,
    pub logs: Vec<String>,
    pub tasks: Vec<AgentTask>,
    pub auth: AuthState,
    pub plan_limits: PlanLimits,
    pub infra_customers: Vec<InfraCustomer>,
    pub usage: Vec<UsageMetric>,
    pub invoice: InvoiceSummary,
    pub billing_events: Vec<BillingEvent>,
}

impl VibeSnapshot {
    pub fn mock() -> Self {
        Self {
            projects: mock_projects(),
            templates: mock_templates(),
            files: mock_files(),
            tables: mock_tables(),
            deployments: mock_deployments(),
            domains: mock_domains(),
            analytics: mock_analytics(),
            logs: mock_terminal_logs(),
            tasks: mock_agent_tasks(),
            auth: mock_auth_state(),
            plan_limits: mock_plan_limits(),
            infra_customers: mock_infra_customers(),
            usage: mock_usage_metrics(),
            invoice: mock_invoice(),
            billing_events: mock_billing_events(),
        }
    }
}

fn state() -> &'static RwLock<HashMap<String, VibeSnapshot>> {
    STORE.get_or_init(|| {
        let mut map = HashMap::new();
        for project in mock_projects() {
            map.insert(project.id.clone(), VibeSnapshot::mock());
        }
        RwLock::new(map)
    })
}

fn with_snapshot<F, T>(project_id: &str, f: F) -> T
where
    F: FnOnce(&mut VibeSnapshot) -> T,
{
    let mut guard = state().write().expect("VibeSnapshot lock poisoned");
    let entry = guard
        .entry(project_id.to_string())
        .or_insert_with(VibeSnapshot::mock);
    f(entry)
}

pub async fn get_vibe_snapshot(project_id: String) -> Result<VibeSnapshot, ServerFnError> {
    let guard = state().read().unwrap();
    Ok(guard
        .get(&project_id)
        .cloned()
        .unwrap_or_else(VibeSnapshot::mock))
}

pub async fn run_wasi_job(project_id: String) -> Result<VibeSnapshot, ServerFnError> {
    let updated = with_snapshot(&project_id, |snap| {
        let new_id = snap.tasks.len() + 1;
        snap.logs
            .push(format!("[wasi] job {new_id} started on {project_id}"));
        snap.logs
            .push("[wasi] mounting /workspace + /cap".to_string());
        snap.tasks.push(AgentTask {
            id: new_id,
            title: format!("Run WASI job #{new_id}"),
            status: "running".to_string(),
        });
        snap.clone()
    });
    Ok(updated)
}

pub async fn tail_logs(project_id: String) -> Result<VibeSnapshot, ServerFnError> {
    let updated = with_snapshot(&project_id, |snap| {
        snap.logs.push(format!(
            "[logs] streaming /logs/events.ndjson ({project_id})"
        ));
        snap.clone()
    });
    Ok(updated)
}

pub async fn trigger_deploy(project_id: String) -> Result<VibeSnapshot, ServerFnError> {
    let updated = with_snapshot(&project_id, |snap| {
        let version = format!("v0.3.{}", snap.deployments.len() + 2);
        snap.deployments.insert(
            0,
            Deployment {
                version,
                status: "Deploying".to_string(),
                timestamp: chrono::Utc::now(),
            },
        );
        snap.clone()
    });
    Ok(updated)
}

pub async fn provision_infra_customer(project_id: String) -> Result<VibeSnapshot, ServerFnError> {
    let updated = with_snapshot(&project_id, |snap| {
        let new_id = snap.infra_customers.len() + 1;
        let customer_id = format!("customer-new-{new_id}");
        snap.infra_customers.insert(
            0,
            InfraCustomer {
                id: customer_id.clone(),
                subdomain: format!("new-{new_id}.vibe.run"),
                plan: "Starter".to_string(),
                status: "Provisioning".to_string(),
                r2_prefix: format!("customers/new-{new_id}/"),
                d1_database: format!("vibe-customer-new-{new_id}"),
                durable_object: format!("do:customer:new-{new_id}"),
            },
        );
        snap.billing_events.insert(
            0,
            BillingEvent {
                id: format!("evt-provision-{new_id}"),
                label: "Provisioning requested".to_string(),
                amount: "$0.00".to_string(),
                status: "Queued".to_string(),
                timestamp: "just now".to_string(),
            },
        );
        snap.clone()
    });
    Ok(updated)
}

pub async fn refresh_usage(project_id: String) -> Result<VibeSnapshot, ServerFnError> {
    let updated = with_snapshot(&project_id, |snap| {
        snap.usage.push(UsageMetric {
            label: "Bandwidth".to_string(),
            value: "281 GB".to_string(),
            delta: "+0.3%".to_string(),
            limit: Some("1 TB included".to_string()),
            remaining: Some("743 GB left".to_string()),
        });
        snap.billing_events.insert(
            0,
            BillingEvent {
                id: format!("evt-usage-refresh-{}", snap.billing_events.len() + 1),
                label: "Usage refreshed".to_string(),
                amount: "$0.00".to_string(),
                status: "Info".to_string(),
                timestamp: "just now".to_string(),
            },
        );
        snap.clone()
    });
    Ok(updated)
}

pub async fn pay_invoice(project_id: String) -> Result<VibeSnapshot, ServerFnError> {
    let updated = with_snapshot(&project_id, |snap| {
        snap.invoice.status = "Paid".to_string();
        snap.billing_events.insert(
            0,
            BillingEvent {
                id: format!("evt-invoice-paid-{}", snap.billing_events.len() + 1),
                label: format!("Invoice {} paid", snap.invoice.invoice_id),
                amount: snap.invoice.total.clone(),
                status: "Settled".to_string(),
                timestamp: "just now".to_string(),
            },
        );
        snap.clone()
    });
    Ok(updated)
}

pub async fn download_invoice(project_id: String) -> Result<VibeSnapshot, ServerFnError> {
    let updated = with_snapshot(&project_id, |snap| {
        snap.billing_events.insert(
            0,
            BillingEvent {
                id: format!("evt-invoice-download-{}", snap.billing_events.len() + 1),
                label: format!("Invoice {} downloaded", snap.invoice.invoice_id),
                amount: "$0.00".to_string(),
                status: "Info".to_string(),
                timestamp: "just now".to_string(),
            },
        );
        snap.clone()
    });
    Ok(updated)
}
