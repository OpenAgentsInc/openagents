# Runtime Backends

How the runtime abstraction maps to concrete deployment targets.

---

## Backend vs Deployment Mode

**Important distinction:**

- **Backend** = A distinct `RuntimeBackend` implementation with different storage, wake, and hibernation semantics
- **Deployment Mode** = How you run a backend (Docker, Kubernetes, systemd, etc.)

There are **four backends**:

| Backend | Storage | Wake Mechanism | Hibernation |
|---------|---------|----------------|-------------|
| **Browser** | IndexedDB | postMessage | Worker termination |
| **Cloudflare** | DO SQLite | HTTP fetch | DO hibernation |
| **Local** | File SQLite | IPC/socket | Process suspend |
| **Server** | SQLite/Postgres | HTTP/gRPC | Process pool |

Docker and Kubernetes are **deployment modes** for the Server backend, not separate backends:

```
┌─────────────────────────────────────────────────────────────────┐
│                      Agent Application                          │
├─────────────────────────────────────────────────────────────────┤
│                      Runtime Core                               │
│  (Agent trait, Context, Triggers, Lifecycle)                    │
├─────────────────────────────────────────────────────────────────┤
│                      Backend Trait                              │
├──────────────┬──────────────┬──────────────┬───────────────────┤
│   Browser    │  Cloudflare  │    Local     │      Server       │
│    WASM      │   Workers    │    Device    │                   │
└──────────────┴──────────────┴──────────────┴───────────────────┘
                                                      │
                                    ┌─────────────────┼─────────────────┐
                                    │                 │                 │
                               Bare Metal         Docker          Kubernetes
                                 systemd        Compose              Pods
```

If you truly need "one agent = one container" or "one agent = one pod" (for isolation, GPU access, etc.), that's a **variant of the Server backend** with different process management, not a fundamentally different runtime abstraction.

---

## Browser WASM Backend

Agents run directly in the browser via WebAssembly. Zero cloud dependency—compute happens client-side.

This is a first-class deployment target, inspired by [WANIX](https://github.com/tractordev/wanix) (Plan 9 concepts in the browser) and [Apptron](https://github.com/progrium/apptron) (full Linux environment in browser).

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Browser Tab                               │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                    Runtime (WASM)                            ││
│  │  ┌─────────────────────────────────────────────────────┐    ││
│  │  │  Agent (WASI binary)                                │    ││
│  │  │  ┌─────────────────────────────────────────────┐    │    ││
│  │  │  │  Namespace (mounted capabilities)           │    │    ││
│  │  │  │  - IndexedDB storage                        │    │    ││
│  │  │  │  - Web Crypto for signing                   │    │    ││
│  │  │  │  - WebSocket for Nostr relays               │    │    ││
│  │  │  │  - DOM APIs via filesystem                  │    │    ││
│  │  │  └─────────────────────────────────────────────┘    │    ││
│  │  └─────────────────────────────────────────────────────┘    ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                  │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │   IndexedDB     │  │  OPFS Storage   │  │  Service Worker │  │
│  │   (state)       │  │  (files)        │  │  (offline)      │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Mapping

| Runtime Concept | Browser Implementation |
|-----------------|------------------------|
| Agent instance | WASI module in Web Worker |
| Agent ID | IndexedDB key |
| State storage | IndexedDB + OPFS |
| KV storage | IndexedDB |
| Wake trigger | postMessage to Worker |
| Hibernation | Terminate Worker, state in IndexedDB |
| Alarms | setTimeout / Service Worker |
| Connections | WebSocket (native) |
| Identity/Signing | Web Crypto API |

### Implementation Notes

```rust
// Compiled to WASM, runs in browser
pub struct BrowserBackend {
    // IndexedDB handle
    storage: IdbDatabase,
    // Worker pool for agent execution
    workers: WorkerPool,
    // WebSocket manager for Nostr
    ws_manager: WebSocketManager,
}

impl RuntimeBackend for BrowserBackend {
    async fn wake(&self, id: &AgentId, trigger: Trigger) -> Result<TickResult> {
        // Spawn or reuse Web Worker
        let worker = self.workers.get_or_create(id).await?;

        // Send trigger via postMessage
        worker.post_message(&JsValue::from_serde(&trigger)?);

        // Wait for result
        let result = worker.wait_for_result().await?;
        Ok(result)
    }

    async fn hibernate(&self, id: &AgentId) -> Result<()> {
        // Terminate worker, state already in IndexedDB
        self.workers.terminate(id).await?;
        Ok(())
    }
}
```

### WASI Compatibility (Key Insight from WANIX)

WANIX demonstrated that WASI binaries run seamlessly in browser:

```javascript
// From WANIX: creating a WASI task in browser
const tid = (await wanix.readText("task/new/wasi")).trim();
await wanix.writeFile(`task/${tid}/cmd`, `#bundle/agent.wasm`);
await wanix.writeFile(`task/${tid}/ctl`, "start");

// Agent executes in browser, same binary as server
```

This means **same agent binary** runs:
- On server (native)
- On desktop (native)
- In browser (WASM)

### DOM/Browser APIs via Filesystem

Following WANIX pattern, browser capabilities mount as files:

```
/cap/
├── fetch/          # HTTP requests
├── ws/             # WebSocket connections
├── crypto/         # Web Crypto API
├── storage/
│   ├── indexeddb/  # IndexedDB access
│   └── opfs/       # Origin Private File System
├── dom/            # DOM manipulation (careful!)
└── console/        # Browser console
```

### Characteristics

- **Latency:** <10ms (everything local)
- **Scale:** Single agent (or few via Web Workers)
- **Cost:** Zero (user's browser)
- **Limits:** Browser memory/storage quotas
- **Offline:** Yes (with Service Worker)
- **Privacy:** Maximum (data never leaves device)

### When to Use Browser Backend

- **Privacy-first agents** — Data stays on user's device
- **Offline capability** — Works without network
- **Zero-cost** — No server infrastructure
- **Instant startup** — No network round-trip
- **Demos/sandboxes** — Try agents without backend
- **Edge compute** — Distribute compute to clients

### Virtual Network (From Apptron)

Apptron shows browser agents can have network presence:

```
Browser Agent → WebSocket tunnel → Public HTTPS endpoint
                                 (like ngrok but browser-native)
```

This enables:
- Agent exposing HTTP API from browser
- Cross-browser agent communication
- Bridging to external services

---

## Cloudflare Workers Backend

Agents run as Durable Objects on Cloudflare's edge network.

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                 Cloudflare Edge                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │              Worker (HTTP Router)                │   │
│  │  Routes requests to appropriate Durable Object   │   │
│  └─────────────────────┬───────────────────────────┘   │
│                        │                               │
│  ┌─────────────────────▼───────────────────────────┐   │
│  │           Durable Object (Agent)                 │   │
│  │  ┌─────────────────────────────────────────┐    │   │
│  │  │  SQLite Storage (state, KV)             │    │   │
│  │  │  WebSocket Connections (clients)        │    │   │
│  │  │  Alarm API (scheduled wake)             │    │   │
│  │  └─────────────────────────────────────────┘    │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### Mapping

| Runtime Concept | Cloudflare Implementation |
|-----------------|---------------------------|
| Agent instance | Durable Object |
| Agent ID | DO ID from name |
| State storage | DO SQLite API |
| KV storage | DO SQLite or KV namespace |
| Wake trigger | HTTP fetch to DO |
| Hibernation | WebSocket hibernation API |
| Alarms | DO Alarm API |
| Connections | DO WebSocket API |

### Implementation Notes

```rust
pub struct CloudflareBackend {
    // DO namespace binding
    agent_do: DurableObjectNamespace,
    // KV for metadata
    kv: KvNamespace,
}

impl RuntimeBackend for CloudflareBackend {
    async fn wake(&self, id: &AgentId, trigger: Trigger) -> Result<TickResult> {
        // Get DO stub
        let do_id = self.agent_do.id_from_name(&id.to_string())?;
        let stub = do_id.get_stub()?;

        // Send trigger via HTTP
        let req = Request::new_with_init(
            "http://internal/trigger",
            RequestInit::new()
                .with_method(Method::Post)
                .with_body(Some(serde_json::to_string(&trigger)?.into())),
        )?;

        let resp = stub.fetch_with_request(req).await?;
        let result: TickResult = resp.json().await?;
        Ok(result)
    }
}
```

### Characteristics

- **Latency:** ~10-50ms cold start, <1ms warm
- **Scale:** Millions of agents, auto-scaled
- **Cost:** Pay per request + duration + storage
- **Limits:** 30s CPU time, 128MB memory, 1GB storage per DO
- **Regions:** Global edge, single-region consistency

---

## Local Device Backend

Agents run as a daemon on user's machine (laptop, desktop, server).

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   User Device                           │
│  ┌─────────────────────────────────────────────────┐   │
│  │              Agent Daemon                        │   │
│  │  ┌─────────────────────────────────────────┐    │   │
│  │  │  Agent Registry (HashMap<AgentId, Agent>) │    │   │
│  │  │  Event Loop (tokio runtime)              │    │   │
│  │  │  IPC Server (Unix socket / named pipe)   │    │   │
│  │  └─────────────────────────────────────────┘    │   │
│  └─────────────────────┬───────────────────────────┘   │
│                        │                               │
│  ┌─────────────────────▼───────────────────────────┐   │
│  │           SQLite Database                        │   │
│  │  ~/.openagents/agents.db                        │   │
│  │  - agent_state table                            │   │
│  │  - agent_kv table                               │   │
│  │  - agent_alarms table                           │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### Mapping

| Runtime Concept | Local Implementation |
|-----------------|----------------------|
| Agent instance | Struct in memory |
| Agent ID | UUID |
| State storage | SQLite file |
| KV storage | SQLite table |
| Wake trigger | Channel message |
| Hibernation | Serialize to disk, drop from memory |
| Alarms | Tokio timer + persist to SQLite |
| Connections | WebSocket server |

### Implementation Notes

```rust
pub struct LocalBackend {
    // Active agents in memory
    agents: DashMap<AgentId, AgentHandle>,
    // SQLite connection pool
    db: SqlitePool,
    // Event loop handle
    runtime: Handle,
    // Alarm scheduler
    scheduler: AlarmScheduler,
}

impl RuntimeBackend for LocalBackend {
    async fn wake(&self, id: &AgentId, trigger: Trigger) -> Result<TickResult> {
        // Get or create agent handle
        let handle = self.get_or_load(id).await?;

        // Send trigger through channel
        handle.trigger_tx.send(trigger).await?;

        // Wait for result
        let result = handle.result_rx.recv().await?;
        Ok(result)
    }

    async fn hibernate(&self, id: &AgentId) -> Result<()> {
        if let Some((_, handle)) = self.agents.remove(id) {
            // Serialize state to disk
            let state = handle.get_state();
            self.db.save_state(id, &state).await?;
            // Drop handle, freeing memory
        }
        Ok(())
    }
}
```

### Characteristics

- **Latency:** <1ms warm, ~100ms cold (disk load)
- **Scale:** Dozens to hundreds of agents (memory limited)
- **Cost:** Free (user's hardware)
- **Limits:** Device resources (RAM, CPU, disk)
- **Regions:** Single device, can sync to cloud

---

## Server Deployment Mode: Docker

The Server backend can be deployed using Docker containers, orchestrated by Docker Compose or Swarm.

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Docker Host                           │
│  ┌─────────────────────────────────────────────────┐   │
│  │              Gateway Container                   │   │
│  │  (Routes requests to agent containers)          │   │
│  └─────────────────────┬───────────────────────────┘   │
│           ┌────────────┼────────────┐                  │
│           ▼            ▼            ▼                  │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐      │
│  │  Agent A    │ │  Agent B    │ │  Agent C    │      │
│  │  Container  │ │  Container  │ │  Container  │      │
│  │ ┌─────────┐ │ │ ┌─────────┐ │ │ ┌─────────┐ │      │
│  │ │ SQLite  │ │ │ │ SQLite  │ │ │ │ SQLite  │ │      │
│  │ │ Volume  │ │ │ │ Volume  │ │ │ │ Volume  │ │      │
│  │ └─────────┘ │ │ └─────────┘ │ │ └─────────┘ │      │
│  └─────────────┘ └─────────────┘ └─────────────┘      │
│                                                        │
│  ┌─────────────────────────────────────────────────┐   │
│  │              Shared PostgreSQL                   │   │
│  │  (Optional: for cross-agent queries)            │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### Mapping

| Runtime Concept | Docker Implementation |
|-----------------|----------------------|
| Agent instance | Container |
| Agent ID | Container name |
| State storage | Volume-mounted SQLite |
| KV storage | SQLite or Redis |
| Wake trigger | HTTP to container |
| Hibernation | Stop container |
| Alarms | Cron container or internal |
| Connections | Exposed port per container |

### Characteristics

- **Latency:** ~1-5s cold (container start), <10ms warm
- **Scale:** Hundreds of agents (host limited)
- **Cost:** Host infrastructure
- **Limits:** Configurable per container
- **Regions:** Single host or Swarm cluster

---

## Server Deployment Mode: Kubernetes

The Server backend can be deployed on Kubernetes, with agents as StatefulSet pods with persistent volumes.

**Critical:** To maintain the "no parallel ticks ever" guarantee in a multi-node cluster, agents must be pinned/sharded to a single worker (consistent hash by AgentId) OR guarded by a distributed lock keyed by AgentId.

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                 Kubernetes Cluster                      │
│  ┌─────────────────────────────────────────────────┐   │
│  │              Ingress Controller                  │   │
│  │  (Routes to agent services)                     │   │
│  └─────────────────────┬───────────────────────────┘   │
│                        │                               │
│  ┌─────────────────────▼───────────────────────────┐   │
│  │           Agent Operator (CRD)                   │   │
│  │  - Watches Agent custom resources               │   │
│  │  - Manages StatefulSets                         │   │
│  │  - Handles scaling/hibernation                  │   │
│  └─────────────────────────────────────────────────┘   │
│                                                        │
│  ┌────────────────┐  ┌────────────────┐               │
│  │ StatefulSet A  │  │ StatefulSet B  │               │
│  │ ┌────────────┐ │  │ ┌────────────┐ │               │
│  │ │  Pod       │ │  │ │  Pod       │ │               │
│  │ │  (Agent)   │ │  │ │  (Agent)   │ │               │
│  │ └────────────┘ │  │ └────────────┘ │               │
│  │ ┌────────────┐ │  │ ┌────────────┐ │               │
│  │ │    PVC     │ │  │ │    PVC     │ │               │
│  │ │  (State)   │ │  │ │  (State)   │ │               │
│  │ └────────────┘ │  │ └────────────┘ │               │
│  └────────────────┘  └────────────────┘               │
│                                                        │
│  ┌─────────────────────────────────────────────────┐   │
│  │              CockroachDB / PostgreSQL            │   │
│  │  (Shared state for coordination)                │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### Mapping

| Runtime Concept | Kubernetes Implementation |
|-----------------|--------------------------|
| Agent instance | Pod in StatefulSet |
| Agent ID | StatefulSet name |
| State storage | PersistentVolumeClaim |
| KV storage | PVC SQLite or CockroachDB |
| Wake trigger | HTTP/gRPC to Service |
| Hibernation | Scale StatefulSet to 0 |
| Alarms | CronJob or internal timer |
| Connections | Service + Ingress |

### Custom Resource Definition

```yaml
apiVersion: openagents.com/v1
kind: Agent
metadata:
  name: my-agent
spec:
  image: openagents/agent-runtime:latest
  config:
    maxTickDuration: 30s
    maxStorageBytes: 1Gi
  resources:
    requests:
      memory: "256Mi"
      cpu: "100m"
    limits:
      memory: "1Gi"
      cpu: "1000m"
  storage:
    size: 10Gi
    storageClass: fast-ssd
```

### Characteristics

- **Latency:** ~5-30s cold (pod scheduling), <10ms warm
- **Scale:** Thousands of agents (cluster capacity)
- **Cost:** Cluster infrastructure
- **Limits:** Pod resource limits
- **Regions:** Multi-region with federation

---

## Comparison Matrix

### By Backend (Runtime Abstraction)

| Aspect | Browser | Cloudflare | Local | Server |
|--------|---------|------------|-------|--------|
| **Cold start** | <10ms | 10-50ms | 100ms | 100ms-5s* |
| **Warm latency** | <1ms | <1ms | <1ms | <10ms |
| **Max agents** | Single | Millions | Hundreds | Thousands |
| **Ops burden** | Zero | Zero | Low | Medium-High |
| **Cost model** | Free | Pay-per-use | Fixed | Fixed |
| **Offline** | Yes | No | Yes | Yes |
| **Privacy** | Maximum | Cloud | Full | Self-host |

*Server cold start depends on deployment mode (bare metal vs Docker vs K8s)

### Server Deployment Modes

| Aspect | Bare Metal | Docker | Kubernetes |
|--------|------------|--------|------------|
| **Cold start** | 100ms | 1-5s | 5-30s |
| **Ops burden** | Medium | Medium | High |
| **Scale** | Single node | Single node | Multi-node |
| **Isolation** | Process | Container | Pod |
| **Multi-region** | Manual | Manual | Federation |

---

## Backend Selection Guide

### Use Browser When:
- Privacy is paramount (data never leaves device)
- Offline operation essential
- Zero infrastructure cost required
- Building demos or sandboxes
- Distributing compute to clients
- Agent serves single user

### Use Cloudflare When:
- Global distribution matters
- Zero ops is priority
- Pay-per-use is preferred
- Agents are mostly idle (hibernation is free)

### Use Local When:
- Privacy is critical (data never leaves device)
- Offline operation needed
- Development and testing
- Single-user scenarios

### Use Server When:
- Full control over infrastructure
- Multi-agent systems with high throughput
- Custom integrations (GPUs, databases, etc.)
- Compliance requirements (data residency)

**Server deployment mode selection:**
- **Bare metal** — Maximum performance, minimal overhead
- **Docker** — Easy deployment, good isolation, single-node
- **Kubernetes** — Multi-node scaling, auto-healing, but high ops burden

---

## Backend Portability

The same agent code runs on any backend:

```rust
// Define agent once
pub struct MyAgent;

impl Agent for MyAgent {
    type State = MyState;
    type Config = MyConfig;

    fn on_trigger(&self, ctx: &mut AgentContext<MyState>, trigger: Trigger) -> Result<TickResult> {
        // This code runs identically on all backends
        match trigger {
            Trigger::Message(msg) => self.handle_message(ctx, msg),
            Trigger::Alarm(alarm) => self.handle_alarm(ctx, alarm),
            _ => Ok(TickResult::default()),
        }
    }
}

// Deploy to any backend
fn main() {
    // Choose backend at deployment time
    #[cfg(feature = "cloudflare")]
    let backend = CloudflareBackend::new();

    #[cfg(feature = "local")]
    let backend = LocalBackend::new("~/.openagents");

    #[cfg(feature = "server")]
    let backend = ServerBackend::new();  // Deployment mode (Docker/K8s) configured separately

    // Same agent, any backend
    backend.register::<MyAgent>("my-agent");
    backend.run();
}
```

---

## Future Backends

Potential future backend implementations:

- **AWS Lambda + DynamoDB** — Serverless on AWS
- **Fly.io Machines** — Edge containers
- **Raspberry Pi** — IoT edge agents
- **TEE/SGX** — Confidential computing agents
- **React Native** — Mobile app agents
