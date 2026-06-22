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

The package expands across the remaining Phase 0 issues to include authority,
transport, lifecycle, descriptor, receipt, error, progress, elicitation, naming,
and public-safe output rules.
