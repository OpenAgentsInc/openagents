# MCP Server System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #29 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should expose selected local agent capabilities as
an MCP server for trusted callers.

## Target

Build an MCP server system that exports explicit local capabilities without
turning the whole agent runtime into a remote-control endpoint.

Server exposure should be adapter-specific, schema-governed, session-scoped,
and shut down with the parent runtime. It should be possible to expose browser
automation, desktop automation, repository context, task status, or agent
session controls without sharing unrelated authority.

## User-Visible Capability

The user or operator should be able to:

- Start a local MCP server for a bounded capability set.
- See which tools, resources, and prompts are exposed.
- Choose stdio, in-process, local socket, or network transport where policy
  allows.
- Pair or authenticate trusted remote callers.
- Deny or revoke exposed capabilities.
- See server lifecycle, caller, and transport state.
- Route tool calls through normal permission policy.
- Shut down the server when the parent session exits.

The server should never imply that all local agent capabilities are remotely
callable.

## Capability Projection

Each exposed capability should declare:

- Stable server-side name.
- Description.
- Input schema.
- Output schema or output class.
- Required policy refs.
- Required user approval behavior.
- Session affinity.
- Workspace scope.
- Privacy class.
- Timeout and cancellation policy.
- Audit receipt shape.

Projection is not dispatch. Listing a tool should be cheap and safe; executing
it should still pass through policy, session, and approval checks.

## Server Kinds

The system should support multiple server kinds:

- In-process adapter for internal callers.
- Child-process stdio server for external MCP clients.
- Local socket server for same-machine integrations.
- WebSocket or HTTP server for remote integrations when explicitly enabled.

Each kind should share the same capability model and differ only in transport,
auth, and lifecycle management.

## Core Design

Define an `McpServerService` that owns server startup, capability projection,
caller auth, dispatch, and shutdown.

Suggested service boundary:

```ts
interface McpServerService {
  start(request: McpServerStartRequest): Effect.Effect<McpServerReceipt, McpServerError>
  list(request: McpServerListRequest): Effect.Effect<McpServerSnapshot, McpServerError>
  expose(request: McpExposeRequest): Effect.Effect<McpExposeReceipt, McpServerError>
  revoke(request: McpRevokeRequest): Effect.Effect<McpRevokeReceipt, McpServerError>
  dispatch(request: McpServerCallRequest): Effect.Effect<McpServerCallResult, McpServerError>
  stop(request: McpServerStopRequest): Effect.Effect<McpServerStopReceipt, McpServerError>
}
```

Adapters should register capability definitions. The service should enforce
common auth, policy, session isolation, cancellation, logging, and receipts.

## Dispatch Flow

Tool dispatch should follow:

1. Identify caller and server session.
2. Validate tool name and input schema.
3. Check capability exposure.
4. Check workspace and session scope.
5. Check auth and caller trust.
6. Run permission policy.
7. Execute adapter implementation.
8. Classify and bound output.
9. Return result and emit receipt.

The adapter should receive only the data needed for the requested capability.

## Session Isolation

Server instances should isolate:

- Session id.
- Workspace ref.
- Caller identity.
- Exposed capability set.
- Approval state.
- Cancellation state.
- Output artifacts.
- Diagnostic logs.

Remote callers should not inherit interactive terminal focus, prompt text,
clipboard state, or unexpired local approvals unless that sharing is explicitly
modeled.

## Lifecycle

Server lifecycle should include:

- Startup receipt.
- Capability registration.
- Transport ready state.
- Pairing or auth state.
- Parent-runtime heartbeat.
- Graceful shutdown on stdin close, parent exit, or session end.
- Analytics or diagnostic flush where allowed.
- Forced cleanup for hung transports.

Server shutdown should be idempotent.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for server lifecycle and dispatch.
- `Schema` for exposed capability definitions, caller identity, requests,
  responses, and receipts.
- `Layer` for transport, auth, adapter, and policy implementations.
- `Stream` for inbound protocol messages and server lifecycle events.
- `Queue` for dispatch work and cancellation.
- `Ref` for exposed capability registry and active caller sessions.
- `Scope` for transport handles, child process cleanup, and shutdown hooks.

Keep protocol framing and capability dispatch separate so the same adapter can
be exposed through different transports.

## Safety Rules

- Do not expose all native tools by default.
- Do not accept remote callers without an auth or pairing policy.
- Do not let server-side tool calls bypass permission refs.
- Do not leak prompts, file contents, paths, or secrets through capability
  descriptions.
- Do not let a child-process server outlive its parent session.
- Do not trust caller-provided session ids without verification.
- Do not expose desktop or browser control without explicit capability grants.
- Do not run unbounded calls without timeout and cancellation.
- Do not return raw binary or huge outputs inline.

## Tests

Minimum regression coverage:

- Start and stop every supported server kind.
- List exposed tools, resources, and prompts.
- Reject unexposed tools.
- Validate input schemas before dispatch.
- Enforce caller auth and pairing.
- Enforce workspace and session isolation.
- Route dispatched calls through approval policy.
- Cancel in-flight calls on shutdown.
- Exit child-process servers when parent input closes.
- Bound output and produce artifact refs for large results.
- Revoke capabilities during a session.
- Keep public receipts free of private payloads.

## OpenAgents Translation Notes

When promoted, map exposed server capabilities to OpenAgents adapter refs,
capability refs, session refs, workspace refs, policy refs, and audit receipts.
Verify live issue state before claiming MCP server or remote caller behavior is
implemented.

## Decision

The MCP server should be an explicit capability-export layer. It should expose
small, schema-bound adapters to trusted callers while preserving session
isolation, approval policy, and local cleanup authority.
