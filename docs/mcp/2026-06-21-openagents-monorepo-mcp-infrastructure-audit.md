# OpenAgents Monorepo MCP Infrastructure Audit

Date: 2026-06-21

Scope: the current `openagents` monorepo, with focus on preparing a real MCP
server surface for all Autopilot and Pylon functionality.

Status: audit plus 2026-06-22 Phase 0 implementation addendum. This document
does not change runtime authority, expose a new network listener, weaken an
invariant, or grant an MCP caller any new capability.

## Executive Summary

OpenAgents is MCP-literate but not yet MCP-exposed.

The repo already has the right ingredients for a serious MCP implementation:
typed Pylon control commands, a bridge capability model, loopback bearer
guarding, session event streams, refs-only public projections, Autopilot work
lanes that already model MCP server export and MCP capability catalogs, and
Pylon TAS primitives for tools, approvals, credentials, plugins, skills,
workspace boundaries, and receipts.

The missing piece is a canonical MCP gateway that turns those ingredients into
real `initialize`, `tools/list`, `tools/call`, `resources/list`,
`resources/read`, and optionally `prompts/list`/`prompts/get` behavior over a
supported transport. Today the MCP code in Pylon is a pure protocol core, not a
server process. The website surfaces MCP as projected status and audit data,
not as a live MCP transport. Desktop has MCP-adjacent seams for asset ingestion
and document production, but those are injected interfaces rather than live MCP
clients.

The safest direction is to expose Autopilot/Pylon through a narrow, generated,
policy-aware MCP layer built on existing authority boundaries:

- local Pylon MCP server for node-local control and observation;
- OpenAgents Worker MCP facade for public/product/cloud APIs;
- shared MCP contract package for tool definitions, resource URIs, schemas,
  authority classes, receipt requirements, and test fixtures;
- Desktop UI to start, inspect, pair, and revoke local MCP access;
- no direct secret, file, spend, deployment, or shell authority unless routed
  through existing approval and bridge policy.

## Repository MCP Inventory

### Pylon TAS MCP Core

Files:

- `apps/pylon/src/tas/mcp-server.ts`
- `apps/pylon/src/tas/mcp-client.ts`
- `apps/pylon/tests/tas-mcp-server.test.ts`
- `apps/pylon/tests/tas-mcp-client.test.ts`

Current behavior:

- `mcp-server.ts` implements a pure registry and dispatcher for tool contracts.
- `McpToolContract` contains `name`, `handlerKind`, and `readOnly`.
- `handleToolsList` returns registered tool names with handler metadata.
- `dispatchToolCall` returns either `unknown_tool` or a route descriptor.
- `mcp-client.ts` builds typed JSON-RPC-like envelopes for `initialize`,
  `tools/list`, and `tools/call`.
- Client helpers parse protocol errors, tool errors, and successful results.
- Tests cover deterministic request IDs, list/call envelope shape, duplicate
  registration rejection, unknown tool dispatch, and route descriptors.

What this is:

- A good protocol core.
- A low-risk seed for real MCP behavior.
- A place to preserve deterministic, unit-testable request/response logic.

What this is not:

- A production MCP server.
- A stdio, HTTP, WebSocket, or SSE transport.
- A full Model Context Protocol implementation.
- A handler runtime that can invoke Pylon control, Desktop commands, Worker
  APIs, resources, prompts, subscriptions, auth, or receipts.

Gap:

The core needs to grow from "route this named tool to a handler kind" into
"advertise a schema-bound, policy-bound capability and execute it through an
approved adapter."

### Pylon Tool, Approval, Plugin, Skill, and Boundary Primitives

Files:

- `apps/pylon/src/tas/tool-registry.ts`
- `apps/pylon/src/tas/approval-contract.ts`
- `apps/pylon/src/tas/credential-store.ts`
- `apps/pylon/src/tas/plugin-system.ts`
- `apps/pylon/src/tas/skill-system.ts`
- `apps/pylon/src/tas/hook-event.ts`
- `apps/pylon/src/tas/workspace-boundary.ts`
- `apps/pylon/src/tas/non-interactive.ts`
- `apps/pylon/src/tas/task-supervision.ts`
- `apps/pylon/src/tas/delivery-receipt.ts`
- `apps/pylon/src/tas/evidence-receipt.ts`
- `apps/pylon/src/tas/work-intake.ts`
- `apps/pylon/src/tas/semantic-retrieval.ts`

Current behavior:

- Tool contracts have names, input schemas, and read-only flags.
- Approval logic prevents read-only contracts from silently becoming effectful.
- Credential leases are ref-based and expire or revoke without exposing raw
  secrets.
- Plugin and skill registries model contributed commands, tools, hooks, skills,
  and conflict states.
- Workspace boundaries keep path access inside declared workspace roots.
- Non-interactive mode blocks prompt flows that would hang automation.
- Task supervision and receipts model durable outcomes without leaking private
  material.

MCP relevance:

These are the internal laws MCP must obey. The MCP server should not be a
parallel tool system; it should project these contracts into MCP tool and
resource definitions.

Gap:

There is no shared projection layer that converts TAS contracts into MCP tool
metadata with descriptions, JSON schemas, authority classes, receipt behavior,
and approval policy refs.

### Pylon Control Server and Bridge

Files:

- `apps/pylon/src/node/control-server.ts`
- `apps/pylon/src/node/control-cli.ts`
- `packages/autopilot-control-protocol/src/control.ts`
- `packages/autopilot-control-protocol/src/bridge.ts`

Current behavior:

- Pylon runs a loopback control HTTP server, defaulting to `127.0.0.1:4716`.
- `/health` returns the control schema tag and supported capabilities.
- `/command` accepts a local bearer token and typed `ControlCommand` payloads.
- `/events` and `/sessions/:ref/events` provide event streams.
- `/bridge/pair` exchanges one-time bootstrap credentials.
- `/bridge` accepts bridge credentials and checks verbs against granted
  capabilities.
- `control-cli.ts` is a local CLI client for the same control protocol.

Typed command families currently exposed through control:

- wallet receive, send, status, payout target admission, Spark send, Spark
  backup status;
- Apple Foundation Models status and session start;
- assignment polling and acceptance;
- session spawn, reply, list, events, cancel, and artifact read;
- intent submit and list;
- managed account list;
- approval list and resolve;
- coordinator pause, resume, and status;
- cloud deploy and deploy status;
- bridge issue, client list, and revoke.

Bridge verbs:

- `session.list`
- `session.subscribe`
- `session.snapshot`
- `session.history`
- `turn.steer`
- `turn.interrupt`
- `session.cancel`
- `session.pause`
- `session.resume`
- `decision.resolve`
- `artifact.read`
- `capability.list`
- `session.spawn`
- `intent.submit`
- `coordinator.pause`
- `coordinator.resume`
- `deploy.cloud`

Bridge capabilities:

- observe public sessions;
- observe private sessions;
- answer decisions;
- send instructions;
- cancel sessions;
- pause/resume sessions;
- read artifacts;
- spawn sessions;
- deploy cloud work.

MCP relevance:

This is the strongest substrate for a real Pylon MCP server. MCP tools should
wrap bridge/control actions instead of bypassing them. The bridge gives us
capability filtering, revocation, and a vocabulary that maps cleanly to tool
families.

Gap:

No MCP server currently adapts `ControlCommand` or bridge verbs into MCP
tools/resources. The control API is local and proprietary JSON; MCP clients
cannot discover or call it directly.

### Autopilot Desktop Control Layer

Files:

- `apps/autopilot-desktop/src/shared/rpc.ts`
- `apps/autopilot-desktop/src/bun/pylon-control.ts`
- `apps/autopilot-desktop/src/ui/commands.ts`
- `apps/autopilot-desktop/src/bun/asset-ingestion.ts`
- `apps/autopilot-desktop/src/bun/pdf-production.ts`
- `apps/autopilot-desktop/src/bun/verse-turn.ts`
- `apps/autopilot-desktop/src/bun/index.ts`

Current behavior:

- Desktop wraps Pylon loopback control with typed helper functions.
- The UI command registry defines a large set of Foldkit command operations
  for onboarding, training, evidence, accounts, sessions, shell/coding turns,
  Verse input, Apple FM, deployments, approvals, and coordinator control.
- The Bun host builds Verse turns and publishes live pylon-network snapshots
  into the home/Verse scene instead of showing fake counts.
- Asset ingestion is an injected `AssetSource`; comments explicitly allow a
  filesystem or MCP asset source, but the module itself performs no MCP calls.
- PDF production is an injected renderer seam; comments mention an MCP document
  tool as one possible renderer, but there is no live MCP integration.

MCP relevance:

Desktop is the likely operator surface for starting, pairing, and monitoring a
local MCP server. It also has command definitions that could become UI-facing
MCP capability descriptions.

Gap:

Desktop does not launch or supervise an MCP server, expose MCP pairing state,
show connected MCP clients, or let the operator approve/revoke MCP capability
grants. The asset and PDF seams are ready for MCP clients but are not wired.

### Verse World and Three Scene Projections

Files:

- `apps/autopilot-desktop/src/shared/chat-world-scene.ts`
- `apps/autopilot-desktop/src/shared/chat-world-visualization.ts`
- `apps/autopilot-desktop/src/ui/commands.ts`
- `apps/autopilot-desktop/src/bun/verse-turn.ts`
- `apps/openagents.com/workers/api/src/public-tassadar-run-summary-routes.ts`
- `apps/openagents.com/workers/api/src/openagents-openapi.ts`
- `docs/game/2026-06-21-spacetimedb-verse-multiplayer-audit.md`
- `docs/game/2026-06-21-verse-scene-graph-vs-react-three-fiber-audit.md`
- `docs/game/2026-06-21-autopilot-auto-forum-loop-and-verse-reflection-audit.md`
- sibling repository: `/Users/christopherdavid/work/three-effect`

Current behavior:

- The Desktop Verse is the current first-screen 3D environment for Autopilot.
- The scene projects pylons, balances, Tassadar run summaries, bulletin boards,
  forum/world activity, player state, multiplayer/presence primitives, payment
  particles, and selected-object overlays.
- `apps/autopilot-desktop/src/ui/commands.ts` includes Verse commands such as
  `PublishVerseLocalPose` and `RespondToVerseInput`.
- The Tassadar bulletin data is served by the Worker through a public-safe run
  summary route and included in OpenAPI.
- Reusable Three primitives are owned by the sibling `three-effect` repo, while
  Autopilot owns product semantics, pane state, and live data projection.

MCP relevance:

Verse should become an MCP resource surface, not merely a visual shell. Coding
agents need to query what the user is seeing, what world objects exist, which
pylon or bulletin is selected, and what actions are allowed from the current
world context.

The MCP server should expose high-level Verse intent and inspection tools. It
should not expose raw pointer-lock, camera-loop, animation-loop, or arbitrary
DOM/canvas mutation authority. Agents can ask to inspect a selected pylon or
submit an interaction intent; the renderer should remain owner of camera,
movement, frame lifecycle, and first-person controls.

Gap:

There is no MCP resource model for Verse world state, selected entities,
bulletins, pylon cards, player pose, forum activity icons, or Tassadar board
data. There is also no policy boundary that distinguishes safe world reads from
remote input injection.

### OpenAgents.com Worker API, OpenAPI, and Capability Manifest

Files:

- `apps/openagents.com/workers/api/src/openagents-openapi-routes.ts`
- `apps/openagents.com/workers/api/src/openagents-openapi.ts`
- `apps/openagents.com/workers/api/src/openagents-capability-manifest-routes.ts`
- `apps/openagents.com/workers/api/src/openagents-capability-manifest.ts`
- `apps/openagents.com/workers/api/src/*`

Current behavior:

- The Worker publishes `GET /api/openapi.json`.
- The Worker publishes `GET /.well-known/openagents.json`.
- Routes already carry auth modes such as public read, admin bearer, admin
  session, agent bearer, and browser/session variants.
- The manifest already includes resources, actions, auth modes, rate limits,
  caveats, and source links.

MCP relevance:

The OpenAPI document and capability manifest should be the source substrate for
cloud/product MCP tools. They are already explicit enough to generate a
read-only public MCP facade and later authenticated tools for Forum, Pylon,
Autopilot, product promises, training, proofs, and admin/operator workflows.

Gap:

The Worker does not expose an MCP transport. There is no mapping from OpenAPI
operation IDs and manifest actions to MCP tool names, input schemas, output
shaping, auth scopes, or receipt policy.

### Autopilot Work MCP Projection Lanes

Files:

- `apps/openagents.com/apps/web/src/page/loggedIn/autopilot-work/mcp-server-export.ts`
- `apps/openagents.com/apps/web/src/page/loggedIn/autopilot-work/mcp-capability-catalog.ts`
- `apps/openagents.com/apps/web/src/page/loggedIn/autopilot-work/extensibility-effective-config.ts`
- `apps/openagents.com/apps/web/src/page/loggedIn/autopilot-work/extensibility-execution-receipts.ts`
- `apps/openagents.com/apps/web/src/page/loggedIn/page/autopilot-work.ts`
- `apps/openagents.com/apps/web/src/page/loggedIn/model.ts`
- Related tests in the same directories.

Current behavior:

- `mcp-server-export.ts` projects public-safe MCP server export status from
  work projection data.
- Every authority flag in that projection is false by design, including tool
  execution, transport exposure, remote invocation, shell execution, file read,
  settings write, settlement, deployment, credential, and worker payout
  authority.
- `mcp-capability-catalog.ts` projects public-safe MCP catalog data, also with
  all execution, settlement, workspace-write, provider-account, and approval
  bypass authority disabled.
- The projection sanitizes refs and omits private or unsafe material.
- Tests cover rendering, stale or missing evidence, unsafe material omission,
  and status projection.

MCP relevance:

These lanes are valuable because they already model how MCP information should
be rendered safely in public/product UI. They are not execution surfaces, but
they define useful output safety expectations for any future MCP gateway.

Gap:

The projected "MCP server export" is a status lane, not a server. A future
gateway needs an authoritative registry and runtime execution path, while the
web projection should continue to show refs-only evidence of that runtime.

### Probe and Coding Runtime MCP Markers

Files:

- `apps/openagents.com/workers/api/src/probe-coding-runtime-contract.ts`
- `apps/openagents.com/workers/api/src/autopilot-coding-assignment.ts`
- `packages/probe/docs/*`

Current behavior:

- Probe and coding runtime contracts know about MCP as a tool/source category.
- Existing docs discuss MCP-style discovery and hosted execution tool lists.

MCP relevance:

The coding runtime already needs to reason about MCP tools as part of coding
agent execution. This should converge with the same shared MCP contract package
rather than invent a separate coding-only MCP abstraction.

Gap:

There is no single monorepo MCP registry that Probe, Pylon, Desktop, and
openagents.com all consume.

### Foldkit Devtools MCP Dependency

Files:

- root `bun.lock`
- `apps/openagents.com/package.json`
- `apps/openagents.com/bun.lock`

Current behavior:

- The repo includes references to `@foldkit/devtools-mcp`.
- This appears to be a development/devtools dependency path, not an Autopilot
  or Pylon product MCP server.

MCP relevance:

Useful for developer experience, but not a substitute for product MCP
exposure.

Gap:

Do not treat Foldkit devtools MCP as the canonical OpenAgents MCP gateway.

### Existing MCP Audits

Files:

- `docs/autopilot-coder/terminal-agent-systems/2026-06-11-mcp-server-system-audit.md`
- `docs/autopilot-coder/terminal-agent-systems/2026-06-11-mcp-client-system-audit.md`
- `docs/autopilot-coder/terminal-agent-systems/2026-06-16-forge-autopilot-coder-systems-roadmap.md`
- `docs/agents/2026-06-16-flue-framework-openagents-audit.md`
- `docs/agents/2026-06-17-cloudflare-agents-sdk-openagents-audit.md`

Current behavior:

- The terminal-agent audits correctly frame MCP server export as explicit,
  schema-bound, capability-bound, and approval-aware.
- The MCP client audit correctly treats external MCP servers as scoped,
  untrusted capability ingestion.
- The Forge roadmap already says MCP projection exists but does not execute
  MCP calls or host a server.
- Flue and Cloudflare Agents audits document relevant MCP client/server
  patterns without making them current product authority.

MCP relevance:

These docs should be treated as design constraints. They are not
implementation evidence.

Gap:

They should be connected to a concrete implementation ladder and a shared MCP
contract package.

## Authority Model Needed for MCP

MCP must not become a privileged side door around existing Autopilot/Pylon
authority. Every tool, resource, prompt, and notification should carry explicit
metadata:

- stable name;
- description;
- input schema;
- output schema or result envelope;
- `readOnly` flag;
- authority class;
- required bridge capability or product auth scope;
- approval behavior;
- receipt behavior;
- idempotency key policy;
- timeout and cancellation policy;
- projection level: public, operator, private local, private account, admin;
- unsafe output classifier;
- audit event kind.

Suggested authority classes:

- `public_read`: no private data and no state change.
- `operator_read`: local or authenticated operator data, no mutation.
- `private_account_read`: account/session data scoped to an authenticated user
  or paired client.
- `local_node_control`: affects a local Pylon node but does not spend money,
  deploy, write files, or execute shell.
- `coding_session_control`: spawns, steers, interrupts, or cancels coding
  sessions.
- `workspace_read`: reads local workspace or artifacts.
- `workspace_write`: writes local workspace, branches, or generated artifacts.
- `approval_resolution`: resolves an existing approval/decision.
- `deployment`: starts or changes deployment state.
- `wallet_read`: reads wallet balance or status.
- `wallet_spend`: sends sats, admits payout targets, or initiates a payment.
- `admin`: account, billing, treasury, provider, or global operator control.

Hard rules:

- Read-only MCP tools cannot mutate state.
- MCP callers cannot receive raw local tokens, mnemonics, session credentials,
  private prompt bodies, raw logs with secrets, or absolute private file paths.
- Wallet spend, deploy, admin, and workspace-write tools require explicit
  local policy and receipt behavior even if the MCP client is paired.
- Remote MCP callers should use bridge capabilities or product auth scopes,
  not the Pylon dev token.
- Public Worker MCP tools should be generated from public-safe OpenAPI and
  capability manifest metadata, not from implementation internals.

## Proposed Architecture

### 1. Shared MCP Contract Package

Implemented package: `packages/mcp-contract`
(`@openagentsinc/mcp-contract`).

Responsibilities:

- shared MCP JSON-RPC envelope types;
- tool, resource, and prompt definitions;
- authority class definitions;
- schema refs and JSON schemas;
- error envelope conventions;
- result size and artifact-ref conventions;
- receipt refs;
- compatibility tags;
- test fixtures.

Consumers:

- `apps/pylon`;
- `apps/autopilot-desktop`;
- `apps/openagents.com/workers/api`;
- `apps/openagents.com/apps/web`;
- future external clients and docs.

2026-06-22 Phase 0 status:

- Complete under epic
  [#5934](https://github.com/OpenAgentsInc/openagents/issues/5934).
- Child issues completed:
  [#5935](https://github.com/OpenAgentsInc/openagents/issues/5935),
  [#5936](https://github.com/OpenAgentsInc/openagents/issues/5936),
  [#5937](https://github.com/OpenAgentsInc/openagents/issues/5937),
  [#5938](https://github.com/OpenAgentsInc/openagents/issues/5938),
  [#5939](https://github.com/OpenAgentsInc/openagents/issues/5939),
  [#5940](https://github.com/OpenAgentsInc/openagents/issues/5940),
  [#5941](https://github.com/OpenAgentsInc/openagents/issues/5941), and
  [#5942](https://github.com/OpenAgentsInc/openagents/issues/5942).
- Surface import markers:
  `apps/pylon/src/mcp-contract-import.ts`,
  `apps/autopilot-desktop/src/mcp-contract-import.ts`,
  `apps/openagents.com/workers/api/src/mcp-contract-import.ts`, and
  `apps/openagents.com/apps/web/src/mcp-contract-import.ts`.
- No invariant change was needed. The package narrows and documents future MCP
  authority surfaces, but it does not expose a transport, grant a caller, start
  a listener, spend funds, mutate workspaces, or weaken existing runtime
  policy.

### 2. Local Pylon MCP Server

Add a real MCP server under `apps/pylon/src/mcp` or equivalent.

Transport targets:

- stdio first, because it is the common desktop coding-agent integration path;
- loopback HTTP/SSE second, because Desktop and local browser tooling can
  supervise and inspect it;
- remote HTTP only after bridge/auth policy is explicit and tested.

Execution strategy:

- read the local Pylon control endpoint from node config;
- use bridge credentials where possible;
- use the local control token only for operator-owned localhost flows;
- map MCP tool calls to existing control/bridge commands;
- stream long-running session events through MCP resources or notifications;
- return artifact refs for large outputs.

### 3. Worker MCP Facade

Add an MCP facade to the OpenAgents Worker only after the local Pylon server
contract is stable.

Possible routes:

- `/api/mcp` for authenticated product use;
- `/.well-known/openagents-mcp.json` for discovery metadata;
- a read-only public MCP surface for public docs, stats, forum summaries,
  proofs, OpenAPI, and capability manifest data.

Execution strategy:

- generate tools from OpenAPI operation metadata plus capability manifest
  policy;
- start read-only and public;
- add authenticated tools only when they share the same auth and receipt model
  as the normal HTTP endpoints;
- never let MCP bypass endpoint-level auth or rate limits.

### 4. Desktop Pairing and Supervision

Desktop should expose an operator UI for MCP:

- start/stop local Pylon MCP server;
- show transport, port, stdio command, and health;
- show connected clients;
- show capability grants;
- pair/revoke clients;
- inspect recent MCP calls and receipts;
- open docs/snippets for Codex, Claude Code, ChatGPT, and other clients.

Desktop should not become the only MCP runtime. It should supervise Pylon MCP
and use the same contract package.

### 5. Public Projection Continues to Be Refs-Only

The existing Autopilot work MCP server export and capability catalog lanes
should remain public-safe projections. When live MCP exists, those lanes should
display:

- server refs;
- transport refs;
- version refs;
- schema refs;
- policy refs;
- capability refs;
- status;
- blockers;
- last verified receipt refs.

They should still omit raw tokens, private config, local paths, and secret
material.

## Recommended MCP Surface

This is the proposed first comprehensive capability map. Tool names are
illustrative; final names should be generated from the shared contract package.

### Node Health and Status

- `pylon.health`
- `pylon.status`
- `pylon.capabilities.list`
- `pylon.coordinator.status`
- `pylon.accounts.list`

Readiness:

- first wave;
- read-only;
- local operator or bridge observe capability.

### Sessions and Coding Agents

- `pylon.sessions.list`
- `pylon.sessions.snapshot`
- `pylon.sessions.history`
- `pylon.sessions.events`
- `pylon.sessions.spawn`
- `pylon.sessions.reply`
- `pylon.sessions.cancel`
- `pylon.sessions.artifact.read`

Readiness:

- first wave for list/snapshot/history/artifact refs;
- second wave for spawn/reply/cancel;
- must map to bridge capabilities such as observe, send instruction, spawn,
  cancel, and read artifact.

### Approvals and Decisions

- `pylon.approvals.list`
- `pylon.approvals.resolve`

Readiness:

- list is early;
- resolve requires decision-specific policy, receipt refs, and operator
  visibility.

### Intents and Assignments

- `pylon.intents.submit`
- `pylon.intents.list`
- `pylon.assignments.poll`
- `pylon.assignments.accept`

Readiness:

- submit/list can be early if bounded;
- assignment acceptance should require local operator policy because it can
  move work into execution.

### Wallet and Pylon Payments

- `pylon.wallet.status`
- `pylon.wallet.receive`
- `pylon.wallet.backup_status`
- `pylon.wallet.send`
- `pylon.wallet.spark_send`
- `pylon.wallet.admit_payout_target`

Readiness:

- status and receive are early read/receive flows;
- all send/payout flows are high-risk and require explicit approval, receipts,
  amount limits, destination validation, and replay protection.

### Deployments

- `pylon.deploy.status`
- `pylon.deploy.cloud`

Readiness:

- status is early;
- deploy must be gated by bridge capability, owner policy, deployment receipt,
  and environment scoping.

### Apple FM and Local Models

- `pylon.apple_fm.status`
- `pylon.apple_fm.session.start`

Readiness:

- status is early;
- session start should follow coding-session control policy.

### Autopilot Product and Public Data

- `openagents.openapi.read`
- `openagents.capability_manifest.read`
- `openagents.public_activity.list`
- `openagents.training.runs.list`
- `openagents.training.run.read`
- `openagents.proofs.read`
- `openagents.product_promises.list`
- `openagents.pylon_stats.read`

Readiness:

- good Worker MCP first wave if all responses are public-safe and rate limited.

### Forum and Pylon Product Flows

- `openagents.forum.posts.list`
- `openagents.forum.post.read`
- `openagents.forum.post.tip`
- `openagents.pylons.list`
- `openagents.pylon.read`
- `openagents.pylon.tip`

Readiness:

- read operations can come before write/payment operations;
- tipping must share the existing Bitcoin/MDK payment policy, receipts, and UI
  approval expectations.

### Desktop Runtime Utilities

- `desktop.assets.list`
- `desktop.assets.ingest`
- `desktop.document.render_pdf`

Readiness:

- only after the injected runtime clients exist;
- all local file access must obey workspace boundaries and output redaction.

### Verse World Resources and Actions

- `verse.scene.status`
- `verse.world.entities.list`
- `verse.world.entity.read`
- `verse.local_player.pose.read`
- `verse.selection.read`
- `verse.selection.interact`
- `verse.bulletins.list`
- `verse.bulletins.read`
- `verse.pylons.visible.list`
- `verse.pylon.read`
- `verse.pylon.tip`
- `verse.forum_activity.visible.list`

Readiness:

- read-only resources should ship before any remote interaction tools;
- interaction tools should submit high-level intents through existing Desktop
  commands, not mutate Three.js objects directly;
- tipping a pylon from Verse must share the same payment approval and receipt
  policy as other Pylon payment tools;
- camera movement, pointer lock, zoom, animation state, and frame lifecycle
  should stay outside MCP control unless an explicit future accessibility or
  automation mode is designed.

## Main Gaps

1. No production MCP server transport exists for Autopilot/Pylon.
2. Pylon MCP core lacks full tool schemas, descriptions, resources, prompts,
   notifications, and server lifecycle.
3. No adapter maps MCP calls to Pylon control or bridge commands.
4. The shared MCP contract package now spans Pylon, Desktop, Worker, and web
   imports, but runtime MCP surfaces do not yet consume it for live
   descriptors.
5. The canonical MCP authority taxonomy now exists in code, but live
   server-side grant filtering is not implemented yet.
6. No Worker MCP facade maps OpenAPI/capability manifest actions to tools.
7. No Desktop UI exists for MCP server launch, pairing, grants, revocation, or
   call inspection.
8. MCP output safety/redaction rules now exist in the contract, but no live
   gateway classifier enforces them for runtime MCP responses yet.
9. No wallet/deploy/admin MCP policy exists beyond the underlying control
   command safety model.
10. No end-to-end MCP smoke proves that Codex, Claude Code, or another MCP
    client can connect to Pylon and operate within policy.

## Implementation Sequence

### Phase 0: Contract Groundwork

Complete as of 2026-06-22.

1. Add `packages/mcp-contract`.
2. Define authority classes, shared error envelopes, result envelopes, schema
   refs, and tool/resource descriptors.
3. Define transport config, lifecycle, receipt, progress, elicitation, naming,
   and public-safe output contracts.
4. Wire contract imports into Pylon, Autopilot Desktop, Worker/API, and web
   without exposing a runtime MCP transport.

### Phase 1: Read-Only Local Pylon Server

1. Extend the Pylon TAS MCP core to export schema-rich MCP tools using
   `@openagentsinc/mcp-contract`.
2. Implement stdio MCP server transport for Pylon.
3. Expose read-only local tools:
   - `pylon.health`
   - `pylon.capabilities.list`
   - `pylon.coordinator.status`
   - `pylon.accounts.list`
   - `pylon.sessions.list`
   - `pylon.sessions.snapshot`
   - `pylon.wallet.status`
4. Add unit tests for tool lists, schemas, unknown tools, read-only enforcement,
   bridge capability filtering, and redaction.
5. Add a local smoke that starts the server and calls it through a real MCP
   client library or protocol fixture.

### Phase 2: Session Control and Receipts

1. Add session resources and artifact resources.
2. Add `pylon.sessions.spawn`, `pylon.sessions.reply`, and
   `pylon.sessions.cancel`.
3. Add approval list and resolve tools.
4. Add intent submit/list tools.
5. Ensure all control tools create or reference existing receipts.
6. Add cancellation and timeout behavior.
7. Add tests that prove MCP cannot bypass approvals, non-interactive mode, or
   workspace boundaries.

### Phase 3: Desktop Supervision

1. Add Desktop MCP status RPC.
2. Add Desktop UI panels for MCP server state, connected clients, pairing,
   grants, revocation, and call log.
3. Add copyable client configuration snippets.
4. Add Desktop integration tests with fake Pylon control and a fake MCP client.
5. Wire asset ingestion and PDF production to real optional MCP clients only
   after the supervision model is visible.

### Phase 4: Worker Public MCP Facade

1. Add OpenAPI/capability-manifest-to-MCP generation.
2. Expose read-only public tools from the Worker.
3. Add output shaping and rate-limit behavior.
4. Add tests that compare MCP tool schemas with OpenAPI schemas.
5. Update web MCP projection lanes to display live Worker MCP readiness refs.

### Phase 5: Verse World Resources and Actions

1. Add read-only Verse resources for scene status, selected object, visible
   pylons, bulletins, player pose, and public world activity.
2. Keep those resources behind the same retained scene state used by Desktop so
   MCP reads do not remount or reset the Three scene.
3. Add high-level Verse interaction tools that submit intent through Desktop
   commands instead of raw DOM events.
4. Add tests proving MCP reads and interactions do not steal pointer lock,
   freeze animation, reset player pose, or mutate renderer-owned state.
5. Wire pylon tipping from Verse only after payment approval and receipt policy
   is shared with the broader Pylon payment tools.

### Phase 6: Authenticated Product Tools

1. Add browser/session and agent-bearer authenticated MCP flows.
2. Add Forum read/write tools.
3. Add Pylon read/tip tools.
4. Add product promise, proof, training, and public activity tools.
5. Add payment receipts and replay protections for tipping.
6. Add integration tests against local Worker fixtures.

### Phase 7: High-Risk Tools

1. Add wallet receive tools.
2. Add wallet send/Spark send only with explicit policy, amount caps, operator
   confirmation, and receipt proofs.
3. Add deploy.cloud only with environment-bound policy and receipt proofs.
4. Add admin tools last, if at all, behind owner-only policy and separate
   audit trails.

## Test Plan

Minimum unit coverage:

- schema projection for every tool;
- `readOnly` never calls mutating adapters;
- unknown tool errors are stable;
- invalid arguments are rejected before execution;
- bridge capability filters hide ungranted tools;
- unsafe refs and private material are omitted;
- high-risk tools require approval and receipt policy;
- local workspace paths cannot escape declared roots.

Minimum integration coverage:

- start Pylon MCP server over stdio;
- `initialize` succeeds;
- `tools/list` returns only granted tools;
- `tools/call pylon.health` returns live local node health;
- `tools/call pylon.sessions.list` returns a shaped session list;
- fake ungranted client cannot see or call control tools;
- session spawn/reply/cancel follows bridge policy in a fake node;
- wallet status can be read without exposing secrets;
- wallet send is blocked without approval;
- Desktop shows MCP server status and a call log entry;
- Worker read-only MCP facade returns public stats without private material.
- Verse world resources return visible entity and bulletin state without
  remounting or resetting the Three scene.
- Verse interaction tools submit intents without stealing pointer lock,
  freezing animation, or injecting raw DOM events.

Manual smoke before claiming readiness:

- connect Codex to the local Pylon MCP server;
- connect Claude Code to the same server with a different grant;
- prove grants differ in `tools/list`;
- list sessions;
- start a bounded coding session;
- query the visible Verse scene state and selected pylon/bulletin resource;
- inspect events/artifacts;
- revoke one client and prove future calls fail;
- verify the Desktop UI shows the same client and call history.

## Documentation Updates Needed With Implementation

When implementation begins, update:

- `docs/mcp/README.md` with the current server status and run commands;
- `docs/pylon/*` with local MCP server setup and safety model;
- `docs/autopilot-coder/*` with coding-agent MCP connection guidance;
- `docs/launch/*` when MCP support affects release checklists;
- OpenAPI/capability manifest docs when Worker MCP tools become available;
- Autopilot Desktop docs when pairing and revocation UI lands.

## Suggested Issue Ladder

These are the issues that should be opened and completed sequentially when the
implementation work starts:

1. Add shared OpenAgents MCP contract package. Complete in Phase 0 epic
   [#5934](https://github.com/OpenAgentsInc/openagents/issues/5934).
2. Expand Pylon TAS MCP registry to schema-rich MCP descriptors.
3. Implement Pylon stdio MCP server with read-only node status/session tools.
4. Add bridge capability filtering and MCP pairing/revocation.
5. Add session control MCP tools with receipts.
6. Add approval and intent MCP tools.
7. Add Desktop MCP server supervision and client configuration UI.
8. Add Worker read-only MCP facade from OpenAPI/capability manifest metadata.
9. Add Verse world-state MCP resources for selected objects, visible pylons,
   bulletins, and public world activity.
10. Add Verse high-level interaction MCP tools without raw camera/DOM control.
11. Add authenticated Forum and Pylon product MCP tools.
12. Add Bitcoin tipping/payment MCP tools with explicit approval and receipts.
13. Add deployment MCP tools with environment-scoped policy.
14. Add admin MCP tools only after owner-only policy and audit trails are
    proven.
15. Add cross-client MCP smoke tests for Codex and Claude Code.

## Final Assessment

The monorepo is in a good position to expose Autopilot and Pylon through MCP,
but the implementation should be treated as an authority surface, not a thin
protocol wrapper. The safest path is to make MCP a projection of existing
control, bridge, approval, receipt, OpenAPI, and capability-manifest contracts.

Do not build a separate MCP universe. Build one MCP gateway that speaks the
same laws as Pylon and Autopilot already enforce.
