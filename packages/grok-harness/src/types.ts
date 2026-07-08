/**
 * Provisional multi-harness types for Grok until MH-0 (#8581) merges
 * shared literals into @openagentsinc/agent-runtime-schema.
 *
 * Keep names aligned with docs/fable/2026-07-08-multi-harness-*.md
 */

export const GROK_HARNESS_KIND = "grok_cli" as const
export type GrokHarnessKind = typeof GROK_HARNESS_KIND

export const GROK_WORKER_KIND = "grok" as const
export type GrokWorkerKind = typeof GROK_WORKER_KIND

export type MarginalCostClass =
  | "free"
  | "subscription"
  | "api_metered"
  | "not_measured"

export type AuthPlane = "cli_session" | "api_key" | "unknown"

export type GrokFailureClass =
  | "account_exhausted"
  | "account_rate_limited"
  | "account_quota_exhausted"
  | "auth_required"
  | "binary_missing"
  | "timeout"
  | "unknown"

/** Neutral chat turn events (mirror of KhalaCodeDesktopChatTurnEvent shape). */
export type NeutralChatTurnEvent =
  | {
      type: "thread_ready"
      threadId: string
      turnId: string
    }
  | {
      type: "message_start"
      turnId: string
      message: { id: string; role: "assistant"; content: string }
    }
  | {
      type: "message_delta"
      turnId: string
      messageId: string
      delta: string
    }
  | {
      type: "message_replace"
      turnId: string
      message: { id: string; role: "assistant"; content: string }
    }
  | {
      type: "message_done"
      turnId: string
      messageId: string
    }
  | {
      type: "tool_event"
      turnId: string
      event: {
        kind: string
        name?: string
        detail?: string
      }
    }

export type MeteringLabel = "exact" | "not_measured"

export type GrokUsageSnapshot = {
  readonly metering: MeteringLabel
  readonly inputTokens?: number
  readonly outputTokens?: number
  readonly totalTokens?: number
  readonly wallClockMs: number
  readonly model?: string
  readonly plane: AuthPlane
  readonly marginalCostClass: MarginalCostClass
}

export type GrokSessionRef = {
  readonly desktopSessionId: string
  readonly grokSessionId: string
  readonly lastTurnId?: string
  readonly updatedAt: string
  readonly capabilities: {
    readonly resume: boolean
    readonly fork: boolean
  }
}

export type WorkerClaimPin = {
  readonly claimRef: string
  readonly workUnitRef: string
  readonly runRef: string
  readonly repo?: string
  readonly commit?: string
  readonly branch?: string
  readonly verifyCommand?: string
  readonly cwd: string
}

export type WorkerCloseout = {
  readonly ok: boolean
  readonly claimRef: string
  readonly stopReason: string
  readonly text: string
  readonly usage: GrokUsageSnapshot
  readonly failureClass?: GrokFailureClass
}
