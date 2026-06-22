# MCP Audits

This directory tracks Model Context Protocol infrastructure audits, roadmaps,
and implementation planning for exposing OpenAgents, Autopilot, and Pylon
capabilities through MCP servers and clients.

## Current Status

Phase 0 contract groundwork is implemented in
`packages/mcp-contract` as `@openagentsinc/mcp-contract`.

Phase 0 shipped under epic
[#5934](https://github.com/OpenAgentsInc/openagents/issues/5934):

- [#5935](https://github.com/OpenAgentsInc/openagents/issues/5935): shared
  contract package.
- [#5936](https://github.com/OpenAgentsInc/openagents/issues/5936): authority
  taxonomy and grants.
- [#5937](https://github.com/OpenAgentsInc/openagents/issues/5937): transport
  config and lifecycle schemas.
- [#5938](https://github.com/OpenAgentsInc/openagents/issues/5938):
  tool/resource/prompt descriptor schemas and naming rules.
- [#5939](https://github.com/OpenAgentsInc/openagents/issues/5939):
  receipts, tagged errors, progress, and elicitation schemas.
- [#5940](https://github.com/OpenAgentsInc/openagents/issues/5940):
  public-safe output and redaction rules.
- [#5941](https://github.com/OpenAgentsInc/openagents/issues/5941): imports
  through Pylon, Autopilot Desktop, Worker/API, and web.
- [#5942](https://github.com/OpenAgentsInc/openagents/issues/5942):
  contract usage docs and Phase 1 handoff.

Runtime MCP transports remain out of scope until Phase 1. The current package
defines typed contracts, validation helpers, import markers, and docs only; it
does not start a stdio server, loopback listener, remote bridge, or external
MCP client connector.

The next implementation epic should start with a read-only local Pylon MCP
server over stdio, using `@openagentsinc/mcp-contract` for descriptors,
authority filtering, output safety, tagged errors, receipts, and progress
metadata.

## Documents

- `2026-06-21-openagents-monorepo-mcp-infrastructure-audit.md` - current
  monorepo audit and implementation sequence for Autopilot/Pylon MCP exposure.
- `2026-06-21-openagents-overarching-mcp-roadmap.md` - end-to-end MCP server
  and client roadmap for making Pylon and Autopilot fully steerable through
  policy-bound MCP surfaces.
- `../../packages/mcp-contract/README.md` - package-level contract reference
  for Phase 0 schemas, naming, authority classes, output safety, imports, and
  non-goals.
