# Autopilot Earn Runtime Placement Decision

Date: 2026-03-04
Issue: #2819

## Decision

Provider runtime ownership remains in `apps/autopilot-desktop`.

A narrow shared provider substrate crate, `crates/openagents-provider-substrate`, is introduced for reusable provider semantics. It does not change app ownership of provider UX, orchestration, or payout behavior.

Scope note: this decision is for the compute-provider MVP lane and does not pre-decide future liquidity-solver module placement.

Terminology note:

- `OpenAgents Runtime` means the execution environment where jobs run, local runtime state advances, and provenance is produced.
- `OpenAgents Kernel` means the authority layer that verifies outcomes, settles value, and emits canonical receipts.

## Rationale

- `../OWNERSHIP.md` assigns product workflow/state authority to `apps/autopilot-desktop`.
- Existing runtime state lanes (`provider_runtime`, `job_inbox`, `active_job`, `job_history`, `wallet_reconciliation`) are already app-owned and integrated with UI behavior.
- Pulling archived `pylon`/`compute` crates back into this pruned repo would re-expand scope and violate MVP simplification goals.
- The extracted substrate stays narrow: backend health, launch-product derivation, inventory controls, and provider lifecycle semantics that are reusable across `Autopilot` and future provider binaries.

## Boundary Split

- `apps/autopilot-desktop` owns mission-control orchestration, provider mode transitions, job lifecycle projections, and payout UX.
- `crates/openagents-provider-substrate` owns reusable provider-domain types and lifecycle helpers.
- `crates/nostr/client` owns reusable relay transport + NIP-90 helper primitives.
- `crates/spark` owns reusable Spark wallet primitives.

## Practical Outcome

- Backroom provider/wallet patterns are ported as app-layer behavior adaptations.
- Shared provider semantics now live in `crates/openagents-provider-substrate` rather than being duplicated in app code.
- Reusable protocol helper (`submit_job_request_and_await_result`) is implemented in `crates/nostr/client`.
- No broad runtime-crate migration is required for MVP launch.

## Extraction Posture

- Do not extract a broad runtime crate during this MVP pass.
- The allowed extraction for this pass is the narrow provider substrate that is now landed under `crates/openagents-provider-substrate`.
- Keep Mission Control, payout UX state, and app-owned execution snapshots in `apps/autopilot-desktop`.
- Good future extraction candidates remain generic lifecycle/state-machine helpers and execution-attestation helpers, not pane-facing product orchestration.
