# Autopilot Earn Runtime Placement Decision

Date: 2026-03-04
Issue: #2819

## Decision

Provider runtime ownership remains in `apps/autopilot-desktop`.

No new retained runtime crate is introduced for this MVP pass.

Scope note: this decision is for the compute-provider MVP lane and does not pre-decide future liquidity-solver module placement.

Terminology note:

- `OpenAgents Runtime` means the execution environment where jobs run, local runtime state advances, and provenance is produced.
- `OpenAgents Kernel` means the authority layer that verifies outcomes, settles value, and emits canonical receipts.

## Rationale

- `docs/OWNERSHIP.md` assigns product workflow/state authority to `apps/autopilot-desktop`.
- Existing runtime state lanes (`provider_runtime`, `job_inbox`, `active_job`, `job_history`, `wallet_reconciliation`) are already app-owned and integrated with UI behavior.
- Pulling archived `pylon`/`compute` crates back into this pruned repo would re-expand scope and violate MVP simplification goals.

## Boundary Split

- `apps/autopilot-desktop` owns mission-control orchestration, provider mode transitions, job lifecycle projections, and payout UX.
- `crates/nostr/client` owns reusable relay transport + NIP-90 helper primitives.
- `crates/spark` owns reusable Spark wallet primitives.

## Practical Outcome

- Backroom provider/wallet patterns are ported as app-layer behavior adaptations.
- Reusable protocol helper (`submit_job_request_and_await_result`) is implemented in `crates/nostr/client`.
- No crate-level authority migration is required for MVP launch.

## Extraction Posture

- Do not extract a broad runtime crate during this MVP pass.
- If later extraction becomes justified, prefer a narrow product-agnostic core crate rather than moving Mission Control or payout UX state out of `apps/autopilot-desktop`.
- Good extraction candidates later are generic lifecycle/state-machine helpers and execution-attestation helpers, not pane-facing product orchestration.
