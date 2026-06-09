# Issue 4368 Two-Hour Closure Plan

Date: 2026-04-20

## Actual Status

`#4368` is not honestly closed yet.

What is already done:

- The CS336 proof/runtime fixes are on `main`.
- The #4385 local proof runtime can complete the replacement-attempt lane.
- The #4385 local proof runtime can complete the stale-recovery lane with one
  accepted contribution, rewarded closeout, worker/validator quiescence, and
  zero caveats.
- Nexus image `a06a1af62024` built successfully from `main`.
- Treasury recovery report generation no longer treats Spark explicit sync
  timeout as opaque wallet-storage divergence.
- The latest recovery report proves no wallet-storage cutover is needed:
  current and rebuilt cached balances match and the recommendation is
  `no_cutover_needed_sync_timeout_cached`.
- The latest scripted deploy proved the funding-target status poisoning fix on
  the candidate image: `wallet_runtime_status` reached `connected` during
  candidate smoke instead of `treasury_funding_target_timeout:*`.

What is not done:

- Production Nexus has not yet produced fresh post-deploy accepted-work payout
  evidence from the new image.
- Production Spark explicit sync is still slow enough to time out during
  isolated recovery inspection.
- Production payout smoke still stalls after the wallet-status fix because the
  wallet has less spendable balance than the active payout policy and the
  payout loop remains degraded on stale reconciliation state.
- Production placeholder payouts must remain disabled. `#4368` is a homework
  closeout problem, not a periodic liveness-stipend problem.
- The production deploy/smoke loop takes materially longer than the local proof
  loop and must not be the primary iteration mechanism.

## Local Usable Artifact

The thing the operator can use locally is a deterministic proof bundle:

```bash
bash scripts/pylon/issue-4368-local-closure.sh
```

The script:

- runs formatting and diff gates
- runs focused treasury recovery comparison/cutover tests
- runs the homework accepted-work payout integration test
- rebuilds the real `oa` and `nexus-relay` binaries separately
- runs the local post-deploy smoke simulations for both observed Nexus failure
  shapes:
  - zero fresh completed sends, nonzero inference-ready payout targets, and
    `treasury_funding_target_timeout:*` wallet status
  - zero fresh completed sends, nonzero inference-ready payout targets,
    `wallet_runtime_status=connected`, degraded payout loop, and wallet balance
    below the active sats-per-window payout policy
  - homework-only production policy with placeholder payouts disabled and no
    pending accepted-work payout records
- runs the local replacement-attempt proof lane
- runs the local stale-recovery proof lane with worker and validator
- copies `run-report.json`, `authority-state-trace.json`, and
  `proof-summary.json` into `var/proof/issue-4368-local-closure-<stamp>/`
- writes `closure-summary.json`
- shuts down the proof fleet and authority namespaces

Local closure proof is green only if:

- replacement-attempt status is `completed`
- replacement-attempt caveat count is `0`
- stale-recovery status is `completed`
- stale-recovery closeout is `rewarded`
- stale-recovery caveat count is `0`
- stale-recovery accepted contribution count is at least `1`
- both post-deploy smoke simulations report `decision=rollback`, a matching
  `failure_class`, and `reproduced_current_failure=true`
- the homework-only smoke simulation reports `decision=pass` because
  placeholder payouts are disabled and there are no pending accepted-work payout
  records

This local artifact is the fast iteration target. It is not a substitute for
live settlement proof, but it is the primary evidence that the CS336 homework
runtime itself works.

## Two-Hour Path

Minute 0-15:

- Run `scripts/pylon/issue-4368-local-closure.sh`.
- If it fails, do not touch Nexus. Fix the local proof or fixture first.

Minute 15-25:

- Commit/push any local-proof or doc fixes to `main`.
- Comment on `#4368` with the local proof summary path and the exact commit.

Minute 25-95:

- Treat Nexus as a final confirmation lane only.
- If a deploy is already running, let the rollback-gated script finish.
- If a fresh deploy is needed, use the normal build/deploy scripts from the
  exact `main` commit.
- Do not disable post-deploy smoke to make the issue look green.

Minute 95-120:

- If production smoke passes, collect:
  - active image
  - `/healthz`
  - `/v1/treasury/status`
  - accepted-work run/window id
  - rewarded closeout
  - fresh completed payout send after service start
- Then close `#4368`.
- If production smoke fails, do not continue ad hoc Nexus debugging. Open or
  update a focused production-smoke issue with the failed gate and leave
  `#4368` open unless the user explicitly narrows `#4368` to local proof only.

## Rebuild Decision

Do not throw away the local proof runtime. It has caught real scheduler,
artifact, replay, closeout, and payout bugs.

Do replace the old operating model:

- stop using production Nexus as the debugger
- make `scripts/pylon/issue-4368-local-closure.sh` the first gate
- add Spark sync-timeout, funding-target timeout, and post-deploy smoke
  simulations to the local proof runtime
- disable placeholder payout accrual by default; disabled placeholder mode
  should not create payout records at all, and only accepted-work/homework
  closeouts should create new payout records
- split production treasury continuity from CS336 homework proof when it blocks
  unrelated local proof progress

## Closure Rule

`#4368` can be closed only when one of these is true:

- Full rule: local proof bundle is green from `main`, the new image is deployed,
  and production has fresh accepted-work/closeout/payout evidence.
- Narrowed rule: the user explicitly agrees that `#4368` is local CS336 proof
  only, and production treasury settlement is tracked in a separate issue.

Without one of those two conditions, closing `#4368` would be dishonest.
