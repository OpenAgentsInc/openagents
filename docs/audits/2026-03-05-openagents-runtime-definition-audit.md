# 2026-03-05 OpenAgents Runtime Definition Audit

Author: Codex
Status: Complete
Scope: current repo docs and code that use `kernel` and `runtime` in the Autopilot / Earn / economy sense

## Objective

Audit how the repo currently uses `kernel` and `runtime`, then propose a clear definition of an `OpenAgents Runtime` that is separate from the `OpenAgents Kernel`.

Important scope boundary:

- This audit is about the Autopilot / Earn / economy architecture.
- It does not attempt to rename or reinterpret CAD geometry-kernel usage under `crates/cad`, where `kernel` means something different.

## Executive Verdict

Yes. The repo already implies a real split between `runtime` and `kernel`, and it is useful to define that split explicitly.

Proposed short definition:

> The OpenAgents Runtime is the execution environment where agent work actually runs, local job state advances, and evidence/provenance is produced. The OpenAgents Kernel is the economic authority that verifies, settles, and emits canonical receipts.

In current MVP terms:

- `runtime` is mostly an embedded desktop/provider concern living in `apps/autopilot-desktop`.
- `kernel` is mostly an authority model and receipt schema, with the full server-side Kernel Authority API still planned rather than built in this repo.

## Sources Reviewed

Product and authority docs:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/EARN.md`
- `docs/AUTOPILOT_EARN_MVP.md`
- `docs/PANES.md`
- `docs/AUTOPILOT_EARN_RUNTIME_PLACEMENT_DECISION.md`
- `docs/adr/ADR-0001-spacetime-domain-authority-matrix.md`

Kernel-plan docs:

- `docs/plans/economy-kernel.md`
- `docs/plans/economy-kernel-proto.md`
- `docs/plans/diagram.md`

Recent kernel audits:

- `docs/audits/2026-03-05-economy-kernel-plan-vs-built-system-audit.md`
- `docs/audits/2026-03-05-kernel-plan-feedback-clarification-audit.md`

Current implementation references:

- `apps/autopilot-desktop/src/state/provider_runtime.rs`
- `apps/autopilot-desktop/src/sync_lifecycle.rs`
- `apps/autopilot-desktop/src/economy_kernel_receipts.rs`
- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`
- `apps/autopilot-desktop/src/app_state_domains.rs`

## What The Repo Means By `Kernel` Today

### 1. In the kernel-plan docs, `kernel` already means authority

`docs/plans/economy-kernel.md` and `docs/plans/diagram.md` are explicit:

- authority mutations happen over authenticated HTTP,
- `TreasuryRouter` and `Kernel Authority API` are server-side services,
- Nostr and Spacetime are non-authoritative coordination/projection lanes,
- the kernel emits canonical receipts and economy snapshots.

This is not execution-environment language. It is authority language.

### 2. In current desktop code, `kernel` shows up mainly as receipt authority

The strongest in-repo code signal is the earn receipt stream:

- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs` uses stream id `stream.earn_kernel_receipts.v1`.
- The same file hard-codes the authority marker `kernel.authority`.
- The desktop loads and persists a local economy-kernel receipt stream using kernel-shaped receipt objects from `apps/autopilot-desktop/src/economy_kernel_receipts.rs`.

That means current code already treats `kernel` as the namespace for authoritative economic truth, even though the full remote kernel service is not yet present.

### 3. The repo does not currently contain a real server-side kernel implementation

The earlier kernel plan-vs-built audit is still correct:

- there is no repo-local `TreasuryRouter`,
- no repo-local `Kernel Authority API`,
- no kernel HTTP service in code,
- no server-published canonical `/stats` surface in this workspace.

So in this repo today, `kernel` is mostly:

- a normative plan,
- a local receipt model,
- a local mirror/prototype of future authority semantics.

## What The Repo Means By `Runtime` Today

### 1. `runtime` consistently points at execution and operational state

The most direct signals are in `apps/autopilot-desktop`:

- `ProviderRuntimeState` models `offline / connecting / online / degraded`, heartbeat freshness, queue depth, last result, and runtime failure classes.
- `RuntimeSyncConnectionState` and `RuntimeSyncLifecycleManager` model connection, replay, token refresh, backoff, and reconnect behavior.
- `docs/MVP.md` says `Go Online` initializes the embedded Autopilot provider runtime and provider identity.
- `docs/AUTOPILOT_EARN_RUNTIME_PLACEMENT_DECISION.md` keeps provider runtime ownership in `apps/autopilot-desktop`.
- `docs/PANES.md` uses `runtime` as the source for provider status, relay connections, network requests, alerts, job intake, and other execution-facing panes.

This is all “where work runs / how the app stays live” language.

### 2. `runtime` also already means execution provenance

`apps/autopilot-desktop/src/economy_kernel_receipts.rs` defines `ProvenanceAttestationKind::RuntimeIntegrity`.

That only makes sense if `runtime` is the execution environment whose integrity can be attested and then evaluated by something else.

This is a strong boundary clue:

- the runtime produces or carries the attestation,
- the kernel consumes that attestation when making economic decisions.

### 3. `runtime` is used for local event recording, not final authority

`apps/autopilot-desktop/src/app_state_domains.rs` exposes `record_runtime_event(...)`.

That usage fits execution telemetry and simulations. It does not imply final settlement or authority.

### 4. There is no single `OpenAgentsRuntime` type yet

Today the concept is distributed across:

- provider mode state,
- sync lifecycle state,
- job execution state,
- runtime integrity attestations,
- execution/projection panes,
- local runtime event recording.

So the architecture already has a runtime concept, but not yet one named umbrella abstraction.

## Boundary Audit: The Split Already Exists

The current repo makes the following split whether or not it says so explicitly.

| Concern | Runtime | Kernel |
| --- | --- | --- |
| Job execution | Yes | No |
| Tool invocation / local compute | Yes | No |
| Heartbeat / online state | Yes | No |
| Relay connectivity / sync health | Yes | No |
| Evidence and provenance production | Yes | No |
| Runtime integrity attestation | Yes | Evaluates it |
| Verification verdict finalization | No | Yes |
| Settlement truth | No | Yes |
| Liability / warranty / claim truth | No | Yes |
| Canonical economic receipt authority | No, but may record local mirrors | Yes |
| Public economy snapshots / `/stats` authority | No, but may compute local mirrors | Yes |

This is the key architectural point:

- The runtime has execution truth.
- The kernel has economic truth.

Those are related, but they are not the same thing.

## Proposed Definition

### Short Definition

The `OpenAgents Runtime` is the worker-side execution environment that runs agent work, manages execution-local state, and produces the evidence, outputs, and attestations the kernel later evaluates.

The `OpenAgents Kernel` is the authority layer that applies policy, verifies outcomes, settles value, and emits canonical receipts and snapshots.

### Longer Definition

The runtime should be understood as the combination of:

- the provider’s local execution environment,
- any job sandbox or tool runner it uses,
- the runtime-local control loop for accepting, running, and delivering work,
- the runtime-local telemetry and health state needed to operate safely,
- the provenance/evidence generation path for what happened during execution.

The runtime may decide operational questions such as:

- am I online,
- can I accept this job,
- what failed locally,
- what evidence and outputs were produced,
- what runtime-integrity attestations can I attach.

But the runtime should not be described as the final authority for:

- whether the work is economically accepted,
- whether verification is sufficient,
- whether payment is released,
- whether liability attaches,
- whether a receipt is canonical.

Those belong to the kernel.

## MVP Mapping

For the current MVP repo, the practical mapping is:

- `OpenAgents Runtime` = the embedded provider/runtime behavior in `apps/autopilot-desktop`
- `OpenAgents Kernel` = the planned server-authoritative economy system, currently represented locally by receipt/snapshot models and kernel-plan docs

That matches:

- `docs/AUTOPILOT_EARN_RUNTIME_PLACEMENT_DECISION.md`
- `docs/PANES.md`
- `docs/MVP.md` section 9.2
- `docs/plans/economy-kernel.md`
- `docs/plans/diagram.md`

## What The Runtime Owns

If this term is adopted, the OpenAgents Runtime should own:

- execution of local or delegated work,
- provider lifecycle state,
- job intake/accept/reject/run/deliver behavior,
- execution telemetry and runtime health,
- runtime-facing sync and coordination state,
- provenance bundles, tool traces, and execution attestations,
- local replay-safe caches and projections needed for UX.

Examples in the current repo:

- `ProviderRuntimeState`
- `RuntimeSyncLifecycleManager`
- job inbox / active job / provider lane behavior
- `RuntimeIntegrity` provenance attestations
- runtime-facing panes with `source: runtime`

## What The Runtime Does Not Own

The runtime should not own:

- final verification verdicts,
- settlement release truth,
- warranty/claims/dispute truth,
- canonical economy receipt issuance,
- policy-bundle authority,
- authoritative public economy snapshots.

If the runtime writes local receipts in MVP, those should be described as:

- local kernel-shaped mirrors or prototypes,
- client-side receipt recording compatible with future kernel authority,
- not proof that the runtime and kernel are the same component.

## Recommended Terminology

### Preferred naming

Use:

- `OpenAgents Runtime` for the execution environment
- `OpenAgents Kernel` or `Economy Kernel` for the authority layer

Avoid:

- `economy runtime`, because it sounds like money authority
- using `kernel` as a synonym for “where jobs run”
- using `runtime` as a synonym for “final truth”

### Canonical wording suggestion

Suggested sentence for future docs:

> OpenAgents Runtime is the execution environment on the user or provider node where jobs run and provenance is produced; OpenAgents Kernel is the authority layer that verifies outcomes, settles value, and emits canonical receipts.

## Related Boundary Clarifications

### Nostr is not the kernel

Per current docs, Nostr is the open market and coordination substrate:

- request/result transport,
- discovery,
- portable protocol artifacts.

It is not final authority for settlement or verification.

### Spacetime is not the kernel

Per `docs/adr/ADR-0001-spacetime-domain-authority-matrix.md`, Spacetime is authoritative only for named app-db domains such as:

- presence/liveness,
- replay checkpoints,
- non-monetary projections,
- derived counters on those approved domains.

That is not the same as kernel authority.

### Wallet executor is adjacent, but distinct

The kernel plans correctly treat wallet execution as the custody boundary. The runtime can invoke wallet-facing flows and the kernel can depend on wallet proofs, but wallet custody is still its own trust boundary.

## Documentation Gaps Found During Audit

### 1. `apps/runtime` language in `docs/MVP.md` is stale

`docs/MVP.md` still says:

- `apps/runtime` remains the retained authority for execution boundaries and projection publishing.

But this pruned repo has only `apps/autopilot-desktop`, and `docs/AUTOPILOT_EARN_RUNTIME_PLACEMENT_DECISION.md` explicitly keeps provider runtime ownership in that app.

That does not block defining the runtime, but it does mean current docs already need a terminology cleanup.

### 2. The runtime concept is real, but implicit

The repo has:

- runtime state,
- runtime panes,
- runtime lifecycle,
- runtime attestations,
- runtime event logs.

What it lacks is one explicit doc sentence saying what `runtime` means in OpenAgents.

### 3. `kernel` and `runtime` are currently separated by usage, not by glossary

The split is visible if you read across docs and code, but it is not yet written down as a first-class architectural distinction. That is the gap this definition closes.

## Final Recommendation

Adopt `OpenAgents Runtime` as a first-class term.

Definition:

- Runtime = where work runs and execution evidence is produced.
- Kernel = who has final authority over verification, settlement, and receipts.

Deployment implication:

- one kernel may govern many runtimes,
- runtimes may be embedded desktop runtimes in MVP,
- future runtimes may also include dedicated worker or sandbox environments,
- but the authority boundary should remain kernel-side.

This naming fits the codebase as it exists today, reduces ambiguity in the Earn/kernel docs, and creates a clean mental model for future server-authoritative kernel work without pretending the runtime already is that authority.
