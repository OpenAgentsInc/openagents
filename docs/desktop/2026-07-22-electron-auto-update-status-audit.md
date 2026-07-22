# OpenAgents Desktop auto-update status audit

- Date: 2026-07-22
- Scope: the Electron self-update path of OpenAgents Desktop
  (`apps/openagents-desktop`) and its update feed (`apps/oa-updates`).
- Method: read-only code and document review on a clean `origin/main` worktree.
- Author role: audit agent. This document does not change update code.

## Verdict

The update path is **PARTIAL**.

A complete, custom, signed update system exists and it is wired into the running
app. The production feed serves a signed release for the stable channel. The
whole state machine has software-level and fixture-level proof.

Two facts stop this from being a working silent auto-update:

1. The update is **user-initiated**, not automatic. The app checks the feed only
   when the user presses a button in Settings. No code checks the feed on launch
   or on a timer. A stored `autoCheck` preference exists but no code reads it.
2. The **end-to-end update cycle on real hardware for the current `0.1.0` stable
   line is not proven**. The owner accepted one real macOS update cycle at
   `rc.8` to `rc.9`, but that build is not published. For the current stable
   line, the real-host N-1 update, the interrupted-update recovery, and the
   retained-slot rollback receipts are still owed.

So the app can verify, download, install, and relaunch a signed update through a
manual control, and the mechanism is real. The app does not do this silently,
and no receipt proves a real old-to-new update for the shipped stable build.

## What the app has (the in-app updater)

The updater is a custom Effect and TypeScript implementation. It is not
`electron-updater`, not Squirrel, and not `autoUpdater` from Electron. The app
comment records this design (`apps/openagents-desktop/src/main.ts` line 9).

### Wiring into the main process

`apps/openagents-desktop/src/main.ts` builds and holds the update host:

- Line 1771: the update state root is `userData/updates`.
- Line 1772: the channel is `rc` when the app version has `-rc.`, else `stable`.
- Line 1778: `resolveDesktopUpdateFeedConfig` resolves the feed host and the
  pinned key. A rejected configuration disables checks and fails closed.
- Lines 1794 to 1814: the platform applier is `openMacOSUpdateApplier` on macOS
  or `openLinuxAppImageUpdateApplier` on Linux.
- Lines 1849 to 1885: `openDesktopUpdateStagingHost` receives the feed base URL,
  the pinned key, the applier, the child-runtime drain, the migration evidence,
  and the restart function.
- Lines 4008 to 4019: one IPC handler maps the actions `snapshot`, `check`,
  `download`, `open_installer`, `apply`, and `rollback` to the host.

### The update is manual, not automatic

The renderer drives every update action from a button. The Settings surface
shows one "Check for updates" button, then "Download and verify", then "Install
and restart" or "Open DMG", then a rollback button
(`apps/openagents-desktop/src/renderer/shell.ts` lines 7399 to 7444). The intent
handlers call the host actions (`shell.ts` lines 5368 to 5408).

No code starts a check automatically. A search for a launch-time check, a
`setInterval`, or a scheduler in `main.ts`, `renderer/shell.ts`, and
`renderer/boot.ts` finds none. On launch the app calls only
`desktopUpdateHost.reconcile()` (`main.ts` line 8866). `reconcile` recovers
rollback and launch-receipt state. It does not contact the feed.

The preferences schema declares an update section with `autoCheck` (default
`true`) and `autoDownload` (default `false`)
(`apps/openagents-desktop/src/desktop-preferences-contract.ts` lines 137 to 138
and 208). No code reads `autoCheck` or `autoDownload` to start a check or a
download. These fields are dormant. The user must press the button.

### Signature verification runs in the app, not only at publish time

The trust boundary is the signature, never the host or TLS
(`apps/openagents-desktop/src/update-contract.ts` lines 11 to 16).

- `verifySignedUpdateManifest` verifies the manifest through seven fail-closed
  gates: envelope decode, algorithm, key id pin, payload sha256, ed25519 verify,
  manifest schema, and channel cross-check (`update-contract.ts` lines 216 to
  256).
- The client pins the production public key `kid 2dbe811d19f67528`
  (`update-contract.ts` lines 174 to 178). A test holds this constant
  byte-equal to `apps/oa-updates/keys/release-pubkey.json`.
- The staging host calls the verify seam during `check`
  (`apps/openagents-desktop/src/update-staging-host.ts` line 319 for the v2
  release set, line 370 for the v1 manifest).
- The download step verifies the artifact bytes against the signed sha256 and
  the signed byte length (`update-staging-host.ts` lines 421 to 425, and
  `verifyArtifactDigest` in `update-contract.ts` lines 263 to 269).

So the app rejects an unsigned or altered feed. There is no "trust anyway" path.

### The macOS install is a real in-place swap with rollback

`apps/openagents-desktop/src/macos-update-applier.ts` performs a real install:

- Lines 273 to 274: mount the DMG with `hdiutil attach`.
- Lines 176 to 205: verify the mounted app identity, version, architecture,
  code signature, Developer ID team `HQWSG26L43`, and notarization with
  `codesign`, `spctl` or `syspolicy_check`, and `stapler`.
- Line 283: copy the current app into a rollback slot with `ditto`.
- Lines 216 to 240: copy the candidate next to the target, verify it again, then
  replace it atomically with `NSFileManager replaceItemAtURL` through a small
  JXA script.
- Lines 352 to 468: arm a first-launch watchdog. The watchdog rolls back to the
  retained slot when the new build does not write a health receipt in time.

The Linux applier is `openLinuxAppImageUpdateApplier`
(`apps/openagents-desktop/src/linux-update-applier.ts`). It selects the new
AppImage and relaunches through `app.relaunch({ execPath: ... })` (`main.ts`
lines 1815 to 1821).

### The post-update health receipt and automatic rollback

Applying an update is not success. The first healthy launch is success. This
rule comes from the 2026-07-13 dead-update incident
(`update-contract.ts` lines 271 to 326).

- After install, the state holds in `awaiting_launch_receipt`
  (`update-staging-host.ts` lines 506 to 519).
- The new build writes `openagents.desktop.launch_health.v1` after a clean
  renderer start, provider start, and clean shutdown
  (`update-staging-host.ts` lines 681 to 711, called from `main.ts` lines 9439
  to 9446).
- No receipt inside the 10-minute window triggers an automatic rollback
  (`evaluateLaunchReceipt` in `update-contract.ts` lines 361 to 381, and the
  reducer in `apps/openagents-desktop/src/update-rollback.ts`).

## What the feed has (the update server)

`apps/oa-updates` is the `updates.openagents.com` service. It serves the mobile
OTA feed, the Pylon feed, and the desktop feed. The desktop path for this app is
`/desktop/openagents/<channel>/`.

The desktop app fetches the v2 release set first
(`<base>/release-set.json` and `<base>/release-set.sig.json`) in
`update-staging-host.ts` lines 306 to 346. The base URL default is
`https://updates.openagents.com/desktop/openagents/<channel>`
(`update-staging-host.ts` line 220). A bounded v1 fallback exists for macOS
arm64 only, and only before `V1_MIGRATION_END` = `2026-10-14T23:59:59Z`
(`update-staging-host.ts` lines 348 to 407, and
`apps/openagents-desktop/src/release-set-contract.ts` line 524).

The server serves the v2 route from a GCS-backed release-set store
(`apps/oa-updates/src/release-set-feed.ts` lines 484 to 545). It serves the v1
route from a baked descriptor (`apps/oa-updates/src/server.ts` lines 191 to
219). The baked v1 descriptor and manifests live in
`apps/oa-updates/openagents-desktop-dist/`. The most recent baked v1 manifest is
`manifest-rc-0.1.0-rc.13.json`.

A legacy desktop lockout is armed by default
(`apps/oa-updates/src/legacy-desktop-lockout.ts`). It returns a typed `410` to
the old `khala-code-desktop` clients so those clients cannot fetch updates from
this infrastructure. The new `openagents` desktop path is separate.

### The production feed is live for stable

The release receipt `docs/deploy/receipts/2026-07-21-desktop-0.1.0-stable-release.md`
records a live stable feed:

- `https://updates.openagents.com/desktop/openagents/stable/release-set.json`
  returns 200 with version `0.1.0`, channel `stable`, 4 targets, 10 artifacts,
  and key id `2dbe811d19f67528`.
- `.../stable/pointer.json` and `.../stable/release-set.sig.json` return 200 and
  verify against the pinned production key.

The build, sign, notarize, publish, and promote flow ran through the owned
release coordinator. The `0.1.0` GitHub release carries ten signed artifacts for
macOS arm64, macOS x64, Linux arm64, and Linux x64.

## Signing

The private release key never enters the app or the repository. The client pins
only the public key.

- Publish-time signing: `apps/openagents-desktop/scripts/publish-release.ts`
  loads the ed25519 private key only from
  `OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D` and `OPENAGENTS_RELEASE_SIGNING_KID`,
  or from `OPENAGENTS_RELEASE_SECRETS_PATH`
  (`publish-release.ts` lines 131 to 158). The script self-verifies the signed
  manifest through the exact client verify seam before it writes any file
  (`publish-release.ts` lines 15 to 16).
- macOS package signing and notarization use Developer ID team `HQWSG26L43`. The
  runbook is `apps/oa-updates/docs/release-signing-runbook.md`.
- Preflight: `apps/openagents-desktop/scripts/release-preflight.ts` proves a
  clean `origin/main`, version monotonicity, no legacy UI remnants, and the
  Gatekeeper set on built artifacts with `codesign`, `spctl`, and `stapler`. It
  refuses when the Developer ID identity or the notary credentials are absent.

So signature verification runs both at publish time and inside the app update
path. The app does not trust an artifact that fails the signed digest.

## Build and release wiring

The relevant scripts are:

- `apps/openagents-desktop/scripts/build.ts` and
  `apps/openagents-desktop/scripts/stage-and-package.ts` build the app.
- `pnpm --dir apps/openagents-desktop run make:mac` packages, signs, notarizes,
  and staples the macOS `.app` and `.dmg`. It refuses an unsigned release except
  through `OA_ALLOW_UNSIGNED_DEV=1`, which renames the output and which preflight
  and publish always refuse.
- `publish-release.ts` writes the signed manifest and descriptor into
  `apps/oa-updates/openagents-desktop-dist/`.
- `apps/oa-updates/scripts/deploy-cloudrun.sh` deploys the feed service.

The packaged DMG is the artifact the macOS applier consumes. The applier mounts
the DMG, verifies the one contained `.app`, and swaps it into place.

### The stated DMG version residual is not present on current `main`

The audit request names a residual where a DMG was labeled `0.1.0-stable` while
the content was `0.1.1-rc.1`. This exact residual is not present on the current
`main`. The stable receipt names artifacts that are internally consistent, for
example `OpenAgents-0.1.0-stable-darwin-arm64.dmg`
(`docs/deploy/receipts/2026-07-21-desktop-0.1.0-stable-release.md` line 44). A
search for `0.1.1-rc.1` in the desktop app and the deploy receipts finds no
match. The residual appears resolved or it lived outside the current tree. This
audit cannot confirm the historical mislabel from the current source alone.

## Proof status

### Software and fixture proof: complete

The state machine and the feed have strong automated proof with fixture
keypairs and injected transport:

- `apps/openagents-desktop/src/update-staging-host.test.ts` proves the happy
  path and the recovery paths. One test proves "Runtime A install to Runtime B
  health and clean shutdown to Runtime C retained rollback acceptance"
  (line 143). Other tests prove pointer mismatch rejection, corrupt-artifact
  rejection, and bounded reason codes.
- `apps/oa-updates/src/desktop-staging-feed-e2e.test.ts` proves the server and
  client together: it "discovers, verifies, stages, applies, and confirms via
  first-launch receipt" (line 285). It also proves a missed receipt rollback, a
  production-pinned client refusing a staging-signed feed, and a downgrade
  refusal.
- The contract, publisher, preflight, gatekeeper, rollback, and release-set
  tests carry many cases:
  `apps/openagents-desktop/tests/update-contract.test.ts` (14),
  `update-rollback.test.ts` (17), `release-set-contract.test.ts` (11),
  `publish-release.test.ts` (11), `release-preflight.test.ts` (9), and
  `macos-gatekeeper.test.ts` (15).

These tests use fixture keys and injected fetch and applier functions. They
prove the logic. They do not touch a real macOS filesystem swap or a real host.

### Real-hardware proof: partial and owed

- One accepted real macOS cycle exists at `rc.8` to `rc.9`. The owner reviewed
  and accepted the installed journey and the full `rc.8` to `rc.9`
  update, rollback, and reinstall lifecycle. That build is not published
  (`NEEDS_OWNER.md`, the `#8756` resolved entry).
- The `linux-x64` clean-machine receipt proves a first install and a first
  launch only, not an update from an older version
  (`docs/deploy/receipts/2026-07-21-linux-x64-clean-machine-acceptance.md`). Its
  boundary states that the full cross-platform update and rollback acceptance is
  still owner-gated.
- The macOS arm64 signed-candidate receipt states that the N-1 update, the
  interrupted-update recovery, and the retained-slot rollback receipts are still
  owed on real hardware
  (`docs/deploy/receipts/2026-07-20-macos-arm64-signed-candidate.md` lines 19 to
  20 and 122 to 125).

So no receipt proves a real old-to-new update, relaunch on the new version, and
rollback for the current `0.1.0` stable line across the shipped targets.

### There is nothing to update to on stable yet

The app version is `0.1.0` (`apps/openagents-desktop/package.json`). The stable
feed serves `0.1.0`. A `0.1.0` app that checks the stable feed sees the same
version and gets "not strictly newer". No newer stable release exists. A live
update is not observable until a newer stable version is published.

## Mobile is separate

`apps/oa-updates` also serves the Expo mobile OTA feed for
`apps/openagents-mobile`. That path uses the expo-updates protocol and
`expo-signature`. It is a different mechanism from the desktop Electron update.
Do not read a live mobile OTA feed as proof of a live desktop update. The two
share the same service, the same ed25519 release key custody, and the same
Cloud Run host. They do not share the update client or the install mechanism.

## Gaps and the work to finish, in order

1. **Decide auto or manual, then wire the choice.** If the goal is silent or
   on-restart auto-update, add a launch-time check and a periodic check, and
   read the existing `autoCheck` and `autoDownload` preferences to drive them.
   Today the update is a manual Settings action and the preference fields are
   dormant.
2. **Publish a newer stable version.** A `0.1.0` app cannot update to `0.1.0`.
   Publish `0.1.1` or later on stable so a real update is observable.
3. **Run the owner-gated real-hardware acceptance for the current line.** Use
   `apps/openagents-desktop/scripts/run-release-acceptance.ts` with real signed
   previous and candidate DMGs on macOS arm64 and macOS x64, and run the Linux
   equivalent. Capture the N-1 update, the interrupted-update recovery, and the
   retained-slot rollback receipts.
4. **Confirm the v2 release-set path for each shipped target.** The stable
   receipt proves the stable release-set serves for four targets. Prove that a
   packaged app of each target selects and applies its own target artifact from
   the live feed.
5. **Note the Windows boundary.** Windows is an optional unsigned experimental
   portable. It is not part of the signed set and it has no update acceptance.

## Files this audit relied on

- `apps/openagents-desktop/src/main.ts`
- `apps/openagents-desktop/src/update-contract.ts`
- `apps/openagents-desktop/src/update-staging-host.ts`
- `apps/openagents-desktop/src/macos-update-applier.ts`
- `apps/openagents-desktop/src/linux-update-applier.ts`
- `apps/openagents-desktop/src/release-set-contract.ts`
- `apps/openagents-desktop/src/desktop-preferences-contract.ts`
- `apps/openagents-desktop/src/renderer/shell.ts`
- `apps/openagents-desktop/scripts/publish-release.ts`
- `apps/openagents-desktop/scripts/release-preflight.ts`
- `apps/openagents-desktop/scripts/run-release-acceptance.ts`
- `apps/oa-updates/src/release-set-feed.ts`
- `apps/oa-updates/src/server.ts`
- `apps/oa-updates/openagents-desktop-dist/`
- `apps/oa-updates/docs/release-signing-runbook.md`
- `docs/DEPLOYMENT.md`
- `docs/deploy/receipts/2026-07-21-desktop-0.1.0-stable-release.md`
- `docs/deploy/receipts/2026-07-21-linux-x64-clean-machine-acceptance.md`
- `docs/deploy/receipts/2026-07-20-macos-arm64-signed-candidate.md`
- `NEEDS_OWNER.md` (the `#8756` resolved entry)
