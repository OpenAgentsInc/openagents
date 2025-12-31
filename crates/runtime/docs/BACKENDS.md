# Runtime Backends

How the runtime abstraction maps to concrete deployment targets.

---

## Backend Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      Agent Application                          │
├─────────────────────────────────────────────────────────────────┤
│                      Runtime Core                               │
│  (Agent trait, Context, Triggers, Lifecycle)                    │
├─────────────────────────────────────────────────────────────────┤
│                      Backend Trait                              │
├──────────┬──────────┬──────────┬──────────┬────────────────────┤
│Cloudflare│  Local   │  Docker  │   K8s    │     Custom         │
│  Workers │  Device  │Container │  Pods    │    Backend         │
└──────────┴──────────┴──────────┴──────────┴────────────────────┘
```

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

## Docker Container Backend

Agents run in isolated containers, orchestrated by Docker Compose or Swarm.

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

## Kubernetes Backend

Agents run as StatefulSet pods with persistent volumes.

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

| Aspect | Cloudflare | Local | Docker | Kubernetes |
|--------|------------|-------|--------|------------|
| **Cold start** | 10-50ms | 100ms | 1-5s | 5-30s |
| **Warm latency** | <1ms | <1ms | <10ms | <10ms |
| **Max agents** | Millions | Hundreds | Hundreds | Thousands |
| **Ops burden** | Zero | Low | Medium | High |
| **Cost model** | Pay-per-use | Fixed | Fixed | Fixed |
| **Offline** | No | Yes | Yes | Yes |
| **Privacy** | Cloud | Full | Self-host | Self-host |
| **Multi-region** | Built-in | Manual | Manual | Federation |

---

## Backend Selection Guide

### Use Cloudflare When:
- Global distribution matters
- Zero ops is priority
- Pay-per-use is preferred
- Agents are mostly idle (hibernation is free)

### Use Local When:
- Privacy is critical
- Offline operation needed
- Single user / small scale
- Cost sensitive (free after hardware)

### Use Docker When:
- Self-hosting required
- Moderate scale
- Simple deployment
- Team already uses Docker

### Use Kubernetes When:
- Enterprise scale
- Multi-tenant
- Existing K8s infrastructure
- Need advanced orchestration

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

    #[cfg(feature = "kubernetes")]
    let backend = KubernetesBackend::new();

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
- **WASM in Browser** — Client-side agents
- **Raspberry Pi** — IoT edge agents
- **TEE/SGX** — Confidential computing agents
