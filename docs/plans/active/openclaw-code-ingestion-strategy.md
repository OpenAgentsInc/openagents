# OpenClaw Capability Ingestion Strategy for OpenAgents

Date: 2026-02-19  
Status: Active strategy with first parity wave delivered  
Owner: OpenAgents runtime + web control plane  
Depends on:

- `docs/plans/active/elixir-agent-runtime-gcp-implementation-plan.md`
- `docs/plans/active/elixir-ds-runtime-integration-addendum.md`
- `docs/plans/active/elixir-epics.md`
- `docs/plans/active/openclaw-full-parity-roadmap.md`

## Purpose

Define a repeatable system for pulling proven capabilities from `~/code/openclaw` into OpenAgents, including:

1. where imported code should land,
2. when to port to Elixir vs keep/adapt TypeScript/PHP,
3. how to preserve provenance, safety, and runtime invariants,
4. how to ship in small, verifiable increments.

This replaces ad hoc copy/paste with an explicit intake pipeline.

## Strategic Fit

OpenAgents runtime direction is now clear:

- Elixir runtime (`apps/runtime`) owns long-running execution, tools, durable event logs, policy, and replayability.
- Laravel (`apps/openagents.com`) remains control plane + product surface.

OpenClaw has a mature capability surface in exactly the areas OpenAgents needs next:

- typed tool contracts and policy filtering,
- optional plugin tooling with manifest/schema validation,
- loop-detection guardrails,
- hardened web/network tooling (SSRF + DNS pinning),
- lifecycle hook model for policy/observability,
- memory-provider slot patterns.

## Recent implementation reality

This strategy has now been executed through a complete first delivery wave rather than remaining a theoretical intake process. The runtime team carried OpenClaw parity work through issues `#1740` to `#1746` in strict sequence, and each issue landed as an isolated commit with test and contract verification before moving to the next milestone. The result is that ingestion rules in this document have already been pressure-tested against real runtime code paths, including policy composition, loop detection, hook execution, network security boundaries, extension manifest governance, DS workflow orchestration, and parity telemetry classification. The practical value of that sequence is that every imported behavior was translated into a deterministic runtime contract instead of being treated as an opaque upstream copy.

The specific pattern that worked was to freeze behavior with fixtures first, implement the kernel seam in Elixir with explicit reason codes and receipts, and then wire control-plane or protocol surfaces only after runtime behavior was reproducible. This was visible in policy parity where fixture capture and pipeline tests were expanded before claiming parity, in network guard rollout where a guarded seam was installed before broad provider expansion, and in manifest governance where activation was blocked by schema validation rather than runtime assumptions. This same pattern is now the expected standard for any new OpenClaw intake item, because it produced stable merges and made post-merge verification straightforward.

The strategy also proved that duplication control matters in GitHub execution. The duplicate issue batch `#1734` through `#1739` was intentionally closed, and work proceeded through the single canonical chain to avoid split implementation drift. That incident should be treated as a process lesson: one capability, one active issue, one verification path, one closing commit, and one document update cycle.

## Platform Delineation and Routing

OpenAgents supports web, mobile, and desktop clients. Each imported capability must declare a layer and platform variance so routing decisions are deterministic.

Layer values:

1. `Protocol`: shared contracts, schemas, reason codes.
2. `Kernel`: runtime execution semantics and guardrails.
3. `Control plane`: Laravel-authored configuration, UX, and admin workflows.
4. `Client`: platform-specific presentation and local device behavior.

Platform variance values:

- `core_shared`: behavior must stay identical across web/mobile/desktop.
- `client_specific`: intentionally platform-specific behavior.
- `hybrid`: shared kernel behavior with client-specific adapters/presentation.

Routing rules:

1. If a capability affects execution ordering, spend/security, replay, or canonical event semantics, route to `Kernel` (plus `Protocol` if contracts are updated).
2. If a capability is primarily authoring, config, audit, or operator UX, route to `Control plane`.
3. If a capability only changes UI/device behavior, route to `Client`.
4. Mixed capabilities must split implementation by layer but keep one canonical kernel behavior path.

## Non-Negotiable Guardrails

Every imported capability must satisfy OpenAgents invariants:

1. Verification first: no merge without relevant lint/test/build/integration harness.
2. Typed contracts: request/response/tool schemas must be explicit and versioned.
3. Replayability: behavior-impacting decisions must be receipt-visible and reconstructable.
4. Security first: imported network/tooling paths must pass threat review before exposure.
5. Provenance and license hygiene: preserve upstream source/commit attribution and Apache-2.0 compatibility notes.
6. Runtime network purity: runtime-side outbound HTTP is only allowed through `tools/network/*` guard rails and must emit receipt-visible events.
7. Deterministic extensibility: hooks cannot bypass policy/spend controls and must preserve deterministic, replayable behavior.

## Port vs Adapt vs Adopt Decision Matrix

Use this matrix at classification time; if multiple rules match, choose the highest-priority rule in this order: `port` -> `adapt` -> `adopt`.

### `port` to Elixir runtime

Required when the capability affects any of:

- execution ordering,
- spend authorization or cost exposure,
- security boundaries,
- event-log semantics, receipts, or replay.

### `adapt` in Laravel control plane

Use when behavior is control-plane centric and does not alter runtime canonical execution:

- UI/CRUD/configuration flows,
- policy authoring or audit displays,
- integration management/operator workflows.

### `adopt` directly (rare)

Allowed only for low-risk assets that are runtime-agnostic and easy to verify:

- fixtures and test vectors,
- schemas/docs/examples,
- pure algorithms with parity harness coverage and no hidden side effects.

If a capability cannot satisfy the above safely in the current wave, classify as `defer`.

## Capability Inventory and Destination Mapping

Prioritized mapping from OpenClaw to OpenAgents:

| Capability cluster | OpenClaw references | Layer | OpenAgents destination | Decision |
|---|---|---|---|---|
| Tool policy pipeline (profiles, allow/deny, provider-specific filters, plugin groups) | `src/agents/tool-policy.ts`, `src/agents/tool-policy-pipeline.ts`, `src/agents/pi-tools.policy.ts` | `Kernel + Control plane` | `apps/runtime/lib/openagents_runtime/tools/policy/*` + Laravel admin/config controls | Port to Elixir core; Laravel as control-plane editor |
| Optional tool gating + plugin tool metadata | `src/plugins/tools.ts`, `src/plugins/registry.ts` | `Kernel` | Runtime tool registry + policy resolver | Port core logic; keep plugin loading model adapted |
| Hook lifecycle (before/after tool call, prompt/model hooks, message hooks) | `src/plugins/types.ts`, `src/plugins/hooks.ts`, `src/plugins/hook-runner-global.ts` | `Kernel` | `apps/runtime/lib/openagents_runtime/hooks/*` | Port as runtime extension seam, event-log integrated |
| Loop detection and no-progress circuit breaker | `src/agents/tool-loop-detection.ts`, `src/agents/pi-tools.before-tool-call.ts` | `Kernel` | Runtime execution guardrails in run executor/tool task state machine | Port now (high value for autonomous stability) |
| Web/network safety guard (SSRF, DNS pinning, redirect controls) | `src/infra/net/fetch-guard.ts`, `src/infra/net/ssrf.ts`, `src/agents/tools/web-fetch.ts` | `Kernel` | Runtime tool adapters for any outbound HTTP tooling | Port early before expanding external integrations |
| Plugin manifest + schema validation | `docs/plugins/manifest.md`, `src/plugins/manifest.ts`, `src/plugins/manifest-registry.ts`, `src/plugins/schema-validator.ts` | `Protocol + Kernel + Control plane` | OpenAgents integration manifest format + runtime validation + Laravel UI forms | Adapt model; preserve strict validation behavior |
| Memory provider slot model (`memory-core` vs `memory-lancedb`) | `extensions/memory-core/index.ts`, `extensions/memory-lancedb/index.ts`, `docs/concepts/memory.md` | `Kernel` | Runtime memory provider abstraction (`timeline`, compaction, recall adapters) | Adapt architecture; do not copy storage model 1:1 |
| Structured workflow tool (`llm-task`/Lobster pattern) | `extensions/llm-task/src/llm-task-tool.ts`, `docs/tools/llm-task.md`, `docs/tools/lobster.md` | `Kernel + Protocol` | DS-Elixir strategy/tool orchestration boundary | Adapt as DS signature strategy, not a direct shell-style runtime dependency |
| OTel diagnostics service plugin | `extensions/diagnostics-otel/src/service.ts` | `Kernel + Control plane` | Runtime telemetry emitters + ops dashboards | Reuse metric taxonomy ideas; implement natively in Elixir telemetry |

Deprioritized for now:

- channel-specific adapters and gateway/device stack (`extensions/slack`, `extensions/telegram`, etc.),
- local-first WS gateway control plane semantics that do not map to OpenAgents web-first architecture.

## Where Imported Code Goes

### 1) Runtime canonical behavior (`apps/runtime`)

Primary landing zone for imported logic:

- `lib/openagents_runtime/tools/`
- `lib/openagents_runtime/runs/`
- `lib/openagents_runtime/integrations/`
- `lib/openagents_runtime/ds/`

Recommended new module seams:

- `lib/openagents_runtime/tools/policy/` (allow/deny/profile/provider filters)
- `lib/openagents_runtime/tools/extensions/` (manifest + registration + validation)
- `lib/openagents_runtime/tools/network/` (SSRF-safe HTTP helpers)
- `lib/openagents_runtime/hooks/` (lifecycle hook runner + typed hook contracts)
- `lib/openagents_runtime/guards/loop_detection.ex` (or `runs/loop_detection.ex`)

### Runtime Purity Boundary (Required)

1. Runtime modules must not perform ad hoc outbound HTTP from arbitrary code paths.
2. All runtime HTTP egress flows through `lib/openagents_runtime/tools/network/*`.
3. Guard rails at this seam enforce allowlists, DNS pinning, and redirect policy.
4. Every allow/deny/result outcome emits machine-readable receipt events.
5. Bypassing this seam blocks rollout.

### 2) Laravel control plane (`apps/openagents.com`)

Laravel owns authoring, display, and operator controls:

- integration registry CRUD and validation UX,
- tool policy configuration UI,
- rollout/canary toggles,
- debug/audit views for runtime receipts and policy decisions.

Implementation points:

- `app/AI/Runtime/*`
- `app/AI/Tools/*` (during transition)
- `resources/js/pages/*` (admin/settings)

### 3) Contract source-of-truth (`docs` + runtime docs)

Imported capabilities that cross language boundaries must publish versioned schemas:

- `docs/protocol/*` and runtime contract docs,
- OpenAPI + JSON schema artifacts consumed by both Elixir and Laravel.

## Ingestion Operating Model

For each candidate capability, run this pipeline:

1. Intake record
   - capture upstream path(s), commit SHA, license note, capability summary, risk class,
   - assign `layer` and `platform_variance` values.
2. Classification
   - apply the decision matrix in this order: `port`, `adapt`, `adopt`, `defer`,
   - record the explicit rule(s) that triggered the classification decision.
3. Target decision
   - runtime core vs Laravel control plane vs shared contract/doc.
4. Parity harness design
   - fixtures/golden tests defining expected behavior before port starts.
5. Implementation
   - smallest shippable slice behind feature flag.
6. Verification
   - unit/integration + contract checks + replay/receipt assertions.
7. Rollout
   - shadow/canary/default-on, with rollback path.

## Porting Playbook (TypeScript/PHP -> Elixir)

When behavior moves into runtime:

1. Freeze behavior with fixtures
   - extract canonical input/output pairs from upstream tests/docs.
2. Port pure logic first
   - policy evaluators, matchers, validators, loop detectors.
3. Port side-effect boundaries second
   - network calls, task execution, persistence integration.
4. Preserve error taxonomy
   - explicit, typed reason classes so Laravel and receipts remain stable.
5. Add deterministic test coverage
   - ExUnit unit tests + integration tests with frozen fixtures.
6. Wire receipts/events
   - decisions and failures must append durable events with machine-readable reason fields.

## Hook Constraints (Determinism + Safety)

Hooks are supported only under these constraints:

1. Hooks are pure transforms of `(context, event)` -> `(context', events[])` unless explicitly modeled side effects are emitted as receipt events.
2. Hook ordering is deterministic, versioned, and stable across replays.
3. Hooks cannot bypass tool policy evaluation, spend authorization, or security guard rails.
4. Hook-produced decisions that affect execution must emit reason-coded events.
5. Non-deterministic behavior (time/random/network) is prohibited unless fully captured as replay inputs.

## Governance and Provenance

Each imported capability must include:

- upstream repository and commit SHA,
- source file list,
- import date + owner,
- adaptation notes,
- security review checklist result.
- module-level attribution comment in ported code referencing upstream file path(s) + pinned commit SHA.
- vendored-license preservation for any directly adopted source files.

Recommended artifact format:

- `docs/plans/active/openclaw-intake/<capability-id>.md`

This keeps future syncs and audits tractable.

## Drift Reporting Cadence (Required)

OpenClaw imports must be monitored for upstream drift using pinned SHAs and fixture metadata.

- Drift process doc: `docs/plans/active/openclaw-drift-process.md`
- Drift report output: `docs/plans/active/openclaw-drift-report.md`
- Drift report command: `scripts/openclaw-drift-report.sh`

Any report row marked `upstream_head_mismatch`, `missing_pin`, or `invalid_sha` requires an ingestion follow-up issue before the next release cycle.

## Security Review Gates (Required)

Before enabling imported code in production:

1. Network egress policy review (especially web fetch and external APIs).
2. Input sanitization and schema validation review.
3. Secret handling/redaction review.
4. Idempotency and replay behavior review.
5. Abuse/failure mode review (looping, retry storms, stuck reservations).

No gate, no rollout.

## Post-wave execution plan

The next ninety-day window should be treated as consolidation and expansion under the same ingestion discipline that already succeeded. The first part of that window should focus on provenance hardening and drift closure by replacing any pending or missing upstream pins with exact SHAs, refreshing parity fixtures where upstream changes materially affect behavior, and keeping strict drift gating enabled in CI so the parity surface cannot silently diverge. This is not administrative overhead; it is what keeps parity credible when upstream OpenClaw continues moving.

The second part should prioritize the remaining kernel-aligned gap in this strategy, which is memory-provider parity and ergonomics alignment. The implementation should begin from explicit fixture and behavior capture, then land deterministic runtime abstractions for provider slots and expansion semantics, and only then expose optional control-plane knobs in Laravel where operator visibility is required. If this order is reversed, the team will end up with UI-level configuration for behavior that has not yet been stabilized in the runtime, which is exactly what this strategy is meant to prevent.

The third part should continue DS-oriented workflow maturation as runtime-native contracts rather than ad hoc prompt chains. That means adding workflow families only when budgets, traceability, and replay semantics are explicit in receipts, and refusing scope that cannot meet those invariants yet. In parallel, control-plane integration should stay focused on inspection and governance surfaces that consume runtime outputs rather than attempting to duplicate runtime decision logic in PHP.

## How to use this strategy now

This document should now be used as an operational checklist for every OpenClaw intake proposal rather than as a one-time planning artifact. An engineer proposing a capability should start by creating or updating an intake record, defining the upstream source and SHA, and classifying the capability using the port, adapt, adopt, or defer rules before implementation begins. During implementation, the expectation is that parity harnesses and contract checks run alongside feature development, and merge approval depends on those results being green in the same pull request. After merge, drift reporting and issue tracking should be updated immediately so the provenance chain remains continuous.

Success for the current phase is not measured by the raw count of imported features; it is measured by whether imported features remain deterministic, replayable, and governable as the runtime and control plane evolve. If those properties hold while additional capabilities are onboarded, this strategy is working as intended.
