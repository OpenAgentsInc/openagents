# nydus

Nydus is a Bun-based demo harness for LiteClaw tunnel tools. It can run a direct tunnel handshake and a cloud-driven demo where a LiteClaw agent message triggers local workspace tools.

## Install

```bash
bun install
```

## Handshake (direct tunnel)

```bash
cd apps/nydus
LITECLAW_TUNNEL_URL=https://local-tools.example.com \
LITECLAW_TUNNEL_TOKEN=replace-me \
CF_ACCESS_CLIENT_ID=optional \
CF_ACCESS_CLIENT_SECRET=optional \
bun run index.ts
```

This runs `/health` plus `workspace.write/read/edit` directly through the tunnel.

## Cloud Demo (agent message -> local tools)

Prereqs:
- `liteclaw-local-agent` running with `LITECLAW_TUNNEL_TOKEN` and `LITECLAW_LOCAL_ROOT`.
- `cloudflared` running and publishing the hostname.
- LiteClaw worker configured with `LITECLAW_EXECUTOR_KIND=tunnel`, `LITECLAW_TUNNEL_URL`, and `LITECLAW_TUNNEL_TOKEN`.
- Tool policy set to `read-write` for the thread (or provide `LITECLAW_TOOL_ADMIN_SECRET`).

Run:

```bash
cd apps/nydus
LITECLAW_AGENT_BASE_URL=https://openagents.com \
LITECLAW_TOOL_ADMIN_SECRET=replace-me \
NYDUS_LOCAL_ROOT=/path/to/your/repo \
bun run index.ts cloud
```

The script sends a message to the LiteClaw agent and waits for the agent to call `workspace.write/read` through the tunnel. It then verifies the local file content and (by default) checks the `/export` receipts for workspace tool usage.

## Modes

- `handshake` (default): direct tunnel tool invocation.
- `cloud`: agent message triggers local tools.
- `full`: run both handshake and cloud demo.

You can pass the mode as the first argument or via `NYDUS_MODE`.

## Environment

Required (handshake mode):
- `LITECLAW_TUNNEL_URL`
- `LITECLAW_TUNNEL_TOKEN`

Optional:
- `CF_ACCESS_CLIENT_ID` + `CF_ACCESS_CLIENT_SECRET` (Access service token headers)
- `LITECLAW_TUNNEL_ACCESS_CLIENT_ID` + `LITECLAW_TUNNEL_ACCESS_CLIENT_SECRET` (alternate names)
- `LITECLAW_TUNNEL_TEST_PATH` (defaults to `output/liteclaw/nydus-handshake.txt`)
- `LITECLAW_TUNNEL_TEST_CONTENT` (defaults to `nydus-handshake-ok`)
- `LITECLAW_TUNNEL_TIMEOUT_MS` (defaults to `8000`)

Cloud demo options:
- `LITECLAW_AGENT_BASE_URL` (defaults to `https://openagents.com`)
- `LITECLAW_AGENT_THREAD_ID` (defaults to `nydus-<timestamp>`)
- `LITECLAW_TOOL_ADMIN_SECRET` (sets tool policy to `read-write`)
- `NYDUS_LOCAL_ROOT` (defaults to repo root)
- `NYDUS_CLOUD_PATH` (defaults to `output/liteclaw/nydus-sky-tool-<id>.txt`)
- `NYDUS_CLOUD_CONTENT` (defaults to `nydus-sky-tool-ok-<id>`)
- `NYDUS_MESSAGE_TIMEOUT_MS` (defaults to `60000`)
- `NYDUS_TTFT_LIMIT_MS` (defaults to `10000`)
- `NYDUS_FILE_WAIT_MS` (defaults to `15000`)
- `NYDUS_CHECK_EXPORT` (`0` to skip export receipt checks)
- `NYDUS_REQUIRE_LOCAL_RECEIPT` (`0` to allow non-tunnel receipts)
