import { Response } from "@effect/ai"

import type { BlobRef } from "@openagentsinc/dse"

import type { SqlTag } from "../../dseServices"

export type UsageEncoded = typeof Response.Usage.Encoded

export type AiModelReceiptV1 = {
  readonly format: "openagents.ai.model_receipt"
  readonly formatVersion: 1

  readonly receiptId: string
  readonly createdAt: string

  readonly provider: string
  readonly modelId: string
  readonly paramsHash: string

  readonly promptBlobs: ReadonlyArray<BlobRef>
  readonly outputBlobs: ReadonlyArray<BlobRef>

  readonly finish?:
    | {
        readonly reason: string
        readonly usage: UsageEncoded
      }
    | undefined

  readonly toolCallIds?: ReadonlyArray<string> | undefined

  readonly promptTokenEstimate?: number | undefined
  readonly maxPromptTokens?: number | undefined

  readonly timing: {
    readonly startedAtMs: number
    readonly endedAtMs: number
    readonly durationMs: number
  }

  readonly correlation?: {
    readonly agentName: string
    readonly requestId: string
    readonly step: number
  }

  readonly result:
    | { readonly _tag: "Ok" }
    | { readonly _tag: "Error"; readonly errorName: string; readonly message: string }
}

export type AiToolReceiptV1 = {
  readonly format: "openagents.ai.tool_receipt"
  readonly formatVersion: 1

  readonly receiptId: string
  readonly createdAt: string

  readonly toolName: string
  readonly toolCallId: string
  readonly paramsHash: string
  readonly outputHash: string
  readonly latencyMs: number

  readonly sideEffects: ReadonlyArray<{
    readonly kind: string
    readonly target?: string
    readonly method?: string
    readonly status_code?: number | null
    readonly changed?: boolean | null
    readonly detail?: string | null
  }>

  readonly inputBlobs: ReadonlyArray<BlobRef>
  readonly outputBlobs: ReadonlyArray<BlobRef>

  readonly timing: {
    readonly startedAtMs: number
    readonly endedAtMs: number
    readonly durationMs: number
  }

  readonly correlation?: {
    readonly agentName: string
    readonly requestId: string
    readonly step: number
  }

  readonly result:
    | { readonly _tag: "Ok" }
    | { readonly _tag: "Error"; readonly errorName: string; readonly message: string }
}

export function initAiReceiptTables(sql: SqlTag): void {
  sql`create table if not exists ai_model_receipts (
    id text primary key,
    json text not null,
    created_at integer not null
  )`

  sql`create table if not exists ai_tool_receipts (
    id text primary key,
    json text not null,
    created_at integer not null
  )`
}

export function recordAiModelReceipt(sql: SqlTag, receipt: AiModelReceiptV1): void {
  const json = JSON.stringify(receipt)
  const ts = Date.now()
  sql`
    insert into ai_model_receipts (id, json, created_at)
    values (${receipt.receiptId}, ${json}, ${ts})
    on conflict(id) do nothing
  `
}

export function recordAiToolReceipt(sql: SqlTag, receipt: AiToolReceiptV1): void {
  const json = JSON.stringify(receipt)
  const ts = Date.now()
  sql`
    insert into ai_tool_receipts (id, json, created_at)
    values (${receipt.receiptId}, ${json}, ${ts})
    on conflict(id) do nothing
  `
}

export function listAiModelReceipts(
  sql: SqlTag,
  options: { readonly limit?: number } = {},
): ReadonlyArray<unknown> {
  const limit = Math.max(1, Math.min(200, options.limit ?? 50))
  const rows =
    sql<{ json: string }>`
      select json from ai_model_receipts
      order by created_at desc
      limit ${limit}
    ` || []

  const parsed: Array<unknown> = []
  for (const row of rows) {
    try {
      parsed.push(JSON.parse(row.json))
    } catch {
      // Ignore malformed rows.
    }
  }
  return parsed
}

export function listAiToolReceipts(
  sql: SqlTag,
  options: { readonly limit?: number } = {},
): ReadonlyArray<unknown> {
  const limit = Math.max(1, Math.min(200, options.limit ?? 50))
  const rows =
    sql<{ json: string }>`
      select json from ai_tool_receipts
      order by created_at desc
      limit ${limit}
    ` || []

  const parsed: Array<unknown> = []
  for (const row of rows) {
    try {
      parsed.push(JSON.parse(row.json))
    } catch {
      // Ignore malformed rows.
    }
  }
  return parsed
}
