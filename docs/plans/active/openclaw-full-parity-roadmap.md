# OpenClaw Full Parity Roadmap for OpenAgents

Date: 2026-02-19  
Status: Active maintenance plan after M1-M7 delivery  
Owner: OpenAgents runtime + control plane  
Depends on:

- `docs/plans/active/openclaw-code-ingestion-strategy.md`
- `docs/plans/active/elixir-agent-runtime-gcp-implementation-plan.md`
- `docs/plans/active/elixir-ds-runtime-integration-addendum.md`

## Goal

Define the exact path to "100% OpenClaw parity" in OpenAgents, including what we should import, what we should not import, and how to land parity without derailing OpenAgents architecture.

## Delivery narrative from recent issue wave

The recent parity wave was implemented through issues `#1740` through `#1746`, with duplicate issue numbers `#1734` through `#1739` closed quickly after triage so the execution sequence stayed single-tracked. This work also built directly on the earlier fixture-harness issue `#1716`, which established the baseline ingestion and parity capture shape before milestone execution began. The actual implementation path followed the intended milestone order and landed in commits `017faeedb`, `00e10514b`, `5982934cb`, `da32fdc6d`, `70e2826e8`, `6e400d5f8`, and `ba77b4abc`, each one mapped to a single parity milestone and verified before the next one started. This means the roadmap is no longer aspirational for M1 through M7; it is now an operations and maintenance document describing what was shipped, how it works in production, and where remaining parity risk still exists.

The tool policy milestone moved OpenAgents from helper-level parity into full pipeline parity by adding a runtime policy pipeline that composes profile, provider, global, agent, and group scopes with deterministic warning behavior for unknown allowlist entries. The implementation was anchored in `apps/openagents-runtime/lib/openagents_runtime/tools/policy/openclaw_tool_policy_pipeline.ex`, with fixture-driven parity coverage expanded through `apps/openagents-runtime/test/fixtures/openclaw/tool_policy_parity_cases.json` and the capture script in `apps/openagents-runtime/scripts/capture_openclaw_tool_policy_parity.mjs`. The effect of this work is that policy decisions are now computed in a layered, reproducible way that matches OpenClaw semantics, and policy outcomes remain receipt-visible for downstream auditing.

The loop and circuit-breaker milestone then made run continuation deterministic under no-progress conditions by adding OpenClaw-equivalent detectors in `apps/openagents-runtime/lib/openagents_runtime/runs/loop_detection.ex` and integrating the decision point directly into executor frame processing in `apps/openagents-runtime/lib/openagents_runtime/runs/executor.ex`. The runtime now emits `run.loop_detected` with detector metadata and transitions to a terminal failed state using reason class `loop_detected` before new unsafe work is started. This eliminated the previous gap where repeated tool/model churn could continue without a deterministic breaker event.

The hook lifecycle milestone introduced a controlled extension seam through `apps/openagents-runtime/lib/openagents_runtime/hooks/runner.ex`, with typed phases for model resolution, prompt build, tool call boundaries, and message persistence boundaries. The ordering contract is deterministic by priority and identifier, merge behavior for modifying phases is explicit, and bounded failure behavior is encoded so hook exceptions surface as receipt-visible errors rather than taking down execution semantics. The runtime now records both successful hook applications and hook failures as durable run events, which means replay and incident analysis can reconstruct hook influence without reading transient logs.

The network guard milestone established a single outbound HTTP security seam in `apps/openagents-runtime/lib/openagents_runtime/tools/network/guarded_http.ex` and wired comms provider execution to pass through that seam. The guard applies private and metadata endpoint blocking, allowlist checks, DNS pinning across redirects, and bounded redirect controls with deterministic reason codes, and those reasons are preserved in provider results so policy evaluation can make explicit deny decisions with the original SSRF context still attached. The rollout is controlled by `OA_RUNTIME_GUARDED_OUTBOUND_HTTP_ENABLED`, which keeps enforcement switchable while preserving a single implementation path.

The manifest parity milestone generalized extension validation from comms-only logic to a registry-based runtime activation contract. The strict validator in `apps/openagents-runtime/lib/openagents_runtime/tools/extensions/manifest_validator.ex` now enforces required identity and schema fields and emits machine-readable validation failures, and `apps/openagents-runtime/lib/openagents_runtime/tools/extensions/manifest_registry.ex` performs base validation plus pack-specific checks before activation is allowed. This shifted extension loading from permissive runtime assumptions to explicit contract enforcement that can be surfaced in both runtime telemetry and control-plane operator views.

The structured workflow milestone delivered DS-native orchestration parity by implementing `llm_task.v1` and `timeline_map_reduce.v1` workflows in `apps/openagents-runtime/lib/openagents_runtime/ds/workflows/structured_tasks.ex` and expanding signatures in `apps/openagents-runtime/lib/openagents_runtime/ds/signatures/catalog.ex`. The workflows enforce typed inputs and outputs, bounded step and budget controls, and deterministic workflow receipts that include strategy identifiers, trace references, and replay hashes. This is the part of parity that replaced prompt-glue orchestration with runtime-governed DS behavior so map/reduce style execution can be inspected, replayed, and budget-audited.

The observability and drift-hardening milestone completed parity failure telemetry by adding `apps/openagents-runtime/lib/openagents_runtime/telemetry/parity.ex` and class-specific emission paths for `policy`, `loop`, `network`, `manifest`, and `workflow` failures. Metrics stayed cardinality-safe through controlled tags in `apps/openagents-runtime/lib/openagents_runtime/telemetry/metrics.ex`, and drift automation was hardened by upgrading `scripts/openclaw-drift-report.sh` and `.github/workflows/openclaw-drift-report.yml` to support strict actionable gating. In practical terms, parity drift is no longer silent because CI can now fail on actionable rows and produce direct follow-up issue commands.

## Core decision: what "100% parity" means here

"100% parity" for OpenAgents means **runtime-kernel behavior parity** with OpenClaw in the capability clusters that matter to autonomous execution quality, safety, and replayability.

It does **not** mean cloning every OpenClaw product surface (gateway channels, local app UX, onboarding wizard, or control UI). Literal repo-level parity would be the wrong target for OpenAgents and would add large operational drag.

## Constraints

1. OpenAgents runtime remains authoritative for execution semantics (`apps/openagents-runtime`).
2. Laravel remains control plane and product surface (`apps/openagents.com`).
3. Replayability and receipt visibility are non-negotiable (`docs/execution/README.md`, `docs/protocol/README.md`).
4. Imported behavior must preserve OpenAgents security invariants and DS policy receipts.
5. OpenClaw capability drift must remain tracked via pinned SHAs and intake artifacts.

## Non-goals

1. Rebuilding OpenClaw gateway channel matrix inside OpenAgents runtime.
2. Porting OpenClaw desktop/local-first UX and onboarding flows.
3. Maintaining compatibility with every OpenClaw extension package by default.
4. Duplicating OpenClaw repo layout inside OpenAgents.

## Include vs "nah"

### Include (required for parity target)

1. Tool policy layering and normalization semantics.
2. Loop/no-progress detection with deterministic breaker reasons.
3. Hook lifecycle model for pre/post execution phases.
4. SSRF-safe, DNS-pinned outbound HTTP guard seam.
5. Strict integration manifest/schema validation for runtime-loaded tool packs.
6. Memory provider abstraction patterns and compaction contracts.
7. Structured orchestration patterns (`llm-task`/Lobster-like flows) as DS-runtime signatures/jobs.
8. OTel-aligned observability taxonomy and run/tool/session diagnostics.

### Nah (out of scope for this parity plan)

1. Channel plugin matrix (Discord/Telegram/WhatsApp/etc.) as first-class runtime parity work.
2. OpenClaw web control UI parity.
3. OpenClaw local node/device pairing UX parity.
4. OpenClaw onboarding/wizard feature parity.
5. Copying OpenClaw's CLI product surface into OpenAgents.

### Parking lot (only if product direction changes)

1. Selective channel adapter imports where OpenAgents has explicit product demand.
2. Optional extension packaging/distribution UX aligned with OpenAgents control plane.

## Current baseline in OpenAgents (2026-02-19)

| Capability cluster | OpenAgents status | Evidence | Gap to parity |
|---|---|---|---|
| Tool policy pipeline parity | Implemented | `apps/openagents-runtime/lib/openagents_runtime/tools/policy/openclaw_tool_policy_pipeline.ex`, `apps/openagents-runtime/test/openagents_runtime/parity/openclaw_tool_policy_parity_test.exs`, `apps/openagents-runtime/docs/OPENCLAW_POLICY_PARITY_REPORT.md` | Keep upstream fixture SHA pinning current and refresh parity vectors on drift |
| Loop detection + circuit breaker | Implemented | `apps/openagents-runtime/lib/openagents_runtime/runs/loop_detection.ex`, `apps/openagents-runtime/lib/openagents_runtime/runs/executor.ex`, `apps/openagents-runtime/test/openagents_runtime/runs/loop_detection_test.exs` | Tune detector thresholds under production load traces without changing deterministic reason taxonomy |
| Hook lifecycle runner | Implemented | `apps/openagents-runtime/lib/openagents_runtime/hooks/runner.ex`, `apps/openagents-runtime/lib/openagents_runtime/runs/executor.ex`, `apps/openagents-runtime/test/openagents_runtime/hooks/runner_test.exs` | Expand hook catalogue only through typed contracts and replay-safe event emission |
| SSRF + fetch guard seam | Implemented (feature-gated) | `apps/openagents-runtime/lib/openagents_runtime/tools/network/guarded_http.ex`, `apps/openagents-runtime/lib/openagents_runtime/tools/comms/providers/resend_adapter.ex`, `apps/openagents-runtime/test/openagents_runtime/tools/network/guarded_http_test.exs` | Continue rollout hardening and ensure all future runtime HTTP egress uses guarded seam only |
| Integration manifest validation | Implemented | `apps/openagents-runtime/lib/openagents_runtime/tools/extensions/manifest_validator.ex`, `apps/openagents-runtime/lib/openagents_runtime/tools/extensions/manifest_registry.ex`, `docs/protocol/extensions/extension-manifest.schema.v1.json` | Add more pack-specific validators as new extension families are onboarded |
| Memory compaction model | Partial/strong | `apps/openagents-runtime/lib/openagents_runtime/memory/*` | Align provider-slot abstraction and expansion ergonomics with OpenClaw memory-provider patterns |
| Structured orchestration (`llm-task`/Lobster shape) | Implemented for core workflows | `apps/openagents-runtime/lib/openagents_runtime/ds/workflows/structured_tasks.ex`, `apps/openagents-runtime/lib/openagents_runtime/ds/signatures/catalog.ex`, `apps/openagents-runtime/test/openagents_runtime/ds/workflows/structured_tasks_test.exs` | Broaden strategy coverage and add workflow families only when contract and budget semantics are explicit |
| Observability parity + drift hardening | Implemented | `apps/openagents-runtime/lib/openagents_runtime/telemetry/parity.ex`, `apps/openagents-runtime/lib/openagents_runtime/telemetry/metrics.ex`, `scripts/openclaw-drift-report.sh`, `docs/plans/active/openclaw-drift-process.md` | Resolve current actionable drift rows and maintain strict CI drift gate discipline |

## Milestones

1. **M1: Policy parity completion**
2. **M2: Loop detection parity**
3. **M3: Hook lifecycle parity**
4. **M4: Network guard parity**
5. **M5: Manifest/extension parity**
6. **M6: Workflow parity (`llm-task`/Lobster semantics via DS runtime)**
7. **M7: Observability parity + drift automation hardening**

## Implementation notes

### M1: Policy parity completion

Scope:
- Complete OpenClaw-equivalent policy pipeline composition (profile/provider/global/agent/group).
- Preserve additive plugin-group expansion and unknown-allowlist warnings semantics.
- Keep decisions receipt-visible in DS evaluator.

Primary locations:
- `apps/openagents-runtime/lib/openagents_runtime/tools/policy/*`
- `apps/openagents-runtime/test/openagents_runtime/parity/*`

### M2: Loop detection parity

Scope:
- Implement detector state and evaluation in run/tool execution path.
- Emit reason-coded events consumed by DS policy evaluator:
  - `loop_detected.no_progress`
  - additional OpenClaw-equivalent detector reason classes

Primary locations:
- `apps/openagents-runtime/lib/openagents_runtime/runs/*`
- `apps/openagents-runtime/lib/openagents_runtime/tools/*`
- `apps/openagents-runtime/lib/openagents_runtime/ds/policy_reason_codes.ex`

### M3: Hook lifecycle parity

Scope:
- Define typed hook points for:
  - before model resolve
  - before prompt build
  - before/after tool call
  - before message write/persist
- Support deterministic ordering and explicit merge rules for modifying hooks.
- Force side-effecting hooks to emit receipt-visible events.

Primary locations:
- `apps/openagents-runtime/lib/openagents_runtime/hooks/*` (new)
- `apps/openagents-runtime/lib/openagents_runtime/runs/*`
- `apps/openagents-runtime/lib/openagents_runtime/ds/*`

### M4: Network guard parity

Scope:
- Introduce single outbound HTTP seam in runtime tools:
  - allowed-host pattern matching
  - private-network/metadata IP blocking
  - DNS pinning
  - bounded redirects
  - audit events on blocked attempts
- Block ad hoc outbound HTTP outside this seam.

Primary locations:
- `apps/openagents-runtime/lib/openagents_runtime/tools/network/*` (new)
- tool adapters that perform HTTP fetches

### M5: Manifest/extension parity

Scope:
- Extend current comms manifest validation to generalized extension manifest schema:
  - required ID + schema contract
  - optional capabilities and UI hints
  - strict validation before runtime load
- Maintain control-plane visibility in Laravel for manifest errors and status.

Primary locations:
- runtime extension registry/validator modules
- `docs/protocol/*` schemas
- Laravel integration surfaces under `apps/openagents.com/app/AI/Runtime/*`

### M6: Workflow parity via DS runtime

Scope:
- Provide a DS-native equivalent to OpenClaw's structured orchestration tools:
  - JSON-constrained task execution
  - bounded model/tool budgets
  - deterministic receipt/tracing for orchestration steps
- Implement as runtime signatures/jobs, not shell-coupled subprocess tools.

Primary locations:
- `apps/openagents-runtime/lib/openagents_runtime/ds/signatures/*`
- `apps/openagents-runtime/lib/openagents_runtime/ds/predict.ex`
- `apps/openagents-runtime/lib/openagents_runtime/ds/compile/*`

### M7: Observability parity + drift hardening

Scope:
- Complete parity telemetry for run/tool/queue/session failure classes.
- Keep OpenClaw drift checks active and actionable with issue generation workflow.

Primary locations:
- `apps/openagents-runtime/lib/openagents_runtime/telemetry/*`
- `apps/openagents-runtime/docs/OBSERVABILITY.md`
- `docs/plans/active/openclaw-drift-process.md`
- `docs/plans/active/openclaw-drift-report.md`

## Verification

Run at minimum after each milestone:

```bash
# Runtime tests (including parity fixtures)
cd apps/openagents-runtime
mix test

# Contract convergence checks
mix runtime.contract.check

# OpenClaw drift report (repo root)
cd ../..
scripts/openclaw-drift-report.sh
```

Success criteria:
1. Relevant parity fixtures pass and remain pinned to explicit upstream SHA.
2. New parity capability emits deterministic reason-coded events/receipts.
3. No bypass path exists around runtime policy/security seams.
4. Control-plane contract docs stay convergent with implementation.

## Rollback

1. Feature-flag each parity cluster at runtime boundary.
2. Keep last-known-good behavior path for each milestone until parity slice is stable.
3. If parity regression appears, disable milestone flag and rely on previous stable path while preserving event-log continuity.

## Decision log

- 2026-02-19: Defined OpenAgents parity target as runtime-kernel parity (not full OpenClaw product parity).
- 2026-02-19: Locked include vs "nah" scope to prevent roadmap inflation and architecture drift.
- 2026-02-19: Completed M1-M7 implementation wave through issues `#1740` to `#1746` and transitioned this document from delivery planning to parity operations and maintenance.

## How to use the shipped parity surface

The current parity surface is designed to be exercised through the runtime harness first and then observed through receipts and telemetry, rather than treated as a black box. In daily engineering work, the most reliable entry point is to run the runtime test harness in `apps/openagents-runtime`, then run contract checks, and finally regenerate drift output from repository root so policy, workflow, extension, and observability paths are all validated together. Teams integrating new runtime tools should route outbound HTTP through `OpenAgentsRuntime.Tools.Network.GuardedHTTP`, register extension manifests through `OpenAgentsRuntime.Tools.Extensions.ManifestRegistry`, and rely on DS structured workflows for bounded orchestration instead of ad hoc control loops. Operator usage should treat parity telemetry as the first debugging source because failure classes are now normalized across policy denials, loop breaks, network blocks, manifest rejection, and workflow errors.

The intended usage pattern for control-plane and runtime collaboration is to let Laravel continue owning authoring and operator UX while runtime enforces canonical behavior and emits machine-readable evidence of what happened. When a policy decision or workflow branch is questioned, the answer should come from run events and receipts rather than from informal logs, because that is the path that preserves replayability and future regression testing. The same principle applies to extension onboarding: manifests should be iterated against schema contracts before activation, and any validation failure should be treated as a contract mismatch rather than a runtime exception to work around.

## What should happen next

The immediate next operational priority is to clear actionable drift by replacing any pending pins with exact upstream SHAs and then deciding, capability by capability, whether upstream change requires a fixture refresh or can remain pinned with an explicit rationale. The broader product priority is to keep OpenClaw parity focused on kernel behavior and avoid accidental expansion into channel-matrix feature cloning that would dilute runtime hardening work. The major remaining parity-shaped gap in this roadmap is memory-provider behavior alignment, and that should be executed with the same pattern used in M1 through M7: freeze behavior with fixtures, port deterministic core logic, surface outcomes in receipts, and only then expose new controls through Laravel. If that sequence is maintained, parity stays auditable and cumulative rather than becoming a moving target.
