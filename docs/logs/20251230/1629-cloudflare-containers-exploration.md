# Cloudflare Containers Exploration

**Date:** 2024-12-30 17:00
**Status:** Research / Planning

## Context

We implemented a tunnel architecture where users run `openagents connect` to bridge their local machine to the web UI. This works but requires user action. Cloudflare Containers could eliminate the CLI requirement entirely.

## What Are Cloudflare Containers?

Cloudflare Containers enable deployment of containerized workloads integrated with Workers. Per the docs: "Run code written in any programming language, built for any runtime, as part of apps built on Workers."

Unlike regular Workers/DOs (which are WASM sandboxes), containers have:

- **Real filesystem** - can clone repos, write files
- **Shell access** - can spawn processes
- **TCP networking** - can make arbitrary network calls
- **Persistent storage** - data survives restarts
- **Any runtime** - not limited to WASM

This is exactly what autopilot needs.

## Rust SDK Support

**Good news:** workers-rs has full container support as of September 2025 (commit `8ddef15`).

The API is available via `state.container()` in Durable Objects:

```rust
use worker::*;

#[durable_object]
pub struct AutopilotContainer {
    state: State,
    env: Env,
}

impl DurableObject for AutopilotContainer {
    fn new(state: State, env: Env) -> Self {
        // Get container from state
        let container = state.container().expect("failed to get container");

        // Start if not running
        if !container.running() {
            let mut options = ContainerStartupOptions::new();
            options.add_env("CLAUDE_API_KEY", "sk-ant-...");
            options.add_env("REPO_URL", "https://github.com/user/repo");
            options.enable_internet(true);

            container.start(Some(options)).expect("failed to start");
        }

        Self { state, env }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let container = self.state.container().ok_or("no container")?;

        // Get Fetcher for container's HTTP port
        let fetcher = container.get_tcp_port(8080)?;

        // Forward request to container
        fetcher.fetch_request(req).await
    }
}
```

### Container API Methods

```rust
impl Container {
    /// Check if container is running
    pub fn running(&self) -> bool;

    /// Start container with optional configuration
    pub fn start(&self, options: Option<ContainerStartupOptions>) -> Result<()>;

    /// Wait for container to exit
    pub async fn wait_for_exit(&self) -> Result<()>;

    /// Gracefully stop container
    pub async fn destroy(&self, error: Option<&str>) -> Result<()>;

    /// Send signal to container process
    pub fn signal(&self, signo: i32) -> Result<()>;

    /// Get Fetcher for a TCP port (for HTTP requests to container)
    pub fn get_tcp_port(&self, port: u16) -> Result<Fetcher>;
}

impl ContainerStartupOptions {
    pub fn new() -> Self;
    pub fn set_entrypoint(&mut self, entrypoint: &[&str]);
    pub fn enable_internet(&mut self, enable: bool);
    pub fn add_env(&mut self, key: &str, value: &str);
}
```

## Architecture Comparison

### Current: Tunnel Architecture

```
┌─────────────┐                  ┌─────────────────┐                  ┌─────────────────┐
│   Browser   │◄───WebSocket────►│  CF Worker      │◄───WebSocket────►│  User's Machine │
│  (WASM)     │                  │  (Relay DO)     │                  │  (CLI)          │
└─────────────┘                  └─────────────────┘                  └─────────────────┘
                                                                              │
                                                                     ┌───────┴───────┐
                                                                     │   autopilot   │
                                                                     │  (local fs)   │
                                                                     └───────────────┘
```

**Pros:**
- User controls compute and API keys
- Full access to local repos
- No container costs

**Cons:**
- Requires `openagents connect` CLI
- User must keep terminal open
- Network latency to user's machine

### Proposed: Container Architecture

```
┌─────────────┐                  ┌─────────────────┐
│   Browser   │◄───WebSocket────►│  CF Worker      │
│  (WASM)     │                  │  (Relay DO)     │
└─────────────┘                  └────────┬────────┘
                                          │
                                 ┌────────┴────────┐
                                 │  CF Container   │
                                 │  (per user/task)│
                                 │                 │
                                 │  - cloned repo  │
                                 │  - autopilot    │
                                 │  - claude CLI   │
                                 └─────────────────┘
```

**Pros:**
- No CLI required - fully cloud
- Edge-located compute (low latency)
- User just clicks "Start" in browser

**Cons:**
- Container costs (per minute/hour)
- Need to handle user's Claude API key securely
- Repo clone time on cold start

## How Cloudflare Containers Work

### Container Definition

```dockerfile
# Container image for autopilot
FROM rust:1.83-slim

# Install git and other tools
RUN apt-get update && apt-get install -y git

# Copy pre-built autopilot binary
COPY autopilot /usr/local/bin/autopilot

# Entry point
ENTRYPOINT ["autopilot", "serve", "--port", "8080"]
```

### Durable Object Integration

```rust
#[durable_object]
pub struct AutopilotContainer {
    state: State,
    env: Env,
    container: Option<Container>,
}

impl DurableObject for AutopilotContainer {
    async fn fetch(&self, req: Request) -> Result<Response> {
        // Spawn container if not running
        if self.container.is_none() {
            self.container = Some(
                self.state.container()
                    .image("openagents/autopilot:latest")
                    .env("CLAUDE_API_KEY", &self.get_user_api_key())
                    .env("REPO_URL", &self.get_repo_url())
                    .spawn()
                    .await?
            );
        }

        // Forward request to container
        self.container.as_ref().unwrap()
            .fetch(req)
            .await
    }
}
```

### Container Lifecycle

1. **Cold Start**: User starts task → DO spawns container → clone repo → start autopilot
2. **Warm**: Container stays alive while task runs, WebSocket streams to browser
3. **Idle Shutdown**: After N minutes of inactivity, container hibernates
4. **Resume**: Next request wakes container (repo still there from volume)

## Implementation Plan

### Phase 1: Container Image

Build and publish autopilot container image:

```
crates/autopilot-container/
├── Dockerfile
├── entrypoint.sh
└── README.md
```

The container needs:
- Rust autopilot binary (pre-built)
- Claude Code CLI binary
- Git
- Basic shell tools

### Phase 2: Container DO

New Durable Object that manages container lifecycle:

```rust
// crates/web/worker/src/container.rs

#[durable_object]
pub struct AutopilotContainer {
    state: State,
    env: Env,
}

impl AutopilotContainer {
    /// Spawn container for user's task
    async fn spawn_for_task(&self, user_id: &str, repo: &str, api_key: &str) -> Result<()> {
        // 1. Get or create container
        // 2. Clone repo into container
        // 3. Start autopilot daemon
        // 4. Return WebSocket URL for streaming
    }

    /// Stop container and cleanup
    async fn stop(&self) -> Result<()> {
        // Graceful shutdown
    }
}
```

### Phase 3: Updated Routes

```rust
// New endpoints in lib.rs

// Start a task in container (no CLI needed)
(Method::Post, "/api/task/start") => {
    let body = req.text().await?;
    with_auth(&req, &env, |user| {
        routes::task::start_container_task(user, env.clone(), body)
    }).await
}

// Stream task output via WebSocket
(Method::Get, path) if path.starts_with("/api/task/ws/") => {
    routes::task::websocket(req, env).await
}
```

### Phase 4: Credential Handling

Options for Claude API key:

1. **Claude OAuth** (preferred) - User already authenticated, use their tokens
2. **Bring Your Own Key** - User pastes API key, stored encrypted in KV
3. **Our Credits** - User pays us, we use our API key

```rust
enum ApiKeySource {
    ClaudeOAuth { user_id: String },        // Tokens from OAuth flow
    UserProvided { encrypted_key: String }, // User's own key
    PlatformCredits { user_id: String },    // Our API key, user pays credits
}
```

## Cost Analysis

### Cloudflare Container Pricing (estimated)

- **Compute**: ~$0.02/hour per container
- **Storage**: ~$0.02/GB-month for volumes
- **Requests**: Free within DO limits

### User Cost Scenarios

| Usage | Container Hours | Est. Cost |
|-------|-----------------|-----------|
| Light (1 task/day, 10min each) | 5 hrs/month | $0.10 |
| Medium (5 tasks/day, 15min each) | 37 hrs/month | $0.74 |
| Heavy (20 tasks/day, 20min each) | 200 hrs/month | $4.00 |

Plus Claude API costs (user's key or credits).

## Security Considerations

1. **API Key Storage**: Encrypt at rest, never log, short TTL
2. **Container Isolation**: Each user gets separate container/DO
3. **Repo Access**: Only clone repos user has access to (GitHub OAuth)
4. **Network**: Container can't access other user containers
5. **Secrets in Repo**: Warn users about .env files in repos

## Migration Path

1. **Now**: Tunnel architecture (requires CLI)
2. **Next**: Container option (no CLI, higher cost)
3. **Future**: User chooses based on preference/cost

Both architectures can coexist:
- Power users: `openagents connect` (free, uses their machine)
- Casual users: Container mode (pay per use, zero setup)

## Open Questions

1. **Cold start time**: How long to clone repo + start autopilot?
2. **Volume persistence**: How long do repos stay cached?
3. **Container limits**: Max memory, CPU, disk?
5. **Claude CLI in container**: Licensing/distribution issues?

## Next Steps

1. ~~Research actual Cloudflare Containers API availability~~ ✓ (workers-rs supports it)
2. Build minimal container image with autopilot
3. Test container spawn from DO
4. Benchmark cold start times
5. Implement streaming from container to browser

---

## NIP-90 Integration: Nostr Data Vending Machine

### The Bigger Picture

NIP-90 defines a protocol for "jobs" on Nostr - users post job requests (kind 5xxx), service providers bid and execute, results are published. This creates a decentralized compute marketplace.

### Two Compute Tiers

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          NOSTR NETWORK (NIP-90)                              │
│                                                                              │
│  Job Requests (kind 5xxx) ───────────────────────────────────────────────►  │
│  ◄─────────────────────────────────────────────── Job Results (kind 6xxx)   │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                    ┌──────────────────┴──────────────────┐
                    │                                     │
                    ▼                                     ▼
    ┌───────────────────────────────┐    ┌───────────────────────────────────┐
    │  LOCAL COMPUTE (Free Tier)    │    │  CLOUD COMPUTE (Paid Tier)        │
    │                               │    │                                    │
    │  User runs `openagents        │    │  CF Container per user             │
    │  connect --nip90`             │    │  - Spawned on demand               │
    │  - Provides own API key       │    │  - User pays credits               │
    │  - Earns sats for jobs        │    │  - We pay API costs                │
    │  - Full local access          │    │  - No local access                 │
    └───────────────────────────────┘    └───────────────────────────────────┘
```

### Local Compute: User as Service Provider

User runs their own NIP-90 service provider:

```bash
# User's machine becomes a NIP-90 service provider
openagents connect --nip90 --skills "code,analysis"

# What this does:
# 1. Connects to Nostr relays
# 2. Subscribes to NIP-90 job requests matching user's skills
# 3. Runs autopilot locally with user's Claude API key
# 4. Publishes results back to Nostr
# 5. User earns sats for completed jobs
```

Architecture:
```
┌───────────────────────────────────────────────────────────────────┐
│  USER'S MACHINE                                                    │
│                                                                    │
│  openagents connect --nip90                                        │
│  ├── Nostr client (subscribes to job requests)                    │
│  ├── Job queue (manages pending work)                             │
│  ├── Autopilot daemon (executes jobs)                             │
│  │   └── Claude API (user's key)                                  │
│  └── Result publisher (posts to Nostr)                            │
│                                                                    │
│  User controls:                                                    │
│  - Which job types to accept                                      │
│  - Price per job                                                  │
│  - Max concurrent jobs                                            │
│  - Which repos to allow access                                    │
└───────────────────────────────────────────────────────────────────┘
```

### Cloud Compute: Containers as Service Providers

For users who want "always on" without running a machine:

```
┌───────────────────────────────────────────────────────────────────┐
│  CLOUDFLARE EDGE                                                   │
│                                                                    │
│  User's Durable Object (Agent)                                     │
│  ├── Nostr WebSocket to relay                                     │
│  ├── Subscribes to jobs matching user's config                    │
│  ├── Spawns container on job arrival                              │
│  │   └── Container: autopilot + cloned repo                       │
│  ├── Streams results back via Nostr                               │
│  └── Bills user credits for container time + API                  │
│                                                                    │
│  User configures via web UI:                                       │
│  - Which job types to accept                                      │
│  - Auto-approve rules                                             │
│  - Credit budget per day/week                                     │
└───────────────────────────────────────────────────────────────────┘
```

### Hybrid: DO as Coordinator, Fan-out to Containers

For complex jobs, a single DO could coordinate multiple containers:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         USER'S AGENT (Durable Object)                        │
│                                                                              │
│  Nostr listener ──► Job arrives ──► Analyze job ──► Fan out                 │
│                                                          │                   │
│                                     ┌────────────────────┼────────────────┐  │
│                                     │                    │                │  │
│                                     ▼                    ▼                ▼  │
│                              ┌───────────┐        ┌───────────┐    ┌──────┐ │
│                              │ Container │        │ Container │    │ ...  │ │
│                              │ (subtask) │        │ (subtask) │    │      │ │
│                              └───────────┘        └───────────┘    └──────┘ │
│                                     │                    │                │  │
│                                     └────────────────────┼────────────────┘  │
│                                                          │                   │
│  Aggregator ◄─────────────────────────────────────◄──────┘                   │
│       │                                                                      │
│       ▼                                                                      │
│  Publish result to Nostr                                                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Economic Model

| Provider Type | Who Pays What | Who Earns |
|---------------|---------------|-----------|
| Local compute (tunnel) | User pays own API key | User earns sats from requesters |
| Cloud compute (container) | Platform pays container + API, user pays credits | Platform earns from user credits |
| Hybrid | Split based on job complexity | Split based on contribution |

### NIP-90 Job Types We Could Support

| Kind | Type | Example |
|------|------|---------|
| 5000 | Generic compute | "Run this shell script" |
| 5100 | Code task | "Implement this feature in my repo" |
| 5200 | Analysis | "Review this PR for security issues" |
| 5300 | Documentation | "Generate docs for this codebase" |

### Implementation Phases

**Phase 1: Local NIP-90 Provider**
- Add `--nip90` flag to `openagents connect`
- Subscribe to job requests on Nostr
- Execute via local autopilot
- Publish results

**Phase 2: DO + Container Provider**
- DO maintains Nostr connection (WebSocket hibernation)
- Spawns container on job arrival
- Streams progress via Nostr
- Bills user credits

**Phase 3: Marketplace UI**
- Web dashboard showing available jobs
- Provider reputation/ratings
- Job history and earnings
- Credit purchase flow

### Key Insight

Containers are just **one primitive** in the marketplace:
- Some users provide local compute (free tier, earn sats)
- Some users pay for cloud compute (paid tier, spend credits)
- Both can participate in the same NIP-90 job market
- The DO is the "agent brain" - it can talk to Nostr, coordinate containers, manage state

## References

- [Cloudflare Containers Documentation](https://developers.cloudflare.com/containers/)
- [Durable Objects Documentation](https://developers.cloudflare.com/durable-objects/)
- [workers-rs Crate](https://docs.rs/worker/latest/worker/)
- [NIP-90: Data Vending Machine](https://github.com/nostr-protocol/nips/blob/master/90.md)
