type SessionFilterQuery = {
  text?: string
  state?: string
  agentKind?: string
  origin?: string
}

type RowRecord = Record<string, unknown>

function isRecord(value: unknown): value is RowRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function readString(value: unknown): string {
  return typeof value === "string" ? value : ""
}

function readFilter(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined

  const trimmed = value.trim()
  return trimmed.length > 0 ? trimmed : undefined
}

function updatedAtValue(row: unknown): string {
  if (!isRecord(row)) return ""
  return readString(row.updatedAt)
}

export function sortByUpdatedAtDesc(rows: any[]): any[] {
  if (!Array.isArray(rows)) return []

  return rows
    .map((row, index) => ({ row, index }))
    .sort((a, b) => {
      const aUpdatedAt = updatedAtValue(a.row)
      const bUpdatedAt = updatedAtValue(b.row)

      if (aUpdatedAt < bUpdatedAt) return 1
      if (aUpdatedAt > bUpdatedAt) return -1
      return a.index - b.index
    })
    .map(({ row }) => row)
}

export function filterSessions(rows: any[], q: SessionFilterQuery): any[] {
  if (!Array.isArray(rows)) return []

  const text = readFilter(q?.text)?.toLowerCase()
  const state = readFilter(q?.state)
  const agentKind = readFilter(q?.agentKind)
  const origin = readFilter(q?.origin)

  return rows.filter((row) => {
    if (!isRecord(row)) return false

    if (text !== undefined) {
      const haystack = `${readString(row.sessionRef)} ${readString(row.latestActivity)}`.toLowerCase()
      if (!haystack.includes(text)) return false
    }

    if (state !== undefined && readString(row.state) !== state) return false
    if (agentKind !== undefined && readString(row.agentKind) !== agentKind) return false
    if (origin !== undefined && readString(row.origin) !== origin) return false

    return true
  })
}
