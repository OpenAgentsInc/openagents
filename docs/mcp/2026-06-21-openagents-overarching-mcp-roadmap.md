# OpenAgents MCP Roadmap

Date: 2026-06-21

Scope: an overarching roadmap for making OpenAgents, Autopilot, and Pylon
first-class MCP servers and first-class MCP clients.

Status: roadmap only. This document does not expose any transport, grant any
new authority, or change runtime behavior.

## Goal

OpenAgents should treat MCP as a real control plane, not as a narrow tool
adapter.

The end state is:

- Pylon is fully steerable through an MCP server.
- Autopilot Desktop is fully inspectable and steerable through an MCP server.
- OpenAgents.com exposes public and authenticated product capability through a
  Worker MCP facade.
- Autopilot and Pylon can safely consume external MCP servers as clients.
- MCP tools, resources, prompts, progress events, elicitations, approvals,
  receipts, and revocation all follow the same OpenAgents policy model.

"Fully steerable" does not mean arbitrary remote control. It means every
meaningful Autopilot/Pylon operation has a typed MCP tool, resource, prompt, or
subscription with explicit authority, capability refs, policy refs, approval
behavior, timeout behavior, output shaping, and receipt semantics.

## Non-Negotiable Principles

1. MCP cannot bypass existing authority.
2. MCP cannot become a second hidden tool system.
3. Server and client behavior must share one contract package.
4. Read-only metadata and effectful control must be separate.
5. Wallet spend, deployment, workspace write, admin, and approval resolution
   require explicit policy and receipts.
6. Public MCP output must stay public-safe and refs-only where appropriate.
7. Local Desktop/Verse control must not remount scenes, steal input, reset
   player state, or mutate renderer-owned internals.
8. External MCP servers are untrusted until scoped, approved, and policy
   filtered.

## Two-Sided MCP Strategy

OpenAgents needs both MCP directions.

### OpenAgents As MCP Server

External clients should be able to connect to OpenAgents/Pylon/Autopilot and
steer the system through declared capability.

Server surfaces:

- local Pylon MCP server;
- Autopilot Desktop MCP supervision and operator UI;
- OpenAgents Worker MCP facade;
- optional in-process MCP server bridge for embedded SDK/runtime sessions.

### OpenAgents As MCP Client

Autopilot and Pylon should be able to connect to outside MCP servers and expose
approved external tools/resources/prompts to coding agents and operator flows.

Client surfaces:

- local external MCP tools for coding sessions;
- project-scoped MCP config for trusted repositories;
- user-scoped MCP config for personal tools;
- managed/enterprise MCP config for organization-controlled tools;
- dynamic MCP config for SDK/session-specific tools;
- plugin-provided MCP tools;
- IDE/desktop-local MCP connections where explicitly trusted.

Both directions must use the same policy language and telemetry model.

## Shared MCP Contract Package

Create a shared package, tentatively `packages/openagents-mcp-contract`.

It should own:

- MCP JSON-RPC envelope helpers;
- tool descriptors;
- resource descriptors;
- prompt descriptors;
- notification descriptors;
- authority classes;
- capability refs;
- policy refs;
- receipt refs;
- result envelope schema;
- error envelope schema;
- progress event schema;
- elicitation schema;
- client/server status schema;
- transport config schema;
- connection lifecycle status schema;
- test fixtures.

Every MCP surface should import from this package instead of inventing local
string names or private schema shapes.

## Authority Classes

Every MCP tool, resource, prompt, and subscription should declare an authority
class.

Initial classes:

- `public_read`: public-safe data only.
- `operator_read`: local or authenticated operator data, no mutation.
- `private_account_read`: user/account scoped data, no mutation.
- `workspace_read`: reads local workspace or artifact material.
- `workspace_write`: writes workspace files, branches, generated artifacts, or
  local repo state.
- `local_node_control`: changes Pylon node runtime state without spending,
  deploying, or writing files.
- `coding_session_control`: spawns, steers, interrupts, cancels, or resumes
  coding sessions.
- `approval_resolution`: resolves a pending approval or decision.
- `payment_read`: reads balances, offers, invoices, payment status, or payout
  readiness.
- `payment_receive`: creates receive offers or invoices without spending.
- `payment_spend`: sends sats, tips posts, tips pylons, admits payout targets,
  or dispatches settlement.
- `deployment`: starts, updates, or cancels deployment flows.
- `admin`: global, operator, account, provider, billing, treasury, or policy
  authority.

Required descriptor fields:

- stable name;
- title;
- description;
- input schema;
- output schema or resource schema;
- read-only flag;
- destructive flag;
- open-world flag;
- authority class;
- capability refs;
- policy refs;
- auth scopes;
- approval behavior;
- receipt behavior;
- timeout;
- idempotency key rules;
- output classification;
- safe logging policy.

## Server Roadmap

### Server Surface A: Local Pylon MCP Server

Purpose:

Expose the local Pylon node as a safe, capability-filtered MCP server.

Transports:

- stdio first, for local coding agents;
- loopback Streamable HTTP second, for Desktop and local automation;
- loopback SSE or event resources for long-running subscriptions;
- remote access only through an explicit bridge/pairing model.

Required tool groups:

- `pylon.health`
- `pylon.capabilities.list`
- `pylon.node.status`
- `pylon.accounts.list`
- `pylon.coordinator.status`
- `pylon.coordinator.pause`
- `pylon.coordinator.resume`
- `pylon.assignments.poll`
- `pylon.assignments.accept`
- `pylon.intents.submit`
- `pylon.intents.list`
- `pylon.sessions.list`
- `pylon.sessions.snapshot`
- `pylon.sessions.history`
- `pylon.sessions.events`
- `pylon.sessions.spawn`
- `pylon.sessions.reply`
- `pylon.sessions.interrupt`
- `pylon.sessions.cancel`
- `pylon.sessions.artifact.read`
- `pylon.approvals.list`
- `pylon.approvals.resolve`
- `pylon.wallet.status`
- `pylon.wallet.receive`
- `pylon.wallet.backup_status`
- `pylon.wallet.send`
- `pylon.wallet.spark_send`
- `pylon.wallet.admit_payout_target`
- `pylon.deploy.status`
- `pylon.deploy.cloud`
- `pylon.apple_fm.status`
- `pylon.apple_fm.session.start`

Required resources:

- `pylon://node/status`
- `pylon://capabilities`
- `pylon://accounts`
- `pylon://coordinator/status`
- `pylon://assignments`
- `pylon://intents`
- `pylon://sessions`
- `pylon://sessions/{sessionRef}`
- `pylon://sessions/{sessionRef}/history`
- `pylon://sessions/{sessionRef}/events`
- `pylon://artifacts/{artifactRef}`
- `pylon://approvals`
- `pylon://wallet/status`
- `pylon://deployments/{deploymentRef}`

Required prompts:

- `pylon.triage_session`
- `pylon.summarize_session`
- `pylon.prepare_approval`
- `pylon.explain_wallet_status`
- `pylon.plan_deployment`
- `pylon.debug_failed_assignment`

Hard requirements:

- MCP server wraps Pylon control/bridge commands; it does not bypass them.
- Connected clients receive only tools allowed by their grant.
- Bridge revocation immediately removes authority.
- Tool calls produce receipt refs for meaningful mutations.
- Long-running session tools emit progress.
- Session streams can be consumed without polling the entire session state.

### Server Surface B: Autopilot Desktop MCP Server

Purpose:

Make the Desktop app inspectable and steerable without exposing raw GUI or DOM
control.

Required tool groups:

- `autopilot.desktop.status`
- `autopilot.desktop.diagnostics.read`
- `autopilot.desktop.mcp.clients.list`
- `autopilot.desktop.mcp.client.revoke`
- `autopilot.desktop.mcp.server.start`
- `autopilot.desktop.mcp.server.stop`
- `autopilot.desktop.preferences.read`
- `autopilot.desktop.preferences.update`
- `autopilot.desktop.accounts.list`
- `autopilot.desktop.accounts.select`
- `autopilot.desktop.coding_mode.enter`
- `autopilot.desktop.coding_mode.exit`
- `autopilot.desktop.coding_panes.list`
- `autopilot.desktop.coding_panes.open`
- `autopilot.desktop.coding_panes.close`
- `autopilot.desktop.coding_panes.focus`
- `autopilot.desktop.coding_composer.submit`
- `autopilot.desktop.shell.turn.spawn`
- `autopilot.desktop.shell.input.respond`

Required resources:

- `autopilot://desktop/status`
- `autopilot://desktop/diagnostics`
- `autopilot://desktop/mcp/clients`
- `autopilot://desktop/accounts`
- `autopilot://desktop/preferences`
- `autopilot://desktop/coding-mode`
- `autopilot://desktop/panes`
- `autopilot://desktop/transcript/{sessionRef}`

Hard requirements:

- Desktop MCP server never exposes raw local secrets.
- Desktop MCP server never exposes arbitrary click/type/screen authority by
  default.
- Desktop server should supervise Pylon MCP, not fork an incompatible control
  plane.
- Operator UI must show connected MCP clients, active grants, recent calls,
  and revocation controls.

### Server Surface C: Verse World MCP Resources

Purpose:

Make the Verse world inspectable and steerable at the semantic level.

Read-only tools/resources:

- `verse.scene.status`
- `verse.world.entities.list`
- `verse.world.entity.read`
- `verse.local_player.pose.read`
- `verse.camera.state.read`
- `verse.selection.read`
- `verse.bulletins.list`
- `verse.bulletins.read`
- `verse.pylons.visible.list`
- `verse.pylon.read`
- `verse.forum_activity.visible.list`
- `verse.tassadar.board.read`

Effectful tools:

- `verse.selection.interact`
- `verse.local_player.intent.submit`
- `verse.pylon.tip`
- `verse.forum_post.open`
- `verse.bulletin.dismiss`
- `verse.coding_mode.enter`

Required resources:

- `verse://scene/status`
- `verse://world/entities`
- `verse://world/entities/{entityRef}`
- `verse://selection`
- `verse://player/local/pose`
- `verse://camera`
- `verse://bulletins`
- `verse://bulletins/{bulletinRef}`
- `verse://pylons/visible`
- `verse://pylons/{pylonRef}`
- `verse://tassadar/board`

Hard requirements:

- MCP reads cannot remount the Three scene.
- MCP reads cannot reset player pose.
- MCP interactions cannot inject raw DOM events.
- MCP interactions cannot steal pointer lock.
- Camera, animation loop, renderer lifecycle, and frame resources stay owned by
  the Verse runtime.
- Verse MCP tools submit high-level intents through Desktop/Pylon commands.

### Server Surface D: OpenAgents Worker MCP Facade

Purpose:

Expose OpenAgents.com product capabilities over MCP without bypassing the
normal HTTP API, OpenAPI, capability manifest, auth, policy, and rate limits.

Transports:

- Streamable HTTP for authenticated clients;
- public read-only route for discoverable public data;
- no remote admin route until owner policy and audit trails are proven.

Read-only public tools:

- `openagents.openapi.read`
- `openagents.capability_manifest.read`
- `openagents.public_activity.list`
- `openagents.pylon_stats.read`
- `openagents.training.runs.list`
- `openagents.training.run.read`
- `openagents.training.run.summary`
- `openagents.tassadar.bulletin.read`
- `openagents.proofs.read`
- `openagents.product_promises.list`
- `openagents.forum.posts.list`
- `openagents.forum.post.read`
- `openagents.forum.activity.list`

Authenticated product tools:

- `openagents.forum.post.create`
- `openagents.forum.post.reply`
- `openagents.forum.post.tip`
- `openagents.pylons.list`
- `openagents.pylon.read`
- `openagents.pylon.tip`
- `openagents.pylon.register`
- `openagents.assignments.claim`
- `openagents.evidence.submit`
- `openagents.receipts.read`

Operator/admin tools:

- `openagents.training.window.plan`
- `openagents.training.window.activate`
- `openagents.training.window.reconcile`
- `openagents.training.evidence_packet.build`
- `openagents.training.evidence.admit`
- `openagents.payout.ledger.transition`
- `openagents.provider_accounts.read`
- `openagents.provider_accounts.update`

Hard requirements:

- Generated MCP schemas must agree with OpenAPI/capability manifest schemas.
- Public tools remain public-safe.
- Authenticated tools use existing auth modes.
- Payment and payout tools produce receipt refs and reject replay.
- MCP tools cannot create authority absent from the underlying API route.

### Server Surface E: In-Process MCP Bridge

Purpose:

Support embedded runtimes, SDK sessions, tests, and local workers that need MCP
server behavior without spawning a separate process.

Capabilities:

- paired in-memory transport;
- multiple named in-process servers per session;
- request ID correlation;
- setup/teardown hooks;
- message forwarding over existing session control channels;
- test fixture support.

Hard requirements:

- In-process servers are explicit session capabilities.
- They do not bypass normal tool policy.
- They are visible in MCP status.
- They can be revoked or disconnected.

## Client Roadmap

### Client Surface A: MCP Config Model

Autopilot/Pylon should support MCP server config from:

- local private project config;
- shared project config;
- user config;
- managed/enterprise config;
- session/dynamic config;
- plugin-provided config;
- IDE-provided config;
- Desktop-discovered local config.

Required config fields:

- server name;
- transport type;
- command/args/env for stdio;
- URL for remote transports;
- static headers;
- dynamic headers helper ref;
- OAuth config;
- trust scope;
- enabled/disabled state;
- approval state;
- policy refs;
- source refs;
- plugin refs where applicable.

Required transport types:

- stdio;
- Streamable HTTP;
- SSE;
- WebSocket;
- IDE-local transports;
- in-process linked transport;
- bridge/proxy transport for remote sessions.

### Client Surface B: Config Policy and Approval

External MCP servers should be treated as untrusted capability ingestion.

Required policy behavior:

- shared project config starts pending;
- operator can approve one server, reject one server, approve all, or reject
  all;
- non-interactive sessions require explicit trust mode before project config
  auto-approval;
- managed policy can allow or deny by server name;
- managed policy can allow or deny by command signature;
- managed policy can allow or deny by URL pattern;
- managed config can take exclusive control when needed;
- plugin-provided MCP servers are deduped against manual servers;
- disabled servers are shown but not connected.

### Client Surface C: Connection Lifecycle

Required states:

- connected;
- failed;
- needs auth;
- pending approval;
- disabled;
- reconnecting;
- stale config;
- revoked.

Required behavior:

- separate concurrency limits for local process launches and remote network
  connects;
- connection timeouts;
- tool/resource/prompt fetch cache;
- cache invalidation on config hash changes;
- reconnect on selected transient transport failures;
- cleanup on server removal;
- stale client cleanup when plugin/session config changes;
- safe stderr capture for stdio servers;
- redacted transport logging.

### Client Surface D: OAuth and Remote Auth

Required behavior:

- OAuth metadata discovery;
- dynamic client registration where supported;
- configured client ID support;
- token storage outside tracked files;
- token refresh;
- token revocation;
- needs-auth state;
- explicit reauth tool/action;
- auth failure caching to prevent repeated network storms;
- step-up auth support for servers that ask for additional scopes;
- support for trusted organization identity delegation when configured.

### Client Surface E: Tool Projection

External MCP tools should be converted into native OpenAgents tool descriptors.

Required projection behavior:

- name tools as `mcp__{server}__{tool}` by default;
- preserve original server/tool name separately;
- preserve input schema;
- preserve description with length caps;
- preserve read-only, destructive, and open-world hints;
- expose search/read classification for UI compaction;
- route calls through native approval policy;
- expose structured content and `_meta` where safe;
- support progress events;
- support abort/cancel;
- support session-expiry retry where applicable;
- hide tools from servers without grants or approvals.

### Client Surface F: Resource Projection

External MCP resources should become inspectable resources and optional native
read tools.

Required behavior:

- list resources per server;
- attach server provenance;
- read resource by URI;
- classify text, image, audio, and binary resource content;
- persist binary content where appropriate;
- resize/downsample large images before model inclusion;
- never expose private binary bytes directly in public projections;
- refresh resources on list-changed notifications.

### Client Surface G: Prompt and Skill Projection

External MCP prompts should become command-like entries.

Required behavior:

- list prompts per server;
- show server provenance;
- map prompt arguments to invocation input;
- fetch prompt content lazily;
- transform returned text/image/audio/resource content safely;
- optionally discover skill-like resources only behind explicit feature and
  trust gates;
- never execute remote skill code inline.

### Client Surface H: Elicitation

MCP servers may ask the user or operator for more information.

Required behavior:

- form elicitation UI;
- URL elicitation UI;
- two-phase "open URL then retry" flow;
- hooks that can accept, decline, cancel, or modify elicitation results;
- completion notification handling;
- cancellation on abort;
- no silent auto-accept for sensitive elicitations.

### Client Surface I: Output Handling

MCP output can be huge or binary.

Required behavior:

- content-size estimation;
- result truncation with explicit warning;
- file persistence for large text outputs;
- binary blob persistence with safe file refs;
- image resizing/downsampling;
- per-tool result size policy;
- output previews;
- public-safe redaction;
- transcript resume reconstruction where needed.

### Client Surface J: Management UI

Autopilot Desktop and terminal surfaces should expose MCP management.

Required UI:

- list configured servers;
- show status;
- show transport;
- show source/scope;
- show tool/resource/prompt counts;
- show approval state;
- show auth state;
- enable/disable server;
- reconnect server;
- approve/reject project server;
- revoke auth;
- view tools;
- view resources;
- view prompts;
- inspect recent calls;
- copy client config snippets;
- show policy blockers.

## Fully Steerable Pylon and Autopilot

The server roadmap should be measured by whether external MCP clients can
perform the same operator workflows available inside Autopilot, within policy.

Minimum steering workflows:

1. Discover Pylon node health and capabilities.
2. Read wallet status and Pylon sats.
3. List coding accounts and active sessions.
4. Spawn a Codex coding session.
5. Spawn a Claude coding session.
6. Spawn a local/Apple FM session where available.
7. Send a turn to a running session.
8. Stream session events.
9. Read artifacts and diffs.
10. Resolve approvals.
11. Cancel or interrupt a session.
12. Pause/resume coordinator work.
13. Submit an intent.
14. Poll and accept assignments.
15. Read Verse scene state.
16. Interact with selected Verse object by semantic intent.
17. Read a bulletin board.
18. Read pylon detail from the world.
19. Tip a forum post with receipt policy.
20. Tip a pylon with receipt policy.
21. Read product/public activity and proof state.
22. Start a deployment only with deployment authority.
23. Read and revoke MCP client grants.

## Implementation Phases

### Phase 0: Contract Groundwork

Deliverables:

- shared MCP contract package;
- authority taxonomy;
- transport config schema;
- lifecycle status schema;
- tool/resource/prompt descriptor schema;
- receipt/error/progress/elicitation schemas;
- naming rules;
- public-safe output rules.

Exit criteria:

- Pylon, Desktop, Worker, and web projections can import shared MCP types.
- Existing Pylon TAS MCP tests still pass.
- No runtime transport is exposed yet.

### Phase 1: Read-Only Local Pylon MCP Server

Deliverables:

- stdio MCP server entrypoint for Pylon;
- loopback HTTP MCP server behind explicit local flag;
- read-only health/status/session/account/wallet tools;
- resource list/read for node status and sessions;
- MCP client compatibility smoke.

Exit criteria:

- external MCP client can initialize;
- `tools/list` returns granted read-only tools;
- `tools/call pylon.health` hits the real local node;
- `resources/list` returns live local resources;
- no mutating tools are present.

### Phase 2: Pylon Steering Tools

Deliverables:

- session spawn/reply/interrupt/cancel tools;
- approval list/resolve tools;
- intent submit/list tools;
- coordinator pause/resume tools;
- assignment poll/accept tools;
- artifact read resources;
- progress and event subscriptions.

Exit criteria:

- external MCP client can spawn and steer a real coding session.
- approval resolution produces a receipt.
- cancellation works.
- ungranted clients cannot see or call steering tools.

### Phase 3: Desktop Supervision and Verse Resources

Deliverables:

- Desktop MCP server status UI;
- connected client list;
- grants and revocation;
- recent call log;
- copyable client config;
- Verse read-only resources;
- Verse semantic interaction tools.

Exit criteria:

- operator can start/stop/revoke MCP from Desktop.
- Verse reads do not remount the scene.
- Verse interactions do not steal camera or pointer control.

### Phase 4: OpenAgents MCP Client Ingestion

Deliverables:

- external MCP config scopes;
- approval gates for shared project config;
- transport support for stdio, HTTP, SSE, WebSocket, and in-process;
- OAuth and needs-auth state;
- external tool projection;
- external resource projection;
- external prompt projection;
- elicitation UI;
- output truncation/persistence.

Exit criteria:

- Autopilot coding sessions can use approved external MCP tools.
- External MCP tools cannot bypass native permission policy.
- Disabled and rejected servers never connect.
- Auth failures surface as needs-auth, not repeated noisy failures.

### Phase 5: Worker Public MCP Facade

Deliverables:

- generated read-only Worker MCP tools from OpenAPI and capability manifest;
- public-safe product resources;
- read-only public transport;
- schema parity tests.

Exit criteria:

- external MCP client can read OpenAgents public stats, public activity,
  training summaries, forum public activity, and proof state.
- public MCP output contains no private material.

### Phase 6: Authenticated Product MCP

Deliverables:

- authenticated forum tools;
- authenticated pylon tools;
- authenticated receipt tools;
- authenticated product promise/proof/training tools;
- browser/session and agent bearer auth modes.

Exit criteria:

- authenticated MCP client can tip a forum post and receive a receipt.
- authenticated MCP client can tip a pylon and receive a receipt.
- payment attempts are idempotent and replay-safe.

### Phase 7: High-Risk Authority

Deliverables:

- payment spend tools;
- deployment tools;
- workspace write tools;
- admin tools;
- hard approval flows;
- amount caps and environment caps;
- receipt and audit trails.

Exit criteria:

- high-risk tools are absent unless explicitly granted.
- high-risk calls require policy and approval.
- every successful call has a receipt ref.
- failed/rejected calls are visible and explainable.

### Phase 8: Compatibility and Certification

Deliverables:

- compatibility smoke for Codex;
- compatibility smoke for Claude Code;
- compatibility smoke for common MCP inspector/client tools;
- Desktop-packaged app smoke;
- Worker staging smoke;
- docs for client setup.

Exit criteria:

- two external clients can connect with different grants.
- grants produce different `tools/list` results.
- revocation works during a live session.
- full steering smoke passes without local UI reset or black-frame regression.

## Testing Matrix

Unit tests:

- schema descriptor generation;
- tool naming;
- resource URI parsing;
- prompt descriptor generation;
- authority class assignment;
- read-only enforcement;
- policy filter behavior;
- approval state transitions;
- OAuth state classification;
- output truncation and persistence;
- unsafe output omission.

Integration tests:

- Pylon stdio MCP initialize/list/call;
- Pylon loopback MCP initialize/list/call;
- Pylon bridge grant filtering;
- session spawn/reply/cancel;
- approval resolution with receipt;
- wallet status read;
- wallet spend blocked without approval;
- external stdio MCP ingestion;
- external HTTP MCP ingestion;
- external prompt projection;
- external resource projection;
- needs-auth remote server state;
- project MCP approval/rejection.

Browser/Desktop smokes:

- Desktop starts with MCP server disabled by default where appropriate;
- Desktop can start local MCP server;
- Desktop shows connected clients;
- Desktop revokes a client;
- Verse state is readable through MCP;
- Verse scene does not remount during MCP reads;
- coding overlay continues to receive input while MCP is active.

Worker smokes:

- public MCP facade returns public stats;
- OpenAPI schema parity;
- capability manifest parity;
- authenticated forum read/write;
- authenticated pylon tip;
- payment receipt replay rejection.

Security tests:

- no raw mnemonic/token output;
- no raw private prompt output;
- no absolute private path in public projections;
- no external MCP tool bypasses approval;
- denied server never connects;
- rejected project server never connects;
- disabled server never connects;
- ungranted server tool cannot be called by guessed name;
- high-risk tool absent from low-trust clients.

## Documentation Plan

Required docs:

- `docs/mcp/README.md`: current MCP status and docs index.
- `docs/mcp/*roadmap*`: this roadmap.
- `docs/pylon/*`: local Pylon MCP server setup and safety model.
- `docs/autopilot-coder/*`: coding-agent MCP client/server setup.
- `docs/game/*`: Verse MCP resource and interaction model.
- `docs/payments/*`: MCP tipping/spend authority and receipts.
- `docs/launch/*`: release gates and smoke commands.
- public OpenAgents docs: Worker MCP facade usage once available.

## Suggested Issue Ladder

1. Add shared OpenAgents MCP contract package.
2. Implement Pylon stdio MCP server with read-only tools.
3. Add loopback HTTP MCP transport behind local operator flag.
4. Add Pylon bridge grant filtering to MCP tool/resource lists.
5. Add Pylon session steering tools.
6. Add Pylon approval, intent, coordinator, and assignment tools.
7. Add Pylon event/resource subscriptions.
8. Add Desktop MCP supervision UI.
9. Add Verse read-only MCP resources.
10. Add Verse semantic interaction MCP tools.
11. Add external MCP client config scopes and approval gates.
12. Add external MCP transport support.
13. Add external MCP OAuth and needs-auth flows.
14. Add external MCP tool/resource/prompt projection.
15. Add MCP elicitation UI and hooks.
16. Add MCP output persistence/truncation.
17. Add Worker public read-only MCP facade.
18. Add authenticated Worker MCP product tools.
19. Add Forum and Pylon tipping tools through MCP.
20. Add deployment MCP tools with explicit policy.
21. Add wallet spend MCP tools with amount caps and receipts.
22. Add admin MCP tools behind owner-only policy.
23. Add Codex and Claude Code MCP compatibility smokes.
24. Add packaged Desktop MCP regression smoke.

## Final Target

The target is one coherent MCP control plane:

- Pylon exposes local node control.
- Autopilot exposes Desktop and Verse state.
- OpenAgents.com exposes product and public/cloud capability.
- Autopilot/Pylon can consume external MCP tools safely.
- Every capability is typed, scoped, approved, observable, revocable, and
  receipt-bearing where needed.

MCP should become the universal way for coding agents and operator agents to
inspect and steer OpenAgents, without creating a shortcut around OpenAgents'
existing authority model.
