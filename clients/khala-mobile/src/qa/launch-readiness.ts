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
  generatedOn: "2026-07-07" | "2026-07-09"
  overallVerdict: KhalaMobileLaunchReadinessVerdict
  checks: readonly KhalaMobileLaunchReadinessCheck[]
}>

export const khalaMobileLaunchReadinessReceipt: KhalaMobileLaunchReadinessReceipt = {
  checks: [
    {
      blockerRefs: [],
      evidenceRefs: [
        "docs/khala-mobile/2026-07-09-straight-line-e2e-agentflampy-receipt.md",
        "clients/khala-mobile/src/qa/straight-line-e2e.ts",
        "clients/khala-mobile/scripts/build-seeded-ios.sh",
        "clients/khala-mobile/scripts/straight-line-e2e-run.sh",
      ],
      id: "seeded_public_safe_github_account",
      notes:
        "RESOLVED 2026-07-09 (recorded on #8543): the owner-approved public-safe seed is GitHub user AgentFlampy (created 2026-07-07, public) with the fork AgentFlampy/openagents (verified fork of OpenAgentsInc/openagents). The seeded Khala credential lives only in the gitignored ~/work/.secrets/khala-maestro.env and is wired into the harness as the sign-in identity; the fork is the repo-pick target. The repo-list/credits routes additionally need a one-time captured mobile OpenAuth USER session (tracked on the E2E check below) — a session-capture step, not a missing account.",
      ownerActionRefs: ["NEEDS_OWNER.md#khala-mobile-p08-launch-readiness"],
      requiredForIssue: 8543,
      title: "Seeded public-safe GitHub test account",
      verdict: "PASS",
    },
    {
      blockerRefs: [
        "blocker.ios.full_straight_line_e2e_missing_receipt",
        "blocker.android.full_straight_line_e2e_missing_receipt",
        "blocker.khala_mobile.repo_list_requires_github_backed_mobile_session",
        "blocker.cx3.in_vm_cloud_execution_lane_missing.openagents#8547",
      ],
      evidenceRefs: [
        "clients/khala-mobile/.maestro/flows/SignedInThreadSmoke.yaml",
        "clients/khala-mobile/.maestro/flows/RepoPickerReachable.yaml",
        "clients/khala-mobile/.maestro/flows/StraightLineRepoPick.yaml",
        "clients/khala-mobile/src/qa/straight-line-e2e.ts",
        "docs/khala-mobile/2026-07-09-straight-line-e2e-agentflampy-receipt.md",
        "docs/khala-code/receipts/2026-07-07-qam-6-android-lane-definition.md",
      ],
      id: "ios_android_full_straight_line_e2e",
      notes:
        "The unattended harness is wired to the AgentFlampy seed (src/qa/straight-line-e2e.ts is the typed leg registry). Runnable legs are receipted in the 2026-07-09 straight-line receipt; the repo-bind and credits legs remain gated on a one-time captured AgentFlampy mobile OpenAuth session (the seeded agent token 401s those user-session-only routes BY DESIGN), and the push/writeback leg remains gated on CX-3's in-VM cloud-execution lane (#8547). The FULL #8543 path on both platforms is therefore still not proven.",
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
  generatedOn: "2026-07-09",
  issue: 8543,
  overallVerdict: "INCONCLUSIVE",
  schema: KhalaMobileLaunchReadinessSchemaId,
}

/** The original 2026-07-07 owner-gate action list. It now lives verbatim in
 * the NEEDS_OWNER archive (docs/ops/2026-07-09-needs-owner-archive.md) after
 * the 2026-07-09 NEEDS_OWNER trim; the first action resolved 2026-07-09
 * (AgentFlampy + fork, recorded on #8543). */
export const khalaMobileLaunchReadinessOwnerActions = [
  "Create or approve a public-safe GitHub test account for Khala Mobile launch readiness.",
  "Grant only the repo scopes needed for the smoke repo and writeback proof.",
  "Seed a visible $10 launch credit grant and record the public-safe grant receipt ref.",
  "Run the full straight-line E2E on iOS simulator and Android emulator.",
  "Review the launch promises/copy pass only after both platform E2E receipts exist.",
] as const

/** The single REMAINING owner tap for the harness (2026-07-09): one
 * interactive GitHub sign-in as AgentFlampy so the mobile-user-session-only
 * legs (repo list/bind, credits) can run. Kept in the live NEEDS_OWNER.md. */
export const khalaMobileLaunchReadinessRemainingOwnerAsk =
  "one GitHub sign-in as AgentFlampy" as const

export const isKhalaMobileLaunchReady = (
  receipt: KhalaMobileLaunchReadinessReceipt,
): boolean => receipt.overallVerdict === "PASS" && receipt.checks.every(check => check.verdict === "PASS")
