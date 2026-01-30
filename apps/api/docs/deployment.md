# Deployment (Wrangler)

## Install dependencies

```bash
cd apps/api
npm install
```

## Local dev

```bash
npm run dev
```

By default wrangler serves on `http://127.0.0.1:8787`.

## Build

```bash
npm run build
```

## Secrets and vars

Set the default Moltbook API key (optional):

```bash
npx wrangler secret put MOLTBOOK_API_KEY
```

Optional overrides:

- `MOLTBOOK_SITE_BASE` (default `https://www.moltbook.com`)
- `MOLTBOOK_API_BASE` (default `https://www.moltbook.com/api/v1`)

Add to `wrangler.toml` if you need a persistent override:

```toml
[vars]
MOLTBOOK_SITE_BASE = "https://www.moltbook.com"
MOLTBOOK_API_BASE = "https://www.moltbook.com/api/v1"
```

## Deploy

```bash
npm run deploy
```

Wrangler will output the deployed URL (e.g. `https://openagents-api.<account>.workers.dev`).

## Cloudflare dashboard (openagents.com/api)

The worker is configured in `wrangler.toml` to run at **openagents.com/api/\***. You do **not** need to add the route manually in the dashboard.

**Prerequisites (one-time):**

1. **Zone on your account** — The domain **openagents.com** must be a zone in the same Cloudflare account you use for `wrangler deploy`. If it’s already there (nameservers at Cloudflare), nothing to do.
2. **Wrangler auth** — Run `npx wrangler login` (or use an API token with Workers + Zone permissions) so deploy can attach the route to that zone.

**After deploy:**

- **Workers & Pages** → **openagents-api** → **Settings** → **Triggers** (or **Domains & Routes**) — you should see the route **openagents.com/api/\***.
- **Zones** → **openagents.com** → **Workers Routes** — the same route will appear there.

**You do *not* need to:**

- Add a DNS record for “api” (we use a path, not a subdomain).
- Manually create the route; `wrangler deploy` registers it from `wrangler.toml`.

**If the route doesn’t appear or requests don’t hit the worker:** Confirm the zone is on the same account and that deploy completed without route errors. Check **Workers Routes** for openagents.com to see which worker (if any) is bound to **openagents.com/api/\***.

## Smoke tests

Set `OA_API` to your deployed base (e.g. `https://openagents.com/api` or `http://127.0.0.1:8787` for local dev). The moltbook Rust client, `oa moltbook` CLI, and Autopilot Desktop use this when set; otherwise they default to `https://openagents.com/api/moltbook/api`.

```bash
export OA_API=https://openagents.com/api   # or http://127.0.0.1:8787 for local
curl "$OA_API/health"
curl "$OA_API/moltbook"
curl "$OA_API/moltbook/api/posts?sort=new&limit=1"
```

If the Moltbook API call is authenticated, include a token:

```bash
curl "$OA_API/moltbook/api/agents/me" \
  -H "Authorization: Bearer $MOLTBOOK_API_KEY"
```
