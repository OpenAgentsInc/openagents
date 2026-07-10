# APP-MOBILE: greenfield OpenAgents mobile — Effect Native + React Native

## Outcome

Build a new **OpenAgents** iOS/Android app from scratch at
`apps/openagents-mobile`. Sarah is home and coding-fleet supervision is the
first deep capability. Effect Native owns the application/component/intent
model; React Native and Expo are host and renderer machinery.

This is not a rename or in-place rewrite of `clients/khala-mobile`. That package
is deprecated and frozen as a parity, contract, native-module, and migration
reference until the new app proves its cutover.
Public claim authority is planned `openagents.mobile_app.v1`; the legacy
`khala_code.mobile_mvp.v1` record is withdrawn but remains dereferenceable
history.

## Current status

The greenfield identity/icon/OTA floor is landed. TestFlight 0.4.3 build 106 and
0.5.0 build 107 reached `VALID`; build 107 is the simulator-pixel-proven typed
glass shell/drawer/composer loop. Build-108 source at 0.5.1, focused tests,
typecheck, simulator pixel receipt, and lockfile are on `main` through
`e30028a7e1`, but there is no recorded ASC/`VALID` receipt for build 108. Sarah/
Sync cross-device continuity, Android, owner-device acceptance, the remaining
GL integration, and legacy-client retirement remain open.

## Identity locks

1. Display/product name: `OpenAgents`.
2. iOS bundle identifier: `com.openagents.app`.
3. Android package/application ID: `com.openagents.app`.
4. Copy the current Khala Code mobile application icon into the new app; do not
   load it at runtime from the deprecated package. The canonical source is
   `clients/khala-mobile/assets/images/icon.png`, SHA-256
   `0a1865ac6d1efc792d365d9a37af9e6ffa3270fa7c8731f36129f35371bfc7ce`.
   Add an automated identity/icon oracle so the name, both identifiers, and
   copied icon digest cannot drift.

The legacy package currently uses `com.openagents.khala.mobile`; that identifier
is not the destination identity. `com.openagents.app` is an owner-designated
existing application identifier and wins for the new app.

## Scope

1. Scaffold an independent `@openagentsinc/openagents-mobile` application with
   no imports from the legacy app package or its screen tree.
2. Make Sarah conversation, voice, Blueprint, active fleet runs, steering,
   approvals, receipts, and exact closeout state the home flow.
3. Use shared typed services and Khala Sync for conversation/fleet continuity
   across web, mobile, and desktop; retain direct account connection/settings
   as an expert recovery path.
4. Extract or reimplement reusable platform-neutral contracts, auth, Sync,
   push, credits, secure storage, OTA, STT, and Apple FM capabilities behind
   Effect/Effect Schema boundaries. Do not copy the legacy UI architecture.
5. Create a new OpenAgents mobile OTA namespace/channel and repoint the owned
   `apps/oa-updates` publish path. Never reuse a legacy Khala product feed by
   accident and never use EAS build/submit/update.
6. Define typed migration or an explicit clean-start policy for local data,
   keychain/secure-store entries, deep links, and store identity. Do not silently
   read legacy secrets or databases.
7. Port applicable behavior contracts into the new app's registry and prove
   iOS and Android QAM/Maestro, local build/submission, update, and recovery
   gates before cutover.
8. Verify store ownership, provisioning/signing, and monotonically advancing
   build/version numbers against the actual existing `com.openagents.app`
   App Store Connect and Play Console records before upload; do not inherit the
   legacy Khala app's local build numbers.
9. After parity and migration receipts exist, remove the deprecated app from
   active workspace, install, release, and update paths.

## Non-goals

- Do not convert, rename, or keep shipping `clients/khala-mobile` as OpenAgents.
- Do not preserve legacy navigation or visual structure merely because it
  exists.
- Do not block the immediate Sarah Fleet Command burn on full mobile completion.

## Exit

A Sarah conversation and active multi-stream run started on web appear live in
the new OpenAgents mobile app; a mobile steer/approval changes the same run and
appears in OpenAgents Desktop. Both platforms render through Effect Native. A
clean iOS and Android build proves the exact `OpenAgents` name,
`com.openagents.app` identifiers, icon digest, owned OTA path, and relevant
ported behavior contracts. The legacy Khala Code mobile app is no longer an
installable or releasable product surface.
