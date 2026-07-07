export const KhalaMobileStoreSubmissionSchemaId =
  "openagents.khala_mobile.store_submissions.v1" as const

export type KhalaMobileStoreSubmissionReviewState =
  | "not_submitted"
  | "in_review"
  | "approved"
  | "rejected"

export type KhalaMobileStoreSubmissionPlatform = "ios_app_store_connect" | "android_play_console"

export type KhalaMobileStoreSubmissionRecord = Readonly<{
  platform: KhalaMobileStoreSubmissionPlatform
  issue: 8544
  submissionId: string | null
  reviewState: KhalaMobileStoreSubmissionReviewState
  buildRef: string | null
  consoleUrl: string
  blockerRefs: readonly string[]
  requiredOwnerActions: readonly string[]
  evidenceRefs: readonly string[]
}>

export type KhalaMobileStoreSubmissionReceipt = Readonly<{
  schema: typeof KhalaMobileStoreSubmissionSchemaId
  issue: 8544
  generatedOn: "2026-07-07"
  p0ExitSatisfied: false
  submissions: readonly [KhalaMobileStoreSubmissionRecord, KhalaMobileStoreSubmissionRecord]
}>

export const khalaMobileStoreSubmissionOwnerActions = [
  "Create or confirm the App Store Connect app record for com.openagents.khala.mobile.",
  "Upload the final locally built iOS archive through Apple Transporter or Xcode Organizer.",
  "Enter current App Store metadata, screenshots, privacy answers, age rating, and review notes.",
  "Submit the iOS build for review and record the App Store Connect submission ID and review state.",
  "Create or confirm the Play Console app record for com.openagents.khala.mobile.",
  "Upload the final locally signed Android App Bundle to the intended Play track.",
  "Enter current Play listing, data-safety, content-rating, tester/release notes, and review answers.",
  "Submit the Play release and record the Play Console release/submission ID and review state.",
] as const

export const khalaMobileStoreSubmissionReceipt: KhalaMobileStoreSubmissionReceipt = {
  generatedOn: "2026-07-07",
  issue: 8544,
  p0ExitSatisfied: false,
  schema: KhalaMobileStoreSubmissionSchemaId,
  submissions: [
    {
      blockerRefs: [
        "owner.app_store_connect_submission_required",
        "blocker.ios.submission_id_missing",
        "blocker.ios.review_state_missing",
        "blocker.p08.full_launch_e2e_not_green",
      ],
      buildRef: null,
      consoleUrl: "https://appstoreconnect.apple.com/apps",
      evidenceRefs: [
        "docs/khala-mobile/2026-07-06-app-store-submission-pack.md",
        "docs/khala-code/receipts/2026-07-07-qam-8-launch-readiness.md",
        "NEEDS_OWNER.md#khala-mobile-p09-store-submissions",
      ],
      issue: 8544,
      platform: "ios_app_store_connect",
      requiredOwnerActions: khalaMobileStoreSubmissionOwnerActions.slice(0, 4),
      reviewState: "not_submitted",
      submissionId: null,
    },
    {
      blockerRefs: [
        "owner.play_console_submission_required",
        "blocker.android.submission_id_missing",
        "blocker.android.review_state_missing",
        "blocker.p08.full_launch_e2e_not_green",
        "blocker.android.release_aab_not_receipted",
      ],
      buildRef: null,
      consoleUrl: "https://play.google.com/console",
      evidenceRefs: [
        "docs/khala-mobile/2026-07-06-android-build-and-upload-runbook.md",
        "docs/khala-code/receipts/2026-07-07-qam-6-android-lane-definition.md",
        "docs/khala-code/receipts/2026-07-07-qam-8-launch-readiness.md",
        "NEEDS_OWNER.md#khala-mobile-p09-store-submissions",
      ],
      issue: 8544,
      platform: "android_play_console",
      requiredOwnerActions: khalaMobileStoreSubmissionOwnerActions.slice(4),
      reviewState: "not_submitted",
      submissionId: null,
    },
  ],
}

export const khalaMobileStoreSubmissionsAreP0Complete = (
  receipt: KhalaMobileStoreSubmissionReceipt,
): boolean =>
  receipt.p0ExitSatisfied &&
  receipt.submissions.every(
    submission => submission.submissionId !== null && submission.reviewState === "in_review",
  )
