export const KhalaMobileLaunchReadinessSchemaId =
  "openagents.khala_mobile.launch_readiness.v1" as const

export type KhalaMobileLaunchReadinessVerdict =
  | "PASS"
  | "FAIL"
  | "INCONCLUSIVE"
  | "OWNER_GATED"

export type KhalaMobileLaunchReadinessCheck = Readonly<{
  id: string
  title: string
  requiredForIssue: number
  verdict: KhalaMobileLaunchReadinessVerdict
  evidenceRefs: readonly string[]
  blockerRefs: readonly string[]
  ownerActionRefs: readonly string[]
  notes: string
}>

export type KhalaMobileLaunchReadinessReceipt = Readonly<{
  schema: typeof KhalaMobileLaunchReadinessSchemaId
  issue: 8543
  generatedOn: "2026-07-07"
  overallVerdict: KhalaMobileLaunchReadinessVerdict
  checks: readonly KhalaMobileLaunchReadinessCheck[]
}>

export const khalaMobileLaunchReadinessReceipt: KhalaMobileLaunchReadinessReceipt = {
  checks: [
    {
      blockerRefs: ["owner.github_seeded_public_safe_account"],
      evidenceRefs: [
        "docs/khala-mobile/2026-07-07-signed-in-thread-smoke-receipt.md",
        "clients/khala-mobile/scripts/signed-in-thread-smoke-run.sh",
      ],
      id: "seeded_public_safe_github_account",
      notes:
        "The older AgentFlampy smoke account proves a signed-in thread smoke, but #8543 needs an owner-approved launch seed with repo authorization, credit grant visibility, dispatch permission, and writeback scope.",
      ownerActionRefs: ["NEEDS_OWNER.md#khala-mobile-p08-launch-readiness"],
      requiredForIssue: 8543,
      title: "Seeded public-safe GitHub test account",
      verdict: "OWNER_GATED",
    },
    {
      blockerRefs: [
        "blocker.ios.full_straight_line_e2e_missing_receipt",
        "blocker.android.full_straight_line_e2e_missing_receipt",
      ],
      evidenceRefs: [
        "clients/khala-mobile/.maestro/flows/SignedInThreadSmoke.yaml",
        "docs/khala-mobile/2026-07-07-signed-in-thread-smoke-receipt.md",
        "docs/khala-code/receipts/2026-07-07-qam-6-android-lane-definition.md",
      ],
      id: "ios_android_full_straight_line_e2e",
      notes:
        "Existing receipts stop at signed-in interaction and Android launch/sign-in handoff. They do not prove the #8543 path: sign in, $10 grant visible, pick repo, dispatch turn, live updates, push/writeback link, and credit drain on both platforms.",
      ownerActionRefs: ["NEEDS_OWNER.md#khala-mobile-p08-launch-readiness"],
      requiredForIssue: 8543,
      title: "Full straight-line E2E on iOS simulator and Android emulator",
      verdict: "INCONCLUSIVE",
    },
    {
      blockerRefs: [
        "blocker.launch_copy_owner_signoff_missing",
        "blocker.promise_registry_no_green_flip_without_e2e",
      ],
      evidenceRefs: [
        "clients/khala-mobile/src/i18n/copy.ts",
        "clients/khala-mobile/tests/ux-contracts.test.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      id: "promises_copy_pass",
      notes:
        "The app copy remains bounded by existing copy tests, but #8543's launch promises/copy pass is not green until the owner signs off against the complete iOS and Android E2E receipts.",
      ownerActionRefs: ["NEEDS_OWNER.md#khala-mobile-p08-launch-readiness"],
      requiredForIssue: 8543,
      title: "Promises/copy pass for launch surfaces",
      verdict: "INCONCLUSIVE",
    },
  ],
  generatedOn: "2026-07-07",
  issue: 8543,
  overallVerdict: "INCONCLUSIVE",
  schema: KhalaMobileLaunchReadinessSchemaId,
}

export const khalaMobileLaunchReadinessOwnerActions = [
  "Create or approve a public-safe GitHub test account for Khala Mobile launch readiness.",
  "Grant only the repo scopes needed for the smoke repo and writeback proof.",
  "Seed a visible $10 launch credit grant and record the public-safe grant receipt ref.",
  "Run the full straight-line E2E on iOS simulator and Android emulator.",
  "Review the launch promises/copy pass only after both platform E2E receipts exist.",
] as const

export const isKhalaMobileLaunchReady = (
  receipt: KhalaMobileLaunchReadinessReceipt,
): boolean => receipt.overallVerdict === "PASS" && receipt.checks.every(check => check.verdict === "PASS")
