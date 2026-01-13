# OpenAgents Repository Map

This document is a **codebase orientation guide**: what lives where, how the crates relate, and where to start based on what you want to change.

For "what is OpenAgents" + Quick Start, see **[README.md](./README.md)**.
For values/mission, see **[MANIFESTO.md](./MANIFESTO.md)**.
For canonical terminology, see **[GLOSSARY.md](./GLOSSARY.md)**.

## The stack (at a glance)

OpenAgents is organized as a layered stack:

- **Products**: Autopilot, Onyx, GitAfter (user-facing surfaces)
- **Execution**: Adjutant + Autopilot loop (plan/act/verify)
- **Compiler layer**: dsrs (DSPy-style signatures/modules/optimizers)
- **Runtime + infra**: tools, sandboxes, logging, provider routing
- **Protocols**: Nostr messaging + typed job schemas + receipts (where applicable)

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
- `crates/autopilot/` — user-facing app + CLI for autonomous coding runs
- `crates/autopilot-core/` — core execution loop, replay/session plumbing
- `crates/onyx/` — local-first markdown editor
- `crates/gitafter/` — Nostr-native git collaboration surface (NIP-34)
- `crates/coder/` — Codex-focused terminal/app-server UI surface (if present)

### Execution + learning
- `crates/adjutant/` — execution engine + decision pipelines + session store + auto-optimization hooks
- `crates/dsrs/` — Rust DSPy (signatures/modules/optimizers/tracing/providers)

### Runtime + tooling
- `crates/runtime/` — tool mediation, sandbox boundaries, tick model (Plan 9-ish FS surface where applicable)
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
- "I want to add/optimize DSPy signatures" → `crates/dsrs/` (+ decision pipelines in `crates/adjutant/`)
- "I want better replay/artifacts" → `crates/autopilot-core/` + specs in `crates/dsrs/docs/REPLAY.md` and `crates/dsrs/docs/ARTIFACTS.md`
- "I want marketplace job schemas / verification" → `crates/protocol/` (+ any dispatchers)
- "I want RLM/FRLM improvements" → `crates/rlm/`, `crates/frlm/`, and routing decisions in `crates/adjutant/`

## Doc pointers (non-repetitive)

- Implementation priorities + MVP gates: [ROADMAP.md](./ROADMAP.md)
- Current status / what's actually wired: [SYNTHESIS_EXECUTION.md](./SYNTHESIS_EXECUTION.md)
- Architecture & strategy: [SYNTHESIS.md](./SYNTHESIS.md)
- Formal paper: [PAPER.md](./PAPER.md)
