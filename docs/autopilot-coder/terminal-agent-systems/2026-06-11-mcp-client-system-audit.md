# MCP Client System Audit

Date: 2026-06-11

This is system #28 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should connect to external MCP servers, project
their tools, resources, prompts, and skills into the agent runtime, and enforce
per-server trust.

## Target

Build an MCP client system that treats every external server as a scoped,
policy-governed capability provider.

The agent should expose MCP tools and resources through the same typed runtime
shape as native tools, while preserving server identity, transport state,
authentication state, approval state, and privacy boundaries.

## User-Visible Capability

The user should be able to:

- Add MCP servers at user, project, local, managed, session, or extension
  scope.
- Approve project-provided servers before they run.
- See connected, pending, disabled, failed, and needs-auth states.
- Reconnect or disable individual servers.
- Authenticate remote servers when they require OAuth or token refresh.
- Browse server tools, resources, prompts, and skill-like entries.
- See transport and capability summaries without exposing secrets.
- Receive clear errors for invalid config, auth failure, transport failure, or
  duplicate server suppression.
- Use MCP-provided tools through normal approval and permission flow.

The MCP client should make server trust explicit. A connected server is not the
same as a fully trusted server.

## Configuration Model

MCP server config should include:

- Server id.
- Scope.
- Transport: stdio, streamable HTTP, SSE, WebSocket, in-process adapter, or
  managed remote adapter.
- Command and args for local process transports.
- URL for network transports.
- Header config or header-helper ref.
- OAuth config or auth-provider ref.
- Enabled or disabled state.
- Extension provider ref when the server is extension-owned.
- Policy refs.
- Approval status.

Config merging should be deterministic. Manual user or project config should
win over duplicate extension-provided config unless an explicit policy says
otherwise.

## Connection Lifecycle

Use a typed lifecycle:

1. Load scoped config.
2. Validate schema.
3. Apply policy and project-approval gates.
4. Mark disabled, pending, failed, or connectable.
5. Establish transport.
6. Initialize server.
7. Fetch capabilities.
8. Normalize tool, resource, prompt, and skill names.
9. Register projected capabilities with the agent runtime.
10. Watch list-change notifications.
11. Reconnect when remote transports close.
12. Cleanup on disable, session end, or config change.

Connection updates should be batched so many servers do not thrash UI state.

## Core Design

Define an `McpClientService` that owns config loading, connection management,
capability projection, and reconnection.

Suggested service boundary:

```ts
interface McpClientService {
  loadConfig(request: McpConfigLoadRequest): Effect.Effect<McpConfigSet, McpClientError>
  connect(request: McpConnectRequest): Effect.Effect<McpConnectionReceipt, McpClientError>
  reconnect(request: McpReconnectRequest): Effect.Effect<McpConnectionReceipt, McpClientError>
  toggle(request: McpToggleRequest): Effect.Effect<McpToggleReceipt, McpClientError>
  capabilities(request: McpCapabilitiesRequest): Effect.Effect<McpCapabilitySet, McpClientError>
  callTool(request: McpToolCallRequest): Effect.Effect<McpToolCallResult, McpClientError>
  readResource(request: McpResourceReadRequest): Effect.Effect<McpResourceResult, McpClientError>
}
```

The service should publish connection events. Tool execution should still flow
through the agent's normal tool registry and permission system.

## Tool And Resource Projection

Projected MCP capabilities should include:

- Server id.
- Original server-provided name.
- Runtime-safe normalized name.
- Description with length caps.
- Input schema.
- Output classification.
- Permission requirements.
- Timeout policy.
- Auth state.
- Collapse or display hints.
- Public-safe summary rules.

Large, binary, or structured outputs should be persisted as local artifact refs
when needed. The model should receive a bounded summary, not unbounded remote
payloads.

## Authentication

Auth handling should support:

- OAuth discovery and login.
- Token refresh.
- Auth-needed connection state.
- Short-lived cache for repeated auth failures.
- Header helper execution under policy.
- Step-up auth signals from servers.
- Token revocation or reset.

Secrets must never appear in logs, help output, status bars, public receipts,
or server names.

## Trust And Channels

Some MCP servers may also act as channels for messages or remote approvals.
That must be a separate capability:

- Server declares channel capability.
- Server is in an allowlist or policy ref.
- User enables the channel for the current session.
- Permission-relay capability is separate from message capability.
- Replies are structured events, not free-form chat text.
- Pending approval ids are short-lived and single-use.

The terminal UI, remote channel, hooks, and automated classifiers may race to
answer a permission request, but only one resolver should claim it.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for client lifecycle and capability projection.
- `Schema` for configs, states, transports, tools, resources, prompts, and
  receipts.
- `Layer` for transport constructors, auth providers, config stores, and
  policy providers.
- `Stream` for server notifications and connection events.
- `Queue` for reconnect and capability-refresh work.
- `Ref` for active connection state and pending approval resolvers.
- `Schedule` for reconnection backoff and auth retry suppression.
- `Scope` for transport cleanup and child-process lifecycle.

Transport implementation should be swappable without changing the runtime tool
contract.

## Safety Rules

- Do not connect project-provided servers before approval.
- Do not log server headers, tokens, or raw auth errors containing secrets.
- Do not let MCP tools bypass the native approval system.
- Do not treat server-provided descriptions as trusted instructions.
- Do not register duplicate tools without a deterministic namespace.
- Do not let remote channel text become permission approval.
- Do not reconnect disabled servers.
- Do not leave child processes or network connections alive after cleanup.
- Do not send unbounded server output into model context.
- Do not let extension-provided servers override manual config silently.

## Tests

Minimum regression coverage:

- Validate every supported transport config.
- Merge user, project, local, managed, session, and extension scopes.
- Require approval for project-scoped servers.
- Connect, fail, disable, and reconnect servers.
- Convert auth failures into needs-auth state.
- Refresh tool, resource, and prompt lists after server notifications.
- Normalize names without collisions.
- Persist large or binary outputs as refs.
- Truncate descriptions and server instructions.
- Block unapproved channel servers.
- Resolve structured remote approval once and ignore duplicates.
- Cleanup transports on disable and shutdown.

## OpenAgents Translation Notes

When promoted, map MCP servers to OpenAgents capability refs, policy refs,
adapter refs, private artifact refs, approval refs, and public-safe receipts.
Verify live issue state before claiming MCP client, channel, or remote approval
behavior is implemented.

## Decision

The MCP client should be a scoped capability-ingestion layer. It should connect
servers, project capabilities, and handle auth while leaving trust, approval,
and transcript authority to the core OpenAgents runtime.
