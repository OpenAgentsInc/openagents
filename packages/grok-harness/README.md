# `@openagentsinc/grok-harness`

Grok Build CLI as a Khala Code multi-harness adapter (MH-3 / MH-4).

The production chat facade controls Grok through the shared Agent Client
Protocol transport and session runtime. It admits only the trusted
`grok agent stdio` launch, probes and pins executable identity, negotiates
advertised authentication, streams `session/update`, settles the prompt stop
reason once, and uses protocol cancellation before process shutdown.
Cached-token remains preferred when advertised. Interactive `grok.com` and
enterprise `oidc` methods require an explicit typed owner continuation; the
default and cancellation paths stop before `authenticate`.
An existing local Grok login is sufficient for headless ACP operation; no API
key is required. `xai.api_key` is only an optional, explicitly configured
alternative.
An explicit `requestedInteractiveAuthMethod` can select an advertised
interactive method ahead of an existing cached token, allowing a user-requested
login or cancellation without deleting cached credentials.

Capabilities are false unless supported admission evidence and an installed
authority broker both authorize them. The raw `acp-client` and in-process mock
remain fixture-only compatibility seams; caller-provided command arrays are
refused by the production path. The terminal `worker-executor` remains the
separate RL/claimed-work contract and is not a generic ACP implementation.

Grok's current source uses Rust `agent-client-protocol` 0.10.4 with unstable
features, resolving schema 0.11.4, while OpenAgents uses stable wire version 1
and `schema-v1.19.0`. See the
[T3 Code Agent Client Protocol implementation teardown](../../docs/teardowns/2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md)
for the compatibility verdict and ordered replacement plan.

`mock-acp-server` is retained only for the existing RL-worker/chat-runtime
contract. It is randomized, narrow, and is not wire conformance evidence. New
wire cases belong in
[`agent-client-protocol-conformance`](../agent-client-protocol-conformance/README.md),
which uses the generated protocol authority and production stdio transport.

## Scope

| Module              | Role                                                                      |
| ------------------- | ------------------------------------------------------------------------- |
| `mock-acp-server`   | In-process ACP fixture (initialize → authenticate → session/new → prompt) |
| `grok-peer-runtime` | Trusted admission plus shared transport/session runtime composition       |
| `acp-client`        | Deprecated raw JSON-RPC fixture client; never the production launch path  |
| `event-projector`   | Shared canonical bridge output → legacy neutral chat compatibility events |
| `chat-runtime`      | Axis A: startThread / startTurn / interruptTurn                           |
| `session-store`     | desktop session ↔ Grok session id mapping                                 |
| `worker-executor`   | Axis B: claim-shaped worker run behind a pylon-core-shaped port           |
| `readiness`         | `grok version` / models / auth plane probe                                |
| `rate-limit-probe`  | RL-1..2 concurrent CLI plane measurement                                  |

## Scripts

```bash
# Unit / fixture tests (no network, no live Grok required)
pnpm --dir packages/grok-harness run test

# Opt-in live candidate smoke; retains no prompt or response text artifact
GROK_ACP_LIVE=1 pnpm --dir packages/grok-harness run live-acp-smoke

# Live RL probe (uses local grok login / free window)
pnpm --dir packages/grok-harness run rl-probe -- --concurrency 1,2,4,8 --prompt "Reply with only: ok"
```

## Issues

- MH-3: OpenAgentsInc/openagents#8589
- MH-4: OpenAgentsInc/openagents#8590
- ACP-6: OpenAgentsInc/openagents#8893
