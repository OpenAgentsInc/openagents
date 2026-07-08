# `@openagentsinc/grok-harness`

Grok Build CLI as a Khala Code multi-harness adapter (MH-3 / MH-4).

**Does not edit `agent-runtime-schema`.** Consumes MH-0 contracts when they
land; until then uses provisional local literals that match the planned
shape (`grok_cli`, `marginal_cost_class`, typed failures).

## Scope

| Module | Role |
| --- | --- |
| `mock-acp-server` | In-process ACP fixture (initialize → authenticate → session/new → prompt) |
| `acp-client` | JSON-RPC stdio client for real `grok agent stdio` or mock |
| `event-projector` | ACP `session/update` chunks → neutral chat turn events |
| `chat-runtime` | Axis A: startThread / startTurn / interruptTurn |
| `session-store` | desktop session ↔ Grok session id mapping |
| `worker-executor` | Axis B: claim-shaped worker run behind a pylon-core-shaped port |
| `readiness` | `grok version` / models / auth plane probe |
| `rate-limit-probe` | RL-1..2 concurrent CLI plane measurement |

## Scripts

```bash
# Unit / fixture tests (no network, no live Grok required)
bun run --cwd packages/grok-harness test

# Live RL probe (uses local grok login / free window)
bun run --cwd packages/grok-harness rl-probe -- --concurrency 1,2,4,8 --prompt "Reply with only: ok"
```

## Issues

- MH-3: OpenAgentsInc/openagents#8589
- MH-4: OpenAgentsInc/openagents#8590
