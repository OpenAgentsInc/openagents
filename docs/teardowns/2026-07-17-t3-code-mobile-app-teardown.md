# T3 Code Mobile App Teardown — 2026-07-17

Read-only implementation audit of the open-source T3 Code mobile application,
its shared client runtime, and the server/desktop/relay surfaces that make the
phone a remote coding controller. The source is `pingdotgg/t3code`, pinned to
commit
[`8b5469863ae1dd696e696de30240ec3da607962d`](https://github.com/pingdotgg/t3code/tree/8b5469863ae1dd696e696de30240ec3da607962d)
(`fix(server): stabilize non-repository Git diagnostics (#4077)`, committed
2026-07-17). The local clone at `projects/repos/t3code` was fast-forwarded and
read as external reference material only.

The Git/worktree safety and offline/reconnect sections were extended on
2026-07-27 against T3 Code
`476d69cd1d5d89b4aad77df8fc01c71e34af930c`.
That later pin is the authority for those two addenda.

This is the mobile-specific successor to the broad
[T3 Code teardown](./2026-07-13-t3-code-teardown.md). That earlier document
remains the authority for the provider adapters, event-sourced server core,
desktop shell, and general security assessment at its older pin. This document
uses the newer pin and follows the phone path end to end.

Evidence labels:

- **[source]** — observed in the pinned source tree.
- **[schema]** — encoded in a typed wire or persistence contract.
- **[docs]** — stated by T3 Code's checked-in documentation.
- **[test]** — asserted by a checked-in test or automation harness.
- **[inferred]** — an architectural conclusion from several observations.
- **[limitation]** — a boundary on what this source audit proves.

No T3 service, Clerk account, relay, push provider, device, or credential was
used. Source proves implementation shape, not App Store availability or live
service reliability. T3's own mobile README says the app is still in
development and is not distributed. [docs] [limitation]

## Executive conclusion

T3 Code mobile is a **full native remote workbench and controller**, not a PWA,
responsive web wrapper, or status companion. It is a separate Expo/React Native
application for iOS and Android that reuses T3's typed Effect client runtime and
wire contracts, then independently implements phone/tablet navigation and five
native interaction hosts. At this pin, the app can:

1. discover, pair with, authenticate to, and retain multiple T3 execution
   environments.
2. browse projects and archived/current threads across those environments.
3. create a project or task, select provider/model/runtime settings, and start
   or continue agent work.
4. stream the normalized thread feed, answer provider questions, approve tool
   calls, and steer the active session.
5. browse project files, render source/Markdown/image/web content, inspect and
   comment on diffs, and invoke typed Git branch/commit/push actions.
6. attach to a thread-scoped interactive terminal through a native
   Ghostty-derived VT surface.
7. queue messages durably while disconnected and drain them after the matching
   environment reconnects.
8. receive agent-aware notifications and deep-link to the exact environment and
   thread. And
9. surface activity through iOS widgets/Live Activities, app shortcuts, and
   system share targets. [source] [schema]

The mobile app does **not** execute coding agents on the phone. A T3 server owns
projects, worktrees, provider processes, terminals, Git, checkpoints, and
projections. The phone selects an `ExecutionEnvironment`, connects over direct
or relay-managed transport, and issues the same typed commands as web/desktop.
That boundary is the core of the implementation. [source] [inferred]

The measured source footprint at the pin is 451 TypeScript/TSX/Swift/Kotlin
files and approximately 75,343 lines under `apps/mobile`, plus 129 files and
approximately 23,372 lines in `packages/client-runtime`. Tests, generated code,
and native bindings are included in those counts. This is already a substantial
product even though distribution remains pre-release. [source] [limitation]

## 1. Product topology

```text
T3 Code mobile (Expo / React Native / native modules)
  |
  | Effect RPC over authenticated WebSocket
  | snapshot + ordered deltas + typed commands
  v
selected T3 server / ExecutionEnvironment
  |-- orchestration projections and provider commands
  |-- project/worktree/filesystem/Git/checkpoints
  |-- PTY/terminal sessions
  |-- provider adapters: Codex, Claude, Cursor, Grok, OpenCode
  |
  +-- direct LAN/tailnet ws(s)
  +-- one-time pairing -> client session credential
  +-- T3 Connect relay discovery + DPoP-bound access
       `-- managed tunnel reaches the same local server
```

The important invariant is that relay and mobile are not alternate engines.
They are access and projection layers around one server-owned environment. The
same thread/project identity is visible to web, Electron, and mobile because
all clients consume the same server projections. [source] [schema]

Primary source surfaces:

| Concern                              | Pinned source                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Mobile bootstrap and variants        | [`apps/mobile/README.md`](https://github.com/pingdotgg/t3code/blob/8b5469863ae1dd696e696de30240ec3da607962d/apps/mobile/README.md), [`app.config.ts`](https://github.com/pingdotgg/t3code/blob/8b5469863ae1dd696e696de30240ec3da607962d/apps/mobile/app.config.ts)                                                                                                                                                       |
| Application composition and routes   | [`src/App.tsx`](https://github.com/pingdotgg/t3code/blob/8b5469863ae1dd696e696de30240ec3da607962d/apps/mobile/src/App.tsx), [`src/Stack.tsx`](https://github.com/pingdotgg/t3code/blob/8b5469863ae1dd696e696de30240ec3da607962d/apps/mobile/src/Stack.tsx)                                                                                                                                                               |
| Shared connection/projection runtime | [`packages/client-runtime/src`](https://github.com/pingdotgg/t3code/tree/8b5469863ae1dd696e696de30240ec3da607962d/packages/client-runtime/src)                                                                                                                                                                                                                                                                           |
| Pairing and environment registry     | [`features/connection`](https://github.com/pingdotgg/t3code/tree/8b5469863ae1dd696e696de30240ec3da607962d/apps/mobile/src/features/connection), [`connection`](https://github.com/pingdotgg/t3code/tree/8b5469863ae1dd696e696de30240ec3da607962d/apps/mobile/src/connection)                                                                                                                                             |
| Thread workbench                     | [`features/threads`](https://github.com/pingdotgg/t3code/tree/8b5469863ae1dd696e696de30240ec3da607962d/apps/mobile/src/features/threads)                                                                                                                                                                                                                                                                                 |
| Files, review, terminal              | [`features/files`](https://github.com/pingdotgg/t3code/tree/8b5469863ae1dd696e696de30240ec3da607962d/apps/mobile/src/features/files), [`features/review`](https://github.com/pingdotgg/t3code/tree/8b5469863ae1dd696e696de30240ec3da607962d/apps/mobile/src/features/review), [`features/terminal`](https://github.com/pingdotgg/t3code/tree/8b5469863ae1dd696e696de30240ec3da607962d/apps/mobile/src/features/terminal) |
| Native hosts                         | [`apps/mobile/modules`](https://github.com/pingdotgg/t3code/tree/8b5469863ae1dd696e696de30240ec3da607962d/apps/mobile/modules)                                                                                                                                                                                                                                                                                           |
| Cloud/relay setup                    | [`docs/cloud/t3-connect-clerk.md`](https://github.com/pingdotgg/t3code/blob/8b5469863ae1dd696e696de30240ec3da607962d/docs/cloud/t3-connect-clerk.md)                                                                                                                                                                                                                                                                     |
| Store screenshot proof harness       | [`docs/operations/mobile-app-store-screenshots.md`](https://github.com/pingdotgg/t3code/blob/8b5469863ae1dd696e696de30240ec3da607962d/docs/operations/mobile-app-store-screenshots.md)                                                                                                                                                                                                                                   |

## 2. It is a native application, not the web renderer on a phone

The app uses Expo 56, React Native 0.85, React Navigation native stack,
`@effect/atom-react`, Expo SQLite/SecureStore/Notifications/Updates, Uniwind,
Legend List, Shiki/Pierre diff tooling, Clerk Expo auth, and local Expo modules.
Its `app.config.ts` declares only iOS and Android platforms. No WebView owns the
application shell. [source]

There are three side-by-side application identities:

- `development` — Expo dev client, `t3code-dev` scheme.
- `preview` — persistent internal preview, `t3code-preview` scheme. And
- `production` — store identity, `t3code` scheme. [source] [docs]

Native modules make Expo Go insufficient. Local builds prebuild native iOS or
Android projects. Preview/production profiles also exist for EAS. The app uses
fingerprint-compatible OTA runtime selection so a JavaScript update cannot be
delivered to a binary missing required native code. [source] [docs]

The React Native application root installs one Effect atom registry, cloud
authentication, appearance preferences, gesture/keyboard/safe-area providers,
native navigation theme, incoming-share coordinator, confirmation host, and
overlay portal. Deep links accept production, preview, development, and Expo
launcher schemes while filtering launcher/share lifecycle URLs that are not
navigation. [source]

## 3. Shared domain runtime, independent UI product

T3 gets cross-client consistency from contracts and runtime code, not from one
shared UI tree. `packages/contracts` defines schema-only environment, auth,
project, thread, terminal, filesystem, Git, review, and orchestration shapes.
`packages/client-runtime` owns the reusable client behavior. Mobile provides
native persistence and presentation. [source] [schema]

The shared runtime supplies:

- environment-scoped atom families and queries.
- connection resolution, authorization, generation fencing, reconnect, and
  wakeups.
- cached shell/thread snapshots followed by sequenced WebSocket deltas.
- schema-versioned persistence abstractions.
- typed commands with parallel, serial, single-flight, latest-only, or keyed
  scheduling semantics. And
- state families for projects, threads, filesystem, Git/VCS, review, preview,
  terminal, provider models, archived threads, and orchestration. [source]

Mobile's `src/state/*` mostly composes these factories against its one
connection runtime. This prevents networking and projection logic from being
rewritten per screen while leaving the actual mobile information architecture
free to be native. [source] [inferred]

The tradeoff is a duplicated presentation system: T3 web and mobile share the
server/runtime but independently implement composer, conversation, diff,
terminal, tokens, navigation, focus, and accessibility. The source contains
real cross-platform product parity, but not one executable component model.
[source] [inferred]

## 4. Environment discovery, pairing, and access

The application is multi-environment from the beginning. An environment is a
server-owned execution location. An endpoint is one way to reach it. The
mobile registry can retain several endpoint profiles and present their
connection phase independently. [schema] [source]

### 4.1 Direct pairing

The Add Environment screen accepts a host and optional code or scans a QR code
with Expo Camera. Pairing links are parsed into a normalized endpoint, then the
client exchanges the short-lived bootstrap credential for an ordinary session
credential. Long-lived credentials are persisted through the mobile secure
store implementation, not retained as the QR/link secret. [source] [schema]

The server auth contract distinguishes loopback-browser, remote-reachable, and
other exposure modes, plus bootstrap methods and ordinary access methods. It
can enumerate and revoke pairing links and client sessions. [schema]

### 4.2 Direct LAN, tailnet, and desktop-managed access

The connection catalog is transport-neutral. A phone can reach a server over
direct `ws://`/`wss://` endpoints, including LAN and Tailscale addresses. The
desktop shell can advertise Tailscale endpoints or construct an SSH-forwarded
server endpoint. The mobile client still consumes an ordinary server URL and
does not embed an SSH implementation. [source]

The iOS application declares local-network usage and currently permits
arbitrary transport loads. That improves development/LAN reachability but is a
meaningful security relaxation: the product can connect to plaintext or
privately certified endpoints if the user configures them. [source]

### 4.3 T3 Connect

T3 Connect is optional and disappears when its public Clerk/relay configuration
is absent. When configured, Clerk authenticates the user. The relay discovers
linked environments and mints scoped access. Mobile generates a DPoP key,
exchanges a relay bootstrap for a proof-bound environment token, and opens a
WebSocket with a short-lived, single-purpose ticket rather than placing the
bearer credential in the socket URL. Proof-bound access tokens can be cached
and refreshed without making the relay the thread/session database. [source]
[schema] [docs]

On the host, `t3 connect link` records durable exposure intent and a supervised
`cloudflared` connector exposes the same local server through the managed
tunnel. Connector failure is observed and reconciled. The relay plane supplies
identity, environment discovery/access, tunnel coordination, and push—not
coding execution. [source] [docs]

This is operationally convenient but not provider-neutral infrastructure: the
current implementation is specifically Clerk + Cloudflare Tunnel + the hosted
relay. Direct endpoint paths remain the self-hosted escape hatch. [limitation]

## 5. Navigation and workspace information architecture

The root native stack makes the workbench route-addressable. Major routes are:

- Home/current threads and archived threads.
- Thread detail.
- thread Terminal, Review, Files tree, and individual File.
- Git overview, commit, branches, and confirmation sheets.
- Connections and Add Environment.
- New Task plus repository/local-project destination flows. And
- Settings for environments, appearance, client storage, authentication,
  waitlist, and legal documents. [source]

Thread routes are deliberately flat in the root native navigator so iOS shared
headers morph rather than nesting a second navigation controller. Sheets are
modeled as overlays that do not change the underlying workspace selection.
[source]

On compact phones, navigation is progressive: list → thread → focused
terminal/review/file surfaces. On wider devices, `AdaptiveWorkspaceLayout`
uses available geometry rather than device labels, adding sidebar and inspector
panes when the viewport can support them. The threshold is approximately
720×600, with bounded/persisted pane widths. [source]

This is a controller-oriented IA rather than a scaled desktop. The thread stays
central, while high-density repository surfaces become contextual routes or
panes. [inferred]

## 6. Thread and agent control

The thread screen is the primary control surface. It combines normalized
conversation/activity projection, provider/runtime status, pending interaction
cards, a rich composer, worktree context, and navigation to repository tools.
[source]

Implemented control paths include:

- create a new task for a selected project/repository.
- choose available provider/model/runtime inputs exposed by the environment.
- send a new turn or continue the selected thread.
- display streaming provider-normalized messages and activity.
- answer structured user-input requests.
- approve or reject pending tool requests.
- stop/cancel active work where the server exposes the command.
- archive, unarchive, or delete threads.
- inspect project/thread work logs and worktree identity. And
- use attachments, file/path context, share-sheet input, and composer commands.
  [source]

`ThreadFeed` uses a keyboard-aware Legend List, bottom anchoring, content
retention, and explicit keyboard/inset behavior. Rich Markdown and code are
rendered through a native/text host with bounded highlighting machinery rather
than a generic WebView. This matters because mobile controller usefulness
depends on reliable long-stream reading while the keyboard and network state
change. [source] [inferred]

## 7. Files, review, and Git

T3 does not stop at chat. The mobile controller projects the server-owned
workspace into several typed surfaces:

- searchable/browsable file tree.
- source file viewer with syntax highlighting.
- Markdown, image, and web-preview handling.
- review model over checkpoint/worktree diff data.
- per-file additions/deletions and file navigation.
- selected-line review comments that can re-enter the composer.
- Git overview and branch selection.
- commit, push, pull, publish, and confirmation flows gated by server
  capability/readiness. And
- progress overlays and stale/error presentation for mutations. [source]

The native review surface draws large diffs in Swift/Kotlin while JavaScript
owns selection, navigation, comments, and fallback/highlight policy. This is a
purpose-built foreign host rather than trying to render a desktop diff DOM on a
phone. [source]

The phone still trusts server-side repository authority. It does not receive a
raw host root and independently run Git. It sends typed actions scoped to the
selected environment/project/thread/worktree. [source] [inferred]

That server boundary does not make every Git action transactional. Pull
requires an upstream and uses fast-forward-only. Push is nonforce with an
explicit refspec. The multi-step feature-branch, commit, push, and
pull-request action can still stop after any successful step. The selected-file
commit path resets the current index before staging the requested paths and
does not restore the user's prior staged selection after a later failure.
[source]

Worktree and checkpoint safety is also uneven:

- task bootstrap creates Git state before thread metadata records it, so a
  process stop can orphan a branch and worktree
- worktree removal accepts force on the environment and is not a durable
  cleanup operation
- checkpoint capture correctly uses a temporary index
- checkpoint restore runs `git clean -fd`, which removes untracked nonignored
  files
- the UI confirmation does not explicitly describe that untracked-file loss
- the mobile client does not offer the desktop's optional linked-worktree
  cleanup decision when deleting a thread [source] [inferred]

Omega should copy the remote authority and conservative pull/push defaults.
It should retain its stronger managed-worktree provenance checks, staged and
unstaged archive checkpoints, and refusal to clean unknown files by default.
[recommendation]

## 8. A real interactive terminal

The terminal is a first-class route and optional thread overlay. Opening it
attaches to the server's thread-scoped terminal identity with cwd/worktree,
column, and row inputs. The client receives a snapshot plus output stream,
replays buffered bytes, sends keyboard input, resizes the PTY, detects exit,
and can explicitly reopen a stale cached subscription. [source]

`modules/t3-terminal` vendors the `libghostty-vt` C API and implements native
iOS and Android terminal views. A text fallback exists when the native surface
is unavailable. Appearance settings cover font size, theme, and other terminal
preferences. [source]

This is one of the largest differences between “mobile supervision” and a full
controller: a user can inspect and intervene in the actual environment without
handoff to Desktop. [inferred]

## 9. Offline behavior and persistence

Mobile persistence uses Expo SQLite for schema-versioned client data and Expo
SecureStore for credentials. Cached shell/thread projections allow the app to
paint retained state before live synchronization. The UI distinguishes
`cached`, `synchronizing`, and `live` domain state from the socket connection.
Connection generation prevents an old session from becoming current. Ordered
shell and detail sequences remove replay overlap. A completion marker prevents
the UI from claiming that replay has caught up early. [source]

The connection supervisor treats offline network state, transient transport
failure, and blocked authorization/configuration as different states.
Transient connection delays grow from one to 16 seconds and retry
indefinitely. Establishment and foreground probes time out after 15 seconds.
The backoff resets only after 30 stable seconds. Returning to the foreground
probes the socket and independently resubscribes shell and thread data from
their latest cursors. [source] [test]

This means a healthy WebSocket is not enough to claim healthy data. It also
means a dead domain stream can be repaired without waiting for the underlying
socket to fail. [inferred]

The thread outbox persists queued messages before exposing them in the in-memory
atom. Mutations are serialized. Updates cannot resurrect an item removed by a
concurrent successful drain. Clearing one environment is scoped. The drain
waits for the corresponding environment connection. Existing-thread messages
always take this path, even while online, and the composer clears only after
the disk write succeeds. [source] [test]

The queue stores one schema-v3 JSON file per message. The item retains stable
thread, message, and command IDs, attachments, model, runtime, interaction
mode, and optional new-task data. It derives stable IDs for the three setting
updates that precede turn start. Transport and interruption failures retry
with one-to-16-second backoff. Deterministic turn failures remove the item,
while setting synchronization failures remain retryable. [source]

The delivery gate is state-aware:

| Item                    | Authoritative state                          | Result                            |
| ----------------------- | -------------------------------------------- | --------------------------------- |
| New task                | shell not live                               | wait                              |
| New task                | thread now exists                            | remove duplicate local intent     |
| Existing-thread message | target absent and shell not live             | wait                              |
| Existing-thread message | target absent and shell live                 | remove because the target is gone |
| Existing-thread message | target exists but is busy                    | wait                              |
| Existing-thread message | connected, target exists, and target is idle | synchronize settings, then send   |

Only the queueing phone is durable here. The environment still owns command
acceptance and all execution. Most other mutations remain live-only, including
approval, interrupt, Git actions, and terminal input. [source] [inferred]

There are four important limits:

1. Running partial thread detail is not persisted. An app process restart
   offline returns to the last settled transcript, while a temporary
   disconnect in the same process can keep the in-memory partial stream.
2. A queued worktree task does not persist its generated temporary branch.
   Retries can generate different `t3code/<random>` branches behind the same
   final command ID.
3. Queue files are written directly rather than through a visible
   temporary-file and atomic-rename protocol. A corrupt file is logged and
   ignored, not quarantined or repaired.
4. Online new-task creation uses a direct path rather than the durable outbox.
   An ambiguous response retains the draft, but a manual retry creates new
   identifiers and can duplicate work. [source] [inferred]

This is durable offline submission intent, not offline agent execution. The
message remains pending until the authoritative environment accepts it. The
app does not pretend cached state is current when reconnect/authorization is
unavailable. [source] [inferred]

Terminal recovery is snapshot-based rather than phone-owned. Every attach
receives persisted environment history, then buffered concurrent events, then
live output. The client bounds the displayed buffer to 512 KiB. Input, resize,
clear, restart, and close never enter the outbox and must not be replayed after
an offline interval. [source]

For Omega, the pattern to copy is the separation among transport health,
projection health, and command delivery. The pattern to strengthen is the
command envelope. Omega should persist the complete normalized payload,
including worktree branch and base commit, bind it to a payload digest and
target generation, use atomic queue storage, quarantine corruption, and keep a
visible terminal receipt for deterministic failures. [recommendation]

## 10. Ambient and system-level controller surfaces

T3 extends control beyond the foreground app:

- Expo Notifications registers an authorized mobile endpoint with the selected
  environment/relay and routes notification taps to an exact environment and
  thread.
- iOS `AgentActivity` widgets/Live Activities show current agent state on the
  lock screen and Dynamic Island-class surfaces.
- an iOS widget can receive frequent updates and push-backed activity.
- app shortcuts open recent/new-task flows.
- the system share extension accepts text, URLs, and images into a durable
  inbox, then opens the real new-task flow. And
- Android notification and adaptive/monochrome icon configuration is present.
  [source]

These are projections and entry points, not separate command authorities. The
foreground app still resolves their payload against the current environment
catalog and navigation state. [source] [inferred]

## 11. The five native host islands

| Module               | Responsibility                                            | Boundary                                                                        |
| -------------------- | --------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `t3-composer-editor` | Native multiline composer/input behavior                  | Native text system emits controlled editor revisions to React Native            |
| `t3-markdown-text`   | Selectable rich Markdown/code text and file icons         | JavaScript prepares content. Native iOS text layout handles selection/rendering |
| `t3-native-controls` | Native header buttons and keyboard commands               | Platform chrome/commands only. Domain actions remain typed callbacks            |
| `t3-review-diff`     | High-performance native diff drawing and interaction      | Review model/selection stay in application state                                |
| `t3-terminal`        | Ghostty-derived VT parsing/rendering/input on iOS/Android | PTY/process remain server-side                                                  |

This is a pragmatic architecture: keep application and remote-runtime state in
Effect/React Native, but move interactions that materially benefit from native
text, rendering, keyboard, or terminal behavior into narrow modules. [source]
[inferred]

Its cost is substantial platform code and duplicate fallbacks. Each island
needs Swift/Kotlin lifecycle, accessibility, theming, performance, and version
compatibility work. T3's native lint task and static check exist because the
surface is no longer “just Expo.” [source] [docs]

## 12. Verification and release posture

The mobile source has extensive unit tests for connection, relay auth, DPoP,
environment rows, files, review, terminal, outbox, sharing, shortcuts, and
presentation helpers. Native Swift/Kotlin lint is part of the documented check
path when tools are installed. [test] [docs]

The strongest end-to-end artifact is the app-store screenshot harness. It:

1. creates three disposable real T3 servers and Git repositories.
2. seeds deterministic threads, activities, terminal history, diffs, and
   pending outbox items.
3. pairs clean app installations through the production connection flow.
4. opens the real Home, Thread, Terminal, Review, and Environments routes.
5. captures iPhone, iPad, Android phone, and Android tablet light/dark matrices.
   and
6. validates dimensions, color mode, file size, count, and store-slot
   constraints. [test] [docs]

This is unusually good visual/route integration evidence. It is still seeded
local-environment evidence, not a proof of Clerk, relay, APNs, hostile network,
or App Store release behavior. [limitation]

The app has EAS development/preview/production profiles and Expo Updates. T3's
README nevertheless explicitly states that mobile is not currently
distributed. “Implemented,” “buildable,” “showcase-tested,” and “shipping to
users” must therefore remain separate status claims. [docs]

## 13. Security and authority assessment

### Strong mobile boundaries

- Bootstrap pairing credentials are distinct from ordinary client sessions.
- Relay access uses DPoP proof-of-possession rather than bearer possession
  alone.
- Credentials use platform secure storage. Projection caches use SQLite.
- The phone never becomes the provider credential or process host.
- Server operations cross schema-validated typed contracts.
- Cached/offline state is modeled separately from connected authoritative
  state. [source] [schema]

### Material weaknesses or limits

- iOS currently enables arbitrary transport loads, so secure transport depends
  on endpoint choice.
- Direct remote endpoints and the optional hosted relay have different trust
  properties that the product must explain clearly.
- T3 Connect depends on a specific hosted identity/tunnel topology.
- The phone can invoke consequential Git/process/approval operations, but T3
  still lacks OpenAgents-style authority manifests and durable execution/
  delivery receipts.
- Worktree bootstrap, forced cleanup, stacked Git actions, and checkpoint
  restore are not one durable transaction. A command can fail after a branch,
  worktree, index, filesystem, or remote push already changed.
- The server accepts caller-selected worktree paths without a visible
  managed-root confinement check. Mobile should not be treated as a safe
  cleanup authority merely because it uses typed RPC.
- Checkpoint restore can delete untracked nonignored files, while the current
  confirmation does not state that consequence explicitly.
- The mobile queue does not atomically quarantine corrupt entries and does not
  persist the generated temporary worktree branch.
- Server execution remains environment-local and inherits T3's broader
  containment weakness: mobile reachability is well protected while the
  wrapped agent may still run with unrestricted host authority.
- Threads can be controlled remotely but cannot be checkpointed, detached,
  moved, and resumed on a different host as one portable session. [source]
  [inferred]

## 14. What OpenAgents should take from this implementation

Adapt directly:

1. Treat mobile as a complete client of the same controller protocol, not a
   chat-only projection.
2. Make environment discovery/pairing, connection health, and cached/offline
   truth the first product layer.
3. Give a selected coding session compact Thread, Files, Changes, Terminal,
   Preview, and Artifacts modes.
4. Use typed foreign hosts for native terminal, diff, editor, Markdown, and
   platform chrome only where they materially improve the interaction.
5. Make the outbox, notification/deep-link resolver, share target, quick
   actions, and lock-screen status part of the controller contract.
6. Test the real app against disposable real servers and repositories on phone
   and tablet geometries, not only component fixtures. [inferred]

Strengthen rather than copy:

1. Keep OpenAgents' one Effect Native component/intent/token model instead of
   T3's separate web and mobile application trees.
2. Put every remote file, Git, PTY, preview, approval, and session-control
   action behind revocable capability grants and durable outcomes.
3. Preserve fail-closed containment and expose requested versus effective
   policy.
4. Keep Google Cloud/owned infrastructure authority. Do not import T3's
   Cloudflare Tunnel/EAS/GitHub Actions topology.
5. Make the stable object a portable coding session that can change hosts,
   rather than an environment-local thread reachable from several clients.
6. Separate observed notification/Live Activity status from completion or
   release authority. [inferred]

## Final assessment

T3 Code's mobile implementation answers the concrete product question the
earlier broad teardown left open: **what does a phone need in order to be the
real controller for coding agents running elsewhere?** Its answer is not just
streaming chat. It is environment access, durable projections, pending-action
cards, a rich composer, repository navigation, native diff review, Git
writeback, a PTY, offline intent, notifications, lock-screen presence, and
phone/tablet-native navigation over the same server command plane.

OpenAgents already has the more ambitious authority and portability model. T3
shows that model will not be competitive until it is expressed through a
similarly complete foreground and ambient mobile product. The correct parity
target is therefore T3's controller breadth plus OpenAgents' stronger grants,
containment, receipts, one-UI substrate, and host-portable session identity—not
feature imitation in isolation.
