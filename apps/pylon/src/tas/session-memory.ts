export type MemoryRecord = {
  readonly ref: string
  readonly kind: string
  readonly createdAt: number
  readonly lastUsedAt: number
  readonly salience: number
}

export type RecallOptions = {
  readonly nowMs: number
}

const recencyScore = (record: MemoryRecord, nowMs: number): number => {
  const ageMs = Math.max(0, nowMs - record.lastUsedAt)

  return 1 / (1 + ageMs)
}

const recallScore = (record: MemoryRecord, nowMs: number): number =>
  record.salience + recencyScore(record, nowMs)

export function recallOrder(
  records: readonly MemoryRecord[],
  options: RecallOptions,
): readonly MemoryRecord[] {
  return records
    .map((record, index) => ({
      index,
      record,
      score: recallScore(record, options.nowMs),
    }))
    .sort((left, right) => {
      const scoreDelta = right.score - left.score

      if (scoreDelta !== 0) {
        return scoreDelta
      }

      return left.index - right.index
    })
    .map(({ record }) => record)
}

export function addOrUpdate(
  records: readonly MemoryRecord[],
  record: MemoryRecord,
): readonly MemoryRecord[] {
  const existingIndex = records.findIndex((candidate) => candidate.ref === record.ref)

  if (existingIndex === -1) {
    return [...records, record]
  }

  return records.map((candidate, index) =>
    index === existingIndex
      ? {
          ...candidate,
          ...record,
          lastUsedAt: record.lastUsedAt,
        }
      : candidate,
  )
}
