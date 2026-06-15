# OpenAgents Web Deploy Runbook

This runbook is the production guardrail for deploying `openagents.com` Worker
code and web assets together.

## Invariant

Do not deploy `openagents.com` with a bare `wrangler deploy` after web/UI
changes. The Worker must be deployed with the built web asset directory passed
explicitly:

```sh
cd apps/openagents.com/workers/api
npx wrangler deploy --assets ../../apps/web/dist
```

For Worker-only deploys that must skip container image rollout, keep the same
explicit assets argument:

```sh
cd apps/openagents.com
bun run build:web
cd workers/api
npx wrangler deploy --containers-rollout=none --assets ../../apps/web/dist
```

The `--assets ../../apps/web/dist` argument is mandatory. A deploy can otherwise
publish a Worker version whose `ASSETS` binding returns 404 for `/` and the
hashed web bundle, even though Wrangler reports a successful Worker upload.

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
`apps/web/dist`, and deploys Wrangler with `--assets ../../apps/web/dist`.

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
