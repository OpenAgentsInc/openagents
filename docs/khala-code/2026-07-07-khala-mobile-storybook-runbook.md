# Khala Mobile Storybook Runbook

This runbook boots the on-device Storybook for Khala Code mobile from
`clients/khala-mobile`.

Use a separate simulator when another agent is already using the default
simulator. Do not shut down or erase someone else's booted device.

## Prerequisites

- Work from a clean `openagents` checkout or a fresh worktree.
- Use local iOS simulator builds only.
- Do not use EAS hosted builds or hosted OTA updates for this workflow.
- If `main` has advanced, fetch/rebase before changing Storybook files.

## Boot Storybook

From the repo root:

```sh
cd clients/khala-mobile
bun install
```

Pick a simulator that is not already in use:

```sh
xcrun simctl list devices available
```

Boot the selected simulator. Replace the UUID with the target simulator:

```sh
xcrun simctl boot <SIMULATOR_UUID> || true
xcrun simctl bootstatus <SIMULATOR_UUID> -b
```

Build and install the local iOS app with the Storybook app-root flag enabled:

```sh
STORYBOOK_ENABLED=true EXPO_PUBLIC_STORYBOOK_ENABLED=true EXPO_NO_TELEMETRY=1 \
  bunx expo run:ios --device <SIMULATOR_UUID> --port 8082
```

That command builds the simulator app, installs it on the selected device,
starts Metro, and opens the development-client URL. `STORYBOOK_ENABLED=true`
enables the Metro/Storybook resolver path. `EXPO_PUBLIC_STORYBOOK_ENABLED=true`
is the app-root flag for Expo's client bundle.

If the app is already installed and you only need to restart Metro on a
non-default port, use:

```sh
STORYBOOK_ENABLED=true EXPO_PUBLIC_STORYBOOK_ENABLED=true EXPO_NO_INTERACTIVE=1 \
  bunx expo start --localhost --port 8082 --clear
```

Then open the development-client URL:

```sh
xcrun simctl openurl <SIMULATOR_UUID> \
  'com.openagents.khala.mobile://expo-development-client/?url=http%3A%2F%2Flocalhost%3A8082'
```

The simulator should show Storybook with the bottom on-device toolbar and the
initial story.

## Verify

Confirm Metro is serving the Storybook entry:

```sh
# In the Metro output, look for:
# iOS Bundled ... clients/khala-mobile/.rnstorybook/index.ts
```

Capture a screenshot:

```sh
mkdir -p /tmp/khala-storybook
xcrun simctl io <SIMULATOR_UUID> screenshot \
  /tmp/khala-storybook/khala-mobile-storybook.png
```

## Troubleshooting

If the app shows the normal Khala sign-in screen, it is probably loading the
normal app root or the wrong Metro port. Stop Metro and rebuild/reopen with
both Storybook env vars:

```sh
STORYBOOK_ENABLED=true EXPO_PUBLIC_STORYBOOK_ENABLED=true EXPO_NO_TELEMETRY=1 \
  bunx expo run:ios --device <SIMULATOR_UUID> --port 8082
```

Confirm Metro serves the Storybook bundle:

```text
iOS Bundled ... clients/khala-mobile/.rnstorybook/index.ts
```

If the command opens the app shell anyway, confirm `clients/khala-mobile/index.tsx`
still imports `./src/app`; Storybook should be selected by Metro's
`STORYBOOK_ENABLED=true` resolver in `clients/khala-mobile/metro.config.cjs`,
which aliases that import to `.rnstorybook/app-root.ts`.

If Storybook shows an unreadable warning notification, verify the Storybook
LogBox color override is present:

- `clients/khala-mobile/.rnstorybook/logbox/LogBoxStyle.js`
- `clients/khala-mobile/metro.config.cjs` aliases LogBox's `LogBoxStyle`
  only when `STORYBOOK_ENABLED=true`

If Metro reports duplicate native view registration errors after changing
Storybook dependencies, stop Metro and restart with `--clear`.

If port `8082` is already taken, choose another free port and set
`RCT_jsLocation` to the same host and port.

```sh
lsof -nP -iTCP:8082 -sTCP:LISTEN || true
```

## Cleanup

Leave another agent's simulator alone. If this workflow booted a dedicated
simulator and no one else needs it, it is safe to shut down only that simulator:

```sh
xcrun simctl shutdown <SIMULATOR_UUID>
```
