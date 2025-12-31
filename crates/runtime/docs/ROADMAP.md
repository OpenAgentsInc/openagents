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

### References

- [CONTAINERS.md](CONTAINERS.md) — full spec

---

## Milestone 7 — Nostr Transport + Driver

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

### References

- [DRIVERS.md](DRIVERS.md) — Nostr driver
- [AGENT-SPECIFIC.md](AGENT-SPECIFIC.md) — identity

---

## Milestone 8 — Cloudflare Backend (DO Runtime + Compute Provider)

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

### References

- [BACKENDS.md](BACKENDS.md) — Cloudflare backend
- [COMPUTE.md](COMPUTE.md) — CloudflareProvider

---

## Milestone 9 — Cloud Containers + Daytona

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

## Milestone 10 — DVM Providers

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

## Milestone 11 — Browser Backend (WASM)

**Goal:** Same agent binary runs in browser.

### Tasks

- Minimal runtime + namespace
- Capabilities limited; route compute/containers through OpenAgents API providers

### Exit Criteria

Demo: browser agent with `/compute` + `/containers` via cloud.

### References

- [BACKENDS.md](BACKENDS.md) — Browser backend
- [PRIOR-ART.md](PRIOR-ART.md) — WANIX inspiration

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
| M7 | Nostr | Real identity + messaging |
| M8 | Cloudflare | DO backend + Workers AI |
| M9 | Cloud containers | Remote sandboxes |
| M10 | DVM | Decentralized compute/containers |
| M11 | Browser | WASM runtime |

---

## Critical Path

For the fastest path to a usable system:

```
M1 → M2 → M3 → M4 → M5 → M6
```

This gives you:
- Local runtime with persistent state
- Filesystem abstraction
- HTTP control plane
- Budget enforcement
- AI compute (`/compute`)
- Code execution (`/containers`)

The full abstraction is validated before adding Cloudflare/browser/DVM complexity.
