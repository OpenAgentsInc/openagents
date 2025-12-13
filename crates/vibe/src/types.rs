use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VibeTab {
    Projects,
    Editor,
    Database,
    Deploy,
    Infra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub description: String,
    pub language: String,
    pub updated: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectTemplate {
    pub id: String,
    pub name: String,
    pub category: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatabaseTable {
    pub name: String,
    pub rows: Vec<DatabaseRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatabaseRow {
    pub id: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deployment {
    pub version: String,
    pub status: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Domain {
    pub host: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalyticsData {
    pub label: String,
    pub value: String,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: usize,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthState {
    pub npub: String,
    pub plan: String,
    pub status: String,
    pub token_preview: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanLimits {
    pub plan: String,
    pub ai_prompts: String,
    pub agent_runs: String,
    pub infra_credits: String,
    pub api_rate: String,
    pub billing_cycle: String,
}

impl Default for AuthState {
    fn default() -> Self {
        Self {
            npub: "npub1...".to_string(),
            plan: "Free".to_string(),
            status: "anonymous".to_string(),
            token_preview: "jwt-***".to_string(),
        }
    }
}

impl Default for PlanLimits {
    fn default() -> Self {
        Self {
            plan: "Free".to_string(),
            ai_prompts: "100/day".to_string(),
            agent_runs: "10/day".to_string(),
            infra_credits: "$0".to_string(),
            api_rate: "60 rpm".to_string(),
            billing_cycle: "Monthly".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InfraCustomer {
    pub id: String,
    pub subdomain: String,
    pub plan: String,
    pub status: String,
    pub r2_prefix: String,
    pub d1_database: String,
    pub durable_object: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageMetric {
    pub label: String,
    pub value: String,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BillingEvent {
    pub id: String,
    pub label: String,
    pub amount: String,
    pub status: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvoiceLine {
    pub description: String,
    pub quantity: String,
    pub total: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvoiceSummary {
    pub period: String,
    pub total: String,
    pub lines: Vec<InvoiceLine>,
}

impl Default for InvoiceSummary {
    fn default() -> Self {
        Self {
            period: "Pending".to_string(),
            total: "$0.00".to_string(),
            lines: vec![],
        }
    }
}

pub fn mock_projects() -> Vec<Project> {
    vec![
        Project {
            id: "workspace".to_string(),
            name: "Workspace Snapshot".to_string(),
            kind: "OANIX env".to_string(),
            description: "Plan 9-style namespace with /workspace + /cap mounts".to_string(),
            language: "Rust / WASM".to_string(),
            updated: "2m ago".to_string(),
            status: "Ready".to_string(),
        },
        Project {
            id: "chat-ops".to_string(),
            name: "Chat Ops".to_string(),
            kind: "LLM agent".to_string(),
            description: "Agent that triages issues and writes patches in OANIX".to_string(),
            language: "TypeScript".to_string(),
            updated: "14m ago".to_string(),
            status: "Running".to_string(),
        },
        Project {
            id: "payments".to_string(),
            name: "Payments API".to_string(),
            kind: "Service".to_string(),
            description: "Spark + Lightning payments exposed via /cap/payments".to_string(),
            language: "Rust".to_string(),
            updated: "1h ago".to_string(),
            status: "Deploying".to_string(),
        },
        Project {
            id: "browser".to_string(),
            name: "Browser Agent".to_string(),
            kind: "WASM agent".to_string(),
            description: "Headless browser driver in WASI using HttpFs + WsFs".to_string(),
            language: "Rust / JS".to_string(),
            updated: "1d ago".to_string(),
            status: "Ready".to_string(),
        },
    ]
}

pub fn mock_templates() -> Vec<ProjectTemplate> {
    vec![
        ProjectTemplate {
            id: "starter-web".to_string(),
            name: "Web Starter".to_string(),
            category: "Web".to_string(),
            summary: "Dioxus + Vite starter with OANIX namespace mounts".to_string(),
        },
        ProjectTemplate {
            id: "agent-runtime".to_string(),
            name: "Agent Runtime".to_string(),
            category: "Agents".to_string(),
            summary: "Prewired agent loop with OANIX scheduler + executors".to_string(),
        },
        ProjectTemplate {
            id: "payments".to_string(),
            name: "Payments Agent".to_string(),
            category: "Commerce".to_string(),
            summary: "Spark + Lightning wallet exposed as file capabilities".to_string(),
        },
        ProjectTemplate {
            id: "browser-kit".to_string(),
            name: "Browser Kit".to_string(),
            category: "Automation".to_string(),
            summary: "WASI + HttpFs/WsFs example with task + logs mounts".to_string(),
        },
    ]
}

pub fn mock_files() -> Vec<ProjectFile> {
    vec![
        ProjectFile {
            path: "/task/spec.json".to_string(),
            kind: "json".to_string(),
        },
        ProjectFile {
            path: "/workspace/src/main.rs".to_string(),
            kind: "rust".to_string(),
        },
        ProjectFile {
            path: "/workspace/src/lib.rs".to_string(),
            kind: "rust".to_string(),
        },
        ProjectFile {
            path: "/workspace/README.md".to_string(),
            kind: "md".to_string(),
        },
        ProjectFile {
            path: "/cap/http/requests".to_string(),
            kind: "cap".to_string(),
        },
        ProjectFile {
            path: "/cap/ws/control".to_string(),
            kind: "cap".to_string(),
        },
        ProjectFile {
            path: "/logs/events.ndjson".to_string(),
            kind: "log".to_string(),
        },
    ]
}

pub fn mock_tables() -> Vec<DatabaseTable> {
    vec![DatabaseTable {
        name: "runs".to_string(),
        rows: vec![
            DatabaseRow {
                id: "run-481".to_string(),
                values: vec![
                    "scheduled".to_string(),
                    "OANIX".to_string(),
                    "task:deploy".to_string(),
                    "2m ago".to_string(),
                ],
            },
            DatabaseRow {
                id: "run-480".to_string(),
                values: vec![
                    "completed".to_string(),
                    "OANIX".to_string(),
                    "task:tests".to_string(),
                    "12m ago".to_string(),
                ],
            },
            DatabaseRow {
                id: "run-479".to_string(),
                values: vec![
                    "failed".to_string(),
                    "OANIX".to_string(),
                    "task:lint".to_string(),
                    "16m ago".to_string(),
                ],
            },
        ],
    }]
}

pub fn mock_deployments() -> Vec<Deployment> {
    vec![
        Deployment {
            version: "v0.3.2".to_string(),
            status: "Live".to_string(),
            timestamp: chrono::Utc::now(),
        },
        Deployment {
            version: "v0.3.1".to_string(),
            status: "Healthy".to_string(),
            timestamp: chrono::Utc::now() - chrono::Duration::hours(4),
        },
    ]
}

pub fn mock_domains() -> Vec<Domain> {
    vec![
        Domain {
            host: "vibe.openagents.dev".to_string(),
            status: "Active".to_string(),
        },
        Domain {
            host: "preview.vibe.dev".to_string(),
            status: "Provisioning".to_string(),
        },
    ]
}

pub fn mock_analytics() -> Vec<AnalyticsData> {
    vec![
        AnalyticsData {
            label: "Req/min".to_string(),
            value: "420".to_string(),
            delta: "+8%".to_string(),
        },
        AnalyticsData {
            label: "p95".to_string(),
            value: "122ms".to_string(),
            delta: "-5%".to_string(),
        },
        AnalyticsData {
            label: "Errors".to_string(),
            value: "0.21%".to_string(),
            delta: "-0.1%".to_string(),
        },
        AnalyticsData {
            label: "Active agents".to_string(),
            value: "18".to_string(),
            delta: "+3".to_string(),
        },
    ]
}

pub fn mock_terminal_logs() -> Vec<String> {
    vec![
        "[task] scheduled deploy:run".to_string(),
        "[oanix] mount /workspace, /task, /logs, /cap/http, /cap/ws".to_string(),
        "[wasi] launching app.wasm (pid=12)".to_string(),
        "[cap/http] POST https://api.openagents.com/v1/build".to_string(),
    ]
}

pub fn mock_agent_tasks() -> Vec<AgentTask> {
    vec![
        AgentTask {
            id: 1,
            title: "Deploy app.wasm".to_string(),
            status: "running".to_string(),
        },
        AgentTask {
            id: 2,
            title: "Refactor router.rs".to_string(),
            status: "queued".to_string(),
        },
        AgentTask {
            id: 3,
            title: "Tail logs".to_string(),
            status: "streaming".to_string(),
        },
        AgentTask {
            id: 4,
            title: "Sync with NostrFs".to_string(),
            status: "complete".to_string(),
        },
    ]
}

pub fn mock_auth_state() -> AuthState {
    AuthState {
        npub: "npub1vibe...demo".to_string(),
        plan: "Pro".to_string(),
        status: "verified".to_string(),
        token_preview: "jwt-abc123...".to_string(),
    }
}

pub fn mock_plan_limits() -> PlanLimits {
    PlanLimits {
        plan: "Pro".to_string(),
        ai_prompts: "1,000/day".to_string(),
        agent_runs: "100/day".to_string(),
        infra_credits: "$50".to_string(),
        api_rate: "300 rpm".to_string(),
        billing_cycle: "Dec cycle".to_string(),
    }
}

pub fn mock_infra_customers() -> Vec<InfraCustomer> {
    vec![
        InfraCustomer {
            id: "customer-acme".to_string(),
            subdomain: "acme.vibe.run".to_string(),
            plan: "Growth".to_string(),
            status: "Active".to_string(),
            r2_prefix: "customers/acme/".to_string(),
            d1_database: "vibe-customer-acme".to_string(),
            durable_object: "do:customer:acme".to_string(),
        },
        InfraCustomer {
            id: "customer-studio".to_string(),
            subdomain: "studio.vibe.run".to_string(),
            plan: "Starter".to_string(),
            status: "Provisioning".to_string(),
            r2_prefix: "customers/studio/".to_string(),
            d1_database: "vibe-customer-studio".to_string(),
            durable_object: "do:customer:studio".to_string(),
        },
        InfraCustomer {
            id: "customer-zen".to_string(),
            subdomain: "zenlabs.vibe.run".to_string(),
            plan: "Scale".to_string(),
            status: "Active".to_string(),
            r2_prefix: "customers/zen/".to_string(),
            d1_database: "vibe-customer-zen".to_string(),
            durable_object: "do:customer:zen".to_string(),
        },
    ]
}

pub fn mock_usage_metrics() -> Vec<UsageMetric> {
    vec![
        UsageMetric {
            label: "Worker reqs".to_string(),
            value: "12.4M".to_string(),
            delta: "+6%".to_string(),
        },
        UsageMetric {
            label: "DO reqs".to_string(),
            value: "3.1M".to_string(),
            delta: "+2%".to_string(),
        },
        UsageMetric {
            label: "R2 storage".to_string(),
            value: "38 GB".to_string(),
            delta: "+1.4 GB".to_string(),
        },
        UsageMetric {
            label: "AI tokens".to_string(),
            value: "2.1M".to_string(),
            delta: "-4%".to_string(),
        },
        UsageMetric {
            label: "Bandwidth".to_string(),
            value: "280 GB".to_string(),
            delta: "+9%".to_string(),
        },
    ]
}

pub fn mock_invoice() -> InvoiceSummary {
    InvoiceSummary {
        period: "Dec 2024".to_string(),
        total: "$3,068.10".to_string(),
        lines: vec![
            InvoiceLine {
                description: "Platform fee (Scale)".to_string(),
                quantity: "1".to_string(),
                total: "$2,499.00".to_string(),
            },
            InvoiceLine {
                description: "Worker requests (500M)".to_string(),
                quantity: "1".to_string(),
                total: "$1,000.00".to_string(),
            },
            InvoiceLine {
                description: "Durable Objects (100M)".to_string(),
                quantity: "1".to_string(),
                total: "$500.00".to_string(),
            },
            InvoiceLine {
                description: "R2 storage (500 GB)".to_string(),
                quantity: "1".to_string(),
                total: "$10.00".to_string(),
            },
            InvoiceLine {
                description: "AI inference (5M tokens)".to_string(),
                quantity: "1".to_string(),
                total: "$500.00".to_string(),
            },
            InvoiceLine {
                description: "Credits + discounts".to_string(),
                quantity: "1".to_string(),
                total: "-$1,440.90".to_string(),
            },
        ],
    }
}

pub fn mock_billing_events() -> Vec<BillingEvent> {
    vec![
        BillingEvent {
            id: "evt-invoice-2412".to_string(),
            label: "Invoice generated (Dec 2024)".to_string(),
            amount: "$3,068.10".to_string(),
            status: "Pending".to_string(),
            timestamp: "3m ago".to_string(),
        },
        BillingEvent {
            id: "evt-usage-2412".to_string(),
            label: "Usage update (Scale)".to_string(),
            amount: "$5,409.00".to_string(),
            status: "Estimated".to_string(),
            timestamp: "1h ago".to_string(),
        },
        BillingEvent {
            id: "evt-credit-2412".to_string(),
            label: "Credits applied".to_string(),
            amount: "-$2,000.00".to_string(),
            status: "Settled".to_string(),
            timestamp: "2h ago".to_string(),
        },
    ]
}
