# `@openagentsinc/grok-harness`

Grok Build CLI as a Khala Code multi-harness adapter (MH-3 / MH-4).

This package is a narrow Grok fixture, not OpenAgents' shared Agent Client
Protocol implementation. It currently covers outbound
initialize/auth/session/prompt requests and delegates validated `session/update`
admission/projection to [`agent-client-runtime-bridge`](../agent-client-runtime-bridge/README.md).
Capabilities are false by default. This package does not itself install the bidirectional permission, filesystem, or
terminal surface required for general Agent Client Protocol support. Grok's
current source uses Rust `agent-client-protocol` 0.10.4 with unstable features,
resolving schema 0.11.4, while the OpenAgents target starts from current stable
wire version 1 and `schema-v1.19.0`. See the
[T3 Code Agent Client Protocol implementation teardown](../../docs/teardowns/2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md)
for the compatibility verdict and ordered replacement plan.

`mock-acp-server` is retained only for the existing RL-worker/chat-runtime
contract. It is randomized, narrow, and is not wire conformance evidence. New
wire cases belong in
[`agent-client-protocol-conformance`](../agent-client-protocol-conformance/README.md),
which uses the generated protocol authority and production stdio transport.

## Scope

| Module             | Role                                                                      |
| ------------------ | ------------------------------------------------------------------------- |
| `mock-acp-server`  | In-process ACP fixture (initialize → authenticate → session/new → prompt) |
| `acp-client`       | JSON-RPC stdio client for real `grok agent stdio` or mock                 |
| `event-projector`  | Shared canonical bridge output → legacy neutral chat compatibility events |
| `chat-runtime`     | Axis A: startThread / startTurn / interruptTurn                           |
| `session-store`    | desktop session ↔ Grok session id mapping                                 |
| `worker-executor`  | Axis B: claim-shaped worker run behind a pylon-core-shaped port           |
| `readiness`        | `grok version` / models / auth plane probe                                |
| `rate-limit-probe` | RL-1..2 concurrent CLI plane measurement                                  |

## Scripts

```bash
# Unit / fixture tests (no network, no live Grok required)
pnpm --dir packages/grok-harness run test

# Live RL probe (uses local grok login / free window)
pnpm --dir packages/grok-harness run rl-probe -- --concurrency 1,2,4,8 --prompt "Reply with only: ok"
```

## Issues

- MH-3: OpenAgentsInc/openagents#8589
- MH-4: OpenAgentsInc/openagents#8590
