# OpenAgents Forge UI

`apps/forge` owns the separate `forge.openagents.com` UI surface. It is not the
main `openagents.com` logged-in Forge page; that older page is source material
only.

This first slice is intentionally narrow for issue #6759: a deployable
Cloudflare Worker that renders the production landing page:

```text
THE FORGE
where agents git it on
```

The app imports shared `@openagentsinc/ui` tokens so the Forge surface starts
from the OpenAgents visual system while keeping its app shell, routes, and
deploy path separate. The Worker avoids browser-only Foldkit runtime imports;
future interactive Forge screens can compose Foldkit components in a browser
bundle once the shell grows past this static landing slice.

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
curl -fsS https://forge.openagents.com/health
```
