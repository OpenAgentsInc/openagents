# M9 Live Rate-Limit Rotation Gate Record

Date: 2026-06-12

Issue: #4767

Claim kind: `m9_live_rate_limit_rotation`

Gate status: ready to close.

## Decision Record

Required receipt kinds:

- `smoke_passed`
- `verification_passed`
- `usage_threshold_crossed`

Required live evidence refs:

- `evidence.live.rate_limit_rotation.1`

Live evidence refs supplied:

- `evidence.live.rate_limit_rotation.1`

Source commit refs:

- `commit.openagents.9cd15c4a0`
- `commit.openagents.a92bd60e3`
- `commit.openagents.8b79ef2c2`

Smoke receipt authority refs:

- `authority.gate.smoke.m9.ci_safe.9cd15c4a0`

Missing evidence: none for #4767.

Accepted deferrals: none for #4767. This record does not close or satisfy #4768,
#4771, #4772, #4777, #4781, #4782, #4783, or #4749.

## CI-Safe Leg

The deterministic M9 smoke remains the CI-safe leg:

- Smoke: `apps/openagents.com/workers/api/src/autopilot-rate-limit-rotation-smoke.ts`
- Focused tests: `apps/openagents.com/workers/api/src/autopilot-rate-limit-rotation-smoke.test.ts`
- Gate regression: `apps/openagents.com/workers/api/src/autopilot-gate-proof-authority.test.ts`
- CI-safe source commit ref: `commit.openagents.9cd15c4a0`

## Live Route Leg

The live route leg used the deployed ChatGPT/Codex account pool for
`chris@openagents.com`.

First live failover evidence, recorded on #4767 before this Gate record:

- Initial lease: `provider-account-lease_ref_1cb7f9734fdb4cd9b781db8104fee700`
- Initial account: `provider-account_ref_2dd6a8b25aad4d93a42947bec62c8465`
- Run: `run.m9.live_rotation.20260612`
- Assignment: `assignment.m9.live_rotation.20260612`
- Failure class: `rate_limited`
- Outcome: `retrying`
- Account state action: `timed_cooldown`
- Next lease: `provider-account-lease_ref_fd9a4982d0d34a54a07bcf72dadc46f0`
- Next account: `provider-account_ref_7b41e0634ec743b6a4855379b3e0fb18`

Work-order continuity live evidence, recorded after a third account was
connected:

- Work order: `autopilot_work_order.a52fa1ed-e509-42cb-8ef9-44ea98422313`
- Selected runner kind: `requester_pylon`
- Selected Pylon: `pylon.artanis.m9.rotation.continuity.20260612`
- Assignment:
  `pylon_assignment.autopilot_work_order.a52fa1ed-e509-42cb-8ef9-44ea98422313.task.m9_rotation_continuity.20260612.artanis.01`
- Run: `run.m9_rotation_continuity.a52fa1ed`
- Previous lease: `provider-account-lease_ref_16d16655153347b1aa716acebab0e7d2`
- Previous account: `provider-account_ref_7b41e0634ec743b6a4855379b3e0fb18`
- Failure class: `rate_limited`
- Outcome: `retrying`
- Account state action: `timed_cooldown`
- Next lease: `provider-account-lease_ref_da937744a4f04e629ea99fcbffa3451b`
- Next account: `provider-account_ref_53cdc41d2e584920b83268fd0d77183d`
- Cooldown until: `2026-06-12T04:03:46.876Z`
- Replacement lease release status: `succeeded`
- Active leases after release: zero

## Continuity Evidence

The work order delivered after the live route failover. Its execution closeout
contains:

- Assignment ref:
  `pylon_assignment.autopilot_work_order.a52fa1ed-e509-42cb-8ef9-44ea98422313.task.m9_rotation_continuity.20260612.artanis.01`
- Artifact refs:
  - `artifact.m9_rotation_continuity.pre_state_retained`
  - `artifact.m9_rotation_continuity.post_state_builds_on_pre_state`
- Build ref: `build.m9_rotation_continuity.context_chain`
- Closeout ref: `closeout.m9_rotation_continuity.assignment_completed`
- Proof refs:
  - `proof.m9_rotation_continuity.pre_state_snapshot`
  - `proof.m9_rotation_continuity.route_rotation_verified`
  - `proof.m9_rotation_continuity.work_order_chain`
- Result ref: `result.m9_rotation_continuity.closeout.delivered`
- Summary ref: `summary.m9_rotation_continuity.public_safe`
- Test ref: `test.m9_rotation_continuity.live_pylon_dispatch`

The delivered projection reports:

- State: `delivered`
- Next action: `caller.review_autopilot_closeout`
- Runner kind: `requester_pylon`
- Public-safe closeout: true
- Accepted-work authority: false
- Worker-payout authority: false

## Boundary

This closes only the #4767 proof question: live rate-limit rotation can retry on
a different connected account without losing the work-order context chain.

It does not create deploy authority, spend authority, accepted-work authority,
settlement authority, worker-payout authority, or broader MVP readiness.
