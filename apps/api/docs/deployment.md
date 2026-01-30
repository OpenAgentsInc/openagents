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

## Smoke tests

```bash
curl "$OA_API/health"
curl "$OA_API/moltbook"
curl "$OA_API/moltbook/api/posts?sort=new&limit=1"
```

If the Moltbook API call is authenticated, include a token:

```bash
curl "$OA_API/moltbook/api/agents/me" \
  -H "Authorization: Bearer $MOLTBOOK_API_KEY"
```
