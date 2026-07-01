/**
 * Blueprint Signature 3 — `issue-close-safe`
 *
 * Closing an issue requires proof it is safe to close, with EPICs protected.
 *
 * This is a pure, ordered-predicate state machine implementing the gate from
 * `docs/artanis/2026-06-28-blueprint-signature-governed-autonomous-ops.md`:
 *
 *   UNCHECKED → LABELS_READ → EPIC_SAFE → CLOSE_VERIFIED → SAFE_TO_CLOSE
 *
 * Only the terminal state `SAFE_TO_CLOSE` unlocks emitting a `Closes #N`.
 *
 * State semantics: each named state above the start represents an ordered
 * predicate that has been satisfied. The reported `state` is the highest
 * predicate that passed; the machine stops at the first predicate that fails.
 *
 * EPIC protection (the #6376 failure class — a single sub-PR auto-closing an
 * epic):
 *  - If THIS issue is itself an EPIC, the normal gate refuses. An EPIC may only
 *    be closed through a separate, higher-risk-ceiling path, modeled here as the
 *    explicit `epicCloseAuthorized` guard flag. Without it the gate locks at the
 *    EPIC_SAFE stage.
 *  - If this issue is a sub-issue of an EPIC and is NOT the last open sub-issue,
 *    the gate locks at the EPIC_SAFE stage and refuses to advance.
 *
 * This module is a pure library (types + functions). It is intentionally not
 * wired into the live supervisor/watcher; wiring is a follow-up.
 */

export const ISSUE_CLOSE_SAFE_STATES = [
  "UNCHECKED",
  "LABELS_READ",
  "EPIC_SAFE",
  "CLOSE_VERIFIED",
  "SAFE_TO_CLOSE",
] as const

export type IssueCloseSafeState = (typeof ISSUE_CLOSE_SAFE_STATES)[number]

export const ISSUE_CLOSE_SAFE_EVIDENCE = {
  labels: "evidence://issue/labels",
  parentEpicCheck: "evidence://issue/parent-epic-check",
  prBodyContainsCloses: "evidence://pr/body-contains-closes",
} as const

export type IssueCloseSafeEvidenceRef =
  (typeof ISSUE_CLOSE_SAFE_EVIDENCE)[keyof typeof ISSUE_CLOSE_SAFE_EVIDENCE]

export interface IssueCloseSafeInputs {
  readonly issueNumber: number
  /** evidence://issue/labels — the full label set read from the issue. */
  readonly issueLabels: ReadonlyArray<string> | null
  /** Parent EPIC issue number, or null when this issue has no parent EPIC. */
  readonly parentEpicNumber: number | null
  readonly prNumber: number
  /** evidence://pr/body-contains-closes — the PR body to scan for `Closes #N`. */
  readonly prBody: string
  /**
   * evidence://issue/parent-epic-check — when `parentEpicNumber !== null`,
   * whether this issue is the last open sub-issue of that EPIC. Required to be
   * `true` before a sub-issue close can advance past the EPIC_SAFE stage.
   */
  readonly isLastOpenSubIssue?: boolean
  /**
   * Explicit marker that THIS issue is itself an EPIC. When omitted, epic-ness
   * is inferred from the label set (any label equal to "epic", case-insensitive).
   */
  readonly isEpic?: boolean
  /**
   * Higher-risk-ceiling guard. Closing an EPIC itself is structurally blocked
   * unless this is explicitly `true` (the separate manual signature path).
   */
  readonly epicCloseAuthorized?: boolean
}

export interface IssueCloseSafeResult {
  readonly state: IssueCloseSafeState
  /** True only when the terminal state SAFE_TO_CLOSE is reached. */
  readonly canClose: boolean
  readonly identity: Readonly<{
    readonly issueNumber: number
    readonly prNumber: number
  }>
  /** Whether THIS issue was treated as an EPIC. */
  readonly isEpic: boolean
  readonly satisfiedEvidence: ReadonlyArray<IssueCloseSafeEvidenceRef>
  readonly missingEvidence: ReadonlyArray<IssueCloseSafeEvidenceRef>
  /** True when an ordered predicate failed and halted the machine. */
  readonly locked: boolean
  /** The stage the machine is locked at (the predicate it could not pass). */
  readonly lockedAt: IssueCloseSafeState | null
  readonly blockedReason: string | null
}

const STANDARD_CLOSING_KEYWORD =
  /\b(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\s+#(\d+)/gi

/** Extract every issue number referenced by a GitHub closing keyword. */
export function extractClosesIssueNumbers(prBody: string): ReadonlyArray<number> {
  if (typeof prBody !== "string" || prBody.length === 0) {
    return []
  }
  const found: Array<number> = []
  // Reset lastIndex defensively since the regex is module-scoped with /g.
  STANDARD_CLOSING_KEYWORD.lastIndex = 0
  let match: RegExpExecArray | null
  while ((match = STANDARD_CLOSING_KEYWORD.exec(prBody)) !== null) {
    const parsed = Number.parseInt(match[1], 10)
    if (Number.isFinite(parsed)) {
      found.push(parsed)
    }
  }
  return found
}

/** Whether the PR body contains a closing keyword targeting `issueNumber`. */
export function prBodyClosesIssue(prBody: string, issueNumber: number): boolean {
  return extractClosesIssueNumbers(prBody).includes(issueNumber)
}

/** Whether the issue should be treated as an EPIC. */
export function issueIsEpic(inputs: IssueCloseSafeInputs): boolean {
  if (typeof inputs.isEpic === "boolean") {
    return inputs.isEpic
  }
  if (!Array.isArray(inputs.issueLabels)) {
    return false
  }
  return inputs.issueLabels.some((label) => label.trim().toLowerCase() === "epic")
}

/**
 * Evaluate the `issue-close-safe` gate. Pure function: same inputs always
 * produce the same result.
 */
export function evaluateIssueCloseSafe(
  inputs: IssueCloseSafeInputs,
): IssueCloseSafeResult {
  const satisfied: Array<IssueCloseSafeEvidenceRef> = []
  const isEpic = issueIsEpic(inputs)
  const identity = {
    issueNumber: inputs.issueNumber,
    prNumber: inputs.prNumber,
  }

  const lock = (
    state: IssueCloseSafeState,
    lockedAt: IssueCloseSafeState,
    blockedReason: string,
    missing: ReadonlyArray<IssueCloseSafeEvidenceRef>,
  ): IssueCloseSafeResult => ({
    state,
    canClose: false,
    identity,
    isEpic,
    satisfiedEvidence: satisfied,
    missingEvidence: missing,
    locked: true,
    lockedAt,
    blockedReason,
  })

  // Predicate 1 — labels read.
  if (!Array.isArray(inputs.issueLabels)) {
    return lock("UNCHECKED", "LABELS_READ", "issue label set was not provided", [
      ISSUE_CLOSE_SAFE_EVIDENCE.labels,
    ])
  }
  satisfied.push(ISSUE_CLOSE_SAFE_EVIDENCE.labels)

  // Predicate 2 — EPIC safety.
  if (isEpic) {
    if (inputs.epicCloseAuthorized !== true) {
      return lock(
        "LABELS_READ",
        "EPIC_SAFE",
        "this issue is an EPIC; closing it requires the separate higher-risk-ceiling path (epicCloseAuthorized)",
        [
          ISSUE_CLOSE_SAFE_EVIDENCE.parentEpicCheck,
          ISSUE_CLOSE_SAFE_EVIDENCE.prBodyContainsCloses,
        ],
      )
    }
  } else if (inputs.parentEpicNumber !== null) {
    if (inputs.isLastOpenSubIssue !== true) {
      return lock(
        "LABELS_READ",
        "EPIC_SAFE",
        `issue #${inputs.issueNumber} is a sub-issue of EPIC #${inputs.parentEpicNumber} and is not the last open sub-issue`,
        [
          ISSUE_CLOSE_SAFE_EVIDENCE.parentEpicCheck,
          ISSUE_CLOSE_SAFE_EVIDENCE.prBodyContainsCloses,
        ],
      )
    }
  }
  satisfied.push(ISSUE_CLOSE_SAFE_EVIDENCE.parentEpicCheck)

  // Predicate 3 — PR body contains a matching `Closes #N`.
  if (!prBodyClosesIssue(inputs.prBody, inputs.issueNumber)) {
    return lock(
      "EPIC_SAFE",
      "CLOSE_VERIFIED",
      `PR #${inputs.prNumber} body does not contain a closing keyword for issue #${inputs.issueNumber}`,
      [ISSUE_CLOSE_SAFE_EVIDENCE.prBodyContainsCloses],
    )
  }
  satisfied.push(ISSUE_CLOSE_SAFE_EVIDENCE.prBodyContainsCloses)

  return {
    state: "SAFE_TO_CLOSE",
    canClose: true,
    identity,
    isEpic,
    satisfiedEvidence: satisfied,
    missingEvidence: [],
    locked: false,
    lockedAt: null,
    blockedReason: null,
  }
}
