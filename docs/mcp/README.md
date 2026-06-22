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

**The CRM MCP server (epic #5991) is built** — the first OpenAgents function
served over MCP.

- **Endpoint:** `POST /api/mcp` (stateless Streamable-HTTP JSON-RPC:
  `initialize`, `tools/list`, `tools/call`, `resources/list`, `resources/read`,
  `ping`).
- **Discovery:** `GET /.well-known/openagents-mcp.json` (public, refs-only:
  server + transport + the public-safe tool/resource catalog).
- **Auth:** admin Bearer token (full CRM authority on the `X-OpenAgents-Tenant`
  / default tenant) OR a scoped MCP grant token (declared authority classes +
  bound tenant). Mint/list/revoke at `POST/GET/DELETE /api/operator/crm/mcp-grants`.
- **Tools (grant-filtered, ungranted = absent):** read tools (`operator_read`)
  over contacts/accounts/lists/activities/engagement/opportunities/imports/
  templates/commands/queue + `crm.contact.render`; `crm.send.command.propose`
  (proposes only — sends nothing); `crm.template.upsert` (workspace_write);
  `crm.send.command.approve`/`.reject` (approval_resolution); `crm.import.run`
  (workspace_write); `crm.batch.send` (dry-run only over MCP).
- **Safety:** results output-safety-projected (operator class); tenant bound to
  the credential (client `args.tenant` ignored); agent sends are
  propose → human-approve. Built on `@openagentsinc/mcp-contract`.

The read-only **local Pylon stdio server** is the next MCP server after this
epic, reusing the transport + grant patterns proven here. See
`2026-06-22-crm-mcp-server-phase-1-audit.md`.

## Documents

- `2026-06-22-crm-mcp-server-phase-1-audit.md` - **next-phase audit**: expose
  the CRM as the first OpenAgents MCP server (Worker Streamable-HTTP facade),
  with the CRM tool/resource map, architecture, gaps, and issue ladder.
- `2026-06-21-openagents-monorepo-mcp-infrastructure-audit.md` - current
  monorepo audit and implementation sequence for Autopilot/Pylon MCP exposure.
- `2026-06-21-openagents-overarching-mcp-roadmap.md` - end-to-end MCP server
  and client roadmap for making Pylon and Autopilot fully steerable through
  policy-bound MCP surfaces.
- `../../packages/mcp-contract/README.md` - package-level contract reference
  for Phase 0 schemas, naming, authority classes, output safety, imports, and
  non-goals.
