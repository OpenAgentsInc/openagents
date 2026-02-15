# autopilot-worker

Cloudflare Worker runtime pieces used by the web product and automation paths.

## Development

This project is typically driven via the web app surface:
- `apps/web` runs the main Worker entry and deploy pipeline.

For local work in this package:

```bash
cd apps/autopilot-worker
npm install
npm run lint
npm test
```

## Code Map

- `apps/autopilot-worker/src/` runtime logic
- `apps/autopilot-worker/src/tools.ts` tool contracts/handlers
- `apps/autopilot-worker/src/dseCatalog.ts` DSE catalog (signatures/modules)
- `apps/autopilot-worker/scripts/autopilot-smoke.ts` smoke test script

