import { sortSessions, type SessionSortRow } from "./session-sort.js"

export type SessionGroupRow = {
  sessionRef: string
  state: string
  parentRef?: string | null
  updatedAt: string
}

export type SessionGroupNode<T extends SessionGroupRow = SessionGroupRow> = {
  parent: T
  children: SessionGroupNode<T>[]
}

export function groupSessionsByParent(
  rows: SessionGroupRow[],
): { tree: { parent: any, children: any[] }[], topCount: number, totalCount: number } {
  const sortedRows = sortSessions(rows satisfies SessionSortRow[])
  const nodeByRef = new Map<string, SessionGroupNode>()

  for (const row of sortedRows) {
    nodeByRef.set(row.sessionRef, {
      parent: row,
      children: [],
    })
  }

  const childRefs = new Set<string>()

  for (const row of sortedRows) {
    const parentRef = row.parentRef

    if (
      parentRef !== undefined &&
      parentRef !== null &&
      parentRef !== row.sessionRef &&
      nodeByRef.has(parentRef)
    ) {
      const parent = nodeByRef.get(parentRef)
      const child = nodeByRef.get(row.sessionRef)

      if (parent !== undefined && child !== undefined) {
        parent.children.push(child)
        childRefs.add(row.sessionRef)
      }
    }
  }

  const tree = sortedRows
    .filter((row) => !childRefs.has(row.sessionRef))
    .map((row) => nodeByRef.get(row.sessionRef))
    .filter((node): node is SessionGroupNode => node !== undefined)

  return {
    tree,
    topCount: tree.length,
    totalCount: sortedRows.length,
  }
}
