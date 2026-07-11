import {
  openKhalaConversationLive,
  type KhalaConversationLiveMetrics,
  type KhalaConversationLiveSubscription,
  type KhalaConversationLiveUpdate,
  type KhalaSyncAgentTimeline,
  type KhalaSyncLiveAgentGraph,
  type KhalaSyncConversation,
} from "@openagentsinc/khala-sync-client"

export type DesktopRuntimeLiveSubscribeRequest = Readonly<{
  subscriptionRef: string
  generation: number
  threadRef: string
  afterCursor?: number | null
}>

export type DesktopRuntimeLiveSubscribeResult =
  | Readonly<{ status: "subscribed" | "already_subscribed" }>
  | Readonly<{ status: "stale_generation"; activeGeneration: number }>
  | Readonly<{ status: "capacity_exceeded" | "unavailable" }>

export type DesktopRuntimeLiveSubscriptions = Readonly<{
  subscribe: (
    request: DesktopRuntimeLiveSubscribeRequest,
    listener: (update: KhalaConversationLiveUpdate) => void | Promise<void>,
  ) => Promise<DesktopRuntimeLiveSubscribeResult>
  unsubscribe: (subscriptionRef: string, generation: number) => Promise<boolean>
  metrics: (
    subscriptionRef: string,
    generation: number,
  ) => KhalaConversationLiveMetrics | null
  activeCount: () => number
  reset: () => Promise<void>
  dispose: () => Promise<void>
}>

type ActiveSubscription = Readonly<{
  generation: number
  subscription: KhalaConversationLiveSubscription
}>

/**
 * Process-owned registry behind the future Runtime Gateway live-event wire.
 *
 * Registry mutations serialize so a replacement always closes its prior
 * generation before the new scope opens. An old renderer may not unsubscribe
 * a replacement generation, and a bounded slot count prevents abandoned
 * renderer subscriptions from becoming unbounded host work.
 */
export const createDesktopRuntimeLiveSubscriptions = (input: Readonly<{
  conversation: () => KhalaSyncConversation | null
  timeline: () => KhalaSyncAgentTimeline | null
  agentGraph?: () => KhalaSyncLiveAgentGraph | null
  maxSubscriptions?: number
}>): DesktopRuntimeLiveSubscriptions => {
  const configuredMaximum = input.maxSubscriptions ?? 32
  const maxSubscriptions = Number.isSafeInteger(configuredMaximum) && configuredMaximum > 0
    ? Math.min(configuredMaximum, 64)
    : 32
  const active = new Map<string, ActiveSubscription>()
  let disposed = false
  let operations: Promise<void> = Promise.resolve()

  const serialize = <Value>(operation: () => Promise<Value>): Promise<Value> => {
    const result = operations.then(operation, operation)
    operations = result.then(() => undefined, () => undefined)
    return result
  }
  const closeAll = async (): Promise<void> => {
    const closing = [...active.values()]
    active.clear()
    await Promise.all(closing.map(entry => entry.subscription.close()))
  }

  return {
    subscribe: (request, listener) => serialize(async () => {
      if (disposed) return { status: "unavailable" }
      if (!Number.isSafeInteger(request.generation) || request.generation <= 0) {
        return { status: "unavailable" }
      }
      const current = active.get(request.subscriptionRef)
      if (current !== undefined) {
        if (request.generation < current.generation) {
          return {
            status: "stale_generation",
            activeGeneration: current.generation,
          }
        }
        if (request.generation === current.generation) {
          return { status: "already_subscribed" }
        }
        await current.subscription.close()
        active.delete(request.subscriptionRef)
      } else if (active.size >= maxSubscriptions) {
        return { status: "capacity_exceeded" }
      }

      const conversation = input.conversation()
      if (conversation === null) return { status: "unavailable" }
      try {
        const subscription = await openKhalaConversationLive({
          conversation,
          timeline: input.timeline() ?? undefined,
          agentGraph: input.agentGraph?.() ?? undefined,
          subscriptionRef: request.subscriptionRef,
          generation: request.generation,
          threadRef: request.threadRef,
          ...(request.afterCursor === undefined
            ? {}
            : { afterCursor: request.afterCursor }),
        }, listener)
        active.set(request.subscriptionRef, {
          generation: request.generation,
          subscription,
        })
        return { status: "subscribed" }
      } catch {
        return { status: "unavailable" }
      }
    }),
    unsubscribe: (subscriptionRef, generation) => serialize(async () => {
      const current = active.get(subscriptionRef)
      if (current === undefined || current.generation !== generation) return false
      active.delete(subscriptionRef)
      await current.subscription.close()
      return true
    }),
    metrics: (subscriptionRef, generation) => {
      const current = active.get(subscriptionRef)
      return current === undefined || current.generation !== generation
        ? null
        : current.subscription.metrics()
    },
    activeCount: () => active.size,
    reset: () => serialize(async () => {
      if (disposed) return
      await closeAll()
    }),
    dispose: () => serialize(async () => {
      if (disposed) return
      disposed = true
      await closeAll()
    }),
  }
}
