# OpenAgents Desktop product architecture and fastest delivery path

- Date: 2026-07-10
- Status: binding target architecture for #8574 under MASTER_ROADMAP R0–R7
- Product: OpenAgents Desktop at `apps/openagents-desktop`
- Companion client: OpenAgents mobile at `apps/openagents-mobile`
- Contract freeze consumed:
  [`khala.identity_sync_contract.v1`](./2026-07-10-r1-r2-identity-sync-contract.md)
- Evidence:
  [ChatGPT, Claude, and OpenCode adaptation analysis](../teardowns/2026-07-10-openagents-product-adaptation-analysis.md),
  [OpenCode source teardown](../teardowns/2026-07-10-opencode-desktop-app-teardown.md),
  [Desktop parity audit](./2026-07-10-opencode-khala-openagents-desktop-parity-audit.md),
  [current Desktop guarantees](../../apps/openagents-desktop/GUARANTEES.md)

This document freezes architecture and dependency order. It does not claim that
the target is implemented. Current guaranteed behavior remains limited to
[`apps/openagents-desktop/GUARANTEES.md`](../../apps/openagents-desktop/GUARANTEES.md),
current code, tests, and receipts.

## Decision in one sentence

**OpenAgents Desktop is a tokenless local Effect Native client over a host-owned
runtime gateway; that gateway composes Khala Sync, Pylon, workspace services,
and isolated execution behind one typed query/command/event contract, while
mobile joins the same durable conversation and outcome stream during the first
real runtime slice—not after Desktop parity is complete.**

Electron supplies windows and native capability. It is not the product
architecture. Pylon supplies runtime orchestration. It is not a second desktop
product. Khala Sync supplies cross-device continuity. It is not an execution
engine. Effect Native supplies application components and intents. It is not
authority.

## Why this is the fastest coherent path

The repository already contains more authority and runtime substrate than the
new clients expose:

- typed chat and Fleet entities, mutations, cursors, tombstones, offline
  queues, SQLite semantics, and fault tests in `packages/khala-sync*`;
- durable run, work-unit, attempt, approval, command-outcome, account, and
  receipt authority behind the Worker/Pylon stack;
- Codex and Claude runtime adapters plus named isolated account custody in
  Pylon;
- a hardened Electron boundary, local workspace grants, bounded read/save,
  typed Git status/diff, recent Codex history, Settings/device auth, and a
  closed command palette in `apps/openagents-desktop`; and
- an Effect Native mobile shell ready to consume the same identity and Sync
  contracts.

Building a new OpenCode-like local server, new client state model, or new fleet
authority would delay the product and create migrations. The shortest path is
to expose existing authority through one stable Desktop gateway, land R1/R2 in
both clients, prove one cross-device conversation, then deepen Desktop and
mobile in parallel.

## Target topology

```text
OpenAgents Desktop renderer (sandboxed, tokenless)
  Effect Native views + local presentation state
                 |
                 | fixed schema-decoded projections and typed intents
                 v
Electron preload + main host
  keychain / windows / grants / menus / updates / recovery
                 |
                 | private runtime protocol; never exposed as raw IPC/HTTP
                 v
Desktop Runtime Gateway
  Khala Sync client + SQLite mirror
  workspace / file / Git / PTY services
  Pylon control and runtime projections
  command policy / capability / outcome reconciliation
        |                         |
        |                         +--> local Codex / Claude / Grok adapters
        |                              isolated named homes + execution profiles
        |
        +--> openagents.com API / Khala Sync / Source Authority
                    |
                    +--> Agent Computers / remote workrooms
                    |
                    +--> OpenAgents mobile
                         secure identity + SQLite mirror + compact projections
```

The gateway is a logical boundary, not permission to build a new general-
purpose daemon. Its interface is stable even if process placement evolves:

1. The first R1/R2/D1 slices may compose lightweight gateway services in the
   Electron main process to reuse the current app safely.
2. CPU-heavy history, filesystem watch, PTY, engine supervision, extension,
   and long-lived stream work moves behind one Electron utility process before
   D3/D4 broadens those capabilities.
3. The renderer-facing contract does not change when a service moves. The
   renderer never learns a loopback URL, bearer token, provider credential,
   process handle, or raw `MessagePort`.

This placement rule preserves delivery speed without making today's main-
process composition tomorrow's application architecture.

## Frozen architecture decisions

### A1. Stock Electron remains a thin hardened host

Keep the current guarantees: context isolation on, renderer sandbox on, Node
integration and webviews off, web security on, restrictive CSP, and deny-by-
default permissions, navigation, new windows, and webview attachment.

Electron owns:

- application/window lifecycle, routes, deep links, menus, updates, crash and
  unresponsive recovery;
- OS keychain, notifications, pickers, explicitly granted paths, and public-
  safe diagnostics;
- lifecycle and supervision of the private runtime gateway; and
- the schema-decoded bridge to the renderer.

Electron does not own domain truth, provider selection policy, FleetRun state,
or arbitrary filesystem/process authority.

### A2. The renderer is a local, versioned, tokenless Effect Native client

The primary application renderer ships with the signed app. It does not load
`openagents.com` or another live website as privileged application code.

The renderer may hold:

- Effect Native view state and typed intent refs;
- bounded, redacted projections supplied by the host;
- local-only focus, selection, layout, scroll, and draft presentation state;
  and
- visually distinct optimistic overlays allowed by the Khala Sync contract.

It may not hold:

- OpenAgents bearer/refresh tokens, provider credentials, Pylon control tokens,
  raw local paths outside an explicit projection, or extension secrets;
- arbitrary IPC channel names, Node/Electron objects, process handles, generic
  filesystem handles, loopback server credentials, or raw private runtime
  events; or
- authority inferred from transcript prose, local pixels, an optimistic toast,
  or an accepted-but-not-completed command.

### A3. One closed renderer bridge exposes projections and typed intents

The preload surface grows only through closed Effect Schema unions. Its stable
shape has three responsibilities:

1. bootstrap a bounded application/session/capability projection;
2. invoke one registered typed intent with a stable command/idempotency
   identity; and
3. subscribe to one typed projection/event stream with owned disposal and
   bounded buffering.

This is not a stringly generic RPC escape hatch. Every union member names its
input, output, policy, capability, approval posture, redaction class, and
supported client surfaces. Main validates sender, frame, origin, schema,
capability, and target scope. Responses are decoded again before the renderer
uses them.

The existing command registry is the UI index over this contract. Palette,
keybinding, native menu, direct button, mobile control, and future model-
proposed actions must converge on the same command ID and runtime outcome.

### A4. The Desktop Runtime Gateway is the only local application-service seam

OpenCode proves that a server-first workbench scales better than dozens of
renderer-native capabilities. OpenAgents adapts the topology but strengthens
the trust boundary: the authenticated runtime client stays host-owned.

The gateway exposes internally:

- **queries** for bounded current projections and capability discovery;
- **commands** for schema-decoded, idempotent intent submission; and
- **events** for ordered facts with connected, heartbeat, stale, reconnect,
  must-refetch, disposal, and terminal semantics.

It owns backpressure, adjacent-delta coalescing, replay cursors, cancellation,
resource scopes, and deterministic shutdown. A transport timeout becomes
`unknown_pending_reconcile`; it never becomes success.

Do not create a second public API, second local run database, or general
renderer-facing localhost service. Extend current packages first. Extract a
new shared package only after at least two consumers prove the boundary.

### A5. Pylon is composed, not cloned

Pylon and `packages/pylon-core` remain the local multi-engine supervisor and
account/runtime authority. The Desktop gateway adapts their existing typed
capabilities; it does not reimplement account custody, claims, assignment
execution, engine health, quota, breakers, receipts, or worktree policy.

Provider-specific process protocols terminate in Pylon/runtime adapters. The
renderer and mobile see provider-neutral capabilities and typed failures.
Named account refs, isolation homes, usage truth, and no-substitution rules
remain enforced at the runtime boundary.

Pylon's orchestration SQLite and the Desktop Khala Sync SQLite mirror have
different purposes. They may not become competing authorities:

- Pylon local storage owns runtime supervision/recovery facts until they are
  durably projected;
- Source Authority owns accepted run/work/attempt/command/receipt truth; and
- the Desktop Sync store is a cache/offline queue over those projections.

### A6. Khala Sync lands before broad Desktop parity

R1/R2 is not a late integration phase. It is the state substrate for D1 and the
first mobile value.

The initial shared catalog is exactly the frozen contract:

- personal, thread, and FleetRun scopes;
- `chat_thread` and `chat_message`;
- current Fleet entities and `fleet_command_outcome`;
- server-derived identity, monotonic versions/cursors, tombstones, explicit
  stale/reconnecting/must-refetch/denied states; and
- durable mutation identity with applied/rejected/duplicate outcomes.

Desktop stores its Sync SQLite mirror outside the renderer. Mobile uses the
same store semantics through the accepted Expo SQLite binding. Neither client
may create an app-local thread or Fleet schema to move faster.

### A7. Mobile continuation is part of the first real conversation exit

Desktop D1 is not complete when Desktop alone streams a provider turn. Its
first complete vertical slice proves:

1. the same server-derived owner signs in on Desktop and mobile;
2. Desktop creates or opens one durable thread;
3. a real streamed turn projects ordered text/runtime state into that thread;
4. mobile observes the same thread/message refs, versions, phase, and terminal
   outcome;
5. mobile can submit one safe follow-up or interrupt through the same typed
   intent path;
6. both clients restart and reconcile without duplicate objects; and
7. revocation, cursor gap, and lost acknowledgement fail closed according to
   `khala.identity_sync_contract.v1`.

The first mobile slice is intentionally narrow: sign-in, thread catalog,
thread detail, stream/outcome state, follow-up/interrupt, and Sync health. It
does not wait for mobile files, terminal, preview, writeback, full Fleet, push,
or polish. Those build on the proven seam.

### A8. Workspace authority stays local and capability-shaped

Files, Git, review, and PTY are runtime services, not renderer privileges.

- A user-selected root creates a host-owned grant.
- All paths are canonicalized beneath that grant; symlink/traversal escape
  fails closed.
- Reads, writes, watches, search, Git operations, and PTY sessions use fixed
  typed operations with size/output/time budgets.
- Edits use revision/conflict checks and atomic writes.
- Git never accepts arbitrary argv from the renderer.
- PTY connect/replay/input/resize/close is one scoped lifecycle. A ticket or
  equivalent capability is bound to the exact workspace/session and consumed
  by the host, not exposed as a general server credential.
- Effect Native editor, diff, and terminal foreign hosts receive serializable
  configuration and emit typed intents; library instances do not escape.

Local workspace state syncs only as safe refs or explicit content selected for
a durable action. Raw paths, file contents, terminal output, and private diffs
do not enter general Sync projections.

### A9. Host authority and execution isolation are separate

The Electron renderer sandbox protects the host from renderer compromise; it
does not sandbox agent-generated shell or code.

Every execution names one profile:

- projection-only;
- workspace-bounded local;
- isolated guest;
- explicit owner-local danger mode; or
- managed Agent Computer.

The runtime receipt records the selected profile, grants, workspace/workroom
refs, engine version, egress posture, usage truth, verification, and evidence
refs. Mobile never receives local danger mode or local shell authority. Remote
mobile coding uses brokered workroom capabilities and safe branch/PR writeback.

### A10. Extensions enter through an isolated signed catalog

MCP, MCPB, skills, plugins, and custom tools are compatibility inputs, not
ambient authority. Each catalog item declares publisher/provenance, immutable
hash, runtime, network/filesystem/secret/browser/UI capabilities, tool schemas,
policy, update, rollback, and per-session enablement.

Do not load third-party npm/plugin code into the primary Desktop gateway merely
because OpenCode does. Stdio/HTTP MCP and executable skills run in the selected
execution profile with owned lifecycle and redacted receipts. Extension breadth
waits until D1–D3 and the isolation contract are proven.

### A11. Distribution is a compatible component set

Before D6 distribution, one signed compatibility manifest records the Electron
host, renderer, gateway/runtime protocol, Pylon/engine adapters, native modules,
extension catalog inputs, sandbox/workroom images, Sync schema window, and
update channel. It names hashes, signatures, protocol ranges, migration,
rollback, and last-known-good state.

Electron fuses, entitlements, asar integrity, signing/notarization, update,
rollback, and clean-machine recovery are mechanical release oracles. A passing
development smoke is not a release receipt.

## Data and authority ownership

| State | Authority | Desktop persistence | Mobile persistence | Renderer visibility |
| --- | --- | --- | --- | --- |
| OpenAgents identity/session | Server/OpenAuth | Main/keychain; session phase projection | Secure storage; session phase projection | Phase and safe account refs only |
| Conversations | Worker/Khala Sync mutators | Main/runtime SQLite Sync mirror | Expo SQLite Sync mirror | Typed thread/message projections |
| Fleet run/work/attempt/approval/outcome/receipt | Worker/Pylon/Source Authority | SQLite Sync mirror + local runtime reconciliation | SQLite Sync mirror | Owner-safe bounded projections |
| Pylon account credentials and health probes | Pylon named-account custody | Pylon-owned isolated homes | Never | Readiness/quota/blocker projection only |
| Workspace files/Git/PTY | Host runtime under explicit grant | Granted workspace + bounded local metadata | Remote workroom only | Typed bounded projections, no general handle |
| Window/layout/focus/drafts | Client-local | Window-scoped local store | Device-local store | Yes; never Source Authority |
| Raw provider/runtime events | Runtime private evidence store | Host/private refs as required | Never raw | Redacted event algebra only |
| Extensions and sandbox images | Signed catalog/update authority | Host-managed cache | Discovery/enablement projection | Metadata and typed capabilities only |

## Stable protocol properties

The exact schemas remain owned by current source packages and the serialized
Sol integration lane. Implementations must preserve these properties:

- protocol and capability version negotiation;
- stable owner, device, runtime, thread, session, run, work-unit, attempt,
  command, outcome, artifact, and receipt refs;
- ordered event cursor with bounded replay and `must_refetch` beyond retention;
- explicit connected/heartbeat/stale/reconnecting/disposed/terminal phases;
- command idempotency and separate delivery/effective outcomes;
- cancellation and resource disposal;
- redaction/provenance on every renderer/mobile projection;
- additive current/previous compatibility window and explicit reset for a
  breaking schema; and
- deterministic fixtures replayable without live providers.

Do not create speculative R3–R7 entity schemas in client code. The open DRAFT
register in `khala.identity_sync_contract.v1` remains the freeze queue for
`device_session`, project/session, workroom, preview, artifact, writeback, and
receipt additions.

## Fastest delivery sequence

The sequence is a dependency graph, not a mandate to serialize independent
client work.

### F0 — freeze and preserve the truthful foundation

- Land this architecture and reconcile roadmap/README links.
- Keep the Desktop boundary, local save/Git/command work, mobile shell, and
  existing R0 tests green.
- Remove or label any fake/dormant authority surface.

Exit: architecture has one owner, current guarantees remain honest, and no
implementation lane must guess the renderer/runtime/Sync boundary.

### F1 — shared identity and Sync adapters

- Desktop bearer session in OS keychain/main; mobile bearer session in secure
  storage.
- Desktop interactive auth uses public client `openagents-desktop` and an
  [RFC 8252](https://www.rfc-editor.org/rfc/rfc8252.html) literal IPv4 loopback
  callback at `http://127.0.0.1:{ephemeral-port}/auth/callback`, with GitHub
  authorization-code + S256 only. It does not register or contend for mobile
  `openagents://auth`; the callback listener binds loopback only and closes
  after one bounded result.
- Freeze and land `device_session` only through the R1 contract owner.
- Bind Desktop main/runtime SQLite and mobile Expo SQLite to existing Khala Sync
  client semantics.
- Render exact session and Sync phases before product breadth.

Exit: both clients resolve the same server-derived owner/scope, can be revoked
independently, and show no cached row as live before reconciliation.

### F2 — one real conversation, continued on mobile

- Replace local five-thread/request-response authority with `chat_thread` /
  `chat_message` and the provider-neutral runtime event algebra.
- Ship streaming, interrupt, reconnect, terminal outcome, and minimum composer
  context on Desktop.
- Ship the narrow mobile continuation slice defined in A7 concurrently.
- Run cross-client restart, duplicate, cursor-gap, revocation, and lost-ACK
  fixtures.

Exit: one real Desktop turn continues on mobile with matching refs/versions and
one safe mobile action converges back. This is the earliest product milestone.

### F3 — projects, sessions, and the shared command plane

- Extend the landed Desktop command registry so direct controls, palette,
  keybindings, native menus, and routes call the same typed command IDs.
- Add authoritative project/session identity only after its Sol schema freeze.
- Add tabs, search/archive, deep links, restore, and selected context.
- Carry only safe active-context continuity to mobile.

Exit: no material Desktop action is an unregistered callback or ad hoc route.

### F4 — deepen both workbenches in parallel

Desktop lane:

- recursive lazy files, watch/cache/search, editor save/conflict, Git review,
  comments/revert, and scoped PTY foreign hosts.

Mobile lane:

- repository/thread binding, managed workroom lifecycle, compact plan/files/
  changes, bounded remote terminal/preview, artifacts, verification, and safe
  writeback.

Exit: a useful coding loop completes on each form factor without widening
renderer or phone authority.

### F5 — runtime/settings and extension compatibility

- provider/model/runtime selection, account readiness, permissions, MCP,
  catalog provenance, preferences, accessibility, notifications, diagnostics,
  and recovery;
- isolate executable extensions before enabling breadth.

Exit: every visible setting mutates real host/runtime state or explains why it
is unavailable; credentials and private payloads remain host-only.

### F6 — authoritative Fleet cockpit and attention loop

- Compose existing Fleet/approval/command-outcome/receipt projections into
  Desktop and mobile.
- Reuse the shared command plane for steer, approve/reject, pause/resume/drain/
  stop, and Inbox acknowledgement.
- Prove one real Codex+Claude run from both clients.

Exit: both clients converge on the same durable effective outcomes and receipts
with zero transcript/pixel inference.

### F7 — productization and dogfood

- Identity freeze, fuses, signing/notarization, component manifest, update,
  rollback, diagnostics, physical iOS/Android proof, push/deep links, and
  sustained cross-device dogfood.

Exit: MASTER_ROADMAP R7.

## Parallelization rule

After F1 freezes the shared identity/Sync seam, three lanes may run without
waiting for the whole roadmap:

| Lane | Owns | Must not independently change |
| --- | --- | --- |
| Desktop product | gateway adapter, renderer projections, local workspace and command surfaces | Sync schemas, server mutators, Pylon authority |
| Mobile product | secure session adapter, SQLite binding, compact projections, remote-workroom UX | Sync schemas, local Desktop capability, workroom authority |
| Sol/shared contracts | schemas, migrations, generated clients, compatibility fixtures, Source Authority projection | app-local layout/presentation |

All three meet in cross-client fixtures. File-disjoint work that changes the
same schema, migration, command identity, or authority policy is still
contract-colliding and remains serialized.

## Verification ladder

Architecture acceptance requires more than unit coverage:

1. **Boundary oracle:** packaged/source checks for sandbox, CSP, fuses, sender/
   origin validation, token absence, closed IPC, and renderer import bans.
2. **Contract fixtures:** schema round trips and deterministic runtime-event
   replay across current and previous supported versions.
3. **Cross-client fixture:** Desktop/mobile identity, Sync conversation,
   mutation/outcome, revocation, restart, duplicate, gap, and lost-ACK proof.
4. **Workbench tests:** traversal/symlink/budget/conflict, fixed Git operations,
   PTY reconnect/dispose, cache eviction, large-thread and tab-switch budgets.
5. **Execution receipt:** named account, execution profile, grants, usage truth,
   verification, and closeout.
6. **Packaged/live receipt:** signed builds, clean install, update/rollback,
   physical devices, real cross-device continuation, and owner acceptance.

Every proof reports the narrowest true rung: code-landed, fixture-proven,
deployed, live-proven, owner-accepted, or closed.

## Explicit non-goals before F7

- browser-runtime fork;
- privileged live-web renderer;
- ambient screen recording or inferred personal memory;
- a new OpenCode server clone or second Pylon;
- renderer-held localhost/server credentials;
- multi-window workbench depth and WSL unless a supported user path requires
  them;
- arbitrary computer use, scheduling, SSH environment management, or complex
  artifact studios before the core authority/isolation loop is proven; and
- extension breadth before signatures, capability declarations, isolation,
  update, rollback, and receipts exist.

## Failure modes this architecture prevents

| Failure | Prevention |
| --- | --- |
| Renderer compromise becomes full local runtime authority | No bearer/loopback/Pylon credential or generic runtime transport enters renderer |
| Desktop and mobile show different “truth” | Same Sync scopes, versions, entities, mutation IDs, and durable outcomes |
| Accepted command is displayed as completed work | Delivery and effective outcome remain separate durable records |
| Electron sandbox is mistaken for code sandbox | Explicit execution profile and receipt for every run |
| Pylon and Desktop create two run universes | Gateway composes Pylon/Source Authority; no second claim or run database |
| Local UI state leaks into Source Authority | Presentation state remains device/window scoped; only typed mutations cross |
| Extension ecosystem widens authority invisibly | Signed catalog, declared capabilities, isolated lifecycle, per-session enablement |
| Early mobile work waits behind full Desktop parity | F2 requires the narrow mobile continuation proof before D3–D6 breadth |
| Fast implementation hardens into main-process sprawl | Gateway interface is stable; heavy services move to utility process before D3/D4 breadth |

## Final architectural test

Any proposed Desktop feature must answer all seven questions before it lands:

1. Which typed command/query/event owns it?
2. Which service is authoritative?
3. Which process holds the capability and credentials?
4. What may cross into renderer, mobile, Sync, diagnostics, and receipts?
5. How does it reconnect, replay, cancel, dispose, and fail closed?
6. Which execution profile and policy apply?
7. Which automated oracle and proof rung demonstrate it?

If the answer is “the renderer can call a new IPC/HTTP method and update local
state,” the feature is not architecturally ready.
