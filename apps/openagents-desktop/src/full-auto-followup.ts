import type { FableLocalEvent } from "./fable-local-contract.ts"

export type FullAutoPromotedFollowup = Readonly<{
  queueRef: string
  clientUserMessageId: string
  message: string
}>

/**
 * Transfers one durable queue promotion from a main-owned background turn to
 * the next Full Auto dispatch. Foreground turns stay renderer-owned. `take`
 * is destructive so overlapping reconciliation passes cannot replay the same
 * promoted identity.
 */
export const makeFullAutoFollowupHandoff = () => {
  const promoted = new Map<string, FullAutoPromotedFollowup>()
  return {
    observe: (input: Readonly<{
      threadRef: string
      background: boolean
      fullAuto: boolean
      event: FableLocalEvent
    }>): void => {
      if (
        !input.background || !input.fullAuto ||
        input.event.kind !== "followup_promoted" ||
        input.event.intentRef === undefined ||
        input.event.clientUserMessageId === undefined
      ) return
      promoted.set(input.threadRef, {
        queueRef: input.event.queueRef,
        clientUserMessageId: input.event.clientUserMessageId,
        message: input.event.message,
      })
    },
    take: (threadRef: string): FullAutoPromotedFollowup | null => {
      const value = promoted.get(threadRef) ?? null
      if (value !== null) promoted.delete(threadRef)
      return value
    },
  }
}
