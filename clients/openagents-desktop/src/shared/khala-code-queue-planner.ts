export type KhalaCodeQueueCandidateKind = "issue" | "pull_request"

export type KhalaCodeQueueCandidateState = "closed" | "open"

export type KhalaCodeQueueLane = {
  readonly description?: string
  readonly laneRef: string
  readonly priority: number
  readonly title: string
}

export type KhalaCodeQueueCandidateDispatch =
  | {
      readonly state: "ready"
    }
  | {
      readonly reason: KhalaCodeRetryReason
      readonly retryAt: string
      readonly state: "retry"
    }
  | {
      readonly reason: KhalaCodeBlockReason
      readonly state: "blocked"
    }

export type KhalaCodeQueueCandidate = {
  readonly closedAt?: string | null
  readonly dispatch?: KhalaCodeQueueCandidateDispatch
  readonly kind: KhalaCodeQueueCandidateKind
  readonly labels?: readonly string[]
  readonly laneRefs: readonly string[]
  readonly mergedAt?: string | null
  readonly number: number
  readonly priority: number
  readonly repository: string
  readonly state: KhalaCodeQueueCandidateState
  readonly supersededBy?: string | null
  readonly title: string
  readonly updatedAt?: string | null
  readonly url?: string | null
}

export type KhalaCodeActiveClaim = {
  readonly accountRef?: string | null
  readonly assignmentRef?: string | null
  readonly candidateRef: string
  readonly claimedAt: string
  readonly claimRef: string
  readonly laneRef: string
}

export type KhalaCodeSkipReason =
  | "duplicate_candidate"
  | "duplicate_claim"
  | "lane_mismatch"
  | "superseded"

export type KhalaCodeClaimReason =
  | "priority_lane_capacity"
  | "priority_lane_dry_run"

export type KhalaCodeRetryReason =
  | "capacity_unavailable"
  | "failed_before_accept"
  | "provider_cooldown"
  | "verification_failed"

export type KhalaCodeBlockReason =
  | "account_not_ready"
  | "missing_verification"
  | "owner_action_required"
  | "unsafe_candidate"

export type KhalaCodeMergeReason = "merged_upstream"

export type KhalaCodeCloseReason =
  | "closed_unmerged"
  | "issue_closed"
  | "pull_request_closed"

export type KhalaCodeQueueReason =
  | {
      readonly kind: "skip"
      readonly reason: KhalaCodeSkipReason
    }
  | {
      readonly kind: "claim"
      readonly reason: KhalaCodeClaimReason
    }
  | {
      readonly kind: "retry"
      readonly reason: KhalaCodeRetryReason
      readonly retryAt: string
    }
  | {
      readonly kind: "block"
      readonly reason: KhalaCodeBlockReason
    }
  | {
      readonly kind: "merge"
      readonly reason: KhalaCodeMergeReason
    }
  | {
      readonly kind: "close"
      readonly reason: KhalaCodeCloseReason
    }

export type KhalaCodeQueueDecision =
  | "block"
  | "claim"
  | "close"
  | "merge"
  | "retry"
  | "skip"

export type KhalaCodeQueueRow = {
  readonly candidate: KhalaCodeQueueCandidate
  readonly candidateRef: string
  readonly claim?: KhalaCodePlannedClaim
  readonly decision: KhalaCodeQueueDecision
  readonly laneRef: string | null
  readonly reason: KhalaCodeQueueReason
}

export type KhalaCodePlannedClaim = {
  readonly candidateRef: string
  readonly claimRef: string
  readonly laneRef: string
  readonly plannedAt: string
}

export type PlanKhalaCodeQueueInput = {
  readonly activeClaims?: readonly KhalaCodeActiveClaim[]
  readonly candidates: readonly KhalaCodeQueueCandidate[]
  readonly lanes: readonly KhalaCodeQueueLane[]
  readonly maxClaims?: number
  readonly now: string
  readonly targetLaneRef: string
}

export type KhalaCodeQueuePlan = {
  readonly claims: readonly KhalaCodePlannedClaim[]
  readonly lane: KhalaCodeQueueLane | null
  readonly rows: readonly KhalaCodeQueueRow[]
  readonly targetLaneRef: string
}

export const khalaCodeCandidateRef = (
  candidate: Pick<KhalaCodeQueueCandidate, "kind" | "number" | "repository">,
): string => {
  const prefix = candidate.kind === "pull_request" ? "pr" : "issue"
  return `${candidate.repository}#${prefix}.${candidate.number}`
}

const claimRefFor = (
  candidateRef: string,
  laneRef: string,
  plannedAt: string,
): string =>
  `khala_code_claim.${laneRef}.${candidateRef.replace(/[^a-z0-9._-]+/gi, "_")}.${plannedAt.replace(/[^0-9TZ]+/g, "")}`

const closedReasonFor = (
  candidate: KhalaCodeQueueCandidate,
): KhalaCodeQueueReason => {
  if (candidate.kind === "pull_request" && candidate.mergedAt !== null && candidate.mergedAt !== undefined) {
    return { kind: "merge", reason: "merged_upstream" }
  }
  if (candidate.kind === "issue") return { kind: "close", reason: "issue_closed" }
  return { kind: "close", reason: "closed_unmerged" }
}

const lanePriority = (lanes: readonly KhalaCodeQueueLane[], laneRef: string): number =>
  lanes.find(lane => lane.laneRef === laneRef)?.priority ?? 0

const sortedCandidates = (
  candidates: readonly KhalaCodeQueueCandidate[],
  lanes: readonly KhalaCodeQueueLane[],
  targetLaneRef: string,
): readonly KhalaCodeQueueCandidate[] =>
  [...candidates].sort((left, right) => {
    const targetDelta =
      Number(right.laneRefs.includes(targetLaneRef)) -
      Number(left.laneRefs.includes(targetLaneRef))
    if (targetDelta !== 0) return targetDelta

    const laneDelta =
      Math.max(0, ...right.laneRefs.map(ref => lanePriority(lanes, ref))) -
      Math.max(0, ...left.laneRefs.map(ref => lanePriority(lanes, ref)))
    if (laneDelta !== 0) return laneDelta

    const priorityDelta = right.priority - left.priority
    if (priorityDelta !== 0) return priorityDelta

    const updatedDelta =
      Date.parse(right.updatedAt ?? "1970-01-01T00:00:00.000Z") -
      Date.parse(left.updatedAt ?? "1970-01-01T00:00:00.000Z")
    if (updatedDelta !== 0) return updatedDelta

    return left.number - right.number
  })

export const planKhalaCodeQueue = (
  input: PlanKhalaCodeQueueInput,
): KhalaCodeQueuePlan => {
  const maxClaims = Math.max(0, input.maxClaims ?? 1)
  const targetLane =
    input.lanes.find(lane => lane.laneRef === input.targetLaneRef) ?? null
  const activeClaimRefs = new Set(
    (input.activeClaims ?? []).map(claim => claim.candidateRef),
  )
  const seenCandidateRefs = new Set<string>()
  const rows: KhalaCodeQueueRow[] = []
  const claims: KhalaCodePlannedClaim[] = []

  for (const candidate of sortedCandidates(
    input.candidates,
    input.lanes,
    input.targetLaneRef,
  )) {
    const candidateRef = khalaCodeCandidateRef(candidate)
    const laneRef = candidate.laneRefs.includes(input.targetLaneRef)
      ? input.targetLaneRef
      : candidate.laneRefs[0] ?? null

    if (seenCandidateRefs.has(candidateRef)) {
      rows.push({
        candidate,
        candidateRef,
        decision: "skip",
        laneRef,
        reason: { kind: "skip", reason: "duplicate_candidate" },
      })
      continue
    }
    seenCandidateRefs.add(candidateRef)

    if (!candidate.laneRefs.includes(input.targetLaneRef)) {
      rows.push({
        candidate,
        candidateRef,
        decision: "skip",
        laneRef,
        reason: { kind: "skip", reason: "lane_mismatch" },
      })
      continue
    }

    if (candidate.state === "closed") {
      const reason = closedReasonFor(candidate)
      rows.push({
        candidate,
        candidateRef,
        decision: reason.kind,
        laneRef,
        reason,
      })
      continue
    }

    if (candidate.supersededBy !== null && candidate.supersededBy !== undefined) {
      rows.push({
        candidate,
        candidateRef,
        decision: "skip",
        laneRef,
        reason: { kind: "skip", reason: "superseded" },
      })
      continue
    }

    if (activeClaimRefs.has(candidateRef)) {
      rows.push({
        candidate,
        candidateRef,
        decision: "skip",
        laneRef,
        reason: { kind: "skip", reason: "duplicate_claim" },
      })
      continue
    }

    if (candidate.dispatch?.state === "blocked") {
      rows.push({
        candidate,
        candidateRef,
        decision: "block",
        laneRef,
        reason: { kind: "block", reason: candidate.dispatch.reason },
      })
      continue
    }

    if (candidate.dispatch?.state === "retry") {
      rows.push({
        candidate,
        candidateRef,
        decision: "retry",
        laneRef,
        reason: {
          kind: "retry",
          reason: candidate.dispatch.reason,
          retryAt: candidate.dispatch.retryAt,
        },
      })
      continue
    }

    if (claims.length >= maxClaims) {
      rows.push({
        candidate,
        candidateRef,
        decision: "retry",
        laneRef,
        reason: {
          kind: "retry",
          reason: "capacity_unavailable",
          retryAt: input.now,
        },
      })
      continue
    }

    const claim = {
      candidateRef,
      claimRef: claimRefFor(candidateRef, input.targetLaneRef, input.now),
      laneRef: input.targetLaneRef,
      plannedAt: input.now,
    } satisfies KhalaCodePlannedClaim
    claims.push(claim)
    activeClaimRefs.add(candidateRef)
    rows.push({
      candidate,
      candidateRef,
      claim,
      decision: "claim",
      laneRef,
      reason: {
        kind: "claim",
        reason: maxClaims === 0 ? "priority_lane_dry_run" : "priority_lane_capacity",
      },
    })
  }

  return {
    claims,
    lane: targetLane,
    rows,
    targetLaneRef: input.targetLaneRef,
  }
}
