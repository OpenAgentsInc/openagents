# P0 TRACK: OpenAgents mobile Sync, remote coding, and fleet control

- Issue: #8597
- Program parent: #8566
- Destination: `apps/openagents-mobile`
- Status: active P0 under Master Revision 27 / R0–R7 / M0–M7
- Authority: [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md)
- Bounded leaves:
  [`../2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md`](../2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md)
- Capability/port ledger:
  [`../2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md`](../2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md)

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
- The current in-memory generic Khala chat does not provide
  authenticated cross-device Sync, FleetRun/account authority, command
  outcomes, remote workrooms, or receipts.
- A host-owned Expo SQLite adapter now reuses the shared Khala Sync store core,
  persists one installation identity and offline queue across restart, closes
  before OTA reload, and reports only local durability. Browser PKCE entry and
  authenticated network Sync remain unimplemented.
- A versioned Expo SecureStore vault now holds one native access/refresh-token
  record plus the server-derived owner ref with device-only accessibility.
  Malformed/old-epoch records purge fail-closed and view state sees only
  credential-present-unverified. Browser PKCE remains; recovered validation is
  the following landed boundary.
- Recovered credentials now validate through `GET /api/mobile/auth/session`.
  OpenAuth rotation rewrites the vault; 401/403 or server-derived owner mismatch
  purges it; transient/network/schema failure retains it but projects only
  unavailable. A verified session still does not claim live Sync.
- The #8597 Sarah-removal/composer claim released at `e8bf6b8603`. Recheck live
  claims before editing; the next honest action is browser PKCE/sign-out then
  authenticated Sync composition, not rebuilding removed persona/demo/local
  catalog state.

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

## Paused/non-goals

- Sarah/persona/relationship home;
- avatar, opener, voice, ASR/VAD, video, media cache/admission;
- demo Minerals/pricing or StoreKit work;
- aesthetic Liquid Glass iteration not required for accessibility, correctness,
  platform support, or R0–R7;
- conversion or continued shipping of `clients/khala-mobile`.

## Exit

On physical iOS and Android devices, the authenticated owner selects a
repository, starts or resumes one real isolated remote workroom, streams and
steers the agent, inspects/edits files, reviews the exact diff, runs a bounded
command, opens a managed preview, verifies and safely writes back a branch/PR,
and receives one durable receipt. The same thread/workroom/FleetRun/work/
attempt/command/outcome refs continue on Desktop. The flow survives offline,
lost acknowledgement, restart, reconnect, token revocation, workroom expiry/
reclaim, migration, update, push, and handoff without presenting local or
optimistic state as authority. Owned release lanes are proven and the
deprecated mobile product/install/release path cannot ship.
