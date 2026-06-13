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
- EAS Build + EAS Submit for device builds / TestFlight (see `TESTFLIGHT.md`).

## Expo conventions

- The official Expo Claude Code plugin (`expo@claude-plugins-official`) is
  enabled via `.claude/settings.json` — it teaches known-good Expo/RN patterns
  (Expo Skills). If you see "Plugin 'expo' is enabled but isn't installed here",
  run `claude plugin install expo@claude-plugins-official` and restart.
- The remote Expo MCP Server (`https://mcp.expo.dev/mcp`, HTTP transport) gives
  live access to Expo docs + EAS (build logs, TestFlight feedback, submissions).
  Run `/mcp` once to complete the OAuth login before relying on it.
- For SDK version specifics, prefer the Expo MCP Server / Expo Skills over
  guessing. Use `npx expo install <pkg>` (not bare `npm/bun add`) so native
  module versions stay aligned to the SDK.
- Treat `app.config.ts` as the single source for app identity:
  - iOS bundle id: `com.openagents.autopilot-mobile`
  - Android package: `com.openagents.autopilotmobile`
  - EAS owner: `openagents`, project id `33dc1fb6-1b11-486d-baa0-7946302fdc68`

## EAS / TestFlight

- `eas.json` holds build profiles + `submit.production.ios` (team `HQWSG26L43`).
- The full ship runbook is `TESTFLIGHT.md`. Owner-only steps (`eas login`,
  Apple Developer + App Store Connect record, `ascAppId`) can't be automated.

## Boundaries

- Shared control/bridge types live in `@openagentsinc/autopilot-control-protocol`
  (`packages/autopilot-control-protocol`). Re-add it as a workspace dep when
  wiring live Pylon control (CL-6); do not fork the protocol here.
- This app is the mobile *client*; node/runtime authority lives in Pylon
  (`apps/pylon`). Keep that boundary — render and relay, don't reimplement.
