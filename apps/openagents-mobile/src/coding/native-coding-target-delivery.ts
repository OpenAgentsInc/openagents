import type {
  MobileCodingInput,
  MobileCodingTarget,
  MobileCodingTargetResolution,
} from "./mobile-coding-navigation"

export const MAX_PENDING_NATIVE_CODING_TARGETS = 16

export type NativeCodingTargetDelivery = Readonly<{
  enqueue: (input: MobileCodingInput) => void
  flush: () => Promise<void>
  close: () => void
  pendingCount: () => number
}>

/**
 * Serializes native URL/notification delivery around live catalog authority.
 * Authority-unavailable inputs remain bounded and retry after reconnect; every
 * other rejection is terminal and never reaches navigation.
 */
export const openNativeCodingTargetDelivery = (input: Readonly<{
  resolve: (candidate: MobileCodingInput) => Promise<MobileCodingTargetResolution>
  activate: (
    target: MobileCodingTarget,
    source: MobileCodingInput["source"],
  ) => Promise<boolean>
  rejected?: (resolution: Extract<MobileCodingTargetResolution, { state: "rejected" }>) => void
}>): NativeCodingTargetDelivery => {
  let closed = false
  let pending: MobileCodingInput[] = []
  let activeFlush: Promise<void> | null = null

  const run = async (): Promise<void> => {
    while (!closed && pending.length > 0) {
      const candidate = pending[0]!
      const resolution = await input.resolve(candidate)
      if (closed) return
      if (resolution.state === "rejected") {
        if (resolution.reason === "authority_unavailable") return
        pending = pending.slice(1)
        input.rejected?.(resolution)
        continue
      }
      if (!await input.activate(resolution.target, candidate.source)) return
      pending = pending.slice(1)
    }
  }

  return {
    enqueue: candidate => {
      if (closed) return
      pending = [...pending, candidate].slice(-MAX_PENDING_NATIVE_CODING_TARGETS)
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
