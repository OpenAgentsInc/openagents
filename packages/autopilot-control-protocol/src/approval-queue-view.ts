type RawRecord = Record<string, unknown>

type ApprovalQueueRow = {
  ref: string
  kind: string
  prompt: string
}

type ApprovalQueueView = {
  pending: ApprovalQueueRow[]
  count: number
}

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function readString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined
}

function readPendingList(raw: unknown): unknown[] {
  if (Array.isArray(raw)) return raw
  if (!isRecord(raw)) return []

  const pending = raw.pending
  if (Array.isArray(pending)) return pending

  return []
}

function projectPendingApproval(row: RawRecord): ApprovalQueueRow | undefined {
  const ref = readString(row.ref)
  const kind = readString(row.kind)
  const prompt = readString(row.prompt)

  if (ref === undefined || kind === undefined || prompt === undefined) {
    return undefined
  }

  return { ref, kind, prompt }
}

export function projectApprovalQueue(raw: unknown): ApprovalQueueView {
  const pending: ApprovalQueueRow[] = []

  for (const row of readPendingList(raw)) {
    if (!isRecord(row)) continue

    const projected = projectPendingApproval(row)
    if (projected === undefined) continue

    pending.push(projected)
  }

  return {
    pending,
    count: pending.length,
  }
}
