# LiteClaw Tunnel Executor

This document explains how to use the tunnel-backed executor so LiteClaw can read and edit local files. For Cloudflare Tunnel architecture and Access patterns, see `docs/liteclaw/cloudflare-tunnel.md`.

## What The Tunnel Does

The tunnel executor routes workspace tool calls from the LiteClaw worker to a local HTTP server (`liteclaw-local-agent`). It enables `workspace.read`, `workspace.write`, and `workspace.edit` against a local directory while preserving the Sky tool receipt contract.

## When To Use It

Use the tunnel executor when you want LiteClaw to operate on local repos without container infrastructure.

## Components

- LiteClaw worker (`apps/liteclaw-worker/`) dispatches tool calls to the tunnel.
- Local agent (`apps/liteclaw-local-agent/`) executes file operations with path scoping.
- Cloudflared tunnel (or equivalent) exposes the local agent to the worker.

## Setup

1. Start the local agent.

```bash
cd apps/liteclaw-local-agent
LITECLAW_LOCAL_ROOT=/path/to/your/repo \
LITECLAW_TUNNEL_TOKEN=replace-me \
npm run dev
```

2. Expose the local agent with a tunnel.

```bash
cloudflared tunnel --url http://localhost:8787
```

3. Configure the LiteClaw worker.

```bash
LITECLAW_EXECUTOR_KIND=tunnel
LITECLAW_TUNNEL_URL=https://<your-tunnel-host>
LITECLAW_TUNNEL_TOKEN=replace-me
```

4. Ensure tool policy allows write tools.

```bash
curl -sS -X POST \
  -H "content-type: application/json" \
  -H "x-liteclaw-admin-secret: $LITECLAW_TOOL_ADMIN_SECRET" \
  -d '{"policy":"read-write"}' \
  https://<host>/agents/chat/<thread-id>/tool-policy
```

## How It Works

1. The model requests a tool call.
2. The worker sends a signed JSON payload to `POST /tools/invoke` on the tunnel.
3. The local agent performs the file operation inside `LITECLAW_LOCAL_ROOT`.
4. The local agent returns `output` and a signed receipt.
5. The worker verifies the signature, hashes, and emits Sky tool receipts.

The worker expects the local receipt signature to match the HMAC of the receipt payload using `LITECLAW_TUNNEL_TOKEN`.

## Security Controls

- `LITECLAW_TUNNEL_TOKEN` is required for authentication and receipt signing.
- `LITECLAW_LOCAL_ROOT` limits the workspace scope.
- `LITECLAW_LOCAL_ALLOWED_TOOLS` limits tool usage.
- Tool budgets and allowlists still apply in the worker.

Rotate the tunnel token to revoke access.

## Local Agent Configuration

Optional env vars supported by `apps/liteclaw-local-agent`:

- `PORT` (default `8787`).
- `LITECLAW_LOCAL_ALLOWED_TOOLS` (default `workspace.read,workspace.write,workspace.edit`).
- `LITECLAW_LOCAL_MAX_BYTES` (default `200000`).
- `LITECLAW_LOCAL_MAX_BODY_BYTES` (default `1000000`).

## Verifying The Tunnel

Run the smoke harness with tunnel support:

```bash
cd apps/liteclaw-worker
LITECLAW_SMOKE_EXECUTOR_KIND=tunnel \
LITECLAW_SMOKE_ADMIN_SECRET=replace-me \
LITECLAW_EXECUTOR_KIND=tunnel \
LITECLAW_TUNNEL_URL=https://<your-tunnel-host> \
LITECLAW_TUNNEL_TOKEN=replace-me \
LITECLAW_TOOL_ADMIN_SECRET=replace-me \
LITECLAW_HTTP_ALLOWLIST=example.com \
LITECLAW_EXTENSION_ALLOWLIST=sky.echo@0.1.0 \
LITECLAW_EXTENSION_ADMIN_SECRET=replace-me \
npm run smoke
```

## Troubleshooting

- `Tunnel executor is not configured.`
Set `LITECLAW_TUNNEL_URL` and `LITECLAW_TUNNEL_TOKEN` on the worker.

- `Invalid tunnel receipt signature.`
Ensure the worker and local agent share the same `LITECLAW_TUNNEL_TOKEN`.

- `Workspace file not found.`
Check `LITECLAW_LOCAL_ROOT` and ensure the path exists.

- `Tool access is read-only for this thread.`
Set tool policy to `read-write` via `/tool-policy`.

- `Outbound tool data budget exceeded for this run.`
Increase `LITECLAW_TOOL_MAX_OUTBOUND_BYTES` or reduce tool outputs.
