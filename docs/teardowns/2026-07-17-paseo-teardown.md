# Paseo Teardown — 2026-07-17

Read-only architecture and product audit of the public `getpaseo/paseo`
source tree at an exact, freshly fast-forwarded commit. The audit did not build
Paseo, launch its daemon or clients, connect an agent provider, pair a remote
device, enroll a Hub, or inspect any local Paseo state.

## TL;DR

Paseo is a self-hosted, multi-provider coding-agent control surface built around
**one persistent Node daemon and many clients**. Claude Code, Codex, Copilot,
OpenCode, Pi, OMP, and generic ACP agents run on the user's machine while an
Expo application supplies mobile, web, and Electron interfaces; a CLI and a
typed client SDK speak the same WebSocket protocol. Direct connections and an
optional end-to-end encrypted relay let those clients reconnect without making
the renderer the execution authority. [source]

```text
Expo mobile / web / Electron / CLI
                 |
        typed JSON + binary WebSocket
                 |
          persistent Paseo daemon
      +----------+-----------+----------+
      |          |           |          |
   agents    timelines     PTYs      workspaces
 direct/ACP append-only   binary     Git/worktrees
      |          |           |          |
      +------- files and local JSON state --------+
                 |
        direct LAN or E2EE relay
                 |
          optional Paseo Hub
       least-privileged execution link
```

Paseo's best work is in the contracts around that shape:

- live `agent_stream` events provide immediacy while paged authoritative
  history provides correctness, with epochs and sequence cursors preventing a
  reconnect or projection from silently dropping committed rows;
- managed Paseo subagents and provider-owned child sessions remain distinct,
  while parentage, detach, archive, tab close, and workspace activity have
  explicit semantics;
- cancellation does not claim success until the provider acknowledges it or a
  terminal provider event arrives;
- protocol schemas, binary terminal frames, app-level liveness, and default RPC
  timeouts are separate concerns rather than one overloaded connection state;
- terminal output uses worker isolation, leading/trailing coalescing,
  revision-aware replay, and backpressure-gated snapshots;
- agent tools have a transport-neutral catalog with native injection when a
  provider supports it and MCP fallback otherwise; and
- the new Hub relationship grants only `hub.execution.*`, persists an
  execution identifier before acknowledgement, and treats duplicate creates
  as idempotent. [source] [test]

The source is equally candid about its limits:

- a locally reachable daemon is a trusted operator endpoint. With no password,
  network reachability is authority; with a password, every authenticated
  client still receives broad operator power. [source]
- agents run as the daemon's OS user and file preview can read any regular file
  that user can read. Docker can move the boundary, but every mounted secret
  and workspace remains available inside it. [source]
- relay encryption has fresh session keys but no replay protection within a
  live session. [source]
- persistence is mostly file-backed JSON without a general schema migration
  framework; some stores still write directly, and interrupted loops are
  demoted from `running` to `stopped` rather than recovered from a durable
  lease. [source]
- Hub stream frames are transient, and restart closes an interrupted active
  turn rather than replaying the prompt. [source]
- the public service proxy deliberately does not apply the daemon password to
  proxied workspace services. [source]

The central OpenAgents decision is: **adapt Paseo's one-daemon/many-client
shape, timeline delivery law, explicit subagent semantics, cancellation
acknowledgement, terminal pipeline, transport-neutral tools, and scoped
idempotent Hub execution; reject reachability or a shared password as
authority, host-user execution as containment, transient streams as recovery,
file JSON as execution truth, unauthenticated public service projection, and
workflow automation as release proof.**

## 1. Snapshot, provenance, and limitations

### 1.1 Exact source identity

Before inspection, the local reference clone was clean, on `main`, and equal to
`origin/main`. A fast-forward-only pull confirmed the current tip below.

| Artifact          | Identity                                                                                          | What it establishes                       |
| ----------------- | ------------------------------------------------------------------------------------------------- | ----------------------------------------- |
| Public repository | `https://github.com/getpaseo/paseo`                                                               | Public source and history                 |
| Audited commit    | `a1de743ef67dde4fe7c48d045a3714f65dfa5e90` on `main`                                              | Exact snapshot used here                  |
| Commit time       | `2026-07-18T03:07:44+08:00`                                                                       | Freshness of the audited tip              |
| Commit subject    | `fix(app): align thinking section scroll layout with other detail sections (#1884)`               | Latest audited change                     |
| Product version   | `0.2.0-beta.1`                                                                                    | Root and package release line             |
| License           | GNU Affero General Public License v3 or later                                                     | Strong source-reuse boundary              |
| Source scale      | 3,176 tracked files; about 694,729 tracked TypeScript/TSX lines across 2,730 TypeScript/TSX files | Approximate implementation scale          |
| Test surface      | 1,014 tracked `test` or `spec` files                                                              | Broad executable evidence surface         |
| Recent history    | 69 commits since `2026-07-15T00:00:00Z`                                                           | Material activity in the requested window |

Recent work included Hub enrollment, independent project identity for every
added folder, lower-volume workspace and timeline synchronization, pluggable
GitHub/GitLab/Gitea/Forgejo forge support, ACP foreground-agent lifetime fixes,
archived-workspace recovery, terminal resize repairs, and desktop package and
rollout hardening. The Hub change alone added thousands of lines of protocol,
relationship, retry, execution, ownership, and WebSocket tests. [history]

### 1.2 Evidence labels

- **`[source]`** — tracked source, docs, manifests, or workflows at the commit;
- **`[schema]`** — a Zod, TypeScript, ACP, WebSocket, persistence, or client
  contract;
- **`[test]`** — a tracked executable test or CI check;
- **`[history]`** — Git history at or before the audited commit;
- **`[inferred]`** — reasoned from several observations; and
- **`[limitation]`** — a source-only audit boundary.

There are intentionally no `[runtime]` observations in this document.

### 1.3 Limits

Source cannot prove provider behavior, relay confidentiality in production,
Hub deployment policy, mobile background delivery, store signing, desktop
updates, terminal latency on real devices, daemon recovery under host failure,
or whether public services are deployed safely. The tests described below were
inspected but not run in the read-only upstream clone. [limitation]

## 2. Product and repository shape

Paseo presents one interface for several coding agents, keeps their processes
alive when a client closes, and supports concurrent workspaces and split panes.
Projects are user-added folders; workspaces are durable placements that may use
the project checkout or a Git worktree. Clients can reconnect locally or from a
phone, and the daemon remains the lifecycle and timeline source of truth.
[source]

| Package               | Primary role                                                           |
| --------------------- | ---------------------------------------------------------------------- |
| `@getpaseo/protocol`  | Zod-authored wire schemas, client capabilities, binary terminal frames |
| `@getpaseo/client`    | Typed daemon client and connection facade                              |
| `@getpaseo/server`    | Daemon, providers, agents, storage, Git, terminals, relay, Hub         |
| `@getpaseo/app`       | Shared Expo/React Native mobile and web application                    |
| `@getpaseo/desktop`   | Electron host and managed-daemon distribution                          |
| `@getpaseo/cli`       | Run, list, attach, send, pair, service, and Hub commands               |
| `@getpaseo/relay`     | Optional encrypted remote transport                                    |
| `@getpaseo/highlight` | Shared syntax-highlighting support                                     |
| `expo-two-way-audio`  | Native streaming audio bridge                                          |
| website               | Public product and documentation surface                               |

The application stack is Expo 54, React Native 0.81, React 19, React Native
Web, TanStack Query, Zustand, and xterm. Electron 41 wraps the shared app. The
daemon uses Node, Express, `ws`, `node-pty`, Zod, ACP and MCP SDKs, direct
provider SDKs, local speech support, and Git/forge integrations. [source]

This is a credible shared-client architecture, not merely a web view stretched
across platforms. The native audio module, notifications, mobile persistence,
desktop managed daemon, web deployment, and terminal renderer still impose
platform-specific seams, but they project the same workspace, agent, and
timeline contracts. [source] [inferred]

## 3. The daemon is the product kernel

The daemon owns agent lifecycle, provider processes, timelines, PTYs, projects,
workspaces, Git operations, schedules, loops, notifications, relay sessions,
and optional Hub relationships. Clients are projections and control surfaces.
Closing a renderer does not inherently close the work. [source]

`ManagedAgent` exposes explicit initializing, idle, running, error, and closed
states. Providers can be native integrations or ACP processes. Current built-in
paths include Claude, Codex app-server, Copilot ACP, OpenCode, Pi, OMP, and a
generic ACP catalog. Provider-native resume handles and histories remain part
of the adapter rather than being flattened into one fake universal session.
[source]

OpenAgents should adapt the kernel/projection split but make every client action
name an admitted capability, engine generation, workload identity, target, and
effect. A persistent daemon improves continuity; it does not itself establish
which party authorized a mutation. [inferred]

## 4. Protocol: one connection, distinct concerns

`packages/protocol` is the wire source of truth. Zod schemas describe JSON
requests, responses, broadcasts, and capability negotiation. A generated
ahead-of-time validator handles inbound WebSocket traffic on constrained
clients; Paseo documents a representative 353 KB provider snapshot improving
from roughly 10.9 ms and 5.9 MB allocated to 2.5 ms and 1.2 MB. Compiler version
and patches are pinned, and regression tests constrain supported schema forms.
[source] [test]

The same WebSocket carries JSON control messages and compact binary terminal
frames. App-level ping/pong exists because browser and React Native WebSocket
APIs do not expose protocol pings. A default 60-second RPC timeout is separate
from liveness. New RPCs use dotted names with correlated `.request` and
`.response` forms while compatibility paths retain older flat names. [schema]

This separation matters:

- transport alive does not mean a provider operation succeeded;
- an RPC timeout does not prove the daemon or agent died;
- a mobile focus heartbeat may influence notification routing but cannot gate
  timeline correctness;
- a terminal byte stream should not inflate into JSON object graphs; and
- capability negotiation is safer than guessing from version strings. [source]

Paseo's append-only schema discipline is practical, but OpenAgents should also
generate closed authority schemas, cryptographically bind negotiated versions,
and receipt any projection loss. The upstream generated validator intentionally
passes unknown object keys through; that is forward-compatible parsing, not a
safe authority boundary. [source] [inferred]

## 5. Timeline delivery is an explicit law

Paseo states the central invariant directly: if the daemon commits timeline
rows, a connected client that opens or resumes the agent eventually displays
every row through the current tail. It implements this with two paths:

1. live `agent_stream` messages optimize immediacy; and
2. authoritative paged fetches establish completeness. [source]

Fetched pages target projected items, not raw storage rows. Responses expose
`seqStart`, `seqEnd`, source sequence ranges, collapse metadata, and cursors so
merged reasoning or tool lifecycle items do not make clients lose their source
position. When a forward page says `hasNewer: true`, the client immediately
continues from `endCursor` until it reaches `hasNewer: false`. [schema]

Every run has a timeline epoch. Delayed live events from an old epoch are
ignored. Historical actions such as Fork carry the exact epoch and projected
item `seqEnd`; the daemon validates that the epoch is current and the source
sequence still exists, then slices before projection so later lifecycle rows do
not leak into the chosen context. [source] [test]

```text
live stream ---------------------> immediate renderer update
     |                                      |
     | may reconnect or observe a gap       |
     v                                      v
authoritative fetch -- page until hasNewer=false
     |
 epoch + source sequence ranges
     |
 complete projected timeline
```

This is Paseo's strongest reusable contract. OpenAgents should adapt it into
Thread/Turn/Item truth with durable broker offsets, gap proofs, projection
receipts, owner-visible recovery, and exact source lineage. Live delivery must
never be confused with durable acceptance, and a renderer cache must never be
the completeness oracle. [inferred]

## 6. Cancellation and split-brain avoidance

Paseo does not change a running agent to idle merely because a client requested
cancellation. The state changes only after provider acknowledgement or a
provider terminal event. A rejection or timeout keeps the agent running. This
avoids the most dangerous cancellation lie: the UI says stopped while the
provider continues to use tools. [source]

OpenAgents should retain this acknowledgement rule and strengthen it with a
cancel intent receipt, provider request identity, deadline, escalation policy,
observed process/workload termination, fence advancement, and a terminal
outcome. “Request sent” and “execution stopped” are separate events. [inferred]

## 7. Subagents: two layers, not one fiction

Paseo separates managed agents from provider-owned child sessions.

Managed creation explicitly chooses `relationship.kind`:

- `subagent` stamps `paseo.parent-agent-id` and joins the parent's subagent
  track; or
- `detached` may inherit configuration and working directory but remains a
  sibling/root agent without parent lifecycle. [schema]

Detaching removes only the parent label; it does not stop, archive, move, or
restart the child. Archiving is a global soft delete and recursively archives
managed descendants. Closing a root tab archives globally, while closing a
subagent tab is client-local. Same-workspace descendants can affect an
ancestor's activity status; cross-workspace children report in their own
workspace. [source]

Provider-owned children are different. A provider may expose child-session
events and read-only child timelines within its own runtime. Paseo merges their
descriptors into the subagent track without pretending the daemon created or
fully controls them. Tests reject stale epochs and preserve locally dismissed
provider children across history replay. [source] [test]

OpenAgents should adapt the distinction into a complete graph:

| Concern            | Managed child                                 | Provider-owned child                         |
| ------------------ | --------------------------------------------- | -------------------------------------------- |
| Identity authority | OpenAgents work-unit and workload identity    | Provider-native identity plus mapped alias   |
| Lifecycle          | admitted create/cancel/archive/detach         | observed provider events and bounded control |
| Transcript         | canonical durable Thread/Turn/Item projection | loss-accounted imported projection           |
| Authority          | explicit delegated capability                 | never inferred from parent visibility        |
| Recovery           | lease, fence, receipt, replay decision        | provider resume/import capability            |

Parentage is useful metadata, not an authorization shortcut. A child needs its
own admitted authority and containment even when the UI presents it beneath a
parent. [inferred]

## 8. Projects, workspaces, and Git placement

Paseo recently made every added folder an independent project. Project identity
uses the exact lexically normalized selected root rather than `realpath` or Git
top-level inference. Workspace IDs are opaque durable placement identifiers;
`cwd` is the execution directory and `worktreeRoot` describes the checkout that
backs it. This prevents nested folders or multiple intentional views of a
repository from being silently merged into one product object. [source]

The daemon watches projects that become Git repositories, provisions and
recovers worktrees, archives workspaces separately from agents, runs workspace
scripts, and exposes forge-neutral Git operations. A recent abstraction added
GitLab and Gitea/Forgejo/Codeberg beside GitHub and moved RPC naming toward
forge-neutral domains. [history] [source]

OpenAgents should adapt the separation between user-selected project identity,
workspace identity, execution directory, checkout, and forge identity. It
should additionally bind exact commit/tree identity, worktree generation,
dirty-state evidence, isolation profile, writeback authority, and merge receipt
to each work unit. Lexical paths alone must not become security identity.
[inferred]

## 9. Terminal pipeline and performance law

Terminals run through a PTY worker and headless xterm state. Output is coalesced
before process boundaries, binary-framed over WebSocket, and rendered by the
client emulator. Leading/trailing throttling preserves fast initial feedback
without turning sustained output into an IPC storm. A coalesced batch carries
the last chunk's revision so snapshot replay does not skip newer bytes. [source]

Snapshot fallback is deliberately conjunctive: output since the last snapshot
must exceed 256 KB **and** the client transport must have more than 4 MB
buffered. A draining client continues to receive the stream. This replaced a
design that repeatedly emitted large cell-grid snapshots during ordinary build
output. [source]

PTY sizing is last-interacting-client-wins. A client claims size only on a real
viewport change or direct user interaction. Passive attach, visibility restore,
font settling, and renderer refits must not resize the shared PTY. [source]

The performance notes also preserve a useful negative result: a single roughly
250 KB agent-stream payload can delay terminal echo by about 100 ms because
daemon serialization and browser parsing share hot loops; relay NaCl/base64
work adds more JavaScript contention. [source]

OpenAgents should adapt these rules as executable performance oracles, then
separate control, timeline, and bulk-artifact channels; bound every queue;
measure end-to-end input echo; and receipt any dropped, collapsed, or snapshot
transition. “WebSocket connected” is not a latency or loss guarantee.
[inferred]

## 10. Tools and provider composition

Paseo defines shared tools in a transport-neutral catalog. Providers that can
accept native tool definitions receive them directly; providers without that
path can consume the same capabilities through an MCP server. The agent model
does not need to know whether a tool crossed an in-process adapter or MCP.
[source]

This is the right reuse seam for schemas, descriptions, validation, and result
normalization. It is not sufficient for policy. OpenAgents should compile every
tool invocation against the actual target, caller, workload generation,
capability, data classification, network policy, and containment profile. MCP
discovery and native provider registration are transports, not grants.
[inferred]

## 11. Direct, relay, and local trust

The optional relay uses a persistent daemon Curve25519 identity, an ephemeral
phone key, QR-fragment trust material, and NaCl XSalsa20-Poly1305 encryption.
Fresh session keys prevent ciphertext replay across sessions, and the relay
cannot read payload contents. It can observe connection metadata and public
handshake material. Within a live session, Paseo does not track nonce reuse or
message counters, so replay protection is not implemented. [source]

The direct daemon boundary is more permissive. Loopback without a password is
the default local posture. Network exposure can add a bcrypt-backed password
carried as HTTP Bearer or WebSocket subprotocol, with health and preflight
exceptions. Once connected, a client is a trusted operator. Host and CORS checks
reduce DNS-rebinding and browser-origin risk but are defense in depth. [source]

File preview can read any regular file available to the daemon user; workspace
paths are convenience, not confinement. Agent subprocesses inherit the user's
authority. A Docker image runs non-root, but mounted workspaces, credentials,
and state are fully available to agents in the container. [source]

OpenAgents should borrow the pairing ergonomics and zero-knowledge relay goal,
then add monotonically bound counters, transcript commitments, device and
engine identities, short-lived proof-of-possession capabilities, revocation,
rekey, audience-scoped projections, and relay-independent receipts. A single
shared operator password is not adequate for mobile, browser, automation, and
Hub clients with different powers. [inferred]

## 12. Hub: a promising narrow execution relationship

Hub is explicitly separate from the relay. The daemon opens an outbound direct
WebSocket after `paseo hub connect`. Relationship identity and a private
credential are persisted before enrollment. Trusted interactive clients may
hold broad `*` access, but Hub receives only `hub.execution.*`; the same matcher
handles exact names and trailing namespace wildcards. [source]

Execution creation is keyed by a Hub-supplied execution ID. The daemon stores
ownership before acknowledging creation. If acknowledgement is lost, retrying
the same create returns the existing agent without starting a second turn. Hub
cannot use the relationship for trusted-client hello/resume, browser access,
binary terminals, retained events, or general broadcast. [source] [test]

The boundary is incomplete but honest:

- transient stream frames are not durably replayed;
- a daemon restart closes an interrupted active turn;
- the original prompt is not persisted and replayed;
- an offline disconnect request stays in a retrying state; and
- an authorization rejection deletes the credential. [source]

OpenAgents should adapt narrow relationship grants, daemon-owned execution
ownership, idempotent external IDs, and explicit revocation. It should add a
durable admitted work record before launch, replayable event log, lease and
fence, signed workload identity, prompt/artifact commitment, cancellation
receipt, and terminal outcome receipt. Idempotent create prevents duplicates;
it does not by itself provide exactly-once effects. [inferred]

## 13. Persistence, schedules, and loops

Paseo stores daemon state as Zod-validated JSON under its home directory. Most
stores use temporary-file rename, but some use direct writes. There is no
general schema-version and migration framework; compatibility relies largely
on optional/default fields plus inline normalization, with a targeted workspace
backfill exception. [source]

Agents, provider handles, projects, workspaces, schedules, loop records,
notification tokens, daemon keys, and settings each have their own shapes.
Schedules can target existing or newly created agents with cron or interval
cadence. Loop records live together in `loops.json`; writes are direct but
serialized by an in-memory queue. On restart, every `running` loop becomes
`stopped` with an interruption log. [schema]

That is suitable for a local beta product that favors inspectability and repair,
but it is not an execution ledger. OpenAgents should reject timers and mutable
JSON as autonomy recovery. Recurring work needs durable admission, occurrence
identity, idempotency, lease/fence ownership, bounded retries, missed-run policy,
side-effect receipts, and an owner-visible terminal state. [inferred]

## 14. Service projection

Paseo can proxy workspace services under generated hostnames. Localhost remains
available, while optional public listener and base-domain configuration support
remote previews through wildcard DNS and a reverse proxy. The documentation is
explicit that public services are **not protected by the daemon password**.
[source]

OpenAgents should not inherit that default. Preview publication must be an
admitted capability with an exact service generation, authenticated audience,
network policy, content-security policy, lifetime, revocation path, audit trail,
and public-exposure warning. A convenient hostname must never silently widen a
workload's authority or data audience. [inferred]

## 15. Release and operational evidence

All workspaces share one version and release together. Paseo documents a
preparation/go-ahead split, beta and stable channels, npm publication, Docker
images, Electron builds, an Android APK, EAS mobile-store delivery, generated
release notes, updater manifests, and a 36-hour linear desktop rollout. Desktop
manifests can be adjusted after publication, but there is no pause, recall, or
downgrade; a bad release requires a superseding hotfix. [source]

Eleven GitHub workflow files cover CI, app/relay/website deployment, desktop,
rollout, Docker, Nix, APK, and notes. This is broad operational craft, but it
conflicts with OpenAgents' invariant that GitHub Actions are not release
authority. OpenAgents can study the artifact matrix and rollout states while
keeping release eligibility, provenance, signing, workload identity, health,
and rollback evidence in its own receipted release system. [source] [inferred]

The AGPL license also makes architecture study different from code reuse.
OpenAgents should port independently expressed contracts and tests, not copy
implementation chunks into differently governed product code. [source]

## 16. Testing posture

The repository has more than a thousand tracked test/spec files across daemon,
protocol, app, providers, Git, worktrees, relay, Hub, terminal, persistence, and
end-to-end flows. Particularly relevant suites cover:

- Hub relationship retries, remote behavior, ownership, execution sessions,
  idempotent creation, and scoped message schemas;
- selective timeline delivery, viewed-timeline catch-up, gap recovery, stale
  epoch rejection, and projected sequence cursors;
- independent-folder projects, Git appearance after registration, workspace
  reconciliation, restart recovery, and worktree behavior;
- terminal snapshots, revisions, input modes, resize races, and reconnects;
- provider subagent history, local dismissal, and stale live updates;
- protocol generation, compatibility, and representative large messages; and
- forge resolution across GitHub, GitLab, Gitea, Forgejo, and Codeberg. [test]

The test breadth is a genuine strength. The remaining gap is proof composition:
tests of each subsystem do not automatically prove that a remote instruction
was admitted by the right owner, ran in the claimed containment, preserved
every event, produced only authorized effects, and shipped as the exact signed
artifact. OpenAgents should turn the best Paseo invariants into end-to-end
receipt and fault suites. [inferred]

## 17. Adapt / study / reject

| Paseo mechanism                         | Stance                         | OpenAgents-native adaptation                                                                 |
| --------------------------------------- | ------------------------------ | -------------------------------------------------------------------------------------------- |
| Persistent daemon, many clients         | Adapt                          | One engine authority projected through Effect Native web/mobile/desktop and protocol clients |
| Live plus authoritative timeline        | Adapt                          | Durable offsets, epoch/fence identity, gap proof, projection lineage, replay receipts        |
| Managed vs provider-owned subagents     | Adapt                          | Complete graph with separate identity, authority, lifecycle, transcript, and recovery planes |
| Cancellation acknowledgement            | Adapt                          | Cancel intent, provider ack, observed termination, fence advancement, terminal receipt       |
| Transport-neutral tools                 | Adapt with stronger boundaries | Native/MCP adapters behind one target-specific capability and policy compiler                |
| Terminal worker/coalescing/backpressure | Adapt                          | Bounded binary streams, independent channels, exact loss/snapshot evidence, latency oracles  |
| Relay pairing and E2EE                  | Adapt with stronger boundaries | Device identity, counters, rekey, DPoP capabilities, transcript commitments, revocation      |
| Hub `hub.execution.*` and idempotency   | Adapt with stronger boundaries | Durable work IDs, leases, fences, replay, workload identity, signed receipts                 |
| Project/workspace/path separation       | Adapt                          | Exact ref/tree/worktree generation, placement, dirty-state, writeback and merge evidence     |
| Shared Expo app and Electron host       | Study                          | Compare platform seams with Effect Native rather than copying the frontend stack             |
| File-backed JSON persistence            | Reject as execution truth      | Transactional durable models with explicit schema versions and migrations                    |
| Loop/schedule restart behavior          | Reject as autonomy recovery    | Occurrence admission, leases, idempotency, missed-run policy, effects and receipts           |
| Trusted operator connection             | Reject                         | Per-client identity and least-privilege proof-of-possession capability                       |
| Host-user execution                     | Reject as containment          | Workload isolation, filesystem/network/secrets policy, attestation and observed cleanup      |
| Public proxy without daemon auth        | Reject                         | Explicit audience-bound publication grants and revocation                                    |
| GitHub/EAS workflow success             | Reject as release authority    | Signed, receipted release eligibility and rollout authority                                  |

## 18. Recommended OpenAgents sequence

1. **Codify the delivery law.** Specify that every admitted Turn/Item eventually
   projects through the current tail, then test reconnect, pagination, stale
   epochs, compaction, projection loss, and slow clients.
2. **Unify engine clients.** Make web, mobile, desktop, CLI, and remote control
   consume one generated authenticated runtime contract without granting equal
   powers to every client.
3. **Separate subagent planes.** Preserve managed work units and provider-native
   children with exact identity, authority, transcript, lifecycle, and recovery
   mappings.
4. **Make cancellation truthful.** Receipt request, acknowledgement, escalation,
   fence advancement, observed stop, and terminal outcome separately.
5. **Harden the remote seam.** Keep outbound pairing and zero-knowledge relay
   ergonomics while adding replay defense, DPoP grants, revocation, rekey, and
   audience commitments.
6. **Build the narrow Hub contract.** Retain scoped external execution IDs and
   idempotent creates, then add durable work, leases, replay, workload identity,
   effect records, and terminal receipts.
7. **Port the terminal laws.** Encode coalescing revision, resize ownership,
   snapshot thresholds, bounded queues, and input-echo latency as cross-client
   conformance tests.
8. **Keep autonomy on the ledger.** Replace mutable timers and loop JSON with
   admitted occurrences, durable scheduling, fencing, retry policy, and proof.
9. **Gate preview publication.** Require exact service generation, audience,
   expiry, policy, and revocation for every non-local exposure.
10. **Preserve release authority.** Study Paseo's platform matrix and staged
    rollout without importing workflow success as provenance or eligibility.

## 19. Bottom line

Paseo is the strongest open reference in this teardown set for a cross-device,
multi-provider agent daemon whose clients can disconnect and reconnect without
becoming runtime authority. More importantly, it writes down several laws that
agent products often leave implicit: live delivery is not completeness,
cancellation is not complete before acknowledgement, provider children are not
the same as managed agents, passive clients must not fight over terminal size,
and duplicate remote creates need stable execution identity.

OpenAgents should take those laws seriously and implement them inside its
stronger authority model. Paseo's convenience boundaries—trusted connected
operators, host-user execution, file JSON, transient Hub streams, incomplete
relay replay defense, and unauthenticated service projection—are precisely
where OpenAgents' capabilities, containment, durable work, leases, fences,
receipts, and signed release discipline must remain stricter.

## Source map

Primary evidence at the audited commit included:

- `README.md`, `SECURITY.md`, `package.json`, and package manifests;
- `docs/product.md`, `docs/architecture.md`, `docs/agent-lifecycle.md`,
  `docs/data-model.md`, `docs/timeline-sync.md`, `docs/hub.md`,
  `docs/providers.md`, `docs/protocol-validation.md`,
  `docs/rpc-namespacing.md`, `docs/terminal-performance.md`,
  `docs/service-proxy.md`, `docs/docker.md`, and `docs/release.md`;
- `packages/protocol/src/messages.ts`, client capabilities, terminal schemas,
  and Hub/provider-subagent tests;
- `packages/server/src/server/agent`, `packages/server/src/server/hub`, session,
  WebSocket, timeline, workspace, schedule, loop, terminal, relay, forge, and
  persistence implementations and tests;
- `packages/app/src/runtime`, timeline, agent-stream, subagent, workspace,
  terminal, and session stores and tests; and
- `.github/workflows/*` plus Git history through
  `a1de743ef67dde4fe7c48d045a3714f65dfa5e90`.
