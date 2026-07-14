# ChatGPT (Codex) desktop "can't be opened" after in-app update — root-cause analysis

Date: 2026-07-13 (late night)
Machine: the owner's MacBook Pro, macOS 26.4 (25E246)
Status: diagnosed; fix is a one-line owner command (below). Not caused by
T3 Code, not caused by OpenAI's Codex→ChatGPT rename, and — the useful part —
not actually caused by the update itself.

## Symptom

The owner ran T3 Code's one-click provider-update flow (which updated the
local Codex CLI and OpenCode), then clicked the update button inside the live
ChatGPT (Codex) desktop app. The app quit to apply the update and never came
back. Every subsequent launch shows the generic macOS dialog:

> The application "ChatGPT" can't be opened.

## Evidence chain

All commands were run read-only on the affected machine tonight.

1. **The updated bundle is intact and correctly signed.**
   `/Applications/ChatGPT.app` is build `26.707.71524` (5263), bundle id
   `com.openai.codex`, executable present and arm64.
   `codesign --verify --deep --strict` passes; `spctl -a -t exec` says
   `accepted, source=Notarized Developer ID, origin=Developer ID Application:
   OpenAI OpCo, LLC (2DC432GLL2)`. The embedded provisioning profile
   ("Codex Desktop AKV 2026-05-24") is valid until 2044. Nothing about the
   artifact is damaged.
2. **The failure is at exec time, inside AMFI.** `open -a` fails with
   `RBSRequestErrorDomain Code=5` / `NSPOSIXErrorDomain Code=163 "Launchd job
   spawn failed"`. The unified log shows the precise chain on every attempt:

   ```text
   amfid: Failed sending CPValidateProvisioningDictionaries command to
          com.apple.taskgated.helper: ... Code=42001
   amfid: Failure validating against provisioning profiles
   amfid: Requirements for restricted entitlements failed to validate, error -67671
   amfid: Restricted entitlements not validated, bailing out. Error: AMFI -400
   amfid: /Applications/ChatGPT.app/Contents/MacOS/ChatGPT not valid
   launchd: xpcproxy exited due to OS_REASON_EXEC ... code = 163
   ```

   The binary declares restricted entitlements (`com.apple.application-identifier`,
   team identifier, keychain access groups), which require the embedded
   provisioning profile to be validated by `taskgated-helper` before exec.
3. **The validation daemon is gone from launchd.**
   `launchctl print system/com.apple.taskgated.helper` → "Could not find
   service … in domain for system". The binary exists
   (`/usr/libexec/taskgated-helper`) and its LaunchDaemon plist exists
   (`/System/Library/LaunchDaemons/com.apple.taskgated-helper.plist`), but
   the on-demand service is not registered, so amfid's XPC bootstrap lookup
   fails with "No such process".
4. **The daemon was removed by launchd this morning, hours before anything
   else happened.** The log shows `taskgated-helper` working normally through
   the night (it was validating OpenAI's own "CUA Service CLI" profile at
   08:33:45), then:

   ```text
   08:34:34 launchd: service inactive: com.apple.taskgated-helper
   08:34:34 launchd: removing service: com.apple.taskgated-helper
   08:35:19 launchd: failed lookup: name = com.apple.taskgated.helper,
            requestor = amfid[355], error = 3: No such process
   ```

   From 08:35 onward there are **243 identical validation failures** in
   today's log — every provisioning-profile validation on this machine has
   failed since 08:34. The machine has been up ~2.8 days; no reboot since.

## Root cause

**macOS machine state, not the app.** launchd removed the
`com.apple.taskgated-helper` on-demand daemon from the system domain at
08:34:34 this morning (an OS-level fault on macOS 26.4 — an "inactive"
on-demand daemon should be relaunched on demand, not removed). From that
moment, no app requiring provisioning-profile validation could pass a *fresh*
AMFI check on this machine.

The in-app update was only the trigger, not the cause: AMFI validation is
cached per executable (CDHash). The old ChatGPT build had been validated back
when the helper worked, so it kept launching. The update installed a new
binary with a new CDHash, which required a fresh validation — which hit the
dead daemon — so the perfectly-signed, perfectly-notarized new build is
refused with the maximally unhelpful "can't be opened".

## What it was NOT

- **Not T3 Code.** T3's provider-maintenance flow updates the *Codex CLI*
  (npm `@openai/codex` / native installer) and OpenCode — a different
  artifact, a different trust plane (the CLI has no provisioning profile),
  and the wrong time: the daemon was removed at 08:34, the app update applied
  at ~20:17, and this machine's first T3 server run was 22:32 tonight. T3
  never touches `/Applications/ChatGPT.app`, launchd, or the AMFI stack.
  No avoidance action is needed on the T3 side.
- **Not the Codex→ChatGPT rename.** The bundle identity has been
  `com.openai.codex` throughout; the rename shipped Jul 9 and updated fine.
  Signature, notarization, and profile on the new build all verify.
- **Not a bad OpenAI artifact.** The DMG-class failure we documented for
  T3 Code earlier today (unsigned container) is a different disease; this
  bundle passes every artifact check. OpenAI's only sin here is UX: their
  updater replaced a working app and did not detect that the relaunch failed,
  leaving the user with a dead icon and a generic OS dialog.

## Fix (owner, one line — or reboot)

Re-register the daemon (needs admin password):

```sh
sudo launchctl bootstrap system /System/Library/LaunchDaemons/com.apple.taskgated-helper.plist
```

Verify, then launch:

```sh
launchctl print system/com.apple.taskgated-helper | head -3   # should now exist
open -a /Applications/ChatGPT.app
```

A reboot fixes it equally (launchd re-bootstraps all system daemons at boot).
Until one of these happens, **any** app on this machine that carries an
embedded provisioning profile will fail its next fresh AMFI validation —
notably Claude.app (which embeds one): applying a Claude update before the
fix would kill it the same way. OpenAgents Desktop is **not** exposed: it
ships no provisioning profile and no restricted entitlements
(`apps/openagents-desktop/forge.config.ts`, `build/entitlements.mac.plist`),
so this failure class cannot reach it.

## Lessons for OpenAgents deployment

1. **Post-update relaunch verification with rollback.** OpenAI's updater
   swapped a working app for one the machine refused to exec and called it a
   day. Our update contract (`apps/openagents-desktop/src/update-contract.ts`,
   `src/update-rollback.ts`) already models rollback; the missing piece this
   incident names is a *launch receipt*: after applying an update, the
   updater keeps the previous version staged until the new build has
   demonstrably launched once (app writes a first-launch receipt; no receipt
   within a bounded window → automatic rollback + diagnostic). File under the
   CUT-26 lane.
2. **Stay out of the provisioning-profile trust plane unless a capability
   demands it.** Restricted entitlements pull desktop apps into a validation
   path with a machine-local single point of failure (this daemon). OpenAgents
   Desktop currently needs none of it — keep it that way deliberately; any
   future entitlement addition (Secure Enclave keys, app groups, DeviceCheck)
   must note this failure class in its review.
3. **Diagnose at the right layer.** The artifact-level checks from the T3
   DMG incident (`codesign`/`spctl`/`stapler`) all PASS here; the failure
   only appears in the unified log (`amfid`/`launchd`). An `oa doctor`-style
   diagnostic should include both layers: artifact verification and a bounded
   launch-failure log scrape (`RBSRequestErrorDomain`, `OS_REASON_EXEC`,
   `AppleMobileFileIntegrityError`) so "can't be opened" is never the end of
   the investigation.
4. **Third-party harness updates and our supervision.** T3 is exonerated
   here, but the incident sharpens the teardown's provider-maintenance
   adaptation: when OpenAgents-owned surfaces one-click-update a harness
   (CLI or app), the maintenance action should verify the updated artifact
   *launches/answers a version probe* before reporting success — the same
   launch-receipt principle applied to tools we update on the user's behalf.

## Timeline (Central time, 2026-07-13)

| Time | Event |
| --- | --- |
| overnight–08:33 | `taskgated-helper` serving validations normally |
| 08:34:34 | launchd marks the daemon inactive and **removes the service** |
| 08:35:19 | first of 243 failed `CPValidateProvisioningDictionaries` calls |
| ~20:17 | ChatGPT in-app update applies build 26.707.71524 (new CDHash) |
| ~20:18+ | every launch: AMFI -400 → "The application 'ChatGPT' can't be opened." |
| 22:32 | first `npx t3@latest` server run on this machine (post-dates everything) |
| ~22:56–22:57 | diagnosis reproduced live; daemon confirmed unregistered |
