# Pylon Plan: Narrow Provider Connector

Status: proposed  
Date: 2026-03-07  
Primary implementation context: issue `#3116` "Master Task: Compute Market full implementation program"

## Summary

This plan defines `Pylon` as an active near-term product program, not as a speculative future idea and not as a restore of the archived runtime.

The new definition is:

**Pylon is the standalone provider binary for the OpenAgents Compute Market.**

It is a narrow, installable, headless compute connector that can run on a spare laptop, desktop, workstation, or server, detect eligible local backends, expose standardized compute products, connect that supply to the OpenAgents network, execute supported jobs locally, emit delivery evidence, and track payouts.

Pylon should also be shaped so bounded `sandbox_execution` can become the next compute family on the same provider substrate without reopening the product boundary. That means declared sandbox profiles, explicit resource and policy limits, and receiptable execution evidence. It does not mean arbitrary host access or labor-mode autonomous task execution.

This plan does **not** change the current MVP ownership model:

- `Autopilot` remains the primary user-facing product and the active owner of the current provider loop in `docs/MVP.md`.
- `Nexus` remains the network authority and control plane.
- `#3116` remains the near-term implementation program for canonical compute-market truth.
- The archived `crates/pylon` must not be restored wholesale.

The correct framing is:

- `Autopilot` = product surface
- `Pylon` = standalone supply connector
- `Nexus` = authority/control plane

This document updates the older MVP-only posture that Pylon should not be treated as a separate product surface. That older posture remains correct for current implementation ownership inside `#3116`, but the product plan is now to split out a narrow standalone provider connector as part of the current compute-market execution wave, alongside the remaining open repo issue slate where possible and immediately after it where blocking dependencies require it.

## Timing And Priority

Pylon is not a "someday" follow-on. It should be treated as the next planned supply-side packaging step for the current compute-market program.

As of 2026-03-07, `gh issue list --state open` shows 41 open issues in this repo. Those issues currently break down into two active programs:

- 8 open Compute Market issues: `#3109` through `#3116`
- 33 open PM / Spacetime rollout issues: `#3071` through `#3103`

This plan should therefore be read as:

- do the canonical compute-market truth work in the current compute-market issue tranche
- do the extraction design and boundary prep alongside the current repo-wide open issue slate
- start the standalone `apps/pylon` implementation as soon as the blocking provider-substrate issues are complete
- finish `Pylon` alongside the tail of the current open slate or immediately after it if overlap would destabilize active desktop and PM work

The key point is sequencing, not delay: `Pylon` is part of the current delivery arc, not a detached future roadmap item.

## Why This Plan Exists

The current repo has two true things at once.

First, the MVP and active compute-market work are explicitly `Autopilot`-first. `docs/MVP.md`, `docs/OWNERSHIP.md`, and `docs/autopilot-earn/AUTOPILOT_EARN_RUNTIME_PLACEMENT_DECISION.md` keep provider runtime ownership in `apps/autopilot-desktop` for the current pass.

Second, the archived Pylon audit shows that the old runtime did contain useful provider patterns, but it failed as a clean product boundary because it grew into too many things at once: provider node, buyer client, wallet shell, host runtime, browser bridge, Codex shell, and RLM bundle.

What is missing is a plan that preserves the good part of Pylon without reviving its scope drift. That plan is:

- keep `#3116` landing inside the current MVP ownership boundaries
- make the provider substrate canonical and reusable there first
- land the narrow provider/runtime seam as `crates/openagents-provider-substrate`
- package that seam as `Pylon`

## What Pylon Is

Pylon v0 should own exactly these responsibilities:

1. Machine and provider initialization.
2. Backend detection, backend health checks, and extension-ready sandbox runtime/profile detection.
3. Compute-product and execution-profile derivation from detected supply.
4. Inventory publication and truthful provider lifecycle state.
5. Acceptance and execution of supported compute jobs inside declared backend or sandbox constraints.
6. Delivery evidence and receipt emission.
7. Earnings and payout-state tracking.
8. Local status, recent jobs, and failure visibility.

Pylon is a provider-side resource-plane node. It is not the whole product.

## What Pylon Is Not

These are explicit non-goals for the new Pylon:

- no buyer mode
- no sovereign-agent host mode
- no agent subprocess supervisor
- no browser or WebSocket bridge
- no Codex shell
- no RLM product surface
- no labor-mode autonomous task marketplace
- no local general-purpose workstation runtime bundle
- no broad wallet shell beyond what payout correctness requires
- no revival of archived `crates/pylon`

If a feature does not directly support "turn a machine into network-visible compute supply," it should stay out of Pylon.

## Relationship To The Current Repo

This plan is additive to the current repo direction, not a reversal of it.

### Near-term truth during `#3116`

- `apps/autopilot-desktop` continues to own mission control, onboarding, provider UX, payout UX, and buyer-facing product workflows.
- `apps/nexus-control` continues to own authority-side compute-market state and projections.
- `crates/openagents-kernel-core` and `crates/openagents-kernel-proto` continue to own reusable compute types, contracts, and generated wire shapes.
- `crates/nostr/*` and `crates/spark` continue to own transport and wallet primitives.

### Pylon path during and immediately after `#3116`

- A narrow shared provider substrate now exists as `crates/openagents-provider-substrate`.
- `apps/pylon` can be added as a standalone CLI/service binary on top of that substrate.
- `Autopilot` can embed the same substrate instead of maintaining an incompatible provider stack.

This means the right sequence is **canonize first, extract second, package third**, with extraction work starting during the current issue slate rather than being deferred indefinitely.

## Current Open-Issue Fit

Reviewing the current open issues changes the practical framing in four ways.

### 1. Closed groundwork should not be restated as pending

The first compute-market groundwork slice is already closed:

- `#3104` launch taxonomy, product families, and capability envelope
- `#3105` canonical earn-loop linkage
- `#3106` Apple Foundation Models launch backend work
- `#3107` durable compute authority persistence and read models
- `#3108` expanded proto packages and generated authority clients

Pylon should build on those completed decisions and implementations rather than treating them as still-open prerequisites.

### 2. The remaining open compute-market issues split into blockers, alignment work, and non-blockers

Open issues that Pylon should align with closely:

- `#3109` provider inventory and buyer spot procurement in `Autopilot`
- `#3110` delivery proofs, metering, and launch-family settlement evidence
- `#3115` observability, policy breakers, verification matrix, and rollout gates

Of those, `#3110` and `#3115` are the strongest blockers for a truthful public Pylon launch. `#3109` matters for shared provider truth and desktop integration, but its buyer-facing `Autopilot` UI scope is not itself a blocker for a narrow standalone provider node.

The current open compute-market issues are still launch-family scoped around `inference` and `embeddings`. `sandbox_execution` should therefore be treated as a Pylon-extension track that reuses the same provider substrate, receipt model, and rollout controls rather than creating a second provider stack.

### 3. Later market-layer issues must stay compatible but should not block Pylon v0

These open issues are important for the full Compute Market, but they should not block the first narrow provider connector:

- `#3111` forward physical capacity sales and substitution controls
- `#3112` compute indices and governance controls
- `#3113` bounded cash-settled hedging instruments
- `#3114` reservation rights, options, swaps, and strips

Pylon v0 should emit inventory, capability-envelope truth, delivery evidence, and receipts in ways that are compatible with those later layers. It should not wait for those layers to be product-complete before existing as a provider node.

### 4. The PM / Spacetime issue slate is parallel work, not a semantic dependency

The 33 open PM issues from `#3071` through `#3103` are a separate execution program around PM domains, Spacetime rollout, coordination, bounties, and cutover evidence. They are not direct product dependencies for Pylon, but they matter operationally because they share:

- `apps/autopilot-desktop`
- sync and replay expectations
- authority-boundary discipline
- truthful wallet and payout messaging

That means Pylon work should avoid broad app-shell churn and should not entangle provider extraction with PM domains, Spacetime coordination flows, or bounty settlement projections.

## Launch Scope Aligned With `#3116`

The new Pylon should inherit the same launch compute-market truth already being defined in `#3116`.

### Product families at launch

- `compute` remains the umbrella market category
- `inference` and `embeddings` are the first live compute product families

### Launch backend products

- `ollama.text_generation`
- `ollama.embeddings`
- `apple_foundation_models.text_generation`

### Explicitly not launch scope unless the repo truth changes

- `apple_foundation_models.embeddings`

Issue `#3116` and `docs/plans/compute-market-full-implementation-plan.md` currently frame Apple Foundation Models as an inference backend at launch. Pylon should not overclaim beyond that.

### Capability envelope, not product identity

Accelerator and machine traits belong in the capability envelope, not in the primary product identity. At minimum, the model should support fields such as:

- `backend_family`
- `execution_kind`
- `model_family` or `model_policy`
- `accelerator_vendor`
- `accelerator_family`
- `memory_gb`
- `platform_constraints`
- `concurrency_posture`
- `latency_posture`

This keeps the launch market honest. Pylon is publishing compute products backed by local runtimes, not pretending to broker a raw GPU market on day one.

### Planned next compute family: `sandbox_execution`

The current repo-wide launch tranche under `#3116` remains `inference` and `embeddings`, but the Pylon plan should explicitly reserve the next compute-family extension for bounded `sandbox_execution`.

The ratified contract surface for this family now lives in `docs/pylon/PYLON_SANDBOX_CONTRACT.md`. That document is the source of truth for the execution taxonomy, sandbox profile model, capability-envelope extensions, job-contract fields, receipt requirements, safety restrictions, and the hard Compute-vs-Labor boundary.

`sandbox_execution` means:

- bounded machine execution
- explicit runtime contract
- explicit resource limits
- explicit filesystem policy
- explicit network policy
- explicit artifact and output policy
- explicit timeout and termination policy
- machine-verifiable receipts and delivery evidence

It does not mean:

- arbitrary host access
- privileged execution
- hidden secrets access
- open-ended autonomous problem solving
- labor-style agent tasks disguised as compute

### Compute vs Labor boundary

The hard boundary is:

- If the buyer says "run this bounded thing in this declared sandbox," that is Compute.
- If the buyer says "figure out what to do and do it," that is Labor.

Pylon may execute the first kind of request. It must not quietly absorb the second.

### Initial sandbox execution classes

The first bounded execution classes should stay narrow:

- `sandbox.container.exec`
- `sandbox.python.exec`
- `sandbox.node.exec`
- `sandbox.posix.exec`

Optional later classes can exist, but the initial set should stay policy-first and independently matchable.

### Sandbox profile and capability extensions

The capability envelope should be able to describe sandbox supply as well as model-serving supply. In addition to the launch fields above, Pylon should carry support for fields such as:

- `sandbox_profile`
- `os_family`
- `arch`
- `cpu_cores`
- `disk_gb`
- `network_posture`
- `filesystem_posture`
- `toolchain_inventory`
- `container_runtime`
- `max_timeout_s`

Each sandbox execution offer should also carry a first-class sandbox profile. At minimum that profile should include:

- `runtime_family`
- `runtime_version`
- `sandbox_engine`
- `os_family`
- `arch`
- `cpu_limit`
- `memory_limit_mb`
- `disk_limit_mb`
- `timeout_limit_s`
- `network_mode`
- `filesystem_mode`
- `workspace_mode`
- `artifact_output_mode`
- `secrets_mode`
- `allowed_binaries`
- `toolchain_inventory`
- `container_image` or `runtime_image_digest`

### Sandbox execution contract, receipts, and policy

When `sandbox_execution` lands, the job contract should include explicit fields such as:

- `execution_class`
- `entrypoint_type`
- `payload_ref` or embedded payload
- `arguments`
- `environment_policy`
- `resource_request`
- `timeout_request_s`
- `network_request`
- `filesystem_request`
- `expected_outputs`
- `artifact_policy`
- `determinism_posture`

Sandbox execution receipts should be explicit enough to keep this family inside Compute rather than Labor. At minimum they should capture:

- provider identity
- compute product ID
- sandbox profile ID or digest
- runtime image or environment digest
- job input digest
- command or entrypoint digest
- start time
- end time
- exit code or termination reason
- stdout digest
- stderr digest
- artifact digests
- resource usage summary
- payout linkage
- verification or attestation posture

Hard restrictions should remain explicit:

- no privileged containers
- no host root mounts
- no undeclared filesystem access
- no undeclared network access
- no hidden secrets injection
- no arbitrary long-lived daemonization unless explicitly supported
- no silent persistence outside declared workspace or artifact paths

Implementation work for `sandbox_execution` should treat those rules as contract requirements rather than as optional planning notes.

## Dependencies On The `#3116` Program

The standalone Pylon path should be developed alongside the parts of `#3116` that establish canonical compute-market truth, but it should not ship ahead of the blocking substrate work.

### Groundwork already closed and should be reused, not reopened

- `#3104` launch compute taxonomy, product families, and capability envelope
- `#3105` canonicalize the current earn loop into compute-market inventory, receipts, and delivery linkage
- `#3106` revive or replace the Apple Foundation Models bridge for the launch inference backend
- `#3107` durable authority persistence and read models
- `#3108` generated compute-market proto packages and clients

### Open compute-market issues that matter most for Pylon v0 ship timing

- `#3110` automated delivery proofs, metering, and settlement evidence
- `#3115` observability, policy breakers, verification matrix, and rollout gates

These are the strongest blockers for a truthful standalone launch.

### Open compute-market issues that should align with Pylon but are not strict blockers for the standalone provider binary

- `#3109` productize provider inventory and buyer spot procurement in `Autopilot`
- `#3111` forward physical capacity sales and substitution controls
- `#3112` compute indices and governance controls
- `#3113` bounded cash-settled hedging instruments
- `#3114` reservation rights, options, swaps, and strips

`#3109` should share provider-runtime truth with Pylon. `#3111` through `#3114` are later market-layer extensions that Pylon must stay compatible with, but they should not delay the initial narrow provider connector.

### Work that remains `Autopilot`-owned even if Pylon exists

- provider onboarding UX
- mission control
- buyer spot procurement and quote flows
- rich wallet UX
- product panes and operator-facing market views

Pylon depends on the compute-market substrate work above. It does not absorb the `Autopilot`-owned product surfaces listed below.

In practical terms, this means:

- design, naming, CLI shape, and extraction boundaries can proceed now
- shared substrate extraction should begin once the relevant provider/runtime seams are stable
- the standalone binary should land alongside the end of the current repo issue slate or immediately after it, without blocking PM rollout work and without waiting for derivative-market issues to be fully productized

## Rollout Plan

### Phase 0: Keep `#3116` Autopilot-first

Do not create a standalone Pylon runtime by bypassing the current MVP ownership model.

Use the existing `#3116` program to make the provider substrate real inside the current repo shape:

- canonical compute products and capability envelope
- provider inventory truth
- delivery-proof emission
- durable authority state
- explicit backend health and product readiness
- truthful payout and receipt linkage

Output of this phase:

- the existing `Go Online` loop becomes canonical compute-market truth
- the provider substrate has stable seams
- the repo has enough truth to extract from without guessing

This phase is already in motion through the current issue slate and should continue in parallel with Pylon boundary planning.

### Phase 1: Extract Only The Narrow Provider Substrate

Once the `#3116` provider path is stable, extract only the reusable provider/runtime seam.

Good extraction candidates:

- backend detection
- backend health model
- compute-product derivation
- extension-ready sandbox runtime and profile detection seams
- provider lifecycle state machine
- local persistence schema and read model
- job execution adapters
- bounded sandbox execution adapters and policy hooks
- delivery-evidence emitters
- local control and status API

Do not extract:

- mission control state
- pane orchestration
- buyer workflows
- rich wallet UX
- app-specific onboarding or product copy

If a new shared crate is created later, it should be narrow and product-agnostic. It should not become a new monolith.

That extracted boundary should be shaped so `sandbox_execution` plugs into the same substrate through runtime/profile detection, bounded execution adapters, and receipt emission rather than through app-specific side paths.

This extraction should begin during the later part of the current issue slate, not after a long pause.

### Phase 2: Add `apps/pylon`

After the substrate exists, add `apps/pylon` as the standalone provider binary.

The first cut should be service-style and intentionally small:

- `pylon init`
- `pylon doctor`
- `pylon serve`
- `pylon status`
- `pylon online`
- `pylon offline`
- `pylon pause`
- `pylon resume`
- `pylon backends`
- `pylon inventory`
- `pylon products`
- `pylon jobs`
- `pylon earnings`
- `pylon receipts`
- `pylon config show`
- `pylon config set ...`

`pylon serve` plus `pylon status --json` is enough for the first real version. Daemon wrappers and service managers can come later.

This phase should be treated as the immediate follow-on ship target once the blocking substrate work is complete.

### Phase 3: Embed The Same Runtime In Autopilot

Once `apps/pylon` is real, `Autopilot` should use the same provider substrate rather than maintaining a forked provider truth.

The shared runtime should become the source of truth for:

- backend readiness
- product derivation
- provider lifecycle state
- recent jobs
- receipts and earnings linkage
- local control and diagnostics

`Autopilot` remains the richer product shell around that runtime.

## Proposed Repo Topology

### Current ownership during active `#3116` work

| Path | Responsibility |
| --- | --- |
| `apps/autopilot-desktop` | provider UX, mission control, onboarding, payout UX, buyer flows, current provider runtime ownership |
| `apps/nexus-control` | compute-market authority state, durable mutations, read models, settlement truth |
| `crates/openagents-kernel-core` | reusable compute types, authority client contracts, validation helpers, receipt helpers |
| `crates/openagents-kernel-proto` | generated compute-market wire contracts |
| `crates/nostr/*` | transport and protocol primitives |
| `crates/spark` | wallet primitives |

### Target topology once extraction is justified

| Path | Responsibility |
| --- | --- |
| `apps/pylon` | standalone provider CLI/service binary |
| `apps/autopilot-desktop` | product UX and embedding shell for the shared provider runtime |
| `crates/openagents-provider-*` or similar narrow shared crate(s) | provider substrate only: backend detection, lifecycle state machine, product derivation, local persistence, control API, evidence adapters |
| `apps/nexus-control` | authority/control plane |

The exact shared-crate name is less important than keeping the boundary narrow.

## Runtime Model

Pylon should use a service-style runtime with CLI control.

### Local state

Use local SQLite for:

- config metadata
- provider identity metadata
- provider state
- backend inventory snapshots
- detected sandbox profiles
- published products
- recent jobs
- delivery proofs and receipts
- payout history
- health and error events

### Local control surface

Expose one small machine-readable admin surface:

- Unix socket or localhost HTTP
- JSON responses
- usable by the Pylon CLI
- usable by `Autopilot` when embedding the same runtime

Minimum operations:

- status
- online or offline transition
- pause or resume
- backend health
- execution class and sandbox profile summary
- inventory summary
- recent jobs
- earnings summary
- receipt listing

## State Model

### Runtime states

- `unconfigured`
- `ready`
- `online`
- `paused`
- `draining`
- `degraded`
- `offline`
- `error`

### Backend health states

- `healthy`
- `unsupported`
- `unavailable`
- `misconfigured`
- `disabled`

Truthful status matters more than optimistic status.

### Sandbox job states

When `sandbox_execution` is enabled, execution-level states should remain explicit:

- `queued`
- `assigned`
- `running`
- `completed`
- `failed`
- `timed_out`
- `killed`
- `rejected`
- `verified`
- `settled`

## Acceptance Criteria

The first standalone Pylon deserves to exist only if all of the following are true:

1. It can be installed and initialized on a machine without `Autopilot`.
2. It can truthfully detect supported local backends and keep a clear path for declared sandbox runtime and profile detection.
3. It can expose launch compute products for `inference` and `embeddings` without overclaiming unsupported supply, and it is shaped so bounded `sandbox_execution` can land without a second provider stack.
4. It can go online, go offline, pause, and resume with explicit local state.
5. It can accept and execute supported compute jobs locally under declared backend or sandbox constraints.
6. It can emit delivery evidence and receipts linked to the canonical compute flow.
7. It can show recent jobs, earnings, and payout state locally.
8. `Autopilot` can embed the same provider substrate instead of maintaining a separate incompatible stack.
9. No buyer, host, bridge, Codex-shell, RLM, or labor-market scope creeps back in.

## Testing Expectations

The plan should be considered complete only if the eventual implementation proves:

- backend detection and product derivation are unit-tested
- sandbox profile detection and policy matching are unit-tested when that family is enabled
- provider lifecycle transitions are integration-tested
- local persistence is restart-safe
- delivery-proof emission is tied to real execution completion
- sandbox execution receipts capture declared profile, digests, termination data, and resource summary when that family is enabled
- `Autopilot` embedding and standalone `Pylon` both pass against the same provider-runtime contract
- rollout gates align with the verification matrix already called for in `#3115`

## Final Guardrail

The old Pylon failed because it tried to be the whole local OpenAgents universe.

The new Pylon should be small enough that its purpose is obvious:

**it turns a machine into truthful, network-visible compute supply, including bounded sandbox execution where policy allows.**
