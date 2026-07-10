# FC-5: live Codex + Claude runtime and two-client Fleet proof

- Issue: #8640
- Parent: #8638
- Status: active P0 proof lane; coordinator/owner-gated hot paths
- Authority: [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md)

This live-proof issue supplies runtime evidence to the reliable persona-neutral
Desktop/mobile coding and Fleet program. It does not make Sarah the required
front door, own mobile information architecture, or replace #8547/#8636's
remote-workroom acceptance. Persona/A/V/presentation scope is paused.

## Outcome

Prove the real mixed-account Fleet runtime, then prove reliable Desktop/mobile
observation and control. The landed Sarah route may initiate Phase A as a
compatibility adapter; it is not the required product front door and does not
by itself complete R3 or R7.

## Phase A — real mixed-account runtime

- At least two simultaneous pinned public work units under one FleetRun.
- Codex and Claude each complete useful work through explicitly named isolated
  owner accounts.
- An authenticated typed adapter starts/manages the run and performs one steer
  or approval round trip.
- Zero duplicate claims, default account homes, silent provider substitution,
  or manually launched per-assignment shells.
- Exact provider usage where available; explicit `not_measured` otherwise.
- Verification and accepted closeout survive reconnect.
- Latency, progress freshness, stalls/reconnects, friction, and proof rung are
  recorded without hiding misses in an aggregate pass.

No fixture, synthetic harness result, default provider home, or provider
substitution satisfies Phase A.

## R3/R7 client acceptance

After the runtime receipt:

- Desktop and mobile show the same run/work/attempt/account/approval/command/
  outcome/receipt refs and versions through Khala Sync;
- one or more controls are exercised from each client;
- every command reaches exactly one durable accepted/rejected/failed outcome or
  remains explicitly unknown-pending-reconcile until reconciled;
- lost acknowledgement, duplicate/out-of-order delivery, restart, stale
  generation, and handoff do not duplicate work or fabricate success; and
- a sustained owner dogfood receipt proves direct software is the reliable
  operational path.

Phase A can close the Fleet runtime rung before the full client rung. Do not
call that product cutover complete.

## Current live truth

- C1 is crossed: #8637, #8633, and #8639 are closed; the minimum-safe command/
  reconnect stack is deployed.
- A historical Grok canary completed; Grok is postponed while funded capacity
  is unavailable and is not an acceptance item.
- A Claude canary failed closed before verification on credential scanning and
  produced no accepted work/spend.
- Named Codex homes require isolated owner reauthentication; default
  `~/.codex` must never be used or modified for automatic work.
- Type-boundary/scanner/acceptance paths have coordinator-owned claims. Respect
  the live claim ledger; do not duplicate or weaken them.

## Operational decision

- Existing compatibility-route canaries may continue only at their honest
  proof rung.
- A clean Phase A receipt accepts the mixed-account runtime as substrate.
- R7, not C2 alone, accepts Desktop/mobile as the owner-facing software path.
- This Codex app remains coordinator, independent verifier, development, and
  break-glass while reliability work continues.
- #8547/#8636's minimum remote-workroom path is a separate P0 dependency for
  mobile R6/R7. It does not change #8640 Phase A's owner-local runtime exit;
  advanced managed-capacity/provider breadth follows R7.

## Evidence bundle

- pinned deployment/source/app versions;
- run/work/attempt/assignment/session/account/target refs;
- exact verification and public-safe artifact refs;
- exact usage or explicit `not_measured`, separate compute economics, and no-
  spend failure truth;
- claim uniqueness/concurrency and typed fallback history;
- command idempotency/outcomes and reconnect/restart evidence;
- Desktop/mobile matching refs/versions and handoff receipt;
- operator minutes, latency/freshness/stall distribution, friction ledger; and
- could-not-prove list with owners.

## Exit

#8640 closes when the issue body explicitly records which of these are complete:

1. clean live Codex+Claude runtime receipt;
2. R3 Desktop/mobile control receipt; and
3. R7 owner-accepted dogfood/product cutover receipt.

If issue scope closes at Phase A only, R3/R7 residuals must be transferred to
#8566/#8574/#8597 with exact acceptance and owners. Sarah/persona/A/V quality
is not an exit criterion.
