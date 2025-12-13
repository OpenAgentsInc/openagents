# Coder Technical Architecture

Detailed technical specification for implementing the Coder platform.

---

## 1. System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           CODER SYSTEM ARCHITECTURE                           │
└─────────────────────────────────────────────────────────────────────────────┘

                    ┌─────────────────────────────────────┐
                    │         USER INTERFACES              │
                    │  ┌─────────┐ ┌─────────┐ ┌───────┐  │
                    │  │ Desktop │ │   Web   │ │  API  │  │
                    │  │ (GPUI)  │ │ (WASM)  │ │(REST) │  │
                    │  └────┬────┘ └────┬────┘ └───┬───┘  │
                    └───────┼───────────┼──────────┼──────┘
                            │           │          │
                            └─────────┬─┘          │
                                      │            │
                    ┌─────────────────▼────────────▼──────┐
                    │         CODER RUNTIME LAYER           │
                    │  ┌─────────────────────────────────┐ │
                    │  │     MechaCoder + AI Backend     │ │
                    │  │  Claude │ OpenAI │ Workers AI   │ │
                    │  └─────────────────────────────────┘ │
                    │  ┌─────────────────────────────────┐ │
                    │  │          OANIX Kernel           │ │
                    │  │  Namespace │ Scheduler │ WASI   │ │
                    │  └─────────────────────────────────┘ │
                    └─────────────────┬───────────────────┘
                                      │
                    ┌─────────────────▼───────────────────┐
                    │      CLOUDFLARE EDGE LAYER          │
                    │  ┌────────┐ ┌────────┐ ┌─────────┐  │
                    │  │Workers │ │  DOs   │ │   AI    │  │
                    │  └────────┘ └────────┘ └─────────┘  │
                    │  ┌────────┐ ┌────────┐ ┌─────────┐  │
                    │  │   R2   │ │   D1   │ │   KV    │  │
                    │  └────────┘ └────────┘ └─────────┘  │
                    └─────────────────────────────────────┘
```

---

## 2. Component Specifications

### 2.1 Coder Web (Browser Application)

**Target:** `coder.run` - Browser-based IDE

**Technology:**
- Dioxus 0.7 compiled to WASM
- ~2-3 MB bundle size target
- Service Worker for offline capability

**Architecture:**
```rust
// crates/coder-web/src/main.rs
use dioxus::prelude::*;
use coder::{CoderScreen, CoderSnapshot};

fn main() {
    // Launch browser app
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Global state
    let auth = use_context_provider(|| Signal::new(AuthState::default()));
    let project = use_context_provider(|| Signal::new(ProjectState::default()));

    rsx! {
        Router::<Route> {}
    }
}

#[derive(Routable, Clone)]
enum Route {
    #[route("/")]
    Home {},
    #[route("/project/:id")]
    Project { id: String },
    #[route("/marketplace")]
    Marketplace {},
    #[route("/settings")]
    Settings {},
}
```

**Key Features:**
1. **Authentication**: Nostr keypair (browser-generated, stored in IndexedDB)
2. **Project Storage**: IndexedDB for local, R2 sync for cloud
3. **Real-time Sync**: WebSocket to Cloudflare DO
4. **AI Integration**: Fetch to `/api/chat/ws` → Claude streaming

### 2.2 Cloudflare Worker Architecture

**Current Implementation** (`crates/cloudflare/`):
- `lib.rs`: Entry point, routes to Relay DO
- `relay_do.rs`: NIP-01 Nostr relay
- `dvm.rs`: NIP-90 job processing
- `signing.rs`: Schnorr signatures for WASM

**Extended Architecture:**
```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    CLOUDFLARE WORKERS ARCHITECTURE                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  WORKER: coder-api (main entry point)                                │    │
│  │  Routes:                                                             │    │
│  │    /api/auth/*      → Auth handlers                                  │    │
│  │    /api/project/*   → Project CRUD                                   │    │
│  │    /api/chat/ws     → AI chat (Claude streaming)                     │    │
│  │    /api/agent/*     → Agent execution                                │    │
│  │    /api/infra/*     → Infrastructure provisioning                    │    │
│  │    /ws              → WebSocket upgrade → Customer DO                │    │
│  └────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│          ┌─────────────────────────┼─────────────────────────┐              │
│          ▼                         ▼                         ▼              │
│  ┌───────────────┐        ┌───────────────┐        ┌───────────────┐       │
│  │  RELAY DO     │        │  PROJECT DO   │        │  CUSTOMER DO  │       │
│  │  (Nostr)      │        │  (per-project)│        │  (per-tenant) │       │
│  │               │        │               │        │               │       │
│  │  - NIP-01     │        │  - File state │        │  - Billing    │       │
│  │  - NIP-90 DVM │        │  - Agent runs │        │  - Quotas     │       │
│  │  - Pub/Sub    │        │  - Logs       │        │  - Settings   │       │
│  └───────────────┘        └───────────────┘        └───────────────┘       │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  STORAGE LAYER                                                       │    │
│  │                                                                       │    │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │    │
│  │  │     R2      │  │     D1      │  │     KV      │                  │    │
│  │  │  (Objects)  │  │  (SQLite)   │  │   (Cache)   │                  │    │
│  │  │             │  │             │  │             │                  │    │
│  │  │ - Artifacts │  │ - Users     │  │ - Sessions  │                  │    │
│  │  │ - Builds    │  │ - Projects  │  │ - Config    │                  │    │
│  │  │ - Logs      │  │ - Billing   │  │ - Limits    │                  │    │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.3 OANIX Integration Layer

**OANIX provides runtime abstraction** (from `crates/oanix/`):

```rust
// Integration with Coder
use oanix::{Namespace, OanixEnv, FileService};
use oanix::services::{WorkspaceFs, LogsFs, HttpFs, NostrFs};

pub struct CoderProject {
    id: String,
    env: OanixEnv,
}

impl CoderProject {
    pub async fn new(project_id: String, base_path: PathBuf) -> Self {
        // Build namespace for this project
        let namespace = Namespace::builder()
            // User's project files
            .mount("/workspace", WorkspaceFs::new(base_path.join("workspace")))
            // Build outputs
            .mount("/dist", WorkspaceFs::new(base_path.join("dist")))
            // Agent logs and ATIF
            .mount("/logs", LogsFs::new())
            // HTTP capability for agents
            .mount("/cap/http", HttpFs::new())
            // Nostr capability for publishing
            .mount("/cap/nostr", NostrFs::new("wss://relay.coder.run"))
            .build();

        let env = OanixEnv::new(namespace);
        Self { id: project_id, env }
    }

    pub async fn run_agent(&self, job: AgentJob) -> Result<JobId, Error> {
        // Submit to OANIX scheduler
        let job_spec = JobSpec {
            kind: JobKind::Wasi {
                module: job.agent_wasm.clone(),
                args: job.args.clone(),
            },
            priority: job.priority,
            timeout: Duration::from_secs(300),
        };

        self.env.scheduler().submit(job_spec).await
    }
}
```

**FileService → Cloudflare Mapping:**

| OANIX Service | Browser Implementation | Edge Implementation |
|---------------|----------------------|---------------------|
| `WorkspaceFs` | IndexedDB | DO SQLite + R2 |
| `LogsFs` | Memory + IndexedDB | DO SQLite |
| `HttpFs` | `fetch()` API | `fetch()` in Worker |
| `NostrFs` | WebSocket to relay | DO → Relay DO |
| `MemFs` | Memory | Memory |

### 2.4 AI Chat Backend

**Current:** `crates/mechacoder/` with Claude SDK integration

**Extended for Coder:**
```rust
// crates/coder/src/ai/mod.rs

pub struct AiBackend {
    claude: Option<ClaudeClient>,
    openai: Option<OpenAiClient>,
    workers_ai: Option<WorkersAiClient>,
    ollama: Option<OllamaClient>,
}

impl AiBackend {
    pub async fn stream_chat(
        &self,
        messages: Vec<Message>,
        config: ChatConfig,
    ) -> impl Stream<Item = ChatEvent> {
        // Try backends in priority order
        match config.provider {
            Provider::Claude => self.claude_stream(messages).await,
            Provider::OpenAi => self.openai_stream(messages).await,
            Provider::WorkersAi => self.workers_ai_stream(messages).await,
            Provider::Ollama => self.ollama_stream(messages).await,
            Provider::Auto => self.auto_select_stream(messages).await,
        }
    }
}

// Cloudflare Worker handler
#[worker::event(fetch)]
pub async fn handle_chat_ws(
    req: Request,
    env: Env,
    _ctx: Context,
) -> Result<Response, worker::Error> {
    // Upgrade to WebSocket
    let pair = WebSocketPair::new()?;
    let server = pair.server;
    let client = pair.client;

    // Spawn handler
    wasm_bindgen_futures::spawn_local(async move {
        let ai = AiBackend::from_env(&env);

        while let Some(msg) = server.events().next().await {
            match msg {
                WebSocketEvent::Message(data) => {
                    let req: ChatRequest = serde_json::from_str(&data)?;

                    // Stream response back
                    let stream = ai.stream_chat(req.messages, req.config).await;
                    pin_mut!(stream);

                    while let Some(event) = stream.next().await {
                        server.send(&serde_json::to_string(&event)?)?;
                    }
                }
                WebSocketEvent::Close(_) => break,
            }
        }
    });

    Response::from_web_socket(client)
}
```

---

## 3. Data Models

### 3.1 Core Entities

```sql
-- D1 Schema

-- Users (identified by Nostr pubkey)
CREATE TABLE users (
    npub TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    plan TEXT NOT NULL DEFAULT 'free',
    stripe_customer_id TEXT,
    settings_json TEXT
);

-- Projects
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    owner_npub TEXT NOT NULL REFERENCES users(npub),
    name TEXT NOT NULL,
    template TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    settings_json TEXT,
    UNIQUE(owner_npub, name)
);

-- Project files (metadata only, content in R2)
CREATE TABLE project_files (
    project_id TEXT NOT NULL REFERENCES projects(id),
    path TEXT NOT NULL,
    size INTEGER NOT NULL,
    hash TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (project_id, path)
);

-- Agent runs
CREATE TABLE agent_runs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    agent_kind TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    input_json TEXT,
    output_json TEXT,
    error TEXT
);

-- Billing events
CREATE TABLE billing_events (
    id TEXT PRIMARY KEY,
    user_npub TEXT NOT NULL REFERENCES users(npub),
    event_type TEXT NOT NULL,
    amount_sats INTEGER NOT NULL,
    timestamp INTEGER NOT NULL,
    metadata_json TEXT
);

-- Infrastructure customers (resale)
CREATE TABLE infra_customers (
    id TEXT PRIMARY KEY,
    owner_npub TEXT NOT NULL REFERENCES users(npub),
    subdomain TEXT UNIQUE NOT NULL,
    plan TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    settings_json TEXT
);
```

### 3.2 TypeScript Types (for web client)

```typescript
// types.ts

interface User {
  npub: string;
  plan: 'free' | 'pro' | 'team' | 'business' | 'enterprise';
  createdAt: number;
  settings: UserSettings;
}

interface Project {
  id: string;
  ownerNpub: string;
  name: string;
  template?: string;
  createdAt: number;
  updatedAt: number;
  settings: ProjectSettings;
}

interface ProjectFile {
  projectId: string;
  path: string;
  size: number;
  hash: string;
  updatedAt: number;
}

interface AgentRun {
  id: string;
  projectId: string;
  agentKind: AgentKind;
  status: 'pending' | 'running' | 'completed' | 'failed';
  startedAt: number;
  completedAt?: number;
  input: Record<string, unknown>;
  output?: Record<string, unknown>;
  error?: string;
}

type AgentKind =
  | 'scaffold'
  | 'refactor'
  | 'add_endpoint'
  | 'generate_tests'
  | 'fix_bug'
  | 'explain_code';

interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: number;
  toolUses?: ToolUse[];
}

interface ToolUse {
  id: string;
  name: string;
  input: Record<string, unknown>;
  output?: string;
  status: 'running' | 'completed' | 'error';
}
```

---

## 4. API Specification

### 4.1 REST Endpoints

```yaml
# OpenAPI 3.0 specification

paths:
  /api/auth/challenge:
    post:
      summary: Get Nostr auth challenge
      requestBody:
        content:
          application/json:
            schema:
              type: object
              properties:
                npub:
                  type: string
      responses:
        200:
          content:
            application/json:
              schema:
                type: object
                properties:
                  challenge:
                    type: string
                  expiresAt:
                    type: integer

  /api/auth/verify:
    post:
      summary: Verify signed challenge
      requestBody:
        content:
          application/json:
            schema:
              type: object
              properties:
                npub:
                  type: string
                signature:
                  type: string
      responses:
        200:
          content:
            application/json:
              schema:
                type: object
                properties:
                  token:
                    type: string
                  expiresAt:
                    type: integer

  /api/projects:
    get:
      summary: List user's projects
      security:
        - bearerAuth: []
      responses:
        200:
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/Project'
    post:
      summary: Create new project
      security:
        - bearerAuth: []
      requestBody:
        content:
          application/json:
            schema:
              type: object
              properties:
                name:
                  type: string
                template:
                  type: string

  /api/projects/{id}:
    get:
      summary: Get project details
    put:
      summary: Update project
    delete:
      summary: Delete project

  /api/projects/{id}/files:
    get:
      summary: List project files
    post:
      summary: Create/update file
    delete:
      summary: Delete file

  /api/projects/{id}/agent:
    post:
      summary: Run agent on project
      requestBody:
        content:
          application/json:
            schema:
              type: object
              properties:
                kind:
                  type: string
                  enum: [scaffold, refactor, add_endpoint, generate_tests]
                input:
                  type: object

  /api/chat/ws:
    get:
      summary: WebSocket for AI chat
      description: Upgrade to WebSocket for streaming chat
```

### 4.2 WebSocket Protocol

```typescript
// Client → Server messages
type ClientMessage =
  | { type: 'chat'; content: string; projectId?: string }
  | { type: 'cancel' }
  | { type: 'ping' };

// Server → Client messages
type ServerMessage =
  | { type: 'text_delta'; text: string }
  | { type: 'tool_start'; toolId: string; name: string }
  | { type: 'tool_input'; toolId: string; json: string }
  | { type: 'tool_result'; toolId: string; output: string; isError: boolean }
  | { type: 'done'; error?: string }
  | { type: 'pong' };
```

---

## 5. Infrastructure Resale Architecture

### 5.1 Multi-Tenant Isolation

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    MULTI-TENANT ARCHITECTURE                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  CONTROL PLANE (coder-control worker)                                │    │
│  │                                                                       │    │
│  │  - Customer provisioning                                             │    │
│  │  - Billing aggregation                                               │    │
│  │  - Usage metering                                                    │    │
│  │  - Quota enforcement                                                 │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                              │                                               │
│         ┌────────────────────┼────────────────────┐                         │
│         │                    │                    │                         │
│         ▼                    ▼                    ▼                         │
│  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐                 │
│  │  acme.      │      │  startup.   │      │  agency.    │                 │
│  │  coder.run   │      │  coder.run   │      │  coder.run   │                 │
│  │             │      │             │      │             │                 │
│  │  Customer   │      │  Customer   │      │  Customer   │                 │
│  │  Durable    │      │  Durable    │      │  Durable    │                 │
│  │  Object     │      │  Object     │      │  Object     │                 │
│  │             │      │             │      │             │                 │
│  │  - Isolated │      │  - Isolated │      │  - Isolated │                 │
│  │  - Metered  │      │  - Metered  │      │  - Metered  │                 │
│  └─────────────┘      └─────────────┘      └─────────────┘                 │
│         │                    │                    │                         │
│         ▼                    ▼                    ▼                         │
│  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐                 │
│  │  R2:acme/   │      │  R2:startup/│      │  R2:agency/ │                 │
│  │  D1:acme    │      │  D1:startup │      │  D1:agency  │                 │
│  └─────────────┘      └─────────────┘      └─────────────┘                 │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Provisioning Flow

```rust
// crates/coder-infra/src/provisioning.rs

pub struct InfraProvisioner {
    cf_api: CloudflareApi,
    billing: BillingService,
}

impl InfraProvisioner {
    pub async fn provision_customer(
        &self,
        req: ProvisionRequest,
    ) -> Result<CustomerEnvironment, Error> {
        // 1. Validate customer and plan
        let customer = self.billing.validate_customer(&req.customer_id).await?;

        // 2. Create subdomain
        let subdomain = self.allocate_subdomain(&req.preferred_subdomain).await?;

        // 3. Create Durable Object namespace entry
        let do_id = self.cf_api.create_durable_object(
            "CustomerDO",
            &format!("customer:{}", req.customer_id),
        ).await?;

        // 4. Create R2 bucket (or use shared with prefix)
        let r2_prefix = format!("customers/{}/", req.customer_id);

        // 5. Create D1 database
        let d1_db = self.cf_api.create_d1_database(
            &format!("coder-customer-{}", req.customer_id),
        ).await?;

        // 6. Run migrations
        self.run_customer_migrations(&d1_db).await?;

        // 7. Configure DNS
        self.cf_api.create_dns_record(
            &subdomain,
            "CNAME",
            "coder-api.openagents.workers.dev",
        ).await?;

        // 8. Return environment details
        Ok(CustomerEnvironment {
            customer_id: req.customer_id,
            subdomain,
            do_id,
            r2_prefix,
            d1_database: d1_db.name,
            status: EnvironmentStatus::Active,
        })
    }
}
```

### 5.3 Usage Metering

```rust
// crates/coder-infra/src/metering.rs

#[derive(Serialize, Deserialize)]
pub struct UsageRecord {
    customer_id: String,
    timestamp: i64,
    metric: UsageMetric,
    value: i64,
}

#[derive(Serialize, Deserialize)]
pub enum UsageMetric {
    WorkerRequests,
    DurableObjectRequests,
    DurableObjectDuration { ms: i64 },
    R2ClassAOperations,
    R2ClassBOperations,
    R2Storage { bytes: i64 },
    D1Reads,
    D1Writes,
    D1Storage { bytes: i64 },
    AiTokens { model: String },
    BandwidthEgress { bytes: i64 },
}

impl UsageMeter {
    pub async fn record(&self, record: UsageRecord) -> Result<(), Error> {
        // Batch writes to D1 for efficiency
        self.buffer.push(record);

        if self.buffer.len() >= 100 || self.last_flush.elapsed() > Duration::from_secs(60) {
            self.flush().await?;
        }

        Ok(())
    }

    pub async fn calculate_bill(
        &self,
        customer_id: &str,
        period: BillingPeriod,
    ) -> Result<Bill, Error> {
        let usage = self.get_usage(customer_id, period).await?;

        let line_items = vec![
            LineItem {
                description: "Worker Requests".into(),
                quantity: usage.worker_requests,
                unit_price: 0.000002, // $2/million
                total: usage.worker_requests as f64 * 0.000002,
            },
            LineItem {
                description: "Durable Object Requests".into(),
                quantity: usage.do_requests,
                unit_price: 0.000005, // $5/million
                total: usage.do_requests as f64 * 0.000005,
            },
            LineItem {
                description: "R2 Storage (GB)".into(),
                quantity: usage.r2_storage_gb,
                unit_price: 0.05, // $0.05/GB
                total: usage.r2_storage_gb as f64 * 0.05,
            },
            LineItem {
                description: "AI Tokens (1K)".into(),
                quantity: usage.ai_tokens / 1000,
                unit_price: 0.10, // $0.10/1K tokens
                total: (usage.ai_tokens / 1000) as f64 * 0.10,
            },
        ];

        Ok(Bill {
            customer_id: customer_id.into(),
            period,
            line_items,
            total: line_items.iter().map(|i| i.total).sum(),
        })
    }
}
```

---

## 6. Deployment Architecture

### 6.1 CI/CD Pipeline

```yaml
# .github/workflows/deploy.yml

name: Deploy Coder Platform

on:
  push:
    branches: [main]
  workflow_dispatch:

jobs:
  build-web:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown

      - name: Build Coder Web (WASM)
        run: |
          cd crates/coder-web
          cargo build --release --target wasm32-unknown-unknown
          wasm-bindgen --out-dir pkg --target web \
            target/wasm32-unknown-unknown/release/coder_web.wasm

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: coder-web
          path: crates/coder-web/pkg/

  deploy-workers:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown

      - name: Install wrangler
        run: npm install -g wrangler

      - name: Build and deploy Workers
        run: |
          cd crates/cloudflare
          wrangler deploy
        env:
          CLOUDFLARE_API_TOKEN: ${{ secrets.CF_API_TOKEN }}

  deploy-web:
    needs: [build-web]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
        with:
          name: coder-web
          path: dist/

      - name: Deploy to Cloudflare Pages
        uses: cloudflare/pages-action@v1
        with:
          apiToken: ${{ secrets.CF_API_TOKEN }}
          accountId: ${{ secrets.CF_ACCOUNT_ID }}
          projectName: coder-web
          directory: dist/
```

### 6.2 Environment Configuration

```toml
# crates/cloudflare/wrangler.toml

name = "coder-api"
main = "build/worker/shim.mjs"
compatibility_date = "2024-12-01"

[build]
command = "cargo install -q worker-build && worker-build --release"

# Durable Objects
[[durable_objects.bindings]]
name = "RELAY"
class_name = "RelayDurableObject"

[[durable_objects.bindings]]
name = "PROJECT"
class_name = "ProjectDurableObject"

[[durable_objects.bindings]]
name = "CUSTOMER"
class_name = "CustomerDurableObject"

[[migrations]]
tag = "v1"
new_sqlite_classes = ["RelayDurableObject", "ProjectDurableObject", "CustomerDurableObject"]

# KV Namespaces
[[kv_namespaces]]
binding = "CONFIG"
id = "xxx"

[[kv_namespaces]]
binding = "SESSIONS"
id = "xxx"

# R2 Buckets
[[r2_buckets]]
binding = "ARTIFACTS"
bucket_name = "coder-artifacts"

# D1 Databases
[[d1_databases]]
binding = "DB"
database_name = "coder-main"
database_id = "xxx"

# AI
[ai]
binding = "AI"

# Environment variables
[vars]
ENVIRONMENT = "production"

# Routes
[[routes]]
pattern = "api.coder.run/*"
zone_name = "coder.run"

[[routes]]
pattern = "*.coder.run/*"
zone_name = "coder.run"
```

---

## 7. Security Architecture

### 7.1 Authentication Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    NOSTR AUTHENTICATION FLOW                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. Client generates Nostr keypair (or uses existing)                       │
│                                                                              │
│     Browser                     Coder API                                    │
│        │                            │                                        │
│        │  POST /auth/challenge      │                                        │
│        │  { npub: "npub1..." }      │                                        │
│        │ ─────────────────────────► │                                        │
│        │                            │                                        │
│        │  { challenge: "random",    │                                        │
│        │    expiresAt: 123... }     │                                        │
│        │ ◄───────────────────────── │                                        │
│        │                            │                                        │
│  2. Client signs challenge with nsec                                        │
│                                                                              │
│        │  POST /auth/verify         │                                        │
│        │  { npub: "npub1...",       │                                        │
│        │    signature: "sig..." }   │                                        │
│        │ ─────────────────────────► │                                        │
│        │                            │  Verify Schnorr signature              │
│        │                            │  Create/update user record             │
│        │  { token: "jwt...",        │  Issue JWT                             │
│        │    expiresAt: 123... }     │                                        │
│        │ ◄───────────────────────── │                                        │
│        │                            │                                        │
│  3. Client includes JWT in subsequent requests                              │
│                                                                              │
│        │  GET /api/projects         │                                        │
│        │  Authorization: Bearer jwt │                                        │
│        │ ─────────────────────────► │                                        │
│        │                            │  Verify JWT                            │
│        │  { projects: [...] }       │  Check permissions                     │
│        │ ◄───────────────────────── │                                        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Authorization Model

```rust
// crates/coder/src/auth/permissions.rs

#[derive(Debug, Clone)]
pub enum Permission {
    // Project permissions
    ProjectRead,
    ProjectWrite,
    ProjectDelete,
    ProjectShare,

    // Agent permissions
    AgentRun,
    AgentConfigure,

    // Billing permissions
    BillingRead,
    BillingManage,

    // Team permissions
    TeamManage,
    TeamInvite,

    // Admin permissions
    AdminAll,
}

pub struct AuthContext {
    pub user_npub: String,
    pub plan: Plan,
    pub team_id: Option<String>,
    pub permissions: HashSet<Permission>,
}

impl AuthContext {
    pub fn can(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission) ||
        self.permissions.contains(&Permission::AdminAll)
    }

    pub fn can_access_project(&self, project: &Project) -> bool {
        // Owner always has access
        if project.owner_npub == self.user_npub {
            return true;
        }

        // Team members have access to team projects
        if let Some(team_id) = &self.team_id {
            if project.team_id.as_ref() == Some(team_id) {
                return true;
            }
        }

        // Check explicit sharing
        project.shared_with.contains(&self.user_npub)
    }
}
```

### 7.3 Rate Limiting

```rust
// crates/coder/src/middleware/rate_limit.rs

pub struct RateLimiter {
    kv: KvNamespace,
}

impl RateLimiter {
    pub async fn check(
        &self,
        key: &str,
        limit: u32,
        window_secs: u32,
    ) -> Result<RateLimitResult, Error> {
        let window_key = format!("ratelimit:{}:{}", key, self.current_window(window_secs));

        let current: u32 = self.kv.get(&window_key)
            .await?
            .map(|v| v.parse().unwrap_or(0))
            .unwrap_or(0);

        if current >= limit {
            return Ok(RateLimitResult::Exceeded {
                limit,
                remaining: 0,
                reset_at: self.next_window(window_secs),
            });
        }

        // Increment counter
        self.kv.put(&window_key, (current + 1).to_string())
            .expiration_ttl(window_secs * 2)
            .execute()
            .await?;

        Ok(RateLimitResult::Allowed {
            limit,
            remaining: limit - current - 1,
            reset_at: self.next_window(window_secs),
        })
    }
}

// Rate limits by plan
pub fn limits_for_plan(plan: &Plan) -> RateLimits {
    match plan {
        Plan::Free => RateLimits {
            ai_prompts_per_day: 100,
            api_requests_per_minute: 60,
            agent_runs_per_day: 10,
        },
        Plan::Pro => RateLimits {
            ai_prompts_per_day: 1000,
            api_requests_per_minute: 300,
            agent_runs_per_day: 100,
        },
        Plan::Team | Plan::Business => RateLimits {
            ai_prompts_per_day: 10000,
            api_requests_per_minute: 1000,
            agent_runs_per_day: 1000,
        },
        Plan::Enterprise => RateLimits {
            ai_prompts_per_day: u32::MAX,
            api_requests_per_minute: u32::MAX,
            agent_runs_per_day: u32::MAX,
        },
    }
}
```

---

## 8. Implementation Priorities

### Phase 1: MVP (Weeks 1-4)

**Must Have:**
1. [ ] Coder Web running on `coder.run` (Dioxus WASM)
2. [ ] Nostr authentication working
3. [ ] Project CRUD operations
4. [ ] File editing with preview
5. [ ] AI chat with Claude integration
6. [ ] 3 launch templates (landing, dashboard, API)
7. [ ] Basic deployment to Cloudflare Pages

**Nice to Have:**
1. [ ] Multiple AI providers
2. [ ] Team collaboration
3. [ ] Custom domains

### Phase 2: Growth (Weeks 5-8)

**Must Have:**
1. [ ] Subscription billing (Stripe + Lightning)
2. [ ] Infrastructure resale MVP
3. [ ] Usage metering
4. [ ] Marketplace foundation
5. [ ] OANIX integration (real agents)

**Nice to Have:**
1. [ ] SSO integration
2. [ ] Advanced analytics
3. [ ] White-label option

### Phase 3: Scale (Weeks 9-12)

**Must Have:**
1. [ ] Enterprise tier features
2. [ ] SOC 2 compliance
3. [ ] Geographic expansion (EU)
4. [ ] Multi-region deployment
5. [ ] Full marketplace launch

**Nice to Have:**
1. [ ] HIPAA compliance
2. [ ] On-premise option
3. [ ] FedRAMP path

---

*Document Version: 1.0*
*Last Updated: December 2024*
