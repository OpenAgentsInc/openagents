# autopilot.builtin_compute_agent.v1 — vertex-fleet work note

Date: 2026-06-20
Worker: vertex-fleet/autopilot.builtin_compute_agent.v1
Promise state: **yellow** (no flip this pass — hard rule honoured)

## Blocker advanced

**`blocker.product_promises.openagents_compute_metering_live_smoke_missing`**

## What was built

A typed metering-smoke projection fixture for the builtin compute agent's
go-online flow, analogous to `pylon-install-to-bitcoin-smoke.ts`:

| File | Purpose |
|---|---|
| `apps/openagents.com/workers/api/src/builtin-compute-agent-metering-smoke.ts` | Typed projection fixture — models the 7-step metered go-online flow (installer_signed → pylon_readiness_checked → hosted_compute_readiness_checked → grant_issued → session_bounded → usage_recorded → public_quota_projection), enforces cap constants from the live grant module, redaction-scans outputs for raw key material. |
| `apps/openagents.com/workers/api/src/builtin-compute-agent-metering-smoke.test.ts` | 19 vitest tests covering CI mode (ci_no_live_sessions), live mode (live_from_signed_install), all per-step blocker conditions, cap-constant mismatch detection, redaction guard, and schema decode round-trip. All 19 pass. |

The fixture:
- Imports cap constants directly from `builtin-compute-agent-grant.ts` so any
  policy change is automatically caught.
- Never materialises or logs raw API keys — only secret-REFs.
- Supports two modes: `ci_no_live_sessions` (logic validation only, no live
  sessions required) and `live_from_signed_install` (all 7 steps must supply
  real public refs to be `passed`).
- The `live_from_signed_install` projection is the exact evidence shape a live
  smoke runner fills in when the signed/notarised installer is available.

## What remains (not cleared this pass)

| Blocker | Status |
|---|---|
| `builtin_compute_agent_signed_recut_missing` | **Still open.** Requires a signed+notarised Autopilot Desktop build with the built-in-agent source and packaged OpenAgents compute credentials — owner-gated, needs a real recut. |
| `builtin_compute_agent_live_from_install_smoke_missing` | **Still open.** Requires a from-install go-online session on a clean Mac with the signed installer — depends on the signed recut. |
| `openagents_compute_metering_live_smoke_missing` | **Partially advanced.** The projection fixture and test infrastructure for capturing a live metered session now exist; the blocker is not dropped because no live session evidence has been captured yet. A live smoke run filling in real refs will produce `status: live_metered_session_verified` and clear this blocker. |

## check:deploy

Passed. 19/19 metering smoke tests pass; all 3 check:deploy test files pass
(product-promises.test.ts, mullet/routes.test.ts, public-proof-replay-routes.test.ts).
