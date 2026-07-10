# OpenAgents mobile — v0.5.2 build 112: minerals sheet survives video end (P0, GL-2 #8648)

Owner P0 feedback on TestFlight build 111 (verbatim): the Buy Minerals Liquid
Glass sheet auto-dismisses when the background reply video ends/loops. Wrong.
**The sheet must stay open until the USER dismisses it** — selecting a price
pack or "Not now".

## Root cause

`AskVideoDismissed` was one intent doing two jobs: the shell's `playToEnd`
playback listener and the user's video-tap dismissal both dispatched it, and
its handler cleared `mineralsSheetOpen` along with `askVideoPlaying`
(`apps/openagents-mobile/src/screens/home-core.ts`). So the 8s reply video
reaching its end silently closed the sheet out from under the user.

## Fix (JS-only — view program)

Sheet lifecycle decoupled from playback state ENTIRELY:

- New typed `AskVideoEnded` intent (playback-lifecycle event; wired to the
  `expo-video` `playToEnd` listener via `chrome.askVideoEnded`). It ends the
  takeover only — never touches `mineralsSheetOpen`.
- `AskVideoDismissed` (user tap on the video) also no longer closes the sheet.
- The ONLY intents that close the sheet are the user's: `MineralPackSelected`
  and `MineralsSheetDismissed` ("Not now"). When the video ends while the
  sheet is open, the background resumes per surface mode (Sarah loop / black)
  BEHIND the still-open sheet.
- `BUNDLE_TAG` bumped to `2026-07-09.embedded-112`.

Behavior contract landed in the same change (owner-statement verbatim):
`openagents_mobile.minerals_sheet_user_dismiss_only.v1` in
`packages/behavior-contracts/src/openagents-apps.ts` — state `enforced`, tier
`test-sweep`, oracle `apps/openagents-mobile/tests/home-shell-core.test.ts`.

## Tests

New oracle "minerals sheet survives video end/dismiss; ONLY user intents
close it" drives the real Home view program: `AskVideoEnded` (and a spurious
repeat), then a user video-tap `AskVideoDismissed`, both leave
`mineralsSheetOpen: true`; `MineralsSheetDismissed` and `MineralPackSelected`
each close it. The full-loop renderer test now asserts the sheet SURVIVES
`dismissAskVideo` and closes on `dismissMineralsSheet`. Mobile suite 21 pass,
behavior-contracts 34 pass, typecheck clean.

## Simulator pixel proofs (upload gate; iPhone 17 Pro, Release, cliclick taps)

1. `receipts/2026-07-10-build112-sheet-open-during-video.png` — one simulated
   tap on the composer starts the reply video (with audio) under the chrome;
   at the 4s midpoint the Liquid Glass Buy Minerals sheet flies up.
2. `receipts/2026-07-10-build112-sheet-survives-video-end.png` — **the money
   shot**: 26s after the composer tap (the 8s video ended long before; the
   status clock has rolled a minute), the sheet is STILL OPEN with the Sarah
   loop resumed behind it. On build 111 this exact moment auto-dismissed the
   sheet.
3. `receipts/2026-07-10-build112-sheet-user-dismissed.png` — one simulated
   tap on "Not now" closes the sheet; Sarah loop + full glass chrome remain.

## Delivery — BOTH rails

- **OTA to the owner's installed build 111 (instant):** the fix is JS-only —
  the prebuilt native project at buildNumber 111 fingerprints to exactly build
  111's runtime `5c5dc31566d3b3337308c46c2840e86ca5ca65ef`. Published via
  `apps/oa-updates/scripts/publish-ota.sh` (owner `openagents-mobile`, channel
  `openagents-production`, Cloud Run revision `oa-updates-00090-s5n`); live
  manifest verified: update id `4536dd82-7d66-4e90-835a-34ce905e3e01`,
  createdAt `2026-07-10T05:07:09.703Z`, signed (keyid main). Build 111's 3s
  poll pulls it on next launch; the drawer footer flips to
  `Bundle 2026-07-09.embedded-112`.
- **TestFlight build 112:** version FROZEN at `0.5.2` (standing owner rule —
  build numbers only), `ios.buildNumber` 112. `expo prebuild --clean` re-run
  AFTER the bump (Info.plist verified `CFBundleVersion=112` pre-archive; no
  stale-identity slip), `xcodebuild archive` (Team `HQWSG26L43`, ASC key
  auth), manual-signing `-exportArchive` (profile `com.openagents.app
  AppStore`), `xcrun altool --upload-app`. Upload + `processingState=VALID`
  evidence on #8648.
