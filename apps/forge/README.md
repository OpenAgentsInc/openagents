# Forge

Forge is a separate Cloudflare Worker app for the Forge operator UI. It deploys
from `apps/forge/` to `forge.openagents.com`; the older
`apps/openagents.com/apps/web/src/page/loggedIn/page/forge.ts` page is historical
source material only and is not the canonical Forge surface.

This slice intentionally renders public-safe placeholder state behind the shell
contract. The layout names the panes the Forge API will fill next:

- work queue
- change inspector
- verification state
- merge queue
- git/ref views

Future API dependency: the shell expects a public-safe Forge contract that can
provide queue items, file diffs, verification receipts, merge readiness, and
git/ref projection rows without exposing private prompts, local paths, secrets,
customer data, or raw provider output. Until that contract exists, the Worker
serves deterministic stub rows and marks them with `data-forge-provenance`.

## Commands

```sh
bun run --cwd apps/forge typecheck
bun run --cwd apps/forge test
bun run --cwd apps/forge dev
bun run --cwd apps/forge deploy
```

`bun run --cwd apps/forge deploy` uses Wrangler from this package and targets the
production `forge.openagents.com` custom domain environment.
