# Extension Capabilities (ACPExt) Negotiation

OpenAgents uses ACPExt capability negotiation during `initialize` to advertise support for non-core (dotted) methods such as `orchestrate.explore.*`.

## Initialize Response

During handshake the desktop server includes extension flags under `agent_capabilities.ext_capabilities`:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocol_version": "0.2.2",
    "agent_capabilities": {
      "ext_capabilities": {
        "orchestrate_explore": true
      }
    }
  }
}
```

If the server cannot support the capability (e.g., missing platform features), it will advertise `false`.

## Gating Behavior

- Calls to `orchestrate.explore.*` are accepted only when `ext_capabilities.orchestrate_explore == true`.
- If not supported/advertised, the server responds with a JSON-RPC error:

```json
{"jsonrpc":"2.0","id":"<id>","error":{"code":-32601,"message":"orchestrate.explore not supported"}}
```

## Examples

Start request:

```json
{
  "jsonrpc": "2.0",
  "id": "orch-1",
  "method": "orchestrate.explore.start",
  "params": {
    "root": "/Users/me/code/project",
    "policy": { "allow_external_llms": false, "allow_network": false }
  }
}
```

Success response (when supported):

```json
{
  "jsonrpc": "2.0",
  "id": "orch-1",
  "result": { "session_id": "...", "plan_id": "...", "status": "started" }
}
```

See also: `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/agent.swift` for capability types.

