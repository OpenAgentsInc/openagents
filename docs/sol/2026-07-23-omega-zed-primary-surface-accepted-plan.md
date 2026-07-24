# Omega, the Zed-based OpenAgents IDE — accepted plan

- Class: accepted owner authority and plan ledger
- Status: active
- Date: 2026-07-23
- Updated: 2026-07-23
- Plan revision: 4
- Owner authority: current owner conversation
- Base commit: `c348abb746f70d58f240e8f24a7f6f4ce802f239`
- Dispatch: plan and contract admission only
- Product name: Omega
- Primary surface: OpenAgents Desktop and IDE
- Source analysis:
  [`Nostr-first OpenAgents Desktop with Rust and Effect`](./2026-07-23-nostr-first-rust-effect-desktop-analysis.md)
- Source teardowns:
  [`Zed`](../teardowns/2026-07-18-zed-teardown.md),
  [`Cursor`](../teardowns/2026-07-11-cursor-product-teardown.md), and
  [`Buzz`](../teardowns/2026-07-21-buzz-teardown.md)

## Owner outcome

The owner accelerated this plan on 2026-07-23.
The [Omega roadmap](../omega/ROADMAP.md) now owns the implementation order.
The first target is the branded `v0.2.0-rc1` bootstrap candidate.
Current Desktop behavior comes next.
Selected Buzz workroom and Nostr outcomes follow Desktop parity.

The bootstrap candidate does not claim Desktop parity.
It does not make Omega the primary release.
All release, migration, and cutover gates in this plan still apply.

OpenAgents will make Omega its primary Desktop and IDE destination.
Omega will start as a tracked fork of Zed.
The fork and all primary Desktop client code will live in a separate Omega
repository.
Omega will not be a subtree, workspace member, or client package in this
monorepo.
OpenAgents will use the complete Zed editor and project substrate.
It will not recreate that substrate in the current Electron shell.

Omega will be the deep-work surface for company work.
It will combine the durable OpenAgents workroom with a first-class native IDE.
It will connect conversation, decisions, code, reviews, evidence, approval,
delivery, and signed social context.

OpenAgents will not run Buzz as a separate team product.
The planned OpenAgents Buzz installation and its separate forge program are
canceled.
Omega will implement the useful Buzz outcomes as native GPUI workroom panes.
The result must also let a user attach an existing configured agent, such as
Hermes.
Omega must not replace the agent's home, memory, skills, tools, identity,
provider setup, or credentials.

This decision selects the destination.
It does not make the current Electron application obsolete today.
The current application remains the supported release and the rollback source
until Omega passes the cutover gates in this plan.

## Decision

1. Omega is the product name for the new OpenAgents Desktop and IDE plan.
2. OpenAgents will fork Zed at an exact audited commit.
3. OpenAgents will keep a tracked upstream relationship.
   The fork must have a small owned patch, a rebase budget, and an update
   record.
4. The tracked Zed fork and all Omega client implementation will live in a
   separate Omega repository.
5. The OpenAgents monorepo will own reusable TypeScript packages and Effect
   services.
   It will also own Effect Schema contracts, generated-client sources,
   conformance fixtures, and platform adapters.
   Omega and other clients can consume these artifacts.
6. Omega Rust will own the native application core. This core includes editor,
   buffer, project, language, Git, terminal, task, extension, local and remote
   IDE, ACP process, and native enforcement mechanics.
7. A packaged Node 24 and Effect service will run beside the Rust application.
   It will first host the current OpenAgents product control plane while the
   migration moves suitable storage, runtime, and native enforcement into Rust.
8. Effect will continue to define ProductSpec and AssuranceSpec contracts,
   policy composition, action semantics, and typed coordination where Effect is
   the stronger implementation.
9. Rust can own durable state, runtime state machines, credential custody,
   receipts, and other product domains after an admitted packet proves semantic
   parity and one-authority cutover. Rust does not gain authority only because
   code moves to Rust.
10. Rust and Effect will use one generated, versioned process protocol.
   They will not keep two project, document, thread, run, command, or receipt
   stores synchronized by convention.
11. A supervised Rust Nostr component can own protocol mechanics, local event
    storage, relay sessions, replay, and signature operations.
    A typed OpenAgents admission still decides what those facts mean.
12. Omega will reproduce the useful Buzz workroom behavior in native GPUI
    panes.
    It will not run Buzz as a second product, relay, forge, or authority.
13. Mobile and web will continue to use the same durable OpenAgents work record.
    They will not depend on Zed state.
14. ProductSpec, AssuranceSpec, invariants, release contracts, and repository
    law must change through their own admitted packets before product-code
    migration or primary cutover.
15. Omega will support bring-your-own agents.
    An existing agent can attach through ACP or another admitted adapter.
    Omega must preserve the agent's existing configuration and show the exact
    capability boundary.

## Repository and package boundary

The Omega repository starts as the tracked Zed fork.
It owns:

- the Zed source tree, upstream remotes, pins, patches, and merge records
- Omega Rust, GPUI panes, client state, native commands, and desktop-only
  workers
- Omega-only adapters, application assets, packaging, updates, and release
  integration
- the Rust side of the generated local protocol and the supervisor that starts
  the packaged Node service
- Omega client tests, accessibility proof, performance proof, and release proof

The OpenAgents monorepo owns reusable platform code.
It will define and produce selected consumable TypeScript packages, Effect
services and layers, Effect Schema contracts, generated-client sources,
conformance fixtures, shared runtimes, and cloud-service adapters.
It also owns `openagents.com`, Pylon, cloud services, and independent clients
that do not belong in the Zed-derived Desktop application.

Not every current workspace package is ready for cross-repository use.
OMEGA-01 and OMEGA-03 must define a supported export surface for each selected
package.
Each artifact must have compiled JavaScript and declarations.
It must have concrete dependency versions, provenance, and a software bill of
materials.
It must also have compatibility tests, an immutable version, and a digest
before Omega consumes it.
An acceptance or release build cannot depend on a relative path into this
monorepo or on an unpublished `workspace:*` edge.

The dependency direction is from Omega to released OpenAgents artifacts.
A shared monorepo package must not depend on Omega client internals.
Omega must pin exact package or service artifacts in its lock and component
manifest.
It must not copy shared TypeScript or Effect source into the fork.
The canonical process schema stays in the monorepo.
Omega consumes generated Rust bindings or another immutable Rust contract
artifact and does not maintain a second handwritten schema.

The source for the reusable `omega-effectd` service stays in the OpenAgents
monorepo.
The current Electron client can consume its workspace build during extraction.
The Omega repository owns the thin bootstrap, supervisor, and package
integration for the released service artifact.
One compatibility manifest binds both repository commits, OpenAgents package
versions and digests, the generated protocol version, Node version, and data
schema version.

Client code can stay in the monorepo only when it has a separate runtime and
release boundary that makes placement in Omega incorrect.
Each exception must name its client owner, shared package edges, release path,
and migration or retirement gate.
An exception cannot create a second primary Desktop.

The current exceptions are:

- `apps/openagents-desktop` remains as the supported Electron migration,
  compatibility, and rollback client.
  It can receive already-admitted closure work and required security,
  reliability, release, extraction, migration, and rollback changes.
  New primary Desktop product work belongs in Omega.
- `apps/openagents-mobile` remains the React Native and Expo mobile client.
  It consumes monorepo packages directly and has its own native, OTA, signing,
  and store-release path.
  It does not depend on GPUI, Zed state, or an Omega-local store.
- `apps/openagents.com` remains the web and public trust surface.
  Pylon and cloud services remain service or operator surfaces, not Omega
  client code.

Omega becoming the primary Desktop client is not a reason to move mobile into
the Omega repository.
A later mobile move requires a separate accepted migration packet.
That packet must replace all workspace-only dependencies with immutable
artifacts.
It must transfer Expo, native build, OTA, signing, provenance, test, and
rollback ownership.
It must preserve one shared authority.
It must prove that the new repository has more value than atomic cross-client
changes.

If mobile moves, it will normally use its own repository instead of entering
the Zed fork.

Shared logic must stay independent of GPUI, Electron renderer, React Native,
and web rendering.
Client components stay with their owning client unless an accepted packet
proves a portable component boundary.

## The Cursor lesson

Cursor started from a mature editor and added an agent product around it.
It continued to merge upstream changes.
OpenAgents will use the same basic strategy with Zed.

OpenAgents will keep these parts of the strategy:

- start with a complete, high-quality editor substrate
- put agent work in the primary workflow
- keep upstream merges as normal product work
- measure product value through complete user journeys
- keep the product layer distinct from the upstream editor layer

OpenAgents will not copy these Cursor failure modes:

- fragmented local stores with unclear authority
- opaque identity, usage, or charge truth
- silent changes to user defaults
- update speed without complete regression proof
- closed custody that prevents export, inspection, or recovery

## Product thesis

Omega is one home screen for company work, with the IDE as its deepest mode.
A person can talk with a team, give work to agents, inspect the work, approve
it, and deliver it in one surface.

The durable OpenAgents thread and workroom remain the product object.
A Zed project, buffer, panel, or channel is a projection of that object.
The editor is not a separate authority.

Omega must support this complete journey:

1. Open a current company work record.
2. Inspect its exact conversation, decisions, code, and evidence.
3. Attach an existing configured agent or select an OpenAgents-managed agent.
4. Queue, steer, interrupt, or replace an agent in the same durable thread.
5. Open the attached project and file without delay.
6. Edit, review, test, and verify the change.
7. Approve and perform the exact delivery action.
8. Inspect the commit, release, or service receipt.
9. Continue or supervise the same work from mobile or web.

## Architecture boundary

```text
Omega repository
  Omega Rust application
  GPUI, editor, buffer, project, Git, language, terminal, tasks,
  local/remote IDE, ACP client, native enforcement, OS credential broker,
  component supervisor, Rust-owned stores
                         |
       one generated, versioned local process protocol
                         |
 packaged Node 24 + omega-effectd artifact
  published from the OpenAgents monorepo and pinned by Omega
  ProductSpec and AssuranceSpec contracts, workroom coordination,
  policy composition, approvals, provider rules, Full Auto product
  logic, Sync/cloud adapters, and not-yet-migrated durable services
                         |
  isolated extension host, oa-nostrd/signer, ACP/provider workers,
       user-owned agents, Codex, Claude, Hermes, Grok, MCP,
       Pylon, and cloud APIs
```

This is one product with two supervised processes.
It is not one source repository.
The app package includes an exact Node 24 component and the compiled
`omega-effectd` program.
Omega Rust launches the service and binds it to an inherited standard-I/O or
private local-socket channel.
The service does not open a public port.
The release manifest binds Rust, Node, service, protocol, and data-schema
versions to one signed compatibility set.

Zed's `NodeRuntime` shows that the Rust application can find, install, and
launch Node programs.
Omega must not use that dynamic download path for its product control service.
The authority service uses the exact signed Node component in the Omega
package.
A later Node single-executable build can reduce package complexity.
It must remain a separate supervised process because the process boundary
limits failures.

The current Electron main process contains several OpenAgents authorities.
These authorities include Full Auto control, IDE project and document state,
Git and terminal state, credential custody, runtime lifecycle, and updates.
OMEGA-01 assigns each domain to Rust, Node/Effect, or an isolated worker.
OMEGA-03 first extracts the current product control plane into
`omega-effectd`.
Later packets move selected domains to Rust with differential replay and an
atomic store cutover.

Omega uses a new `omega-openagents` protocol.
It does not overload the Zed collaboration or remote editor RPC.
The protocol has a component-manifest handshake and process generations.
It also has stable refs, idempotency, cursors, acknowledgements, cancellation
fences, audience, hard frame limits, bounded queues, and backpressure.
A random per-launch capability protects a standard-I/O, Unix-socket, or named-
pipe transport.
A transport acknowledgement is not an OpenAgents receipt.

Omega can start with a read-only or partial shell spike.
It cannot win the primary-surface gate if it does not exercise the same
authority, workroom, Full Auto, migration, and receipt paths.

Each native worker must have:

- a declared purpose and capability set
- hard file, byte, output, queue, concurrency, and time limits
- a generation and cancellation fence
- overload, gap, and restart states
- process supervision and a circuit breaker
- memory-pressure behavior
- a typed degraded mode

Omega must not put editor, Git, Nostr, index work, and agent execution in one
fatal Rust domain.

## Zed source decision

The source audit uses Zed commit
`f032f4d433da3747f9d7bcc9e9cd52d6ca3fb3e4`.

| Zed source | Observed boundary | Omega decision |
| --- | --- | --- |
| `crates/zed/src/main.rs` | The Rust entry point creates `NodeRuntime`, language services, extension host, client, workspace store, update service, and one global `AppState`. | Keep Omega as the Rust composition root. Launch and supervise `omega-effectd` from this root. |
| `crates/workspace/src/workspace.rs` | `AppState` holds language, client, workspace, file-system, Node-runtime, and session services. | Add a small Omega service supervisor and protocol client. Do not put a second project graph in Node. |
| `crates/project/src/project.rs` | `Project` already composes worktree, buffer, LSP, Git, task, terminal, debug, context-server, agent-server, and local/remote state. | Make this Rust graph authoritative for IDE mechanics. Remove the Electron `IdeProject` graph after migration proof. |
| `crates/node_runtime/src/node_runtime.rs` | `NodeRuntime` manages system or downloaded Node and exposes npm commands for persistent processes. | Reuse the lifecycle pattern, but package a fixed Node 24 for `omega-effectd`. Do not use PATH or a network download for the authority service. |
| `crates/project/src/agent_server_store.rs` and `crates/agent_servers/src/acp.rs` | `AgentServerStore` resolves local or remote agent commands. `AcpConnection::stdio` connects to external agent processes. | Keep the ACP client and process supervision in Rust. Let Effect select and admit product-level agent work through the local protocol. |
| `crates/remote_server` and `crates/proto` | A headless Rust project and typed protobuf protocol support a remote editor and reconnection. | Reuse the local/remote project model. Create a separate generated OpenAgents protocol instead of adding product authority to Zed RPC. |
| `crates/extension_host/src/wasm_host.rs` and `capability_granter.rs` | Wasmtime hosts versioned extensions in the application process and checks process, download, and npm capabilities. | Move the extension host to an isolated Rust process. Keep a stricter manifest and owner-grant intersection. Do not expose the Node service as a general extension host. |
| `crates/terminal`, `crates/project/src/lsp_store.rs`, and `crates/task` | Rust owns terminal, language-server, and task processes and projects their state. | Move local process supervision to Rust. Keep policy admission and receipt semantics in the shared contract. |
| `crates/crashes/src/crashes.rs` | A separate crash handler records failures but does not restart application components. | Add an Omega supervisor with health checks, restart budgets, generations, fences, and circuit breakers. |
| `crates/remote/src/protocol.rs` | The remote socket protocol announces a frame length without a clear allocation limit at the read boundary. | Put a tested hard frame limit before allocation in each new Omega transport. |
| `crates/auto_update` and GPUI auxiliary-executable paths | The update path is not a complete Omega multi-component digest proof. Auxiliary bundle lookup is incomplete on Linux and Windows at this pin. | Add one signed component graph and explicit companion packaging tests on every supported target. |

This source shape supports a Rust-first Omega.
It does not support an embedded JavaScript user interface or a second Node IDE
backend.
The useful Node boundary is one adjacent product and policy service.

## Initial domain allocation

| Domain | Initial owner | Migration direction |
| --- | --- | --- |
| GPUI, editor, buffers, projects, worktrees, search, language, Git, terminal, tasks, and debug | Omega Rust | Rust from the first fork |
| local/remote project transport and ACP process transport | Omega Rust | Rust from the first integration |
| extension host and guest capability enforcement | isolated Rust process | Extract from the GPUI process before third-party extension admission |
| Nostr event validation, archive, outbox, relay pool, replay, signer requests | isolated Rust `oa-nostrd` and signer process | Rust before the first Nostr product slice |
| OS key custody, file grants, process enforcement, update and component health | Omega Rust | Rust after contract and release admission |
| ProductSpec and AssuranceSpec parsers, policy composition, action semantics, approval rules | Node/Effect | Keep in Effect until a Rust port has differential semantic proof |
| workroom, thread, Full Auto, provider policy, Sync, cloud adapters | Node/Effect first | Port only when a packet removes duplicate code or closes a measured gap |
| durable event and receipt storage | current Effect service first | Select one Rust or Effect store in OMEGA-01. Migrate atomically. Never dual-write as authority. |
| provider, MCP, user-owned agent, and managed-worker processes | isolated workers under Rust supervision | Keep process and resource control in Rust. Preserve an attached agent's home and configuration. Keep product admission in the shared contract. |

Rust ports use the same standard as upstream imports:

- one target-owned contract
- one source and destination owner
- differential fixtures and replay
- versioned data migration
- one cutover point
- deletion of the old authority path
- rollback evidence

## What Omega takes from Zed

Omega will start from the coherent Zed application graph:

- GPUI application and native renderer
- the editor, rope, text, buffer, and excerpt model
- project and worktree identity
- language services, diagnostics, search, and tasks
- Git and terminal integration
- local and remote project symmetry
- agent and Agent Client Protocol integration points
- native performance and interaction behavior

OpenAgents will retain exact upstream provenance.
It will record the Zed commit, source tree, licenses, notices, modifications,
dependency closure, and build instructions.

OpenAgents will not treat upstream benchmark or accessibility claims as target
evidence.
The packaged Omega application must pass its own performance, accessibility,
recovery, update, and release gates.

## Buzz replication scope in Omega

Omega will reproduce product outcomes from Buzz.
It will not seek source, screen, or event-kind parity.
The OpenAgents ProductSpec and AssuranceSpec will define acceptance.

The current Buzz review uses these exact source snapshots:

- Buzz `acfbb1bb6af54cb29cb152496ff43b8285dcb8cf`
- the earlier teardown and Git follow-up snapshots in the Buzz teardown
- Hermes Agent `5be99b6fce16e7d5304196bc9faf3f0cdfc3031f`

The Buzz snapshot is version `0.4.23`.
Its managed-agent runtime catalog names Goose, Claude Code, Codex, and Buzz
Agent.
It accepts a custom executable path, but its configuration bridge and runtime
metadata still depend on the known catalog.
This is the product gap in the owner-supplied feedback.

The Hermes snapshot provides an ACP standard-I/O adapter through `hermes acp`.
That adapter loads the user's existing `HERMES_HOME` and `.env`.
It also uses the existing Hermes sessions, memory, skills, tools, and MCP
configuration.
Omega can therefore attach Hermes without creating a second Hermes profile.
The Hermes ACP documentation identifies gaps in native messaging and scheduled
task features.
Omega must show each adapter gap instead of claiming complete agent parity.

### Capability disposition

| Buzz product behavior | Omega disposition |
| --- | --- |
| Home, inbox, mentions, blockers, and recent activity | Required as a native GPUI attention pane. |
| Stream channels, threads, replies, reactions, pins, and bookmarks | Required as native GPUI workroom and thread panes. |
| Direct and group messages | Required through the durable OpenAgents thread model. Private content keeps its current audience rules. |
| Agent directory, personas, teams, presence, and status | Required as an agent roster and workroom membership view. |
| Existing user-configured agents | Required. Omega attaches them through an admitted adapter and does not replace their configuration. |
| Conversation history and agent context | Required. A workroom joins exact conversation, decisions, files, changes, reviews, and receipts. |
| Search | Required across authorized workroom records, projects, files, symbols, changes, and receipts. |
| Files, uploads, previews, canvases, and media comments | Required for files and previews. Canvas and frame comments can follow after the first dogfood slice. |
| Git repositories, changes, patches, reviews, approvals, and merge state | Required in the native editor and review panes. Git refs and admitted policy remain authority. |
| Workflows and human approval steps | Required after the first workroom slice. Structural loop prevention is mandatory. |
| Read state, reminders, scheduled posts, and notifications | Required through current OpenAgents continuity and notification contracts. |
| Forum and long-form social posts | Project the existing OpenAgents Forum where useful. Do not create a second forum authority. |
| Membership, roles, moderation, and tombstones | Required before a multi-user workroom release. Enforcement stays at the OpenAgents identity and command seams. |
| Voice huddles | Deferred. Reuse the governed audio lane only after a new voice decision. Do not clone the Buzz media stack. |
| Agent metrics and live observability | Required as typed usage, health, progress, stall, and receipt projections. |
| Owner-readable agent memory | Required as an invariant. The owner audit cannot depend on agent cooperation. |
| Agent-first JSON command surface | Use Pylon and the generated Omega protocol. Do not add a Buzz CLI dependency. |
| ACP subprocess pool, queues, crash recovery, and typed stalls | Required under the Omega Rust supervisor. |
| Runtime conformance replay | Required at selected identity, command, store, migration, and receipt seams. |
| Nostr identity, groups, DMs, Git facts, pairing, and signed evidence | Keep as optional protocol interoperability and signed projections. Nostr is not workroom authority. |
| Buzz custom NIPs and selected standard NIPs | Use only through a versioned OpenAgents allow-list. Define the signer, audience, retention, and authority rules. |
| Buzz self-hosting, relay, Postgres, Redis, MinIO, search, and admin stack | Canceled. Omega does not depend on a Buzz installation. |
| Buzz Tauri, Flutter, web, and admin clients | Rejected. GPUI Omega, OpenAgents mobile, and openagents.com own the product surfaces. |
| Relay-as-workspace and custom-kind dispatch as product policy | Rejected. A signed event is an input or projection, not an accepted command or outcome. |
| Buzz non-streaming agent turn model | Rejected. Omega keeps loss-accounted streaming and durable native histories. |
| Community-pooled model compute | Reconcile with Pylon and NIP-90. Do not create a second compute authority. |

### Bring-your-own agent contract

Omega must not make an agent portable by copying its secret files.
It must make the connection portable through an explicit adapter contract.

The first adapter classes are:

1. an ACP standard-I/O executable, such as `hermes acp`
2. an ACP local or remote endpoint with authenticated transport
3. a native OpenAgents harness
4. a bounded terminal adapter for an agent that has no ACP support
5. an MCP tool provider that is not represented as a full agent

An attached agent record must contain:

- one stable external-agent reference
- adapter type, executable or endpoint identity, and version
- an opaque configuration-home grant
- configuration ownership and mutation policy
- declared and observed capabilities
- project and workroom memberships
- process generation, health, stall, restart, and cancel state
- signer and OpenAgents identity bindings
- audience, retention, and provenance rules
- exact action and outcome receipt references

The default configuration policy is `use_existing_read_only`.
Omega does not run setup, login, model selection, provider changes, skill
installation, memory migration, or configuration writes during attachment.
The user can admit a separate configuration change later.
The adapter receives only the minimum transport and workroom values.
It must not receive a replacement home by default.

The agent keeps its existing provider, model, credentials, skills, memory,
MCP servers, tools, sessions, and local preferences.
Omega shows those facts as agent-owned or externally owned.
It does not claim that it can inspect every private value.
Omega policy still limits which project, command, file, network, publication,
and delivery effects the agent can request.
An external configuration cannot widen OpenAgents authority.

Detaching the agent revokes its Omega capability and workroom membership.
It does not delete or rewrite the agent home.
Restarting Omega must reconnect or show a typed degraded state.
It must not silently mint a replacement agent or identity.

The first Hermes acceptance journey is:

1. Detect the existing `hermes` executable and ACP adapter.
2. Select the existing `HERMES_HOME` without reading secret values.
3. Show the adapter, version, configuration owner, and capability preview.
4. Attach through `hermes acp` without running setup or login.
5. Add Hermes to one Omega workroom and send one bounded task.
6. Stream text, tools, permissions, progress, and the final outcome.
7. Prove that existing Hermes memory, skills, and MCP tools stay available.
8. Prove that attachment and detachment do not change the configured home.
9. Restart Omega and resume or report a typed reconnect failure.
10. Prove that no credential or private configuration value entered a log,
    Sync record, Nostr event, or public receipt.

### Authority boundary

The owning OpenAgents service admits every action through current owner, scope,
generation, and policy checks.
Rust enforces local capabilities.
Node/Effect composes product policy until an admitted Rust port replaces that
exact authority.
Nostr remains a signed projection bus and an admitted input.

## Transcript evidence

This plan uses the last ten numbered OpenAgents transcripts.
Episode 261 is a future script and is not implementation evidence.

| Episode | Digest | Required implication |
| --- | --- | --- |
| 252 | `b7acb3d0ea180a3841ec7434163b53db8cdc7a0d8cb8e0ffcfffb136f6e4de08` | Keep ProductSpec, AssuranceSpec, execution, and receipts separate. |
| 253 | `b0458f03ae53ef5b5ab3a24098bfa70493f9774a62da58d0f928963da6debb74` | Own the fork, pin, provenance, update policy, and reversal seam. |
| 254 | `9b772c544739f926783a6f108f2a249923fa2fbf3edf8c772c4afa1053551664` | Preserve the multi-harness workroom and repair its observed thread and queue failures. |
| 255 | `78624a95626cd24873368b95cae16fa602dae7b0f99731872a27182159d80fd8` | Keep the durable thread as the product object and keep queue, steer, and interrupt distinct. |
| 256 | `22fb9b2a6e31803251277fb552621b11abba7f946b4204054235862aa25c3825` | Preserve the current release target matrix and same-thread harness failover. |
| 257 | `c9caa425edee3ffaf1485f6bcdcc75899e8113873d3dd78815524ec48902708b` | Make file open, OS association, project movement, and editor/workroom mode change acceptance criteria. |
| 258 | `a7b1624950037ac1628f047f2a1cb09355ab25e5c73b1dd814745b9c03a5dd22` | Isolate and bound Git, index, Nostr, and agent workers. |
| 259 | `173b087fde40f53c7d86d8bc29c17ded85f880d20d357a3601d844df91627b26` | Put signed identity, evidence, reputation, and economic coordination in the IDE without relay authority. |
| 260 | `6c413845a8fc9da64153cfcbf96d4042d1360b924050221095b478cc41ae3aed` | Keep Omega in one cross-device company-work record. |
| 261 | `40a8280260d86fa9699e5cadd5165f84e4c64faa028efa4de77951b534dea050` | Use the agent-identity narrative only. Do not use it as technical proof. |

Episode 255 praised the current signed Effect Native Electron shell.
The owner now changes that shell decision.
This plan records a new decision and does not present it as prior transcript
consensus.

## Ordered packet plan

| Packet | Outcome | Dependency | Dispatch state |
| --- | --- | --- | --- |
| OMEGA-00 | Create and admit the separate Omega tracked-fork repository. Freeze an exact Zed pin, upstream remotes, branch policy, license, provenance, source, patch, merge, source-delivery, deletion, and cross-repository receipt records. Prove a clean stock build in that repository. | this plan | not admitted for mutation |
| OMEGA-01 | Write the Omega ProductSpec delta and complete contract crosswalk. Assign each domain to Rust, Node/Effect, or an isolated worker, and assign its source repository and artifact owner. Select the single store and generated protocol source for each shared identity. Define the package export, version, digest, dependency-direction, and conformance rules. Propose repository-law and invariant changes. | OMEGA-00 | not admitted |
| OMEGA-02 | Write the Omega AssuranceSpec delta, common journey, threat model, benchmark, accessibility checks, release checks, process-failure checks, and independent-verifier rules. | OMEGA-01 | not admitted |
| OMEGA-03 | In the OpenAgents monorepo, extract, test, and produce the consumable Node 24 `omega-effectd` service and required TypeScript packages. Keep current Desktop as the first workspace consumer. In the Omega repository, pin and package the immutable service artifact. Preserve exact behavior. | OMEGA-01/02 | not admitted |
| OMEGA-04 | In the Omega repository, add the Rust service supervisor and consume the separate generated `omega-openagents` protocol. In the monorepo, keep the canonical schema, fixtures, and generated-client sources. Add component handshake, local capability, version ranges, refs, generations, hard frame limits, bounded queues, backpressure, cancellation, overload, gap, restart, health, and conformance fixtures. Start read-only. | OMEGA-03 | not admitted |
| OMEGA-05 | In the Omega repository, move admitted local process, native enforcement, credential-custody, update, event-store, or receipt mechanics to Rust in bounded packets. Each port uses differential replay and deletes the old authority path at cutover. Reusable Effect semantics remain monorepo packages until their own admitted cutover. | OMEGA-04 | not admitted |
| OMEGA-06 | In the Omega repository, port the durable workroom, thread, provider, harness, and Full Auto product projections to GPUI. Execute the OMEGA-WR fast path below. Keep portable contracts and services in monorepo packages and each exact authority in its OMEGA-01 owner. | OMEGA-04/05 | not admitted |
| OMEGA-07 | In the Omega repository, join the workroom to the Zed project, editor, Git, language, review, and terminal graph. Prove immediate file open, OS association, project movement, and editor/workroom mode change. | OMEGA-06 | not admitted |
| OMEGA-08 | In the Omega repository, add the Omega-native social and optional Nostr interoperability core. Keep reusable schemas and semantic fixtures in the monorepo. Prove signer isolation, relay gaps, offline replay, identity, reputation, evidence, Git facts, extension-host isolation, and an authority that cannot expand. Do not deploy Buzz. | OMEGA-04/05/06 | not admitted |
| OMEGA-09 | Preserve local and managed placement plus mobile and web control over the same work record. Keep React Native mobile and web independent of Omega source, GPUI, and local state. | OMEGA-06/07 | not admitted |
| OMEGA-10 | Migrate versioned user data without credential export. Quiesce active runs, preserve stable refs, prove export, import, rollback, and N-1 Electron recovery. | OMEGA-06/08/09 | not admitted |
| OMEGA-11 | Build the complete signed target matrix and cross-repository compatibility manifest for Rust, remote-server, Node, Effect, Nostr, extension-host, worker, packages, and protocol artifacts. Bind both repository commits and every artifact digest. Prove target-specific companion discovery, GPL source delivery, update, rollback, recovery, accessibility, performance, and owner acceptance. | OMEGA-00 through OMEGA-10 | not admitted |
| OMEGA-12 | Make Omega the primary release and retire the Electron shell only after the rollback window and final disposition. | OMEGA-11 | owner gate |

Each implementation packet requires a current claim, exact paths, hot contracts,
tests, and final proof.
Run parallel work only on paths that do not overlap.
The shared-contract owner must first freeze the applicable schema.

## Omega workroom fast path

This path puts the workroom in Omega early.
It does not wait for every later Rust migration.
It uses the minimum `omega-effectd` and protocol slice that can preserve one
authority and one durable work record.
Native panes and Omega-only adapters land in the Omega repository.
Portable contracts, Effect services, projections, and conformance fixtures
land as consumable monorepo packages.

| Packet | Outcome | Dependency | Dispatch state |
| --- | --- | --- | --- |
| OMEGA-WR-00 | Freeze the workroom ProductSpec delta, native pane grammar, item identities, and first dogfood journey. | OMEGA-00/01/02 | not admitted |
| OMEGA-WR-01 | Add native GPUI Workrooms, Threads, Agents, and Attention pane types. Keep the first view read-only against real durable records. | OMEGA-03/04 and WR-00 | not admitted |
| OMEGA-WR-02 | Add channel, thread, reply, reaction, presence, read-state, search, and notification projections. | WR-01 | not admitted |
| OMEGA-WR-03 | Add the external-agent adapter contract and the Hermes acceptance journey. Add Codex, Claude, Goose, Grok, Pylon, and other agents through the same registry. | WR-01 and OMEGA-04 | not admitted |
| OMEGA-WR-04 | Join workroom messages to project, file, terminal, changes, diff, review, approval, test, commit, and delivery panes. | WR-01/03 and OMEGA-07 | not admitted |
| OMEGA-WR-05 | Add receipts, blockers, decisions, workflow approval steps, and structural loop prevention. | WR-02/04 | not admitted |
| OMEGA-WR-06 | Add signed identity, owner-readable memory status, and optional Nostr input and projection adapters. | WR-02/03/05 and OMEGA-08 | not admitted |
| OMEGA-WR-07 | Add multi-user membership, moderation, tombstones, Forum projection, and public-safe social views. | WR-02/05/06 | not admitted |
| OMEGA-WR-08 | Move OpenAgents daily coordination into the proven Omega workroom. Keep GitHub as source and claim authority for both repositories until a separate cutover passes. Do not route Omega source through the monorepo. | WR-01 through WR-07 | owner dogfood gate |

The first useful dogfood target is WR-01 through WR-04.
The team must open a workroom and attach Hermes or another existing agent.
The team must discuss one feature and inspect its project and changes.
The team must then review, approve, and see the delivery receipt in Omega.

## Canceled Buzz and forge issue program

The owner cancels the separate issue graph.
The reusable requirements move into the Omega packets above.
Keep landed code unless a later packet proves that no active path uses it.

| Issue | Owner disposition | Preserved Omega requirement |
| --- | --- | --- |
| #9194 | Closed as not planned on 2026-07-23. | One native company work home in Omega. |
| #9195 | Closed as not planned after a claim release on 2026-07-23. | No separate Buzz deployment. Preserve a public-safe retirement inventory. |
| #9196 | Closed as not planned on 2026-07-23. | Stable identity, membership, revocation, and isolated signers. |
| #9197 | Closed as not planned on 2026-07-23. | Native channels, threads, reactions, presence, and public-safe agent posts. |
| #9198 | Closed as not planned on 2026-07-23. | Bounded Sarah and agent read, post, failure, and receipt paths. |
| #9199 | Closed as not planned on 2026-07-23. | NIP-34 and ngit fixtures can remain optional Omega interop tests. |
| #9200 | Closed as not planned on 2026-07-23. | Native claim and work activity panes with one-writer authority. |
| #9201 | Closed as not planned on 2026-07-23. | Re-admit durability, replay, and load proof only for an Omega-owned service. |
| #9202 | Closed as not planned on 2026-07-23. | Reconcile push admission with the Omega Rust Git core. |
| #9203 | Closed as not planned on 2026-07-23. | Native patch, review, approval, merge, and receipt panes. |
| #9204 | Closed as not planned on 2026-07-23. | Keep ngit and ngit-grasp as optional research evidence. |
| #9185 | Closed as not planned on 2026-07-23. | Preserve the landed typed ledger, signer, durable store, and subscription code. Cancel its residual hosted relay program. |

Issue closure is a program disposition.
It does not revert the commits that landed through #9185.
It does not prove the deletion of an external cloud resource.

The GitHub audit at `2026-07-23T23:30:40Z` verified that every issue in the
table had state `CLOSED`, reason `NOT_PLANNED`, and label `wontfix`.
The #9195
[`CLAIM-RELEASE`](https://github.com/OpenAgentsInc/openagents/issues/9195#issuecomment-5064534570)
records the unlanded worktree and the separate cloud-retirement boundary.
The #9194
[`epic closeout`](https://github.com/OpenAgentsInc/openagents/issues/9194#issuecomment-5064540276)
links the issue graph to this plan.

## Product and authority gates

Before OMEGA-03:

- admit the Omega ProductSpec and AssuranceSpec
- reconcile Desktop, Full Auto, IDE, mobile, and release contracts
- revise repository law and invariants through their own review
- create and admit the dedicated Omega repository and its tracked Zed upstream
- define the supported monorepo package and service export surfaces
- prove immutable artifact versions, digests, concrete dependencies,
  provenance, and cross-repository conformance
- define the stable product, bundle, channel, tag, state-root, and migration
  identity

Before OMEGA-08:

- freeze one generated protocol with Effect Schema contract conformance and
  generated Rust and TypeScript codecs
- freeze the Nostr event allow-list
- define OS-backed signer custody and sign-request admission
- prove that the archive, relay, and index cannot use seed or `nsec` material
- define separate purpose, retention, export, deletion, and Sync policy for
  each store
- prove that a user-owned agent can attach without a configuration or secret
  copy
- prove that detach and restart do not change the external agent home

Before OMEGA-11:

- pass the complete Desktop target matrix required by current release authority
- bind Omega, Node 24, `omega-effectd`, and native components to one signed
  compatibility and rollback record
- bind the Omega commit, OpenAgents monorepo commit, package versions and
  digests, generated protocol version, and schema version in each release
  receipt
- supply GPL Corresponding Source, notices, build instructions, and source
  receipts after legal review
- pass the same ProductSpec and AssuranceSpec journey in the current Desktop
  and Omega
- prove that Omega wins on editor quality without a regression in durable
  threads, Full Auto, authority, evidence, recovery, accessibility, or
  cross-device control

## Cutover and rollback

Omega becomes the primary release only when all of these facts are true:

- the accepted ProductSpec and AssuranceSpec name the exact Omega subject
- independent assurance accepts the complete journey
- the owner observes the installed packaged journey
- data import, export, deletion, update, and rollback pass
- the patch and rebase cost stay within the accepted budget
- the release supplies required source and license material
- mobile and web continue the same durable work record
- the current Electron application can read the retained N-1 recovery state

OpenAgents will not delete the current Electron source, release artifacts, or
recovery path during the rollback window.
Old Electron receipts remain Electron evidence.
They do not become Omega evidence.

## Reversal tests

| Risk | Stop or reverse when |
| --- | --- |
| The fork becomes an uncontrolled product | The owned patch or merge conflict rate exceeds the accepted budget. |
| GPL duties do not fit the release | Legal review or source-delivery proof does not accept the release model. |
| Repository dependency reverses | A monorepo package imports Omega client code or requires the Zed fork. |
| Omega copies shared source | Stop when Omega copies TypeScript or Effect service source. |
| Package and protocol versions drift | A build or receipt cannot bind both repository commits and all package, service, generated-protocol, and schema versions. |
| Omega code enters the monorepo | Stop when new Omega-only code lands in the monorepo. |
| A second Desktop grows | Stop when a monorepo client gets new primary Desktop work. |
| Rust and Effect create split product authority | A domain has two writable authoritative stores or can settle differently in Rust and Node. |
| Omega is one fatal domain | An editor, Git, index, Nostr, or agent fault can terminate the complete durable work record. |
| The bridge creates split authority | Electron, Omega, mobile, or web can settle one action differently. |
| Nostr widens authority | A signature, post, relay acknowledgement, or membership can start or settle work without Effect admission. |
| Agent attachment becomes agent takeover | Omega changes an external agent home, login, provider, model, memory, skill, tool, or credential without a separate admitted action. |
| External agents become a policy bypass | An attached agent can request an effect outside the declared capability and OpenAgents policy intersection. |
| Migration proof loses truth | The migration proof does not preserve stable refs, active-run checkpoints, credentials, or N-1 recovery. |
| Accessibility becomes worse | The packaged editor journey fails the accepted accessibility gate. |
| Omega does not improve the product | The complete journey does not materially improve editor and work completion results. |
| Upstream quality degrades | The fork cannot merge a supported Zed update within the accepted time and patch budget. |

## Non-goals

This plan does not:

- authorize product-code mutation
- authorize a public availability, parity, or replacement claim
- authorize release, production enablement, spend, or credential export
- place the Zed fork, Omega GPUI code, or new Omega client implementation in
  the OpenAgents monorepo
- copy monorepo package or Effect-service source into Omega
- move React Native mobile into Omega without a separate accepted client
  migration
- move a product domain to Rust without semantic proof and one-authority cutover
- make a relay the company database or command authority
- import the complete Buzz server, relay, Tauri, or Flutter application
- run a separate OpenAgents Buzz community or forge product
- claim complete Buzz feature or protocol parity
- copy or replace an attached agent's configuration by default
- make mobile or web depend on GPUI or local Omega state
- reopen closed work only because its current host is Electron

## Plan claim

- Actor: `codex-root-omega-plan-20260723`
- Claimed at: `2026-07-23T22:54:48Z`
- Base: `8f047b841d5bb809dae73d31b530cfe27227f740`
- Branch: `codex/omega-zed-primary-plan`
- Worktree:
  `/Users/christopherdavid/work/openagents-worktrees/omega-zed-primary-plan-20260723`
- Scope: Omega decision record, current-plan reconciliation, Fast Follow intent,
  and documentation indexes
- Hot contracts: Sol roadmap, Fast Follow revision, Sol document manifest, and
  STE inventory
- Excluded contracts: ProductSpec, AssuranceSpec, `AGENTS.md`, `INVARIANTS.md`,
  release specifications, and product code
- Verification: Sol document checks, Fast Follow validation, STE checks,
  internal-link checks, and changed-file review

This plan claim ends after the documentation commit lands on `main`.
OMEGA-00 and later packets require new implementation claims.

## Plan revision 2 claim

- Actor: `codex-root-omega-buzz-plan-20260723`
- Claimed at: `2026-07-23T23:15:49Z`
- Base: `db331e6d1770041fc1c25b8e934a70d0ea01faf4`
- Branch: `codex/omega-buzz-native-plan`
- Worktree:
  `/Users/christopherdavid/work/openagents-worktrees/omega-buzz-native-plan-20260723`
- Scope: cancel the separate Buzz program, define complete Omega-native
  workroom scope, add bring-your-own agent requirements, reconcile plans, and
  record issue disposition
- Hot contracts: Sol roadmap, Fast Follow revision, Sol document manifest,
  STE inventory, and issue state
- Excluded contracts: ProductSpec, AssuranceSpec, `AGENTS.md`, `INVARIANTS.md`,
  release specifications, and product code
- Verification: GitHub issue audit, source review, Sol document checks, Fast
  Follow validation, STE checks, internal-link checks, and changed-file review

This claim ends after both documentation commits land on `main`, the canceled
issues have terminal comments, and the task worktrees and branches are
reconciled.

## Plan revision 3 claim

- Actor: `codex-root-omega-repository-boundary-20260723`
- Claimed at: `2026-07-23T23:54:44Z`
- Base: `c348abb746f70d58f240e8f24a7f6f4ce802f239`
- Branch: `codex/omega-repository-boundary`
- Worktree:
  `/Users/christopherdavid/work/openagents-worktrees/omega-repository-boundary`
- Scope: define the separate Omega repository, consumable monorepo package and
  Effect-service boundary, Electron migration exception, and independent mobile
  placement
- Hot contracts: Sol master roadmap, Omega plan, mobile plan, Sol document
  manifest, and STE inventory
- Excluded contracts: ProductSpec, AssuranceSpec, `AGENTS.md`, `INVARIANTS.md`,
  release specifications, package manifests, lockfile, and product code
- Verification: Sol document checks, STE checks, internal-link checks, and
  changed-file review

This claim ends after the documentation commit lands on `main`.
OMEGA-00 and later packets require new implementation claims in the repository
that owns each packet.

## Plan revision 4 claim

- Actor: `codex-root-omega-roadmap-20260723`
- Claimed at: `2026-07-23T23:45:00Z`
- Base: `11816aa41c`
- Branch: `codex/omega-roadmap`
- Worktree:
  `/Users/christopherdavid/work/openagents-worktrees/omega-roadmap`
- Scope: define the accelerated Omega release, Desktop parity, Buzz port,
  migration, and cutover order
- Hot contracts: Sol master roadmap, Omega plan, Sol document manifest, and
  STE inventory
- Excluded contracts: ProductSpec, AssuranceSpec, `AGENTS.md`, `INVARIANTS.md`,
  release specifications, package manifests, lockfile, and product code
- Verification: source audits, Sol document checks, STE checks, internal-link
  checks, and changed-file review

This claim ends after the documentation commit lands on `main`.
Each implementation packet needs a new claim in its owning repository.
