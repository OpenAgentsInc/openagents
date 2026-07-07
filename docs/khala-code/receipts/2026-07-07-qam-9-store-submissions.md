# QAM-9 / P0.9 Khala Mobile Store Submission Receipt

Date: 2026-07-07
Issue: OpenAgentsInc/openagents#8544
Schema: `openagents.khala_mobile.store_submissions.v1`
P0 exit satisfied: `false`

## Submission State

| Platform | Submission ID | Review state | Verdict |
| --- | --- | --- | --- |
| App Store Connect | _missing_ | `not_submitted` | blocked |
| Play Console | _missing_ | `not_submitted` | blocked |

## Evidence Refs

- `clients/khala-mobile/src/qa/store-submissions.ts`
- `clients/khala-mobile/tests/store-submissions.test.ts`
- `NEEDS_OWNER.md#khala-mobile-p09-store-submissions`
- `docs/khala-mobile/2026-07-06-app-store-submission-pack.md`
- `docs/khala-mobile/2026-07-06-android-build-and-upload-runbook.md`
- `docs/khala-code/receipts/2026-07-07-qam-8-launch-readiness.md`

## Blockers

- `owner.app_store_connect_submission_required`
- `owner.play_console_submission_required`
- `blocker.ios.submission_id_missing`
- `blocker.android.submission_id_missing`
- `blocker.ios.review_state_missing`
- `blocker.android.review_state_missing`
- `blocker.p08.full_launch_e2e_not_green`
- `blocker.android.release_aab_not_receipted`

## Why #8544 Stays Open

#8544 defines "Submitted" as in review at both stores, with both submission IDs
and review states recorded as registry evidence. No App Store Connect or Play
Console submission was executed in this environment, and P0.8 still lacks the
full launch E2E receipts that should feed the final submission pack. The exact
owner console steps and URLs are recorded in `NEEDS_OWNER.md`.
