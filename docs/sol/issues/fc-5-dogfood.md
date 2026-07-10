# FC-5: live Sarah Codex + Claude coding burn

Parent: #8638

## Outcome

The owner uses Sarah—not manual terminal orchestration—to burn real OpenAgents
backlog concurrently across named, isolated Codex and Claude accounts.

## Phase A — immediate local daily-driver unblock

This phase is the live-account rung transferred when #8633 closed at its
production-code/integrated-fixture boundary. It must use real owner-linked
Codex and Claude credentials; no fixture, synthetic harness result, default
provider home, or provider substitution satisfies the following acceptance
list.

- At least two simultaneous pinned public work units under one FleetRun.
- Codex and Claude each complete at least one useful unit on owner-local
  capacity through explicitly named isolated accounts.
- Sarah starts the run, reports progress, handles one live steer or approval,
  and presents verification and accepted closeout.
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

## Grok disposition

Grok is postponed by owner decision on 2026-07-10. The connected account is
quota/payment exhausted and no additional quota is planned soon. Existing Grok
adapters, typed HTTP-402 state, fixtures, and the accepted historical canary
remain regression substrate, but Grok is not a Phase A acceptance item and
cannot hold this issue or the daily-driver cutover open. Reactivation receives
a separately scoped issue only when funded capacity exists.

The hybrid local/managed-cloud acceptance formerly duplicated here belongs to
#8636. It is not part of this issue's exit and does not block the owner-local
cutover.

## Current live truth

- C1 is crossed: #8637, #8633, and #8639 are closed; the exact command-loop
  fixture shipped through production revision `00069-h2k`.
- A real Sarah→Grok run completed useful work with accepted closeout and
  reconnect-visible evidence. This is historical proof, not the current
  provider requirement.
- A real Sarah→Claude run failed closed before verification on the long-lived
  SCM credential scanner. It produced zero completed work, a rejected no-spend
  closeout, a released claim, and no landed patch; the scanner/diagnostic gap
  must be repaired without weakening the invariant.
- Named Codex homes require isolated owner reauthentication. The default
  `~/.codex` home must never be touched.
- The clean physical dependency/API typecheck gate remains an implementation
  blocker; its active hot-file claim must be respected.

## Operational cutover decision

- C1 permits low-risk Sarah canaries while this Codex app remains coordinator,
  independent verifier, and break-glass.
- Between C1 and a clean Phase A receipt, Sarah is canary-only and routine work
  remains in this app. A failed or incomplete canary does not advance the gate:
  record the fallback/friction, repair in place, and remain at C1.
- A clean Phase A receipt changes the default: new bounded owner backlog work
  starts through Sarah/Khala/Pylon, not through a manually orchestrated Codex
  app session.
- Keep the Codex app as break-glass, control-plane development, and independent
  evidence review. Record every fallback in the friction ledger.

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

## Exit

One clean, Sarah-started, simultaneous Codex+Claude Phase A receipt closes this
issue and authorizes the default owner-local operational cutover to
Sarah/Khala/Pylon. Managed-cloud hybrid proof remains #8636; Grok is postponed.
