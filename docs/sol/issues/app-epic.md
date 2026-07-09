# EPIC APP-1: three OpenAgents apps on Effect Native

## Owner direction

The product surface collapses to three applications:

1. **OpenAgents web** — `openagents.com`, `/sarah`, and `/forum*`.
2. **OpenAgents mobile** — one iOS/Android app, Sarah at home.
3. **OpenAgents Desktop** — Sarah plus the deep coding-fleet cockpit.

All three author one Effect Native application/component/intent model. Sarah
and the former Khala Code features are capabilities inside these apps, not
separate products.

Authority: `docs/sol/MASTER_ROADMAP.md`.

## Child lanes

- #8634 — one web host + aggressive public-route retirement.
- #8635 — retained Forum routes in the Effect Native web app.
- #8595 — retained root landing + production cutover.
- #8597 — OpenAgents mobile consolidation.
- #8574 — OpenAgents Desktop consolidation.

Parallel presentation companion: #8610.

## Shared implementation laws

- Effect Native typed component set and typed intents are the architecture;
  React/RN/DOM hosts are adapters only.
- One OpenAgents token system and one relationship/run vocabulary.
- New component demand goes upstream; no app-local one-off primitives.
- Every converted surface deletes the implementation it replaces.
- Existing launch, auth, Forum, payment, receipt, and authority contracts stay
  green through migration.
- Product names in new copy are OpenAgents, OpenAgents Desktop, and Sarah.
  Khala and Pylon remain engine-room names where technically useful.

## Known integration red

`check:effect-topology` still expects Effect `4.0.0-beta.70` while the four
vendored Effect Native packages require `4.0.0-beta.94`. Repair the guard or
align the runtime before claiming the epic's deploy gate is green.

## Exit

Only the three named applications remain as product surfaces. Web, mobile, and
desktop render from Effect Native, share Sarah conversation/fleet state through
typed services and Khala Sync, and have no separate Khala Code or Pylon UI
architecture left.
