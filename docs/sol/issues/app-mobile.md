# P0 TRACK: OpenAgents mobile Sync, remote coding, and fleet control

- Issue: #8597
- Program parent: #8566
- Destination: `apps/openagents-mobile`
- Status: active P0 under Master Revision 105 / R0–R7 / M0–M7
- Dispatch: no; current/next/open language below is pinned issue-source prose
- Authority: [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md)
- Current work and ownership: live issue comments plus
  [`../CLAIM_PROTOCOL.md`](../CLAIM_PROTOCOL.md)
- Capability/port ledger:
  [`../2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md`](../2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md)
- Local coding cutover graph:
  [`../2026-07-11-openagents-coding-cutover-issue-plan.md`](../2026-07-11-openagents-coding-cutover-issue-plan.md)

The bounded local-coding cutover leaves through CUT-26 are closed; #8707 owns
the remaining installed named-Codex/Claude and per-task physical-iOS/Android-
emulator acceptance. PORT-00 through PORT-02 are closed. Remote workrooms,
host movement, managed providers, any-host mobile control, portable-session
voice, signed R7 dogfood, and legacy retirement remain explicitly owned by
open PORT-03 through PORT-08 #8748–#8753 and are never inferred from the local
cutover.

## Outcome

Build one OpenAgents iOS/Android client for repository-bound agent coding,
remote workrooms, cross-device continuity, fleet attention/control, outcomes,
receipts, and optional Desktop handoff. It is not a Sarah/persona front door or
a compressed Desktop layout. It is a compact phone-native workbench that can
complete useful remote coding. Effect Native owns application/component/intent
semantics; React Native/Expo and native modules are least-authority hosts/
renderers.

This is not a rename or in-place conversion of `clients/khala-mobile`. The
legacy app remains a frozen contract/native/release extraction source until
successor and migration proof.

The useful Khala Code MVP behavior and QA corpus must be ported or explicitly
dispositioned. Importing the legacy package/component tree is forbidden.

## Current truthful baseline

- Greenfield identity/icon/owned OTA and local iOS release floor are landed.
- Build 116 at `e8bf6b8603` removes the Sarah/persona/demo/local catalog path,
  makes the native SwiftUI Liquid Glass composer the sole Khala input, passes
  typecheck plus 20 tests/69 expectations, archives/exports, and was accepted
  for App Store delivery. App Store processing/`VALID` and owner physical-
  device acceptance remain distinct unproven rungs.
- Closed CUT-01 #8681 (`bab737f565`) supersedes that temporary build-116
  architecture in source: the
  app-local SwiftUI module is deleted, one Effect Native tree owns the chrome,
  drawer, transcript, and composer, and `render-rn` owns iOS 26+ SwiftUI plus
  Android/older-iOS material lowering. Typecheck and deterministic cross-
  lowering/native-build tests pass. Physical installed-product acceptance
  remains CUT-27 #8707.
- The generic local Khala fallback remains explicitly separate from confirmed
  authority; it does not claim a provider, FleetRun, account, remote workroom,
  command outcome, or receipt.
- A host-owned Expo SQLite adapter now reuses the shared Khala Sync store core,
  persists one installation identity and offline queue across restart, and
  closes before OTA reload.
- A versioned Expo SecureStore vault now holds one native access/refresh-token
  record plus the server-derived owner ref with device-only accessibility.
  Malformed/old-epoch records purge fail-closed and view state sees only
  credential-present-unverified.
- Recovered credentials now validate through `GET /api/mobile/auth/session`.
  OpenAuth rotation rewrites the vault; 401/403 or server-derived owner mismatch
  purges it; transient/network/schema failure retains it but projects only
  unavailable.
- Mobile entry/exit now uses one imperative AuthRequest with the exact
  `openagents-khala-mobile` GitHub authorization-code + S256 contract and
  canonical `openagents://auth`. It verifies the server-derived owner before
  saving and clears locally only after the server proves both access and
  refresh revocation. Typed Effect Native intents own the visible actions.
- A verified session now composes the shared production HTTP/WebSocket Sync
  engine under the server-derived personal scope. #8671 switches the visible
  mobile Home to confirmed authoritative conversations when live, and the
  two-native-host fixture proves Desktop start → mobile follow-up → restart
  convergence with matching refs/versions/cursor.
- Closed #8676/#8677 prove the bounded same-ref physical-iPhone continuation
  and command/event fault-convergence prerequisites. Closed CUT-01 through
  CUT-26 add the local workbench, Fleet/control, Android-emulator, packaging,
  and legacy-lockout implementation receipts. #8707 still must map each
  counted installed Codex and Claude task to its literal iOS/Android-emulator
  continuation rows; prerequisite receipts cannot be inferred as that mapping.
- The remote workroom, graph-wide host move/failback, any-host directory,
  managed-provider, portable voice, signed dogfood, and legacy mobile
  retirement exit remains unproved and explicitly PORT-owned. Keep #8597 open
  after the bounded #8707 local rung closes.

## Identity locks

1. Display/product name: `OpenAgents`.
2. iOS bundle identifier: `com.openagents.app`.
3. Android package/application ID: `com.openagents.app`.
4. App icon bytes match the pinned copied icon digest
   `0a1865ac6d1efc792d365d9a37af9e6ffa3270fa7c8731f36129f35371bfc7ce`.
5. OTA/release/store identity is owned by OpenAgents and never reuses a legacy
   Khala feed or build number silently.

## R0/R1/R2/R3/R6 scope

1. Keep clean typecheck/tests/builds and honest unconfigured/offline/
   reconnecting/stale/must-refetch/denied/failed/ready states.
2. Use the canonical authenticated owner/org/device/session contract.
3. Subscribe to the same Khala Sync conversation/project/Fleet/work/attempt/
   account/approval/command/outcome/receipt projections as Desktop.
4. Make the default home recent work, repositories, Sync health, attention/
   approvals, active threads/workrooms/runs, outcomes, receipts, push returns,
   and handoff.
5. Add run/work/attempt detail and worker/account readiness without inferring
   state from chat text.
6. Port GitHub/repository selection and thread binding, rich streamed runtime
   events, composer context, queue/steer/interrupt/retry, named account/model/
   execution-target readiness, push/deep links, and cross-agent handoff.
   Preserve the canonical parent/child graph through a compact explicit drawer,
   causal inline child activity, bounded latest durable activity, and direct
   independent child transcript access; never silently cap or flatten children.
7. Add typed remote-workroom lifecycle and compact Thread, Files, Changes,
   Terminal, Preview, and Artifacts/Receipts modes. Remote file/process/port/
   writeback capabilities are brokered, bounded, revocable, and receipt-backed.
8. Share steer, approve/reject, pause/resume/stop command IDs, idempotency,
   policy, and durable outcomes with Desktop.
9. Prove background/foreground, restart, offline queue, dropped acknowledgement,
   duplicate/out-of-order events, schema migration/rollback, notifications,
   accessibility, deep links, and physical-device behavior.
10. Prove local iOS and Android build/install/recovery/update gates; never use
   EAS.
11. Define explicit migration or clean-start policy for local data, secure
   storage, deep links, push, and legacy app state.
12. Extract and rewrite useful architecture guards, unit/mount tests, stories,
    Maestro journeys, visual baselines, crash/connectivity checks, and release
    gates against the new app.
13. Add an authorized any-host session directory and typed stop/checkpoint/
    move/resume/failback controls. Tap and any supported conflict-safe shortcut
    invoke the same registered action and durable outcome as Desktop.
14. Add persona-neutral explicit ASR/TTS/barge-in over the normal session
    command registry, with visible microphone state, text fallback, no raw-
    audio retention by default, and no voice-only authority.

## Host and product boundaries

- Mobile does not expose raw **local device** filesystem/process authority,
  provider credential material, arbitrary ports/network, or force writeback.
  Remote files/editing, typed Git changes, bounded terminal, managed preview,
  artifacts, and branch/PR writeback are required through owner-scoped
  workroom capabilities.
- The phone UI uses progressive contextual navigation and accessible controls;
  it does not squeeze Desktop panes into a small viewport. Desktop handoff is a
  convenience for higher-density work, not an MVP dependency.
- The build-115 SwiftUI module is a bounded current presentation lowering, not
  a parallel application-state model or an active polish program.
- Generic Khala chat may remain a bounded conversation capability but cannot
  claim a specific backing worker/model, FleetRun, account, cost, payment,
  verification, or receipt without owning authority.
- Direct account recovery/settings remains available behind typed capabilities.

## Explicit non-goals

- Sarah/persona/relationship home;
- avatar, opener, persona voice, video, media cache/admission, ambient capture,
  or any voice-only authority path; persona-neutral ASR/TTS/barge-in under the
  normal typed session command contract remains required by the current master
  roadmap;
- demo Minerals/pricing or StoreKit work;
- aesthetic Liquid Glass iteration not required for accessibility, correctness,
  platform support, or R0–R7;
- conversion or continued shipping of `clients/khala-mobile`.

## Exit

On physical iOS and the Android emulator, the authenticated owner selects a
repository, starts or resumes one real isolated remote workroom, streams and
steers the agent, inspects its complete nested topology and one independent
child transcript, inspects/edits files, reviews the exact diff, runs a bounded
command, opens a managed preview, verifies and safely writes back a branch/PR,
and receives one durable receipt. The same session/thread/agent/workroom/
FleetRun/work/attempt/command/outcome refs continue on Desktop and across an
authorized host move. One explicit voice follow-up or interrupt uses the same
typed command path as text. The flow survives offline, lost acknowledgement,
restart, reconnect, token revocation, workroom expiry/reclaim, migration,
update, push, and handoff without presenting local or optimistic state as
authority. Owned release lanes are proven and the deprecated mobile product/
install/release path cannot ship.
