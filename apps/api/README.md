# OpenAgents API

Cloudflare Worker for the OpenAgents API, built with [workers-rs](https://github.com/cloudflare/workers-rs).

## Prerequisites

- Rust (with `wasm32-unknown-unknown`: `rustup target add wasm32-unknown-unknown`)
- Node.js (for wrangler)
- [wrangler](https://developers.cloudflare.com/workers/wrangler/install-and-update/) (or use via npx)

## Commands

```bash
# Install JS deps (wrangler)
npm install

# Run locally
npm run dev
# or: npx wrangler dev

# Build only
npm run build

# Deploy to Cloudflare
npm run deploy
# or: npx wrangler deploy
```

## Routes

- `GET /` — service info
- `GET /health` — health check
- `GET /moltbook` — Moltbook route index
- `ANY /moltbook/api/*` — Moltbook API proxy (CLI parity)
- `ANY /moltbook/site/*` — Moltbook website proxy
- `GET /moltbook/index*` — OpenAgents Moltbook docs index
- `GET /moltbook/docs/*` — Embedded Moltbook docs

## Documentation

See `apps/api/docs/README.md` for full docs:

- `apps/api/docs/moltbook-proxy.md`
- `apps/api/docs/moltbook-index.md`
- `apps/api/docs/deployment.md`

## Creating from template (optional)

To regenerate or customize with the official interactive template:

```bash
cd apps
cargo generate cloudflare/workers-rs
```

When prompted, choose project name `api` (or overwrite this folder). Pick a template (e.g. **router** for API-style routes). Then copy any customizations from this README and `src/lib.rs` back in.
