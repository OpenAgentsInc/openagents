export type UnifiedSessionOrigin = "local" | "bridge" | "cloud" | "external"

export type UnifiedSessionRow = {
  sessionRef: string
  origin: UnifiedSessionOrigin
  state: string
  latestActivity: string
  parentRef: string | null
  updatedAt: string
}

export type MergeSessionViewsInput = {
  local?: any[]
  bridge?: any[]
  cloud?: any[]
  external?: any[]
}

type RawRecord = Record<string, unknown>

type OrderedSessionRow = UnifiedSessionRow & {
  originalIndex: number
}

const ORIGINS = ["local", "bridge", "cloud", "external"] as const

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function readString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined
}

function projectSession(
  origin: UnifiedSessionOrigin,
  value: unknown,
  originalIndex: number,
): OrderedSessionRow | undefined {
  if (!isRecord(value)) return undefined

  const sessionRef = readString(value.sessionRef)
  if (sessionRef === undefined || sessionRef.length === 0) return undefined

  return {
    sessionRef,
    origin,
    state: readString(value.state) ?? "unknown",
    latestActivity: readString(value.latestActivity) ?? "",
    parentRef: readString(value.parentRef) ?? null,
    updatedAt: readString(value.updatedAt) ?? "",
    originalIndex,
  }
}

function orderParentsBeforeChildren(rows: OrderedSessionRow[]): OrderedSessionRow[] {
  const bySessionRef = new Map(rows.map((row) => [row.sessionRef, row]))
  const emitted = new Set<string>()
  const visiting = new Set<string>()
  const ordered: OrderedSessionRow[] = []

  const emit = (row: OrderedSessionRow) => {
    if (emitted.has(row.sessionRef)) return
    if (visiting.has(row.sessionRef)) {
      ordered.push(row)
      emitted.add(row.sessionRef)
      return
    }

    visiting.add(row.sessionRef)

    if (row.parentRef !== null) {
      const parent = bySessionRef.get(row.parentRef)
      if (parent !== undefined) emit(parent)
    }

    visiting.delete(row.sessionRef)

    if (!emitted.has(row.sessionRef)) {
      ordered.push(row)
      emitted.add(row.sessionRef)
    }
  }

  for (const row of rows) emit(row)

  return ordered
}

export function mergeSessionViews(input: MergeSessionViewsInput): UnifiedSessionRow[] {
  const ordered: OrderedSessionRow[] = []

  for (const origin of ORIGINS) {
    const sessions = input[origin]
    if (!Array.isArray(sessions)) continue

    for (const session of sessions) {
      const row = projectSession(origin, session, ordered.length)
      if (row !== undefined) ordered.push(row)
    }
  }

  const bySessionRef = new Map<string, OrderedSessionRow>()
  for (const row of ordered) {
    if (!bySessionRef.has(row.sessionRef)) bySessionRef.set(row.sessionRef, row)
  }

  const sortedRows = [...bySessionRef.values()].sort(
    (a, b) => b.updatedAt.localeCompare(a.updatedAt) || a.originalIndex - b.originalIndex,
  )

  return orderParentsBeforeChildren(sortedRows).map(({ originalIndex: _originalIndex, ...row }) => row)
}
