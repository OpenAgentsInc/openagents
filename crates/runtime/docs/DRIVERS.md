# Event Drivers

How external events become agent messages.

---

## The Driver Model

Agents don't handle HTTP, WebSocket, or Nostr directly. They receive **envelopes** through a mailbox. Drivers translate external events into envelopes.

```
┌─────────────────────────────────────────────────────────────────┐
│                      External World                             │
├──────────┬──────────┬──────────┬──────────┬────────────────────┤
│   HTTP   │    WS    │  Nostr   │ Scheduler│     GitHub         │
│ requests │  frames  │  events  │  timers  │    webhooks        │
└────┬─────┴────┬─────┴────┬─────┴────┬─────┴─────────┬──────────┘
     │          │          │          │               │
     ▼          ▼          ▼          ▼               ▼
┌─────────────────────────────────────────────────────────────────┐
│                         Drivers                                 │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────────────┐│
│  │  HTTP  │ │   WS   │ │ Nostr  │ │Schedule│ │    Webhook     ││
│  │ Driver │ │ Driver │ │ Driver │ │ Driver │ │    Driver      ││
│  └───┬────┘ └───┬────┘ └───┬────┘ └───┬────┘ └───────┬────────┘│
│      │          │          │          │              │         │
│      └──────────┴──────────┴──────────┴──────────────┘         │
│                            │                                    │
│                            ▼                                    │
│                    ┌───────────────┐                           │
│                    │   Envelope    │                           │
│                    │    Queue      │                           │
│                    └───────┬───────┘                           │
│                            │                                    │
└────────────────────────────┼────────────────────────────────────┘
                             │
                             ▼
                    ┌───────────────┐
                    │     Agent     │
                    │  (one mailbox)│
                    └───────────────┘
```

---

## Envelope Types

All external events normalize to envelopes:

```rust
/// A message delivered to an agent's mailbox
pub struct Envelope {
    /// Unique message ID
    pub id: EnvelopeId,

    /// When the event occurred
    pub timestamp: Timestamp,

    /// Source of the event
    pub source: Source,

    /// The actual payload
    pub payload: Payload,

    /// Metadata for routing/debugging
    pub metadata: Metadata,
}

/// Where the envelope came from
pub enum Source {
    /// HTTP request
    Http { method: String, path: String, remote_addr: Option<String> },

    /// WebSocket message
    WebSocket { connection_id: String },

    /// Nostr event
    Nostr { relay: String, event_id: String },

    /// Scheduled alarm
    Scheduler { alarm_id: String, scheduled_for: Timestamp },

    /// Another agent
    Agent { from: AgentId },

    /// External webhook
    Webhook { source: String, event_type: String },

    /// Manual/CLI invocation
    Manual { invoker: Option<String> },

    /// System event (lifecycle)
    System,
}

/// The message content
pub enum Payload {
    /// Text content
    Text(String),

    /// Structured JSON
    Json(serde_json::Value),

    /// Binary data
    Binary(Vec<u8>),

    /// Typed agent message
    AgentMessage(AgentMessage),

    /// Nostr event
    NostrEvent(NostrEvent),

    /// HTTP request body
    HttpRequest(HttpRequest),

    /// Tick trigger
    Tick(TickCause),
}
```

---

## Driver Trait

```rust
/// A driver translates external events into envelopes
#[async_trait]
pub trait Driver: Send + Sync {
    /// Driver name for logging/metrics
    fn name(&self) -> &str;

    /// Start the driver, sending envelopes to the provided sink
    async fn start(&self, sink: EnvelopeSink) -> Result<DriverHandle>;

    /// Stop the driver gracefully
    async fn stop(&self) -> Result<()>;
}

/// Handle to a running driver
pub struct DriverHandle {
    /// Unique driver instance ID
    pub id: String,

    /// Channel to stop the driver
    pub stop_tx: oneshot::Sender<()>,

    /// Join handle for the driver task
    pub handle: JoinHandle<Result<()>>,
}

/// Sink for envelopes (mpsc channel)
pub type EnvelopeSink = mpsc::Sender<Envelope>;
```

---

## Standard Drivers

### HTTP Driver

Exposes agents via HTTP endpoints:

```rust
pub struct HttpDriver {
    /// Listen address
    bind: SocketAddr,

    /// Agent router
    router: AgentRouter,

    /// TLS config (optional)
    tls: Option<TlsConfig>,
}

impl Driver for HttpDriver {
    fn name(&self) -> &str { "http" }

    async fn start(&self, sink: EnvelopeSink) -> Result<DriverHandle> {
        // Start HTTP server
        // Route requests to agents based on path
        // Convert requests to envelopes
    }
}
```

Request mapping:
```
POST /agents/{id}/inbox  →  Envelope { payload: Json(body) }
GET  /agents/{id}/status →  Envelope { payload: Query("status") }
```

### WebSocket Driver

Manages persistent connections:

```rust
pub struct WebSocketDriver {
    /// Connection registry
    connections: ConnectionRegistry,

    /// Heartbeat interval
    heartbeat: Duration,
}

impl Driver for WebSocketDriver {
    fn name(&self) -> &str { "websocket" }

    async fn start(&self, sink: EnvelopeSink) -> Result<DriverHandle> {
        // Accept WebSocket upgrades
        // Track connections per agent
        // Convert frames to envelopes
        // Enable broadcast back to connections
    }
}
```

### Nostr Driver

Connects to Nostr relays:

```rust
pub struct NostrDriver {
    /// Relay URLs
    relays: Vec<String>,

    /// Subscriptions per agent
    subscriptions: HashMap<AgentId, Vec<Filter>>,

    /// Signer for publishing
    signer: Arc<dyn SigningService>,
}

impl Driver for NostrDriver {
    fn name(&self) -> &str { "nostr" }

    async fn start(&self, sink: EnvelopeSink) -> Result<DriverHandle> {
        // Connect to relays
        // Subscribe to agent mentions, DMs
        // Convert Nostr events to envelopes
    }
}
```

Subscription filters:
```rust
// DMs to agent
Filter { kinds: [4], "#p": [agent_pubkey] }

// Mentions in notes
Filter { kinds: [1], "#p": [agent_pubkey] }

// NIP-90 job requests
Filter { kinds: [5xxx], "#p": [agent_pubkey] }
```

### Scheduler Driver

Manages alarms and recurring tasks:

```rust
pub struct SchedulerDriver {
    /// Alarm storage (persisted)
    storage: Box<dyn AlarmStorage>,

    /// Timer wheel for efficient scheduling
    timers: TimerWheel,
}

impl Driver for SchedulerDriver {
    fn name(&self) -> &str { "scheduler" }

    async fn start(&self, sink: EnvelopeSink) -> Result<DriverHandle> {
        // Load persisted alarms
        // Set up timer wheel
        // Fire envelopes when alarms trigger
    }
}
```

Alarm types:
```rust
pub enum AlarmType {
    /// Fire once at specific time
    Once { at: Timestamp },

    /// Fire repeatedly
    Recurring { interval: Duration, next: Timestamp },

    /// Cron expression
    Cron { expr: String, next: Timestamp },
}
```

### Webhook Driver

Receives external webhooks:

```rust
pub struct WebhookDriver {
    /// Webhook endpoint
    bind: SocketAddr,

    /// Signature verification per source
    verifiers: HashMap<String, Box<dyn WebhookVerifier>>,

    /// Routing rules
    routes: Vec<WebhookRoute>,
}

impl Driver for WebhookDriver {
    fn name(&self) -> &str { "webhook" }

    async fn start(&self, sink: EnvelopeSink) -> Result<DriverHandle> {
        // Start webhook server
        // Verify signatures (GitHub, Stripe, etc.)
        // Route to appropriate agents
    }
}
```

---

## Driver Configuration

Drivers are configured per-runtime:

```yaml
# runtime.yaml
drivers:
  http:
    enabled: true
    bind: "0.0.0.0:8080"
    tls:
      cert: /path/to/cert.pem
      key: /path/to/key.pem

  websocket:
    enabled: true
    heartbeat: 30s

  nostr:
    enabled: true
    relays:
      - wss://relay.damus.io
      - wss://nos.lol

  scheduler:
    enabled: true
    storage: sqlite  # or postgres, redis

  webhook:
    enabled: true
    bind: "0.0.0.0:9000"
    sources:
      github:
        secret: ${GITHUB_WEBHOOK_SECRET}
      stripe:
        secret: ${STRIPE_WEBHOOK_SECRET}
```

---

## Backend-Specific Drivers

Some drivers only make sense on certain backends:

| Driver | Local | Cloudflare | Server |
|--------|-------|------------|--------|
| HTTP | ✅ axum | ✅ fetch handler | ✅ axum |
| WebSocket | ✅ tokio-tungstenite | ✅ DO WebSocket | ✅ tokio-tungstenite |
| Nostr | ✅ nostr-client | ⚠️ via fetch | ✅ nostr-client |
| Scheduler | ✅ tokio timers | ✅ DO alarms | ✅ tokio timers |
| Webhook | ✅ HTTP server | ✅ fetch handler | ✅ HTTP server |
| CLI | ✅ stdin/IPC | ❌ | ✅ stdin/IPC |
| FUSE | ✅ fuser | ❌ | ✅ fuser |

---

## Response Path

Drivers also handle responses back to the external world:

```rust
/// Response from agent to external world
pub enum Response {
    /// HTTP response
    Http(HttpResponse),

    /// WebSocket frame to send
    WebSocket { connection_id: String, frame: Frame },

    /// Nostr event to publish
    Nostr(NostrEvent),

    /// No response needed
    None,
}

/// Agent context includes response capability
impl AgentContext {
    /// Send response back through the driver
    pub fn respond(&self, response: Response) -> Result<()>;

    /// Broadcast to all WebSocket connections
    pub fn broadcast(&self, event: &str, data: impl Serialize);

    /// Publish Nostr event
    pub async fn publish_nostr(&self, event: NostrEvent) -> Result<()>;
}
```

---

## Custom Drivers

Applications can implement custom drivers:

```rust
/// Example: Slack driver
pub struct SlackDriver {
    token: String,
    app_id: String,
}

impl Driver for SlackDriver {
    fn name(&self) -> &str { "slack" }

    async fn start(&self, sink: EnvelopeSink) -> Result<DriverHandle> {
        // Connect to Slack RTM API
        // Convert Slack events to envelopes
        // Handle @mentions, DMs, slash commands
    }
}

/// Example: Discord driver
pub struct DiscordDriver {
    token: String,
    guild_ids: Vec<String>,
}

impl Driver for DiscordDriver {
    fn name(&self) -> &str { "discord" }

    async fn start(&self, sink: EnvelopeSink) -> Result<DriverHandle> {
        // Connect to Discord gateway
        // Convert Discord events to envelopes
    }
}
```

---

## The Plumber Integration

Drivers feed the plumber, which applies routing rules:

```
┌─────────────────────────────────────────────────────────────────┐
│                         Drivers                                 │
│  HTTP  │  WS  │  Nostr  │  Scheduler  │  Webhook  │  Custom    │
└────────┴──────┴─────────┴─────────────┴───────────┴─────────────┘
                              │
                              ▼
                    ┌───────────────────┐
                    │     Plumber       │
                    │  (routing rules)  │
                    └─────────┬─────────┘
                              │
         ┌────────────────────┼────────────────────┐
         ▼                    ▼                    ▼
    ┌─────────┐          ┌─────────┐          ┌─────────┐
    │ Agent A │          │ Agent B │          │ Agent C │
    │  inbox  │          │  inbox  │          │  inbox  │
    └─────────┘          └─────────┘          └─────────┘
```

See [PLAN9.md](./PLAN9.md) for plumber rule configuration.
