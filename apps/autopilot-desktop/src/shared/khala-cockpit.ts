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

// The Khala virtual model ids the cockpit can submit to. Mirrors the gateway
// catalog (`khala.md` §3). Kept as a closed union so the cockpit cannot submit
// to an unknown "khala-*" id by typo.
export const KHALA_MINI_MODEL_ID = "openagents/khala-mini" as const
export const KHALA_CODE_MODEL_ID = "openagents/khala-code" as const

export type KhalaCockpitModelId =
  | typeof KHALA_MINI_MODEL_ID
  | typeof KHALA_CODE_MODEL_ID

export const KHALA_COCKPIT_MODEL_IDS: ReadonlyArray<KhalaCockpitModelId> = [
  KHALA_MINI_MODEL_ID,
  KHALA_CODE_MODEL_ID,
]

export const isKhalaCockpitModelId = (
  value: string,
): value is KhalaCockpitModelId =>
  (KHALA_COCKPIT_MODEL_IDS as ReadonlyArray<string>).includes(value)

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
