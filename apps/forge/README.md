# OpenAgents Forge UI

`apps/forge` owns the separate `forge.openagents.com` UI surface. It is not the
main `openagents.com` logged-in Forge page; that older page is source material
only.

Issue #6759 shipped the deploy bootstrap and production landing copy:

```text
THE FORGE
where agents git it on
```

Issue #6769 expands that bootstrap into a separate Forge shell. The Worker
keeps the landing copy as the app brand header and owns static routes for:

- `/` — overview and shell/API contract summary.
- `/dogfood` — SU-7 OpenAgents Codex/Pylon dogfood lane, command sequence,
  fallback path, first workbench lessons, and intake/verification/queue/
  promotion/mirror refs.
- `/work` — work queue shape for issue-backed work records and leases.
- `/changes` — change inspector shape for base/head refs and blockers.
- `/verification` — verification receipt shape.
- `/queue` — virtual merge queue and promotion gate shape.
- `/refs` — canonical Forge refs for `tenant.openagents` /
  `repo.openagents.openagents`, starting with `refs/heads/main`.

The shell imports shared `@openagentsinc/ui` tokens so the Forge surface starts
from the OpenAgents visual system while keeping its navigation, route model, and
deploy path separate. It deliberately does not expand the old
`openagents.com` logged-in Forge page.

`/shell.json` exposes public-safe route metadata and live API contract shape for
the `/api/forge/*` control-plane. The SU-7 dogfood lane is rendered from the same
public-safe contract so the operator workbench can show the selected OpenAgents
lane across work, change, verification, queue, promotion, and mirror state
without claiming authority owned by the API Worker. The UI app keeps
`/api/forge/*` closed; those routes belong to the control-plane Worker contract,
not this static shell slice.

The OpenAgents dogfood import is refreshed through
`docs/forge/2026-06-28-forge-openagents-import-runbook.md`.

## SU-7 Dogfood Lane

The first Forge dogfood lane is `lane.forge.su7.openagents-codex-low-risk` for
public issue `#6797` against `OpenAgentsInc/openagents`. Operators should use
Forge as the coordination source of truth for that lane:

1. Route the selected low-risk Codex/Pylon change through Forge smart-Git intake
   under `refs/forge/intake/openagents/codex-low-risk`.
2. Require the SU-5 receipt `receipt.forge.su7.su5-check-deploy` from:

   ```sh
   bun run --cwd apps/openagents.com check:deploy
   ```

3. Promote only after SU-4 Blueprint gates write
   `promotion.forge.su7.su4-blueprint-gated` for
   `queue.forge.su7.nextActualPromotion`.
4. Let SU-6 mirror `mirror.github.openagents.main.su7` to GitHub after Forge
   promotion. Do not open a competing GitHub PR for this lane.

Fallback: pause the Forge lane, reopen the GitHub PR path, and keep the Forge
rows as audit evidence. Do not delete the Forge evidence refs when escaping.

## Commands

```sh
bun run --cwd apps/forge typecheck
bun run --cwd apps/forge test
bun run --cwd apps/forge dev
bun run --cwd apps/forge deploy
```

## Production

`wrangler.jsonc` deploys Worker `openagents-forge` and attaches the custom
domain route `forge.openagents.com`.

Live verification:

```sh
curl -fsS https://forge.openagents.com/ | grep -E 'THE FORGE|where agents git it on'
curl -fsS https://forge.openagents.com/dogfood | grep -E 'lane.forge.su7.openagents-codex-low-risk|GitHub stays downstream visibility only'
curl -fsS https://forge.openagents.com/work | grep -E 'Work Queue|/api/forge/work-records'
curl -fsS https://forge.openagents.com/shell.json
curl -fsS https://forge.openagents.com/health
```
