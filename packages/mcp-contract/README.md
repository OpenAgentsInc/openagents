# OpenAgents MCP Contract

`@openagentsinc/mcp-contract` is the shared, transport-neutral contract for
OpenAgents MCP server and client work.

Phase 0 intentionally defines types and validation helpers only. It does not
start a stdio server, loopback listener, remote bridge, or external MCP client.
Later Pylon, Autopilot Desktop, Worker, web, Verse, payment, and coding-agent
surfaces should import these contracts instead of defining local protocol
shapes.

Current Phase 0 scope:

- shared contract package metadata;
- schema decode helpers;
- authority taxonomy and grant filtering;
- transport config and lifecycle schemas;
- tool, resource, and prompt descriptors;
- naming and resource URI rules;
- package status metadata for docs and compatibility checks.

## Authority And Grants

The contract models MCP authority as explicit classes such as `public_read`,
`operator_read`, `private_account_read`, `workspace_read`,
`coding_session_control`, `approval_resolution`, `payment_read`,
`payment_receive`, and the high-risk classes `workspace_write`,
`payment_spend`, `deployment`, and `admin`.

Servers and clients should filter descriptors by granted authority before
returning list results. Ungranted capabilities must be absent from
`tools/list`, `resources/list`, and prompt projections rather than visible as
disabled entries. High-risk classes are absent by default and require explicit
grants before they can appear.

## Transport And Lifecycle

The contract models planned client/server transport kinds without starting
them: `stdio`, `loopback_http`, `streamable_http`, `sse`, `websocket`,
`ide_local`, `in_process`, and `bridge_proxy`.

MCP server config records also carry source metadata for local private config,
shared project config, user config, managed config, dynamic discovery, plugins,
IDE-provided config, and Desktop-discovered servers. Lifecycle status is
explicitly tagged, including `pending_approval`, `needs_auth`, `rejected`,
`revoked`, and `blocked_by_policy`, so clients can explain state without
retrying noisy failures.

Credential values are not part of transport config. Private config may carry
local credential refs, but public/debug projections omit those refs and expose
only the transport kind plus non-secret metadata.

## Descriptors And Names

Tool, resource, and prompt descriptors carry authority requirements, risk class,
schema refs, receipt/progress behavior, staleness metadata, and output handling
policy. Descriptor metadata is sufficient for grant filtering without importing
runtime app code.

OpenAgents MCP tool and prompt names use lowercase dotted identifiers such as
`pylon.health`, `verse.scene.state`, and `coding.session.spawn`. Resource URIs
use the `mcp://openagents/<namespace>/<path>` form. Phase 0 namespaces are
`pylon`, `autopilot`, `verse`, `worker`, `forum`, `payments`, and
`coding-session`.

The package expands across the remaining Phase 0 issues to include authority,
transport, lifecycle, descriptor, receipt, error, progress, elicitation, naming,
and public-safe output rules.
