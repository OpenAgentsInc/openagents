# EN-1 Stage1 Effect Native Receipt

Issue: OpenAgentsInc/openagents#8567

Route: `/stage1` in the TanStack Start app.

Deploy target: `openagents-com-start-stage1` on Cloud Run. This is a staging
service, not the `openagents.com` root homepage.

## Scope

- `/stage1` mounts a thin React route shell and renders the public launch slice
  through the Effect Native DOM renderer.
- `/`, `/new`, and `/demo` stay untouched for owner review and future root
  cutover work.
- Public projections are loaded from the existing OpenAgents public APIs:
  Khala tokens served, Pylon stats, and Khala Code plans.
- The app-local Effect Native snapshot is copied from
  OpenAgentsInc/effect-native commit `6dda1d443321d815eff342058b8f53c26615b721`
  so the Start app can prove the renderer path while the upstream packages are
  still early.

## Comparison Receipt

- Baseline `/new` remains the existing React launch surface.
- `/stage1` intentionally uses the current Effect Native primitive set only:
  stack, text, card, list, spacer, and button.
- Known EN-1 visual deltas are accepted at this stage: no root cutover, no
  marketing media hero, no icon/media primitive parity, explicit owner-copy
  placeholders, and explicit live-data fallback states.
- The route is safe to compare against `/new` and `/demo` without changing
  those routes.

## Verification

- `bun run --cwd apps/openagents.com/apps/start test -- src/routes/-stage1-effect-native.test.tsx`
- `bun run --cwd apps/openagents.com/apps/start typecheck`
- `bun run --cwd apps/openagents.com/apps/start build`
- `bun run --cwd apps/openagents.com/packages/effect-native-tokens typecheck`
- `bun run --cwd apps/openagents.com/packages/effect-native-core typecheck`
- `bun run --cwd apps/openagents.com/packages/effect-native-render-dom typecheck`
- Local Cloud Run wrapper smoke: `/internal/healthz`, `/stage1`, and proxied
  `/api/public/khala-code/plans`.

## Follow-Ups

- Upstream Effect Native schema/style exactness bug:
  OpenAgentsInc/effect-native#44.
- Root homepage copy/cutover stays gated by OpenAgentsInc/openagents#8565.
