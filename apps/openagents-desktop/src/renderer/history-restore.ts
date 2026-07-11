import type { CodexHistoryCatalog } from "../codex-history-contract.ts"

/**
 * Restore a root or descendant only when its canonical root is inside the
 * eagerly mounted catalog window. A child ref is never itself expected in the
 * root-only sidebar catalog.
 */
export const restorableHistoryThreadRef = (
  catalog: CodexHistoryCatalog,
  selectedThreadRef: string | null | undefined,
  visibleRootCount: number,
): string | null => {
  if (selectedThreadRef === null || selectedThreadRef === undefined) return null
  const byRef = new Map(catalog.agents.map((agent) => [agent.threadRef, agent] as const))
  let current = byRef.get(selectedThreadRef)
  if (current === undefined) return null
  const seen = new Set<string>()
  while (current.parentThreadRef !== null) {
    if (seen.has(current.threadRef)) return null
    seen.add(current.threadRef)
    const parent = byRef.get(current.parentThreadRef)
    if (parent === undefined) return null
    current = parent
  }
  const rootIndex = catalog.roots.findIndex((root) => root.threadRef === current.threadRef)
  return rootIndex >= 0 && rootIndex < visibleRootCount ? selectedThreadRef : null
}
