# Updates

This directory contains the pure, unit-testable policy core for the in-app Expo
Updates flow. OTA publishing is through our own OpenAgents Updates server.
Remaining operator steps:

- `npx expo install expo-updates`
- Keep `app.config.ts` `updates.url` pointed at
  `https://updates.openagents.com/autopilot/manifest`
- Publish JS-only updates with `apps/oa-updates/scripts/publish-ota.sh`
- Wire a `useUpdates()`-based hook that calls `update-policy`, then
  `Updates.fetchUpdateAsync()` and `Updates.reloadAsync()` when the policy
  returns `download_and_reload`
