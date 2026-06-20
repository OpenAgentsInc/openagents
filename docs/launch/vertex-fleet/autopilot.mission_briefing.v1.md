# autopilot.mission_briefing.v1 — vertex-fleet worker note

Promise state: **yellow** (unchanged — no state flip in this change).

## Blocker advanced

`blocker.product_promises.mission_briefing_live_mission_citation_missing` —
"The remaining gate is at least one live mission citing this briefing JSON."

This change builds the missing **structural citation linkage** between a live
coding Autopilot mission and the Mission Briefing projection it cites. Before
this, nothing connected `CodingAutopilotMissionRecord.latestBriefingRef` to the
`AutopilotMissionBriefingProjection.briefingRef` returned by
`GET /api/autopilot/work/{workOrderRef}/briefing`, so "a mission cites this
briefing" was not a verifiable, typed fact.

## What was built

- `apps/openagents.com/workers/api/src/autopilot-mission-briefing-citation.ts`
  - `missionCitesBriefing(mission, briefing)` — true iff the mission's
    `latestBriefingRef` equals the briefing's `briefingRef`.
  - `missionBriefingCitation({ mission, briefing, nowIso })` — a public-safe
    `autopilot_mission_briefing_citation` projection that records the
    mission→briefing link, whether the mission actually cites this briefing
    (`briefingCitedByMission`), the derived `decisionNeeded` flag (true for
    `blocked | delivered | needs_input | payment_required | revision_required`),
    the `nextActionState`, `riskLevel`, and only public-safe
    `proofRefs` / `verificationRefs` / `decisionReasonRefs`. It throws
    `AutopilotMissionBriefingCitationUnsafe` if any surfaced ref is
    private/secret/payment/wallet material.
- `apps/openagents.com/workers/api/src/autopilot-mission-briefing-citation.test.ts`
  (4 tests, all pass): citation agreement, mismatched-briefing flagging,
  `decisionNeeded` for an automated `retry_later` state, and a no-secret-leak +
  unsafe-ref refusal assertion.

## Validate

- `cd apps/openagents.com/workers/api && bunx vitest run src/autopilot-mission-briefing-citation.test.ts` → 4 pass.
- `bunx tsc -p tsconfig.json --noEmit` → my files add 0 errors. One pre-existing,
  unrelated error remains: `src/training-data-refinery.ts(18,3): TS6133
  'Cs336A4EvalDeltaMeasurementRef' is declared but its value is never read`
  (last touched by commit 859e65e05, not by this change).

## What remains for green (owner-gated)

The citation **mechanism** now exists and is tested, but the blocker is not
fully cleared: it still requires at least one **real production live mission**
whose mission record cites a real briefing JSON (decision-needed state, real
artifact/test/proof refs) — captured and referenced from a launch receipt by an
owner-signed, receipt-first upgrade per `proof.claim_upgrade_receipts.v1`. The
blocker stays listed.
