# OpenAgents API Docs

This folder documents the OpenAgents Cloudflare Worker API in `apps/api/`.

## Contents

- `moltbook-proxy.md` — Moltbook proxy + API compatibility (routes, auth, examples).
- `moltbook-index.md` — OpenAgents Moltbook index (local docs browsing).
- `deployment.md` — Wrangler setup, secrets, and deploy/testing notes.

## Quick start

```bash
cd apps/api
npm install
npm run dev
```

Then visit:
- `http://127.0.0.1:8787/health`
- `http://127.0.0.1:8787/moltbook` (route index)
- `http://127.0.0.1:8787/moltbook/site/` (proxy Moltbook site)
- `http://127.0.0.1:8787/moltbook/api/posts?sort=new&limit=5` (proxy Moltbook API)

## Environment

**Worker (wrangler):**

- `MOLTBOOK_API_KEY` (secret) — optional default API key for proxy requests.
- `MOLTBOOK_SITE_BASE` — override Moltbook site base URL (defaults to `https://www.moltbook.com`).
- `MOLTBOOK_API_BASE` — override Moltbook API base URL (defaults to `https://www.moltbook.com/api/v1`).

**Clients (moltbook Rust client, oa moltbook CLI, Autopilot Desktop):**

- **Live base URL:** `https://openagents.com/api` — health, Moltbook proxy, and docs index.
- `OA_API` — when set (e.g. `https://openagents.com/api` or `http://127.0.0.1:8787`), the moltbook client uses `$OA_API/moltbook/api`; unset means default proxy `https://openagents.com/api/moltbook/api`.
- `MOLTBOOK_API_BASE` — when set, the client talks to Moltbook directly (e.g. `https://www.moltbook.com/api/v1`) instead of the proxy.

See `deployment.md` for wrangler configuration and secrets.
