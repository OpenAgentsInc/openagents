# QAM-6 Android Lane Definition Receipt

Date: 2026-07-07

Issue: #8541

Status: SCHEDULABLE FOUNDATION ONLY

This receipt records the Android emulator lane definition for QAM-6. It is not
an exit receipt for #8541: the launch flows have not been proven green in the
QAM-5 nightly row in this change, and Android-keyed visual baselines have not
been produced from a fresh nightly run.

## Shipped

- Added `clients/khala-mobile/scripts/android-emulator-test-run.sh`.
- Added package script `qa:android:emulator`.
- Added Android emulator Maestro and adb screencap nodes to the QAM-5 nightly
  row model.
- Added static tests proving the runner contains:
  - AVD create/boot path.
  - `adb wait-for-device`.
  - `sys.boot_completed` boot proof.
  - Shared `LaunchFallback.yaml` Maestro flow.
  - Shared `LaunchGitHubSignInInteraction.yaml` Maestro flow.
  - `adb exec-out screencap -p` capture path.
  - Public-safe JSON receipt path via `KHALA_ANDROID_EMULATOR_RECEIPT`.
- Added behavior contract:
  `khala_mobile.qa.android_emulator_lane_definition.v1`.

## What Still Must Be Proven

- The Android emulator lane must run on the owned Tailnet Mac nightly row.
- The launch flows must be green in that row.
- Android screencaps must produce Android-keyed visual baseline artifacts.
- The resulting receipts must be attached before #8541 can close.

## Verification

- PASS: `bun test tests/maestro-policy.test.ts tests/ux-contracts.test.ts`
  in `clients/khala-mobile`
  - 18 pass, 0 fail
- PASS: `bun test src/mobile-nightly.test.ts` in `apps/qa-runner`
  - 6 pass, 0 fail
- PASS: `bun run --cwd apps/qa-runner typecheck`
- PASS: `bun run --cwd clients/khala-mobile qa:mobile:gate`
  - TypeScript pass
  - Dependency Cruiser pass
  - 398 pass, 0 fail
  - receipt: `clients/khala-mobile/var/qa-mobile-gate/khala-mobile-release-gate.latest.json`
- BLOCKED OUT OF SCOPE: `bun run check:deploy`
  - Existing architecture budget failures remain in Worker and desktop/tooling
    surfaces outside the QAM-6 edit scope, including:
    - `workers/api/src/khala-cloud-runtime-dispatch.ts`
    - `workers/api/src/khala-cloud-runtime-inference-block.ts`
    - `workers/api/src/khala-cloud-runtime-execution-token.ts`
    - `workers/api/src/khala-cloud-runtime-dispatch-admin-route.ts`
    - `workers/api/src/cloud/cloud-coding-session-routes.ts`
