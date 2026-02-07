import { Hydration, Registry } from "@effect-atom/atom"

import type { Registry as AtomRegistry } from "@effect-atom/atom/Registry"

export const makeAtomRegistry = (): AtomRegistry =>
  Registry.make({
    // Batch atom recomputations into microtasks (React-less runtime).
    scheduleTask: (f) => queueMicrotask(f),
  })

/**
 * v1 SSR dehydrate payload format (see `src/effuse-host/ssr.ts`):
 * - JSON object: `{ [routeId]: { atomState?: DehydratedAtom[] } }`
 */
export const hydrateAtomRegistryFromDocument = (registry: AtomRegistry): void => {
  if (typeof document === "undefined") return

  const node = document.getElementById("effuse-dehydrate")
  if (!node) return

  const text = node.textContent ?? ""
  if (!text.trim()) return

  try {
    const parsed: any = JSON.parse(text)
    if (!parsed || typeof parsed !== "object") return

    for (const fragment of Object.values(parsed)) {
      if (!fragment || typeof fragment !== "object") continue
      const atomState = (fragment as any).atomState
      if (!Array.isArray(atomState)) continue
      Hydration.hydrate(registry, atomState as ReadonlyArray<Hydration.DehydratedAtom>)
    }
  } catch (err) {
    console.warn("[EffuseApp] Failed to parse SSR dehydrate payload", err)
  }
}

