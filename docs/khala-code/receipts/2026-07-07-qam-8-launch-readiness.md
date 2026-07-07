# QAM-8 / P0.8 Khala Mobile Launch Readiness Receipt

Date: 2026-07-07
Issue: OpenAgentsInc/openagents#8543
Schema: `openagents.khala_mobile.launch_readiness.v1`
Verdict: `INCONCLUSIVE`

## What Is Recorded

This receipt records the launch-readiness delta between the existing
`khala_mobile.platform.launched_app_interaction_smoke.v1` evidence and the
stricter #8543 exit criteria.

Existing evidence proves a signed-in iOS simulator thread smoke and Android
launch/sign-in handoff. It does not yet prove the full straight-line launch path
on both platforms:

1. sign in,
2. visible $10 grant,
3. repo selection,
4. turn dispatch,
5. live updates,
6. push/writeback link,
7. credits drain.

## Checks

| Check | Verdict | Blocker |
| --- | --- | --- |
| Seeded public-safe GitHub test account | `OWNER_GATED` | `owner.github_seeded_public_safe_account` |
| iOS + Android full straight-line E2E | `INCONCLUSIVE` | `blocker.ios.full_straight_line_e2e_missing_receipt`, `blocker.android.full_straight_line_e2e_missing_receipt` |
| Promises/copy pass | `INCONCLUSIVE` | `blocker.launch_copy_owner_signoff_missing`, `blocker.promise_registry_no_green_flip_without_e2e` |

## Evidence Refs

- `clients/khala-mobile/src/qa/launch-readiness.ts`
- `clients/khala-mobile/tests/launch-readiness.test.ts`
- `NEEDS_OWNER.md#khala-mobile-p08-launch-readiness`
- `docs/khala-mobile/2026-07-07-signed-in-thread-smoke-receipt.md`
- `docs/khala-code/receipts/2026-07-07-qam-6-android-lane-definition.md`

## Why #8543 Stays Open

#8543 requires enforced full E2E receipts on both iOS simulator and Android
emulator, plus an owner-approved promises/copy pass. Those proofs are
device/account truths and cannot be simulated into a closing receipt.

The owner-gated steps are documented in `NEEDS_OWNER.md`.
