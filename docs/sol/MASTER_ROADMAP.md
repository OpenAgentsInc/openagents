# MASTER ROADMAP — reliable synced coding and fleet software on Desktop/mobile

- Class: authority
- Date: 2026-07-10
- Updated: 2026-07-12
- Revision: 88
- Status: canonical OpenAgents implementation roadmap
- Supersedes: [`docs/fable/MASTER_ROADMAP.md`](../fable/MASTER_ROADMAP.md)
- Dispatch: yes, together with live issues and
  [`CLAIM_PROTOCOL.md`](./CLAIM_PROTOCOL.md)
- Pre-compaction source: Revision 86 at `4239689e24`
- Issue-source index: [`issues/README.md`](./issues/README.md)
- Documentation cleanup: SOL-DOC-01 through SOL-DOC-04 are closed.
  SOL-DOC-04 [#8726](https://github.com/OpenAgentsInc/openagents/issues/8726)
  compacted this authority from 2,189 to 557 lines at `6bfe97fddb` without
  changing product proof. SOL-DOC-05 normalizes receipts and closed issue
  sources next. See the
  [`cleanup ledger`](./2026-07-12-documentation-cleanup-audit-and-retirement-plan.md).

This file owns current product direction, durable gates and laws, the current
issue projection, and dependency order. It does not repeat implementation
diaries. Current code, tests, live issues, deployments, and receipts own factual
proof; dated plans and analyses are evidence only.

## Owner decisions

1. **Reliable Desktop/mobile fleet software is P0.** One trustworthy system
   carries the same authenticated identity, conversations, coding sessions,
   FleetRuns, work, attempts, approvals, commands, outcomes, and receipts over
   Khala Sync. Owners can start, inspect, steer, approve, pause, resume, stop,
   and recover without guessing which device or cache owns reality.
2. **Sarah is removed, not the front door.** Named-persona, relationship-first,
   avatar, opener, Sarah/persona voice, video, and presentation backlog is
   closed not-planned. `/sarah/*` is tombstoned, the Sarah app/surface is
   deleted, and retired behavior contracts remain evidence. The server-side
   `/api/sarah/fleet-runs` intake remains a temporary typed authority adapter
   until an explicit rename/deletion gate closes; it does not authorize Sarah
   UI or product sequencing. Persona-neutral voice is governed by decision 21.
3. **Desktop and mobile are the active clients.** Web remains a supported
   public/API/operations surface; landing, Forum, portal, persona, and broad
   route-conversion expansion does not preempt the reliability program.
4. **Retained application UI uses Effect Native.** Web, mobile, Desktop, and
   canvas share typed components and intents. Electron, React Native/Expo, DOM,
   native, and canvas are hosts/renderers, not parallel application models.
5. **Khala Sync is cross-device authority, not chat transport.** It distributes
   owner-scoped projections and durable mutation outcomes. Local stores are
   caches/offline queues except inside the explicit device-local identity tier.
6. **Sol owns this roadmap.** Fable is adversarial strategic evidence; its
   roadmap and pre-reset queue are historical.
7. **Desktop and mobile are greenfield.** The destinations are
   `apps/openagents-desktop` (Effect Native/Electron) and
   `apps/openagents-mobile` (Effect Native/React Native/Expo), both named
   `OpenAgents`; mobile uses `com.openagents.app`. Deprecated clients are
   extraction/migration sources, not conversion targets. The owner-designated
   mobile icon remains the exact
   `clients/khala-mobile/assets/images/icon.png` bytes with SHA-256
   `0a1865ac6d1efc792d365d9a37af9e6ffa3270fa7c8731f36129f35371bfc7ce`.
8. **Terra is an execution lane under Sol.** Terra may claim ready,
   low-collision vertical slices. Sol owns priority, dependency order,
   hot-contract integration, and roadmap reconciliation. Current assignment
   comes from live issues and claims, not a cached lane in prose. The durable
   lane contract is
   [`2026-07-10-terra-execution-lane.md`](./2026-07-10-terra-execution-lane.md).
9. **Desktop reaches practical OpenCode parity.** Project/session navigation,
   streamed agent work, composer/context, files/editor/review/terminal,
   commands, providers/models/MCP/permissions, settings, diagnostics,
   lifecycle, and distribution are required. This is behavior parity, not a
   code or pixel clone; multi-window depth and WSL may follow.
10. **Reliability is proven cross-device.** Unit tests or a one-device demo do
    not satisfy restart, reconnect, offline catch-up, duplicate suppression,
    conflict/refetch, real Desktop-to-mobile continuation, and mixed-fleet
    receipts.
11. **The useful Khala Code MVP capability set moves into mobile.** Port
    behavior, contracts, and test vectors for auth/repositories, bound threads,
    rich turns, targets, steering, approvals, files, changes, artifacts,
    terminal, preview, push, handoff, fleet state, and release QA—not its legacy
    component tree or authority shortcuts.
12. **Remote workrooms are a mobile MVP dependency.** Phone coding uses an
    owner-scoped workroom/Agent Computer through typed lifecycle, file,
    process, preview, network, grant, writeback, and receipt APIs. The phone
    never gains raw local filesystem, shell, credential, or execution
    authority. #8547 and #8636 are P0 integration lanes.
13. **Desktop is tokenless over one host-owned Runtime Gateway; mobile joins
    at D1.** The signed renderer receives bounded projections and emits closed
    typed intents. Identity, credentials, raw runtime events, workspace/process
    handles, and Sync SQLite stay in the host. The first real conversation
    slice includes mobile continuation before broad D3–D6 depth.
14. **There is one program parent.** #8566 owns R0–R7; #8574/#8597 are client
    tracks; #8547/#8636 are bounded remote-workroom tasks. Closed substrate,
    proofs, and `wontfix` records are not dormant epics. New defects or owner
    outcomes receive new bounded issues.
15. **Loss-accounted parent/subagent history is a D1 exit.** Supported source
    items render exactly once or enter an explicit redaction/gap equation.
    Parent/child/grandchild topology, independent transcripts, tools, errors,
    usage, and final outcomes remain inspectable without an age ceiling.
    Provider-native history stays owner-local and out of Khala Sync until
    explicitly adopted through a typed projection. Its detailed contract is
    [`desktop-codex-subagent-history.md`](./issues/desktop-codex-subagent-history.md).
16. **Predictable visible behavior is a release contract.** Stable recent
    sessions, bounded fast paint, complete paging, loading/failure/privacy,
    accessibility, restart, and real host/device journeys require executable
    UX contracts. Descendants never leak into the top-level root catalog.
    Screenshots and videos demonstrate; they do not prove.
17. **Effect service topology is explicit.** Process, WorkContext,
    conversation/run, request/command, and foreign-host/view scopes declare
    direction, freshness, replacement, and disposal. Promise/native/provider
    bridges remain perimeter code; interruption remains cancellation.
18. **Coding sessions are host-independent.** Durable session, WorkContext,
    command, event, agent, and receipt identities never derive from machine,
    process, path, Pylon home, or cloud vendor. V1 promises fenced
    stop/checkpoint/detach/attach/resume, not live process-memory migration.
19. **Owner and managed cloud are peer target classes.** Owner-managed nodes,
    Agent Computers, and audited provider adapters use one lifecycle,
    isolation, snapshot, preview, cleanup, fallback, and receipt contract.
    Clients never call vendor control planes or silently change custody,
    provider, account, data posture, or isolation.
20. **Cross-host work uses a general capability broker.** Provider, SCM,
    MCP/tool, and API credentials stay in approved custody and reach one
    owner/session/attachment/target/tool/TTL scope through short-lived leases
    or gateways. Moves revoke source grants and mint fresh target grants;
    checkpoints, Sync, logs, prompts, and receipts never carry secrets.
21. **Mobile reaches every authorized session and supports conversational
    voice.** The owner-scoped directory exposes targets, attachment,
    capabilities, freshness, isolation, topology, and durable controls.
    Explicit persona-neutral ASR/TTS/barge-in uses the same typed command,
    approval, outcome, and receipt path as text with explicit visible
    microphone state. TTS, ASR hypotheses, model prose, and playback never
    prove a command or outcome. No ambient recording, raw-audio retention by
    default, voice-only authority, Sarah, avatar, or video.
22. **Agent topology is a live operating surface.** A child-start edge appears
    at its causal parent with exact identity, lifecycle, and bounded redacted
    activity; the complete graph stays navigable and every child opens its own
    transcript. Pointer and conflict-safe hotkeys use the same intents. Replay,
    reconnect, and movement never flatten, re-root, duplicate, or leave an old
    child accepting work.

## Product and authority model

**OpenAgents is remote-first software for coding and fleet work from Desktop
and mobile: one durable session can execute on, stop on, and move between
authorized local, owner-managed, and managed hosts without forking identity,
state, authority, secrets, topology, or receipts.**

```text
intent -> policy -> orchestration -> execution
  ^                                  |
  |                                  v
state <- Khala Sync <- evidence <- durable outcome

mobile <------------ same authority ------------> Desktop
```

| Layer | Canonical responsibility |
| --- | --- |
| Identity | Authenticated owner/org/device/account/capability scopes plus an explicit device-local tier |
| Comprehension | Khala inference, semantic selectors, typed tools, and Blueprint drafts |
| Control | Owner scope, policy, budget, approvals, registered intents, and idempotency |
| Orchestration | Fleet planning, routing, claims, Pylon, target and harness selection |
| Execution | Codex/Claude/provider workers on authorized local or remote targets |
| Evidence | Verification, exact or explicitly unmeasured usage, and receipts |
| Continuity | Khala Sync, durable events/cursors, Blueprint/provenance, checkpoints, and resumable work |

Desktop is the high-density local/remote coding workbench and Fleet cockpit.
Mobile is a compact remote coding, attention, control, review, and continuity
client; it can finish useful repository work without first opening Desktop but
never receives raw device/host authority. Web hosts required public, API,
auth, promise, receipt, health, legal, and operations surfaces; it is not a
third active product-expansion queue. Retained product routes are `/`,
`/forum`, and `/promises`; infrastructure exceptions include privacy/terms,
auth callbacks, APIs, assets, health, manifests, and receipt/promise-integrity
routes. `/sarah/*` remains a tombstone.

## Non-goals and non-revival boundary

Closed not-planned unless an owner decision explicitly reopens them:

- Sarah UI/front door, persona/relationship/role/named-colleague expansion;
- avatar, opener, pre-rendered clips, Sarah/persona voice, realtime video,
  media cache/admission, and presentation-quality experiments;
- optional glass/visual lowering not required for accessibility, interaction
  correctness, platform support, or an active reliability gate;
- broad landing, Forum, portal, CRM, sales, and route-conversion expansion;
- named-assistant standing responsibilities, Blueprint-as-company-brain
  maturation, role/template expansion, in-conversation payment, and broad
  assurance/connector/network programs without new bounded owner decisions;
- literal OpenCode cloning, new local fleet/sync authority, live process-memory
  migration, silent provider/target fallback, or clients calling vendor APIs;
- a parallel app-local React Native/SwiftUI/shadcn/Zod/oRPC/TanStack
  application architecture outside Effect Native host/renderer boundaries.

Security, privacy, data-loss, accessibility, production-outage, active-cost
removal, promise/API integrity, and supported compatibility repairs remain
allowed as new bounded issues. Proven contracts and tombstones are not deleted
merely to express closure. Retired UI never regrows through a compatibility
adapter.

## Durable acceptance gates

| Gate | Required scope | Exit evidence |
| --- | --- | --- |
| R0 — truthful green foundation | Clean Desktop/mobile typecheck, tests, builds, capability manifests, and explicit unconfigured/offline/reconnecting/stale/refetch/failed/ready states | Neither fixture, cache, transcript, nor optimistic UI is presented as authority |
| R1 — identity and session | One owner/org scope, device registration, stable conversation/project/session refs, readiness, refresh/revocation, and persona-neutral bootstrap | Both clients see the same authorized catalog; revoke either device; no secret/private payload crosses renderer or public evidence |
| R2 — Khala Sync continuity | Canonical projections/outcomes, monotonic cursor/version, tombstones, bounded caches, idempotency, conflict/refetch | Cross-device change plus restart reconstructs matching refs/versions without duplicates or invented completion |
| R3 — Fleet, targets, workrooms | Named worker/account/target, workroom lifecycle, one claim registry, approvals/controls, explicit quota/policy/isolation/fallback, usage and receipts | One real Codex+Claude fleet is controlled from both clients; owner-local, owner-managed remote, and managed targets use one registry; every command has one durable outcome with no silent provider/target/isolation substitution |
| R4 — interruption and movement | Offline queue, lost ACK, replay/order faults, restart/revocation, host-independent identity, stable agent graph/cursors, generation-fenced attachment, secret-free checkpoint, detach/attach/move/failback | Fault injection and real movement produce no lost accepted intent, double execution, two live generations, orphaned child, topology corruption, leaked secret, false success, or indefinite spinner; stale clients/hosts converge or fail closed |
| R5 — Desktop workbench | D0–D6: sessions/topology, composer, projects/files/editor/Git/PTY, commands, providers/models/MCP/permissions, settings/diagnostics, Fleet, lifecycle/distribution | Everyday OpenCode-parity workflow completes in hardened Effect Native/Electron and survives restart/reconnect |
| R6 — mobile any-host coding | Directory/topology, repositories/threads, rich turns, targets, compact files/changes/terminal/preview/artifacts, writeback, Fleet controls, push/deep links, stop/checkpoint/move/resume, text and voice | Physical iOS plus Android emulator—nothing gates on physical Android—access every authorized enrolled-host session, inspect causal child activity and its transcript, use text or voice to follow up/interrupt and request a move, inspect/change/review/run/preview/write back, and supervise R3 without raw filesystem, shell, vendor, credential, or voice-only authority |
| R7 — release and dogfood | Signed/recoverable Desktop, iOS/Android artifacts, schema/migration/rollback, diagnostics, Sync/graph/attachment/broker/command/reconnect telemetry | Sustained cross-host/Desktop/mobile dogfood exercises voice, secret revocation, update/restart/offline/lost-ACK, move/failback, and reclaim without fork, duplication, leak, orphan, data loss, or false authority |

Desktop depth remains seven explicit acceptance gates:

| Gate | Scope | Exit |
| --- | --- | --- |
| D0 | Hardened predictable truthful shell, manifest/capability agreement | Clean typecheck/tests/bundle plus isolated real-Electron smoke |
| D1 | Real stream, loss-accounted history, canonical live topology, independent transcripts, composer/interactions, early mobile continuation | Real stream continues on physical mobile with matching refs/versions/cursor/phase/outcome and reconnect creates no duplicate, orphan, or inaccessible child |
| D2 | Stable projects/sessions, commands/keybindings, catalogs/restore, enforced Effect topology | Pointer/keyboard actions are equivalent; restart restores exact selection; topology oracle rejects forbidden edges |
| D3 | Grant-bounded workspace/files/editor/Git/review/PTY/preview | Conflict-safe restart/edit/diff/context/terminal journey preserves bounds and post-image |
| D4 | Providers/models/reasoning, MCP/plugins/skills, permissions, settings, diagnostics, isolation/extensions | A real host mutation follows typed policy with no renderer secret or ambient authority |
| D5 | Server-authoritative Fleet attention/control/outcomes/receipts | Desktop/mobile show matching state and durable controls converge under replay/reconnect |
| D6 | Fuses, signing/notarization, install/update/rollback/recovery, legacy lockout, support diagnostics | Clean machine completes install, first run, update/interruption/resume, rollback/reinstall, uninstall, and support export |

### Desktop host boundary

- The Runtime Gateway is private host composition, not a public server, second
  Pylon, claim registry, run database, or state authority.
- Electron keeps sandbox/context isolation, `nodeIntegration: false`,
  restrictive CSP, deny-by-default permission/navigation/window-open policy,
  origin/sender validation, and fixed schema-decoded least-authority preload
  methods.
- CPU-heavy history/watch, PTY, engine supervision, and executable extensions
  move behind one utility process before broad D3/D4 use without changing the
  renderer protocol. Monaco/editor and terminal depth are typed foreign-host
  nodes, never generic renderer filesystem/process authority.

## Identity, Sync, workroom, and movement contracts

### Two-tier identity

1. **Device-local identity is the no-login default.** An immutable local
   identity and local store can own purely local work and pairing. It cannot
   assert a hosted owner, enter hosted transport, or label local rows
   server-confirmed.
2. **OpenAgents account is an opt-in link.** Verified sign-in links rather than
   replaces local identity, enables cross-device/hosted authority, and is
   reversible. Sign-out never deletes local work.

The binding R1/R2 detail is
[`khala.identity_sync_contract.v1`](./2026-07-10-r1-r2-identity-sync-contract.md).

### Khala Sync laws

- Owning services decide claims, attempts, custody, approvals, command
  acceptance, money, credentials, and terminal outcomes. Sync distributes
  typed projections/results; cache state never promotes itself.
- Every mutation carries owner scope, stable target ref, version or explicit
  commutative semantics, idempotency key, and durable typed outcome. Timeout is
  `unknown_pending_reconcile`, never success or automatic unsafe replay.
- Reconnect uses ordered cursors and bounded gap detection. Missing retained
  history produces `must_refetch`, never transcript inference or last-write-
  wins on authority fields.
- Optimistic/local state is explicit and reversible. Approval, stop, payment,
  claim, credential, and destructive actions are not committed before
  authoritative acknowledgement.
- Desktop/mobile share one Effect Native domain, identifiers, enums, errors,
  Sync semantics, intent IDs, and outcome grammar even where host capabilities
  differ.

### Remote workrooms and portable sessions

Keep four facts separate:

1. a client continues a synchronized conversation;
2. a work unit is placed on a selected target;
3. a workroom survives restart on one host;
4. an execution session is quiesced, checkpointed, fenced off one host, and
   attached on another.

#8547 proves the first managed Agent Computer/workroom, including authorized
capacity, usage, bounded workspace/writeback, verification, and reclaim.
#8636 then proves one claim registry across owner-local and managed placement.
Routing cannot be accepted around an unaccepted target.

Portable movement additionally requires:

- host-independent `coding_session`, WorkContext, graph, and event identity;
- at most one generation-fenced attachment accepting new work;
- content-addressed, secret-free checkpoints carrying durable events,
  repository post-image/diff, catalog generation, approvals, artifacts, and
  receipt refs—not credentials, homes, handles, memory, PTYs, sockets, PIDs, or
  provider-hidden state;
- owner-managed enrollment plus managed target adapters behind one target
  descriptor and no client vendor API;
- capability leases scoped to owner/session/attachment/target/tool/TTL, source
  revocation, target reauthorization, cleanup, and receipts;
- typed stop/checkpoint/detach/attach/move/abort/failback outcomes that preserve
  parent/child topology and per-thread cursors.

The detailed gap analysis is
[`2026-07-11-remote-first-portable-coding-sessions-pathway.md`](./2026-07-11-remote-first-portable-coding-sessions-pathway.md).

## Mobile capability contract

The complete extraction ledger is
[`2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md`](./2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md).
Every legacy capability is ported, replaced by a stronger contract, or rejected
with a reason.

| Group | Required result |
| --- | --- |
| Identity/repositories | Sign in or remain local, select repository, bind/resume thread, revoke safely |
| Conversation | Rich reasoning/text/tool/usage/file/writeback events plus queue/steer/interrupt/recover survive background, restart, reconnect, and target handoff |
| Accounts/targets | Named readiness and policy; no default account or silent provider/target/isolation fallback |
| Workroom | Typed create/resume/stop/reclaim, files, processes/PTY, preview, network policy, snapshots, artifacts |
| Review/writeback | Exact files/diff/verification/post-image and safe branch/PR writeback; no force |
| Fleet/attention | Same run/work/attempt/approval/control/outcome/receipt refs and versions as Desktop |
| Quality/release | Clean builds, accessibility, crash/connectivity, migration/recovery, physical iOS and Android-emulator proof |

## Proof vocabulary

Every claim uses the narrowest true rung:

1. **code-landed** — source is on `main`;
2. **fixture-proven** — bounded deterministic tests/models/fixtures pass;
3. **deployed/distributed** — intended artifact/config is verifiably present;
4. **live-proven** — a real target path produced the named receipt;
5. **owner-accepted** — the owner reviewed and accepted live behavior;
6. **closed** — issue exit, residuals, docs, and duplicate-path deletion are
   reconciled.

No rung implies the next. A timeout, build upload, deployment, screenshot,
worker closeout, or polished UI never manufactures live or owner acceptance.

### Cutover proof boundaries

| Gate | State and meaning |
| --- | --- |
| C0 | Closed historical implementation baseline for the durable Fleet seam |
| C1 | Closed/deployed minimum-safe run, claim, reconnect, and supervision substrate |
| C2 | Closed #8640 simultaneous named Codex+Claude mixed-account proof; substrate only, never client cutover or Sarah authority |
| C3 | Open product integration boundary: #8547 accepted managed workroom before #8636 owner-local plus managed-cloud receipt under one claim registry |

R7, not C2, is owner-facing software cutover. The historical C0–C3 evidence
and fallback rules remain in the
[`fleet cutover analysis`](./2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md).

## Current implementation and issue truth

Snapshot: `origin/main` at `4239689e24` plus live issue state through the latest
SOL-DOC-04 refresh. Refresh live issues again before dispatch.

- The foundational local coding graph through CUT-25 is landed/closed except
  CUT-09's distinct lifecycle acceptance. CUT-16 is closed with physical-iOS
  VoiceOver; CUT-25 is closed with cross-client Fleet control.
- CUT-26 published signed/notarized/stapled `0.1.0-rc.1`, whose installed
  artifact exposed a V8 snapshot-fuse boot failure. The fix is on `main`; the
  next monotonic release candidate must finish packaging/notarization and pass
  clean-machine first run, named-account readiness, a real coding smoke,
  update/interruption/resume, rollback/reinstall, uninstall, and diagnostics
  export.
- CUT-27 closes the bounded ordinary Codex/Claude-to-OpenAgents cutover only
  after its open dependencies and exception register reconcile. It does not
  manufacture remote-workroom, portability, broker, any-host, or voice proof.
- #8676 is ready for one real named-Codex Desktop conversation continued under
  the same refs on physical iPhone plus sign-out/revocation denial. #8677
  retains fault-convergence proof and is coupled to CUT-09's remaining literal
  physical network-gap/revocation acceptance.
- #8547 owns the first accepted brokered Agent Computer/workroom; #8636 follows
  with live hybrid local/managed routing. Before that live acceptance, #8636
  may still compose its managed-cloud runner into intake/activation, remove the
  hard unconfigured placeholder, persist routing/fallback history through
  Sync, project capacity/claims, and enforce per-work-unit quota/cost/data-
  posture constraints. Portability, general broker,
  owner-managed enrollment, provider-adapter breadth, any-host directory, and
  voice need bounded leaves rather than silent expansion of those issues.
- Closed #8640 remains the accepted simultaneous named Codex+Claude substrate
  receipt; it is not current work or a product-front-door decision.
- `apps/pylon/src/orchestration` and `apps/pylon/src/node` remain protected
  load-bearing Fleet core during the open correctness proof. Streamlining is a
  separately bounded post-proof program under the
  [`Pylon streamlining audit`](../fable/2026-07-11-pylon-streamlining-audit.md).

### Canonical open issue projection

| Issue | Current role |
| --- | --- |
| [#8566](https://github.com/OpenAgentsInc/openagents/issues/8566) | Sole R0–R7 program parent |
| [#8574](https://github.com/OpenAgentsInc/openagents/issues/8574) | Desktop D0–D6/R5 track |
| [#8597](https://github.com/OpenAgentsInc/openagents/issues/8597) | Mobile R0–R7/R6 track |
| [#8547](https://github.com/OpenAgentsInc/openagents/issues/8547) | First real brokered Agent Computer/workroom |
| [#8636](https://github.com/OpenAgentsInc/openagents/issues/8636) | One claim registry across local/managed routing |
| [#8676](https://github.com/OpenAgentsInc/openagents/issues/8676) | Real streamed Desktop-to-physical-mobile continuation |
| [#8677](https://github.com/OpenAgentsInc/openagents/issues/8677) | Command/event/lifecycle fault convergence |
| [#8689](https://github.com/OpenAgentsInc/openagents/issues/8689) | CUT-09 remaining lifecycle acceptance |
| [#8706](https://github.com/OpenAgentsInc/openagents/issues/8706) | CUT-26 distribution/update/rollback/legacy lockout |
| [#8707](https://github.com/OpenAgentsInc/openagents/issues/8707) | CUT-27 local coding cutover declaration |

Closed `wontfix`/not-planned tombstones include #8595, #8610, #8634, #8635,
#8642, #8643, #8646, and #8650. They are not dormant queues.

## Current execution order

Live issues and claims control exact selection. At this snapshot:

1. In parallel, run the combined physical-iPhone journey for #8676 and CUT-09:
   same-ref named-Codex continuation,
   literal offline/network gap, exactly-once reconnect, then unlink/sign-out/
   revocation denial without replay. Complete CUT-26's next monotonic RC and
   clean-machine lifecycle independently.
2. Close CUT-09, then #8677 when its full matrix/live rung reconciles; close
   #8676 only on its literal continuation/denial receipt. Close CUT-27 last for
   the local cutover after #8676/#8677/CUT-09/CUT-26 and the exception register
   are satisfied. On the clean installed candidate, CUT-27 still proves one
   non-trivial named-Codex and one named-Claude repository task through
   project/session, composer/context, questions/approvals, edits, tests/
   preview, Git review, and durable terminal receipts; physical-iOS plus
   Android-emulator continuation/control/interruption; accessibility, privacy,
   security, provenance, and rollback evidence; and the product/runbook change
   making OpenAgents Desktop the default local surface for the proven scope.
3. Continue disjoint #8636 source/persistence/capacity work, but complete
   #8547's accepted managed workroom before #8636's live hybrid-routing receipt.
   Keep metering, target custody, quota/cost/data posture, usage, writeback,
   reclaim, and fallback explicit.
4. File bounded leaves for portable-session attachment/checkpoint authority,
   general capability broker, owner-managed targets, first audited provider
   adapter, any-host directory, and persona-neutral mobile voice before
   mutation. Serialize shared schemas, migrations, command IDs, catalogs, and
   policy.
5. Compose those substrates into remaining Desktop R5, mobile R6, and R7
   dogfood: local→managed→owner-managed movement, stable topology/cursors,
   cross-device control, voice follow-up, revocation, lost ACK, restart/update,
   failback, and reclaim with no fork, duplication, leak, orphan, or false
   authority.

Owner or external gates shift capacity to another ready R0–R7 slice, never to
closed presentation backlog. Claims are refreshed before every mutation.

## Implementation laws

1. **Integrate before inventing.** Existing FleetRun, plan DAG, intents, Sync,
   assignments, and receipts must be shown insufficient before new schemas.
2. **One claim registry.** No local/cloud/provider/harness path owns a private
   work universe.
3. **Named accounts and targets.** Automation never uses a default provider
   home or silently substitutes custody, target, model, or isolation.
4. **Authority stays distributed.** Clients present/request; owning services
   decide execution, credentials, money, policy, and terminal outcomes.
5. **Private evidence, bounded projections.** Raw work stays on its owning
   plane; completion remains independently verifiable.
6. **Effect Native for retained UI.** Shared components/intents move upstream;
   hosts/renderers are capability boundaries. Deletion beats conversion for
   retired surfaces.
7. **Desktop parity is an exit.** Placeholder panes, a secure shell, or a
   Fleet-only cockpit do not satisfy #8574.
8. **One action contract.** Desktop/mobile/automation use the same registered
   intents, policy, approvals, idempotency, and durable outcomes. Prose/pixels
   are never authority. Material action/tool selection never uses ad hoc
   string/keyword routing; it goes through the central typed/semantic registry.
9. **No surface regrowth.** New active client capability lands in Desktop or
   mobile; web receives required API/public/operations work only.
10. **Motion has integration bias.** A blocked owner/cloud gate shifts to a
    disjoint ready gate, not speculative framework or closed presentation work.
11. **Claims are explicit.** Live issue comments are the cross-session ledger;
    root coordinates same-session work. Name hot files/contracts and never
    steal on elapsed time alone.
12. **Proof rungs never collapse.** Code, fixture, distribution, live, owner,
    and closure are reported separately.
13. **Challenges retain falsifiers.** Material strategy disagreements keep an
    owner, tripwire, review point, and disposition in
    [`CHALLENGE_LEDGER.md`](./CHALLENGE_LEDGER.md).
14. **Terra pulls ready leaves.** It may ship claimed low-collision slices; Sol
    retains shared-contract integration and roadmap reconciliation.
15. **Mobile is remote workbench, not local-authority clone.** Its coding,
    terminal, preview, writeback, fleet, receipts, and handoff use bounded
    workroom capabilities and phone-native navigation.
16. **Timeout is not outcome.** Accepted, rejected, failed, and
    `unknown_pending_reconcile` are durable distinct states; retries follow
    idempotency and evidence.
17. **Renderer is not runtime client.** No credential, raw stream/process,
    generic IPC, loopback authority, or second state universe enters it.
18. **Mobile continuity is an early latch.** The first complete real Desktop
    conversation continues on mobile before broad D3–D6 is called the product
    path.
19. **History completeness is measured.** Every supported item is rendered,
    explicitly redacted, or an explicit counted gap; paging is not retention.
20. **UX promises are executable gates.** Stable behavior contracts plus real
    host/device journeys cover paint, loading, ordering, accessibility,
    failure, restart, and privacy.
21. **Effect scope direction is enforced.** Wider scopes never capture
    narrower authority; ambient cwd, AsyncLocalStorage, renderer paths, and
    module singletons do not select runtime authority.
22. **One request processor, many transports.** Embedded, Electron, socket,
    remote Pylon, mobile Sync, browser, and tests share decoding, WorkContext,
    policy, handlers, transactions, events, and receipts. Public values reuse
    canonical Effect Schema identity; `ManagedRuntime`, Promise, and native
    bridges stay in named perimeter modules.
23. **Interruption is cancellation.** Provider/tool/subagent/stream/host/UI
    adapters preserve cancellation and run owned finalizers once; defects do
    not become success or recoverable tool failure.
24. **Session authority is not placement.** One generation-fenced attachment
    accepts new work; a stale source cannot regain authority implicitly.
25. **Checkpoints are secret-free and honest.** Durable state may move;
    credentials, homes, handles, memory, PTYs, sockets, and false continuity do
    not.
26. **Capabilities reauthorize per target.** Move revokes source leases and
    mints fresh target leases; neither Sync nor checkpoint transports secrets.
27. **Voice is modality, not authority.** It compiles to the same typed command,
    approval, idempotency, outcome, and receipt path as text with visible
    microphone state, confirmation, and fallback. TTS, ASR hypotheses, model
    prose, and playback never prove command or outcome.
28. **Agent topology is durable session state.** Canonical identities, parent
    edges, lifecycle, transcript refs, and per-thread cursors survive replay and
    movement; the renderer never reconstructs them from prose.
29. **Streaming is not recovery authority.** Durable per-thread events repair
    the bounded projection before volatile delivery resumes; preview/spinner/
    socket health never proves completion.
30. **Fast interaction has one authority path.** Pointer, palette, menu, and
    conflict-safe hotkeys dispatch the same effective intent; local focus is
    view state, while controls receive policy and durable outcomes.

## Completion and reconciliation

Every closeout reports:

- landed commit and deployed/distributed version where applicable;
- exact tests, models, smokes, live and owner receipts;
- acceptance items and their narrowest proof rung;
- authority/security boundaries exercised;
- legacy/duplicate path deleted or explicitly retained with a gate;
- residual could-not-prove list and owner;
- next dependency-ready issue.

Reconcile this master and live issue bodies after material landings, owner
priority changes, issue disposition, or challenge decisions. Update execution,
cutover, and operating docs on critical-path change and at least weekly during
the P0 burn. Update subsystem/authority/Effect Native contracts on boundary
change and at least monthly while cited. Dated analyses and receipts remain
pinned evidence; do not silently freshen them into current queues. Remove
superseded current-state prose instead of stacking amendments.

## Historical detail index

The Revision 86 pre-compaction body remains in Git at `4239689e24`. Dedicated
evidence remains discoverable here:

- [`documentation cleanup plan`](./2026-07-12-documentation-cleanup-audit-and-retirement-plan.md)
- [`checked-in issue sources`](./issues/README.md)
- [`binding CUT dependencies and historical status`](./2026-07-11-openagents-coding-cutover-issue-plan.md)
- [`historical CUT-27 readiness audit`](./2026-07-12-cut27-cutover-readiness-audit.md)
- [`retired July 10 execution diary`](./2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md)
- [`Desktop architecture boundary`](./2026-07-10-openagents-desktop-product-architecture.md)
- [`historical Desktop parity baseline`](./2026-07-10-opencode-khala-openagents-desktop-parity-audit.md)
- [`remote-first portable-session pathway`](./2026-07-11-remote-first-portable-coding-sessions-pathway.md)
- [`Sol receipts and dated analyses`](./README.md)

Do not regrow this file into a landing diary. Promote policy here, keep live
state in issues/code/receipts, and retire obsolete history through the ordered
cleanup plan.
