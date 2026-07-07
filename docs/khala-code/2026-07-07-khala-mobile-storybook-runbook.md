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

Build and install the local iOS app:

```sh
bun run prebuild:ios
xcodebuild \
  -workspace ios/KhalaCode.xcworkspace \
  -scheme KhalaCode \
  -configuration Debug \
  -destination 'id=<SIMULATOR_UUID>' \
  build
```

Locate the built simulator app:

```sh
KHALA_APP="$(
  find "$HOME/Library/Developer/Xcode/DerivedData" \
    -path '*KhalaCode.app' \
    -type d \
    | tail -1
)"
```

Install it:

```sh
xcrun simctl install \
  <SIMULATOR_UUID> \
  "$KHALA_APP"
```

Start Storybook Metro on a non-default port when another agent may be using
`8081`:

```sh
STORYBOOK_ENABLED=true EXPO_NO_INTERACTIVE=1 \
  bunx expo start --localhost --port 8082 --clear
```

In another shell, point this app on this simulator at the Storybook Metro port:

```sh
xcrun simctl spawn <SIMULATOR_UUID> \
  defaults write com.openagents.khala.mobile RCT_jsLocation localhost:8082
```

Launch the app:

```sh
xcrun simctl launch \
  --terminate-running-process \
  <SIMULATOR_UUID> \
  com.openagents.khala.mobile
```

The simulator should show Storybook with the bottom on-device toolbar and the
initial story, usually `Khala/Primitives/Button/Basic`.

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
normal app bundle or the wrong Metro port. Confirm the app log requests `8082`:

```sh
xcrun simctl launch \
  --terminate-running-process \
  --console-pty \
  <SIMULATOR_UUID> \
  com.openagents.khala.mobile
```

Look for:

```text
GET http://localhost:8082/.expo/.virtual-metro-entry.bundle
```

If it still requests `8081`, set the simulator app preference again:

```sh
xcrun simctl spawn <SIMULATOR_UUID> \
  defaults write com.openagents.khala.mobile RCT_jsLocation localhost:8082
```

If Storybook shows an unreadable white warning bar, verify these are present:

- `@storybook/react-native-ui` in `clients/khala-mobile/package.json`
- `LogBox.ignoreLogs` for the known Storybook gesture-handler warning in
  `clients/khala-mobile/.rnstorybook/index.ts`

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
