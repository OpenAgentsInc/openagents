import { Effect } from "effect"
import { boundText, html } from "@openagentsinc/effuse"
import type { BoundedText, ToolPartModel, ToolPartStatus } from "@openagentsinc/effuse"

import { UiBlobStore } from "../blobStore"

import type { ToolContract } from "../../effect/contracts"
import type {
  ChatDseBudgetExceededPart,
  ChatDseCompilePart,
  ChatDsePromotePart,
  ChatDseRollbackPart,
  ChatDseSignaturePart,
  ChatDseToolPart,
  ChatMessage,
} from "../../effect/chatProtocol"
import type {
  DseBudgetExceededCardModel,
  DseCompileCardModel,
  DsePromoteCardModel,
  DseRollbackCardModel,
  DseSignatureCardModel,
  L402PaymentStateCardModel,
  PaymentStateKind,
  RenderPart,
} from "../../effuse-pages/autopilot"

type UiPart = ChatMessage["parts"][number]
const LIGHTNING_L402_FETCH_TOOL_NAME = "lightning_l402_fetch"
const LIGHTNING_L402_APPROVE_TOOL_NAME = "lightning_l402_approve"

function safeStableStringify(value: unknown, indent = 2): string {
  if (value == null) return String(value)
  if (typeof value === "string") return value
  try {
    return JSON.stringify(value, null, indent)
  } catch {
    return String(value)
  }
}

const asRecord = (value: unknown): Record<string, unknown> | null =>
  value && typeof value === "object" ? (value as Record<string, unknown>) : null

const asString = (value: unknown): string | undefined =>
  typeof value === "string" && value.length > 0 ? value : undefined

const asFiniteNumber = (value: unknown): number | undefined =>
  typeof value === "number" && Number.isFinite(value) ? value : undefined

const asBoolean = (value: unknown): boolean | undefined =>
  typeof value === "boolean" ? value : undefined

const asTextState = (value: unknown): "streaming" | "done" | undefined =>
  value === "streaming" || value === "done" ? value : undefined

function isTextPart(part: unknown): part is { readonly type: "text"; readonly text: string; readonly state?: string } {
  const rec = asRecord(part)
  return rec?.type === "text" && typeof rec.text === "string"
}

function isToolPart(
  part: unknown,
): part is
  | {
      readonly type: `tool-${string}`
      readonly toolCallId: string
      readonly state: string
      readonly toolName?: string
      readonly input?: unknown
      readonly output?: unknown
      readonly errorText?: string
      readonly preliminary?: boolean
      readonly approval?: { readonly id: string; readonly approved?: boolean; readonly reason?: string }
      readonly rawInput?: unknown
    }
  | {
      readonly type: "dynamic-tool"
      readonly toolName: string
      readonly toolCallId: string
      readonly state: string
      readonly input?: unknown
      readonly output?: unknown
      readonly errorText?: string
      readonly preliminary?: boolean
      readonly approval?: { readonly id: string; readonly approved?: boolean; readonly reason?: string }
      readonly rawInput?: unknown
    } {
  const rec = asRecord(part)
  if (!rec) return false
  const type = rec.type
  if (type === "dynamic-tool") {
    return (
      typeof rec.toolName === "string" &&
      typeof rec.toolCallId === "string" &&
      typeof rec.state === "string"
    )
  }
  return (
    typeof type === "string" &&
    type.startsWith("tool-") &&
    typeof rec.toolCallId === "string" &&
    typeof rec.state === "string"
  )
}

function getToolPartName(part: { type: string; toolName?: string }): string {
  if (part.type === "dynamic-tool") return String(part.toolName ?? "tool")
  if (part.type.startsWith("tool-")) return part.type.slice("tool-".length)
  return part.type
}

function isDseSignaturePart(part: unknown): part is ChatDseSignaturePart {
  const rec = asRecord(part)
  return rec?.type === "dse.signature" && typeof rec.id === "string" && typeof rec.signatureId === "string" && typeof rec.state === "string"
}

function isDseToolPart(part: unknown): part is ChatDseToolPart {
  const rec = asRecord(part)
  return rec?.type === "dse.tool" && typeof rec.id === "string" && typeof rec.toolName === "string" && typeof rec.toolCallId === "string" && typeof rec.state === "string"
}

function isDseCompilePart(part: unknown): part is ChatDseCompilePart {
  const rec = asRecord(part)
  return rec?.type === "dse.compile" && typeof rec.id === "string" && typeof rec.signatureId === "string" && typeof rec.jobHash === "string" && typeof rec.state === "string"
}

function isDsePromotePart(part: unknown): part is ChatDsePromotePart {
  const rec = asRecord(part)
  return rec?.type === "dse.promote" && typeof rec.id === "string" && typeof rec.signatureId === "string" && typeof rec.state === "string"
}

function isDseRollbackPart(part: unknown): part is ChatDseRollbackPart {
  const rec = asRecord(part)
  return rec?.type === "dse.rollback" && typeof rec.id === "string" && typeof rec.signatureId === "string" && typeof rec.state === "string"
}

function isDseBudgetExceededPart(part: unknown): part is ChatDseBudgetExceededPart {
  const rec = asRecord(part)
  return rec?.type === "dse.budget_exceeded" && typeof rec.id === "string" && typeof rec.state === "string"
}

const TOOL_IO_MAX_CHARS = 4000
const DSE_PREVIEW_MAX_CHARS = 2000

const toToolStatus = (state: string): ToolPartStatus => {
  switch (state) {
    case "output-available":
      return "tool-result"
    case "output-error":
      return "tool-error"
    case "output-denied":
      return "tool-denied"
    case "approval-requested":
    case "approval-responded":
      return "tool-approval"
    default:
      return state.startsWith("input-") ? "tool-call" : "tool-call"
  }
}

const toToolStatusFromDseState = (state: string): ToolPartStatus => {
  if (state === "ok") return "tool-result"
  if (state === "error") return "tool-error"
  if (state === "approval-requested" || state === "approval-responded") return "tool-approval"
  return "tool-call"
}

const putText = ({ text, mime }: { readonly text: string; readonly mime?: string }) =>
  Effect.sync(() => UiBlobStore.putText({ text, mime }))

const toBoundedText = (value: unknown, opts: { readonly maxChars: number; readonly mime: string }): BoundedText => {
  return Effect.runSync(
    boundText({
      text: safeStableStringify(value),
      maxChars: opts.maxChars,
      putText,
      mime: opts.mime,
    }),
  )
}

const toBoundedTextString = (text: string, opts: { readonly maxChars: number; readonly mime: string }): BoundedText => {
  return Effect.runSync(
    boundText({
      text,
      maxChars: opts.maxChars,
      putText,
      mime: opts.mime,
    }),
  )
}

const toolModelFromCore = (opts: {
  readonly toolName: string
  readonly toolCallId: string
  readonly status: ToolPartStatus
  readonly summary: string
  readonly input?: unknown
  readonly output?: unknown
  readonly errorText?: string
  readonly extra?: ReturnType<typeof html> | null
}): ToolPartModel => {
  const input = opts.input !== undefined ? toBoundedText(opts.input, { maxChars: TOOL_IO_MAX_CHARS, mime: "application/json" }) : undefined
  const output =
    opts.output !== undefined ? toBoundedText(opts.output, { maxChars: TOOL_IO_MAX_CHARS, mime: "application/json" }) : undefined
  const error =
    opts.errorText && opts.errorText.length > 0
      ? toBoundedTextString(opts.errorText, { maxChars: TOOL_IO_MAX_CHARS, mime: "text/plain" })
      : undefined

  return {
    status: opts.status,
    toolName: opts.toolName,
    toolCallId: opts.toolCallId,
    summary: opts.summary,
    details: {
      extra: opts.extra ?? undefined,
      input,
      output,
      error,
    },
  }
}

export type L402PaymentMetadata = {
  readonly toolName: typeof LIGHTNING_L402_FETCH_TOOL_NAME
  readonly toolCallId: string
  readonly status: "completed" | "cached" | "blocked" | "failed"
  readonly taskId?: string
  readonly paymentId?: string
  readonly amountMsats?: number
  readonly responseStatusCode?: number
  readonly responseContentType?: string
  readonly responseBytes?: number
  readonly responseBodyTextPreview?: string
  readonly responseBodySha256?: string
  readonly cacheHit?: boolean
  readonly paid?: boolean
  readonly cacheStatus?: string
  readonly paymentBackend?: string
  readonly approvalRequired?: boolean
  readonly proofReference?: string
  readonly denyReason?: string
  readonly denyReasonCode?: string
  readonly host?: string
  readonly quotedAmountMsats?: number
  readonly url?: string
  readonly method?: string
  readonly scope?: string
  readonly maxSpendMsats?: number
}

const paymentStateFromMetadataStatus = (status: L402PaymentMetadata["status"]): Exclude<PaymentStateKind, "payment.intent"> => {
  if (status === "completed") return "payment.sent"
  if (status === "cached") return "payment.cached"
  if (status === "blocked") return "payment.blocked"
  return "payment.failed"
}

const paymentStateFromToolStart = (opts: {
  readonly toolName: string
  readonly state: string
}): PaymentStateKind | null => {
  if (opts.toolName !== LIGHTNING_L402_FETCH_TOOL_NAME) return null
  if (
    opts.state === "start" ||
    opts.state.startsWith("input-") ||
    opts.state === "approval-requested" ||
    opts.state === "approval-responded"
  ) {
    return "payment.intent"
  }
  return null
}

const formatSatsFromMsats = (msats: number): string => {
  const sats = Math.round((msats / 1000) * 1000) / 1000
  const text = Number.isInteger(sats)
    ? String(sats)
    : sats.toFixed(3).replace(/0+$/, "").replace(/\.$/, "")
  return `${text} sats`
}

const formatPolicyDeniedReason = (metadata: L402PaymentMetadata): string | undefined => {
  if (metadata.status !== "blocked") return undefined

  switch (metadata.denyReasonCode) {
    case "amount_over_cap": {
      if (metadata.quotedAmountMsats === undefined || metadata.maxSpendMsats === undefined) return undefined
      return `Blocked: quoted ${formatSatsFromMsats(metadata.quotedAmountMsats)} > cap ${formatSatsFromMsats(metadata.maxSpendMsats)}`
    }
    case "host_not_allowlisted": {
      return metadata.host ? `Blocked: host not allowlisted (${metadata.host})` : "Blocked: host not allowlisted"
    }
    case "host_blocked": {
      return metadata.host ? `Blocked: host blocked (${metadata.host})` : "Blocked: host blocked"
    }
    default:
      return undefined
  }
}

const paymentStateCardFromTool = (opts: {
  readonly toolName: string
  readonly toolCallId: string
  readonly state: string
  readonly input?: unknown
  readonly output?: unknown
  readonly errorText?: string
}): L402PaymentStateCardModel | null => {
  const metadata = toL402PaymentMetadata({
    toolName: opts.toolName,
    toolCallId: opts.toolCallId,
    input: opts.input,
    output: opts.output,
    state: opts.state,
  })

  if (metadata) {
    const deniedReason = formatPolicyDeniedReason(metadata) ?? metadata.denyReason
    return {
      state: paymentStateFromMetadataStatus(metadata.status),
      toolCallId: metadata.toolCallId,
      taskId: metadata.taskId,
      url: metadata.url,
      method: metadata.method,
      maxSpendMsats: metadata.maxSpendMsats,
      quotedAmountMsats: metadata.quotedAmountMsats,
      amountMsats: metadata.amountMsats,
      responseStatusCode: metadata.responseStatusCode,
      responseContentType: metadata.responseContentType,
      responseBytes: metadata.responseBytes,
      responseBodySha256: metadata.responseBodySha256,
      cacheHit: metadata.cacheHit,
      paid: metadata.paid,
      cacheStatus: metadata.cacheStatus,
      paymentBackend: metadata.paymentBackend,
      proofReference: metadata.proofReference,
      denyReason: deniedReason,
      denyReasonCode: metadata.denyReasonCode,
      host: metadata.host,
      statusLabel: metadata.status,
    }
  }

  const intentState = paymentStateFromToolStart({ toolName: opts.toolName, state: opts.state })
  if (!intentState) return null

  const input = asRecord(opts.input)
  const output = asRecord(opts.output)

  return {
    state: intentState,
    toolCallId: opts.toolCallId,
    taskId: asString(output?.taskId),
    url: asString(input?.url),
    method: asString(input?.method),
    maxSpendMsats: asFiniteNumber(input?.maxSpendMsats),
    statusLabel: opts.state,
    denyReason: opts.errorText,
  }
}

const l402ToolSummary = (state: PaymentStateKind, model: L402PaymentStateCardModel): string => {
  const status = model.statusLabel ? ` (${model.statusLabel})` : ""
  if (state === "payment.intent") return `payment.intent${status}`
  if (state === "payment.sent") {
    return model.proofReference ? `payment.sent • proof ${model.proofReference}` : `payment.sent${status}`
  }
  if (state === "payment.cached") {
    return model.proofReference ? `payment.cached • proof ${model.proofReference}` : `payment.cached${status}`
  }
  if (state === "payment.blocked") {
    return model.denyReason ? `payment.blocked • ${model.denyReason}` : `payment.blocked${status}`
  }
  return model.denyReason ? `payment.failed • ${model.denyReason}` : `payment.failed${status}`
}

const isL402PaymentStatus = (
  value: unknown,
): value is L402PaymentMetadata["status"] =>
  value === "completed" || value === "cached" || value === "blocked" || value === "failed"

const toL402PaymentMetadata = (opts: {
  readonly toolName: string
  readonly toolCallId: string
  readonly input?: unknown
  readonly output?: unknown
  readonly state?: string
}): L402PaymentMetadata | null => {
  if (opts.toolName !== LIGHTNING_L402_FETCH_TOOL_NAME && opts.toolName !== LIGHTNING_L402_APPROVE_TOOL_NAME) return null
  const input = asRecord(opts.input)
  const output = asRecord(opts.output)

  const terminal =
    opts.toolName === LIGHTNING_L402_APPROVE_TOOL_NAME
      ? asRecord(output?.terminal)
      : output

  const outputStatus = terminal?.status
  const status = isL402PaymentStatus(outputStatus)
    ? outputStatus
    : opts.state === "output-error"
      ? "failed"
      : null
  if (!status) return null

  return {
    toolName: LIGHTNING_L402_FETCH_TOOL_NAME,
    toolCallId: opts.toolCallId,
    status,
    taskId: asString(terminal?.taskId) ?? asString(output?.taskId),
    paymentId: asString(terminal?.paymentId),
    amountMsats: asFiniteNumber(terminal?.amountMsats) ?? asFiniteNumber(output?.maxSpendMsats) ?? asFiniteNumber(input?.maxSpendMsats),
    responseStatusCode: asFiniteNumber(terminal?.responseStatusCode),
    responseContentType: asString(terminal?.responseContentType),
    responseBytes: asFiniteNumber(terminal?.responseBytes),
    responseBodyTextPreview: asString(terminal?.responseBodyTextPreview),
    responseBodySha256: asString(terminal?.responseBodySha256),
    cacheHit: asBoolean(terminal?.cacheHit),
    paid: asBoolean(terminal?.paid),
    cacheStatus: asString(terminal?.cacheStatus),
    paymentBackend: asString(terminal?.paymentBackend),
    approvalRequired: asBoolean(terminal?.approvalRequired),
    proofReference: asString(terminal?.proofReference),
    denyReason: asString(terminal?.denyReason),
    denyReasonCode: asString(terminal?.denyReasonCode),
    host: asString(terminal?.host),
    quotedAmountMsats: asFiniteNumber(terminal?.quotedAmountMsats),
    url: asString(output?.url) ?? asString(input?.url),
    method: asString(output?.method) ?? asString(input?.method),
    scope: asString(output?.scope) ?? asString(input?.scope),
    maxSpendMsats: asFiniteNumber(output?.maxSpendMsats) ?? asFiniteNumber(terminal?.maxSpendMsats) ?? asFiniteNumber(input?.maxSpendMsats),
  }
}

export const extractL402PaymentMetadata = (
  parts: ReadonlyArray<unknown>,
): ReadonlyArray<L402PaymentMetadata> => {
  const out: Array<L402PaymentMetadata> = []

  for (const p of parts) {
    if (isDseToolPart(p)) {
      const metadata = toL402PaymentMetadata({
        toolName: String(p.toolName),
        toolCallId: String(p.toolCallId),
        input: p.input,
        output: p.output,
        state: String(p.state ?? ""),
      })
      if (metadata) out.push(metadata)
      continue
    }

    if (isToolPart(p)) {
      const metadata = toL402PaymentMetadata({
        toolName: getToolPartName(p),
        toolCallId: String(p.toolCallId),
        input: p.input ?? p.rawInput,
        output: p.output,
        state: String(p.state ?? ""),
      })
      if (metadata) out.push(metadata)
    }
  }

  return out
}

export function toAutopilotRenderParts(input: {
  readonly parts: ReadonlyArray<UiPart>
  readonly toolContractsByName?: Record<string, ToolContract> | null
}): Array<RenderPart> {
  const out: Array<RenderPart> = []

  for (const p of input.parts) {
    if (isTextPart(p)) {
      if (p.text.length === 0) continue
      const prev = out.at(-1)
      if (prev?.kind === "text" && prev.state === p.state) {
        out[out.length - 1] = { kind: "text", text: prev.text + p.text, state: prev.state }
      } else {
        out.push({ kind: "text", text: p.text, state: asTextState(p.state) })
      }
      continue
    }

    if (isToolPart(p)) {
      const toolName = getToolPartName(p)
      const state = String(p.state ?? "")
      const rawInput = p.rawInput
      const toolInput = p.input ?? rawInput
      const paymentModel = paymentStateCardFromTool({
        toolName,
        toolCallId: String(p.toolCallId),
        state,
        input: toolInput,
        output: p.output,
        errorText: typeof p.errorText === "string" ? p.errorText : undefined,
      })
      if (paymentModel) out.push({ kind: "payment-state", model: paymentModel })

      const meta = input.toolContractsByName?.[toolName]
      const extra =
        meta?.usage || meta?.description
          ? html`
              <div data-effuse-tool-meta="1">
                ${meta.usage ? html`<div data-effuse-tool-usage="1">${meta.usage}</div>` : null}
                ${meta.description ? html`<div data-effuse-tool-description="1">${meta.description}</div>` : null}
              </div>
            `
          : null

      const model = toolModelFromCore({
        toolName,
        toolCallId: String(p.toolCallId),
        status: toToolStatus(state),
        summary: paymentModel ? l402ToolSummary(paymentModel.state, paymentModel) : state,
        input: toolInput,
        output: p.output,
        errorText: typeof p.errorText === "string" ? p.errorText : undefined,
        extra,
      })

      out.push({ kind: "tool", model })
      continue
    }

    if (isDseToolPart(p)) {
      const state = String(p.state ?? "")
      const duration = typeof p.timing?.durationMs === "number" ? ` (${p.timing.durationMs}ms)` : ""
      const paymentModel = paymentStateCardFromTool({
        toolName: String(p.toolName ?? "tool"),
        toolCallId: String(p.toolCallId ?? ""),
        state,
        input: p.input,
        output: p.output,
        errorText: typeof p.errorText === "string" ? p.errorText : undefined,
      })
      if (paymentModel) out.push({ kind: "payment-state", model: paymentModel })
      const summary = paymentModel ? l402ToolSummary(paymentModel.state, paymentModel) : `${state}${duration}`

      const model = toolModelFromCore({
        toolName: String(p.toolName ?? "tool"),
        toolCallId: String(p.toolCallId ?? ""),
        status: toToolStatusFromDseState(state),
        summary,
        input: p.input,
        output: p.output,
        errorText: typeof p.errorText === "string" ? p.errorText : undefined,
      })

      out.push({ kind: "tool", model })
      continue
    }

    if (isDseSignaturePart(p)) {
      const rlmTraceBlobId = asString(asRecord(asRecord(p.rlmTrace)?.blob)?.id)

      const previewText = p.outputPreview !== undefined ? safeStableStringify(p.outputPreview) : null
      const outputPreview = previewText
        ? Effect.runSync(
            boundText({
              text: previewText,
              maxChars: DSE_PREVIEW_MAX_CHARS,
              putText,
              mime: "application/json",
            }),
          )
        : undefined

      const errorTextRaw = typeof p.errorText === "string" ? p.errorText : null
      const errorText = errorTextRaw
        ? Effect.runSync(
            boundText({
              text: errorTextRaw,
              maxChars: DSE_PREVIEW_MAX_CHARS,
              putText,
              mime: "text/plain",
            }),
          )
        : undefined

      const contextPressureText = p.contextPressure !== undefined ? safeStableStringify(p.contextPressure) : null
      const contextPressure = contextPressureText
        ? Effect.runSync(
            boundText({
              text: contextPressureText,
              maxChars: DSE_PREVIEW_MAX_CHARS,
              putText,
              mime: "application/json",
            }),
          )
        : undefined

      const promptRenderStatsText = p.promptRenderStats !== undefined ? safeStableStringify(p.promptRenderStats) : null
      const promptRenderStats = promptRenderStatsText
        ? Effect.runSync(
            boundText({
              text: promptRenderStatsText,
              maxChars: DSE_PREVIEW_MAX_CHARS,
              putText,
              mime: "application/json",
            }),
          )
        : undefined

      const rlmTraceText = p.rlmTrace !== undefined ? safeStableStringify(p.rlmTrace) : null
      const rlmTrace = rlmTraceText
        ? Effect.runSync(
            boundText({
              text: rlmTraceText,
              maxChars: DSE_PREVIEW_MAX_CHARS,
              putText,
              mime: "application/json",
            }),
          )
        : undefined

      const model: DseSignatureCardModel = {
        id: p.id,
        state: p.state,
        signatureId: p.signatureId,
        compiled_id: p.compiled_id,
        receiptId: p.receiptId,
        modelId: asString(asRecord(p.model)?.modelId),
        provider: asString(asRecord(p.model)?.provider),
        modelRoute: asString(asRecord(p.model)?.route),
        modelFallbackId: asString(asRecord(p.model)?.fallbackModelId),
        strategyId: asString(p.strategyId),
        strategyReason: asString(p.strategyReason),
        durationMs: p.timing?.durationMs,
        budget: p.budget ? { limits: p.budget.limits, usage: p.budget.usage } : undefined,
        contextPressure,
        promptRenderStats,
        rlmTrace,
        rlmTraceBlobId,
        outputPreview,
        errorText,
      }

      out.push({ kind: "dse-signature", model })
      continue
    }

    if (isDseCompilePart(p)) {
      const errorTextRaw = typeof p.errorText === "string" ? p.errorText : null
      const errorText = errorTextRaw
        ? Effect.runSync(
            boundText({
              text: errorTextRaw,
              maxChars: DSE_PREVIEW_MAX_CHARS,
              putText,
              mime: "text/plain",
            }),
          )
        : undefined

      const model: DseCompileCardModel = {
        id: p.id,
        state: p.state,
        signatureId: p.signatureId,
        jobHash: p.jobHash,
        candidates: typeof p.candidates === "number" ? p.candidates : undefined,
        best: p.best ? { compiled_id: p.best.compiled_id, reward: p.best.reward } : undefined,
        reportId: typeof p.reportId === "string" ? p.reportId : undefined,
        errorText,
      }

      out.push({ kind: "dse-compile", model })
      continue
    }

    if (isDsePromotePart(p)) {
      const model: DsePromoteCardModel = {
        id: p.id,
        state: p.state,
        signatureId: p.signatureId,
        from: typeof p.from === "string" ? p.from : undefined,
        to: typeof p.to === "string" ? p.to : undefined,
        reason: typeof p.reason === "string" ? p.reason : undefined,
      }
      out.push({ kind: "dse-promote", model })
      continue
    }

    if (isDseRollbackPart(p)) {
      const model: DseRollbackCardModel = {
        id: p.id,
        state: p.state,
        signatureId: p.signatureId,
        from: typeof p.from === "string" ? p.from : undefined,
        to: typeof p.to === "string" ? p.to : undefined,
        reason: typeof p.reason === "string" ? p.reason : undefined,
      }
      out.push({ kind: "dse-rollback", model })
      continue
    }

    if (isDseBudgetExceededPart(p)) {
      const model: DseBudgetExceededCardModel = {
        id: p.id,
        state: p.state,
        message: typeof p.message === "string" ? p.message : undefined,
        budget: p.budget && typeof p.budget === "object" ? p.budget : undefined,
      }
      out.push({ kind: "dse-budget-exceeded", model })
      continue
    }
  }

  return out
}
