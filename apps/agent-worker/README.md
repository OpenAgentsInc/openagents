# openagents-agent-worker

Cloudflare Worker that handles `/internal/chat` (and related endpoints) for the OpenAgents web app. Used when the web app has `AGENT_WORKER_URL` set; provides durable chat via Durable Objects.

## Deploy

```bash
npm run deploy
# or: wrangler deploy
```

After the first deploy, note the worker URL (e.g. `https://openagents-agent-worker.<your-subdomain>.workers.dev`).

## Required secret

The web app proxies `/chat` to this worker and sends `X-OA-Internal-Key`. This worker must have the **same** value configured:

```bash
wrangler secret put OA_INTERNAL_KEY
```

Use the same value you set for:

- Convex (`OA_INTERNAL_KEY` in Convex env)
- Rust API (`OA_INTERNAL_KEY` where `apps/api` is deployed)
- Web app (`wrangler secret put OA_INTERNAL_KEY` on `openagents-web-app`)

If the key is missing or different, this worker returns 401 for `/internal/chat` and the web app falls back to in-process chat.

## Web app wiring

Set `AGENT_WORKER_URL` on the web app (openagents-web-app) to this worker’s URL so the web app knows where to proxy `/chat`:

- Cloudflare Dashboard → Workers & Pages → openagents-web-app → Settings → Variables and Secrets, or
- `wrangler secret put AGENT_WORKER_URL` (value: this worker’s URL, e.g. `https://openagents-agent-worker.<account>.workers.dev`)

See `apps/web/docs/zero-to-openclaw-30s.md` (“Why /assistant Chat Can Return unauthorized” and “How the agent-worker gets the correct key”) for the full flow and checklist.
