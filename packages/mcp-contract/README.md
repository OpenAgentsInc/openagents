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
- package status metadata for docs and compatibility checks.

The package expands across the remaining Phase 0 issues to include authority,
transport, lifecycle, descriptor, receipt, error, progress, elicitation, naming,
and public-safe output rules.
