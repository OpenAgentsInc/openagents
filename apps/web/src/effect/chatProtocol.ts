export type ChatRole = "user" | "assistant"

export type ChatTextPart = {
  readonly type: "text"
  readonly text: string
  readonly state?: "streaming" | "done"
}

export type ChatToolPart = {
  readonly type: `tool-${string}` | "dynamic-tool"
  readonly toolName?: string
  readonly toolCallId: string
  readonly state: string
  readonly input?: unknown
  readonly output?: unknown
  readonly errorText?: string
  readonly preliminary?: boolean
  readonly approval?: { readonly id: string; readonly approved?: boolean; readonly reason?: string }
  readonly rawInput?: unknown
}

export type ChatDseBasePart = {
  readonly type:
    | "dse.signature"
    | "dse.tool"
    | "dse.compile"
    | "dse.promote"
    | "dse.rollback"
    | "dse.budget_exceeded"
  readonly v: number
  readonly id: string
  readonly state: string
  readonly tsMs?: number
}

export type ChatDseSignaturePart = ChatDseBasePart & {
  readonly type: "dse.signature"
  readonly signatureId: string
  readonly compiled_id?: string
  readonly receiptId?: string
  readonly model?: {
    readonly modelId?: string
    readonly provider?: string
    readonly route?: string
    readonly fallbackModelId?: string
  }
  readonly strategyId?: string
  readonly strategyReason?: string
  readonly timing?: { readonly durationMs?: number }
  readonly budget?: { readonly limits?: Record<string, number>; readonly usage?: Record<string, number> }
  readonly contextPressure?: unknown
  readonly promptRenderStats?: unknown
  readonly rlmTrace?: unknown
  readonly outputPreview?: unknown
  readonly errorText?: string
}

export type ChatDseToolPart = ChatDseBasePart & {
  readonly type: "dse.tool"
  readonly toolName: string
  readonly toolCallId: string
  readonly timing?: { readonly durationMs?: number }
  readonly input?: unknown
  readonly output?: unknown
  readonly errorText?: string
}

export type ChatDseCompilePart = ChatDseBasePart & {
  readonly type: "dse.compile"
  readonly signatureId: string
  readonly jobHash: string
  readonly candidates?: number
  readonly best?: { readonly compiled_id: string; readonly reward?: number }
  readonly reportId?: string
  readonly errorText?: string
}

export type ChatDsePromotePart = ChatDseBasePart & {
  readonly type: "dse.promote"
  readonly signatureId: string
  readonly from?: string
  readonly to?: string
  readonly reason?: string
}

export type ChatDseRollbackPart = ChatDseBasePart & {
  readonly type: "dse.rollback"
  readonly signatureId: string
  readonly from?: string
  readonly to?: string
  readonly reason?: string
}

export type ChatDseBudgetExceededPart = ChatDseBasePart & {
  readonly type: "dse.budget_exceeded"
  readonly message?: string
  readonly budget?: { readonly limits?: Record<string, number>; readonly usage?: Record<string, number> }
}

export type ChatPart =
  | ChatTextPart
  | ChatToolPart
  | ChatDseSignaturePart
  | ChatDseToolPart
  | ChatDseCompilePart
  | ChatDsePromotePart
  | ChatDseRollbackPart
  | ChatDseBudgetExceededPart
  | { readonly type: string; readonly [k: string]: unknown }

/** LLM finish part from the stream (usage, reason, modelId when available). */
export type ChatMessageFinish = {
  readonly reason?: string
  readonly usage?: {
    readonly inputTokens?: number
    readonly outputTokens?: number
    readonly totalTokens?: number
    readonly promptTokens?: number
    readonly completionTokens?: number
  }
  readonly modelId?: string
  readonly provider?: string
  readonly modelRoute?: string
  readonly modelFallbackId?: string
  readonly timeToFirstTokenMs?: number
  readonly timeToCompleteMs?: number
}

export type ChatMessage = {
  readonly id: string
  readonly role: ChatRole
  readonly parts: ReadonlyArray<ChatPart>
  readonly runId?: string
  /** Present for assistant messages when the stream included a finish part (LLM usage/model). */
  readonly finish?: ChatMessageFinish
}
