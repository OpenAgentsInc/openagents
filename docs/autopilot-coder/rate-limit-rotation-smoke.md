# Autopilot Rate-Limit Rotation Smoke (M9)

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Run the CI-safe leg from the repository root:

```sh
bun run --cwd apps/openagents.com/workers/api smoke:autopilot-coder:rate-limit-rotation
```

The smoke proves that a mission hitting a provider rate limit mid-run
rotates provider-account leases through the real lease and failover
policies and completes with context intact:

1. Build a two-account connected fleet fixture and select the initial
   lease through the production `provider-account-lease-policy:v1`
   selector.
2. Execute mission turns, each producing an artifact that builds on
   the previous turn's artifact and a chained context fingerprint.
3. Induce a `rate_limited` failure at a mid-mission turn through the
   fault-injection seam at the lease layer.
4. Classify the failure with the production failover policy
   (`timed_cooldown`, one-hour cooldown on the limited account) and
   re-select through the same lease policy.
5. Record a `retry_account` continuation decision carrying
   `evidence.account_rate_limit` and `risk.account_rotation_needed`,
   evidence-only, with Action Submission required for direct effects.
6. Resume the interrupted turn under the rotated lease and complete
   the mission.
7. Verify continuity by inspecting the run's records, not the absence
   of errors: same mission record throughout, both account-lease refs
   on the mission, the resumed turn's context fingerprint equal to the
   last pre-rotation fingerprint, the artifact `buildsOn` chain
   spanning the rotation boundary, and pre-rotation artifacts retained
   in the delivered mission record.
8. Exercise the failure arm: a one-account fleet yields a typed
   blocked mission with a `blocker.account_fleet_exhausted.*` ref and
   an `escalate` continuation decision queued as
   `request_customer_input` — never a silent stall.
9. Project the mission and continuation decision for operator and
   customer audiences and scan the receipt and projections for
   provider-account material, secrets, and raw timestamps.

The smoke is deterministic (a repeated run produces an identical
receipt), no-spend, and CI-safe. The receipt carries `generatedAt`
and the smoke version `autopilot-rate-limit-rotation-smoke:v1`.

The companion route-level regression for the live failover surface is:

```sh
bunx vitest run --root apps/openagents.com/workers/api src/operator-provider-account-routes.test.ts
```

That suite covers the deployed `/api/operator/provider-accounts/chatgpt-codex/leases/failover`
route: cooldown writes, next-lease acquisition, retry receipts,
exhausted-fleet `409 blocked` receipts, and redacted failover history.

## Live leg (operator runbook)

The live variant requires a deployed Worker, the operator admin
bearer token, and a target user with **two or more connected healthy
ChatGPT/Codex accounts**. It exercises the same policy code the
CI-safe leg pins. Do not paste the admin token into tracked files or
logs.

1. Confirm fleet readiness (expect two or more connected healthy
   accounts, no stale reconnect markers):

   ```sh
   curl -sS -X POST https://openagents.com/api/operator/provider-accounts/chatgpt-codex/fleet-dashboard \
     -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
     -H 'content-type: application/json' \
     -d '{"email":"<target-user-email>"}'
   ```

2. Acquire the initial lease for a real run (record `leaseRef` from
   the response):

   ```sh
   curl -sS -X POST https://openagents.com/api/operator/provider-accounts/chatgpt-codex/leases \
     -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
     -H 'content-type: application/json' \
     -d '{"email":"<target-user-email>","requestedAction":"customer_order_fulfillment","runId":"<run-id>","assignmentId":"<assignment-id>","orderId":"<order-id>"}'
   ```

3. Run the mission until the leased account hits a real provider rate
   limit (or drive the account to its limit with parallel work), then
   report the failure through the failover route. A `201 retrying`
   response carries the next lease; the limited account is placed on a
   timed cooldown:

   ```sh
   curl -sS -X POST https://openagents.com/api/operator/provider-accounts/chatgpt-codex/leases/failover \
     -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
     -H 'content-type: application/json' \
     -d '{"email":"<target-user-email>","previousLeaseRef":"<lease-ref-from-step-2>","failureClass":"rate_limited","requestedAction":"customer_order_fulfillment","attemptNumber":1,"maxAttempts":3}'
   ```

4. Resume the same run under the returned next lease without
   resetting the workspace, mission record, or thread state, and let
   it complete.

5. Verify the rotation receipts and the redaction posture:

   ```sh
   curl -sS -X POST https://openagents.com/api/operator/provider-accounts/chatgpt-codex/leases/failover-history \
     -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
     -H 'content-type: application/json' \
     -d '{"email":"<target-user-email>","assignmentId":"<assignment-id>"}'
   ```

6. Record the continuity evidence: the mission record must show both
   account-lease refs, the post-rotation artifacts must build on the
   pre-rotation artifacts (same mission/work order, no restart from
   zero), and no response may contain auth material, tokens, or raw
   provider payloads.

Failure arm, live: with only one eligible account connected (or
`attemptNumber == maxAttempts`), step 3 returns `409` with
`outcome: "blocked"` and a typed blocker receipt instead of a next
lease. The run must surface a needs-input/blocked state, never a
silent stall.

A passing live run with these receipts is still required before
product copy claims smart multi-account routing beyond the CI-safe
contract; link the receipts from the promise/registry evidence when
that copy lands.
