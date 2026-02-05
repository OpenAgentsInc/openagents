# nydus

Nydus is a Bun-based handshake script for the LiteClaw tunnel executor. It validates that a Cloudflare Tunnel + Access-protected hostname can reach the local `liteclaw-local-agent` and run basic workspace tools.

## Install

```bash
bun install
```

## Run

```bash
cd apps/nydus
LITECLAW_TUNNEL_URL=https://local-tools.example.com \
LITECLAW_TUNNEL_TOKEN=replace-me \
CF_ACCESS_CLIENT_ID=optional \
CF_ACCESS_CLIENT_SECRET=optional \
bun run index.ts
```

## Environment

Required:

- `LITECLAW_TUNNEL_URL` (tunnel hostname, e.g. `https://burrow.openagents.com`)
- `LITECLAW_TUNNEL_TOKEN` (shared bearer token for `/tools/invoke`)

Optional:

- `CF_ACCESS_CLIENT_ID` + `CF_ACCESS_CLIENT_SECRET` (Access service token headers)
- `LITECLAW_TUNNEL_ACCESS_CLIENT_ID` + `LITECLAW_TUNNEL_ACCESS_CLIENT_SECRET` (same as above, alternate names)
- `LITECLAW_TUNNEL_TEST_PATH` (defaults to `output/liteclaw/nydus-handshake.txt`)
- `LITECLAW_TUNNEL_TEST_CONTENT` (defaults to `nydus-handshake-ok`)
- `LITECLAW_TUNNEL_TIMEOUT_MS` (defaults to `8000`)

The script performs a `/health` check and then runs `workspace.write/read/edit` against the local agent using the tunnel.
