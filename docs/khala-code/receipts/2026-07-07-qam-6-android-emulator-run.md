# QAM-6 Android Emulator Lane Receipt

Date: 2026-07-07

Issue: #8541

Status: PARTIAL DEVICE RECEIPT, EXIT STILL BLOCKED

This receipt records a real local Android emulator run for the QAM-6 lane. It
does not close #8541 because the signed-in thread smoke requires a public-safe
seeded signed-in environment that was not present for this run.

## Command

```bash
KHALA_ANDROID_ARTIFACT_DIR=/tmp/khala-android-qam6 \
KHALA_ANDROID_EMULATOR_RECEIPT=/tmp/khala-android-qam6/android-emulator-lane.latest.json \
bash clients/khala-mobile/scripts/android-emulator-test-run.sh
```

## Result

- PASS: Android debug build assembled successfully.
- PASS: APK installed on AVD `khala_test`.
- PASS: Maestro `LaunchFallback.yaml`.
- PASS: Maestro `LaunchGitHubSignInInteraction.yaml`.
- INCONCLUSIVE: `SignedInThreadSmoke.yaml`, skipped because public-safe signed-in
  seed environment was absent.

## Receipt

```json
{
  "schema": "openagents.khala_mobile.android_emulator_lane.v1",
  "verdict": "inconclusive",
  "reason": "launch/sign-in flows passed; signed-in flow skipped because seeded public-safe thread env was absent",
  "avdName": "khala_test",
  "appId": "com.openagents.khala.mobile",
  "flows": [
    ".maestro/flows/LaunchFallback.yaml",
    ".maestro/flows/LaunchGitHubSignInInteraction.yaml",
    ".maestro/flows/SignedInThreadSmoke.yaml"
  ]
}
```

## Blocker

- Needs a public-safe signed-in Android fixture/account before the lane can
  truthfully emit a passed receipt for signed-in coding-agent chat coverage.
