# Meta-agent loopback ACP server — Zed demo (META-2, #9181)

OpenAgents Desktop can show the meta-agent as an ACP agent. It does this on a
loopback server. This inverts the four ACP client peers that Desktop already
uses.

Desktop is normally an ACP client of an external agent. Here an external ACP
host drives the OpenAgents meta-agent instead. Zed is one such host. Our own ACP
client is another.

- Module: `apps/openagents-desktop/src/meta-agent-acp-server.ts`
- Protocol core: the SDK `makeAcpAgentServerConnection`
  (`@openagentsinc/agent-harness-contract`, ai#39) over the real
  `metaAgentHarness`.
- Behavior contract:
  `openagents_desktop.meta_agent.loopback_acp_server_default_off.v1`.

## Security posture

Four rules always hold.

1. Default OFF. The server starts only when `OPENAGENTS_DESKTOP_ACP_SERVER=1`.
   With the flag unset there is no listener and no behavior change.
2. Loopback only. The listener binds `127.0.0.1`. It refuses any other host as a
   construction invariant. ACP has no bearer. The server sends an empty
   `authMethods` list. Loopback plus deny-by-default are the v0 boundary.
3. Deny-by-default permissions. The v0 decider denies each gated tool call. A
   gated tool call has `operator_escalation_required` authority. The decider
   does not ask the connected ACP client. No gated tool runs without an explicit
   owner permission broker (the `decidePermission` option). The ACP surface is
   never a bypass.
4. Read-only shape. The surface is a conversation and prompt surface. It is
   `operator_read`-shaped. It is never a mutation, credential, settlement,
   release, or public-claim path.

## Framing

The server speaks newline-delimited JSON-RPC 2.0. This is ACP ndjson. Each
inbound line is one message object. Each outbound message is one JSON object plus
a newline. One accepted TCP socket is one ACP connection.

## Start the server (owner-run)

Start Desktop, or a dev launch, with the gate on:

```sh
OPENAGENTS_DESKTOP_ACP_SERVER=1 \
OPENAGENTS_DESKTOP_ACP_SERVER_PORT=4517 \
  oa-dev            # or your normal Desktop launch
```

`OPENAGENTS_DESKTOP_ACP_SERVER_PORT` is optional. Omit it for an ephemeral port.
Desktop main prints the loopback endpoint on start:

```
[openagents-desktop meta-agent-acp] listening tcp://127.0.0.1:4517
```

## Point Zed at the loopback server

Zed launches an ACP agent as a command. That command speaks ACP over stdio. A
one-line pipe connects Zed stdio to the loopback TCP port. Add this block to Zed
`settings.json`:

```json
{
  "agent_servers": {
    "OpenAgents": {
      "command": "sh",
      "args": ["-c", "exec nc 127.0.0.1 4517"]
    }
  }
}
```

`nc` (or `ncat`) ships on macOS and most Linux hosts. Any transparent stdio-to-TCP
bridge works. `socat - TCP:127.0.0.1:4517` is one alternative. In Zed, select the
**OpenAgents** agent and start a thread.

Zed runs the ACP handshake (`initialize`, then `session/new`). Each message you
send becomes one `session/prompt` turn. The meta-agent text arrives as
`session/update` `agent_message_chunk` notifications. The turn ends with an ACP
`stopReason`. A gated tool call raises a `session/request_permission` request.
The v0 deny-by-default decider refuses that call first, so Zed is never asked.

Some ACP hosts can dial a TCP endpoint directly. Point such a host at
`tcp://127.0.0.1:<port>` and drop the bridge command. The wire protocol is the
same either way.

## Headless conformance (no Zed required)

Desktop consumes four ACP peers in production. Our own ACP client adapter is a
free conformance oracle for the server. The suite
`apps/openagents-desktop/src/meta-agent-acp-server.test.ts` starts the real
loopback server. It connects a real TCP ACP client. It drives the client with
the SDK `makeAcpHarnessAdapter`. The suite proves five facts.

- The full turn projects a contiguous khala stream (`turn.started`, deltas,
  `turn.finished`, sequences `0..N`). This is the same adapter law every other
  adapter passes, so the composed harness is conformant.
- Consecutive prompt turns each end with their own stop reason.
- The v0 backing is the real `metaAgentHarness` fleet contract, not a stub.
- A gated tool call is denied and never runs. The desktop decider does not
  delegate the approval to the client. The wire still carries the honest
  `tool_call` update.
- The gate is default-off. The loopback guard refuses any non-loopback host.

Run it:

```sh
node_modules/.bin/vp test --run apps/openagents-desktop/src/meta-agent-acp-server.test.ts
```

## v0 vs deferred (real-fleet backing)

v0 landed:

- A loopback ACP server, default-off, with deny-by-default permissions.
- The real SDK `metaAgentHarness` over a fixture echo member
  (`makeReferenceAdapter`).
- Conformance through our own ACP client adapter.
- The Zed steps above.

v1 deferred (#9179):

- Backing the server with the live Codex, Claude, and Grok member harnesses.
- The plug-in point is the module `makeHarness` option. Desktop main passes a
  factory. The factory returns `metaAgentHarness({ members, route })` over the
  dispatch lane's real member harnesses.
- That wiring must reuse the existing dispatch runtime files. The default-on
  dispatch-collapse lane owns those files. So the wiring is a separate change,
  not part of META-2 v0.
