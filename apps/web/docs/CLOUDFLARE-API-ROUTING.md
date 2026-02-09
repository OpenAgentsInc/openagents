# Cloudflare API routing (openagents.com)

## Why POST /api/autopilot/send was 405

Two Workers are deployed on **openagents.com**:

| Worker           | Config        | Route(s)                    |
|------------------|---------------|-----------------------------|
| **openagents-api** | `apps/api`    | `openagents.com/api/*`      |
| **autopilot-web**  | `apps/web`    | `openagents.com/*` + more specific `/api/...` |

Cloudflare uses **most specific route wins**. So:

- `openagents.com/api/*` (openagents-api) was matching **all** `/api/...` traffic.
- Requests to `/api/autopilot/send` were handled by **openagents-api**, which does not implement that path (and may proxy to Vercel/moltbook-site → 404/405).

## Fix (apps/web)

In `apps/web/wrangler.jsonc` we register **more specific** routes for the API paths this worker handles, so they hit **autopilot-web** instead of openagents-api:

- `openagents.com/api/autopilot/*`
- `openagents.com/api/auth/*`
- `openagents.com/api/contracts/*`
- `openagents.com/api/dse/*`
- `openagents.com/*` (catch-all for SSR, assets, callback, etc.)

After deploy, `POST https://openagents.com/api/autopilot/send` is handled by autopilot-web and returns 200 (or 400/500 from our handler), not 405 from the other worker.

## Verifying

```bash
# Should return 200 (or 400 if body invalid), not 405
curl -s -o /dev/null -w "%{http_code}\n" -X POST "https://openagents.com/api/autopilot/send" \
  -H "Content-Type: application/json" \
  -d '{"threadId":"x","text":"hi"}'
```

Response headers should **not** include `x-oa-proxy: moltbook-site` or `x-vercel-id` when the request hits autopilot-web.

## Prelaunch: /autopilot must redirect to /

When `VITE_PRELAUNCH=1`, GET `/autopilot` (without `?key=...` or bypass cookie) must return **302 → /** from **autopilot-web**. The redirect response includes `x-oa-prelaunch: redirect` and `x-oa-request-id`.

### If /autopilot still returns 200 after deploy

1. **Verify which worker is serving the request**
   ```bash
   curl -sI "https://openagents.com/autopilot"
   ```
   - If you see **302**, `location: /`, and **`x-oa-prelaunch: redirect`** → autopilot-web is running and prelaunch is working. Clear browser cache and try again in a private window.
   - If you see **200** or no `x-oa-prelaunch` header → the request is **not** being handled by autopilot-web. Another Worker or origin (e.g. Pages, another worker with `openagents.com/*`) is serving the page.

2. **Check Cloudflare dashboard**
   - **Workers & Pages** → **Overview** (or **Triggers**) for zone **openagents.com**.
   - Ensure **autopilot-web** is the only worker attached to route **openagents.com/** or **openagents.com/***. If another worker or a Pages project is bound to the same or a broader route, it can receive the request first; remove or narrow that route so autopilot-web handles HTML routes.

3. **Purge cache**
   - **Caching** → **Configuration** → **Purge Everything** (or purge only `https://openagents.com/autopilot`) so cached 200 responses are not served.
