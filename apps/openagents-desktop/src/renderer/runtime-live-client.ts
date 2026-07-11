import type {
  DesktopRuntimeGatewayEvent,
  DesktopRuntimeGatewayResponse,
} from "../runtime-gateway-contract.ts"

export type DesktopRuntimeLiveUpdate = Extract<
  DesktopRuntimeGatewayEvent,
  { readonly kind: "conversation.live.update" }
>

export type DesktopRuntimeLiveBridge = Readonly<{
  request: (value: unknown) => Promise<DesktopRuntimeGatewayResponse>
  subscribe: (listener: (event: DesktopRuntimeGatewayEvent) => void) => () => void
}>

export type DesktopRuntimeLiveHandle = Readonly<{
  close: () => Promise<void>
  closed: () => boolean
  cursor: () => number | null
  sequence: () => number
}>

export const openDesktopRuntimeLiveThread = async (input: Readonly<{
  bridge: DesktopRuntimeLiveBridge
  subscriptionRef: string
  generation: number
  threadRef: string
  afterCursor?: number | null
  onUpdate: (update: DesktopRuntimeLiveUpdate) => void
  onError?: (error: unknown) => void
}>): Promise<DesktopRuntimeLiveHandle | null> => {
  if (!Number.isSafeInteger(input.generation) || input.generation <= 0) return null
  let active = false
  let isClosed = false
  let lastSequence = 0
  let lastCursor = input.afterCursor ?? null
  let pendingInitial: DesktopRuntimeLiveUpdate | null = null

  const deliver = (update: DesktopRuntimeLiveUpdate): void => {
    try {
      input.onUpdate(update)
    } catch (error) {
      input.onError?.(error)
    }
  }
  const removeListener = input.bridge.subscribe(event => {
    if (isClosed || event.kind !== "conversation.live.update") return
    const envelope = event.envelope
    if (
      envelope.subscriptionRef !== input.subscriptionRef ||
      envelope.threadRef !== input.threadRef ||
      envelope.generation !== input.generation ||
      envelope.sequence <= lastSequence ||
      (lastCursor !== null && envelope.cursor !== null && envelope.cursor < lastCursor)
    ) return
    lastSequence = envelope.sequence
    if (envelope.cursor !== null) lastCursor = envelope.cursor
    if (!active) pendingInitial = event
    else deliver(event)
  })

  let outcome: DesktopRuntimeGatewayResponse
  try {
    outcome = await input.bridge.request({
      kind: "command",
      commandId: `renderer-live-subscribe-${input.subscriptionRef}-${input.generation}`,
      command: {
        id: "conversation.subscribe",
        subscriptionRef: input.subscriptionRef,
        generation: input.generation,
        threadRef: input.threadRef,
        ...(input.afterCursor === undefined ? {} : { afterCursor: input.afterCursor }),
      },
    })
  } catch (error) {
    removeListener()
    input.onError?.(error)
    return null
  }
  if (
    outcome.kind !== "conversation_subscription_outcome" ||
    outcome.subscriptionRef !== input.subscriptionRef ||
    outcome.generation !== input.generation ||
    (outcome.status !== "subscribed" && outcome.status !== "already_subscribed")
  ) {
    removeListener()
    return null
  }
  active = true
  if (pendingInitial !== null) {
    deliver(pendingInitial)
    pendingInitial = null
  }

  const close = async (): Promise<void> => {
    if (isClosed) return
    isClosed = true
    active = false
    pendingInitial = null
    removeListener()
    try {
      await input.bridge.request({
        kind: "command",
        commandId: `renderer-live-unsubscribe-${input.subscriptionRef}-${input.generation}`,
        command: {
          id: "conversation.unsubscribe",
          subscriptionRef: input.subscriptionRef,
          generation: input.generation,
        },
      })
    } catch (error) {
      input.onError?.(error)
    }
  }

  return {
    close,
    closed: () => isClosed,
    cursor: () => lastCursor,
    sequence: () => lastSequence,
  }
}
