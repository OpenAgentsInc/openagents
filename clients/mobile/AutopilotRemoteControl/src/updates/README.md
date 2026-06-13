# Updates

This directory contains the pure, unit-testable policy core for the in-app EAS
Update flow. Remaining operator steps:

- `npx expo install expo-updates`
- `eas update:configure`, which sets `updates.url`
- Wire a `useUpdates()`-based hook that calls `update-policy`, then
  `Updates.fetchUpdateAsync()` and `Updates.reloadAsync()` when the policy
  returns `download_and_reload`
