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
  RenderPart,
} from "../../effuse-pages/autopilot"

type UiPart = ChatMessage["parts"][number]

function safeStableStringify(value: unknown, indent = 2): string {
  if (value == null) return String(value)
  if (typeof value === "string") return value
  try {
    return JSON.stringify(value, null, indent)
  } catch {
    return String(value)
  }
}

function isTextPart(part: unknown): part is { readonly type: "text"; readonly text: string; readonly state?: string } {
  return (
    Boolean(part) &&
    typeof part === "object" &&
    (part as any).type === "text" &&
    typeof (part as any).text === "string"
  )
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
  if (!part || typeof part !== "object") return false
  const type = (part as any).type
  if (type === "dynamic-tool") {
    return (
      typeof (part as any).toolName === "string" &&
      typeof (part as any).toolCallId === "string" &&
      typeof (part as any).state === "string"
    )
  }
  return (
    typeof type === "string" &&
    type.startsWith("tool-") &&
    typeof (part as any).toolCallId === "string" &&
    typeof (part as any).state === "string"
  )
}

function getToolPartName(part: { type: string; toolName?: string }): string {
  if (part.type === "dynamic-tool") return String(part.toolName ?? "tool")
  if (part.type.startsWith("tool-")) return part.type.slice("tool-".length)
  return part.type
}

function isDseSignaturePart(part: unknown): part is ChatDseSignaturePart {
  return (
    Boolean(part) &&
    typeof part === "object" &&
    (part as any).type === "dse.signature" &&
    typeof (part as any).id === "string" &&
    typeof (part as any).signatureId === "string" &&
    typeof (part as any).state === "string"
  )
}

function isDseToolPart(part: unknown): part is ChatDseToolPart {
  return (
    Boolean(part) &&
    typeof part === "object" &&
    (part as any).type === "dse.tool" &&
    typeof (part as any).id === "string" &&
    typeof (part as any).toolName === "string" &&
    typeof (part as any).toolCallId === "string" &&
    typeof (part as any).state === "string"
  )
}

function isDseCompilePart(part: unknown): part is ChatDseCompilePart {
  return (
    Boolean(part) &&
    typeof part === "object" &&
    (part as any).type === "dse.compile" &&
    typeof (part as any).id === "string" &&
    typeof (part as any).signatureId === "string" &&
    typeof (part as any).jobHash === "string" &&
    typeof (part as any).state === "string"
  )
}

function isDsePromotePart(part: unknown): part is ChatDsePromotePart {
  return (
    Boolean(part) &&
    typeof part === "object" &&
    (part as any).type === "dse.promote" &&
    typeof (part as any).id === "string" &&
    typeof (part as any).signatureId === "string" &&
    typeof (part as any).state === "string"
  )
}

function isDseRollbackPart(part: unknown): part is ChatDseRollbackPart {
  return (
    Boolean(part) &&
    typeof part === "object" &&
    (part as any).type === "dse.rollback" &&
    typeof (part as any).id === "string" &&
    typeof (part as any).signatureId === "string" &&
    typeof (part as any).state === "string"
  )
}

function isDseBudgetExceededPart(part: unknown): part is ChatDseBudgetExceededPart {
  return (
    Boolean(part) &&
    typeof part === "object" &&
    (part as any).type === "dse.budget_exceeded" &&
    typeof (part as any).id === "string" &&
    typeof (part as any).state === "string"
  )
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
  readonly extra?: unknown
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
      extra: opts.extra ? (opts.extra as any) : undefined,
      input,
      output,
      error,
    },
  }
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
        out.push({ kind: "text", text: p.text, state: p.state as any })
      }
      continue
    }

    if (isToolPart(p)) {
      const toolName = getToolPartName(p as any)
      const state = String((p as any).state ?? "")
      const rawInput = (p as any).rawInput
      const toolInput = (p as any).input ?? rawInput

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
        toolCallId: String((p as any).toolCallId),
        status: toToolStatus(state),
        summary: state,
        input: toolInput,
        output: (p as any).output,
        errorText: typeof (p as any).errorText === "string" ? (p as any).errorText : undefined,
        extra,
      })

      out.push({ kind: "tool", model })
      continue
    }

    if (isDseToolPart(p)) {
      const state = String(p.state ?? "")
      const duration = typeof p.timing?.durationMs === "number" ? ` (${p.timing.durationMs}ms)` : ""
      const summary = `${state}${duration}`

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

      const model: DseSignatureCardModel = {
        id: p.id,
        state: p.state,
        signatureId: p.signatureId,
        compiled_id: p.compiled_id,
        receiptId: p.receiptId,
        durationMs: p.timing?.durationMs,
        budget: p.budget ? { limits: p.budget.limits, usage: p.budget.usage } : undefined,
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
        message: typeof (p as any).message === "string" ? String((p as any).message) : undefined,
        budget: (p as any).budget && typeof (p as any).budget === "object" ? (p as any).budget : undefined,
      }
      out.push({ kind: "dse-budget-exceeded", model })
      continue
    }
  }

  return out
}
