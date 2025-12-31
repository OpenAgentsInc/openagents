# Implementation Roadmap

Suggested build order for the OpenAgents Runtime. Each milestone has clear exit criteria.

**Critical path for fastest usable system:** Milestones 1→6 (local runtime + compute + containers + control plane) validates the full abstraction before tackling Cloudflare/browser/DVM complexity.

---

## Milestone 0 — Repo Skeleton + Conformance Harness

**Goal:** Tests can run before features land.

### Tasks

- Create runtime crate modules matching docs: `agent`, `trigger`, `envelope`, `backend`, `budget`, `fs`, `errors`
- Create conformance test crate/module with empty tests + helpers
- Define "single source of truth" types: `Timestamp`, `AgentId`, `EnvelopeId`

### Exit Criteria

```bash
cargo test  # runs, conformance harness compiles
```

---

## Milestone 1 — Core Tick Engine + Storage (MVP Runtime)

**Goal:** One agent can tick deterministically with persisted state.

### Tasks

- Implement `Agent`, `AgentState`, `Trigger`, `TickResult`
- Implement `AgentStorage` with SQLite (local) + in-memory (tests)
- Enforce no parallel ticks per agent (mutex/lock keyed by `AgentId`)
- Implement state load/commit atomicity (one transaction per tick)
- Implement minimal `AgentContext` + `seen`/`mark_seen` (bounded cache persisted)

### Exit Criteria

Conformance tests pass:
- `test_single_tick_lock`
- `test_hibernation_state_persists` (local)
- `test_dedup_helpers` (basic)

### Implementation Notes (completed)

- Added core runtime modules (`crates/runtime/src/agent.rs`, `crates/runtime/src/trigger.rs`, `crates/runtime/src/tick.rs`, `crates/runtime/src/types.rs`).
- Implemented storage backends (`InMemoryStorage`, `SqliteStorage`) with transactional `StorageOp` support in `crates/runtime/src/storage.rs`.
- Added `TickEngine` with per-agent locking, atomic commit, and persisted seen-cache in `crates/runtime/src/engine.rs`.
- Added conformance tests in `crates/runtime/src/tests/mod.rs` for locking, dedup, and local hibernation persistence.

### References

- [DESIGN.md](DESIGN.md) — tick model
- [TRAITS.md](TRAITS.md) — Agent trait

---

## Milestone 2 — Filesystem Core + Namespace + Standard Services

**Goal:** "Everything is a file" works locally in-process.

### Tasks

- Implement `FileService`, `FileHandle`, `Namespace` (longest-prefix matching)
- Mount these services for one agent:
  - `StatusFs`
  - `InboxFs` (+ overflow/deadletter)
  - `LogsFs` (trace watch)
  - `IdentityFs` (stub signer)
  - `GoalsFs` (minimal)
- Fix `SignOnly` semantics

### Exit Criteria

```rust
env.read("/status")           // works
env.write("/inbox", ...)      // works
env.watch("/logs/trace")      // works
```

### Implementation Notes (completed)

- Added filesystem core traits/types in `crates/runtime/src/fs.rs` plus namespace resolution in `crates/runtime/src/namespace.rs`.
- Implemented standard services (`StatusFs`, `InboxFs`, `DeadletterFs`, `LogsFs`, `IdentityFs`, `GoalsFs`) under `crates/runtime/src/services/`.
- Added `AgentEnv` in `crates/runtime/src/env.rs` to mount standard services and enforce SignOnly access.
- Added a minimal `Envelope` type (`crates/runtime/src/envelope.rs`) and `SigningService` stub (`crates/runtime/src/identity.rs`).
- Added env-level tests in `crates/runtime/src/tests/mod.rs` for status read, inbox write, and logs watch.

### References

- [FILESYSTEM.md](FILESYSTEM.md) — FileService trait
- [PLAN9.md](PLAN9.md) — namespace design

---

## Milestone 3 — Control Plane (Local HTTP + CLI)

**Goal:** Operators can drive agents without any "special API."

### Tasks

- Implement HTTP mapping for read/write/list/watch (SSE)
- Implement `agentctl` minimal:
  - `list`, `status`, `send`, `tick`, `logs --follow`
- Implement `/agents/<id>/...` namespace wrapper

### Exit Criteria

```bash
curl -X POST localhost:8080/agents/abc/tick    # ticks agent
curl -X POST localhost:8080/agents/abc/send    # sends message
curl localhost:8080/agents/abc/logs/trace      # tails logs (SSE)
```

### Implementation Notes (completed)

- Added `LocalRuntime` and `ControlPlane` in `crates/runtime/src/control_plane.rs` with HTTP routes for list, status/info, send, tick, and generic filesystem read/write/watch.
- Implemented SSE watch bridge for `watch=1` and byte/JSON responses for file reads.
- Added `agentctl` CLI in `crates/runtime/src/bin/agentctl.rs` for list/status/send/tick/logs.
- Added HTTP control-plane tests in `crates/runtime/src/tests/mod.rs`.

### References

- [CONTROL-PLANE.md](CONTROL-PLANE.md) — HTTP + CLI spec

---

## Milestone 4 — Budgets + Idempotency Journal

**Goal:** Budgeted mounts and dedup are real. Foundation for `/compute` and `/containers`.

### Tasks

- Implement `BudgetPolicy` + `BudgetState` in micro-USD
- Implement `IdempotencyJournal` (memory + SQLite) with TTL
- Implement budget reserve/reconcile primitives

### Exit Criteria

Conformance tests pass:
- `test_budget_enforcement` (mount-level)
- `test_idempotent_effects` (journal semantics)

### Implementation Notes (completed)

- Added budget primitives in `crates/runtime/src/budget.rs` (policy/state tracker, reserve/reconcile).
- Wired mount-level budget charging for write operations in `crates/runtime/src/env.rs` using `AccessLevel::Budgeted`.
- Implemented idempotency journals in `crates/runtime/src/idempotency.rs` with memory + SQLite backends and TTL cleanup.
- Added conformance tests for budget enforcement and idempotency in `crates/runtime/src/tests/mod.rs`.

### References

- [TRAITS.md](TRAITS.md) — BudgetPolicy
- [COMPUTE.md](COMPUTE.md) — idempotency journal

---

## Milestone 5 — /compute (Local Provider First)

**Goal:** Agents can call AI models via filesystem.

### Tasks

- Implement `ComputeFs` with:
  - `/compute/new`
  - `/compute/jobs/<id>/{status,result,stream}`
  - `/compute/usage` (reserved vs spent)
- Implement `LocalProvider` wrapping existing `crates/compute` registry
- Streaming via watch (tokens)

### Exit Criteria

Demo agent can:
- Submit a chat job
- Stream output via watch
- Budgets reserve/reconcile correctly

### Implementation Notes (completed)

- Added compute core types, provider routing, and `/compute` filesystem in `crates/runtime/src/compute.rs`.
- Implemented `LocalProvider` backed by `crates/compute` registry with async execution and streaming.
- Wired budget reservation/reconcile and idempotency response caching in `/compute/new` handling.
- Added compute conformance tests for submit/usage/idempotency and streaming watch in `crates/runtime/src/tests/mod.rs`.
- Skipped env auto-charge for `/compute` to avoid double-charging (compute handles budgets internally).

### References

- [COMPUTE.md](COMPUTE.md) — full spec

---

## Milestone 6 — /containers (Local Docker First)

**Goal:** Agents can run code safely.

### Tasks

- Implement `ContainerFs` with:
  - `/containers/new`
  - `/sessions/<id>/{status,output,result,files,ctl,usage}`
  - `/sessions/<id>/exec/new` → exec_id + exec output watch
  - `/containers/usage` (reserved vs spent)
- Implement `LocalContainerProvider` using Docker API (or CLI wrapper initially)
- Implement file read/write chunking + path validation

### Exit Criteria

"Generate code with `/compute`, test it in `/containers`" works locally.

### Implementation Notes (completed)

- Added container core types, policy, provider trait, and router in `crates/runtime/src/containers.rs`.
- Implemented `/containers` filesystem routes for new sessions, provider info, policy, usage, session status/result/output, exec jobs, and file read/write with URL decoding + chunking.
- Wired budget reservation/reconcile and idempotency caching for `/containers/new`, with reconcile on status/output.
- Implemented `LocalContainerProvider` using Docker CLI for session lifecycle, exec streaming, and file read/write via `docker exec`.
- Added container conformance tests in `crates/runtime/src/tests/mod.rs` and exported container APIs in `crates/runtime/src/lib.rs`.

### References

- [CONTAINERS.md](CONTAINERS.md) — full spec

---

## Milestone 7 — HUD (Web Interface)

**Goal:** Pure viewer over agent filesystem. "Watch files → render panes."

**Implementation:** `crates/web/` using WGPUI components from `crates/wgpui`.

### Tasks

- Implement HUD WebSocket/SSE client connecting to `/agents/<id>/hud/stream`
- Implement WGPUI pane components:
  - **ThreadView** — Render streaming events (tool_start, tool_done, chunk, error)
  - **CodePane** — Live file diffs with syntax highlighting
  - **TerminalPane** — Container stdout/stderr output
  - **MetricsPane** — APM gauge, queue depth, cost ticker
  - **StatusBar** — Agent state, session info, verification status
- Implement redacted public HUD mode (watch `/hud/stream` not `/logs/trace`)
- Implement replay mode for trajectory timelapse
- Add `/hud/settings` UI (public/private toggle, embed_allowed)

### Filesystem Dependencies

| Path | Usage |
|------|-------|
| `/agents/<id>/status` | Header: "Live: working on issue #847" |
| `/agents/<id>/hud/stream` | Main event stream (redacted for public) |
| `/agents/<id>/logs/trace` | Full event stream (owner only) |
| `/agents/<id>/logs/trajectory` | Replay source |
| `/agents/<id>/metrics/apm` | APM gauge |
| `/agents/<id>/metrics/queue` | Issue queue depth |
| `/agents/<id>/goals` | Active goals sidebar |

### Exit Criteria

- HUD renders live streaming events from `/hud/stream`
- Public HUD URL works: `openagents.com/repo/@username/repo`
- Embed works: `<iframe src="openagents.com/repo/@username/repo/embed">`
- Timelapse replay at 20x speed
- Screenshot-worthy: panes opening, code streaming, tools firing

### Implementation Notes (completed)

- Added `/hud` and `/metrics` filesystem services (redacted stream, settings persistence, metric snapshots) plus `/logs/trajectory` replay output in `crates/runtime/src/services/`.
- Mounted `/hud` and `/metrics` in `AgentEnv`, added runtime tests for settings redaction, metrics read/write, and trajectory output.
- Implemented HUD panes (`CodePane`, `TerminalPane`, `MetricsPane`) in `crates/wgpui/src/components/sections/` and wired the HUD layout/rendering flow in `crates/web/client/src/lib.rs`.
- Added HUD SSE client, replay mode, and settings toggles in the web client; updated the worker HUD context to pass `agent_id` and `stream_url`.

### GTM Requirements

The HUD is the product's signature moment. Must deliver:
- **Live fishbowl** — Real session, not demo mode
- **< 10s ah-ha moment** — Visible autonomous action immediately
- **Shareable** — Public by default, embeddable
- **Verifiable** — Links to real GitHub issues/PRs/commits

### References

- [HUD.md](HUD.md) — Full spec (event contract, redaction, ACL)
- [CONTROL-PLANE.md](CONTROL-PLANE.md) — SSE/WebSocket streaming
- `crates/wgpui/` — GPU-rendered UI components
- `crates/web/` — Web application

---

## Milestone 8 — Nostr Transport + Driver

**Goal:** Real agent identity + encrypted comms.

### Tasks

- Implement Nostr driver:
  - Subscribe to DMs / mentions
  - Publish events
- Implement `SigningService` real implementation
  - Local dev: in-memory
  - Local prod: keychain (later)
- Emit envelopes via drivers into inbox

### Exit Criteria

Agent can receive a Nostr DM → tick → reply.

### Implementation Notes (completed)

- Added `NostrSigner` with real Schnorr signing + NIP-44 encryption in `crates/runtime/src/identity.rs` and made it the default signer in `AgentEnv`.
- Implemented driver infrastructure plus `NostrDriver` relay subscriptions and publishing in `crates/runtime/src/drivers/`.
- Added `LocalRuntime::driver_sink` to route `RoutedEnvelope` into agent inboxes and runtime tests for signing, encryption, driver routing, and sink delivery.

### References

- [DRIVERS.md](DRIVERS.md) — Nostr driver
- [AGENT-SPECIFIC.md](AGENT-SPECIFIC.md) — identity

---

## Milestone 9 — Cloudflare Backend (DO Runtime + Compute Provider)

**Goal:** The tick model works on Durable Objects.

### Tasks

- Implement `runtime-cloudflare`:
  - DO storage adapter
  - DO alarms
  - Control-plane fetch routes → filesystem mapping
- Implement `CloudflareProvider` for `/compute` (Workers AI)
- Document cost reconciliation (spent = reserved when usage unavailable)

### Exit Criteria

An agent runs on DO, takes HTTP triggers, can do `/compute/new`.

### Implementation Notes (completed)

- Added `CloudflareStorage` DO SQL adapter plus `DoJournal` for idempotency persistence in `crates/runtime/src/storage.rs` and `crates/runtime/src/idempotency.rs`.
- Implemented `CloudflareProvider` (Workers AI) with async job execution and cost reconciliation that treats reserved `max_cost_usd` as spent when usage is unavailable in `crates/runtime/src/compute.rs`.
- Added `CloudflareAgent` Durable Object backend with agent factory registration, alarm scheduling, and control-plane fetch → filesystem mapping (read/write/list, tick/send) in `crates/runtime/src/cloudflare.rs`, including `/compute` mount when an AI binding is available.

### References

- [BACKENDS.md](BACKENDS.md) — Cloudflare backend
- [COMPUTE.md](COMPUTE.md) — CloudflareProvider

---

## Milestone 10 — Cloud Containers + Daytona

**Goal:** Remote sandboxes via `/containers`.

### Tasks

- Add provider adapters behind `/containers/providers/*`
- Implement OpenAgents API auth surface (token + nostr challenge)
- Implement credits bookkeeping (micro-USD)

### Exit Criteria

Same container request can run locally or via cloud provider based on policy.

### References

- [CONTAINERS.md](CONTAINERS.md) — OpenAgentsProvider, DaytonaProvider

---

## Milestone 11 — DVM Providers

**Goal:** Permissionless decentralized execution.

### Tasks

- Compute DVM: quote/accept/settle lifecycle
- Containers DVM: likely non-interactive, no file access; streaming via feedback events
- Add FX source `/wallet/fx` integration

### Exit Criteria

Agent can buy compute from DVM within reserved max cost.

### References

- [COMPUTE.md](COMPUTE.md) — DvmProvider
- [CONTAINERS.md](CONTAINERS.md) — DvmContainerProvider

---

## Milestone 12 — Browser Backend (WASM)

**Goal:** Same agent binary runs in browser.

### Tasks

- Minimal runtime + namespace
- Capabilities limited; route compute/containers through OpenAgents API providers

### Exit Criteria

Demo: browser agent with `/compute` + `/containers` via cloud.

### Implementation Notes (completed)

- Added WASM-safe feature gating for local/DVM providers plus a lightweight `wasm_http` fetch helper for browser networking.
- Implemented OpenAgents API-backed compute + container providers for WASM, including async token/nostr validation in `OpenAgentsAuth` and bech32 npub handling for browser builds.
- Added `BrowserRuntime` + `BrowserRuntimeConfig` to mount `/compute` and `/containers` via OpenAgents API providers with in-memory storage/journals (browser defaults use `InMemorySigner`).

### References

- [BACKENDS.md](BACKENDS.md) — Browser backend
- [PRIOR-ART.md](PRIOR-ART.md) — WANIX inspiration

---

## Milestone 13 — Browser Persistence (IndexedDB)

**Goal:** Browser runtime survives reloads with persisted agent state + KV.

### Tasks

- Implement IndexedDB-backed `AgentStorage` for state + KV.
- Use IndexedDB by default in `BrowserRuntimeConfig` with a stable DB name.
- Ensure transactional commit semantics for combined state + KV ops.

### Exit Criteria

Reloading the tab preserves agent state and storage entries via IndexedDB.

### Implementation Notes (completed)

- Added `IndexedDbStorage` in `crates/runtime/src/storage.rs` with IndexedDB state/KV stores and transactional ops for state + KV.
- Wired `BrowserRuntimeConfig::new` to default to `IndexedDbStorage` (`openagents-runtime` DB name) in `crates/runtime/src/browser.rs`.
- Added the required IndexedDB `web-sys` feature flags in `crates/runtime/Cargo.toml`.

### References

- [BACKENDS.md](BACKENDS.md) — Browser backend storage mapping

---

## Summary

| Milestone | Goal | Key Deliverable |
|-----------|------|-----------------|
| M0 | Repo skeleton | Conformance harness compiles |
| M1 | Tick engine | Single agent ticks with SQLite |
| M2 | Filesystem | FileService + namespace works |
| M3 | Control plane | HTTP + CLI drives agents |
| M4 | Budgets | Budget + idempotency foundation |
| M5 | /compute | AI models via filesystem |
| M6 | /containers | Code execution via filesystem |
| M7 | **HUD** | **Web UI via WGPUI (`crates/web/`)** |
| M8 | Nostr | Real identity + messaging |
| M9 | Cloudflare | DO backend + Workers AI |
| M10 | Cloud containers | Remote sandboxes |
| M11 | DVM | Decentralized compute/containers |
| M12 | Browser | WASM runtime |
| M13 | Browser persistence | IndexedDB-backed storage |

---

## Critical Path

For the fastest path to a **launchable** system:

```
M1 → M2 → M3 → M4 → M5 → M6 → M7
```

This gives you:
- Local runtime with persistent state
- Filesystem abstraction
- HTTP control plane
- Budget enforcement
- AI compute (`/compute`)
- Code execution (`/containers`)
- **HUD** — The product's signature moment

**M7 (HUD) is GTM-critical.** The live fishbowl, shareable URLs, and "demo is the product" strategy all depend on it. Without the HUD, there's no viral loop.

The full abstraction is validated before adding Cloudflare/browser/DVM complexity.
