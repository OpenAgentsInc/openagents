# QAM-4 Mobile Visual Tier Receipt

Issue: #8539

Status: simulator-backed pass for the owned baseline/blessing workflow, one
real iOS simulator screen checkpoint, and a verified on-device Storybook boot
on a separate iPhone Air simulator.

## Evidence

- `packages/khala-qa-harness/src/mobile-visual-tier.ts` wraps the existing
  `openagents.khala_visual_baselines.v1` engine for mobile captures.
- `packages/khala-qa-harness/src/mobile-visual-tier.test.ts` proves:
  - a starter baseline set can be blessed,
  - an unexplained changed screenshot is blocking,
  - an intentional changed screenshot can be blessed with a recorded reason.
- `clients/khala-mobile/src/qa/mobile-release-gate.ts` exposes the QAM-4
  visual-tier status to the mobile gate and explicitly separates simulator
  screenshot truth from fixture proof.
- `docs/khala-code/receipts/qam-4-sim-captures/iphone-17e-launch.png` was
  captured from booted simulator `iPhone 17e`
  (`8FFF77DA-C4D1-4CCD-ACB7-614A79D7216F`) with:
  `xcrun simctl io ... screenshot`.
- `docs/khala-code/receipts/qam-4-baselines/manifest.json` and
  `docs/khala-code/receipts/qam-4-baselines/screenshots/khala.mobile.screen.thread-empty.iphone-17e.dark.png`
  are the first simulator-backed blessed baseline set.
- `docs/khala-code/receipts/2026-07-07-qam-4-simulator-baseline-result.json`
  records the blessed result for
  `khala.mobile.screen.thread-empty.iphone-17e.dark`.
- `docs/khala-code/receipts/2026-07-07-qam-4-visual-tier.json` records
  `qa:mobile:gate` PASS.
- `docs/khala-code/receipts/2026-07-07-qam-4-storybook-rebuild-blocker.json`
  records the V1 Storybook device attempt as INCONCLUSIVE. The installed app
  did not request the 8082 Storybook bundle, and the Storybook rebuild failed
  before launch with Xcode linker error 65:
  `cannot link directly with SwiftUICore`.
- `docs/khala-code/receipts/2026-07-07-qam-4-storybook-iphone-air.json`
  supersedes the Storybook rebuild blocker for local iOS simulator Storybook
  boot. It records the iPhone Air simulator run, the `.rnstorybook/index.ts`
  Metro bundle line, and the tracked Expo config plugin that reapplies the
  native prebuild fixes.
- `docs/khala-code/receipts/qam-4-storybook-captures/iphone-air-storybook-initial.png`
  is the verified iPhone Air Storybook screenshot from simulator
  `DDE26798-3F92-46B4-B98B-0EDA5F32E672`.

## Limits

- The iPhone Air receipt proves local simulator Storybook boot and visual
  capture. It does not claim hosted CI, EAS, TestFlight, App Store, or owned-Mac
  scheduled execution.
- The screenshot is public-safe empty-thread state only. No private account
  data, prompts, bearer material, or provider payloads are present.
