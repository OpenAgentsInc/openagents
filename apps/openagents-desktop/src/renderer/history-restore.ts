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

/**
 * Where the initial window opens (EP250 bottom-anchored flow, #8675 restore
 * contract): restoring a saved ITEM selection loads the page window AROUND
 * that item (its saved containing-page offset; bidirectional fill takes over
 * from there); otherwise the conversation opens at its END with the newest
 * items visible.
 */
export const historyRestoreFetchPlan = (
  restored: Readonly<{ offset: number; selectedItemRef: string | null }> | null,
  totalItems: number,
  limit: number,
): Readonly<{ offset: number; anchor: "item" | "end" }> =>
  restored !== null && restored.selectedItemRef !== null
    ? { offset: Math.min(Math.max(0, restored.offset), Math.max(0, totalItems - 1)), anchor: "item" }
    : { offset: Math.max(0, totalItems - limit), anchor: "end" }
