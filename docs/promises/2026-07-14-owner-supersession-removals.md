# Owner-Directed Supersession Removals — 2026-07-14

- Class: promise-registry withdrawal record + removal disposition
- Date: 2026-07-14
- Registry pass: `2026-07-14.1`
- Successor product record: `promise:openagents.desktop_app.v1`
- Pre-removal recovery commit (openagents `main`, immediately before the
  removal change): `c7044f5a2870110b331c5a7288caceb85488290a`
  - Recover any deleted path with
    `git show c7044f5a2870110b331c5a7288caceb85488290a:<path>`.

## Owner statement (verbatim, 2026-07-14)

> khala-code-desktop must itself be deprecated and all relevant promises
> removed (OpenAgents desktop supercedes it). ditto for
> apps/autopilot-desktop. sarah get rid of that too etc - i dont give a shit
> wut u do just get that shit cleared out

This statement explicitly supersedes the prior retention clauses for the named
surfaces ("remove the old clients only after parity, migration, and release
proof"): the owner has decided OpenAgents Desktop supersedes them.

## Promise withdrawals (registry pass 2026-07-14.1)

Seven promises flip to `withdrawn` with successor
`promise:openagents.desktop_app.v1`. Withdrawals are downgrades and need no
`promise_transition` receipt per the `mobile.autopilot_remote_control.v1`
precedent (`docs/promises/registry.md`); stable promise IDs, historical
evidence refs, and public receipt/read routes remain served.

| promiseId | prior state | new state |
| --- | --- | --- |
| `autopilot.agent_character_creation.v1` | planned | withdrawn |
| `autopilot.agent_world_scene.v1` | green | withdrawn |
| `autopilot.bitcoin_payment_visualization.v1` | planned | withdrawn |
| `autopilot.builtin_compute_agent.v1` | planned | withdrawn |
| `autopilot.local_apple_fm_tool_chat.v1` | planned | withdrawn |
| `autopilot.pylon_growth_visualization.v1` | planned | withdrawn |
| `khala_code.bundled_fleet_skill.v1` | planned (capability carry-forward) | withdrawn |

Green count moves 34 -> 33 (`autopilot.agent_world_scene.v1` was the one
green record in the set; its historical owner-signed #7030 green transition
stays recorded in the registry notes).

`khala_code.bundled_fleet_skill.v1` leaves the
`khalaCodeCapabilityDisposition` carry-forward ledger because its subject —
the canonical `.agents/skills/khala-fleet/SKILL.md` — was removed at owner
direction; the carry-forward claim no longer exists to port.

## Surface dispositions in the same change

| Surface | Disposition |
| --- | --- |
| `apps/autopilot-desktop/` | REMOVED (superseded by OpenAgents Desktop). No external package imported it; root scripts/workspaces entries removed. The `updates.openagents.com` legacy desktop feed routes remain armed CUT-26 `410` tombstones (`apps/oa-updates/src/legacy-desktop-lockout.ts`). |
| `packages/sarah-take-scoreboard/` | REMOVED (Sarah surface cleanup; clean leaf — no runtime importers). Root script + perimeter-allowlist entries removed; the admitted MVP AssuranceSpec sweep command was re-pinned under its own revision bump. |
| `.agents/skills/khala-fleet/` | REMOVED. The frozen Khala Code client's byte-pin test was reduced to embedded-copy checks with a dated note; the bundled-skill promise is withdrawn above. |
| `clients/khala-code-desktop/` | REMOVED in #8793 after every executable Pylon/QA/package dependency was migrated. QA-owned compatibility fixtures remain in the QA harness, neutral chat events use `agent-runtime-schema`, and active desktop gates target `apps/openagents-desktop`. Historical promise evidence dereferences through `git show c7044f5a28:<path>` and the backroom supersession intake. |
| `packages/autopilot-ui/` | RETAINED: the live `apps/openagents.com/apps/web` app depends on it (`package.json` workspace dep + `@import '@openagentsinc/autopilot-ui/styles.css'`), plus token-parity tests in `packages/ui` and `packages/autopilot-control-protocol`. Not autopilot-desktop-only, so the owner's "etc" does not reach it. |
| `/api/sarah/fleet-runs` (FleetRun authority route) | ALIASED, not removed: neutral canonical path `/api/fleet-runs` added; the legacy path stays a served compatibility alias because shipped OpenAgents desktop/mobile binaries hardcode it (`packages/khala-sync-client` now targets the neutral path for future builds). No 410: fielded clients would lose the live fleet projection. |
| `/api/operator/business/sarah-checkout-links` + CRM Sarah handoff store | RETAINED: live CRM machinery (`crm-reply-routes.ts`, `crm-command.ts`, `crm-mcp.ts`) consumes the handoff store; renaming cascades through production CRM code and D1 migration 0311 — deferred to its own bounded issue. |

Full per-item restoration pointers live in
`docs/refactor/2026-07-14-mvp-prune-ledger.md` (Part 2).

## Evidence-ref integrity

Evidence refs citing removed paths (`apps/autopilot-desktop/**`,
`.agents/skills/khala-fleet/SKILL.md`) are retained verbatim on the withdrawn
records for historical integrity and dereference through git history at the
recovery commit above. This follows the #8610 Sarah-removal precedent (deleted
`apps/sarah/**` with git history as the archive) and the 2026-07-09
app-retirement precedent (withdrawn records keep exact historical refs).
Archived registry/contract material was additionally copied to the backroom
repo intake `openagents-supersession-prune-2026-07-14/`.
