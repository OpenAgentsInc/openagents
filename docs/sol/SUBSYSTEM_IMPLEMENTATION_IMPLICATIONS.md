# Reliable Desktop/mobile subsystem implementation implications

- Class: contract
- Updated: 2026-07-12
- Status: active, revision-independent subsystem boundary companion to
  [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md)
- Dispatch: no current queue; use the master roadmap, live issues/claims,
  current code/tests, and receipts for implementation state

The prior Sarah-first subsystem map is superseded. Its authority, privacy,
typed-service, and receipt principles are retained; its named front door,
relationship-first navigation, and A/V sequencing are not.

This document states durable responsibilities and prohibitions. “Must” names a
maintained product contract, not evidence that the capability is absent today.
Do not infer current gaps, proof rungs, or next work from this file.

## Shared system rule

Desktop and mobile are different capability hosts over one owner-scoped state,
authority, action, and evidence reality:

```text
client intent -> policy -> Fleet/Pylon authority -> execution
      ^                                           |
      |                                           v
client state <- Khala Sync <- receipt/outcome <- evidence
```

- Desktop is the deep coding workbench and Fleet cockpit.
- Mobile is the compact remote coding, attention, fleet control, receipt, and
  handoff client.
- Khala Sync carries canonical projections and durable mutation outcomes.
- Pylon/Worker/Cloud/data services retain their own typed authority.
- Effect Native owns shared application semantics and intents; Electron and
  React Native/Expo are least-authority hosts.

## 1. Identity, session, and Khala Sync

**Durable responsibility:** one authenticated owner/org/device/session
contract that both clients can resume without forking state.

Implementation consequences:

- Reuse `@openagentsinc/khala-sync`, `@openagentsinc/khala-sync-client`,
  `@openagentsinc/khala-sync-server`, and existing identity/auth authorities.
- Canonicalize conversation, project/session, FleetRun, work-unit, attempt,
  worker/account, approval, command, outcome, and receipt keys.
- Carry owner scope, expected version or explicit commutative semantics,
  idempotency key, durable outcome, cursor, version, and tombstone.
- Treat local stores as caches/offline queues. A local row, optimistic overlay,
  staged brief, transcript, or timeout is never authority.
- Reconnect from a durable cursor; a retention gap produces `must_refetch`.
- Make token refresh, device revocation, access change, migration, compatibility
  window, and rollback explicit and tested.
- Preserve private evidence on its owning plane; project only bounded safe
  post-images.

Do not:

- create app-specific Sync protocols or fleet schemas;
- use last-write-wins for authority-bearing state;
- report transport delivery as command completion;
- expose credentials, raw events, prompts, paths, or private repository data.

Related gates: R1, R2, and the current cross-client continuity acceptance in
the master roadmap.

## 2. OpenAgents Desktop

**Durable responsibility:** a practical OpenCode-parity coding workbench and
server-authoritative Fleet cockpit at `apps/openagents-desktop`.

Implementation consequences:

- Keep the hardened Electron boundary: sandbox/context isolation, no renderer
  Node integration, restrictive CSP, deny-by-default navigation/window/
  permission policy, fixed schema-decoded IPC, and host capability grants.
- Maintain authoritative session identity, streamed reasoning/tool/plan/
  request/outcome state, rich composer/context, interrupt/resume, and
  reconnect without reviving a parallel local-only authority.
- Maintain and finish any unsatisfied acceptance for projects/sessions/
  commands, bounded file edit/save, Git review, workspace-bounded PTY,
  runtime/model/provider/MCP/permission settings, diagnostics, and lifecycle/
  distribution according to the current roadmap and live issues. Presence in
  this list does not mean the capability is absent.
- Compose Fleet state from existing Pylon/Sync authority. Direct controls and
  any future automation use the same typed command IDs and outcomes.
- Use typed foreign-host nodes for editor/terminal depth; never generalize raw
  filesystem/process/IPC authority into the renderer.
- Preserve the OpenCode parity audit as the capability benchmark:
  [`2026-07-10-opencode-khala-openagents-desktop-parity-audit.md`](./2026-07-10-opencode-khala-openagents-desktop-parity-audit.md).

Do not:

- convert or import the deprecated Electrobun UI architecture;
- treat the local staged Fleet brief as a FleetRun;
- satisfy parity with placeholder panes, a secure shell alone, or Fleet-only
  status chrome;
- retire legacy paths until replacement and migration/release proof exist.

Related roadmap gates: R0, D0–D6/R5, and R7.

## 3. OpenAgents mobile

**Durable responsibility:** a compact remote coding, supervision, and
continuity client at `apps/openagents-mobile`. It is phone-native rather than
a scaled-down Desktop, but it must complete useful repository work without
requiring Desktop.

Implementation consequences:

- Preserve `OpenAgents`, `com.openagents.app`, pinned icon, owned OTA/release
  identity, and local iOS/Android build discipline.
- Make the neutral default surface recent work, repositories, Sync health,
  attention/approvals, active threads/workrooms/runs, outcomes, receipts, push,
  and optional Desktop handoff.
- Use the same entity refs, versions, command IDs, outcomes, and error grammar
  as Desktop.
- Port the full useful Khala Code MVP behavior/test corpus according to
  [`2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md`](./2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md):
  auth/session recovery, repository selection/binding, rich turns, composer
  controls, account/model/target readiness, remote workroom, files/changes,
  terminal/preview, safe writeback, fleet, push, and release QA.
- Render files, diffs, terminal, preview, artifacts, and writeback through typed
  remote-workroom capabilities. Use progressive contextual navigation; do not
  squeeze Desktop columns onto a phone.
- Preserve background/foreground, offline, restart, notification, deep-link,
  accessibility, and release proof on the acceptance hosts named by the
  current roadmap: physical iOS plus Android emulator. Physical Android may
  add evidence, but the owner decision says nothing gates on it.
- Keep direct account recovery/settings available behind bounded capabilities.
- Treat any app-local SwiftUI module as an explicit host exception, not a
  second state model or a reason for visual expansion.

Do not:

- restore Sarah/persona/demo/Minerals/video product scope;
- present generic Khala chat as account, Fleet, model, payment, or receipt
  authority;
- expose raw **local device** filesystem/process credentials or unbounded host
  authority; remote files/editor/terminal/preview capability is required when
  scoped by the workroom contract;
- use EAS or silently reuse legacy Khala app data/identity.

Related roadmap gates: R0, R1/R2, R6, and R7.

## 4. Fleet, Pylon, workers, and Agent Computers

**Durable responsibility:** one FleetRun/claim/attempt/command/outcome authority
consumed by both clients.

Implementation consequences:

- Integrate existing `FleetRun`, plan DAG, claim registry,
  `KhalaFleetIntent`, worker/account custody, steering, approval, attempt,
  verification, usage, and receipt contracts before adding anything.
- Use named isolated account refs; never automatic default provider homes.
- Keep one claim registry across owner-local and future managed-cloud targets.
- Distinguish accepted, rejected, failed, and unknown-pending-reconcile.
- Preserve exact usage when measured and explicit `not_measured` otherwise.
- Keep owner-local subscription and managed-cloud grant/economic rails distinct.
- Retain closed #8640 as mixed-account runtime proof; it does not establish a
  Sarah front door, complete the Desktop/mobile cutover, or represent current
  work.

Do not:

- create a client-local Fleet universe;
- silently substitute harness, worker, account, or execution target;
- let model prose select authority or claim success;
- treat a container/control-plane mock as production isolation proof or allow
  a client-specific remote-workroom authority.

Related roadmap gates: R3/R4/R6/R7. Brokered workroom and hybrid-routing
slices must use this same authority; their current priority and issue refs come
from the master roadmap.

## 5. Effect Native and host renderers

**Durable responsibility:** the shared component, state, and intent grammar.

Implementation consequences:

- Put reusable component/action demand in Effect Native, with renderer-specific
  lowering behind DOM/RN/native/foreign-host boundaries.
- Maintain closed typed catalogs, renderer conformance, accessibility, lifecycle
  safety, and explicit vendoring/topology guards.
- Allow platform-specific fidelity only when semantics, action IDs, and outcomes
  remain shared.
- Remove duplicate app-local semantics after replacement proof.

Do not:

- use visual polish as a substitute for R0–R7 progress;
- cast around catalog or renderer mismatches;
- create a parallel shadcn/Zod/oRPC/TanStack application architecture;
- reopen glass/lowering migration unless it blocks correctness, accessibility,
  platform support, or an active reliability gate.

## 6. Evidence, behavior contracts, and receipts

**Durable responsibility:** cross-device completion truth that survives reconnect and can
be independently checked.

Implementation consequences:

- Convert fault/interleaving counterexamples into deterministic regression
  tests or explicit model-boundary exceptions.
- Test lost acknowledgements, duplicate/out-of-order events, stale generations,
  cursor gaps, device/server restart, offline queues, migrations, and rollback.
- Keep code-landed, fixture-proven, deployed/distributed, live-proven,
  owner-accepted, and closed distinct.
- Include exact authority, verification, artifact, usage/economics, and
  could-not-prove fields in closeout receipts.

Do not:

- treat screenshots, successful build upload, public counters, or worker
  closeout as independent completion proof;
- weaken behavior contracts to make a migration pass;
- put private production material in formal traces or public receipts.

## 7. Release and operations

**Durable responsibility:** observable, recoverable Desktop/mobile software with one
cross-device incident model.

Implementation consequences:

- Track Sync lag, command latency/outcome reconciliation, reconnects, conflicts,
  duplicate suppression, crash/load/unresponsive recovery, and release health.
- Separate owner action required from agent-fixable failure.
- Provide public-safe diagnostics and bounded support export.
- Prove clean install, update, schema compatibility, rollback, and sustained
  owner dogfood on both clients.
- Preserve signing, notarization, store, provider, and credential owner gates.

Do not:

- create one giant log across private planes;
- claim release acceptance from build validity alone;
- publish, deploy, or rotate credentials without explicit authority.

## 8. Web and commercial surfaces

Web remains a supported public/API/operations surface. Preserve auth, security,
promise/service-deliverable integrity, historical receipts, health, legal, and
required production routes. Under the 2026-07-14 owner decision, payment,
wallet, checkout, tip, payout, market, and settlement mutations are not merely
deferred: they are retired and deleted through the
[`Node/pnpm/Vite Plus conversion contract`](./2026-07-14-node-pnpm-vite-plus-full-conversion-plan.md).
Safe drain/reconciliation and stable read-only history precede deletion. Broad
route conversion, landing, Forum expansion, portal, CRM, sales, and outbound
product work remain closed unless separately reauthorized.

## 9. Closed presentation scope and retained voice modality

- Sarah as named/default front door;
- persona/relationship/role-program/named-colleague expansion;
- avatar, opener, Sarah/persona voice, realtime video, media admission/cache,
  and presentation-quality experiments;
- landing/portal/Forum product expansion;
- optional visual/glass/lowering polish.

Persona-neutral conversational voice is not part of the closed Sarah scope. It
is an R6/R7 input/output modality over the same typed session command and
outcome path as text: explicit microphone state, bounded ASR/TTS/barge-in, text
fallback, no raw-audio retention by default, and no voice-only authority. Do
not restore avatar/video/persona state or create a separate command system to
implement it.

#8733 is a bounded owner-dogfood exception to the broad no-retention default:
an explicit click starts one visibly retained Desktop session, while capture,
egress, retention, and playback remain separately visible; mute/stop/revocation
end new retained-frame acceptance. AUDIO-1 #8734 freezes the shared contract,
AUDIO-3 #8736 owns encrypted GCS plus Cloud SQL custody/delete/export receipts,
and raw media never enters Runtime Gateway events or Khala Sync. This exception
does not authorize ambient auto-start or a public retention default.

Do not delete proven compatibility contracts merely to express closed scope.
Security, privacy, data-loss, active-cost, production-outage, accessibility,
and already-supported compatibility repairs remain allowed.

## Cross-subsystem definition of done

A subsystem slice is complete only when it:

1. consumes the canonical typed identity/state/action contract;
2. preserves owning-service authority and privacy boundaries;
3. is honest about local, pending, unavailable, stale, fixture, and terminal
   state;
4. survives the relevant reconnect/restart/offline/replay cases;
5. produces exact verification and closeout evidence;
6. works on the intended real host/device at the claimed proof rung;
7. removes or freezes the replaced duplicate path; and
8. updates issue status, residuals, next-ready work, and roadmap/docs when the
   program truth changes.
