export type ContextAssemblyItem<Ref = string> = {
  readonly ref: Ref
  readonly priority: number
  readonly tokens: number
  readonly pinned?: boolean
}

export type ContextAssemblyResult<Ref = string> = {
  readonly included: Ref[]
  readonly droppedRefs: Ref[]
  readonly usedTokens: number
}

export function assembleContext<Ref = string>(
  items: ReadonlyArray<ContextAssemblyItem<Ref>>,
  budgetTokens: number,
): ContextAssemblyResult<Ref> {
  const pinnedItems = items.filter((item) => item.pinned === true)
  const candidates = items
    .filter((item) => item.pinned !== true)
    .sort(compareByPriorityThenRef)

  const includedItems: Array<ContextAssemblyItem<Ref>> = [...pinnedItems]
  const droppedRefs: Ref[] = []
  let usedTokens = sumTokens(pinnedItems)

  for (const item of candidates) {
    if (usedTokens + item.tokens <= budgetTokens) {
      includedItems.push(item)
      usedTokens += item.tokens
    } else {
      droppedRefs.push(item.ref)
    }
  }

  return {
    included: includedItems.map((item) => item.ref),
    droppedRefs,
    usedTokens,
  }
}

function compareByPriorityThenRef<Ref>(
  left: ContextAssemblyItem<Ref>,
  right: ContextAssemblyItem<Ref>,
): number {
  const priorityOrder = right.priority - left.priority

  if (priorityOrder !== 0) {
    return priorityOrder
  }

  return String(left.ref).localeCompare(String(right.ref))
}

function sumTokens<Ref>(
  items: ReadonlyArray<ContextAssemblyItem<Ref>>,
): number {
  return items.reduce((total, item) => total + item.tokens, 0)
}
