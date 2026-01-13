# OpenAgents: Agent Contract (READ THIS FIRST)

## What OpenAgents is

OpenAgents is a runtime + compiler + (optional) market for autonomous agents:

- Runtime: executes tool/sandbox/job actions, enforces schemas + retries, records replayable receipts.
- Compiler layer (dsrs / DSPy): expresses agent behavior as typed Signatures + Modules and optimizes them via metrics into policy bundles.
- RLM/FRLM: execution modes for out-of-core reasoning over large repos / long sessions.
- Market layer (Pylon + NIP-90 + relays): makes compute and sandbox execution purchasable with receipts and budgets.
- Verification (tests/builds): anchors correctness; everything else is optimization.

If you are writing code here, you are usually adding:
1) a capability (tool/job/provider/lane),
2) a policy (signature/module/routing),
3) measurement (metrics/labels/counterfactuals/eval).

---

## Authority and conflict rules (non-negotiable)

1) If documentation conflicts with code behavior: CODE WINS.
2) If terminology conflicts across docs: GLOSSARY WINS.
3) If implementation status conflicts across docs: prefer the crate sources + SYNTHESIS_EXECUTION.

Read and apply:
- Canonical terminology: ./GLOSSARY.md
- Current system guide (what is wired): ./SYNTHESIS_EXECUTION.md
- MVP priorities / acceptance: ./ROADMAP.md

---

## Required reading order (fast)

1) ./GLOSSARY.md
2) ./SYNTHESIS_EXECUTION.md
3) ./ROADMAP.md
4) ./PROJECT_OVERVIEW.md (product map; short)
5) ./AGENT_FOUNDATIONS.md (conceptual model; signatures/tools/metrics/RLM)

Then go crate-local:
- dsrs docs: crates/dsrs/docs/README.md + ARCHITECTURE.md + TOOLS.md + METRICS.md + OPTIMIZERS.md + EVALUATION.md
- Autopilot docs: crates/autopilot/docs/
- Autopilot-core flow: crates/autopilot-core/docs/EXECUTION_FLOW.md
- Protocol surface: docs/PROTOCOL_SURFACE.md

---

## Engineering invariants (ship-quality rules)

Verification first
- Do not claim success without running the relevant verification harness (tests/build/lint as appropriate).

No stubs policy
- Do not add TODO-only “NotImplemented”, placeholder returns, mock implementations in production paths.
- If it’s not ready, gate behind a feature flag or remove the code path.

Typed contracts everywhere
- If it gates a decision or action, make it a Signature (or signature-backed pipeline).
- Tools must have schemas; runtime validates schemas before execution.

Everything is logged and replayable
- Tool calls must emit deterministic hashes + receipts.
- Decisions must be recorded with counterfactuals when migrating from legacy heuristics.

Adapters do serialization/parsing only
- Adapters do not own validation/retry logic. Runtime (or meta-operators like Refine) owns retries/guardrails.

---

## “Where do I change things?” (map)

Use this to avoid scattering logic:

### dsrs (compiler layer)
- Signatures/modules/optimizers/metrics/tracing: crates/dsrs/
- Docs: crates/dsrs/docs/
- If you change signature semantics, update docs + ensure parsing/tests still pass.

### Adjutant (execution engine + DSPy decision pipelines)
- DSPy pipelines + session tracking + auto-optimization: crates/adjutant/
- Tool registry (local tools): crates/adjutant/src/tools.rs

### Autopilot (product surfaces)
- UI/CLI orchestration + user-facing flow: crates/autopilot/
- Core execution flow + replay impl: crates/autopilot-core/

### RLM / FRLM
- Local recursion tooling + signatures: crates/rlm/
- Federated recursion conductor + map-reduce: crates/frlm/

### Protocol / Marketplace plumbing
- Typed job schemas + hashing: crates/protocol/
- Node software (provider + host): crates/pylon/
- Relay (agent coordination): crates/nexus/

---

## Checklists (what to do when adding things)

### If you add a new decision point
- Create a Signature with confidence (if it routes/overrides).
- Confidence-gate behavior (fallback to legacy rules when low confidence).
- Record counterfactuals (DSPy output vs legacy output vs executed choice).
- Add outcome labeling (verification_delta, repetition, cost).
- Make it eligible for optimization targeting (rolling accuracy / impact).

### If you add a new tool
- Register it in the canonical tool registry.
- Provide a JSON schema for params; runtime validates before execution.
- Emit a receipt record:
  - tool, params_hash, output_hash, latency_ms, side_effects
- Bound outputs, add timeouts, deterministic failure modes.
- Add tests for schema, truncation, and error behavior.

### If you add a new provider / lane
- Add provider integration + health detection.
- Implement adapter formatting/parsing (no retries here).
- Add cost accounting (tokens/latency/msats).
- Make lane selection policy-driven (signature) and auditable.
- Add fallback/circuit breaker behavior.

### If you “improve performance”
- Don’t hand-tweak prompts inline.
- Convert the behavior into a signature/module, add a metric, compile into a policy bundle.
- Preserve rollback/canary path.

---

## Build + test quick commands (use these before claiming done)

Workspace:
```bash
cargo build --release
cargo test
````

Autopilot:

```bash
cargo build -p autopilot
cargo test  -p autopilot
cargo run   -p autopilot
```

Adjutant + dsrs:

```bash
cargo test -p adjutant
cargo test -p dsrs
```

Pylon:

```bash
cargo build --release -p pylon
cargo test -p pylon
./target/release/pylon doctor
```

Nexus (worker):

```bash
cd crates/nexus/worker
bun install
bun run deploy
```

---

## Artifact expectations (when you finish an agent session)

The canonical output of an autonomous run is the Verified Patch Bundle:

* PR_SUMMARY.md
* RECEIPT.json
* REPLAY.jsonl (or ReplayBundle + exporter until native REPLAY.jsonl is wired)

See:

* crates/dsrs/docs/ARTIFACTS.md
* crates/dsrs/docs/REPLAY.md
* ./ROADMAP.md (MVP gate: Verified Patch Bundle)

---

## Documentation pointers (don’t duplicate; link)

Core:

* ./GLOSSARY.md (canonical vocabulary)
* ./SYNTHESIS_EXECUTION.md (how the system works today)
* ./ROADMAP.md (what to build next; MVP gates)
* ./PROJECT_OVERVIEW.md (product + stack overview)
* ./AGENT_FOUNDATIONS.md (conceptual foundations and checklists)

DSPy/dsrs:

* crates/dsrs/docs/README.md
* crates/dsrs/docs/ARCHITECTURE.md
* crates/dsrs/docs/SIGNATURES.md
* crates/dsrs/docs/TOOLS.md
* crates/dsrs/docs/METRICS.md
* crates/dsrs/docs/OPTIMIZERS.md
* crates/dsrs/docs/EVALUATION.md

Protocol / network:

* docs/PROTOCOL_SURFACE.md
* crates/protocol/
* crates/pylon/
* crates/nexus/

---

## Final note

If you are uncertain whether something belongs in the runtime, dsrs, or a product crate:

* Prefer keeping policy in dsrs/adjutant (Signatures/Modules/Pipelines),
* Keep execution enforcement (schema validation, retries, receipts) in the runtime/tooling layer,
* Keep UI/UX wiring in product crates.

```

If you want, I can also produce a **second variant** that’s even shorter (pure “agent contract” + doc links, no product tables) to keep AGENTS.md under ~80 lines.
::contentReference[oaicite:0]{index=0}
```
