# Effect Native demand register (EN-2 / #8572)

Date: 2026-07-09
Status: living register — every conversion wave appends rows; never invent
local one-off primitives.

This is the monorepo half of the catalog demand loop:

**converting surface → gap row here → upstream effect-native issue/GAPS →
catalog version bump → vendor pin → convert-and-delete.**

## Current vendor pin

See `apps/openagents.com/packages/effect-native-vendor.json`:

- commit / catalog: tracked there (v26 as of 2026-07-09)
- Freshness: `bun apps/openagents.com/scripts/check-effect-native-vendor-freshness.ts`

## Open demand rows

| ID | Surface | Gap | Upstream | Vendor | Consumer issue |
|---|---|---|---|---|---|
| D-SARAH-01 | `/sarah` avatar | Host kind `media-video` | shipped upstream v26 (effect-native#67) | vendored v26; `apps/sarah/src/ui` converted | #8624 #8598 |
| D-SARAH-02 | `/sarah` transcript | Streaming transcript primitive | covered by v17 `Transcript` (effect-native#35 + #26) | vendored v26; `apps/sarah/src/ui` converted off `List`+`Card` | #8624 |
| D-SARAH-03 | `/sarah` mic | Mic state + audio level | waiting — upstream GAPS row (effect-native#66) | n/a — enters when PTT/level metering lands | #8624 |
| D-SARAH-04 | `/sarah` cards | Handoff/checkout/receipt cards | waiting — upstream GAPS row (effect-native#66); `Card`+`Text`+`Button` composition is the honest interim and BM-4 uses it for Actions + Code/Receipts | n/a | #8624 #8630 |
| D-SARAH-05 | `/sarah` first paint | AI disclosure banner component | covered by v16 `StatusBanner` (effect-native#40) | vendored v26; surface adoption is a shell-layout step (copy unchanged) | #8624 |
| D-SARAH-06 | `/sarah` Blueprint map | GraphFigure semantic affordances: badge/accent slot, entry animation, provenance chips, `evidence_backed` edge status | waiting — upstream effect-native#68 | n/a — BM-2 uses existing `GraphFigure` v26 model | #8628 #8575 |
| D-WEB-01 | `/stage1` `/landing-en` | Marketing catalog consumption | shipped upstream v20–v25 | vendored v25 | #8595 |
| D-MB-01 | khala-mobile screens | Full mobile rewrite components | effect-native #52/#64 | vendored v25 | #8597 |
| D-DESK-01 | Khala Code desktop | Full desktop chrome (EN-5) | Phase 4 catalog | vendored v25 | #8574 |

## Process checklist (per wave)

1. [ ] Name the screen and the missing tag/capability (public-safe).
2. [ ] Add a row to this table with consumer issue link.
3. [ ] Update `docs/sarah/EN-GAPS.md` when Sarah is the demander.
4. [ ] File or refresh upstream GAPS/issue in `OpenAgentsInc/effect-native`.
5. [ ] After upstream lands: bump vendor pin, typecheck consumers, convert-and-delete local workaround.

## Closed / satisfied examples

| ID | Note |
|---|---|
| D-WEB-MKT | Marketing primitives (Hero, NavBar, StatsBand, …) demanded by WEB-1-EN — upstream #46–#51, vendored v25 |
| D-MB-PAGER | Mobile Pager / surfaces / swipeable — upstream #60–#63, vendored v25 |

## Exit for EN-2 (#8572)

EN-2 is a **standing process lane**. It is complete as a *lane definition* when:

- [x] This register exists and is linked from EN epic / Sarah EN-GAPS
- [x] Vendor freshness guard remains the anti-staleness mechanism
- [x] Converting surfaces are instructed to append rows (this doc)

Component implementations stay tracked on their consumer issues; EN-2 does not
block forever waiting for every row to ship.
