/**
 * Blueprint Signature 5 — `merge-deploy-gate`
 *
 * Merged != deployed; main never left red.
 *
 * Pure, ordered-predicate state machine implementing the gate from
 * `docs/artanis/2026-06-28-blueprint-signature-governed-autonomous-ops.md`:
 *
 *   MERGED → CHECK_DEPLOY_GREEN → DEPLOYED → SMOKED → LIVE
 *
 * Only the terminal state `LIVE` unlocks the claim "deployment live and
 * verified." Any failed gate transitions to the `RED` state, which BLOCKS all
 * further merges until a rollback evidence ref is presented.
 *
 * Note (the trailing-echo subtlety): the gate verifies the REAL `check:deploy`
 * exit code, not a wrapper's trailing-echo exit. If `checkDeployStdout` contains
 * any `EXIT=<n>` marker that is nonzero, the gate treats the run as failed even
 * when the captured `checkDeployExitCode` is 0.
 *
 * This module is a pure library (types + functions). It is intentionally not
 * wired into the live supervisor/watcher; wiring is a follow-up.
 */

export const MERGE_DEPLOY_STATES = [
  "MERGED",
  "CHECK_DEPLOY_GREEN",
  "DEPLOYED",
  "SMOKED",
  "LIVE",
  "RED",
] as const

export type MergeDeployState = (typeof MERGE_DEPLOY_STATES)[number]

export const MERGE_DEPLOY_EVIDENCE = {
  checkDeployPass: "evidence://merge/check-deploy-pass",
  deployExitCode: "evidence://deploy/exit-code",
  smokeTests: "evidence://deploy/smoke-tests",
  rollback: "evidence://deploy/rollback",
} as const

export type MergeDeployEvidenceRef =
  (typeof MERGE_DEPLOY_EVIDENCE)[keyof typeof MERGE_DEPLOY_EVIDENCE]

export interface SmokeTestResult {
  readonly name: string
  readonly passed: boolean
}

export interface MergeDeployGateInputs {
  readonly prNumbers: ReadonlyArray<number>
  readonly mergeCommitHashes: ReadonlyArray<string>
  /** Captured exit code of `bun run check:deploy` against main-after-merge. */
  readonly checkDeployExitCode: number
  /** Full stdout of the check:deploy run (scanned for `EXIT=<n>` markers). */
  readonly checkDeployStdout: string
  /** Exit code of the deploy step; `null` means deploy was not attempted. */
  readonly deployExitCode: number | null
  readonly smokeTestResults: ReadonlyArray<SmokeTestResult>
  /**
   * evidence://deploy/rollback — a rollback evidence ref. When the gate is RED,
   * presenting this ref unblocks further merges.
   */
  readonly rollbackEvidenceRef?: string | null
}

export interface MergeDeployGateResult {
  readonly state: MergeDeployState
  /** True only when the terminal state LIVE is reached. */
  readonly isLive: boolean
  readonly identity: Readonly<{
    readonly prNumbers: ReadonlyArray<number>
    readonly mergeCommitHashes: ReadonlyArray<string>
  }>
  readonly isRed: boolean
  /** True while a RED gate has no rollback evidence ref presented. */
  readonly blocksFurtherMerges: boolean
  /** The named gate that failed (when RED), else null. */
  readonly failedGate: MergeDeployState | null
  readonly satisfiedEvidence: ReadonlyArray<MergeDeployEvidenceRef>
  readonly missingEvidence: ReadonlyArray<MergeDeployEvidenceRef>
  readonly blockedReason: string | null
}

const EXIT_MARKER = /\bEXIT=(\d+)\b/g

/**
 * Parse every `EXIT=<n>` marker emitted in stdout. Used to defeat a wrapper's
 * trailing-echo masking a real nonzero exit.
 */
export function parseExitMarkers(stdout: string): ReadonlyArray<number> {
  if (typeof stdout !== "string" || stdout.length === 0) {
    return []
  }
  const found: Array<number> = []
  EXIT_MARKER.lastIndex = 0
  let match: RegExpExecArray | null
  while ((match = EXIT_MARKER.exec(stdout)) !== null) {
    const parsed = Number.parseInt(match[1], 10)
    if (Number.isFinite(parsed)) {
      found.push(parsed)
    }
  }
  return found
}

/**
 * The effective check:deploy result. Green only when the captured exit code is 0
 * AND no `EXIT=<n>` marker in stdout is nonzero.
 */
export function checkDeployIsGreen(
  checkDeployExitCode: number,
  checkDeployStdout: string,
): boolean {
  if (checkDeployExitCode !== 0) {
    return false
  }
  return parseExitMarkers(checkDeployStdout).every((code) => code === 0)
}

/**
 * Evaluate the `merge-deploy-gate`. Pure function.
 */
export function evaluateMergeDeployGate(
  inputs: MergeDeployGateInputs,
): MergeDeployGateResult {
  const satisfied: Array<MergeDeployEvidenceRef> = []
  const identity = {
    prNumbers: inputs.prNumbers,
    mergeCommitHashes: inputs.mergeCommitHashes,
  }
  const hasRollback =
    typeof inputs.rollbackEvidenceRef === "string" &&
    inputs.rollbackEvidenceRef.trim().length > 0

  const red = (
    failedGate: MergeDeployState,
    blockedReason: string,
    missing: ReadonlyArray<MergeDeployEvidenceRef>,
  ): MergeDeployGateResult => ({
    state: "RED",
    isLive: false,
    identity,
    isRed: true,
    blocksFurtherMerges: !hasRollback,
    failedGate,
    satisfiedEvidence: satisfied,
    missingEvidence: hasRollback
      ? missing.filter((ref) => ref !== MERGE_DEPLOY_EVIDENCE.rollback)
      : [...missing, MERGE_DEPLOY_EVIDENCE.rollback],
    blockedReason: hasRollback
      ? `${blockedReason} — rollback evidence presented; merges unblocked`
      : `${blockedReason} — main RED, rollback required; further merges blocked`,
  })

  // Gate 1 — MERGED: PRs and matching merge commit hashes present.
  if (inputs.prNumbers.length === 0 || inputs.mergeCommitHashes.length === 0) {
    return red(
      "MERGED",
      "no merged PRs / merge commit hashes provided",
      [],
    )
  }
  if (inputs.prNumbers.length !== inputs.mergeCommitHashes.length) {
    return red(
      "MERGED",
      `PR count (${inputs.prNumbers.length}) does not match merge commit count (${inputs.mergeCommitHashes.length})`,
      [],
    )
  }

  // Gate 2 — CHECK_DEPLOY_GREEN: real check:deploy exit is 0.
  if (!checkDeployIsGreen(inputs.checkDeployExitCode, inputs.checkDeployStdout)) {
    return red(
      "CHECK_DEPLOY_GREEN",
      `check:deploy was not green (exit ${inputs.checkDeployExitCode}, markers ${JSON.stringify(parseExitMarkers(inputs.checkDeployStdout))})`,
      [MERGE_DEPLOY_EVIDENCE.checkDeployPass],
    )
  }
  satisfied.push(MERGE_DEPLOY_EVIDENCE.checkDeployPass)

  // Gate 3 — DEPLOYED: deploy step exited 0.
  if (inputs.deployExitCode !== 0) {
    return red(
      "DEPLOYED",
      inputs.deployExitCode === null
        ? "deploy was not attempted"
        : `deploy exited ${inputs.deployExitCode}`,
      [MERGE_DEPLOY_EVIDENCE.deployExitCode],
    )
  }
  satisfied.push(MERGE_DEPLOY_EVIDENCE.deployExitCode)

  // Gate 4 — SMOKED: at least one smoke ran and all passed.
  if (
    inputs.smokeTestResults.length === 0 ||
    inputs.smokeTestResults.some((smoke) => !smoke.passed)
  ) {
    const failed = inputs.smokeTestResults
      .filter((smoke) => !smoke.passed)
      .map((smoke) => smoke.name)
    return red(
      "SMOKED",
      inputs.smokeTestResults.length === 0
        ? "no smoke tests were run"
        : `smoke tests failed: ${failed.join(", ")}`,
      [MERGE_DEPLOY_EVIDENCE.smokeTests],
    )
  }
  satisfied.push(MERGE_DEPLOY_EVIDENCE.smokeTests)

  return {
    state: "LIVE",
    isLive: true,
    identity,
    isRed: false,
    blocksFurtherMerges: false,
    failedGate: null,
    satisfiedEvidence: satisfied,
    missingEvidence: [],
    blockedReason: null,
  }
}
