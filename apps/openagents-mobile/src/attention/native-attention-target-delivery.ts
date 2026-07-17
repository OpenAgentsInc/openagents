import type { ConfirmedRuntimeAttentionSnapshot } from "@openagentsinc/khala-sync-client"

import {
  resolveMobileAttentionTarget,
  type MobileAttentionCandidate,
  type MobileAttentionResolution,
  type MobileAttentionTarget,
} from "./mobile-attention-target"

export const MAX_PENDING_NATIVE_ATTENTION_TARGETS = 16

export type NativeAttentionTargetDelivery = Readonly<{
  enqueue: (candidate: MobileAttentionCandidate) => void
  flush: () => Promise<void>
  close: () => void
  pendingCount: () => number
}>

/**
 * Holds native return candidates until a confirmed personal attention
 * projection is live. Every accepted candidate is handed to the same Effect
 * Native controller intent used by an in-app attention row.
 */
export const openNativeAttentionTargetDelivery = (input: Readonly<{
  snapshot: () => ConfirmedRuntimeAttentionSnapshot | null
  deliver: (target: MobileAttentionTarget) => Promise<boolean> | boolean
  rejected?: (resolution: Extract<MobileAttentionResolution, { state: "rejected" }>) => void
}>): NativeAttentionTargetDelivery => {
  let closed = false
  let pending: MobileAttentionCandidate[] = []
  let activeFlush: Promise<void> | null = null

  const run = async (): Promise<void> => {
    while (!closed && pending.length > 0) {
      const snapshot = input.snapshot()
      if (snapshot === null) return
      const resolution = resolveMobileAttentionTarget(snapshot, pending[0]!)
      if (resolution.state === "rejected") {
        if (resolution.reason === "authority_unavailable") return
        pending = pending.slice(1)
        input.rejected?.(resolution)
        continue
      }
      if (!await input.deliver(resolution.target)) return
      pending = pending.slice(1)
    }
  }

  return {
    enqueue: candidate => {
      if (closed) return
      pending = [...pending, candidate].slice(-MAX_PENDING_NATIVE_ATTENTION_TARGETS)
    },
    flush: () => {
      if (closed) return Promise.resolve()
      if (activeFlush !== null) return activeFlush
      activeFlush = run().finally(() => { activeFlush = null })
      return activeFlush
    },
    close: () => {
      closed = true
      pending = []
    },
    pendingCount: () => pending.length,
  }
}
