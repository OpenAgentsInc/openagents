# FC-5: live Sarah multi-stream coding burn

Parent: #8638

## Outcome

The owner uses Sarah—not manual terminal orchestration—to burn real OpenAgents
backlog across multiple coding streams.

## Phase A — immediate local unblock

- At least three simultaneous pinned public work units under one FleetRun.
- Codex, Claude, and Grok each complete at least one useful unit on owner-local
  capacity.
- Sarah starts the run, reports progress, handles one steer/approval, and
  presents verification/closeout.
- A browser reconnect reconstructs the same named work-unit state and controls.
- Zero duplicate claims, default account homes, silent harness substitutions,
  or manually launched per-assignment shells.
- Exact provider usage where available; explicit `not_measured` otherwise.
- Measure and report Sarah acknowledgment/run-ref latency, first capacity state,
  first executor progress/blocker, and heartbeat freshness against the FC
  budgets; do not hide misses behind an aggregate pass.
- Record the proof rung for each acceptance item: code-landed, fixture-proven,
  deployed, live-proven, owner-accepted, or closed. No earlier rung implies a
  later one.

## Phase B — hybrid proof

- After #8547 and #8636 are integrated, at least one owner-local unit and one
  managed Agent Computer unit complete useful verified work concurrently in
  one Sarah run, with typed visible target selection and fallback history.
- Preserve one claim registry and one Sarah supervision contract.
- Reconcile compute and model usage truth separately.

## Operational cutover decision

- Before C1, use this Codex app/subagents or explicitly partitioned Codex tabs
  to implement and debug the fleet path.
- Once #8637, #8633, and the minimum safe #8639 seam pass one fixture on a
  pinned integrated deployment, run the first low-risk pinned real issue
  through Sarah as a canary.
- Between C1 and a clean Phase A receipt, Sarah is canary-only and routine work
  remains in this app. A failed or incomplete canary does not advance the gate:
  record the fallback/friction, repair in place, and remain at C1.
- A clean Phase A receipt changes the default: new bounded owner backlog work
  starts through Sarah/Khala/Pylon, not through a manually orchestrated Codex
  app session.
- Keep the Codex app as break-glass, control-plane development, and independent
  evidence review. Record every fallback in the friction ledger.
- Phase B enables default hybrid local/cloud selection; it is not a prerequisite
  for the owner-local switch.

**Clean** means all required evidence comes from one pinned integrated
deployment: owner/auth scope, named isolated accounts, fresh advertised
capacity, claim uniqueness, typed fallback, reconnect, verification, closeout,
and honest usage evidence pass without manual per-assignment shells or silent
substitution.

## Evidence bundle

- Deployment commit and app versions.
- Run, work-unit, assignment, session, account-hash, and target refs.
- Verification commands/results and public-safe artifact refs.
- Usage evidence per completed turn: exact ledger rows where measured;
  otherwise an explicit `not_measured` closeout plus accounting-gap ref. Never
  synthesize counter movement. Private trace/raw-event presence is required
  where the harness ingest route exists; every absence is explicit in
  could-not-prove.
- Claim-uniqueness and concurrency report.
- Before/after operator-minutes and a friction ledger.
- Latency distribution and stall/reconnect events against the named budgets.
- Could-not-prove list.

The #8610 paired text/audio/realtime-video/pre-rendered-opener crossover should
run alongside the first canary cohort when capacity permits, using the same
bounded task classes. Its presentation result does not block C2 unless it
reveals an availability lie or loss of text/fleet control.

## Exit

Phase A closes the immediate coding-unblock milestone and authorizes the
default owner-local operational cutover to Sarah/Khala/Pylon. Phase B closes
the P0 epic's hybrid milestone. Any manual recovery step becomes a typed
backlog item before the receipt is called clean.
