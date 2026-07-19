# Khala Code app retirement and OpenAgents successor promises

- Date: 2026-07-09
- Registry version: `2026-07-09.1`
- Owner direction: retire the Khala Code mobile and Electrobun desktop product
  shells. Build greenfield OpenAgents mobile and Electron desktop apps
- Implementation issues: #8566, #8574, #8597, #8634

## Promise transition

The registry keeps stable history while changing current product truth:

| Promise ID | Prior | Current | Successor/disposition |
| --- | --- | --- | --- |
| `autopilot.desktop_gui_client.v1` | planned | withdrawn | `openagents.desktop_app.v1` |
| `khala_code.desktop_codex_wrapper.v1` | yellow | withdrawn | `openagents.desktop_app.v1` |
| `khala_code.mobile_mvp.v1` | planned | withdrawn | `openagents.mobile_app.v1` |
| `khala_code.forum_hotbar.v1` | planned | withdrawn | exact old-shell placement retired. Forum capability dispositioned under #8574 |
| `khala_code.bundled_fleet_skill.v1` | yellow | planned | canonical skill remains real. New-app packaging is unproven |
| other `khala_code.*` capability/economics IDs | planned/yellow | same conservative state | stable-ID carry-forward only. Old app wiring is historical evidence |

Two planned successor records are new:

- `openagents.desktop_app.v1`: greenfield Effect Native + Electron app at
  `apps/openagents-desktop`, starting from a pinned MIT-licensed
  `LuanRoger/electron-shadcn` snapshot and hardening its unsafe
  `nodeIntegration: true` default before product capability.
- `openagents.mobile_app.v1`: greenfield Effect Native + React Native/Expo app
  at `apps/openagents-mobile`, named `OpenAgents`, using `com.openagents.app` on
  iOS and Android and the pinned current Khala mobile icon digest.

`mobile.fleet_companion.v1` remains planned but now means Sarah/FleetRun
supervision inside the new OpenAgents mobile app, not a fourth companion product
or either deprecated Khala shell.

No green promise changes state. The post-transition registry shape is 145 total:
34 green, 78 planned, 20 yellow, 7 withdrawn, and 6 red.

## Integrity preservation

Withdrawal removes current product copy. It does not erase history. Preserve:

- every stable legacy promise ID and registry version note.
- `/promises`, the stable `/docs/product-promises` meaning/alias, and
  `/api/public/product-promises`.
- public transition/audit/readiness projections and owner-gated transition
  authority.
- existing promise-transition receipts.
- exact `khala_code_download_events` rows and the public-safe download-count
  read.
- existing outside-user run receipts and their public-safe GET route.
- trace/plugin precedent receipts and public-safe read route.
- historical plan/status, entitlement, privacy, payment, and receipt reads.
- every still-valid evidence ref or an explicit stable redirect/archive ref.

These are compatibility/evidence surfaces, not product destinations. #8634 may
retire or redirect `/code` and `/code/download`, but it must not sever a promise,
service-deliverable, receipt, verification, or transition graph.

## Legacy plan fail-closed rule

The Khala Code plan catalog becomes a frozen compatibility/tombstone catalog.
It preserves schema, plan IDs, and historical entitlement/receipt reads, but it
reports purchases unarmed and a retirement reason. The old
`KHALA_CODE_PAID_PLANS_ENABLED` flag cannot resurrect a withdrawn product or
create new purchase authority. A future OpenAgents plan requires a new promise,
copy, pricing, consent/privacy, API authority, and receipt chain.

## Capability carry-forward rule

“Folded into OpenAgents” means disposition, not automatic availability. Every
Khala Code idea is classified as fold into Sarah, retain as a specialist
OpenAgents capability, or extract as a shared engine consumed by the
Sarah-first apps. Only an obsolete legacy implementation may be retired after
its idea has a recorded successor disposition. Legacy source can prove
lineage and help parity. It cannot prove a greenfield app behavior, a store or
desktop release, cross-device continuity, or safe Electron authority.

The binding app decision and exact cutover gates live in
[`../sol/decisions/2026-07-10-greenfield-clients-and-sarah-removal.md`](../sol/decisions/2026-07-10-greenfield-clients-and-sarah-removal.md).
