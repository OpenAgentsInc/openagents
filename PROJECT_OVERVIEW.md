# OpenAgents Repository Map

This document is a **codebase orientation guide**: what lives where, how the crates relate, and where to start based on what you want to change.

For "what is OpenAgents" + Quick Start, see **[README.md](./README.md)**.
For values/mission, see **[MANIFESTO.md](./MANIFESTO.md)**.
For canonical terminology, see **[GLOSSARY.md](./GLOSSARY.md)**.

## The stack (at a glance)

OpenAgents is organized as a layered stack:

- **Products**: Autopilot (wgpui), Autopilot Desktop (WGPUI), Onyx, GitAfter
- **Execution**: Adjutant + Autopilot loop (plan/act/verify)
- **Compiler layer**: dsrs (DSPy-style signatures/modules/optimizers)
- **Runtime + infra**: tools, sandboxes, logging, provider routing
- **Protocols**: Nostr messaging + typed job schemas + receipts (where applicable)

### Product framing: Autonomy-as-a-Service

Autopilot is positioned as **predictable autonomy**: a bounded, auditable
delegation contract with clear scope, time horizon, constraints, verification,
and escalation behavior. Signatures/modules make this sellable by turning each
decision step into a measurable work unit with receipts and utility labels.

## Core flows

### Autopilot (local) execution loop
1. Plan work (DSPy signatures)
2. Execute tool calls (read/edit/bash/search)
3. Verify (tests/build)
4. Iterate until success/failure/cap
5. Emit replay + artifacts for inspection

### Swarm / marketplace (when enabled)
- Dispatch typed jobs (objective/subjective)
- Verify objective outputs deterministically
- Record provenance (job hashes, receipts)

## Crate map (what lives where)

### Products
- `crates/autopilot/` — user-facing app + CLI for autonomous coding runs (wgpui)
- `apps/autopilot-desktop/` — native WGPUI desktop host (winit + wgpu)
- `crates/autopilot-core/` — core execution loop, replay/session plumbing
- `crates/onyx/` — local-first markdown editor
- `crates/gitafter/` — Nostr-native git collaboration surface (NIP-34)
- `crates/coder/` — Codex-focused terminal/app-server UI surface (if present)

### API
- `apps/api/` — OpenAgents Cloudflare Worker; **live:** `https://openagents.com/api` (health, Moltbook proxy, Agent Payments [agents, wallet registry, balance/invoice/pay], docs index). The `oa moltbook` CLI and Autopilot Desktop use this proxy by default; see `apps/api/README.md` and `apps/api/docs/`.
- `apps/indexer/` — Moltbook indexer Worker; **live:** `https://openagents.com/api/indexer` (ingest, backfill, search, wallet-adoption metrics). R2 + D1 + KV + Queues + Cron; see `apps/indexer/README.md` and `private/indexer.md`.
- `apps/spark-api/` — Spark API Worker; **live:** `https://openagents.com/api/spark` (balance, invoice, pay for Agent Payments; stub until Breez SDK + KV adapter). Called by the API when `SPARK_API_URL` is set; see `apps/spark-api/README.md`.

### Execution + learning
- `crates/adjutant/` — execution engine + decision pipelines + session store + auto-optimization hooks
- `crates/dsrs/` — Rust DSPy (signatures/modules/optimizers/tracing/providers)

### Runtime + tooling
- `crates/runtime/` — tool mediation, sandbox boundaries, tick model (Plan 9-ish FS surface where applicable)
- `crates/ai-server/` — AI Gateway server management helpers (desktop)
- `crates/rlm/` — Recursive Language Model executor + context ops/tools
- `crates/frlm/` — federated recursion orchestration (fanout/gather)

### Protocol + networking
- `crates/protocol/` — typed job schemas + deterministic hashing + verification modes
- `crates/nexus/` — relay/coordination infra (NIP-90/NIP-42/NIP-89 related)
- `crates/pylon/` — compute marketplace node (provider/buyer modes, NIP-90 jobs)
- `crates/oanix/` — environment discovery manifests and "what can I do here?" surfaces

### Payments / treasury (as wired)
- `crates/neobank/` — budgeted spending + receipts + routing primitives
- `crates/spark*` — Lightning/Spark integration surfaces (where present)

### UI infra
- `crates/wgpui/` — GPU UI primitives and component system

## Where to start (common contributor paths)

- "I want to improve Autopilot behavior" → `crates/autopilot/`, `crates/autopilot-core/`, `crates/adjutant/`
- "I want to work on Autopilot Desktop UI" → `crates/autopilot_ui/` + `apps/autopilot-desktop/`
- "I want to add/optimize DSPy signatures" → `crates/dsrs/` (+ decision pipelines in `crates/adjutant/`)
- "I want better replay/artifacts" → `crates/autopilot-core/` + specs in `crates/dsrs/docs/REPLAY.md` and `crates/dsrs/docs/ARTIFACTS.md`
- "I want marketplace job schemas / verification" → `crates/protocol/` (+ any dispatchers)
- "I want RLM/FRLM improvements" → `crates/rlm/`, `crates/frlm/`, and routing decisions in `crates/adjutant/`

## Doc pointers (non-repetitive)

- Implementation priorities + MVP gates: [ROADMAP.md](./ROADMAP.md)
- Current status / what's actually wired: [SYNTHESIS_EXECUTION.md](./SYNTHESIS_EXECUTION.md)
- Architecture & strategy: [SYNTHESIS.md](./SYNTHESIS.md)
- Architecture decisions: [docs/adr/](./docs/adr/) (ADRs for contracts and invariants)
- Formal paper: [PAPER.md](./PAPER.md)
