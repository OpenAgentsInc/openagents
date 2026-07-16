# T3 Code and OpenAgents Desktop: full gap analysis — 2026-07-15

## Executive conclusion

T3 Code is currently the broader coding-workbench product. It has five coding
harnesses, project-scoped worktree threads, remote environments, forge flows,
preview automation, desktop support across the major operating systems, and a
real mobile remote-control client. Its React renderer is also substantially
deeper in mature workbench mechanics: virtualized long transcripts, rich
composer nodes, worker-backed diffs, terminal rendering, project trees,
responsive navigation, and a broad settings surface.

OpenAgents Desktop is currently the narrower and more defensible Codex MVP. It
has a hardened Electron boundary, a React renderer governed by an Effect Native
application model, typed host intents, loss-accounted Codex history, explicit
steer/queue semantics, hidden-ref checkpoints with guarded revert, local-first
identity and Sync, release-grade artifact verification, and stronger execution
and receipt direction. It should not erase those advantages in pursuit of
surface breadth.

The largest product gaps are not cosmetic. They are:

1. a long-lived, complete Codex app-server supervisor and lossless native event
   plane;
2. project/thread/worktree lifecycle and parallel execution;
3. Git/forge mutation, terminal, file-tree, diff, preview, and MCP workbench
   depth;
4. remote environment discovery and secure attachment;
5. mobile supervision and attention delivery;
6. cross-platform desktop packaging and automatic update delivery; and
7. additional provider support, if and when the Codex-only launch boundary is
   deliberately widened.

The immediate target should therefore be **Codex workbench completeness**, not
provider-count imitation. T3's provider-neutral architecture is useful design
evidence, but exposing Claude, Cursor, Grok, or OpenCode in the OpenAgents MVP
would widen the product before its one promised provider has complete lifecycle,
event, recovery, and worktree semantics.

**2026-07-16 owner direction:** Agent Client Protocol client support is now an
explicit architecture direction so OpenAgents can control `grok agent stdio`
and other compatible coding agents. This does not by itself reopen visible
multi-provider parity. The
[T3 Code Agent Client Protocol implementation teardown](./2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md)
defines the generated bidirectional client foundation, Grok-first proof, and
capability-gated peer profiles. The scope is client-only.
The canonical event/evidence and reverse-request authority boundary is in the
[Agent Client Protocol runtime bridge ADR](../adr/2026-07-16-agent-client-runtime-bridge.md).

## Scope, snapshots, and method

This document compares implementation, not marketing claims. It reconciles:

- the commit-pinned [T3 Code teardown](./2026-07-13-t3-code-teardown.md),
  including its frontend deep dive;
- the current
  [T3 Code Agent Client Protocol implementation teardown](./2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md);
- the [OpenAgents adaptation analysis](./2026-07-10-openagents-product-adaptation-analysis.md);
- the current [Codex app-server support analysis](./2026-07-15-codex-app-server-client-support-analysis.md);
- the current [OpenAgents Desktop README](../../apps/openagents-desktop/README.md)
  and [guarantee ledger](../../apps/openagents-desktop/GUARANTEES.md); and
- targeted source inspection of both repositories.

The implementation snapshots used for the final reconciliation are:

| Product    | Local source revision                      | Snapshot note                                                                                                                                                     |
| ---------- | ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T3 Code    | `c1ec1915fc16f3dc1ec5d47d9a97f6210a574526` | Local `projects/repos/t3code` `main`; 2026-07-12 Android-support commit. The teardown separately pins the upstream revisions used for its deeper claims.          |
| OpenAgents | `2268392f3501bda8cf2735c69ff784a08c842806` | Local OpenAgents `main` after reconciliation with `origin/main`; includes the generated Codex protocol authority foundation and the latest Desktop startup fixes. |

This is a point-in-time source audit. A type, route, component, or dormant
handler counts as implemented only when the source establishes an executable
product path. A generated protocol declaration is not credited as behavioral
support until the running client decodes, retains, projects, and tests it.

### Status legend

| Mark  | Meaning                                                                   |
| ----- | ------------------------------------------------------------------------- |
| **I** | Implemented in the current OpenAgents Desktop product path                |
| **P** | Partial: a foundation or narrower path exists, but not T3's product depth |
| **M** | Missing from the current OpenAgents Desktop product path                  |
| **D** | Deliberately deferred or excluded from the Codex-only MVP                 |
| **S** | OpenAgents is materially stronger; parity would be a regression           |

## Top-level scorecard

| Capability family           | T3 Code                                                              | OpenAgents Desktop                                                                 | OA status | Material gap                                                                                              |
| --------------------------- | -------------------------------------------------------------------- | ---------------------------------------------------------------------------------- | --------: | --------------------------------------------------------------------------------------------------------- |
| Provider breadth            | Codex, Claude Code, Cursor, Grok, OpenCode                           | Codex-only visible MVP                                                             |     **D** | Four provider adapters, instance management, and provider-neutral UX are absent by policy.                |
| Codex process model         | Persistent provider runtime                                          | One app-server process per active turn                                             |     **P** | No scoped, long-lived supervisor or idle-event/reconnect lifecycle.                                       |
| Codex protocol declarations | Generated pinned protocol package                                    | Generated pinned protocol authority now exists                                     |   **I/P** | The package exists; the production client still uses a narrow manual runtime path.                        |
| Codex behavioral coverage   | Broad subscription, narrow normalization                             | Six client methods, three reverse handlers, eight notifications                    |     **P** | Both are incomplete; OpenAgents remains much farther from the full current protocol.                      |
| Orchestration               | Event-sourced CQRS over SQLite                                       | Typed hosts, ledgers, projections, Sync, but no equivalent whole-product CQRS core |     **P** | No single durable command/event/projection model for every project, thread, provider, and operation.      |
| Project/thread model        | Projects, provider threads, worktrees, checkpoints                   | Workspace + Codex conversations + checkpoints                                      |     **P** | Worktree-per-thread lifecycle and provider-neutral project hierarchy are missing.                         |
| Parallel execution          | Worktree-isolated concurrent threads                                 | Multiple sessions/fleet machinery, no equivalent Desktop worktree lifecycle        |     **P** | Desktop cannot create/manage parallel project worktrees as the primary thread model.                      |
| Git/forge                   | Git mutations and four forge integrations                            | Read-only review plus guarded checkpoint revert                                    |     **P** | Branch, commit, push, PR/MR creation, stack flow, and forge integrations are absent.                      |
| Remote environments         | Environment catalog and several endpoint types                       | Local workspace-first product                                                      |     **M** | No remote environment catalog, attach flow, SSH/Tailscale/tunnel transport, or portable session surface.  |
| Mobile control              | Expo iOS/Android remote workbench                                    | Mobile app exists in monorepo, not a shipped Desktop-control peer                  |     **M** | No equivalent endpoint catalog, thread control, Live Activities, or notification loop.                    |
| Preview/browser automation  | Preview manager, browser, Playwright-style tools, MCP loop           | No equivalent integrated workbench preview loop                                    |     **M** | Visual application work cannot be inspected and controlled inside the Codex workbench.                    |
| Files/editor                | Project tree, file views, search, diffs                              | Workspace file browser/editor/save exists but is de-emphasized in MVP shell        |     **P** | Breadth, navigation, search sessions, and integrated worktree context trail T3.                           |
| Terminal                    | Rich terminal/process surfaces                                       | Typed terminal host/workspace foundations                                          |     **P** | No equivalent xterm-grade, persistent, project/thread terminal product.                                   |
| Timeline                    | Virtualized rich activity/message renderer                           | Typed React timeline with MessageScroller and activity grouping                    |     **P** | Native-event fidelity, virtualization depth, minimap, cache discipline, and item coverage are incomplete. |
| Composer                    | Lexical rich composer and contextual nodes                           | Focused textarea composer, images, steer/queue                                     |   **P/S** | T3 is richer; OpenAgents has clearer explicit concurrency semantics.                                      |
| Renderer architecture       | React 19 + Vite + Effect Atom + Zustand + Base UI/shadcn             | React 19 + Vite + Tailwind/shadcn under Effect Native state/intent authority       |   **I/S** | No need to copy T3's duplicate state topology; component/workbench depth remains the gap.                 |
| Themes/responsiveness       | Light/dark/system, mobile/responsive variants                        | Khala dark desktop theme                                                           |     **P** | Light/system themes and broader responsive behavior are absent.                                           |
| Desktop platforms           | macOS, Windows, Linux; WSL/SSH bridge                                | Release lane centered on signed/notarized macOS arm64                              |     **P** | Windows, Linux, x64, WSL, and equivalent packaging/update proof are missing.                              |
| Updates                     | Stable/nightly application and provider update flows                 | Strong staged/rollback receipts; delivery feed is not guaranteed                   |   **P/S** | OA has safer application semantics but lacks comparable automatic distribution/channel breadth.           |
| Onboarding                  | `npx t3@latest` bootstrap                                            | Conventional install/dev/release paths                                             |     **M** | No equally simple product bootstrap or remote-host onboarding flow.                                       |
| Execution safety            | Defaults to unrestricted execution; weak user receipts               | Typed authority direction, closed IPC, explicit approvals/receipts                 |     **S** | T3 behavior must not be copied.                                                                           |
| Release trust               | Broad artifacts; audited macOS outer artifact had Gatekeeper failure | Fail-closed signed, notarized, stapled artifact and rollback receipt contract      |     **S** | Preserve OpenAgents' stronger gate while expanding platform coverage.                                     |

## 1. Provider and Codex runtime gaps

### 1.1 Provider abstraction

T3 implements four transport families behind five visible harnesses:

- Codex through app-server;
- Claude Code through its SDK;
- Cursor and Grok through Agent Client Protocol adapters; and
- OpenCode through its HTTP server.

It also models provider instances independently, allowing isolated homes,
authentication state, version state, and availability. OpenAgents has related
account-ledger and fleet concepts, but the Desktop MVP intentionally exposes
only Codex. Missing product work includes:

- a provider-neutral session identity and event vocabulary that does not erase
  native provider semantics;
- adapter lifecycle, install, authentication, health, update, and recovery
  contracts per provider;
- multiple visible instances of one provider without credential collision;
- provider choice in new-session, project, settings, and command surfaces; and
- provider-specific capability negotiation rather than lowest-common-denominator
  UI.

**Disposition:** **D** for the MVP. Preserve extension seams, but do not add
non-Codex UI until Codex lifecycle completeness is certified.

### 1.2 Long-lived Codex app-server

T3 gives a provider session a durable runtime owner. OpenAgents currently
starts one app-server process for an active turn. That prevents honest support
for idle notifications, reconnect repair, account-change events, background
compaction, server-owned thread state, and several remote-control behaviors.

OpenAgents needs a scoped supervisor keyed by runtime identity, at minimum:

- executable identity and version/hash;
- `CODEX_HOME` / account identity;
- workspace and environment identity;
- one request-id allocator and pending request registry;
- complete reverse-request dispatch installed before initialization;
- cancellation, timeout, crash, backoff, and restart-repair state;
- notification subscriptions that remain alive between turns; and
- bounded teardown when no window owns or observes the runtime.

This is the single most important architectural gap.

### 1.3 Protocol breadth and fidelity

OpenAgents now owns a generated protocol package and checked-in manifests for
the bundled `0.144.1` executable and a current-source comparison. That closes
the declaration/authority foundation previously identified as missing. It does
not yet close runtime behavior.

The current production path still calls only:

- `initialize`;
- `thread/start` and `thread/resume`; and
- `turn/start`, `turn/steer`, and `turn/interrupt`.

It handles command approval, file approval, and user input, recognizes eight
notification names, and projects those into a narrower product timeline. The
full audited current-source denominator remains 126 client requests, one client
notification, 11 reverse server requests, and 72 server notifications. The
missing families include:

- initialization result retention and capability negotiation;
- complete thread read/list/fork/archive/delete/settings/metadata lifecycle;
- model, feature, permission, account, login, quota, and configuration state;
- skills, hooks, plugins, apps, marketplace, and MCP state;
- review and structured diff lifecycle;
- filesystem, command/process, search, import, and platform services;
- environment, terminal, remote runtime, memory, and realtime families; and
- lossless retention of unknown or currently unpresented native events.

T3 is not a complete behavioral gold standard here. It invokes only a small
fraction of its generated request surface, handles three reverse requests, and
normalizes away many native notification semantics. The correct target is the
shipped Codex binary contract—not T3's omissions.

### 1.4 Native event plane

T3 broadly subscribes to generated notifications but discards many details
during normalization. OpenAgents projects even earlier. OpenAgents needs two
planes:

1. a lossless, versioned, native Codex event journal for replay, diagnostics,
   repair, and future UI; and
2. a portable product projection for messages, activities, plans, approvals,
   usage, and attention.

Every native event needs stable request/thread/turn/item correlation,
observed-at ordering, decode status, retention policy, and an explicit
presentation disposition. Unknown fields must survive upgrades.

## 2. Orchestration, persistence, and recovery gaps

T3's central server uses commands, events, reactors, and SQLite projections as
one orchestration core. Web, Electron, and mobile consume projections rather
than owning provider truth. OpenAgents has several strong durable subsystems—
Khala Sync, conversation ledgers, runtime gateway subscriptions, history
catalogs, account projections, checkpoints, and release receipts—but not one
equivalent workbench-wide event model.

Missing or fragmented concerns include:

- durable project/thread/worktree/provider aggregates;
- command admission followed by explicit accepted/rejected/completed events;
- restart repair across app-server, worktree, terminal, preview, and Git state;
- a single causal timeline spanning local UI intent and provider-native events;
- replayable projections for every window and future mobile peer; and
- durable ownership of queued follow-ups.

OpenAgents should not blindly recreate T3's internal completion receipts as
user-visible authority. It should combine event sourcing with its stronger
admission, approval, evidence, and externally verifiable receipt distinctions.

## 3. Projects, worktrees, and parallel threads

T3 treats a coding thread as a project-bound worktree. It can create parallel
threads without making them mutate one shared checkout, maintain hidden-ref
checkpoints, compare or revert changes, and carry project/Git context through
the UI.

OpenAgents has workspace selection, conversation continuity, hidden-ref turn
checkpoints, staged diff inspection, and guarded revert. It does not yet expose
the complete lifecycle:

- create a thread in the current checkout or a new worktree;
- choose base branch and collision-free worktree path;
- show branch/worktree ownership in every relevant surface;
- discover, attach, detach, archive, and clean stale worktrees safely;
- prevent two sessions from claiming the same mutable worktree accidentally;
- transfer or resume thread ownership after restart;
- reconcile provider thread identity with Git/worktree identity; and
- supervise several active threads with attention, progress, and resource
  state.

This is the second major architecture gap after the provider supervisor. Fleet
and multi-session machinery does not substitute for a user-legible worktree
contract in Desktop.

## 4. Git, review, and forge gaps

T3 implements hidden-ref checkpoints plus mutation flows across GitHub,
GitLab, Bitbucket, and Azure DevOps. It includes branch, commit, push, PR/MR,
comparison, stacked-flow, and generated-description paths.

OpenAgents deliberately reduced its visible MVP surface. It currently has
read-only repository review foundations and checkpoint revert, while recent UI
direction removes the intrusive repository-review panel from the core chat
flow. The missing workbench capabilities are:

- compact, contextual file-change summaries attached to turns;
- worker-backed unified and split diffs for large changes;
- per-file and per-hunk stage/discard with explicit authority;
- branch creation/switching and upstream state;
- commit composition and verified author/repository target;
- push and compare-branch flow;
- PR/MR creation, draft status, title/body generation, and link receipts;
- merge-conflict and dirty-worktree recovery; and
- forge authentication and capability differences.

These actions must land behind typed intents and authority receipts. Copying
T3's breadth without OpenAgents' approval boundary would be a safety
regression.

## 5. Remote environment and access gaps

T3 has first-class `ExecutionEnvironment`, `KnownEnvironment`,
`AccessEndpoint`, and `AdvertisedEndpoint` concepts. It supports direct WS/WSS,
Cloudflare tunnel, Tailscale, and SSH launch/forward paths, with a deliberately
thin cloud relay and environment-local sessions.

OpenAgents Desktop currently defaults correctly to the directory from which it
was opened, but it remains a local-first single-host workbench. Missing product
capabilities include:

- a saved environment catalog with health and last-seen state;
- endpoint discovery, ranking, and explicit trust;
- secure pairing and key rotation;
- SSH launch and port-forward lifecycle;
- Tailnet endpoint discovery;
- reconnect, endpoint failover, and stale-session repair;
- capability-scoped environment credentials;
- remote filesystem, terminal, Git, preview, and provider transport; and
- session portability that distinguishes attaching to a remote runtime from
  copying a transcript.

T3's DPoP and scoped token-exchange design is useful evidence. OpenAgents
should adapt the capability-scoped shape while retaining its own identity,
broker, policy, and receipt authority.

## 6. Mobile and attention gaps

T3 ships a real Expo iOS/Android client over the shared projection protocol. It
has secure endpoint storage, thread lists, composer, markdown, diff, terminal,
approvals, stop/steer controls, push notifications, APNs, and Live Activities.

OpenAgents has a mobile application home in the monorepo, but there is no
equivalent shipped Desktop-control product demonstrated by the audited source.
The gap includes:

- paired host directory and connection health;
- environment/project/thread browsing;
- live transcript and activity projection;
- steer, queue, interrupt, approve, and answer flows;
- attention inbox and deep links;
- background/push notification policy;
- Live Activities or equivalent ongoing-turn status;
- mobile-safe diff and terminal viewers; and
- reconnect/replay behavior with no lost decisions.

This depends on the long-lived supervisor, durable projections, scoped remote
access, and a complete decision inbox. Building the mobile UI first would only
duplicate an unstable host contract.

## 7. Workbench tool gaps

### 7.1 File tree, editor, and search

OpenAgents can browse, edit, and save workspace files and has typed workspace
search foundations. T3 provides a more integrated project tree and contextual
workbench. Remaining gaps include large-tree virtualization, fuzzy navigation,
search-session persistence, recent files, worktree-aware paths, richer syntax
views, conflict states, binary/large-file policy, and tighter change-summary
integration.

### 7.2 Terminal and process sessions

T3 uses xterm-grade terminal components and provider/project process
integration. OpenAgents has host-side terminal primitives but not an equivalent
visible persistent terminal product. Needed work includes PTY session identity,
resize, reconnect, scrollback bounds, process exit state, multiple terminals,
worktree binding, safe link handling, and mobile/read-only projection.

### 7.3 Preview and browser automation

T3 can launch previews, manage ports, render a browser surface, and expose
automation tools back to the coding harness through MCP. OpenAgents lacks this
closed visual-development loop. Needed work includes process/port discovery,
preview health, embedded browser isolation, navigation and screenshot tools,
console/network evidence, Playwright-style actions, permission gates, and MCP
registration scoped to the active project.

### 7.4 Skills, commands, hooks, plugins, apps, and MCP

T3 surfaces provider skills and slash commands and has MCP integration for
preview tools. Current Codex app-server exposes much broader ecosystem
families than OpenAgents presents. OpenAgents needs a Codex-only inventory and
policy surface first: discover, inspect, enable/disable, configure, and explain
skills/hooks/plugins/apps/MCP servers without leaking secrets or silently
widening authority.

## 8. Frontend architecture and state comparison

### 8.1 What T3 implements

T3's web/Electron renderer uses:

- React 19 and React DOM under Vite;
- TanStack Router with browser and hash histories;
- Effect Atom React for shared server/projection state;
- Zustand for local persisted UI state;
- Base UI plus local shadcn/COSS primitives;
- Lexical for the composer;
- Legend List for virtualization;
- `react-markdown`, Shiki, and sanitization for rich content;
- Pierre diff/tree components with workers and caches;
- xterm for terminals;
- dnd-kit and AutoAnimate for interaction polish; and
- Tailwind 4, light/dark/system themes, container queries, pointer/media
  variants, and safe-area behavior.

### 8.2 What OpenAgents implements

OpenAgents now also uses React 19, Vite, Tailwind, and local shadcn components,
but keeps application state, intents, and component grammar under Effect Native
ownership. That is not a missing-React gap. It is an intentional architecture:
React is the renderer host; Effect Native remains the application authority.

OpenAgents should not add TanStack Router, Zustand, or Effect Atom merely to
match dependency names. The actual gaps are mature components and hot-path
behavior:

- virtualized timeline and project lists with stable anchoring;
- worker-backed diff parsing and syntax highlighting;
- bounded markdown/code caches;
- richer contextual composer nodes;
- terminal rendering;
- file-tree performance;
- overlay, menu, tooltip, and keyboard consistency;
- responsive/narrow-window layout; and
- light/system themes if they become product requirements.

T3 also carries costs that OpenAgents should avoid: very large components,
some accessibility gaps, and duplicated web/mobile component and token systems.

## 9. Chat timeline, activity, and composer gaps

Recent OpenAgents work substantially closes the visible shell gap: message
scroll anchoring, centered composer, explicit activities, expandable work
groups, image attachments, focus on new chat, stable sidebar rows, command
navigation, and explicit steer/queue controls exist or are under executable UX
contracts.

The remaining functional gaps are:

- complete Codex item and notification rendering rather than eight manually
  recognized event names;
- lossless replay after reconnect or app restart;
- virtualization for very large histories without losing anchor position;
- bounded rich-text and syntax-highlight caches;
- compact tool-call lifecycle that updates one row rather than emitting
  duplicate status lines;
- rich diff, terminal, file, image, plan, review, and decision cards;
- a minimap or equivalent navigation for long agent turns;
- durable follow-up queue state across restart;
- correct arbitration when several windows observe or answer one request;
- contextual mentions for files, folders, skills, commands, and terminals;
- accessibility coverage for keyboard, screen reader, reduced motion, and
  high contrast; and
- performance budgets backed by real large-thread and slow-host fixtures.

OpenAgents is stronger than T3 in one key interaction: queueing and steering
are explicit concepts. T3 supports draft-ahead and may implicitly steer Codex,
but its keyboard path exposes a second-send seam and it lacks a durable
user-visible follow-up queue. OpenAgents must finish durability and arbitration,
not replace its clearer model with T3's ambiguity.

## 10. Navigation, settings, and onboarding gaps

T3 exposes projects, threads, settings, provider management, source control,
connections, archive, keybindings, and search through a mature shell.
OpenAgents intentionally enforces a small visible MVP allowlist: New session,
Chat, Project home, and Settings, with files/review reachable through bounded
commands rather than permanent primary navigation.

Missing or partial capabilities include:

- a complete settings information architecture;
- keybinding discovery and conflict handling;
- Codex account/login/model/configuration management;
- remote connections;
- source-control and archive settings;
- feature/capability disclosure derived from the supported protocol manifest;
- update channels and automatic-delivery state;
- diagnostics export with redaction; and
- a bootstrap as simple as T3's `npx t3@latest` path.

The visible shell should grow only as capabilities become reliable. Empty or
nonfunctional navigation copied for symmetry would violate the MVP contract.

## 11. Desktop host, platform, and release gaps

### 11.1 Platform coverage

T3 packages macOS arm64/x64, Windows, and Linux, and contains WSL and SSH bridge
support. OpenAgents' demonstrated release lane is presently macOS arm64. Gaps:

- macOS x64/universal validation;
- Windows packaging, signing, install, protocol registration, and updates;
- Linux packages and desktop integration;
- WSL runtime discovery and path translation;
- platform-specific terminal/process behavior; and
- a platform matrix with real install/update/rollback receipts.

### 11.2 Application updates

T3 has stable/nightly application channels and provider maintenance notices.
OpenAgents has a stronger staged update/launch/rollback state machine and
one-click bundled Codex maintenance, but automatic application update delivery
and live feed wiring are explicitly not guaranteed. The missing breadth is
channel metadata, background availability checks, resumable download,
release-note presentation, rollout policy, and equivalent proof on every
platform.

### 11.3 Release integrity

There is no parity gap to close by lowering standards. The audited T3 macOS DMG
could be published while Gatekeeper rejected the outer artifact. OpenAgents
requires the final DMG itself to be signed/notarized/stapled, refuses unsigned
fallback, uses structured names such as `OpenAgents-0.1.2-arm64.dmg`, and
records update/rollback state. Preserve this advantage.

## 12. Security, authority, and privacy comparison

T3 has good local-secret handling, environment-scoped access, DPoP, and a thin
cloud. Its default Codex execution posture is nevertheless
`danger-full-access` with approval disabled, and its internal receipts are not
the user-facing authority/evidence chain OpenAgents requires.

OpenAgents must preserve:

- closed typed IPC and no renderer secrets;
- explicit workspace and runtime authority;
- typed approvals, questions, plans, interruptions, and receipts;
- capability-specific remote credentials;
- safeStorage-backed local custody and local-first identity;
- checkpoint/revert safeguards;
- private-data redaction and reveal-on-intent behavior; and
- fail-closed release/update semantics.

Any T3-derived Git, remote, terminal, preview, or provider feature must enter
through those boundaries. Feature breadth is not permission to create an
ambient privileged renderer.

## 13. Master missing-capability ledger

This table is the consolidated list of meaningful T3 capabilities that are
absent or materially incomplete in OpenAgents Desktop.

| Priority | Capability                                                         | OA state | Dependency / disposition                                                   |
| -------: | ------------------------------------------------------------------ | -------- | -------------------------------------------------------------------------- |
|       P0 | Long-lived Codex app-server supervisor                             | **P**    | Protocol manifest exists; replace per-turn process ownership.              |
|       P0 | Generated runtime decoding of the bundled Codex protocol           | **P**    | Integrate the generated authority package into production transport.       |
|       P0 | Lossless native Codex event journal                                | **M**    | Required before broad projection/UI work.                                  |
|       P0 | Complete reverse-RPC inbox and arbiter                             | **P**    | Own all methods, deny safely where UI is deferred.                         |
|       P0 | Restart/reconnect repair for thread, turn, request, and item state | **P**    | Depends on supervisor and event journal.                                   |
|       P0 | Durable queue/steer ownership                                      | **P**    | Preserve explicit semantics; persist and arbitrate them.                   |
|       P0 | Full Codex lifecycle/account/model/configuration surface           | **P**    | Codex-only scope; derive disclosure from capability manifest.              |
|       P0 | Complete timeline item fidelity                                    | **P**    | Consume native events; do not add one-off string switches.                 |
|       P0 | Large-thread virtualization and bounded render caches              | **P**    | Extend current fast-start and MessageScroller contracts.                   |
|       P1 | Project/thread/worktree aggregate                                  | **P**    | Establish stable IDs and durable lifecycle.                                |
|       P1 | Create/manage worktree per parallel thread                         | **M**    | Requires Git authority and collision-safe cleanup.                         |
|       P1 | Worktree/branch ownership visible throughout UI                    | **M**    | Depends on aggregate.                                                      |
|       P1 | Rich file tree, fuzzy navigation, and search sessions              | **P**    | Build on existing workspace host.                                          |
|       P1 | Persistent xterm-grade terminal sessions                           | **P**    | PTY identity, resize, replay, authority.                                   |
|       P1 | Worker-backed rich diff engine                                     | **P**    | Keep review contextual, not an intrusive bottom panel.                     |
|       P1 | Stage/discard/commit/branch/push workflows                         | **M**    | Typed intents, approvals, receipts.                                        |
|       P1 | PR/MR preparation and creation                                     | **M**    | Starts with GitHub if product priority warrants; do not imply four forges. |
|       P1 | Embedded preview and port/process manager                          | **M**    | Requires process supervision and isolation.                                |
|       P1 | Browser automation tools fed to Codex through MCP                  | **M**    | Requires preview, permissions, evidence capture.                           |
|       P1 | Contextual composer nodes for files/skills/commands                | **P**    | Prefer bounded progressive enhancement over editor rewrite.                |
|       P2 | Remote environment catalog                                         | **M**    | Stable runtime/environment identities first.                               |
|       P2 | SSH launch/forward transport                                       | **M**    | Scoped credentials and reconnect repair.                                   |
|       P2 | Tailnet endpoint discovery                                         | **M**    | Explicit trust and endpoint ranking.                                       |
|       P2 | Capability-scoped token exchange/DPoP                              | **M**    | Adapt shape, retain OpenAgents identity/policy authority.                  |
|       P2 | Remote filesystem/Git/terminal/preview/provider transport          | **M**    | Builds on all remote foundations.                                          |
|       P2 | Mobile paired-host directory and live workbench                    | **M**    | Shared durable projections and access protocol first.                      |
|       P2 | Mobile approval/steer/queue/interrupt                              | **M**    | Complete decision inbox and arbitration first.                             |
|       P2 | Push notifications and Live Activities                             | **M**    | Attention policy and replay-safe deep links.                               |
|       P2 | Automatic application update feed/channels                         | **P**    | Preserve fail-closed staging and rollback semantics.                       |
|       P2 | macOS x64, Windows, Linux, WSL support                             | **M**    | Per-platform release/install/update proof.                                 |
|       P2 | One-command product/host onboarding                                | **M**    | Must not bypass signing, identity, or environment trust.                   |
|       P3 | Skills/hooks/plugins/apps/marketplace management                   | **M/P**  | Codex ecosystem only first.                                                |
|       P3 | Provider-neutral domain projection                                 | **P**    | Keep native plane; avoid lowest-common-denominator loss.                   |
|       P3 | Claude Code adapter and product UI                                 | **D**    | Post-MVP product decision.                                                 |
|       P3 | Cursor adapter and product UI                                      | **D**    | Post-MVP product decision.                                                 |
|       P3 | Grok adapter and product UI                                        | **D**    | Post-MVP product decision.                                                 |
|       P3 | OpenCode adapter and product UI                                    | **D**    | Post-MVP product decision.                                                 |
|       P3 | Multiple instances of each provider                                | **D/P**  | Account/fleet foundations exist; visible multi-provider product does not.  |
|       P3 | GitLab, Bitbucket, Azure DevOps integrations                       | **M**    | Only after one forge path and Git authority are proven.                    |
|       P3 | Light/system themes and broad responsive variants                  | **M/P**  | Product-quality enhancement, not core runtime blocker.                     |

## 14. OpenAgents advantages that must not regress

A gap analysis that counts only T3 features would produce the wrong plan.
OpenAgents already has several stronger contracts:

| OpenAgents advantage                                                 | Why it matters                                                                 |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Effect Native owns application state, intents, and component grammar | React/shadcn can evolve without creating a second application authority.       |
| Closed typed Electron IPC                                            | Renderer compromise does not become ambient host privilege.                    |
| Explicit approval/question/plan/interrupt surfaces                   | Decisions are first-class rather than generic transcript text.                 |
| Explicit steer versus queue semantics                                | User intent is clearer than T3's draft-ahead/implicit steering seam.           |
| Loss-accounted Codex history catalog                                 | Missing, compressed, paged, and subagent history has explicit accounting.      |
| Hidden-ref checkpoint with staged safe revert                        | Revert is guarded rather than a casual destructive action.                     |
| Local-first identity and optional Sync link                          | Local product use is not forced through a cloud account.                       |
| Runtime gateway cursors and recovery direction                       | Multi-surface state can be made replayable rather than live-stream-only.       |
| Fail-closed signed/notarized/stapled release artifact                | The downloadable outer artifact, not just the inner app, is release authority. |
| Update launch and rollback receipts                                  | Applying bytes is not falsely equated with a successful upgrade.               |
| Typed authority/evidence/receipt distinctions                        | Internal completion events cannot silently become user or release authority.   |

## 15. Ordered closure plan

### Phase A — complete one provider

1. Put the generated bundled-version protocol authority in the production
   transport.
2. Replace per-turn app-server processes with the scoped supervisor.
3. Retain every native event losslessly and build portable projections from it.
4. Complete reverse-request arbitration and safe default handlers.
5. Move account, model, configuration, thread, turn, item, review, and queue
   lifecycle onto that runtime.
6. Certify fast startup, huge history, reconnect, restart, and multi-window
   behavior.

### Phase B — complete the local coding workbench

1. Make project/thread/worktree identity durable.
2. Add worktree creation, attachment, ownership, archive, and safe cleanup.
3. Deepen file tree, search, terminal, diff, and contextual review.
4. Add bounded Git mutations and one forge path behind typed authority.
5. Add preview/browser automation and scoped MCP tools.

### Phase C — make it portable

1. Add environment and endpoint catalogs.
2. Add scoped pairing, SSH/Tailnet transports, reconnect, and repair.
3. Project the same durable state into OpenAgents mobile.
4. Add attention delivery, approvals, steer/queue, and Live Activities.

### Phase D — widen distribution and providers

1. Prove signed install/update/rollback on macOS x64, Windows, and Linux.
2. Add automatic delivery channels without weakening fail-closed release gates.
3. Land the generated, bounded, bidirectional Agent Client Protocol client and
   prove `grok agent stdio` without claiming visible provider parity.
4. Decide whether individual ACP or non-ACP providers serve the product;
   implement them one at a time against the same native-plus-portable event
   architecture.
5. Add more forges only after the first mutation path is reliable.

## 16. Definition of closure

“Implemented” should mean all of the following, not merely a visible control:

- **Known:** the exact supported protocol/capability is in a versioned manifest.
- **Decoded:** external inputs and outputs cross generated or reviewed schemas.
- **Owned:** one service owns lifecycle, cancellation, repair, and teardown.
- **Retained:** native state needed for replay or future projection is not lost.
- **Projected:** UI state is derived consistently across windows and peers.
- **Presented:** the interaction is understandable, accessible, and responsive.
- **Authorized:** privileged actions pass explicit policy and approval gates.
- **Recovered:** restart, reconnect, crash, and partial completion are tested.
- **Fast:** startup and hot paths satisfy measured budgets on large real data.
- **Receipted:** completion, evidence, and release authority are not conflated.
- **Shipped:** the installed artifact and update path are proven on the claimed
  platform.

By that definition, T3 itself is not the finish line. It is the strongest
available implementation reference for breadth and workbench mechanics.
OpenAgents' correct endpoint is T3's useful product breadth combined with a
complete Codex protocol owner, Effect Native application authority, safer
execution, durable recovery, and release-grade evidence.

## Primary source map

### T3 Code

- [T3 Code teardown](./2026-07-13-t3-code-teardown.md)
- [T3 Code Agent Client Protocol implementation teardown](./2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md)
- `projects/repos/t3code/apps/server/src`
- `projects/repos/t3code/apps/web/src`
- `projects/repos/t3code/apps/desktop/src`
- `projects/repos/t3code/apps/mobile`
- `projects/repos/t3code/packages`

### OpenAgents

- [OpenAgents adaptation analysis](./2026-07-10-openagents-product-adaptation-analysis.md)
- [Codex app-server support analysis](./2026-07-15-codex-app-server-client-support-analysis.md)
- [OpenAgents Desktop README](../../apps/openagents-desktop/README.md)
- [OpenAgents Desktop guarantees](../../apps/openagents-desktop/GUARANTEES.md)
- `apps/openagents-desktop/src`
- `apps/openagents-desktop/src/renderer`
- `packages/codex-app-server-protocol`
- `apps/openagents.com/packages/effect-native-*`
