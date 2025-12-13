use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VibeTab {
    Projects,
    Editor,
    Database,
    Deploy,
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
    pub timestamp: DateTime<Utc>,
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

// Mock data constructors -------------------------------------------------

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
        ProjectFile { path: "/task/spec.json".to_string(), kind: "json".to_string() },
        ProjectFile { path: "/workspace/src/main.rs".to_string(), kind: "rust".to_string() },
        ProjectFile { path: "/workspace/src/lib.rs".to_string(), kind: "rust".to_string() },
        ProjectFile { path: "/workspace/README.md".to_string(), kind: "md".to_string() },
        ProjectFile { path: "/cap/http/requests".to_string(), kind: "cap".to_string() },
        ProjectFile { path: "/cap/ws/control".to_string(), kind: "cap".to_string() },
        ProjectFile { path: "/logs/events.ndjson".to_string(), kind: "log".to_string() },
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
            timestamp: Utc::now(),
        },
        Deployment {
            version: "v0.3.1".to_string(),
            status: "Healthy".to_string(),
            timestamp: Utc::now() - chrono::Duration::hours(4),
        },
    ]
}

pub fn mock_domains() -> Vec<Domain> {
    vec![
        Domain { host: "vibe.openagents.dev".to_string(), status: "Active".to_string() },
        Domain { host: "preview.vibe.dev".to_string(), status: "Provisioning".to_string() },
    ]
}

pub fn mock_analytics() -> Vec<AnalyticsData> {
    vec![
        AnalyticsData { label: "Req/min".to_string(), value: "420".to_string(), delta: "+8%".to_string() },
        AnalyticsData { label: "p95".to_string(), value: "122ms".to_string(), delta: "-5%".to_string() },
        AnalyticsData { label: "Errors".to_string(), value: "0.21%".to_string(), delta: "-0.1%".to_string() },
        AnalyticsData { label: "Active agents".to_string(), value: "18".to_string(), delta: "+3".to_string() },
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
        AgentTask { id: 1, title: "Deploy app.wasm".to_string(), status: "running".to_string() },
        AgentTask { id: 2, title: "Refactor router.rs".to_string(), status: "queued".to_string() },
        AgentTask { id: 3, title: "Tail logs".to_string(), status: "streaming".to_string() },
        AgentTask { id: 4, title: "Sync with NostrFs".to_string(), status: "complete".to_string() },
    ]
}
