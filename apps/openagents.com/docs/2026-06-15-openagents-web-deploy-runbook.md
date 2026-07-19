# OpenAgents Web Deploy Runbook

Status: superseded. Do not use these Worker deployment commands.
Use `apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh` and the current
deployment rules in `apps/openagents.com/AGENTS.md`.

This runbook is the production guardrail for deploying `openagents.com` Worker
code and web assets together.

## Invariant

Do not deploy `openagents.com` with a bare `wrangler deploy` after web/UI
changes. The Worker must be deployed with the built web asset directory passed
explicitly:

```sh
cd apps/openagents.com/workers/api
npx wrangler deploy --containers-rollout=none --assets ../../apps/web/dist
```

The final Worker upload uses `--containers-rollout=none` so a local
Docker/container rollout probe cannot stall the safe production deploy. Keep the
same explicit assets argument for Worker-only deploys:

```sh
cd apps/openagents.com
bun run build:web
cd workers/api
npx wrangler deploy --containers-rollout=none --assets ../../apps/web/dist
```

The `--assets ../../apps/web/dist` argument is mandatory. A deploy can otherwise
publish a Worker version whose `ASSETS` binding returns 404 for `/` and the
hashed web bundle. Wrangler can still report a successful Worker upload.

## Desktop Build Gate

`openagents.com` deploys must not regress Autopilot Desktop. The canonical
deploy check now runs the desktop deploy verifier before the Worker typecheck
and selected web/API tests:

```sh
cd apps/openagents.com
bun run check:deploy
```

That transitively runs:

```sh
cd ../..
bun run verify:autopilot-desktop:deploy
```

The desktop verifier runs the Foldkit regression tests and the Three/WebGL
training scene smoke. It runs the browser and Bun entrypoint builds. It runs
the full ElectroBun build and a packaged-asset check for the shared
`three-effect` Moksha GLB. Do not deploy if this gate fails. Fix the desktop
failure first.

## Keep shared github-dependency pins consistent across workspaces

`typecheck:web` can fail because a **shared GitHub dependency is pinned to
different commits in two workspaces**. This failure also stops the deploy gate.
Bun installs one hoisted copy for the monorepo. Thus, the web import can resolve
to the older workspace copy and fail to find new symbols.

This failure occurred with `@openagentsinc/three-effect` on 2026-06-16.
`apps/web` pinned `#f1794af`, which exports
`TrainingRunBeam/Burst/EntityDefinition`.
`apps/autopilot-desktop` pinned the older `#0cce6ccd`. Thus, `typecheck:web`
reported that `@openagentsinc/three-effect` had no exported member.

Rules:

- When you bump a **shared** github dependency (notably
  `@openagentsinc/three-effect`, `foldkit`, `nostr-effect`), bump it to the **same
  commit in every workspace that pins it** — today that is `apps/web` **and**
  `apps/autopilot-desktop`. Grep first:
  `grep -rn "three-effect" --include=package.json .` (exclude `node_modules`).
- Run `bun install` after changing any pin and confirm `bun.lock` resolves a
  **single** commit for that dep (`grep -nE "three-effect.*github" bun.lock` →
  one hash), then re-run `bun run typecheck:web`.
- If `typecheck:web` fails on missing exports from a `@openagentsinc/*` github
  dep, suspect a divergent pin / stale install **before** suspecting the web
  code. The fix is usually aligning the pins + `bun install`, not editing the
  importing file.

## JS / Web-Asset-Only Changes

When the change is limited to browser JavaScript, CSS, public docs, or other web
assets, do not run the full container deploy. Build the current web assets and
deploy the Worker with container rollout disabled:

```sh
cd apps/openagents.com
bun run check:deploy
bun run build:web
cd workers/api
npx wrangler deploy --containers-rollout=none --assets ../../apps/web/dist
```

Wrangler's `--containers-rollout=none` flag deploys the Worker without building
or updating any Containers. Use the full package deploy only when the change
also needs container images, remote D1 migrations, or other full deploy
orchestration.

## Canonical Command

Prefer the package deploy command:

```sh
cd apps/openagents.com
bun run --cwd workers/api deploy
```

That command runs deploy checks, applies remote D1 migrations, rebuilds
`apps/web/dist`, and deploys Wrangler with
`--containers-rollout=none --assets ../../apps/web/dist`.

## Required Live Smoke

After every production deploy, verify the live document and the exact JS asset
served by that document before saying the deploy is done:

```sh
curl -fsSI https://openagents.com/

asset_path="$(
  curl -fsS https://openagents.com/ |
    sed -nE 's/.*src="\/(assets\/index-[^"]+\.js)".*/\1/p' |
    head -1
)"
test -n "$asset_path"
curl -fsSI "https://openagents.com/$asset_path"
```

For route changes, also smoke the changed document routes:

```sh
curl -fsSI https://openagents.com/pylon
curl -fsSI https://openagents.com/stats
curl -fsSI https://openagents.com/stats-old
```

If `/` or the hashed JS asset returns 404, the deployment is not complete. Fix
the asset deployment first, usually by rebuilding `apps/web/dist` and redeploying
from `workers/api` with `--assets ../../apps/web/dist`.
