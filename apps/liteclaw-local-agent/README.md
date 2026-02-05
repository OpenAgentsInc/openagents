# LiteClaw Local Agent

A minimal local executor that exposes `workspace.read`, `workspace.write`, and `workspace.edit` over HTTP for LiteClaw tunnel mode.

## Usage

```bash
cd apps/liteclaw-local-agent
LITECLAW_LOCAL_ROOT=/path/to/your/repo \
LITECLAW_TUNNEL_TOKEN=replace-me \
npm run dev
```

Optional env vars:

- `PORT` (default: `8787`)
- `LITECLAW_LOCAL_ALLOWED_TOOLS` (default: `workspace.read,workspace.write,workspace.edit`)
- `LITECLAW_LOCAL_MAX_BYTES` (default: `200000`)
- `LITECLAW_LOCAL_MAX_BODY_BYTES` (default: `1000000`)

## Tunnel smoke test

Use the tunnel smoke script after publishing the tunnel hostname:

```bash
LITECLAW_TUNNEL_URL=https://local-tools.example.com \
LITECLAW_TUNNEL_TOKEN=replace-me \
node scripts/tunnel-smoke.js
```

## Worker configuration

Set these on the LiteClaw worker to route workspace tools through the tunnel:

- `LITECLAW_EXECUTOR_KIND=tunnel`
- `LITECLAW_TUNNEL_URL=https://<your-tunnel-host>`
- `LITECLAW_TUNNEL_TOKEN=replace-me`

Expose the local agent via a tunnel (example):

```bash
cloudflared tunnel --url http://localhost:8787
```

Keep the tunnel token private and rotate it to revoke access.
