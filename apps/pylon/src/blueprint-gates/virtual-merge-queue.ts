import { createHash } from "node:crypto"

/**
 * Pylon-native Virtual Merge Queue.
 *
 * The supervisor should branch new fleet work from the queue's `virtualHead`,
 * not stale `origin/main`. Verified candidates then move from virtual state to
 * actual GitHub PR/merge promotion one at a time. This keeps branch protection
 * intact while moving merge serialization into deterministic local planning.
 *
 * Pure library only: no git, no GitHub API, no filesystem writes.
 */

export type VirtualMergeQueueCandidate = {
  readonly candidateRef: string
  readonly issueNumber: number
  readonly baseCommit: string
  readonly patchCommit: string
  readonly changedFiles?: ReadonlyArray<VirtualMergeQueueFileChange>
  readonly signedOperatorOverrideRef?: string | undefined
  readonly verificationPassed: boolean
  readonly issueOpen: boolean
  readonly hasOpenPullRequest: boolean
  readonly queuedAt: string
}

export type VirtualMergeQueueFileChange = {
  readonly path: string
  readonly status: "added" | "modified" | "deleted" | "renamed"
}

export type VirtualMergeQueueReadyEntry = {
  readonly candidateRef: string
  readonly issueNumber: number
  readonly virtualBaseCommit: string
  readonly virtualHeadCommit: string
  readonly promotionRef: string
  readonly waitsForActualHead: string | null
}

export type VirtualMergeQueueBlockedEntry = {
  readonly candidateRef: string
  readonly issueNumber: number
  readonly blockedReasonRef: VirtualMergeQueueBlockedReasonRef
  readonly status: "blocked" | "needs-rebase"
  readonly detail: string
}

export type VirtualMergeQueueBlockedReasonRef =
  | "virtual_merge_queue.blocked.issue_closed"
  | "virtual_merge_queue.blocked.open_pr_exists"
  | "virtual_merge_queue.blocked.verification_not_passed"
  | "virtual_merge_queue.blocked.stale_base"
  | "virtual_merge_queue.blocked.invalid_commit"
  | "virtual_merge_queue.blocked.duplicate_issue"
  | "virtual_merge_queue.blocked.protected_path_deleted"
  | "virtual_merge_queue.blocked.mass_deletion_threshold"

export type VirtualMergeQueueProjection = {
  readonly actualHead: string
  readonly virtualHead: string
  readonly branchBaseForNextAssignment: string
  readonly ready: ReadonlyArray<VirtualMergeQueueReadyEntry>
  readonly blocked: ReadonlyArray<VirtualMergeQueueBlockedEntry>
  readonly nextActualPromotion: VirtualMergeQueueReadyEntry | null
}

export type VirtualMergeQueuePrFastForwardRequest = {
  readonly supervisorRef: string
  readonly repository: string
  readonly branch: string
  readonly actualHead: string
  readonly prNumber: number
  readonly prBaseCommit: string
  readonly prHeadCommit: string
  readonly promotionRef: string
}

export type VirtualMergeQueuePrFastForwardCommand =
  | {
      readonly kind: "git_fetch_pr_head"
      readonly args: readonly ["git", "fetch", "origin", string]
    }
  | {
      readonly kind: "git_verify_fetched_pr_head"
      readonly args: readonly ["git", "rev-parse", "--verify", "FETCH_HEAD^{commit}"]
      readonly expectedStdout: string
    }
  | {
      readonly kind: "git_checkout_branch"
      readonly args: readonly ["git", "checkout", string]
    }
  | {
      readonly kind: "git_reset_actual_head"
      readonly args: readonly ["git", "reset", "--hard", string]
    }
  | {
      readonly kind: "git_merge_ff_only"
      readonly args: readonly ["git", "merge", "--ff-only", string]
    }
  | {
      readonly kind: "git_push_branch"
      readonly args: readonly ["git", "push", "origin", string]
    }

export type VirtualMergeQueuePrFastForwardPlan = {
  readonly schema: "openagents.pylon.virtual_merge_queue.pr_fast_forward.v1"
  readonly state: "ready"
  readonly repository: string
  readonly branch: string
  readonly supervisorRef: string
  readonly prNumber: number
  readonly issueNumber: number
  readonly promotionRef: string
  readonly expectedActualHeadBefore: string
  readonly fastForwardHead: string
  readonly nextVirtualHeadAfter: string
  readonly commands: ReadonlyArray<VirtualMergeQueuePrFastForwardCommand>
  readonly sourceRefs: readonly [
    "issue.public.github.OpenAgentsInc.openagents.6695",
    "audit.public.docs.artanis.gitafter_cloudflare_artifacts_coordination.2026-06-28",
  ]
}

export type VirtualMergeQueuePrFastForwardBlockedReasonRef =
  | "virtual_merge_queue.pr_fast_forward.blocked.invalid_supervisor"
  | "virtual_merge_queue.pr_fast_forward.blocked.invalid_repository"
  | "virtual_merge_queue.pr_fast_forward.blocked.invalid_branch"
  | "virtual_merge_queue.pr_fast_forward.blocked.invalid_pr"
  | "virtual_merge_queue.pr_fast_forward.blocked.invalid_commit"
  | "virtual_merge_queue.pr_fast_forward.blocked.no_ready_promotion"
  | "virtual_merge_queue.pr_fast_forward.blocked.actual_head_mismatch"
  | "virtual_merge_queue.pr_fast_forward.blocked.promotion_ref_mismatch"
  | "virtual_merge_queue.pr_fast_forward.blocked.pr_base_mismatch"
  | "virtual_merge_queue.pr_fast_forward.blocked.pr_head_mismatch"

export type VirtualMergeQueuePrFastForwardBlocked = {
  readonly schema: "openagents.pylon.virtual_merge_queue.pr_fast_forward.v1"
  readonly state: "blocked"
  readonly blockedReasonRef: VirtualMergeQueuePrFastForwardBlockedReasonRef
  readonly detail: string
}

export type VirtualMergeQueuePrFastForwardResult =
  | VirtualMergeQueuePrFastForwardPlan
  | VirtualMergeQueuePrFastForwardBlocked

const gitCommitShaPattern = /^[a-f0-9]{40}$/i
const safePathPattern = /^(?!\/)(?!.*(?:^|\/)\.\.(?:\/|$))[A-Za-z0-9._/@+-]+(?:\/[A-Za-z0-9._/@+-]+)*$/
const repositoryPattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/
const gitBranchPattern =
  /^(?!-)(?!.*(?:^|\/)\.\.(?:\/|$))(?!.*\/\/)(?!.*@\{)(?!.*[~^:?*\[\]\\\s])(?!.*(?:^|\/)refs\/)(?!.*\.lock(?:\/|$))(?!.*\.$)[A-Za-z0-9._/@+-]+$/
const supervisorRefPattern = /^supervisor\.public\.pylon[_a-z0-9.-]*\.[A-Za-z0-9._:-]{6,160}$/
const signedOperatorOverridePattern = /^override\.signed\.operator\.[A-Za-z0-9._:-]{6,160}$/
const massDeletionThreshold = 5
const protectedDeletedRootPaths = new Set([
  ".githooks",
  "INVARIANTS.md",
  "PRODUCT.md",
  "package.json",
  "tsconfig.json",
])

function stableRef(prefix: string, value: string): string {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function isCommitSha(value: string): boolean {
  return gitCommitShaPattern.test(value)
}

function fastForwardBlocked(
  blockedReasonRef: VirtualMergeQueuePrFastForwardBlockedReasonRef,
  detail: string,
): VirtualMergeQueuePrFastForwardBlocked {
  return {
    schema: "openagents.pylon.virtual_merge_queue.pr_fast_forward.v1",
    state: "blocked",
    blockedReasonRef,
    detail,
  }
}

function hasSignedOperatorOverride(candidate: VirtualMergeQueueCandidate): boolean {
  return candidate.signedOperatorOverrideRef !== undefined &&
    signedOperatorOverridePattern.test(candidate.signedOperatorOverrideRef)
}

function deletedPaths(candidate: VirtualMergeQueueCandidate): ReadonlyArray<string> {
  return (candidate.changedFiles ?? [])
    .filter((change) => change.status === "deleted")
    .map((change) => change.path.trim())
    .filter((path) => path.length > 0)
}

function isProtectedDeletion(path: string): boolean {
  return path.startsWith("apps/") ||
    path.startsWith("clients/") ||
    protectedDeletedRootPaths.has(path) ||
    path.startsWith(".githooks/")
}

function candidateSortKey(candidate: VirtualMergeQueueCandidate): string {
  return `${candidate.queuedAt}\0${candidate.candidateRef}`
}

function block(
  candidate: VirtualMergeQueueCandidate,
  blockedReasonRef: VirtualMergeQueueBlockedReasonRef,
  detail: string,
): VirtualMergeQueueBlockedEntry {
  return {
    candidateRef: candidate.candidateRef,
    issueNumber: candidate.issueNumber,
    blockedReasonRef,
    status: blockedReasonRef === "virtual_merge_queue.blocked.stale_base" ? "needs-rebase" : "blocked",
    detail,
  }
}

export function projectVirtualMergeQueue(input: {
  readonly actualHead: string
  readonly candidates: ReadonlyArray<VirtualMergeQueueCandidate>
}): VirtualMergeQueueProjection {
  const ready: Array<VirtualMergeQueueReadyEntry> = []
  const blocked: Array<VirtualMergeQueueBlockedEntry> = []
  const seenIssues = new Set<number>()
  let virtualHead = input.actualHead

  if (!isCommitSha(input.actualHead)) {
    return {
      actualHead: input.actualHead,
      virtualHead: input.actualHead,
      branchBaseForNextAssignment: input.actualHead,
      ready,
      blocked: input.candidates.map((candidate) =>
        block(
          candidate,
          "virtual_merge_queue.blocked.invalid_commit",
          "actual head is not a pinned 40-character commit",
        ),
      ),
      nextActualPromotion: null,
    }
  }

  for (const candidate of [...input.candidates].sort((a, b) =>
    candidateSortKey(a).localeCompare(candidateSortKey(b)),
  )) {
    if (seenIssues.has(candidate.issueNumber)) {
      blocked.push(
        block(
          candidate,
          "virtual_merge_queue.blocked.duplicate_issue",
          `issue #${candidate.issueNumber} already has an earlier queue candidate`,
        ),
      )
      continue
    }
    seenIssues.add(candidate.issueNumber)

    if (!isCommitSha(candidate.baseCommit) || !isCommitSha(candidate.patchCommit)) {
      blocked.push(
        block(
          candidate,
          "virtual_merge_queue.blocked.invalid_commit",
          "candidate base or patch head is not a pinned 40-character commit",
        ),
      )
      continue
    }
    if (!candidate.issueOpen) {
      blocked.push(
        block(candidate, "virtual_merge_queue.blocked.issue_closed", `issue #${candidate.issueNumber} is not open`),
      )
      continue
    }
    if (candidate.hasOpenPullRequest) {
      blocked.push(
        block(
          candidate,
          "virtual_merge_queue.blocked.open_pr_exists",
          `issue #${candidate.issueNumber} already has an open pull request`,
        ),
      )
      continue
    }
    if (!candidate.verificationPassed) {
      blocked.push(
        block(
          candidate,
          "virtual_merge_queue.blocked.verification_not_passed",
          `issue #${candidate.issueNumber} has no passing verification result`,
        ),
      )
      continue
    }
    const override = hasSignedOperatorOverride(candidate)
    const deletions = deletedPaths(candidate)
    const unsafeDeletionPath = deletions.find((path) => !safePathPattern.test(path))
    const protectedDeletionPath = deletions.find(isProtectedDeletion)
    if (!override && candidate.baseCommit.toLowerCase() !== virtualHead.toLowerCase()) {
      blocked.push(
        block(
          candidate,
          "virtual_merge_queue.blocked.stale_base",
          `candidate base ${candidate.baseCommit} does not match virtual head ${virtualHead}`,
        ),
      )
      continue
    }
    if (!override && (unsafeDeletionPath !== undefined || protectedDeletionPath !== undefined)) {
      blocked.push(
        block(
          candidate,
          "virtual_merge_queue.blocked.protected_path_deleted",
          unsafeDeletionPath !== undefined
            ? `candidate deletes an unsafe path ${unsafeDeletionPath}`
            : `candidate deletes protected path ${protectedDeletionPath}`,
        ),
      )
      continue
    }
    if (!override && deletions.length > massDeletionThreshold) {
      blocked.push(
        block(
          candidate,
          "virtual_merge_queue.blocked.mass_deletion_threshold",
          `candidate deletes ${deletions.length} files; threshold is ${massDeletionThreshold}`,
        ),
      )
      continue
    }

    const promotionRef = stableRef(
      "promotion.public.pylon_virtual_merge_queue",
      `${candidate.issueNumber}:${candidate.baseCommit}:${candidate.patchCommit}`,
    )
    const waitsForActualHead =
      candidate.baseCommit.toLowerCase() === input.actualHead.toLowerCase() ? null : candidate.baseCommit
    ready.push({
      candidateRef: candidate.candidateRef,
      issueNumber: candidate.issueNumber,
      virtualBaseCommit: candidate.baseCommit,
      virtualHeadCommit: candidate.patchCommit,
      promotionRef,
      waitsForActualHead,
    })
    virtualHead = candidate.patchCommit
  }

  return {
    actualHead: input.actualHead,
    virtualHead,
    branchBaseForNextAssignment: virtualHead,
    ready,
    blocked,
    nextActualPromotion: ready.find((entry) => entry.waitsForActualHead === null) ?? null,
  }
}

export function planVirtualMergeQueuePrFastForward(input: {
  readonly projection: VirtualMergeQueueProjection
  readonly request: VirtualMergeQueuePrFastForwardRequest
}): VirtualMergeQueuePrFastForwardResult {
  const { projection, request } = input
  if (!supervisorRefPattern.test(request.supervisorRef)) {
    return fastForwardBlocked(
      "virtual_merge_queue.pr_fast_forward.blocked.invalid_supervisor",
      "supervisor ref must be a public Pylon supervisor ref",
    )
  }
  if (!repositoryPattern.test(request.repository)) {
    return fastForwardBlocked(
      "virtual_merge_queue.pr_fast_forward.blocked.invalid_repository",
      "repository must be an owner/name ref",
    )
  }
  if (!gitBranchPattern.test(request.branch)) {
    return fastForwardBlocked(
      "virtual_merge_queue.pr_fast_forward.blocked.invalid_branch",
      "branch must be a safe git branch name",
    )
  }
  if (!Number.isInteger(request.prNumber) || request.prNumber <= 0) {
    return fastForwardBlocked(
      "virtual_merge_queue.pr_fast_forward.blocked.invalid_pr",
      "pull request number must be a positive integer",
    )
  }
  if (
    !isCommitSha(request.actualHead) ||
    !isCommitSha(request.prBaseCommit) ||
    !isCommitSha(request.prHeadCommit)
  ) {
    return fastForwardBlocked(
      "virtual_merge_queue.pr_fast_forward.blocked.invalid_commit",
      "actual head, PR base, and PR head must be pinned 40-character commits",
    )
  }

  const next = projection.nextActualPromotion
  if (next === null) {
    return fastForwardBlocked(
      "virtual_merge_queue.pr_fast_forward.blocked.no_ready_promotion",
      "virtual merge queue has no PR eligible for actual promotion",
    )
  }
  if (request.actualHead.toLowerCase() !== projection.actualHead.toLowerCase()) {
    return fastForwardBlocked(
      "virtual_merge_queue.pr_fast_forward.blocked.actual_head_mismatch",
      `request actual head ${request.actualHead} does not match queue actual head ${projection.actualHead}`,
    )
  }
  if (request.promotionRef !== next.promotionRef) {
    return fastForwardBlocked(
      "virtual_merge_queue.pr_fast_forward.blocked.promotion_ref_mismatch",
      "request promotion ref is not the next queue promotion",
    )
  }
  if (request.prBaseCommit.toLowerCase() !== projection.actualHead.toLowerCase()) {
    return fastForwardBlocked(
      "virtual_merge_queue.pr_fast_forward.blocked.pr_base_mismatch",
      "PR base must equal the current actual head for a fast-forward merge",
    )
  }
  if (request.prHeadCommit.toLowerCase() !== next.virtualHeadCommit.toLowerCase()) {
    return fastForwardBlocked(
      "virtual_merge_queue.pr_fast_forward.blocked.pr_head_mismatch",
      "PR head must equal the next virtual queue head",
    )
  }

  return {
    schema: "openagents.pylon.virtual_merge_queue.pr_fast_forward.v1",
    state: "ready",
    repository: request.repository,
    branch: request.branch,
    supervisorRef: request.supervisorRef,
    prNumber: request.prNumber,
    issueNumber: next.issueNumber,
    promotionRef: next.promotionRef,
    expectedActualHeadBefore: projection.actualHead,
    fastForwardHead: request.prHeadCommit,
    nextVirtualHeadAfter: projection.virtualHead,
    commands: [
      {
        kind: "git_fetch_pr_head",
        args: ["git", "fetch", "origin", `pull/${request.prNumber}/head`] as const,
      },
      {
        kind: "git_verify_fetched_pr_head",
        args: ["git", "rev-parse", "--verify", "FETCH_HEAD^{commit}"] as const,
        expectedStdout: request.prHeadCommit,
      },
      {
        kind: "git_checkout_branch",
        args: ["git", "checkout", request.branch] as const,
      },
      {
        kind: "git_reset_actual_head",
        args: ["git", "reset", "--hard", projection.actualHead] as const,
      },
      {
        kind: "git_merge_ff_only",
        args: ["git", "merge", "--ff-only", request.prHeadCommit] as const,
      },
      {
        kind: "git_push_branch",
        args: ["git", "push", "origin", `HEAD:${request.branch}`] as const,
      },
    ],
    sourceRefs: [
      "issue.public.github.OpenAgentsInc.openagents.6695",
      "audit.public.docs.artanis.gitafter_cloudflare_artifacts_coordination.2026-06-28",
    ],
  }
}
