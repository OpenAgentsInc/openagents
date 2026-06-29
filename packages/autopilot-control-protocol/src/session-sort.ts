export type SessionSortRow = {
  sessionRef: string
  state: string
  updatedAt: string
  parentRef?: string | null
}

type IndexedSession<T extends SessionSortRow> = {
  row: T
  index: number
}

function compareRootSessions<T extends SessionSortRow>(
  a: IndexedSession<T>,
  b: IndexedSession<T>,
): number {
  const aRunning = a.row.state === "running"
  const bRunning = b.row.state === "running"

  if (aRunning !== bRunning) return aRunning ? -1 : 1
  if (a.row.updatedAt < b.row.updatedAt) return 1
  if (a.row.updatedAt > b.row.updatedAt) return -1
  return a.index - b.index
}

export function sortSessions<T extends SessionSortRow>(rows: T[]): T[] {
  const indexed = rows.map((row, index) => ({ row, index }))
  const byRef = new Map<string, IndexedSession<T>>()

  for (const item of indexed) {
    byRef.set(item.row.sessionRef, item)
  }

  const roots: IndexedSession<T>[] = []
  const childrenByParent = new Map<string, IndexedSession<T>[]>()

  for (const item of indexed) {
    const parentRef = item.row.parentRef

    if (
      parentRef !== undefined &&
      parentRef !== null &&
      parentRef !== item.row.sessionRef &&
      byRef.has(parentRef)
    ) {
      const children = childrenByParent.get(parentRef) ?? []
      children.push(item)
      childrenByParent.set(parentRef, children)
    } else {
      roots.push(item)
    }
  }

  const sorted: T[] = []
  const seen = new Set<string>()

  function appendTree(item: IndexedSession<T>): void {
    if (seen.has(item.row.sessionRef)) return
    seen.add(item.row.sessionRef)
    sorted.push(item.row)

    for (const child of childrenByParent.get(item.row.sessionRef) ?? []) {
      appendTree(child)
    }
  }

  for (const item of roots.sort(compareRootSessions)) {
    appendTree(item)
  }

  return sorted
}
