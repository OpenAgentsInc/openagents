export type RepoMemoryKind = "convention" | "layout" | "command" | "note"

export type RepoMemoryRecord = {
  readonly repoRef: string
  readonly factRef: string
  readonly kind: RepoMemoryKind
  readonly confidence: number
  readonly observedAt: number
}

export type RepoMemoryRecallOptions = {
  readonly nowMs: number
}

const recencyAge = (record: RepoMemoryRecord, nowMs: number): number =>
  Math.max(0, nowMs - record.observedAt)

export function recallForRepo(
  records: readonly RepoMemoryRecord[],
  repoRef: string,
  options: RepoMemoryRecallOptions,
): readonly RepoMemoryRecord[] {
  return records
    .map((record, index) => ({ index, record }))
    .filter(({ record }) => record.repoRef === repoRef)
    .sort((left, right) => {
      const confidenceDelta = right.record.confidence - left.record.confidence

      if (confidenceDelta !== 0) {
        return confidenceDelta
      }

      const recencyDelta =
        recencyAge(left.record, options.nowMs) - recencyAge(right.record, options.nowMs)

      if (recencyDelta !== 0) {
        return recencyDelta
      }

      return left.index - right.index
    })
    .map(({ record }) => record)
}

export function mergeObservation(
  records: readonly RepoMemoryRecord[],
  observation: RepoMemoryRecord,
): readonly RepoMemoryRecord[] {
  const existingIndex = records.findIndex(
    (record) => record.factRef === observation.factRef,
  )

  if (existingIndex === -1) {
    return [...records, observation]
  }

  const existing = records[existingIndex]

  if (existing === undefined || existing.confidence >= observation.confidence) {
    return records
  }

  return records.map((record, index) => (index === existingIndex ? observation : record))
}
