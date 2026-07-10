# EPIC APP-1: three OpenAgents apps on Effect Native — greenfield mobile and desktop

## Owner direction

The product surface collapses to three applications:

1. **OpenAgents web** — `openagents.com`, `/sarah`, `/forum*`, and the retained
   `/promises` assurance surface.
2. **OpenAgents mobile** — one iOS/Android app, Sarah at home.
3. **OpenAgents Desktop** — Sarah plus the deep coding-fleet cockpit.

All three author one Effect Native application/component/intent model. Sarah
and the former Khala Code features are capabilities inside these apps, not
separate products.

Authority: `docs/sol/MASTER_ROADMAP.md`.

## Canonical one-page shape

Maintain one compact diagram in the master roadmap showing the three apps as
authorized projections over the same seven-layer loop:

`relationship -> comprehension -> control -> orchestration -> execution -> evidence -> continuity`

Web and mobile are relationship-first projections; Desktop adds the specialist
cockpit. None owns a separate run, authority, memory, or evidence reality.

## Child lanes

- #8634 — one web host + aggressive public-route retirement.
- #8635 — retained Forum routes in the Effect Native web app.
- #8595 — retained root landing + production cutover.
- #8597 — greenfield OpenAgents mobile (`apps/openagents-mobile`, React
  Native/Expo host).
- #8574 — greenfield OpenAgents Desktop (`apps/openagents-desktop`, Electron
  host, required `LuanRoger/electron-shadcn` bootstrap, reusable Effect Native
  Electron-host gap OpenAgentsInc/effect-native#69).

Parallel presentation companion: #8610.

## Shared implementation laws

- Effect Native typed component set and typed intents are the architecture;
  React/RN/DOM/Electron hosts are adapters only.
- One OpenAgents token system and one relationship/run vocabulary.
- New component demand goes upstream; no app-local one-off primitives.
- Mobile and desktop are new applications, not rename-in-place conversions.
  Their legacy clients are frozen as evidence/extraction sources, cannot gain
  product features or releases, and are removed only after greenfield parity
  and migration proof.
- OpenAgents Desktop starts from the pinned MIT-licensed
  `LuanRoger/electron-shadcn` template, then immediately hardens its Electron
  boundary and replaces starter application semantics with Effect Native,
  Effect Schema, and shared typed services as specified by #8574.
- Existing launch, auth, Forum, payment, receipt, and authority contracts stay
  green through migration.
- Product names in new copy are OpenAgents, OpenAgents Desktop, and Sarah.
  Khala and Pylon remain engine-room names where technically useful.

## Effect topology disposition

The deployment guard now verifies the physical runtime boundary rather than
treating peer-report output as resolution authority. OpenAgents/Omega remains
on Effect `4.0.0-beta.70`, exactly the four vendored Effect Native packages
resolve their isolated Effect `4.0.0-beta.94` line, and the isolated Nostr line
remains Effect 3. This intentional split is no longer a deploy blocker; any
package escaping its declared runtime line fails the guard.

## Exit

Only the three named applications remain as product surfaces. Web, greenfield
mobile, and greenfield Electron desktop render from Effect Native, share Sarah
conversation/fleet state through typed services and Khala Sync, and have no
separate Khala Code, Electrobun, or Pylon UI architecture left. The old mobile
and desktop packages have either been deleted or archived as non-shipping
historical evidence after their applicable contracts and services are ported.
The canonical one-page diagram remains linked from the master roadmap and root
product documentation so alternate projections cannot drift into alternate
authority/state/evidence models.
