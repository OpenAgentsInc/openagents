// Khala cockpit shared projection (M1, #6009, EPIC #6017).
//
// Lane A — Cockpit. This module is the single PURE, DOM-free home for the
// Autopilot cockpit's Khala call path: which `openagents/khala-*` model ids the
// cockpit may submit to, and how to parse the NON-BREAKING `openagents`
// disclosure block off an OpenAI-compatible chat-completions response into a
// public-safe projection the webview can render.
//
// CONSUME-ONLY: the `openagents` block shape is OWNED by the gateway
// (`apps/openagents.com/.../inference/chat-completions-routes.ts`,
// `OpenAgentsReceipt`). This module only READS it; it never defines or mutates
// that shape. If the cockpit needs a new field, that is a gateway PR — see the
// roadmap "Lane A" collision rule.
//
// LIVE-GATING: a cockpit "live" indicator is a product claim. It is TRUE only
// when the response carries a real receipt ref in the `openagents` block. A
// completion with no receipt (e.g. a stub/free route) renders as a real answer
// but an UNVERIFIED / not-live badge. Never claim "live" off a missing receipt.

// The single public Khala virtual model id the cockpit submits to. Public model
// selection intentionally collapses to ONE id: the gateway model-router/catalog
// (`apps/openagents.com/.../inference/pricing.ts` `KHALA_MODEL_ID`) only accepts
// `openagents/khala` (alias `khala`); the old `openagents/khala-mini` /
// `openagents/khala-code` split ids are deprecated/removed and the gateway
// rejects them with `model_unavailable` (the desktop "Talk to Khala" bug). Mini/
// pro/code/pylon names are legacy internal implementation details, never public
// choices.
export const KHALA_MODEL_ID = "openagents/khala" as const

export type KhalaCockpitModelId = typeof KHALA_MODEL_ID

export const KHALA_COCKPIT_MODEL_IDS: ReadonlyArray<KhalaCockpitModelId> = [
  KHALA_MODEL_ID,
]

// The deprecated split ids the gateway no longer serves. Accepted by the guard
// only so a stale stored/forwarded slug NORMALIZES to the single public id
// instead of re-triggering `model_unavailable`; they are never re-emitted.
const DEPRECATED_KHALA_MODEL_IDS: ReadonlyArray<string> = [
  "openagents/khala-mini",
  "openagents/khala-code",
  "openagents/khala-pro",
]

// Normalize any incoming Khala-family slug to the single public id. A blank or
// unknown value falls back to the public id so the cockpit never submits an
// unservable slug.
export const normalizeKhalaCockpitModelId = (
  value: string | undefined,
): KhalaCockpitModelId => KHALA_MODEL_ID

export const isKhalaCockpitModelId = (
  value: string,
): value is KhalaCockpitModelId =>
  value === KHALA_MODEL_ID || DEPRECATED_KHALA_MODEL_IDS.includes(value)

// The public-safe projection of the gateway `openagents` block for the cockpit.
// This is what crosses the RPC boundary to the webview and what the receipt UI
// renders. It carries ONLY the disclosed routing/verification fields; no
// prompts, credentials, or chain-of-thought.
export type KhalaReceiptProjection = Readonly<{
  // The model the cockpit requested (e.g. `openagents/khala-code`).
  requestedModel: string
  // The concrete model that actually served the request (e.g. a provider id).
  servedModel: string
  // The worker/adapter that served it.
  worker: string
  // The route lane the coordinator picked (e.g. `coding`, `cheap`, `default`).
  lane: string
  // The verification class outcome.
  verification: "none" | "test_passed" | "failed"
  // Whether the run is verified (only meaningful for verified lanes).
  verified: boolean | null
  // The dereferenceable receipt ref, when present. Its presence is the LIVE gate.
  receipt: string | null
  // A relative URL to the public receipt, when the gateway metered the run.
  receiptUrl: string | null
  // For `khala-code`: the rubric checks that passed / failed.
  rubric:
    | Readonly<{
        ref: string
        passedChecks: ReadonlyArray<string>
        failedChecks: ReadonlyArray<string>
      }>
    | null
}>

// A cockpit turn result: the rendered answer text plus the receipt projection.
export type KhalaTurnResult = Readonly<{
  ok: boolean
  // The assistant answer (or an honest in-conversation error message). Never a
  // faked answer.
  text: string
  // The receipt projection, when the response carried an `openagents` block.
  receipt: KhalaReceiptProjection | null
  // The LIVE gate: true ONLY when `receipt.receipt` is a non-empty ref. A real
  // completion with no receipt is `live: false` (rendered as unverified).
  live: boolean
}>

const asString = (value: unknown): string | null =>
  typeof value === "string" && value.trim().length > 0 ? value.trim() : null

const asStringArray = (value: unknown): ReadonlyArray<string> => {
  if (!Array.isArray(value)) return []
  return value.filter((v): v is string => typeof v === "string")
}

const asVerification = (
  value: unknown,
): "none" | "test_passed" | "failed" => {
  if (value === "test_passed" || value === "failed") return value
  return "none"
}

// Parse the gateway `openagents` block off a chat-completions body into the
// cockpit projection. Tolerant: an absent or malformed block yields `null`
// (the cockpit then renders the answer as not-live), never a thrown error.
export const parseKhalaReceipt = (
  body: unknown,
): KhalaReceiptProjection | null => {
  if (typeof body !== "object" || body === null) return null
  const block = (body as { openagents?: unknown }).openagents
  if (typeof block !== "object" || block === null) return null
  const record = block as Record<string, unknown>

  const requestedModel = asString(record.requested_model)
  const servedModel = asString(record.served_model)
  const worker = asString(record.worker)
  const lane = asString(record.lane)
  // The minimum disclosure for a real Khala route. A block missing all of these
  // is not a recognizable Khala receipt.
  if (requestedModel === null && servedModel === null && worker === null) {
    return null
  }

  const rubricRaw = record.rubric
  const rubric =
    typeof rubricRaw === "object" && rubricRaw !== null
      ? (() => {
          const r = rubricRaw as Record<string, unknown>
          const ref = asString(r.ref)
          if (ref === null) return null
          return {
            ref,
            passedChecks: asStringArray(r.passed_checks),
            failedChecks: asStringArray(r.failed_checks),
          }
        })()
      : null

  return {
    requestedModel: requestedModel ?? "",
    servedModel: servedModel ?? "",
    worker: worker ?? "",
    lane: lane ?? "default",
    verification: asVerification(record.verification),
    verified: typeof record.verified === "boolean" ? record.verified : null,
    receipt: asString(record.receipt),
    receiptUrl: asString(record.receipt_url),
    rubric,
  }
}

// The LIVE gate. A cockpit run is "live" ONLY when the response carried a real,
// non-empty receipt ref. Everything else (no block, no receipt) is not-live.
export const isLiveReceipt = (
  receipt: KhalaReceiptProjection | null,
): boolean => receipt !== null && receipt.receipt !== null

// The non-streaming body shape both the streaming and non-streaming paths
// produce, so `parseAssistantText` / `parseKhalaReceipt` consume one shape.
export type ReconstructedCompletion = Readonly<{
  choices: ReadonlyArray<{
    readonly index: number
    readonly finish_reason: string
    readonly message: { readonly role: "assistant"; readonly content: string }
  }>
  usage?: unknown
  // The terminal `openagents` receipt/verification block (the gateway emits it
  // on the FINAL stream chunk, after verification runs on the full output).
  openagents?: unknown
}>

// Pull the content delta off one parsed `chat.completion.chunk` frame.
const sseChunkDelta = (parsed: unknown): string | null => {
  if (typeof parsed !== "object" || parsed === null) return null
  const choices = (parsed as { choices?: unknown }).choices
  if (!Array.isArray(choices) || choices.length === 0) return null
  const delta = (choices[0] as { delta?: { content?: unknown } } | undefined)
    ?.delta?.content
  return typeof delta === "string" && delta.length > 0 ? delta : null
}

// Reconstruct a non-streaming completion body from the gateway's SSE chunk
// stream. Each frame is a `data: {...,object:"chat.completion.chunk"}` line; the
// terminal `openagents` block rides on the FINAL chunk before `data: [DONE]`.
// This is the cockpit-side mirror of the gateway's SSE framing — CONSUME-ONLY.
// `onToken` is an optional live-render hook (fired per content delta).
export const reconstructKhalaCompletionFromSse = (
  rawText: string,
  onToken?: (delta: string) => void,
): ReconstructedCompletion => {
  let content = ""
  let finishReason = "stop"
  let usage: unknown
  let openagents: unknown
  for (const frame of rawText.split("\n\n")) {
    for (const line of frame.split("\n")) {
      if (!line.startsWith("data:")) continue
      const payload = line.slice(line.indexOf(":") + 1).trim()
      if (payload === "" || payload === "[DONE]") continue
      let parsed: unknown
      try {
        parsed = JSON.parse(payload)
      } catch {
        continue
      }
      const delta = sseChunkDelta(parsed)
      if (delta !== null) {
        content += delta
        onToken?.(delta)
      }
      const record = parsed as {
        choices?: Array<{ finish_reason?: unknown }>
        usage?: unknown
        openagents?: unknown
      }
      const fr = record.choices?.[0]?.finish_reason
      if (typeof fr === "string") finishReason = fr
      if (record.usage !== undefined && record.usage !== null) {
        usage = record.usage
      }
      if (record.openagents !== undefined && record.openagents !== null) {
        openagents = record.openagents
      }
    }
  }
  return {
    choices: [
      {
        index: 0,
        finish_reason: finishReason,
        message: { role: "assistant", content },
      },
    ],
    ...(usage === undefined ? {} : { usage }),
    ...(openagents === undefined ? {} : { openagents }),
  }
}

// Heuristic: does this response carry an SSE event-stream body? The gateway sets
// `content-type: text/event-stream` for `stream:true`. Tolerant of charset
// suffixes; falls back to false (treat as JSON) when the header is absent.
export const isEventStreamResponse = (contentType: string | null): boolean =>
  contentType !== null && contentType.toLowerCase().includes("text/event-stream")

// A short, public-safe one-line summary of the receipt for the cockpit HUD.
// Jargon-free where possible; carries only disclosed fields.
export const summarizeKhalaReceipt = (
  receipt: KhalaReceiptProjection | null,
): string => {
  if (receipt === null) return "No receipt — answer is not verified."
  const verifiedLabel =
    receipt.verification === "test_passed"
      ? "verified (tests passed)"
      : receipt.verification === "failed"
        ? "verification failed"
        : "not verified"
  const served =
    receipt.servedModel.length > 0 ? receipt.servedModel : "unknown model"
  const liveLabel = isLiveReceipt(receipt) ? "live" : "no receipt"
  return `${served} via ${receipt.lane} lane — ${verifiedLabel} — ${liveLabel}`
}
