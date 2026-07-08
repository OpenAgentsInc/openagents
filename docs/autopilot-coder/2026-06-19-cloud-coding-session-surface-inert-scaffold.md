# Cloud coding-session surface — flag-gated INERT scaffold (wave 2)

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-19
Promise: `autopilot.cloud_coding_sessions.v1` (stays **red**)
Branch: `wave2-cloud-coding-sessions`

> 2026-07-06 update: Khala Code mobile-only MVP now uses the **Agent
> Computer** framing from
> `docs/khala-code/2026-07-06-agent-computers-strategy.md`. The route names
> remain `/v1/cloud-coding-sessions` for compatibility, but the provisioned,
> isolated, metered unit is an Agent Computer: a Firecracker microVM on
> OpenAgents-owned GCE capacity. "Hosted Pylon" language in older notes is
> historical only.

## What this is

The typed Worker-side request/lifecycle surface for **launching a managed
OpenAgents Cloud coding session** — the "our cloud" autonomous-execution lane
that lets a coding session run on OpenAgents GCE / managed-node capacity instead
of on the owner's laptop. It is built to the exact flag-gated **INERT** pattern
already proven by the inference gateway and the sibling Cloud-primitive scaffolds
(`/v1/sandboxes`, `/v1/fine_tuning/jobs`), so nothing changes on the live Worker
until the managed runtime is wired and the EPIC lands.

This advances `cloud_coding_sessions` from red **toward** green by giving the
typed lane + lifecycle + receipt seam. It does **not** flip the promise. Green
still requires a desktop-originated cloud session running a real repo-edit on
Google GCE, streaming to the timeline, producing a content-addressed artifact,
**plus** a dereferenceable `openagents.resource_usage_receipt.v1` and owner
sign-off per `proof.claim_upgrade_receipts.v1`.

## Surface

- `POST /v1/cloud-coding-sessions` — launch a managed cloud coding session.
- `GET /v1/cloud-coding-sessions/:sessionId` — lifecycle read, scoped to the
  authenticated account (cross-account isolation).

Both routes are **INERT (404) by default**, gated behind
`CLOUD_CODING_SESSIONS_ENABLED` (default off). Wired to the same
programmatic-agent auth the inference gateway / sandbox / fine-tuning surfaces
use.

Source: `apps/openagents.com/workers/api/src/cloud/cloud-coding-session-routes.ts`
Tests: `apps/openagents.com/workers/api/src/cloud/cloud-coding-session-routes.test.ts`

## Typed model + seams

- **Typed launch request** — `repoRef` + `objective` (refs/intent only) + a
  cloud `lane` (`cloud-gcp` default, `cloud-shc` fallback) + a `repoTrustTier`
  (`public` / `private` / `regulated`) + bounded `verify` + a `timeoutSeconds`
  with a hard ceiling (`MAX_CLOUD_CODING_TIMEOUT_SECONDS`, a cost/abuse control
  enforced before any placement).
- **Placement policy** (`decidePlacement` / `admissibleLanesForTrustTier`) — a
  pure, checkable encoding of the promise's authority boundary, enforced BEFORE
  any adapter dispatch: `regulated -> SHC-only`, `private -> own/verified`,
  `public -> any`. A regulated repo requesting `cloud-gcp` is refused (403);
  nothing reaches a VM.
- **Managed-runtime adapter seam** (`CloudCodingRuntimeAdapter`) — where the
  real OpenAgents Cloud control plane plugs in (the cloud repo's
  `POST /v1/placement` + per-session GCE VM lease, cloud #86/#87/#88/#90). Ships
  defaulted to a stub/accepting adapter: it leases no VM, runs no repo-edit, and
  `placementRef` / `artifactRef` stay null.
- **Usage/receipt seam** (`CloudCodingMeteringHook`) — the single point where the
  session's runtime usage round-trips into an `openagents.resource_usage_receipt.v1`
  (the #5005 round-trip target) and, when live, a credit debit. Ships a no-op/log
  stub; a real receipt-first ledger hook (`makeLedgerCloudCodingMeteringHook`)
  is available, charging through the SAME atomic credit ledger the inference
  gateway uses, from REAL usage via an injected pure pricing function.

## Honest scope (no green flip)

- The stub adapter provisions no real VM and runs no real repo-edit; the metering
  stub bills nothing. On prod the routes are inert (404).
- No `product-promises.ts` state change. The promise stays **red**; the blocker
  set (`cloud_live_gce_provisioning_is_fake_default_stub`,
  `cloud_gce_event_kinds_do_not_roundtrip_5005`,
  `pylon_remote_bridge_transport_missing`) is unchanged.

## Verification

- `bun run --cwd apps/openagents.com/workers/api test -- src/cloud/cloud-coding-session-routes.test.ts`
  (27 tests) and the full `src/cloud/` suite (70 tests) pass.
- `bun run --cwd apps/openagents.com check:deploy` is green.
