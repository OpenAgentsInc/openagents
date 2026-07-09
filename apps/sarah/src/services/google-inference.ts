/**
 * Inference client for Sarah's text turns (#8594 flip-to-live, #8600 KHS-1).
 *
 * TWO TRANSPORTS, ONE CONTRACT:
 *
 * 1. KHALA GATEWAY (KHS-1, flag-gated, default OFF). When
 *    `SARAH_INFERENCE_GATEWAY_URL` + `SARAH_INFERENCE_GATEWAY_TOKEN` are set,
 *    turns go through the OpenAI-compatible Khala gateway
 *    (`POST {url}/chat/completions`, e.g. `https://openagents.com/api/v1`)
 *    with the agent bearer token. The requested model is the public Khala
 *    alias (`openagents/khala`): its conversational adapter plan LEADS with
 *    the same Gemma 4 gcloud lane Sarah uses today
 *    (`model-router.ts` KHALA_CONVERSATIONAL_ADAPTER_PLAN, owner decision
 *    2026-07-09) and overflows to Vertex Gemini / Fireworks / GLM instead of
 *    Sarah hand-rolling a 429 model chain. Every turn lands an exact
 *    `token_usage_events` row; the request self-attributes as internal demand
 *    via `x-openagents-demand-kind: internal` / `x-openagents-demand-source:
 *    sarah` (the same header rail heartbeat/canary use — see
 *    `chat-completions-routes.ts` `requestAttributionFromHeaders` and
 *    `inference-internal-account.ts`). NEVER the org-cloud no-meter header:
 *    Sarah's demand is metered own/internal usage with receipts, not a bypass.
 *
 * 2. DIRECT GOOGLE (legacy fallback while the gateway env is absent). Talks
 *    to the Generative Language API (project openagentsgemini) with a Gemma 4
 *    model — owner direction 2026-07-09: Gemma 4, not Gemini. Catalog slugs
 *    verified live: `gemma-4-31b-it` (dense, default) and
 *    `gemma-4-26b-a4b-it` (MoE).
 *
 * Gemma 4 is a thinking model: candidates carry scratchpad parts flagged
 * `thought: true` before the answer parts. Thought parts are filtered out
 * and never stored, surfaced, or echoed into transcripts or receipts. On the
 * gateway transport the Gemma 4 adapter already strips thoughts from
 * `content` and routes them to the separate `reasoning_content` delta
 * channel; this client reads ONLY `delta.content` / `message.content`, so
 * scratchpad text can never surface on either transport.
 * Gemma exposes only generateContent/countTokens — no native tool calling —
 * so the text path keeps its deterministic pricing guard upstream and the
 * tool loop stays on the realtime/voice path until a tools-capable lane.
 * (The gateway router also keeps tool-bearing requests off the Gemma lane;
 * Sarah's text requests carry no tools, so they stay on the Gemma-led plan.)
 */

export const SARAH_TEXT_MODEL_DEFAULT = "gemma-4-31b-it"

/** Public Khala alias whose conversational plan leads with the Gemma 4 lane. */
export const SARAH_GATEWAY_MODEL_DEFAULT = "openagents/khala"

const BASE_URL_DEFAULT = "https://generativelanguage.googleapis.com/v1beta"

/**
 * Demand attribution for exact receipts (#8600). These match the gateway's
 * bounded attribution tokens (`safeAttributionToken` /
 * `demandKindFromHeader`), so Sarah's `token_usage_events` rows carry
 * `demand_kind=internal`, `demand_source=sarah` — distinguishable internal
 * dogfood, same rail as heartbeat/canary. The account-ref backstop
 * (`INFERENCE_INTERNAL_ACCOUNT_REFS`) should ALSO list Sarah's agent account
 * so header loss can never leak her turns into the external corpus.
 */
export const SARAH_GATEWAY_DEMAND_HEADERS: Readonly<Record<string, string>> = {
  "x-openagents-demand-kind": "internal",
  "x-openagents-demand-source": "sarah",
  "x-openagents-client": "sarah-server",
}

export type GemmaContent = {
  role: "user" | "model"
  parts: Array<{ text: string }>
}

type GemmaPart = { text?: string; thought?: boolean }

type FetchLike = (input: string, init: RequestInit) => Promise<Response>

export function sarahGoogleInferenceArmed(): boolean {
  return Boolean(process.env.GEMINI_API_KEY?.trim())
}

/** KHS-1 gateway transport is armed only when BOTH env vars are set. */
export function sarahInferenceGatewayArmed(): boolean {
  return Boolean(
    process.env.SARAH_INFERENCE_GATEWAY_URL?.trim() &&
      process.env.SARAH_INFERENCE_GATEWAY_TOKEN?.trim(),
  )
}

/** Whether ANY inference transport is armed (gateway preferred). */
export function sarahInferenceArmed(): boolean {
  return sarahInferenceGatewayArmed() || sarahGoogleInferenceArmed()
}

export type SarahInferenceTransport =
  | "khala_gateway"
  | "google_direct"
  | "not_armed"

/** Which transport a call will take right now (gateway wins when armed). */
export function sarahInferenceTransport(): SarahInferenceTransport {
  if (sarahInferenceGatewayArmed()) return "khala_gateway"
  if (sarahGoogleInferenceArmed()) return "google_direct"
  return "not_armed"
}

export function sarahTextModel(): string {
  return process.env.SARAH_TEXT_MODEL?.trim() || SARAH_TEXT_MODEL_DEFAULT
}

export function sarahGatewayModel(): string {
  return (
    process.env.SARAH_INFERENCE_GATEWAY_MODEL?.trim() ||
    SARAH_GATEWAY_MODEL_DEFAULT
  )
}

/** The model id the active transport will request (for status surfaces). */
export function sarahActiveModelId(): string {
  return sarahInferenceGatewayArmed() ? sarahGatewayModel() : sarahTextModel()
}

/**
 * Each Gemma model has its OWN per-minute quota bucket on the key, so
 * falling back to the MoE variant on 429 roughly doubles burst capacity —
 * the 2026-07-09 owner session showed LiveAvatar firing one inference call
 * per VAD speech fragment, exhausting a single model's RPM in under a
 * minute. The Khala gateway migration (KHS-1) is the durable fix.
 */
export function sarahTextModelChain(): string[] {
  const fallbacks = (process.env.SARAH_TEXT_MODEL_FALLBACKS ?? "gemma-4-26b-a4b-it")
    .split(",")
    .map((model) => model.trim())
    .filter(Boolean)
  return [sarahTextModel(), ...fallbacks.filter((model) => model !== sarahTextModel())]
}

/** Concatenate answer text, dropping `thought: true` scratchpad parts. */
export function extractGemmaReply(parts: ReadonlyArray<GemmaPart>): string {
  return parts
    .filter((part) => part.thought !== true && typeof part.text === "string")
    .map((part) => part.text)
    .join("")
    .trim()
}

export type GemmaTurnUsage = {
  promptTokens: number
  outputTokens: number
  thoughtTokens: number
  totalTokens: number
}

// ---------------------------------------------------------------------------
// Per-day token cost cap (#8600 deliverable 3).
//
// A cheap, process-local guard on Sarah's inference lane: when
// `SARAH_TEXT_DAILY_TOKEN_CAP` is set (> 0), calls refuse with the typed error
// `sarah_daily_token_cap_exceeded` once the UTC day's PROVIDER-REPORTED total
// tokens reach the cap. EXACT-ONLY accounting: the counter accumulates only
// usage totals the provider/gateway actually reported — never an estimate.
// Callers map the refusal to the canned busy reply (isSarahInferenceBusyError).
// Best-effort by design (in-memory, per-process — Sarah is a single Bun
// server); the authoritative ledger is the gateway's `token_usage_events`.
// Unset/invalid cap => the guard is a pure no-op.
// ---------------------------------------------------------------------------

export const SARAH_DAILY_TOKEN_CAP_ERROR = "sarah_daily_token_cap_exceeded"

let dailyTokenUsage = { day: "", totalTokens: 0 }

function utcDay(): string {
  return new Date().toISOString().slice(0, 10)
}

export function sarahDailyTokenCap(): number | null {
  const raw = Number(process.env.SARAH_TEXT_DAILY_TOKEN_CAP ?? NaN)
  return Number.isFinite(raw) && raw > 0 ? Math.floor(raw) : null
}

export function sarahDailyTokenUsage(): { day: string; totalTokens: number } {
  const day = utcDay()
  return dailyTokenUsage.day === day
    ? { ...dailyTokenUsage }
    : { day, totalTokens: 0 }
}

export function resetSarahDailyTokenUsageForTests(): void {
  dailyTokenUsage = { day: "", totalTokens: 0 }
}

function dailyTokenCapExceeded(): boolean {
  const cap = sarahDailyTokenCap()
  if (cap === null) return false
  return sarahDailyTokenUsage().totalTokens >= cap
}

/** Record EXACT provider-reported totals only. Zero/negative => no-op. */
function recordDailyTokenUsage(totalTokens: number): void {
  if (!Number.isFinite(totalTokens) || totalTokens <= 0) return
  const day = utcDay()
  if (dailyTokenUsage.day !== day) {
    dailyTokenUsage = { day, totalTokens: 0 }
  }
  dailyTokenUsage.totalTokens += totalTokens
}

/**
 * Whether a typed inference error should surface the canned "I'm handling a
 * lot of conversations" busy reply (vs the generic trouble-reaching-model
 * reply): provider/gateway rate limits and the daily cost-cap refusal.
 */
export function isSarahInferenceBusyError(error: string): boolean {
  return error.endsWith("_http_429") || error === SARAH_DAILY_TOKEN_CAP_ERROR
}

// ---------------------------------------------------------------------------
// Khala gateway transport (KHS-1 #8600) — OpenAI-compatible chat/completions.
// ---------------------------------------------------------------------------

type GatewayConfig = {
  url: string
  token: string
  model: string
  maxOutputTokens: number
  timeoutMs: number
}

function gatewayConfig(): GatewayConfig | null {
  const url = process.env.SARAH_INFERENCE_GATEWAY_URL?.trim()
  const token = process.env.SARAH_INFERENCE_GATEWAY_TOKEN?.trim()
  if (!url || !token) return null
  return {
    url: url.replace(/\/+$/, ""),
    token,
    model: sarahGatewayModel(),
    maxOutputTokens: Number(process.env.SARAH_TEXT_MAX_OUTPUT_TOKENS ?? 2048),
    timeoutMs: Number(process.env.SARAH_TEXT_TIMEOUT_MS ?? 45_000),
  }
}

/** Map Sarah's Gemma-shaped contents to OpenAI-compatible messages. */
export function toGatewayMessages(
  system: string,
  contents: ReadonlyArray<GemmaContent>,
): Array<{ role: "system" | "user" | "assistant"; content: string }> {
  return [
    { role: "system" as const, content: system },
    ...contents.map((content) => ({
      role:
        content.role === "model" ? ("assistant" as const) : ("user" as const),
      content: content.parts.map((part) => part.text).join(""),
    })),
  ]
}

function gatewayRequestInit(
  config: GatewayConfig,
  body: unknown,
  signal: AbortSignal,
): RequestInit {
  return {
    method: "POST",
    headers: {
      "content-type": "application/json",
      authorization: `Bearer ${config.token}`,
      ...SARAH_GATEWAY_DEMAND_HEADERS,
    },
    signal,
    body: JSON.stringify(body),
  }
}

type GatewayUsageWire = {
  prompt_tokens?: number
  completion_tokens?: number
  total_tokens?: number
}

/**
 * Map the gateway's OpenAI usage block to Sarah's turn usage. The gateway
 * records the provider's authoritative total (which for thinking lanes
 * INCLUDES scratchpad tokens) receipt-first; the visible completion excludes
 * them. `thoughtTokens` is therefore the exact reconciliation gap
 * `total - (prompt + completion)` — the same `unaccountedTokens` derivation
 * the gateway's own telemetry discloses (khala-telemetry.ts) — never a guess.
 */
export function gatewayUsageToTurnUsage(
  usage: GatewayUsageWire | undefined,
): GemmaTurnUsage {
  const promptTokens = usage?.prompt_tokens ?? 0
  const outputTokens = usage?.completion_tokens ?? 0
  const totalTokens = usage?.total_tokens ?? 0
  return {
    promptTokens,
    outputTokens,
    thoughtTokens: Math.max(0, totalTokens - promptTokens - outputTokens),
    totalTokens,
  }
}

const NOT_MEASURED = "not_measured"

type GatewayTelemetryTokens = {
  promptTokens?: number | typeof NOT_MEASURED
  completionTokens?: number | typeof NOT_MEASURED
  totalTokens?: number | typeof NOT_MEASURED
}

/**
 * The gateway's streaming terminal `chat.completion.chunk` carries no
 * top-level `usage`; exact token counts ride the `openagents.telemetry`
 * block (numbers, or the honest `not_measured` sentinel). Sentinels map to 0
 * — the same "absent usage" default the direct transport already uses.
 */
export function gatewayTelemetryToTurnUsage(
  telemetry: GatewayTelemetryTokens | undefined,
): GemmaTurnUsage {
  const measured = (value: number | typeof NOT_MEASURED | undefined): number =>
    typeof value === "number" && Number.isFinite(value) ? value : 0
  return gatewayUsageToTurnUsage({
    prompt_tokens: measured(telemetry?.promptTokens),
    completion_tokens: measured(telemetry?.completionTokens),
    total_tokens: measured(telemetry?.totalTokens),
  })
}

async function generateViaGateway(
  config: GatewayConfig,
  system: string,
  contents: ReadonlyArray<GemmaContent>,
  fetchImpl: FetchLike,
): Promise<
  | { ok: true; reply: string; model: string; usage: GemmaTurnUsage }
  | { ok: false; error: string }
> {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), config.timeoutMs)
  try {
    const response = await fetchImpl(
      `${config.url}/chat/completions`,
      gatewayRequestInit(
        config,
        {
          model: config.model,
          messages: toGatewayMessages(system, contents),
          max_tokens: config.maxOutputTokens,
          stream: false,
        },
        controller.signal,
      ),
    )
    if (!response.ok) {
      return { ok: false, error: `gateway_inference_http_${response.status}` }
    }
    const data = (await response.json()) as {
      model?: string
      choices?: Array<{ message?: { content?: string } }>
      usage?: GatewayUsageWire
    }
    // Only `message.content` is read — the gateway routes thinking-model
    // scratchpad to `reasoning_content`, which is deliberately ignored here.
    const reply = (data.choices?.[0]?.message?.content ?? "").trim()
    if (!reply) return { ok: false, error: "gateway_inference_empty_reply" }
    const usage = gatewayUsageToTurnUsage(data.usage)
    recordDailyTokenUsage(usage.totalTokens)
    return { ok: true, reply, model: data.model ?? config.model, usage }
  } catch (error) {
    return {
      ok: false,
      error:
        error instanceof Error && error.name === "AbortError"
          ? "gateway_inference_timeout"
          : "gateway_inference_unreachable",
    }
  } finally {
    clearTimeout(timeout)
  }
}

async function* streamViaGateway(
  config: GatewayConfig,
  system: string,
  contents: ReadonlyArray<GemmaContent>,
  fetchImpl: FetchLike,
): AsyncGenerator<GemmaStreamEvent> {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), config.timeoutMs)
  try {
    const response = await fetchImpl(
      `${config.url}/chat/completions`,
      gatewayRequestInit(
        config,
        {
          model: config.model,
          messages: toGatewayMessages(system, contents),
          max_tokens: config.maxOutputTokens,
          stream: true,
        },
        controller.signal,
      ),
    )
    if (!response.ok || !response.body) {
      yield {
        type: "error",
        error: `gateway_inference_http_${response.status}`,
      }
      return
    }

    const reader = response.body.getReader()
    const decoder = new TextDecoder()
    let buffer = ""
    let fullText = ""
    let telemetry: GatewayTelemetryTokens | undefined
    for (;;) {
      const { done, value } = await reader.read()
      if (done) break
      buffer += decoder.decode(value, { stream: true })
      let newlineIndex: number
      while ((newlineIndex = buffer.indexOf("\n")) !== -1) {
        const line = buffer.slice(0, newlineIndex).trim()
        buffer = buffer.slice(newlineIndex + 1)
        if (!line.startsWith("data:")) continue
        const payload = line.slice(5).trim()
        if (!payload || payload === "[DONE]") continue
        try {
          const frame = JSON.parse(payload) as {
            choices?: Array<{
              // Only `content` is forwarded. Thinking-model scratchpad rides
              // the separate `reasoning_content` delta channel and is NEVER
              // surfaced (the thought-filtering law on the gateway transport).
              delta?: { content?: string; reasoning_content?: string }
            }>
            openagents?: { telemetry?: GatewayTelemetryTokens }
          }
          if (frame.openagents?.telemetry !== undefined) {
            telemetry = frame.openagents.telemetry
          }
          const delta = frame.choices?.[0]?.delta?.content ?? ""
          if (delta) {
            fullText += delta
            yield { type: "delta", text: delta }
          }
        } catch {
          // Skip malformed frames.
        }
      }
    }
    if (!fullText.trim()) {
      yield { type: "error", error: "gateway_inference_empty_reply" }
      return
    }
    const usage = gatewayTelemetryToTurnUsage(telemetry)
    recordDailyTokenUsage(usage.totalTokens)
    yield { type: "done", fullText: fullText.trim(), usage }
  } catch (error) {
    yield {
      type: "error",
      error:
        error instanceof Error && error.name === "AbortError"
          ? "gateway_inference_timeout"
          : "gateway_inference_unreachable",
    }
  } finally {
    clearTimeout(timeout)
  }
}

export async function generateSarahGemmaReply({
  system,
  contents,
  fetchImpl = fetch,
}: {
  system: string
  contents: ReadonlyArray<GemmaContent>
  fetchImpl?: FetchLike
}): Promise<
  | { ok: true; reply: string; model: string; usage: GemmaTurnUsage }
  | { ok: false; error: string }
> {
  // Cost cap (#8600): typed refusal BEFORE any provider call, on either
  // transport. Callers surface the canned busy reply.
  if (dailyTokenCapExceeded()) {
    return { ok: false, error: SARAH_DAILY_TOKEN_CAP_ERROR }
  }

  // KHS-1: the Khala gateway transport wins whenever it is armed; the direct
  // Google path below remains the fallback until the gateway env is set.
  const gateway = gatewayConfig()
  if (gateway !== null) {
    return generateViaGateway(gateway, system, contents, fetchImpl)
  }

  const apiKey = process.env.GEMINI_API_KEY?.trim()
  if (!apiKey) return { ok: false, error: "google_inference_not_armed" }

  const model = sarahTextModel()
  const baseUrl = (
    process.env.SARAH_GOOGLE_INFERENCE_BASE_URL?.trim() || BASE_URL_DEFAULT
  ).replace(/\/+$/, "")
  // Thought tokens draw from the same output budget, so keep headroom above
  // the visible-answer size we actually want.
  const maxOutputTokens = Number(
    process.env.SARAH_TEXT_MAX_OUTPUT_TOKENS ?? 2048,
  )
  const timeoutMs = Number(process.env.SARAH_TEXT_TIMEOUT_MS ?? 45_000)

  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), timeoutMs)
  try {
    let response: Response | null = null
    // Per-model RPM buckets: walk the model chain on 429 instead of waiting.
    for (const candidateModel of sarahTextModelChain()) {
      response = await fetchImpl(
        `${baseUrl}/models/${candidateModel}:generateContent?key=${apiKey}`,
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          signal: controller.signal,
          body: JSON.stringify({
            systemInstruction: { parts: [{ text: system }] },
            contents,
            generationConfig: { maxOutputTokens },
          }),
        },
      )
      if (response.status !== 429) break
    }
    if (!response || !response.ok) {
      // Never include the URL (it carries the key) in surfaced errors.
      return {
        ok: false,
        error: `google_inference_http_${response?.status ?? 0}`,
      }
    }
    const data = (await response.json()) as {
      candidates?: Array<{ content?: { parts?: GemmaPart[] } }>
      usageMetadata?: {
        promptTokenCount?: number
        candidatesTokenCount?: number
        thoughtsTokenCount?: number
        totalTokenCount?: number
      }
    }
    const parts = data.candidates?.[0]?.content?.parts ?? []
    const reply = extractGemmaReply(parts)
    if (!reply) return { ok: false, error: "google_inference_empty_reply" }
    const usage = data.usageMetadata ?? {}
    recordDailyTokenUsage(usage.totalTokenCount ?? 0)
    return {
      ok: true,
      reply,
      model,
      usage: {
        promptTokens: usage.promptTokenCount ?? 0,
        outputTokens: usage.candidatesTokenCount ?? 0,
        thoughtTokens: usage.thoughtsTokenCount ?? 0,
        totalTokens: usage.totalTokenCount ?? 0,
      },
    }
  } catch (error) {
    return {
      ok: false,
      error:
        error instanceof Error && error.name === "AbortError"
          ? "google_inference_timeout"
          : "google_inference_unreachable",
    }
  } finally {
    clearTimeout(timeout)
  }
}

export type GemmaStreamEvent =
  | { type: "delta"; text: string }
  | { type: "done"; fullText: string; usage: GemmaTurnUsage }
  | { type: "error"; error: string }

/**
 * Streaming variant for latency-sensitive callers (the avatar brain):
 * forwards non-thought answer deltas as Gemma produces them. Thought parts
 * stream first and are dropped without ever being surfaced.
 */
export async function* streamSarahGemmaReply({
  system,
  contents,
  fetchImpl = fetch,
}: {
  system: string
  contents: ReadonlyArray<GemmaContent>
  fetchImpl?: FetchLike
}): AsyncGenerator<GemmaStreamEvent> {
  // Cost cap (#8600): typed refusal BEFORE any provider call, on either
  // transport. Callers surface the canned busy reply.
  if (dailyTokenCapExceeded()) {
    yield { type: "error", error: SARAH_DAILY_TOKEN_CAP_ERROR }
    return
  }

  // KHS-1: the Khala gateway transport wins whenever it is armed. Deltas are
  // forwarded as they arrive so first-byte latency stays fast for the avatar
  // brain (the gateway's pass-through stream emits frame-by-frame).
  const gateway = gatewayConfig()
  if (gateway !== null) {
    yield* streamViaGateway(gateway, system, contents, fetchImpl)
    return
  }

  const apiKey = process.env.GEMINI_API_KEY?.trim()
  if (!apiKey) {
    yield { type: "error", error: "google_inference_not_armed" }
    return
  }
  const model = sarahTextModel()
  const baseUrl = (
    process.env.SARAH_GOOGLE_INFERENCE_BASE_URL?.trim() || BASE_URL_DEFAULT
  ).replace(/\/+$/, "")
  const maxOutputTokens = Number(process.env.SARAH_TEXT_MAX_OUTPUT_TOKENS ?? 2048)
  const timeoutMs = Number(process.env.SARAH_TEXT_TIMEOUT_MS ?? 45_000)

  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), timeoutMs)
  try {
    let response: Response | null = null
    for (const candidateModel of sarahTextModelChain()) {
      response = await fetchImpl(
        `${baseUrl}/models/${candidateModel}:streamGenerateContent?alt=sse&key=${apiKey}`,
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          signal: controller.signal,
          body: JSON.stringify({
            systemInstruction: { parts: [{ text: system }] },
            contents,
            generationConfig: { maxOutputTokens },
          }),
        },
      )
      if (response.status !== 429) break
    }
    if (!response || !response.ok || !response.body) {
      yield { type: "error", error: `google_inference_http_${response?.status ?? 0}` }
      return
    }

    const reader = response.body.getReader()
    const decoder = new TextDecoder()
    let buffer = ""
    let fullText = ""
    let usage: GemmaTurnUsage = {
      promptTokens: 0,
      outputTokens: 0,
      thoughtTokens: 0,
      totalTokens: 0,
    }
    for (;;) {
      const { done, value } = await reader.read()
      if (done) break
      buffer += decoder.decode(value, { stream: true })
      let newlineIndex: number
      while ((newlineIndex = buffer.indexOf("\n")) !== -1) {
        const line = buffer.slice(0, newlineIndex).trim()
        buffer = buffer.slice(newlineIndex + 1)
        if (!line.startsWith("data:")) continue
        const payload = line.slice(5).trim()
        if (!payload || payload === "[DONE]") continue
        try {
          const frame = JSON.parse(payload) as {
            candidates?: Array<{ content?: { parts?: Array<{ text?: string; thought?: boolean }> } }>
            usageMetadata?: {
              promptTokenCount?: number
              candidatesTokenCount?: number
              thoughtsTokenCount?: number
              totalTokenCount?: number
            }
          }
          const meta = frame.usageMetadata
          if (meta) {
            usage = {
              promptTokens: meta.promptTokenCount ?? usage.promptTokens,
              outputTokens: meta.candidatesTokenCount ?? usage.outputTokens,
              thoughtTokens: meta.thoughtsTokenCount ?? usage.thoughtTokens,
              totalTokens: meta.totalTokenCount ?? usage.totalTokens,
            }
          }
          const delta = extractGemmaReply(frame.candidates?.[0]?.content?.parts ?? [])
          if (delta) {
            fullText += delta
            yield { type: "delta", text: delta }
          }
        } catch {
          // Skip malformed frames.
        }
      }
    }
    if (!fullText.trim()) {
      yield { type: "error", error: "google_inference_empty_reply" }
      return
    }
    recordDailyTokenUsage(usage.totalTokens)
    yield { type: "done", fullText: fullText.trim(), usage }
  } catch (error) {
    yield {
      type: "error",
      error:
        error instanceof Error && error.name === "AbortError"
          ? "google_inference_timeout"
          : "google_inference_unreachable",
    }
  } finally {
    clearTimeout(timeout)
  }
}
