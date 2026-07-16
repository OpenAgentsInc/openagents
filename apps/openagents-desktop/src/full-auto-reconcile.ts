import type { FullAutoRegistry } from "./full-auto-registry.ts"

/**
 * Full Auto (#8853): the single decision function called from two trigger
 * points in main.ts -- right after any Full-Auto-flagged turn completes, and
 * once at app startup after existing turn-recovery settles. Both call sites
 * share this exact logic so "should the next turn start" is decided in
 * exactly one durable place, not duplicated between a live-completion path
 * and a separate restart path.
 */
export const FULL_AUTO_CONTINUE_MESSAGE =
  "Continue Full Auto: look at this repository (README, docs, open issues) and do the next concrete useful thing."
export const FULL_AUTO_MAX_CONTINUATIONS = 20

export type FullAutoDispatchResult = Readonly<{ ok: boolean }>
export type FullAutoDispatch = (input: Readonly<{ threadRef: string; message: string }>) => Promise<FullAutoDispatchResult>

export const reconcileFullAutoThreads = async (input: Readonly<{
  registry: FullAutoRegistry
  /** Thread refs with a nonterminal (in-flight or awaiting-recovery) turn right now. */
  nonterminalThreadRefs: () => ReadonlySet<string>
  dispatch: FullAutoDispatch
  onCapReached?: (threadRef: string) => void
  onDispatchFailed?: (threadRef: string, error: unknown) => void
}>): Promise<ReadonlyArray<string>> => {
  const dispatched: string[] = []
  const inFlight = input.nonterminalThreadRefs()
  for (const threadRef of input.registry.enabledThreads()) {
    if (inFlight.has(threadRef)) continue
    const count = input.registry.incrementContinuation(threadRef)
    if (count > FULL_AUTO_MAX_CONTINUATIONS) {
      input.registry.set(threadRef, false)
      input.onCapReached?.(threadRef)
      continue
    }
    try {
      const result = await input.dispatch({ threadRef, message: FULL_AUTO_CONTINUE_MESSAGE })
      if (result.ok) dispatched.push(threadRef)
    } catch (error) {
      input.onDispatchFailed?.(threadRef, error)
    }
  }
  return dispatched
}
