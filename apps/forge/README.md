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
- `/work` — work queue shape for issue-backed work records and leases.
- `/changes` — change inspector shape for base/head refs and blockers.
- `/verification` — verification receipt shape.
- `/queue` — virtual merge queue and promotion gate shape.
- `/refs` — canonical Forge ref namespaces and GitHub mirror state.

The shell imports shared `@openagentsinc/ui` tokens so the Forge surface starts
from the OpenAgents visual system while keeping its navigation, route model, and
deploy path separate. It deliberately does not expand the old
`openagents.com` logged-in Forge page.

`/shell.json` exposes public-safe route metadata and stub preview state shaped
for the future `/api/forge/*` control-plane. Until SU-2 lands, the UI app keeps
`/api/forge/*` closed; those routes belong to the control-plane Worker contract,
not this static shell slice.

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
curl -fsS https://forge.openagents.com/work | grep -E 'Work Queue|/api/forge/work-records'
curl -fsS https://forge.openagents.com/shell.json
curl -fsS https://forge.openagents.com/health
```
