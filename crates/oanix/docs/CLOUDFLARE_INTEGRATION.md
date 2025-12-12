# OANIX Cloudflare Integration

This document explores potential future integration between OANIX and Cloudflare's edge computing platform. It covers both strategic vision and technical implementation details.

---

## Executive Summary

OANIX's Plan 9-inspired architecture—where everything is a file and capabilities are composed via namespaces—maps naturally onto Cloudflare's edge infrastructure. Durable Objects provide stateful compute, Workers provide serverless execution, and storage services (R2, KV, D1) provide persistence. Together, they enable a **globally distributed agent operating system**.

**Key insight:** The `FileService` trait can be implemented by Cloudflare Durable Objects, allowing OANIX namespaces to span browser, edge, and server seamlessly.

---

## Part 1: Strategic Vision

### The Vision: Global Agent OS

Imagine an OANIX environment that:

1. **Runs anywhere**: Same namespace abstraction works in browser, at the edge, and on dedicated servers
2. **Scales globally**: Agent workloads automatically distributed to edge locations nearest users
3. **Persists state**: Agent sessions survive process restarts via Durable Object storage
4. **Connects everything**: Nostr relay at the edge enables global agent-to-agent communication

```
                        ┌─────────────────────────────────────┐
                        │         Global Agent Network        │
                        │                                     │
    ┌─────────┐         │   ┌─────────┐     ┌─────────┐      │         ┌─────────┐
    │ Browser │◄───────►│   │ CF Edge │◄───►│ CF Edge │      │◄───────►│ Server  │
    │ OANIX   │         │   │ (NYC)   │     │ (LON)   │      │         │ OANIX   │
    └─────────┘         │   └────┬────┘     └────┬────┘      │         └─────────┘
                        │        │               │           │
                        │        ▼               ▼           │
                        │   ┌─────────────────────────┐      │
                        │   │  Durable Objects        │      │
                        │   │  (Agent State)          │      │
                        │   └─────────────────────────┘      │
                        │        │               │           │
                        │        ▼               ▼           │
                        │   ┌─────────┐     ┌─────────┐      │
                        │   │   R2    │     │   D1    │      │
                        │   │ (Files) │     │ (Events)│      │
                        │   └─────────┘     └─────────┘      │
                        └─────────────────────────────────────┘
```

### Why Cloudflare for OANIX?

**1. Edge Computing Alignment**

OANIX's WASI-first execution model targets WebAssembly. Cloudflare Workers run WASM natively at 300+ edge locations. This means:
- Sub-50ms latency to most users globally
- Same WASM binary runs browser → edge → server
- No cold start penalty for OANIX agents

**2. Durable Objects = Stateful Services**

OANIX's `FileService` trait requires stateful operations (open file handles, seek positions, write buffers). Cloudflare Durable Objects provide:
- Single-threaded, consistent state per object
- Transactional storage API
- WebSocket hibernation for long-lived connections
- Automatic location affinity (DO migrates to where it's accessed)

**3. Global Distribution**

OANIX namespaces compose services. With Cloudflare:
- Different FileServices can run at different edge locations
- Namespace resolution can be globally distributed
- Agents can collaborate across regions with minimal latency

---

## Part 2: Architecture

### Three-Tier Model

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              OANIX Namespace                            │
│  /task  /workspace  /logs  /cap/http  /cap/ws  /cap/nostr              │
└─────────────────────────────────────────────────────────────────────────┘
        │              │       │           │         │           │
        ▼              ▼       ▼           ▼         ▼           ▼
┌─────────────┐  ┌──────────────────────────────────────────────────────┐
│   Browser   │  │                 Cloudflare Edge                      │
│   ─────────────│                                                       │
│   In-memory │  │  ┌──────────────┐  ┌──────────────┐  ┌────────────┐ │
│   TaskFs    │  │  │ WorkspaceFs  │  │   HttpFs     │  │  NostrFs   │ │
│   LogsFs    │  │  │ (R2 Durable  │  │  (Worker     │  │  (Relay    │ │
│             │  │  │   Object)    │  │   Proxy)     │  │  D.O.)     │ │
│             │  │  └──────────────┘  └──────────────┘  └────────────┘ │
└─────────────┘  └──────────────────────────────────────────────────────┘
                                      │
                                      ▼
                      ┌───────────────────────────────┐
                      │         Origin Server         │
                      │  ─────────────────────────────│
                      │  Heavy compute (WASI runtime) │
                      │  Large file storage           │
                      │  Database connections         │
                      └───────────────────────────────┘
```

**Browser tier:** Fast, local operations (task spec, logs, temp files)
**Edge tier:** Stateful services, caching, real-time communication
**Server tier:** Heavy WASI workloads, persistent storage, external integrations

### Edge-Native Namespaces

A namespace could span all three tiers:

```rust
let namespace = Namespace::builder()
    // Local (browser/native)
    .mount("/task", TaskFs::new(spec))
    .mount("/logs", MemFs::new())

    // Edge (Cloudflare Durable Objects)
    .mount("/workspace", EdgeWorkspaceFs::new("do-workspace-123"))
    .mount("/cap/ws", EdgeWsFs::new("do-websocket-456"))

    // Relay (Cloudflare-hosted Nostr relay)
    .mount("/cap/nostr", CloudflareNostrFs::new("wss://relay.openagents.com"))

    .build();
```

### FileService → Worker Mapping

| OANIX Service | Cloudflare Primitive | Notes |
|---------------|---------------------|-------|
| `MemFs` | Durable Object storage | In-memory → DO transactional storage |
| `WorkspaceFs` | R2 + Durable Object | Large files in R2, metadata in DO |
| `HttpFs` | Worker + fetch | Edge caching, geographic routing |
| `WsFs` | Durable Object + hibernation | Long-lived connections |
| `NostrFs` | `crates/cloudflare/` relay | Already implemented! |
| `TaskFs` | KV + Durable Object | Fast reads via KV, writes to DO |
| `LogsFs` | Logpush + R2 | Structured logs to R2, Logpush for streaming |

### Capability Distribution

The namespace model enables fine-grained capability distribution:

```
Agent A (Untrusted)              Agent B (Trusted)
─────────────────────            ─────────────────────
/task     → TaskFs (RO)          /task     → TaskFs (RW)
/workspace→ CowFs(MapFs)         /workspace→ WorkspaceFs (RW)
/logs     → MemFs                /logs     → LogsFs (streaming)
/cap/http → (not mounted)        /cap/http → HttpFs (full)
/cap/nostr→ NostrFs (RO)         /cap/nostr→ NostrFs (RW)
```

At the edge, capabilities translate to Worker bindings:
- `env.R2` → workspace file access
- `env.KV` → task metadata
- `env.DO` → stateful services
- `env.RELAY` → Nostr relay Durable Object

---

## Part 3: Technical Integration Points

### Durable Objects as FileServices

The core integration: implement `FileService` backed by a Durable Object.

**Conceptual bridge:**

```rust
// OANIX side: FileService trait
pub trait FileService: Send + Sync {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError>;
    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError>;
    fn stat(&self, path: &str) -> Result<Metadata, FsError>;
    // ...
}

// Cloudflare side: Durable Object
#[durable_object]
pub struct FileServiceDO {
    state: State,
    // In-memory cache of file tree
    files: RefCell<HashMap<String, FileNode>>,
}

impl DurableObject for FileServiceDO {
    // HTTP API maps to FileService operations
    async fn fetch(&mut self, req: Request) -> Result<Response> {
        match (req.method(), req.path().as_str()) {
            (Method::Get, path) if path.starts_with("/read/") => {
                self.handle_read(&path[6..]).await
            }
            (Method::Put, path) if path.starts_with("/write/") => {
                self.handle_write(&path[7..], req.bytes().await?).await
            }
            (Method::Get, "/readdir") => {
                let path = req.url()?.query_pairs().find(|(k, _)| k == "path");
                self.handle_readdir(&path.unwrap().1).await
            }
            // ...
        }
    }
}
```

**Client-side FileService implementation:**

```rust
pub struct EdgeFs {
    do_stub: Fetcher,  // Cloudflare Worker binding to DO
    do_id: String,
}

impl FileService for EdgeFs {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError> {
        // Returns a handle that makes HTTP requests to the DO
        Ok(Box::new(EdgeFileHandle {
            fetcher: self.do_stub.clone(),
            path: path.to_string(),
            position: 0,
            flags,
        }))
    }

    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        // Blocking HTTP call to DO
        let url = format!("/readdir?path={}", urlencoding::encode(path));
        let resp = block_on(self.do_stub.fetch(url))?;
        let entries: Vec<DirEntry> = resp.json()?;
        Ok(entries)
    }
}
```

### WebSocket Hibernation + WsFs

The existing `crates/cloudflare/` relay uses hibernation for efficient WebSocket handling. This pattern extends naturally to `WsFs`:

**Current pattern (relay_do.rs):**

```rust
impl DurableObject for RelayDurableObject {
    async fn fetch(&mut self, req: Request) -> Result<Response> {
        if req.headers().get("Upgrade")? == Some("websocket") {
            let pair = WebSocketPair::new()?;
            let server = pair.1;

            // Accept with hibernation - connection survives DO suspend
            self.state.accept_web_socket(&server);

            // Store for later broadcast
            self.websockets.borrow_mut().push(server);

            return Response::from_websocket(pair.0);
        }
        // ...
    }

    // Called when message arrives (even after hibernation)
    async fn websocket_message(&mut self, ws: WebSocket, msg: WebSocketIncomingMessage) {
        // Process message, broadcast to subscribers
    }

    // Called on disconnect
    async fn websocket_close(&mut self, ws: WebSocket, code: u16, reason: String, was_clean: bool) {
        // Cleanup subscription state
    }
}
```

**Integration with WsFs:**

```rust
// WsFs at the edge wraps a Durable Object
pub struct EdgeWsFs {
    do_stub: Fetcher,
}

impl FileService for EdgeWsFs {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError> {
        match path {
            "control" => {
                // Control file for connection management
                Ok(Box::new(WsControlHandle::new(self.do_stub.clone())))
            }
            path if path.starts_with("conns/") => {
                // Per-connection read/write
                let conn_id = &path[6..];
                Ok(Box::new(WsConnHandle::new(self.do_stub.clone(), conn_id)))
            }
            _ => Err(FsError::NotFound),
        }
    }
}

// Writing to control file opens connection
// Writing to conns/{id}/out sends message
// Reading from conns/{id}/in receives messages
```

**Benefits of hibernation:**
- WebSocket connections persist across DO suspend/resume
- Cost-efficient: only charged when processing messages
- Long-lived agent sessions without timeout concerns
- Automatic reconnection handling

### Storage Layer Integration

**R2 for WorkspaceFs (large files):**

```rust
pub struct R2WorkspaceFs {
    bucket: R2Bucket,
    prefix: String,
}

impl FileService for R2WorkspaceFs {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError> {
        let key = format!("{}/{}", self.prefix, path);

        if flags.contains(OpenFlags::WRITE) {
            // R2 multipart upload for large files
            Ok(Box::new(R2WriteHandle::new(self.bucket.clone(), key)))
        } else {
            // R2 range requests for seeking
            Ok(Box::new(R2ReadHandle::new(self.bucket.clone(), key)))
        }
    }

    fn stat(&self, path: &str) -> Result<Metadata, FsError> {
        let key = format!("{}/{}", self.prefix, path);
        let head = block_on(self.bucket.head(&key))?
            .ok_or(FsError::NotFound)?;

        Ok(Metadata {
            size: head.size(),
            modified: head.uploaded(),
            is_dir: false,
        })
    }
}
```

**D1/SQLite for MemFs persistence:**

```rust
pub struct D1MemFs {
    db: D1Database,
    namespace_id: String,
}

impl D1MemFs {
    async fn sync_to_d1(&self, tree: &MemNode) -> Result<()> {
        // Serialize file tree to JSON
        let data = serde_json::to_string(tree)?;

        self.db.prepare(
            "INSERT OR REPLACE INTO namespaces (id, data, updated_at) VALUES (?, ?, ?)"
        )
        .bind(&[&self.namespace_id, &data, &Utc::now().timestamp()])?
        .run()
        .await?;

        Ok(())
    }

    async fn restore_from_d1(&self) -> Result<MemNode> {
        let row = self.db.prepare(
            "SELECT data FROM namespaces WHERE id = ?"
        )
        .bind(&[&self.namespace_id])?
        .first::<String>(None)
        .await?;

        match row {
            Some(data) => Ok(serde_json::from_str(&data)?),
            None => Ok(MemNode::directory()),
        }
    }
}
```

**KV for namespace metadata:**

```rust
// Fast namespace resolution at the edge
pub struct NamespaceRegistry {
    kv: KvNamespace,
}

impl NamespaceRegistry {
    async fn resolve(&self, agent_id: &str) -> Result<NamespaceConfig> {
        // Sub-millisecond reads from KV
        let config = self.kv.get(agent_id).json().await?
            .ok_or(FsError::NotFound)?;
        Ok(config)
    }

    async fn register(&self, agent_id: &str, config: &NamespaceConfig) -> Result<()> {
        self.kv.put(agent_id, serde_json::to_string(config)?)?
            .expiration_ttl(86400)  // 24h TTL
            .execute()
            .await?;
        Ok(())
    }
}
```

### NostrFs + Cloudflare Relay

The existing `crates/cloudflare/` relay implements NIP-01 and NIP-90. Integration with `NostrFs`:

**Current relay capabilities (relay_do.rs):**
- Event storage (in-memory, planned SQLite via D1)
- Subscription management with filters
- WebSocket hibernation
- NIP-90 DVM job request/result kinds

**NostrFs integration:**

```rust
// NostrFs can connect to Cloudflare-hosted relay
let nostr_fs = NostrFs::builder()
    .relay_url("wss://relay.openagents.com")  // Cloudflare Worker
    .keypair(agent_keypair)
    .build();

// Mount in namespace
let namespace = Namespace::builder()
    .mount("/cap/nostr", nostr_fs)
    .build();

// Agent workflow:
// 1. Write job request to /cap/nostr/outbox/request.json
// 2. NostrFs signs and publishes to relay
// 3. Another agent (or DVM) processes job
// 4. Result appears in /cap/nostr/inbox/
```

**NIP-90 job routing at the edge:**

```rust
// Worker routes job requests to appropriate DVMs
impl RelayDurableObject {
    fn process_event(&mut self, event: Event) {
        // Detect job request (kinds 5000-5999)
        if event.kind >= 5000 && event.kind < 6000 {
            // Route to registered DVM for this job kind
            let dvm_id = self.dvm_registry.get(&event.kind);
            if let Some(dvm) = dvm_id {
                // Forward to DVM Durable Object
                self.route_to_dvm(dvm, &event);
            }
        }

        // Store event and broadcast to subscribers
        self.store_event(&event);
        self.broadcast_to_matching(&event);
    }
}
```

**Agent marketplace architecture:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    Cloudflare Edge                              │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Nostr Relay (Durable Object)                │  │
│  │  ┌──────────────────┐  ┌──────────────────────────────┐  │  │
│  │  │ Event Storage    │  │ Subscription Manager         │  │  │
│  │  │ (D1 SQLite)      │  │ (filter matching)            │  │  │
│  │  └──────────────────┘  └──────────────────────────────┘  │  │
│  │                                                          │  │
│  │  ┌──────────────────┐  ┌──────────────────────────────┐  │  │
│  │  │ DVM Registry     │  │ Job Router                   │  │  │
│  │  │ (kind → DVM)     │  │ (request → DVM DO)           │  │  │
│  │  └──────────────────┘  └──────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────┘  │
│                              │                                  │
│         ┌────────────────────┼────────────────────┐            │
│         ▼                    ▼                    ▼            │
│  ┌────────────┐       ┌────────────┐       ┌────────────┐      │
│  │ DVM: Code  │       │ DVM: Image │       │ DVM: Chat  │      │
│  │ Gen (5050) │       │ Gen (5100) │       │ (5050)     │      │
│  └────────────┘       └────────────┘       └────────────┘      │
└─────────────────────────────────────────────────────────────────┘
```

### HTTP Edge Proxy Pattern

HttpFs requests can be proxied through Workers for caching and optimization:

```rust
// Worker proxies HttpFs requests
export default {
    async fetch(request: Request, env: Env): Promise<Response> {
        const url = new URL(request.url);

        if (url.pathname.startsWith('/proxy/')) {
            const targetUrl = url.pathname.slice(7);  // Remove /proxy/

            // Check cache first
            const cacheKey = new Request(targetUrl, request);
            const cache = caches.default;
            let response = await cache.match(cacheKey);

            if (!response) {
                // Fetch from origin
                response = await fetch(targetUrl, {
                    method: request.method,
                    headers: request.headers,
                    body: request.body,
                });

                // Cache if cacheable
                if (response.ok && isCacheable(response)) {
                    const cloned = response.clone();
                    cloned.headers.set('Cache-Control', 'max-age=3600');
                    await cache.put(cacheKey, cloned);
                }
            }

            return response;
        }

        return new Response('Not found', { status: 404 });
    }
};
```

**OANIX side:**

```rust
pub struct EdgeHttpFs {
    worker_url: String,  // https://proxy.openagents.workers.dev
}

impl HttpFs for EdgeHttpFs {
    fn execute_request(&self, req: HttpRequest) -> Result<HttpResponse, FsError> {
        // Route through edge Worker
        let proxy_url = format!("{}/proxy/{}", self.worker_url, req.url);
        let proxy_req = HttpRequest {
            url: proxy_url,
            ..req
        };

        // Executor handles actual HTTP call
        self.executor.execute(proxy_req)
    }
}
```

---

## Part 4: Implementation Roadmap

### Phase 1: Edge Proxy (HttpFs)

**Goal:** Route OANIX HTTP requests through Cloudflare for caching and optimization.

**Tasks:**
1. Deploy Worker that proxies HttpFs requests
2. Implement `EdgeHttpFs` that routes to Worker
3. Add caching headers and cache API usage
4. Test with existing HttpExecutor

**Effort:** Low complexity, high immediate value.

### Phase 2: Durable FileServices

**Goal:** Run MemFs-equivalent at the edge with persistent storage.

**Tasks:**
1. Create `FileServiceDO` Durable Object implementing file operations
2. Implement HTTP API mapping FileService methods
3. Add D1 persistence for crash recovery
4. Create `EdgeFs` client-side FileService implementation
5. Test cross-platform (browser → edge → browser)

**Effort:** Medium complexity, foundational for later phases.

### Phase 3: Namespace Federation

**Goal:** Compose namespaces across browser, edge, and server.

**Tasks:**
1. Implement `FederatedNamespace` that routes by path prefix
2. Create namespace registry in KV
3. Add transparent path resolution
4. Handle connection failures gracefully
5. Test full three-tier namespace

**Effort:** Medium-high complexity, requires careful error handling.

### Phase 4: Full Edge OANIX

**Goal:** Run WASI workloads at the edge.

**Tasks:**
1. Compile OANIX WASI runtime for Workers (or use Cloudflare's)
2. Port `OanixEnv` to edge deployment
3. Implement job scheduling across edge locations
4. Add observability (Logpush, Analytics Engine)
5. Production deployment with monitoring

**Effort:** High complexity, requires significant engineering.

---

## Part 5: Trade-offs & Considerations

### Open Questions

**1. Synchronous vs Asynchronous FileService**

OANIX's `FileService` is synchronous (for WASI compatibility), but Cloudflare Workers are async. Options:
- Block on async (current approach) - works but inefficient
- Add `AsyncFileService` trait - breaks WASI compatibility
- Use wasm_bindgen_futures - browser-specific

**2. State Consistency**

With state distributed across browser, edge, and server:
- How to handle conflicts? (Last-write-wins? CRDTs?)
- What consistency guarantees do we provide?
- How to handle network partitions?

**3. Cost Model**

Cloudflare pricing per request/duration:
- At what scale does edge deployment become cost-effective?
- How to optimize for minimal DO invocations?
- Caching strategies for cost reduction?

**4. Security Boundaries**

With capabilities at the edge:
- How to verify agent identity at the edge?
- Can we trust namespace resolution from edge?
- How to audit capability usage?

### Limitations

**Cloudflare Workers Constraints:**
- 10ms CPU time per request (50ms for paid)
- 128MB memory limit
- No persistent local storage (use KV/R2/D1)
- Limited syscall support in WASM

**OANIX Constraints:**
- Sync FileService may bottleneck on network calls
- Namespace immutability limits dynamic capability grants
- No union mounts (yet)

### Alternative Approaches

**1. Pure Browser + Server (No Edge)**

Skip edge entirely, use direct WebSocket to server:
- Simpler architecture
- Higher latency for distant users
- No edge caching benefits

**2. Fly.io Instead of Cloudflare**

Fly.io offers:
- Full VMs (not just WASM)
- Persistent volumes
- Global distribution

Trade-offs:
- Less edge locations than Cloudflare
- Different pricing model
- No equivalent to Durable Objects

**3. Self-Hosted Edge (Constellation)**

Deploy own edge network:
- Full control
- Complex operations
- Requires significant infrastructure

---

## References

**OANIX:**
- `crates/oanix/src/service.rs` - FileService trait
- `crates/oanix/src/namespace.rs` - Namespace + Mount
- `crates/oanix/src/services/` - Service implementations
- `crates/oanix/docs/ARCHITECTURE.md` - Core architecture

**Cloudflare:**
- `crates/cloudflare/src/relay_do.rs` - Existing Durable Object implementation
- `crates/cloudflare/wrangler.toml` - Worker configuration
- [Durable Objects docs](https://developers.cloudflare.com/durable-objects/)
- [WebSocket Hibernation API](https://developers.cloudflare.com/durable-objects/api/websockets/)

**Agents:**
- `crates/agents/src/core/traits.rs` - AgentExecutor, JobRequest
- `crates/nostr/src/nip90.rs` - NIP-90 job protocol
