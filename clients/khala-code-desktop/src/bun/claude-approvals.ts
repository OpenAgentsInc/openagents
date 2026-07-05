import { Deferred, Effect, Queue } from "effect"

export type ClaudeApprovalSuggestion = {
  readonly [key: string]: unknown
}

export type ClaudeApprovalRequest = {
  readonly id: string
  readonly input: Record<string, unknown>
  readonly options: {
    readonly blockedPath?: string
    readonly decisionReason?: string
    readonly description?: string
    readonly displayName?: string
    readonly suggestions?: readonly ClaudeApprovalSuggestion[]
    readonly title?: string
  }
  readonly toolName: string
}

export type ClaudeApprovalDecision =
  | {
      readonly behavior: "allow"
      readonly decisionClassification?: string
      readonly updatedInput?: Record<string, unknown>
      readonly updatedPermissions?: readonly ClaudeApprovalSuggestion[]
    }
  | {
      readonly behavior: "deny"
      readonly decisionClassification?: string
      readonly interrupt?: boolean
      readonly message: string
    }

export type ClaudeApprovalPending = {
  readonly deferred: Deferred.Deferred<ClaudeApprovalDecision>
  readonly request: ClaudeApprovalRequest
}

export type ClaudeApprovalService = {
  readonly canUseTool: (
    toolName: string,
    input: Record<string, unknown>,
    options: ClaudeApprovalRequest["options"] & { readonly signal?: AbortSignal },
  ) => Promise<ClaudeApprovalDecision>
  readonly pending: () => readonly ClaudeApprovalRequest[]
  readonly respond: (id: string, decision: ClaudeApprovalDecision) => Promise<boolean>
  readonly take: () => Promise<ClaudeApprovalPending>
}

const nonEmpty = (value: unknown): string | undefined =>
  typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined

const suggestionsFrom = (value: unknown): readonly ClaudeApprovalSuggestion[] | undefined =>
  Array.isArray(value)
    ? value.filter((item): item is ClaudeApprovalSuggestion =>
      typeof item === "object" && item !== null && !Array.isArray(item))
    : undefined

const requestOptionsFrom = (
  options: ClaudeApprovalRequest["options"] & { readonly signal?: AbortSignal },
): ClaudeApprovalRequest["options"] => {
  const blockedPath = nonEmpty(options.blockedPath)
  const decisionReason = nonEmpty(options.decisionReason)
  const description = nonEmpty(options.description)
  const displayName = nonEmpty(options.displayName)
  const suggestions = suggestionsFrom(options.suggestions)
  const title = nonEmpty(options.title)
  return {
    ...(blockedPath === undefined ? {} : { blockedPath }),
    ...(decisionReason === undefined ? {} : { decisionReason }),
    ...(description === undefined ? {} : { description }),
    ...(displayName === undefined ? {} : { displayName }),
    ...(suggestions === undefined ? {} : { suggestions }),
    ...(title === undefined ? {} : { title }),
  }
}

export type ClaudeApprovalServiceOptions = {
  /**
   * Fired synchronously the moment a new approval request is queued, before
   * any consumer polls `pending()`. KS-6.9 (#8419): the desktop UI uses this
   * to push an immediate IPC notification instead of waiting on the 1s poll
   * tick, so approval-to-execution latency is bounded by IPC dispatch, not
   * by the polling interval.
   */
  readonly onRequestQueued?: (request: ClaudeApprovalRequest) => void
}

export const createClaudeApprovalService = (
  serviceOptions: ClaudeApprovalServiceOptions = {},
): ClaudeApprovalService => {
  const pending = new Map<string, ClaudeApprovalPending>()
  const queue = Effect.runSync(Queue.unbounded<ClaudeApprovalPending>())
  let sequence = 0

  const respond = async (id: string, decision: ClaudeApprovalDecision): Promise<boolean> => {
    const item = pending.get(id)
    if (item === undefined) return false
    pending.delete(id)
    await Effect.runPromise(Deferred.succeed(item.deferred, decision))
    return true
  }

  return {
    async canUseTool(toolName, input, options) {
      if (options.signal?.aborted === true) {
        return {
          behavior: "deny",
          decisionClassification: "aborted",
          interrupt: true,
          message: "Claude tool approval was aborted.",
        }
      }
      const id = `claude-approval-${Date.now().toString(36)}-${++sequence}`
      const deferred = Effect.runSync(Deferred.make<ClaudeApprovalDecision>())
      const request = {
        id,
        input,
        options: requestOptionsFrom(options),
        toolName,
      }
      const item = { deferred, request }
      pending.set(id, item)
      serviceOptions.onRequestQueued?.(request)
      const onAbort = (): void => {
        void respond(id, {
          behavior: "deny",
          decisionClassification: "aborted",
          interrupt: true,
          message: "Claude tool approval was aborted.",
        })
      }
      options.signal?.addEventListener("abort", onAbort, { once: true })
      await Effect.runPromise(Queue.offer(queue, item))
      try {
        return await Effect.runPromise(Deferred.await(deferred))
      } finally {
        options.signal?.removeEventListener("abort", onAbort)
        pending.delete(id)
      }
    },
    pending: () => [...pending.values()].map(item => item.request),
    respond,
    take: () => Effect.runPromise(Queue.take(queue)),
  }
}

export const createClaudeHeadlessAutoDenyApprovalService = (
  reason = "Claude tool approval denied because Khala Code headless mode cannot ask a human.",
): ClaudeApprovalService => ({
  async canUseTool(toolName) {
    return {
      behavior: "deny",
      decisionClassification: "headless_auto_deny",
      interrupt: false,
      message: `${reason} Tool: ${toolName}.`,
    }
  },
  pending: () => [],
  respond: async () => false,
  take: async () => {
    throw new Error(`${reason} Headless approval queue is unavailable.`)
  },
})
