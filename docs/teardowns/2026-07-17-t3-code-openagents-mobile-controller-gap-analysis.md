# T3 Code vs OpenAgents Mobile — Controller Gap Analysis and Parity Roadmap

- Date: 2026-07-17
- T3 Code pin:
  [`8b5469863ae1dd696e696de30240ec3da607962d`](https://github.com/pingdotgg/t3code/tree/8b5469863ae1dd696e696de30240ec3da607962d)
- OpenAgents comparison pin: `b97e45250f73c7a1df04be3887c93d0a7761f69f`
- T3 mobile evidence:
  [mobile implementation teardown](./2026-07-17-t3-code-mobile-app-teardown.md)
- Existing broad evidence:
  [T3 Code teardown](./2026-07-13-t3-code-teardown.md),
  [T3/OpenAgents Desktop gap analysis](./2026-07-15-t3-code-openagents-desktop-full-gap-analysis.md),
  [latest Desktop UI gap analysis](./2026-07-17-t3-code-openagents-desktop-ui-gap-analysis.md)
- OpenAgents authority:
  [mobile port plan](../sol/2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md),
  [portable-session pathway](../sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md),
  [APP-MOBILE issue ledger](../sol/issues/app-mobile.md), and
  [Effect Native dossier](../effect-native/README.md)
- Status: analysis and proposed delivery roadmap; this document does not admit
  product mutations, dispatch work, widen authority, or replace the Sol issue/
  claim ledgers.

## Implementation update — PORT-06 controller foundation

The first bounded PORT-06 implementation packet landed on 2026-07-17 through
#8944–#8946. OpenAgents mobile now projects confirmed coding sessions into
typed Recent, Repositories, and Attention destinations and renders an Effect
Native metadata-first controller shell. A session can be inspected before its
transcript is activated; the overview shows canonical repository/session/
thread identity, lifecycle, exact provider/runtime availability, last
activity, cursor, and checkpoint presence. Continue reuses the existing
exact-ref activation intent.

This closes the **controller-shell foundation**, not PORT-06 or full T3 mobile
parity. The current coding catalog still does not project a multi-host
environment inventory or host-movement command outcomes, and Files, Changes,
Terminal, Preview, Artifacts/Receipts, push registration, voice, and physical
iOS/Android parity receipts remain open. Cached rows remain counted-but-hidden
whenever Sync authority is withheld.

## Implementation update — portable-session controls

The second bounded PORT-06 packet landed on 2026-07-17 through #8947–#8949.
The shared Sync client now reads confirmed portable sessions, attachment
generations, target directories, accepted commands, and terminal outcomes from
the authenticated owner scope. Mobile joins that authority to the inspected
controller session and exposes typed Stop, Checkpoint, Move, Resume, and
Failback commands with explicit destination selection.

The UI fails closed for stale or malformed authority, ambiguous attachments,
unready/same-source destinations, and commands already in flight. A mobile tap
is shown as queued until a server projection confirms acceptance or a durable
outcome; it is never presented as a completed host move from local mutation
state. Confirmed outcome status and bounded evidence counts reconcile through
the same authenticated Sync subscription.

This closes the **mobile portable-command surface**, not the real-movement
prerequisite or full PORT-06 acceptance. The server/runner must still execute
checkpoint → quiesce → attach generation N+1 → revoke/close generation N,
prove failback and crash recovery across two distinct hosts, and publish the
signed receipts required by PORT-03. Until then, these controls are an honest
command controller over confirmed authority, not evidence that a live session
has physically moved.

## Executive decision

OpenAgents should target **full mobile-controller parity with T3 Code**, but
not implementation identity.

T3 Code is ahead in foreground and ambient controller breadth. Its current
mobile source already exposes multiple environments, projects, threads,
structured approvals/questions, files, review, Git mutations, a real terminal,
offline outbox, share targets, quick actions, push/deep links, and iOS Live
Activities/widgets. Its server remains the execution environment; the phone is
a capable native client of the same control plane as web/desktop.

OpenAgents is ahead in the architecture that should sit underneath those
surfaces:

- local-first identity with optional verified account linking;
- one Effect Native application/component/intent contract;
- exact-ref Khala Sync projections and durable command outcomes;
- explicit runtime target selection and no silent fallback;
- provider-neutral interaction and agent-graph contracts;
- fail-closed authority, containment, grants, and receipts as product
  requirements; and
- a designed host-portable coding-session identity rather than an
  environment-local thread.

But the currently implemented OpenAgents phone is not yet a full controller.
It is a strong 11.8k-line controller **foundation** centered on a single
conversation/drawer/composer surface. It can select confirmed coding sessions,
continue/cancel/resume/retry/close turns, answer questions, approve tools,
select an explicit execution target, inspect Fleet and agent topology, and
survive local cache/restart/reconnect paths. It does not yet expose a remote
workspace, Git review/writeback, PTY, preview, artifacts/receipts, complete
push system, any-host directory, host movement/failback, or voice.

The near-term parity move is therefore not another mobile architecture reset.
It is to **finish the workbench and controller around the contracts already
landed**, in the dependency order already named by PORT-03 through PORT-08.

## 1. Status vocabulary

This analysis keeps four statuses separate:

| Status | Meaning |
| --- | --- |
| **Implemented** | Production source exists in the current app/runtime path. |
| **Deterministically verified** | Current tests/typecheck exercise the implementation without claiming a physical-device or deployed-service result. |
| **Designed** | An authoritative roadmap/contract describes the behavior, but current mobile source does not expose the complete product path. |
| **Live proven** | A deployed/installed physical-device journey has exercised the exact behavior and retained the required receipt. |

During this audit, the bounded mobile suite passed **22 test files / 126 tests**
and mobile TypeScript passed. Those results support deterministic verification,
not App Store, physical iPhone, relay, push, voice, cross-host movement, or
managed-provider claims.

Historical mobile audits are not current authority. In particular, older
documents that say Sync or Fleet are wholly absent are superseded by current
source and `docs/sol/issues/app-mobile.md`. The remaining terminal/workspace/
portability gaps are real; the earlier Sync/Fleet absence claims are not.

## 2. What OpenAgents mobile implements today

### 2.1 One Effect Native application contract

`apps/openagents-mobile/src/screens/home-core.ts` owns typed state, intents,
handlers, and the serializable view program. `src/effect-native/
effect-native-host.tsx` is the React Native mount. React Native/Expo own
safe-area, keyboard, native SDK, and renderer duties—not a second application
model. Architecture tests reject a restored app-local native UI island, direct
application imports of native UI controls, a second composer, or ordinary RN
application controls outside the renderer boundary.

This is a stronger cross-client direction than T3's duplicated web/mobile
component systems. It is also less mature in component breadth: the current
Effect Native catalog does not yet lower the full coding workbench.

### 2.2 Local-first identity, account link, and Sync

The app owns:

- an Expo SQLite local store and stable installation/device-local identity;
- a versioned SecureStore session vault;
- GitHub authorization-code + S256 PKCE sign-in;
- server verification, owner matching, access/refresh rotation, denial purge,
  and dual-revocation sign-out;
- optional authenticated personal-scope HTTP/WebSocket Khala Sync; and
- explicit signed-out, credential-present-unverified, session-ready,
  bootstrapping, catching-up, live, stale, must-refetch, denied, and unavailable
  presentation states.

The account is an upgrade to cross-device/network features, not a prerequisite
for local use. Credentials, owner refs, native handles, and transport objects
do not enter the Effect Native view program.

### 2.3 Confirmed conversations and runtime control

Once personal Sync is live, mobile can list/open/create confirmed threads,
watch a selected thread, append exact-ref messages, and wait for authoritative
projection before presenting the mutation as settled. Provider-neutral
timeline items cover text, reasoning, connection, tool, plan, usage, terminal
status, interruption, approval, question, and failure categories.

Implemented consequential controls are:

- answer provider questions;
- approve or deny tool requests;
- accept a plan, request changes, or replan;
- cancel, resume, retry, or close a run; and
- send a new turn or continue the confirmed active thread.

`pause` is intentionally filtered because no honest current mobile outcome
contract exists for it. That is a visible parity gap, not a hidden alias to
another action.

### 2.4 Coding catalog, drafts, attachments, and explicit targets

The app consumes a confirmed repository/session catalog, persists only the
selected stable refs locally, restores a selected session, and rejects stale,
owner-mismatched, missing, or revoked targets. It can activate a target from
the in-app directory, deep link, or notification-response payload.

The coding composer owns a restart-stable private draft. Native file/image
selection is bounded, hashes bytes with SHA-256, copies accepted content into
the durable app sandbox, and stores only content-addressed ready metadata—raw
picker URIs never become Sync or view authority.

Execution target options distinguish hosted OpenAgents, Agent Computer, Codex,
and Claude lanes. The exact selected target is persisted into submission; an
unavailable or no-longer-advertised target fails closed instead of silently
downgrading.

### 2.5 Fleet and agent topology

The drawer can project Fleet runs, attempt assignment, and closeout refs. The
selected thread can show the confirmed nested agent graph, attention state,
and an exact node inspector with provider, runtime, session, worktree, elapsed,
token, current-action, and terminal fields. The phone bounds rendering at 40
rows and names omitted count instead of silently flattening topology.

This is meaningful supervision. It is not yet Fleet operation breadth: current
mobile does not create or place Fleet runs, batch/swarm work, manage account
priority/quota, or execute host movement.

### 2.6 Accessibility and release identity

The current view contract models reduced motion, dynamic type, and a minimum
44-point target, with deterministic host/view tests. The app has owned iOS/
Android identity, local build paths, and an owned OTA channel. OpenAgents policy
forbids EAS cloud builds/updates and GitHub-hosted CI; that difference from T3
is intentional.

`src/contracts/ux-contracts.ts` records local-first identity, runtime
interactions, coding navigation, accessibility, offline-cache accounting, and
agent graph seams as enforced. Each seam still distinguishes deterministic
evidence from open physical iOS/Android/VoiceOver/TalkBack receipts.

## 3. Designed OpenAgents destination

The active mobile/portable-session documents already describe a controller
broader than T3's:

1. a global authorized directory of sessions and hosts;
2. one host-independent coding-session identity;
3. exclusive attachment generation and fresh target grants;
4. stop, quiesce, checkpoint, detach, attach, move, resume, failback, reclaim,
   and expiry;
5. owner-local, owner-managed/homelab, OpenAgents-managed, and future
   provider-managed targets;
6. compact Thread, Files, Changes, Terminal, Preview, and Artifacts/Receipts
   modes;
7. exact diff review plus safe branch/PR writeback;
8. push/deep links into attention/approval state;
9. persona-neutral ASR/TTS/barge-in over the same typed command algebra; and
10. cross-device/cross-host fault proof with no duplicate execution, unsafe
    grants, secret-bearing checkpoint, silent fallback, or orphaned source.

PORT-00 and PORT-01 establish schemas/durable authority but do not prove a
provider process moved. The issue ledger explicitly keeps PORT-03 through
PORT-08 open for real movement, owner-managed nodes, managed providers,
any-host mobile control, voice, and signed dogfood.

That designed destination is stronger than T3's current environment-local
remote control. The immediate problem is product realization, not missing
vision.

## 4. Capability gap matrix

Legend:

- **I** — implemented in current source;
- **V** — deterministically verified in the current bounded suite;
- **D** — designed in current authority docs;
- **—** — no complete current product path;
- **S** — OpenAgents has a stronger boundary that should be preserved.

| Capability | T3 Code mobile | OpenAgents current | OpenAgents designed | Gap / disposition |
| --- | --- | --- | --- | --- |
| Native iOS + Android app | I; Expo/RN plus native modules | I/V; Expo/RN host over Effect Native | D: owned local builds and physical-device gates | Close installed-device receipts; preserve no-EAS policy. |
| One shared application/component model | Separate mobile UI, shared runtime | **I/V/S**: Effect Native tree/intents/tokens | D: web/RN/native/canvas renderers | OpenAgents leads; do not copy T3's duplicate UI authority. |
| Local-first without account | Direct environment pairing; cloud optional | **I/V/S** | D | Preserve OpenAgents' explicit local-authority tier. |
| Multi-environment/host catalog | **I**: several known environments/endpoints | —: confirmed coding directory, not an any-host environment controller | D: any-host session/environment directory | Major parity gap; make metadata-first host/session directory the next shell. |
| QR/manual pairing to owner host | **I** | — in current app | D via enrollment/grant model | Add owner-managed enrollment after portable authority is live; use scoped grants/DPoP-equivalent proof. |
| Hosted relay discovery | **I** via Clerk/Cloudflare relay | Personal Khala Sync only | D via owned Google Cloud/OpenAgents control plane | Match convenience without Cloudflare or T3 trust topology. |
| Cached projection/reconnect | **I** shared client runtime | **I/V/S** exact cursor, refetch, lifecycle fencing | D | OpenAgents leads on explicit authority/fencing; expand to workroom domains. |
| Offline message intent | **I** durable per-environment outbox | Local/Sync queue and confirmed-command semantics exist, but no T3-equivalent visible multi-environment outbox | D | Add a user-visible durable pending-action/outbox projection across controller commands. |
| Thread catalog/create/continue | **I** | **I/V** confirmed threads and coding selection | D | Close native IA breadth and installed proof. |
| Archive/rename/delete thread | **I** | — | D only indirectly | Add typed lifecycle commands/outcomes; do not infer from chat mutation. |
| Provider/model/runtime selection | **I** | **I/V** explicit execution target catalog | D | OpenAgents target fencing is stronger; expand settings/readiness/account control. |
| Structured questions/approvals/plan review | **I** | **I/V/S** exact-ref durable decisions | D | Product polish/physical acceptance, not a foundational gap. |
| Queue/steer/interrupt/cancel | I, provider-dependent | Partial I/V: send/continue, cancel, resume, retry, close; pause absent | D: shared semantic command registry | Finish queue/steer/pause and expose durable terminal outcomes consistently. |
| Complete nested agent graph | Partial normalized activity | **I/V/S** confirmed graph and independent node detail | D | OpenAgents leads; retain exact topology/loss accounting. |
| Fleet run control | Status/threads, not OpenAgents Fleet | Partial I/V projection only | D: mixed run control, placement, attention | Add run creation/placement/stop/approval/batch controls after host catalog. |
| Repository onboarding/clone/project creation | **I** | —: consumes confirmed catalog | D: authorized repository selection/workroom creation | Major onboarding gap. |
| Files tree/read | **I** | — in current mobile source | D: Files mode | Major foreground gap. |
| Edit/save/conflict | Partial file surfaces | — | D: bounded edit/save/exact identity | Implement through brokered workroom capabilities, never raw paths. |
| Changes/diff/review comments | **I** native review | — | D: exact status/diff/comments/verification | Major foreground gap; first specialist host after files. |
| Branch/commit/push/PR | **I** typed server Git actions | — | D: safe branch/PR writeback with receipt | Match breadth with grant, snapshot, no-force, and exact post-image gates. |
| Interactive terminal | **I** native Ghostty VT, multiple sessions | — | D: bounded PTY/run/spawn | Major foreground gap; requires workroom capability, reconnect, and teardown proof. |
| Managed preview/ports | Server/web capability; mobile route breadth is less central | — | D: Preview mode/authenticated gateway | Build after PTY/process/port capability contract. |
| Artifacts/verification/receipts | Limited activity/diff state; no OpenAgents receipts | — mobile product surface | **D/S** explicit artifacts and receipts | OpenAgents differentiation; must ship with, not after, writeback. |
| Notifications/deep links | **I** registration, delivery, exact navigation | Partial I/V: response/deep-link consumption only | D: push attention loop | Add permission/token registration, backend binding, background receipt, and revocation. |
| Live Activities/widgets | **I** iOS agent activity | — | Draft/deprioritized | After push and honest live status; projection only, never completion authority. |
| Share target/quick actions | **I** | — | Not a core portability dependency | Add after new-task/repository flow exists; reuse normal intents. |
| Tablet adaptive workbench | **I** split sidebar/inspector | Minimal single-tree responsive host | D: compact progressive workbench | Build explicit compact/tablet mode layout with shared Effect Native navigation state. |
| Host stop/checkpoint/move/resume/failback | —; environment-local sessions | — | **D/S** PORT-03/04/06 | OpenAgents' strategic lead remains unimplemented; dependency before “any-host controller” claim. |
| Voice ASR/TTS/barge-in | — in T3 mobile controller | — | D: PORT-07 | Implement over same command/outcome registry after controller controls are complete. |
| Containment/grants/receipts | Weak/default-YOLO server posture | Partial substrate; mobile hides unsupported authority | **D/S** fail-closed observed enforcement | Never trade this advantage for T3-like speed. |
| Store/release status | Buildable/showcase-tested, README says undistributed | Owned identity/OTA; physical acceptance rungs remain distinct | D: owned iOS/Android release | Both require honest installed-product evidence. |

## 5. The real architectural gap

The code-count difference—approximately 75.3k T3 mobile lines versus 11.8k
OpenAgents mobile lines—is evidence of product breadth, not a target metric.
The deeper gap has three layers.

### 5.1 OpenAgents lacks the mobile workbench mode graph

The current app has one primary Effect Native home/transcript/drawer/composer
tree. T3 has native routes and adaptive pane roles for environment list,
project/task creation, thread, files, individual file, review, review comment,
terminal, Git overview/branch/commit/confirm, archive, connections, and settings.

OpenAgents needs a typed mobile navigation/workspace model whose state is part
of the Effect Native program:

```text
Attention / Recent / Repositories / Hosts
  -> selected portable session
       -> Thread
       -> Files
       -> Changes
       -> Terminal
       -> Preview
       -> Artifacts & Receipts
       -> Host / Movement
```

On phone, those are progressive routes/sheets. On tablet, the same model can
lower to list + detail + inspector panes. Route/pane state is presentation;
session/workroom state remains Khala Sync authority.

### 5.2 Designed remote capabilities are not yet consumable mobile services

Desktop/local code and roadmap prose contain file, editor, Git, workroom,
terminal, preview, and portable-session ideas. Mobile cannot consume prose or
desktop preload APIs. Each domain needs a generated, public-safe, owner-scoped
client contract and Effect Native service:

- `SessionDirectory` / `EnvironmentDirectory`;
- `PortableSessionControl`;
- `WorkspaceTree` / `WorkspaceDocument`;
- `ChangeSetReview` / `SafeWriteback`;
- `TerminalSession` / bounded process control;
- `ManagedPreview`;
- `ArtifactCatalog` / `ReceiptResolver`; and
- `AttentionInbox` / push registration and resolution.

Every service must distinguish cached, live, pending, accepted, rejected,
expired, revoked, stale-generation, and unavailable state. A tap is not a
completed move, writeback, or command.

### 5.3 Physical and fault evidence trails deterministic implementation

Current tests are valuable and green, but a full controller is dominated by
hostile lifecycle edges: background suspension, keyboard/inset change, network
loss, lost ACK, device restart, token rotation/revocation, workroom expiry,
terminal reconnect, preview expiration, stale diff, checkpoint transfer,
failed attach, and source-host reclaim.

OpenAgents cannot claim parity from fixtures alone. The exit must be one
installed iOS and Android journey against real owner-local and managed targets,
with exact fault receipts.

## 6. Parity target

“Parity with T3 mobile” should mean the user can complete the same useful
controller job, not that every T3 implementation choice is copied.

A parity-complete OpenAgents phone must let an authorized owner:

1. discover or enroll a host and see honest health/capability/isolation state;
2. select a repository and create or resume a coding session;
3. select an explicit account/model/execution target;
4. stream the complete agent/thread topology and answer every supported
   interaction;
5. queue, steer, interrupt, pause/resume, retry, close, and stop through durable
   commands;
6. browse/read/edit files with revision/conflict protection;
7. inspect the exact changes, comment, verify, and safely commit/push/open a PR;
8. attach to a bounded terminal and open an authenticated managed preview;
9. inspect artifacts, tests, usage truth, grants, and receipts;
10. receive and resolve an attention notification/deep link;
11. continue after background/offline/restart without duplicate submission;
12. use the same controller on compact phone and tablet layouts; and
13. do what T3 cannot: checkpoint and move the same portable session between
    accepted hosts with one live generation and fresh grants.

Voice and lock-screen presence are the last-mile controller multipliers. They
do not substitute for the workbench or movement core.

## 7. Dependency-ordered roadmap

This proposed delivery map composes with, rather than renames, PORT-03 through
PORT-08 and mobile waves M3–M7.

### Phase 0 — Freeze the parity contract and honest catalog

Deliver:

- one machine-readable capability matrix matching the table above;
- typed controller mode/navigation model for compact and expanded layouts;
- exact status vocabulary (`implemented`, `fixture_verified`, `live_proven`,
  `owner_accepted`);
- T3-derived mobile journeys translated into OpenAgents criteria; and
- explicit exclusions: no Cloudflare relay, EAS, GitHub-hosted CI, raw host
  paths, force push, default-open execution, or notification-as-authority.

Exit:

- every required surface has an owner contract, issue/work packet, oracle, and
  dependency; no planned surface is presented as implemented.

### Phase 1 — Any-host/session directory and mobile shell

Maps to PORT-01 consumption and PORT-06 shell work.

Deliver:

- metadata-first authorized environment/session directory;
- recent/attention/repository/host top-level navigation;
- host health, compatibility, isolation, access, freshness, capacity, and
  current attachment projection;
- session detail with complete agent graph and current workroom/target;
- typed deep-link resolver for session, host, approval, and receipt refs; and
- compact phone plus tablet list/detail/inspector lowering through Effect
  Native.

Exit:

- a fresh mobile install lists every authorized adopted session/host without
  exposing credentials or paths; stale/offline rows are visibly non-current;
  exact selection continues on Desktop.

### Phase 2 — Real portable-session movement

Maps to PORT-03 and PORT-04. This phase is substrate-first because UI cannot
honestly claim controls that the runtime does not perform.

Deliver:

- quiesce/checkpoint/detach/attach/move/resume/stop/reclaim state machine;
- exclusive attachment generation and graph-wide descendant fencing;
- secret-free portable checkpoint with exact repository/revision identity;
- fresh target grant redemption and source-host cleanup;
- owner-managed/homelab enrollment and capability advertisement; and
- command/outcome/receipt projection consumable by mobile.

Exit:

- mobile moves one real session from owner-local to owner-managed and back;
  exactly one generation executes; failures converge to resumed-source or
  failed-closed state; no secret-bearing checkpoint or orphaned process.

### Phase 3 — Thread controller completion and push attention

Maps to PORT-06 and M2/M6.

Deliver:

- explicit queue, steer, interrupt, pause, resume, retry, close, and stop;
- thread rename/archive/delete;
- user-visible durable command outbox with retry/expiry/conflict outcomes;
- complete account/model/quota/readiness presentation;
- push permission/token registration, device binding, revocation, background
  delivery, and exact attention resolver;
- notification tap and in-app tap invoking the same registered intent; and
- share/quick-action entry points reusing new-task/session intents.

Exit:

- a backgrounded phone receives a real approval/question/failure/closeout,
  opens the exact session, resolves it once, and converges with Desktop after
  lost ACK/restart/revocation.

### Phase 4 — Files and repository onboarding

Maps to M4 and the brokered workroom capability program.

Deliver:

- repository search/select/clone/bind and workroom creation;
- bounded tree/search/watch with opaque relative refs;
- source/Markdown/image viewer;
- bounded edit/save/save-as/rename/delete;
- exact revision, dirty, external-change, conflict, binary/large/secret/
  symlink/permission outcomes; and
- device-local draft/checkpoint recovery containing no host root or grant.

Exit:

- on iOS and Android, the owner selects a repository, changes a file, survives
  reconnect, resolves an induced conflict, and sees the same exact revision on
  Desktop.

### Phase 5 — Changes, verification, and safe writeback

Maps to M4.

Deliver:

- exact Git status identity and bounded typed diff;
- Effect Native review/file navigator plus renderer-private native diff host;
- line/range comments and reviewed-context attachment;
- discard/revert only against matching snapshot authority;
- verification/test artifact projection;
- create/switch branch, commit, push, and PR through no-force policies; and
- authority, execution, verification, delivery, and writeback receipts.

Exit:

- mobile completes one useful repository change and opens a PR with exact
  pre/post image, verification, grant, target, and receipt refs; a stale diff
  or revoked grant cannot mutate the repository.

### Phase 6 — Terminal, preview, artifacts, and specialist hosts

Maps to M5.

Deliver:

- bounded PTY create/attach/input/resize/clear/restart/terminate;
- output snapshot plus ordered stream and reconnect replay;
- explicit exit/unknown/expired/reclaimed states;
- managed port discovery and authenticated preview gateway;
- preview loading/error/expiry/reclaim;
- artifact/receipt browser; and
- typed Effect Native host contracts for terminal, diff, editor/Markdown, and
  preview, with platform fallbacks and accessibility.

Exit:

- a physical phone runs a bounded command, reconnects without duplicate input,
  opens and expires a managed preview, and resolves the exact terminal/
  preview/artifact receipts.

### Phase 7 — Fleet operations and managed-provider breadth

Maps to PORT-05/06 and M6.

Deliver:

- create/stop/checkpoint/resume Fleet runs;
- batch/swarm/placement controls;
- account priority, readiness, quota, and fallback policy;
- managed OpenAgents target plus one provider-neutral managed adapter;
- complete attention/approval/closeout inbox; and
- one controller path across owner-local, owner-managed, OpenAgents-managed,
  and provider-managed targets.

Exit:

- mobile supervises a mixed run, moves one eligible session, refuses one
  incompatible target with a typed reason, and shows exact placement/outcome
  receipts.

### Phase 8 — Voice, ambient status, and signed dogfood

Maps to PORT-07/08 and M7.

Deliver:

- explicit microphone lifecycle, ASR provisional/final transcript, TTS, and
  barge-in;
- voice follow-up/interrupt using the same typed command IDs and outcomes as
  text;
- no raw-audio retention by default;
- iOS Live Activity/widget and Android equivalent where supported, showing
  bounded observed status only;
- physical iOS and Android-emulator/device fault matrix; and
- signed owner-accepted cross-device/cross-host dogfood receipt.

Exit:

- one mobile-originated task moves owner-local → managed → failback, continues
  on Desktop, accepts one voice follow-up or interrupt, survives offline/lost
  ACK/update/restart/revocation/expiry, and closes with no state fork, duplicate
  execution, unsafe grant, silent downgrade, retained audio, false success, or
  orphaned source.

## 8. Cross-cutting implementation rules

Every phase keeps these rules:

1. **One application model.** New screens/modes extend Effect Native catalogs,
   intents, tokens, and renderer host contracts. They do not create a parallel
   React Native product model.
2. **Metadata first.** Cold/offline clients may show authorized cached rows but
   never infer live readiness or completion.
3. **Stable IDs and exact generations.** Every command names session, host/
   target, attachment generation, grant, command id, and expected state.
4. **Admission is not completion.** Pending, accepted, executing, settled,
   failed, expired, revoked, and conflicted remain distinct.
5. **Brokered remote authority.** Mobile receives opaque refs and bounded data,
   never raw roots, provider credentials, arbitrary ports, or unrestricted
   process handles.
6. **Containment remains separate.** Requested policy and observed effective
   enforcement are explicit; T3's default unrestricted posture is rejected.
7. **Receipts ship with consequential controls.** Move, terminal, preview,
   writeback, verification, and release do not defer evidence design.
8. **Owned infrastructure/release.** Google Cloud and owned runners/updates
   remain authoritative; no Cloudflare runtime, EAS, or GitHub-hosted CI enters
   the OpenAgents product path.
9. **Accessibility is a host contract.** Dynamic type, VoiceOver/TalkBack,
   keyboard, focus, reduced motion, contrast, touch target, terminal, diff, and
   tablet layout are verified on the real lowerings.
10. **Fixture, deployed, and physical evidence stay separate.** A green unit
    suite never closes a device/network/provider acceptance rung.

## 9. Verification program

T3's disposable multi-server screenshot harness is the useful shape to adapt,
with stronger OpenAgents fault and authority coverage.

Build one owned mobile controller harness that provisions:

- owner-local, owner-managed, and managed test environments;
- deterministic repositories with branches, conflicts, diffs, binary/large/
  secret-shaped exclusions, scripts, and preview ports;
- seeded current and stale sessions, nested agents, approvals, questions,
  queued commands, artifacts, and receipts;
- a controllable network proxy for disconnect, delay, duplicate, reorder, and
  lost ACK;
- token/grant revoke and workroom expiry/reclaim controls; and
- real iPhone/iPad plus Android phone/tablet route capture.

Required oracle families:

| Oracle | Minimum proof |
| --- | --- |
| Navigation/adaptive | Same typed mode state lowers correctly on compact phone and expanded tablet; deep links restore exact refs. |
| Sync/offline | Cached state is labeled, sparse sequence forces refetch, outbox drains once, superseded generation cannot apply. |
| Session control | Every consequential command reaches one durable terminal outcome or explicit expiry/revocation/conflict. |
| Movement | One live attachment generation, exact checkpoint, fresh grants, source cleanup/failback, no orphan. |
| Workspace | Opaque relative refs, revision fencing, secret/binary/symlink/size policy, conflict-safe save. |
| Git/writeback | Exact snapshot binding, no force push, verification and post-image receipt. |
| Terminal/preview | Bounded resources, reconnect replay, explicit exit/expiry, authenticated port only. |
| Push/ambient | Registration/revocation, exact attention resolution, no status-to-completion promotion. |
| Accessibility | VoiceOver/TalkBack, dynamic type, keyboard/focus, touch targets, reduced motion, terminal/diff labels. |
| Release | Local owned build/update/install/restart/rollback on supported iOS/Android targets. |

## 10. What parity must not erase

T3 demonstrates that controller completeness is now table stakes. OpenAgents'
reason to exist is not a thinner clone with a different visual system.

OpenAgents should reach T3's breadth while retaining four differentiators:

1. **portable session identity** — access any session on any authorized host
   and move it, rather than merely reaching the same environment remotely;
2. **authority and containment truth** — know what the agent was allowed to do
   and what enforcement actually ran;
3. **durable outcomes and receipts** — consequential mobile taps resolve to
   exact evidence, not optimistic UI or transcript prose; and
4. **one Effect Native application contract** — phone, tablet, Desktop, web,
   native, and future canvas hosts share semantics without duplicating the
   product model.

## Final recommendation

Do not spend the next mobile cycle refining the current single conversation
surface as if presentation were the primary gap. Freeze its working identity,
Sync, exact-ref controls, target selection, agent graph, and accessibility
contracts. Then advance the already-authorized portable-session program in
this order:

1. any-host/session directory and adaptive controller shell;
2. real checkpoint/move/resume/failback substrate;
3. complete thread controls plus push attention;
4. repository onboarding and Files;
5. Changes, verification, and safe PR writeback;
6. Terminal, Preview, Artifacts, and Receipts;
7. Fleet/managed-provider breadth; and
8. voice, ambient status, and signed cross-host dogfood.

That sequence brings OpenAgents to T3-level mobile usefulness without copying
T3's duplicated UI stack, hosted-infrastructure choices, environment-local
session model, weak containment posture, or lack of receipts. The result is the
product the existing OpenAgents design already promises: a full mobile
controller for any authorized coding session on any accepted host.

## 11. Implementation update — MOBILE-PARITY-03

The first bounded attention/return packet from phase 3 landed on 2026-07-17 as
[#8950](https://github.com/OpenAgentsInc/openagents/issues/8950) with leaves
[#8951](https://github.com/OpenAgentsInc/openagents/issues/8951),
[#8952](https://github.com/OpenAgentsInc/openagents/issues/8952), and
[#8953](https://github.com/OpenAgentsInc/openagents/issues/8953).

The server now projects a body-free `runtime_attention` entity into the
authenticated owner's personal Sync scope in the same transaction as the full
thread-private interaction. The confirmed client inbox rejects malformed,
cross-owner, and mismatched rows, separates pending from terminal state, and
never promotes local optimistic data. Mobile resolves an exact
attention/thread/turn tuple against that inbox before navigation.

The authenticated mobile Sync host watches this inbox, and the Effect Native
controller's Attention destination renders pending questions, approvals, and
plan reviews without copying prompt bodies into the personal directory. An
in-app row, `openagents://attention/...` deep link, and bounded notification
payload all converge on the same registered `ControllerAttentionSelected`
intent. Terminal or unknown attention cannot remain actionable; a successful
selection opens the existing thread-scoped interaction detail.

This closes only the deterministic source packet. Expo permission and token
registration, server-side physical notification delivery, restart/device
receipts, thread lifecycle/share controls, and the remaining phase-3 control
breadth are still gaps. Nothing in this update proves installed-device push or
the full T3 mobile controller journey.

## 12. Implementation update — MOBILE-PARITY-03D

The remaining confirmed thread-lifecycle slice of phase 3 landed on 2026-07-17
as [#8954](https://github.com/OpenAgentsInc/openagents/issues/8954), with
authority, reconciliation, and Effect Native leaves
[#8955](https://github.com/OpenAgentsInc/openagents/issues/8955),
[#8956](https://github.com/OpenAgentsInc/openagents/issues/8956), and
[#8957](https://github.com/OpenAgentsInc/openagents/issues/8957).

Chat threads now carry explicit `active`, `archived`, and terminal `deleted`
states. The server admits only active-to-archived, archived-to-active, and
active/archived-to-deleted transitions against the exact confirmed status and
`updatedAt` baseline. Non-active threads cannot accept new messages, repository
bindings, or continuity changes. Delete retains an owner-private metadata
tombstone; it does not copy message bodies into the personal directory.

The mobile adapter separates active and archived confirmed catalogs and waits
for a newer matching personal-scope post-image before reporting completion.
An overlay-only mutation, stale baseline, or reconnect timeout remains pending
or conflicted. The Effect Native drawer exposes inline rename, archive, restore,
and a deliberate two-step permanent-delete control with disabled pending states
and local conflict feedback. Confirmed archive/delete removes the active thread
and composer from navigation.

This closes the lifecycle-controls part of phase 3, not all mobile parity.
Share/quick actions, physical push registration and delivery, installed-device
acceptance, the remaining command breadth, and phase 4 repository Files remain
explicit gaps. The next honest epic is phase 4 Files/repository onboarding
unless the remaining share/push work is deliberately reprioritized first.
