# Projection staleness epic (#4751): invariant, ratchet, and retrofits

Date: 2026-06-11. Branch: `staleness-4751` off `65bd62899`.

The epic adopted Orrery's invariant after eight frozen-projection
instances in ~24 hours: every public projection carries `generatedAt`
(or `lastRebuiltAt`) plus a declared `maxStaleness`, and either
rebuilds on the state transitions that matter or composes live at
read; a projection that cannot meet its own declared staleness says so
in the payload.

## What landed

### 1. The shared contract

`workers/api/src/public-projection-staleness.ts` —
`projection_staleness.v1`: `composition` of `live_at_read` |
`rebuilt_on_transition` | `stored_snapshot`, `maxStalenessSeconds`,
`rebuildsOn` (the write-site invalidation set), plus constructors and
the `projectionStalenessExceeded` / `projectionDataAgeSeconds`
helpers. It extends the vocabulary frozen by the Tassadar trace
factory day-0 contract (`projection_rebuild.v0.1`, #4748) and the
first platform application on #4754, rather than inventing a second
vocabulary. Tests: `public-projection-staleness.test.ts`.

### 2. The invariant in INVARIANTS.md

- `apps/openagents.com/INVARIANTS.md` gained the "Public Projection
  Staleness Declaration" section with the full 26-surface inventory
  (7 compliant, 18 honestly NON-COMPLIANT legacy rows, 1 static-exempt
  plus the openapi.json contract-document exemption note).
- Root `INVARIANTS.md` gained the platform-wide "Public Projection
  Staleness" statement pointing at the app ledger and the check.

### 3. The regression ratchet

`scripts/check-zero-debt-architecture.mjs` (runs in
`check:architecture` inside `check:deploy`) now:

- discovers every `/api/public/...` literal (quoted, template, or
  regex form) in route modules and fails any route not covered by its
  projection-surface ledger — verified red on a probe route;
- greps every `staleness_declared` ledger module for the contract
  token (`maxStalenessSeconds` or the shared module import);
- freezes the legacy set as an exact budget
  (`PUBLIC_PROJECTION_LEGACY_BUDGET = 18`): retrofits must flip the
  row and lower the budget in the same change; new legacy rows fail.

This satisfies "any NEW public projection without
generatedAt+maxStaleness fails review/check tooling" without
retrofitting every legacy surface tonight.

### 4. Retrofits

- `/api/public/artanis/report` (instance 6, #4745): payload now
  carries `generatedAtUnixMs` (numeric because the surface's safety
  scan bans raw ISO strings) and the report contract
  (`live_at_read`); `autonomousLoop` carries its own
  `rebuilt_on_transition` contract (bound 86 400 s,
  `rebuildsOn: [artanis_loop_tick_closeout]`),
  `latestTickAgeSeconds`, `nextTickOverdue` (the tick's own
  next-tick promise), `projectionStale`, and `source`
  (`persisted_loop_ticks` | `typed_example_fallback`). The June-7
  typed-example fallback is now labeled
  `typed_example_fallback` + `projectionStale: true` with caveat refs
  `caveat.public.artanis.loop_projection_example_fallback_not_live_state`
  and `..._exceeds_declared_staleness` instead of being served as
  current loop state. 5 new tests in `artanis-public-report.test.ts`.
- Tip surfaces (the #4753 remainder): `tip-leaderboards`,
  `moderation/tip-earnings`, and `actors/{ref}/tip-earnings` payloads
  declare `generatedAt` + the live-at-read contract; per-post
  `tipStats` blocks carry the contract; creator leaderboard rows list
  ladder `totalCreditedSats`/`totalSweptSats` (swept coverage =
  oldest-credited-first `min(sweptMsat, creditedMsat)`), with honesty
  caveat refs that ranking still keys on settled receipt-backed tips.
  New `forum/tip-leaderboards-staleness.test.ts` (3 tests); updated
  `forum-routes.test.ts` assertions.
- Agent profile (instances 1–2, #4744): verified #4744's live
  owner-claim composition landed (`f83106c33`); the route response now
  declares `generatedAt` + contract
  (`rebuildsOn: [agent_owner_claim_approved,
  agent_owner_x_claim_verified, agent_registration_updated,
  orange_check_entitlement_changed]`); and the second lost write is
  fixed: a verified/approved X-proof challenge
  (`agent_owner_x_claim_challenges`) now composes live into the
  profile as `verificationState: 'x_verified_agent'` with the X claim
  receipt ref in `safeReceiptRefs` (refs only; no handle, token, or
  tweet URL). New route test in `forum-routes.test.ts`.

## Per-instance closure map

| Instance | Issue | State | Disposition against the invariant |
| --- | --- | --- | --- |
| 1. profile frozen at registration | #4744 | CLOSED (`f83106c33`) | Closes against the invariant: live-at-read composition landed pre-epic; this epic added the payload declaration (`generatedAt` + contract). |
| 2. X-proof verified, verificationState stale | documented on #4744 | fixed here | Closes against the invariant: `x_verified_agent` composes the verification write live; regression test on the route. |
| 3. openapi.json frozen | #4752 | OPEN | Static contract document — exempt from the payload rule but its route inventory must track shipped routes; remains the open companion (file locked by the in-flight lane). |
| 4. pylon-stats window self-contradiction | #4735 | CLOSED (`4ca56d0df`) | Predates the rule: fixed by self-describing `counterWindows`; surface still lacks a declared `maxStaleness` contract — listed NON-COMPLIANT in the inventory and counted in the frozen budget. |
| 5. credited tips invisible | #4753 | CLOSED (`932212a1d`) | Closes against the invariant: read paths landed pre-epic; this epic delivered the named remainder (tipStats + leaderboard declarations, ladder credited/swept sats in leaderboards). |
| 6. artanis report asserting stale state | #4745 | CLOSED (`2533856f8`) | Closes against the invariant: rebuild-on-closeout landed pre-epic; this epic added the declaration, age/overdue flags, and the example-fallback labeling. |
| 7. capacity funnel at zero | #4745 | CLOSED | Predates the rule for the declaration: stage derivation fixed; `generatedAt` exists but no contract — listed NON-COMPLIANT, in the frozen budget. |
| 8. x_claim_reward no read path | #4754 | CLOSED (`6acc0573d`) | Closes against the invariant: the read path shipped with `generatedAt` + the #4748-shape contract and is the `staleness_declared` reference row. |

#4746 (evidence refs unresolvable) and #4747 (tips settle invisibly)
closed pre-epic with resolving read paths; their surfaces
(`/api/public/nexus-pylon/receipts/{ref}`, `/api/forum/receipts/{ref}`)
are inventoried NON-COMPLIANT for the payload declaration and sit in
the frozen budget.

## Remainders

- Owed to the locked `workers/api/src/openagents-openapi.ts` (#4752
  lane): document `staleness`/`generatedAt` fields on the retrofitted
  responses (`/api/public/artanis/report`, tip leaderboards/earnings,
  agent profiles, claims rewards), the new leaderboard creator
  `totalCreditedSats`/`totalSweptSats` + `caveatRefs` fields, and the
  `x_verified_agent` verificationState literal.
- Owed to the locked `workers/api/src/training-run-window-routes.ts`
  lane: generatedAt + staleness declarations for the training
  window/leaderboard/eval read surfaces (single frozen-budget row).
- 18 legacy surfaces to retrofit budget-down (inventory in
  `INVARIANTS.md`); highest value next: `/api/public/pylon-stats`,
  `/api/public/launch-dashboard`, `/api/public/pylon-capacity-funnel`.
- Leaderboard post rows do not yet list per-post ladder credited sats
  (recipient-level sweep attribution does not map to posts cheaply);
  covered by the caveat refs.
- Web UI surfacing of credited/swept leaderboard fields remains
  unclaimed (#4753 note).

## Gates

- `bunx vitest run` on touched areas: 313 tests / 28 files green;
  full worker suite 2597/2602 with 5 failures pre-existing on the
  clean base (artanis-forum-delivery, artanis-scheduled-runner,
  openagents-agent-onboarding sha256 — reproduced with changes
  stashed).
- `bun run typecheck:api` exit 0; full `bun run check:deploy` green,
  including the extended `check:architecture` with the new ledger.
