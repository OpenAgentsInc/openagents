# Bare-Agent MCP to Khala Runbook

Date: 2026-06-25

Issue: https://github.com/OpenAgentsInc/openagents/issues/6286

Public-safety boundary: this runbook contains only public-safe command shapes,
fixture refs, route names, and expected schemas. It contains no bearer token,
private URL, raw prompt, raw response, hidden trace, private source, wallet
material, provider credential, or customer data.

## What This Proves

The portable path is:

```text
bare MCP client
  -> khala.request
  -> agent bearer principal
  -> caller-owned linked Pylon capacity
  -> Pylon assignment lease
  -> durable Khala stream handle
  -> khala.resume / khala.status
```

The important invariant is that the token is the scope. A second account may
have its own linked Pylon capacity and may hold the same opaque durable request
id string, but the MCP layer refuses resume/status unless that durable id is
attached to one of the caller's own linked Pylon assignments.

## Fixture Smoke

Run the no-live-spend harness from a clean checkout:

```sh
bun run --cwd apps/openagents.com/workers/api test -- src/coding-capacity-validation.test.ts -t "P4/P6"
```

The fixture covers:

- browser account links an agent token to the account;
- MCP-issued `khala.request` targets a caller-owned Codex Pylon;
- the Worker creates a `unpaid_smoke` Pylon assignment;
- the MCP result returns `assignmentRef`, `durableRequestId`, and
  `durableStreamUrl`;
- `khala.resume` replays the durable suffix without metering;
- another account receives `durable_request_not_authorized` before any durable
  fetch runs.

For the smaller MCP catalog unit suite:

```sh
bun run --cwd apps/openagents.com/workers/api test -- src/khala-mcp.test.ts
```

## MCP Config

Generate a user-facing config block:

```sh
pylon mcp config --json
```

The local stdio shape is:

```json
{
  "mcpServers": {
    "openagents-khala-local": {
      "command": "pylon",
      "args": ["mcp"],
      "env": {
        "OPENAGENTS_AGENT_TOKEN": "${OPENAGENTS_AGENT_TOKEN}",
        "PYLON_OPENAGENTS_BASE_URL": "https://openagents.com"
      }
    }
  }
}
```

The remote HTTP shape is:

```json
{
  "mcpServers": {
    "openagents-khala-remote": {
      "type": "http",
      "url": "https://openagents.com/api/mcp",
      "headers": {
        "Authorization": "Bearer ${OPENAGENTS_AGENT_TOKEN}"
      }
    }
  }
}
```

Do not paste real tokens into committed config. The agent runtime should resolve
`OPENAGENTS_AGENT_TOKEN` from its local secret environment.

## Live Smoke

Use a negative target first. This proves auth and own-capacity denial without
creating an assignment or spending:

```sh
pylon khala request \
  --prompt "Remote-token capacity authorization smoke" \
  --workflow codex_agent_task \
  --pylon-ref pylon.not_linked.authorization_smoke \
  --json
```

Expected result: non-zero JSON containing `target_pylon_not_authorized` or the
equivalent not-linked target reason.

For an owner-approved positive smoke, target a real caller-owned,
heartbeat-fresh Codex Pylon:

```sh
pylon khala request \
  --prompt "Run the public-safe fixture task" \
  --workflow codex_agent_task \
  --pylon-ref <caller-owned-pylon-ref> \
  --json
```

Expected result: JSON with `assignmentRef`, `durableRequestId`, and
`durableStreamUrl`. A bare agent should resume through the MCP tool, not by
sharing the low-level durable read URL directly:

```json
{
  "jsonrpc": "2.0",
  "id": "resume-1",
  "method": "tools/call",
  "params": {
    "name": "khala.resume",
    "arguments": {
      "durableRequestId": "<durableRequestId>",
      "offset": 0
    }
  }
}
```

The local stdio MCP server proxies `khala.resume` and `khala.status` through
the remote `/api/mcp` surface so the Worker can check assignment ownership
before reading the durable suffix. The resume path must not meter. If the same
durable id is presented under a different account token, the MCP surface should
return `durable_request_not_authorized`.
