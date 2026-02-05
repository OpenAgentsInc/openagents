# Cloudflare Tunnel + LiteClaw

This doc explains how Cloudflare Tunnel connects your local machine to LiteClaw running on Workers, and how to secure that connection.

## Summary

Cloudflare Tunnel lets your local computer create outbound-only connections to Cloudflare. Once the tunnel is running, Cloudflare can route requests for a public hostname to your local service. That makes it possible for a Worker (LiteClaw) to call your local executor over HTTPS or WebSocket without exposing a public IP or opening inbound firewall ports.

## Architecture

```
LiteClaw Worker (Workers + Agents SDK)
  -> https://local-tools.example.com/tools/invoke
  -> Cloudflare edge
  -> Cloudflare Tunnel (cloudflared)
  -> http://localhost:8787 (liteclaw-local-agent)
```

Key points:

- `cloudflared` is outbound-only. Your local machine initiates the connection to Cloudflare.
- The hostname (for example, `local-tools.example.com`) is a published route for the tunnel.
- The LiteClaw worker treats the hostname like any other HTTPS origin.

## Security Models

### 1) Shared secret only (simpler)

- Protect `/tools/invoke` with a bearer token.
- The worker sends `Authorization: Bearer <token>`.
- The local agent validates the token and signs the tool receipt.

This is easiest to configure but still exposes the hostname to the public internet. Anyone who can reach the hostname can probe it, so the token must remain private.

### 2) Cloudflare Access + service tokens (cleaner)

- Place the hostname behind Cloudflare Access.
- Create a service token in Cloudflare Access.
- The worker includes `CF-Access-Client-Id` and `CF-Access-Client-Secret` headers.

This locks access to the hostname so only your worker (or other allowed services) can reach it.

### 3) Access + shared secret (recommended for LiteClaw)

Use Access for network-level protection and still require `Authorization: Bearer <token>` for app-level authentication. This matches the current LiteClaw tunnel executor contract.

## LiteClaw Phase 4 Pattern

1. Run the local agent on your machine.
2. Run `cloudflared` to publish a hostname to the local agent.
3. Configure the LiteClaw worker to call the hostname via `LITECLAW_TUNNEL_URL` and `LITECLAW_TUNNEL_TOKEN`.
4. Optionally add Access service token headers for extra protection.

## Required Headers

The worker must include:

- `Authorization: Bearer <LITECLAW_TUNNEL_TOKEN>`

If Access is enabled, also include:

- `CF-Access-Client-Id: <service-token-id>`
- `CF-Access-Client-Secret: <service-token-secret>`

The LiteClaw worker can forward these via env vars:

- `LITECLAW_TUNNEL_ACCESS_CLIENT_ID`
- `LITECLAW_TUNNEL_ACCESS_CLIENT_SECRET`

## Tunnel Executor Contract

The worker sends requests to `POST /tools/invoke` with payload:

```json
{
  "tool_name": "workspace.read",
  "tool_call_id": "uuid",
  "run_id": "uuid",
  "thread_id": "chat-id",
  "args": { "path": "README.md" }
}
```

The local agent returns:

```json
{
  "ok": true,
  "output": { "content": "..." },
  "receipt": {
    "tool_name": "workspace.read",
    "args_hash": "...",
    "output_hash": "...",
    "patch_hash": null,
    "executor_kind": "tunnel",
    "started_at": 0,
    "completed_at": 0,
    "duration_ms": 0
  },
  "signature": "hmac"
}
```

The worker verifies the signature and hashes before emitting Sky receipts.

## Testing

Use the tunnel smoke script to validate the connection:

```bash
cd apps/liteclaw-local-agent
LITECLAW_TUNNEL_URL=https://local-tools.example.com \
LITECLAW_TUNNEL_TOKEN=replace-me \
CF_ACCESS_CLIENT_ID=optional \
CF_ACCESS_CLIENT_SECRET=optional \
node scripts/tunnel-smoke.js
```

The script calls `/health` plus `workspace.write/read/edit` through the tunnel and fails fast if any request is rejected.

## Common Failure Modes

- `401 Unauthorized`: bearer token mismatch.
- `403 Forbidden`: Access policy or service token mismatch.
- `404 Not found`: tunnel route is misconfigured or hostname mismatch.
- `Invalid tunnel receipt signature`: token mismatch between worker and local agent.

## Related Docs

- `docs/liteclaw/tunnel.md` for the LiteClaw-specific setup and troubleshooting.
- `apps/liteclaw-local-agent/README.md` for local agent env vars.
