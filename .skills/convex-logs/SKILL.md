---
name: convex-logs
description: Check and stream Convex deployment logs from the CLI. Use when debugging Convex actions, 401/500 errors, failed queries or mutations, or when you need to see what functions ran and their output.
---

# Check Convex logs

Use this skill when you need to inspect what is happening in a Convex deployment: failed actions, validation errors, 401 unauthorized from external APIs, or which functions ran and how long they took.

## When to use this skill

- User reports an error from Convex (e.g. "unauthorized", "ArgumentValidationError", action timeout).
- You need to confirm which Convex deployment (dev vs prod) is being used.
- You need to see recent function executions (queries, mutations, actions) and their success/failure.
- You are debugging Convex env vars (e.g. OA_INTERNAL_KEY, PUBLIC_API_URL) and want to correlate with API responses in logs.

## Prerequisites

- The project must use Convex and have `npx convex` (or `convex` CLI) available.
- Run Convex commands from the app directory that contains the `convex/` folder (e.g. `apps/web` in a monorepo).
- You must be logged in and have the correct Convex project linked (`npx convex dashboard` to verify).

## Default deployment: dev vs prod

**Important:** By default, `convex logs` and `convex env` target the **dev** deployment. For production issues (e.g. openagents.com), you must pass `--prod`.

- **Dev (default):** `npx convex logs` streams from the project's dev deployment.
- **Prod:** `npx convex logs --prod` streams from the project's production deployment.

Same for env vars: `npx convex env list` shows dev; `npx convex env list --prod` shows production.

## Commands

### Stream or fetch logs

```bash
# From the app that has convex/ (e.g. apps/web)
cd apps/web

# Stream logs (default: dev). Use Ctrl+C to stop.
npx convex logs

# Stream production logs
npx convex logs --prod

# Show last N log entries then exit (no streaming). Good for quick checks.
npx convex logs --prod --history 30

# Include successful executions (default is failures only in some contexts)
npx convex logs --prod --history 25 --success
```

### Log command options

| Option | Meaning |
|--------|--------|
| `--prod` | Use the **production** deployment (required for prod debugging). |
| `--history [n]` | Show the last `n` log entries; then exit instead of streaming. |
| `--success` | Include successful function runs in the output. |
| `--jsonl` | Output raw log events as JSONL. |
| `--deployment-name <name>` | Target a specific deployment (e.g. `dev:effervescent-anteater-82`). |
| `--env-file <path>` | Use env file for CONVEX_DEPLOYMENT or similar. |

### Inspect environment variables

Convex actions do **not** read `.env.local`. They only see variables set in the Convex deployment (Dashboard or CLI). To see what the deployment has:

```bash
# List env vars on dev (default)
npx convex env list

# List env vars on production
npx convex env list --prod

# Get a single variable (e.g. confirm OA_INTERNAL_KEY is set; value is hidden in output)
npx convex env get OA_INTERNAL_KEY --prod
```

Use this to confirm that keys and URLs (e.g. `OA_INTERNAL_KEY`, `PUBLIC_API_URL`) are set on the deployment that's actually serving the app.

## Typical workflow

1. **Reproduce the issue** (e.g. open Hatchery, trigger an action that returns 401).
2. **Fetch recent prod logs** (if the app is production):
   ```bash
   cd apps/web
   npx convex logs --prod --history 30 --success
   ```
3. **Find the failing function** in the output (e.g. `[CONVEX A(openclawApi:getRuntimeStatus)] [ERROR] ... 401 'unauthorized'`).
4. **Confirm which deployment** is used: logs show the deployment name (e.g. `successful-mongoose-647` for prod).
5. **Check env vars** for that deployment: `npx convex env list --prod` (or without `--prod` for dev).
6. **Fix** env in Convex (Dashboard or `npx convex env set ...`) or fix the calling code; redeploy if needed.

## Example log output

```
Watching logs for production deployment successful-mongoose-647...
2/4/2026, 12:39:06 PM [CONVEX A(openclawApi:getInstance)] Function executed in 434 ms
2/4/2026, 12:39:06 PM [CONVEX A(openclawApi:getRuntimeStatus)] [ERROR] '[openclawApi getRuntimeStatus] API error response:' 401 'unauthorized'
2/4/2026, 12:39:06 PM [CONVEX A(openclawApi:getRuntimeStatus)] Uncaught Error: unauthorized ...
```

Here, `getInstance` succeeded but `getRuntimeStatus` got 401 from the external API—often a mismatch of `OA_INTERNAL_KEY` between Convex and the API worker.

## Troubleshooting

- **"No logs" or wrong deployment:** Use `--prod` when the live app uses the production Convex deployment.
- **401 from Convex actions calling an API:** Set `OA_INTERNAL_KEY` (and `PUBLIC_API_URL` if needed) in the **same** Convex deployment that runs the action (`npx convex env set OA_INTERNAL_KEY "..." --prod`), and ensure the API worker has the same key (e.g. `npx wrangler secret put OA_INTERNAL_KEY` in the API app).
- **Timeout when streaming:** Use `--history N` to fetch a finite number of entries and exit instead of streaming.
- **Need a specific deployment:** Use `--deployment-name <name>` (e.g. `dev:effervescent-anteater-82`) for that deployment's logs.
