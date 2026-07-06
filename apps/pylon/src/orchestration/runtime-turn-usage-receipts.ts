import type { KhalaRuntimeEvent } from "@openagentsinc/khala-sync"
import type { KhalaRuntimeLane } from "@openagentsinc/agent-runtime-schema"

export const KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH =
  "/api/khala/cloud/runtime-turn-usage"

export const KHALA_CLOUD_RUNTIME_USAGE_SCHEMA_VERSION =
  "openagents.khala_cloud_runtime_turn_usage.v1" as const

export type RuntimeUsageReceiptProvider = Readonly<{
  backendProfile: string
  model: string
  provider: string
}>

export type RuntimeTurnUsageReceiptInput = Readonly<{
  agentToken: string
  baseUrl: string
  event: Extract<KhalaRuntimeEvent, { kind: "usage.recorded" }>
  fetchImpl?: typeof globalThis.fetch
  lane: KhalaRuntimeLane
  ownerUserId: string
  provider: RuntimeUsageReceiptProvider
  pylonRef: string
  threadId: string
  turnId: string
}>

export type RuntimeTurnUsageReceiptResult =
  | Readonly<{
      ok: true
      insertedTokenUsage: boolean
      tokenUsageEventRef: string | null
      tokensServedDelta: number
    }>
  | Readonly<{
      ok: false
      error: "bad_response" | "network_failed" | "unauthorized" | "validation_failed"
      reason: string | null
      status: number | null
    }>

const boundedReason = (value: unknown): string | null => {
  if (typeof value !== "string" || value.length === 0) return null
  return value.slice(0, 300)
}

const integerOrZero = (value: number | undefined): number =>
  typeof value === "number" && Number.isFinite(value)
    ? Math.max(0, Math.trunc(value))
    : 0

export const recordRuntimeTurnUsageReceipt = async (
  input: RuntimeTurnUsageReceiptInput,
): Promise<RuntimeTurnUsageReceiptResult> => {
  const usage = input.event.usage
  const body = {
    schemaVersion: KHALA_CLOUD_RUNTIME_USAGE_SCHEMA_VERSION,
    backendProfile: input.provider.backendProfile,
    lane: input.lane,
    model: input.provider.model,
    observedAt: input.event.observedAt,
    ownerUserId: input.ownerUserId,
    provider: input.provider.provider,
    pylonRef: input.pylonRef,
    runtimeEventId: input.event.eventId,
    threadId: input.threadId,
    turnId: input.turnId,
    usage: {
      usageRef: usage.usageRef,
      inputTokens: integerOrZero(usage.inputTokens),
      outputTokens: integerOrZero(usage.outputTokens),
      reasoningTokens: integerOrZero(usage.reasoningTokens),
      cacheReadInputTokens: integerOrZero(usage.cacheReadInputTokens),
      cacheWriteInputTokens: integerOrZero(usage.cacheWriteInputTokens),
      totalTokens: integerOrZero(usage.totalTokens),
    },
  }

  const fetchImpl = input.fetchImpl ?? globalThis.fetch
  let response: Response
  try {
    response = await fetchImpl(
      new URL(KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH, input.baseUrl).toString(),
      {
        body: JSON.stringify(body),
        headers: {
          Authorization: `Bearer ${input.agentToken}`,
          "content-type": "application/json",
        },
        method: "POST",
      },
    )
  } catch (error) {
    return {
      error: "network_failed",
      ok: false,
      reason: boundedReason(error instanceof Error ? error.message : error),
      status: null,
    }
  }

  if (response.status === 401) {
    return { error: "unauthorized", ok: false, reason: null, status: 401 }
  }

  let payload: unknown
  try {
    payload = await response.json()
  } catch {
    return {
      error: "bad_response",
      ok: false,
      reason: "response body was not JSON",
      status: response.status,
    }
  }

  const record = payload as {
    insertedTokenUsage?: unknown
    reason?: unknown
    tokenUsageEventRef?: unknown
    tokensServedDelta?: unknown
  }

  if (response.status === 400 || response.status === 403) {
    return {
      error: "validation_failed",
      ok: false,
      reason: boundedReason(record.reason),
      status: response.status,
    }
  }
  if (response.status < 200 || response.status >= 300) {
    return {
      error: "bad_response",
      ok: false,
      reason: boundedReason(record.reason) ?? `unexpected status ${response.status}`,
      status: response.status,
    }
  }

  return {
    insertedTokenUsage: record.insertedTokenUsage === true,
    ok: true,
    tokenUsageEventRef:
      typeof record.tokenUsageEventRef === "string"
        ? record.tokenUsageEventRef
        : null,
    tokensServedDelta:
      typeof record.tokensServedDelta === "number"
        ? record.tokensServedDelta
        : 0,
  }
}
