import type {
  ConfirmedRuntimeInteraction,
  DesktopRuntimeGatewayEvent,
  DesktopRuntimeGatewayResponse,
  RuntimeInteractionDecisionEnvelope,
} from "../runtime-gateway-contract.ts"

export type RuntimeInteractionRequest = (
  value: unknown,
) => Promise<DesktopRuntimeGatewayResponse>

export type RuntimeInteractionDecisionResult =
  | Readonly<{ status: "confirmed_resolved"; interaction: ConfirmedRuntimeInteraction }>
  | Readonly<{ status: "confirmed_expired" | "confirmed_revoked"; interaction: ConfirmedRuntimeInteraction }>
  | Readonly<{ status: "pending_reconcile" | "unavailable" }>

export type DesktopRuntimeInteractionHost = Readonly<{
  list: (threadRef: string) => Promise<ReadonlyArray<ConfirmedRuntimeInteraction> | null>
  decide: (input: Readonly<{
    interactionRef: string
    threadRef: string
    turnRef: string
    envelope: RuntimeInteractionDecisionEnvelope
  }>) => Promise<RuntimeInteractionDecisionResult>
}>

let requestSequence = 0

/**
 * Renderer boundary for canonical durable interactions. A mutation receipt is
 * only an enqueue acknowledgement: this host reports resolved only after the
 * exact confirmed post-image replaces pending. Gateway events wake a re-read,
 * so this adds no renderer timeline polling loop.
 */
export const makeDesktopRuntimeInteractionHost = (options: Readonly<{
  request: RuntimeInteractionRequest
  subscribe?: (listener: (event: DesktopRuntimeGatewayEvent) => void) => () => void
  confirmationTimeoutMs?: number
}>): DesktopRuntimeInteractionHost => {
  const list = async (
    threadRef: string,
  ): Promise<ReadonlyArray<ConfirmedRuntimeInteraction> | null> => {
    const response = await options.request({
      kind: "query",
      requestId: `renderer-interactions-${++requestSequence}`,
      query: { id: "runtime.interactions", threadRef },
    })
    return response.kind === "runtime_interactions" && response.threadRef === threadRef
      ? response.interactions
      : null
  }

  return {
    list,
    decide: async input => {
      const terminal = async (): Promise<RuntimeInteractionDecisionResult | null> => {
        const interactions = await list(input.threadRef)
        if (interactions === null) return { status: "unavailable" }
        const interaction = interactions.find(candidate =>
          candidate.interactionRef === input.interactionRef &&
          candidate.threadId === input.threadRef &&
          candidate.turnId === input.turnRef)
        if (interaction?.status === "resolved" && interaction.decisionRef === input.envelope.decisionRef) {
          return { status: "confirmed_resolved", interaction }
        }
        if (interaction?.status === "expired") return { status: "confirmed_expired", interaction }
        if (interaction?.status === "revoked") return { status: "confirmed_revoked", interaction }
        return null
      }

      const immediate = await terminal()
      if (immediate !== null) return immediate

      let checking = false
      let finish: ((result: RuntimeInteractionDecisionResult) => void) | null = null
      const confirmed = new Promise<RuntimeInteractionDecisionResult>(resolve => { finish = resolve })
      const check = (): void => {
        if (checking || finish === null) return
        checking = true
        void terminal().then(result => {
          checking = false
          if (result !== null && finish !== null) {
            const resolve = finish
            finish = null
            resolve(result)
          }
        })
      }
      const unsubscribe = options.subscribe?.(() => check())
      const response = await options.request({
        kind: "command",
        commandId: `renderer-interaction-decision-${++requestSequence}`,
        command: {
          id: "runtime.decideInteraction",
          interactionRef: input.interactionRef,
          threadRef: input.threadRef,
          turnRef: input.turnRef,
          envelope: input.envelope,
        },
      })
      if (
        response.kind !== "runtime_interaction_decision_outcome" ||
        response.status !== "pending_reconcile"
      ) {
        unsubscribe?.()
        finish = null
        return { status: "unavailable" }
      }

      const afterEnqueue = await terminal()
      if (afterEnqueue !== null) {
        unsubscribe?.()
        finish = null
        return afterEnqueue
      }
      if (unsubscribe === undefined) {
        finish = null
        return { status: "pending_reconcile" }
      }

      const timeout = setTimeout(() => {
        if (finish !== null) {
          const resolve = finish
          finish = null
          resolve({ status: "pending_reconcile" })
        }
      }, options.confirmationTimeoutMs ?? 30_000)
      try {
        return await confirmed
      } finally {
        clearTimeout(timeout)
        unsubscribe()
      }
    },
  }
}
