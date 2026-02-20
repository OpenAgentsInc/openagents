---
name: worker-logs
description: Tail and inspect Cloudflare Worker logs from the CLI. Use when debugging API 401/500, openclaw auth, or web app errors. Covers both the homepage worker (apps/web) and the API worker (apps/api).
---

# Check Cloudflare Worker logs

Use this skill when you need to see what is happening inside the deployed Workers: request paths, response status, console.log output, and errors. Both the main site and the API are separate Cloudflare Workers.

## When to use this skill

- Debugging 401/500 from openagents.com or openagents.com/api.
- Verifying that a Worker receives the expected headers (e.g. X-OA-Internal-Key) and why it might return "unauthorized".
- Seeing console.log / console_error! output from the Rust API or the web app.
- Correlating with Khala logs (Khala calls the API worker; tail the API worker while reproducing).

## Workers in this repo

| Worker | Config | Routes | Purpose |
|--------|--------|--------|---------|
| **openagents-api** | `apps/api/wrangler.toml` | `openagents.com/api/*` | Rust API: openclaw, control, D1, R2, etc. |
| **openagents-web-app** | `apps/web/wrangler.jsonc` | `openagents.com` (main site) | TanStack/React app (Node compat). |

Run `wrangler tail` from the **app directory** that contains that worker's config (or use `--config` / `--cwd`).

## Wrangler tail (real-time only)

Cloudflare does **not** provide historical Worker logs via the CLI. You get a **live stream** of requests and logs. For historical data, use the dashboard: Workers & Pages → your worker → Logs / Real-time Logs or Logpush.

### Basic usage

```bash
# API worker (Rust) — run from apps/api
cd apps/api
npx wrangler tail

# Web app worker — run from apps/web
cd apps/web
npx wrangler tail
```

Leave the command running, then reproduce the issue in the browser. You'll see each request, status, and any console output.

### Tail options

| Option | Meaning |
|--------|--------|
| `--format pretty` | Human-readable (default). |
| `--format json` | One JSON object per log line (e.g. pipe to `jq`). |
| `--status ok` | Only successful requests. |
| `--status error` | Only errors/failures. |
| `--method GET` | Filter by HTTP method. |
| `--search "openclaw"` | Filter by text in console.log messages. |
| `--header "x-oa-internal-key"` | Filter by presence of header. |
| `--sampling-rate 1` | Log 100% of requests (default can sample). |

### Examples

```bash
# API worker: only errors, pretty
cd apps/api
npx wrangler tail --status error --format pretty

# API worker: JSON and filter by URL path with jq
cd apps/api
npx wrangler tail --format json | jq 'select(.url | contains("openclaw"))'

# Web worker: tail while reproducing a page error
cd apps/web
npx wrangler tail
```

### Two workers, two terminals

To see both the site and the API when debugging a flow (e.g. Hatchery calling Khala, Khala calling API):

1. Terminal 1: `cd apps/api && npx wrangler tail --format pretty`
2. Terminal 2: `cd apps/web && npx wrangler tail --format pretty`
3. Optional: Khala logs in a third terminal: `cd apps/web && npx khala logs --prod --success`

Then reproduce; watch for the request to the API worker and any `console.log` / diagnostic output.

## Limitations

- **Real-time only:** No `--history`; tail streams until you Ctrl+C.
- **Sampling:** Under heavy load, tail may sample; use `--sampling-rate 1` to reduce sampling.
- **Max 10 clients:** Up to 10 concurrent tail sessions per worker.
- **Secrets:** Logs must not print secrets; use lengths or "present/absent" in diagnostic logs.

## Diagnostic logging (this repo)

For openclaw auth, the API worker logs:

- **`openclaw auth: no internal key header path=...`** — request reached the worker but the `X-OA-Internal-Key` header was missing (e.g. stripped or not sent by Khala).
- **`openclaw auth 401: path=... provided_len=... expected_len=...`** — header was present but value didn’t match the worker secret (compare lengths; if equal, values differ).
- **`openclaw auth ok path=... key_len=...`** — internal key matched; request was authorized.

Use `wrangler tail` from `apps/api` while reproducing 401 to see which line appears. Khala actions log `[openclawApi <label>] fetch key_len=<n> url=...` before each request; correlate with worker logs to confirm what Khala sent vs what the worker received.
