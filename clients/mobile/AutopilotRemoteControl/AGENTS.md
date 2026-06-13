# AGENTS — Autopilot Remote Control (Expo mobile client)

This is the Expo / React Native mobile client for Autopilot Remote Control. It is
one app inside the larger `openagents` monorepo, which sits inside the
`/Users/christopherdavid/work` umbrella workspace. The workspace-level
`CLAUDE.md`/`AGENTS.md` and any `openagents`-level guidance still apply — this
file adds the Expo-specific context for work scoped to this app.

## Stack

- Expo SDK 55 (`expo` in `package.json` is source of truth for the version).
- React Native + React 19, React Navigation v7, `react-native-web` for the web
  target.
- TypeScript, dynamic app config in `app.config.ts` (not a static `app.json`).
- **Local builds on our own infra** for device builds / TestFlight (see
  `TESTFLIGHT.md`). **EAS Build / EAS Submit / `eas update` are removed — do not
  use them.** Native `.ipa` is compiled locally on this Mac (Xcode / fastlane);
  JS updates ship over our own OTA server (`updates.openagents.com`); only the
  TestFlight upload itself touches Apple's App Store Connect.

## Expo conventions

- The official Expo Claude Code plugin (`expo@claude-plugins-official`) is
  enabled via `.claude/settings.json` — it teaches known-good Expo/RN patterns
  (Expo Skills). If you see "Plugin 'expo' is enabled but isn't installed here",
  run `claude plugin install expo@claude-plugins-official` and restart.
- For SDK version specifics, prefer the Expo docs / Expo Skills over guessing.
  Use `npx expo install <pkg>` (not bare `npm/bun add`) so native module
  versions stay aligned to the SDK. (The Expo build *cloud*/EAS is out; the
  `expo` CLI itself — `expo install`, `expo export`, `expo prebuild` — stays.)
- Treat `app.config.ts` as the single source for app identity:
  - iOS bundle id: `com.openagents.autopilot-mobile`
  - Android package: `com.openagents.autopilotmobile`

## Builds / TestFlight (local, our infra — EAS removed)

- Native `.ipa` is compiled **locally on this Mac** (`expo prebuild` →
  `xcodebuild`/`fastlane`), uploaded to TestFlight via Apple App Store Connect.
- JS-only changes ship **OTA over our own server** (`updates.openagents.com`)
  via `apps/oa-updates/scripts/publish-ota.sh` — no build, no Apple, no Expo.
- The full ship runbook is `TESTFLIGHT.md`. The build runs locally and is fully
  automatable here; only Apple's TestFlight processing is external.
- **Pulling crash logs:** when a TestFlight build crashes, pull the symbolicated
  log yourself via the App Store Connect API — see `CRASH_LOGS.md` and
  `scripts/testflight-crashes.mjs` (no Expo/EAS; uses the ASC API key in
  `.secrets/appstoreconnect.env`).

## Boundaries

- Shared control/bridge types live in `@openagentsinc/autopilot-control-protocol`
  (`packages/autopilot-control-protocol`). Re-add it as a workspace dep when
  wiring live Pylon control (CL-6); do not fork the protocol here.
- This app is the mobile *client*; node/runtime authority lives in Pylon
  (`apps/pylon`). Keep that boundary — render and relay, don't reimplement.
