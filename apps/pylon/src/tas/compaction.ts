export type CompactionDecisionRequest = {
  readonly usedTokens: number
  readonly maxTokens: number
  readonly keepTailCount: number
}

export type CompactionDecision =
  | {
      readonly action: "keep"
      readonly reason: string
    }
  | {
      readonly action: "compact"
      readonly reason: string
    }

export type CompactionSummaryRecordRequest = {
  readonly replacedRefs: readonly string[]
  readonly summaryRef: string
}

export type CompactionSummaryRecord = {
  readonly kind: "compaction_summary_record"
  readonly summaryRef: string
  readonly replacedRefs: readonly string[]
}

export const COMPACTION_THRESHOLD_FRACTION = 0.8

export function decideCompaction(
  request: CompactionDecisionRequest,
): CompactionDecision {
  const thresholdTokens = request.maxTokens * COMPACTION_THRESHOLD_FRACTION

  if (request.usedTokens > thresholdTokens) {
    return {
      action: "compact",
      reason: "token_threshold_exceeded",
    }
  }

  return {
    action: "keep",
    reason: "under_token_threshold",
  }
}

export function buildCompactionSummaryRecord(
  request: CompactionSummaryRecordRequest,
): CompactionSummaryRecord {
  return {
    kind: "compaction_summary_record",
    summaryRef: request.summaryRef,
    replacedRefs: [...request.replacedRefs],
  }
}
