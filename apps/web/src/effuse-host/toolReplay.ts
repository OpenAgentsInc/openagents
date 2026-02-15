/**
 * Tool replay helpers.
 *
 * We persist tool calls/results as Convex `messageParts` rows (e.g. `type: "dse.tool"`).
 * The model prompt historically only included `messages.text`, which makes the model "forget"
 * tool outcomes if they weren't rendered into durable text.
 *
 * This module extracts a bounded, redacted summary of recent tool activity so we can safely
 * inject it into the model context.
 */

export type ToolReplayPartRow = {
  readonly messageId: string
  readonly runId: string
  readonly seq: number
  readonly part: unknown
}

type RecordLike = Record<string, any>

const isRecord = (value: unknown): value is RecordLike =>
  Boolean(value) && typeof value === "object" && !Array.isArray(value)


const readNonEmptyString = (value: unknown): string | null =>
  typeof value === "string" && value.trim().length > 0 ? value.trim() : null

const readFiniteNumber = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null

type DseToolPart = {
  readonly type: "dse.tool"
  readonly toolName?: string
  readonly toolCallId?: string
  readonly state?: string
  readonly input?: unknown
  readonly output?: unknown
  readonly errorText?: string
}

const isDseToolPart = (value: unknown): value is DseToolPart =>
  isRecord(value) && value.type === "dse.tool" && typeof value.toolName === "string"

const clampText = (value: string, maxChars: number): string => {
  const t = value.trim()
  if (t.length <= maxChars) return t
  const slice = t.slice(0, Math.max(0, maxChars))
  const cut = slice.lastIndexOf(" ")
  const bounded = cut >= Math.floor(maxChars * 0.6) ? slice.slice(0, cut) : slice
  return `${bounded.trim()}...`
}

const redactSensitive = (value: unknown, depth: number): unknown => {
  if (depth <= 0) return "[TRUNCATED]"
  if (value == null) return value
  if (typeof value === "string") {
    // Avoid leaking huge payloads into prompt context.
    return value.length > 2_000 ? clampText(value, 2_000) : value
  }
  if (typeof value === "number" || typeof value === "boolean") return value
  if (Array.isArray(value)) {
    const arr = value.slice(0, 20).map((v) => redactSensitive(v, depth - 1))
    return value.length > 20 ? [...arr, `...(${value.length - 20} more)`] : arr
  }
  if (isRecord(value)) {
    const out: RecordLike = {}
    const keys = Object.keys(value)
    for (const key of keys.slice(0, 80)) {
      if (
        /^(invoice|macaroon|preimage|authorization|cookie|token|secret|password|passphrase|seed|api[_-]?key)$/i.test(
          key,
        )
      ) {
        out[key] = "[REDACTED]"
        continue
      }
      out[key] = redactSensitive(value[key], depth - 1)
    }
    if (keys.length > 80) out["__truncated_keys"] = keys.length - 80
    return out
  }
  return String(value)
}

const stableJson = (value: unknown, maxChars: number): string => {
  try {
    const redacted = redactSensitive(value, 5)
    const json = JSON.stringify(redacted)
    return json.length > maxChars ? clampText(json, maxChars) : json
  } catch {
    const fallback = String(value)
    return fallback.length > maxChars ? clampText(fallback, maxChars) : fallback
  }
}

const fmtSatsFromMsats = (msats: number): string => {
  const sats = msats / 1000
  const rounded = Math.round(sats * 1000) / 1000
  return `${Number.isInteger(rounded) ? String(rounded) : String(rounded)} sats`
}

const summarizeLightningTerminal = (value: unknown): string | null => {
  const rec = isRecord(value) ? value : null
  if (!rec) return null

  const status = readNonEmptyString(rec.status)
  if (!status) return null

  const host = readNonEmptyString(rec.host)
  const taskId = readNonEmptyString(rec.taskId)
  const proofReference = readNonEmptyString(rec.proofReference)

  const maxSpendMsats = readFiniteNumber(rec.maxSpendMsats)
  const quotedAmountMsats = readFiniteNumber(rec.quotedAmountMsats)
  const amountMsats = readFiniteNumber(rec.amountMsats)

  const responseStatusCode = readFiniteNumber(rec.responseStatusCode)
  const responseContentType = readNonEmptyString(rec.responseContentType)
  const responseBytes = readFiniteNumber(rec.responseBytes)
  const responseBodyTextPreview = readNonEmptyString(rec.responseBodyTextPreview)
  const responseBodySha256 = readNonEmptyString(rec.responseBodySha256)

  const paid = rec.paid === true
  const cacheHit = rec.cacheHit === true
  const cacheStatus = readNonEmptyString(rec.cacheStatus)
  const paymentBackend = readNonEmptyString(rec.paymentBackend)

  const denyReasonCode = readNonEmptyString(rec.denyReasonCode)
  const denyReason = readNonEmptyString(rec.denyReason)

  const pieces: string[] = [`status=${status}`]
  if (taskId) pieces.push(`taskId=${taskId}`)
  if (host) pieces.push(`host=${host}`)

  if (typeof responseStatusCode === "number") pieces.push(`http=${Math.floor(responseStatusCode)}`)
  if (responseContentType) pieces.push(`contentType=${responseContentType}`)
  if (typeof responseBytes === "number") pieces.push(`bytes=${Math.floor(responseBytes)}`)

  if (typeof maxSpendMsats === "number") pieces.push(`cap=${fmtSatsFromMsats(maxSpendMsats)}`)
  if (typeof quotedAmountMsats === "number") pieces.push(`quoted=${fmtSatsFromMsats(quotedAmountMsats)}`)
  if (typeof amountMsats === "number") pieces.push(`amount=${fmtSatsFromMsats(amountMsats)}`)

  if (paid) pieces.push(`paid=true`)
  if (cacheHit) pieces.push(`cacheHit=true`)
  if (cacheStatus) pieces.push(`cacheStatus=${cacheStatus}`)
  if (paymentBackend) pieces.push(`backend=${paymentBackend}`)

  if (proofReference) pieces.push(`proof=${proofReference}`)

  if (denyReasonCode) pieces.push(`denyCode=${denyReasonCode}`)
  if (denyReason) pieces.push(`denyReason=${clampText(denyReason, 120)}`)

  if (responseBodySha256) pieces.push(`bodySha256=${responseBodySha256}`)
  if (responseBodyTextPreview) pieces.push(`bodyPreview=${JSON.stringify(clampText(responseBodyTextPreview, 360))}`)

  return pieces.join(" ")
}

const summarizeLightningApproveOutput = (value: unknown): string | null => {
  const rec = isRecord(value) ? value : null
  if (!rec) return null

  const ok = rec.ok === true
  const taskId = readNonEmptyString(rec.taskId)
  const taskStatus = readNonEmptyString(rec.taskStatus)
  const denyReason = readNonEmptyString(rec.denyReason)
  const url = readNonEmptyString(rec.url)
  const method = readNonEmptyString(rec.method)
  const scope = readNonEmptyString(rec.scope)
  const maxSpendMsats = readFiniteNumber(rec.maxSpendMsats)

  const pieces: string[] = [`ok=${ok}`]
  if (taskId) pieces.push(`taskId=${taskId}`)
  if (taskStatus) pieces.push(`taskStatus=${taskStatus}`)
  if (url) pieces.push(`url=${url}`)
  if (method) pieces.push(`method=${method}`)
  if (scope) pieces.push(`scope=${scope}`)
  if (typeof maxSpendMsats === "number") pieces.push(`cap=${fmtSatsFromMsats(maxSpendMsats)}`)
  if (denyReason) pieces.push(`denyReason=${clampText(denyReason, 120)}`)

  const terminal = rec.terminal
  const terminalSummary = summarizeLightningTerminal(terminal)
  if (terminalSummary) pieces.push(`terminal{${terminalSummary}}`)

  return pieces.join(" ")
}

const summarizeToolPart = (part: DseToolPart): string => {
  const toolName = String(part.toolName ?? "")
  const state = readNonEmptyString(part.state) ?? "unknown"

  const base = [`tool=${toolName}`, `state=${state}`]

  // Prefer tool-specific summaries.
  if (toolName === "lightning_l402_fetch") {
    const summary = summarizeLightningTerminal(part.output)
    if (summary) return [...base, summary].join(" ")
  }

  if (toolName === "lightning_l402_approve") {
    const summary = summarizeLightningApproveOutput(part.output)
    if (summary) return [...base, summary].join(" ")
  }

  // Generic fallback: emit a bounded redacted JSON snippet.
  if (part.errorText) base.push(`error=${clampText(String(part.errorText), 180)}`)
  if (part.output != null) base.push(`output=${stableJson(part.output, 600)}`)
  else if (part.input != null) base.push(`input=${stableJson(part.input, 600)}`)

  return base.join(" ")
}

export const renderToolReplaySystemContext = (
  rows: ReadonlyArray<ToolReplayPartRow>,
  options?: { readonly maxEvents?: number; readonly maxOutputChars?: number },
): string => {
  const maxEvents =
    typeof options?.maxEvents === "number" && Number.isFinite(options.maxEvents)
      ? Math.max(0, Math.floor(options.maxEvents))
      : 12
  const maxOutputChars =
    typeof options?.maxOutputChars === "number" && Number.isFinite(options.maxOutputChars)
      ? Math.max(200, Math.floor(options.maxOutputChars))
      : 6_000

  const toolParts = rows
    .map((r) => ({ ...r, tool: isDseToolPart(r.part) ? (r.part as DseToolPart) : null }))
    .filter((r): r is ToolReplayPartRow & { tool: DseToolPart } => Boolean(r.tool))
    .filter((r) => String(r.tool.state ?? "") !== "start")

  if (toolParts.length === 0) return ""

  const tail = toolParts.slice(-maxEvents)

  const lines = tail.map((r) => {
    const summary = summarizeToolPart(r.tool)
    return `- runId=${r.runId} msgId=${r.messageId} seq=${r.seq} ${summary}`
  })

  const body = lines.join("\n")
  const bounded = body.length > maxOutputChars ? clampText(body, maxOutputChars) : body

  return `Tool replay (recent tool calls/results, redacted):\n${bounded}`
}
