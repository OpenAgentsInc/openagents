// Concrete owner-armed lane TRANSPORTS for the real decision sweep (#6307).
//
// `real-lane-executor.ts` types the `RealLaneTransport` seam (the only place
// credentials/IO live). THIS module builds the concrete transports:
//   - the Khala transport against the public OpenAI-compatible endpoint
//     (`https://openagents.com/api/v1/chat/completions`, model `openagents/khala`),
//     which is OWN-CAPACITY / no third-party spend and can run NOW; and
//   - a generic OpenAI-compatible transport builder the owner uses for the
//     Fireworks and Vertex (OpenAI-compatible) lanes by supplying their base URL +
//     API key + per-1k rate card. These are BILLABLE and stay dark until armed.
//
// All transports take an INJECTED `fetch` and `clock` so they are Worker-safe and
// fully testable (no real network in tests). The request bodies are bounded,
// public-safe synthetic prompts shaped to the cell's token lengths — the sweep
// measures lane behavior on production-shaped traffic, it does not replay private
// user prompts.
import type { ServedTokensRequestAttribution } from '../served-tokens-recorder'
import type { BenchmarkCell } from './matrix'
import { modelIdForBenchmarkCell } from './opencode-client-runner'
import type { RealLaneHttpResult, RealLaneTransport } from './real-lane-executor'

// Injected dependencies so the transport holds NO ambient IO.
export type TransportDeps = Readonly<{
  // The HTTP caller. The owner injects the real `fetch`; tests inject a fake.
  fetch: typeof fetch
  // Monotonic-ish millisecond clock for the latency split. Injected so tests are
  // deterministic and the module has no top-level `Date.now`.
  now: () => number
}>

// A public-safe per-1k rate card (msat) for a billable lane, so the transport can
// record a cost basis for cost-per-accepted-outcome. The Khala transport uses a
// zero rate card (own capacity — no third-party cost basis recorded here).
export type LaneRateCardMsat = Readonly<{
  perKPromptMsat: number
  perKCompletionMsat: number
  // Fraction of cached input tokens billed (e.g. 0.5 for a 50%-billed cache hit).
  cachedPromptBilledFraction: number
}>

const ZERO_RATE_CARD: LaneRateCardMsat = {
  perKPromptMsat: 0,
  perKCompletionMsat: 0,
  cachedPromptBilledFraction: 0.5,
}

// A typed, public-safe failure for a non-2xx provider response. The runner treats
// a thrown transport as a skipped/unexecuted run; this carries the lane + status
// (never the body, which could leak provider detail) so a failure is diagnosable
// without a generic thrown English string.
export class RealLaneRequestFailedError extends Error {
  readonly _tag = 'RealLaneRequestFailedError'
  constructor(
    readonly lane: RealLaneTransport['lane'],
    readonly status: number,
  ) {
    super(`Real lane "${lane}" request failed: HTTP ${status}`)
    this.name = 'RealLaneRequestFailedError'
  }
}

// Build a bounded, public-safe chat request body shaped to the cell. The prompt is
// a synthetic filler sized to the shape's input length — NOT a real user prompt.
// `maxTokens` caps the generation at the shape's output length so the billable
// lanes stay inside the owner budget cap.
const buildRequestBody = (cell: BenchmarkCell, modelRef: string): string => {
  // A bounded filler string; the provider's tokenizer will land near the target
  // input length. We keep it short and repeat a neutral token so no private data
  // is ever sent. The exact token count is read back from the provider `usage`.
  const approxWords = Math.max(8, Math.floor(cell.shape.inputTokens * 0.7))
  const filler = Array.from({ length: approxWords }, () => 'context').join(' ')
  return JSON.stringify({
    model: modelRef,
    messages: [
      {
        role: 'user',
        content: `Benchmark probe (${cell.workload}). Respond concisely. ${filler}`,
      },
    ],
    temperature: cell.sampling.temperature,
    max_tokens: cell.shape.outputTokens,
    stream: false,
  })
}

// Parse the OpenAI-compatible usage object into the receipt-first token counts.
const parseUsage = (
  body: unknown,
): {
  promptTokens: number
  completionTokens: number
  totalTokens: number
  cachedInputTokens: number
} => {
  const usage =
    typeof body === 'object' && body !== null && 'usage' in body
      ? (body as { usage?: Record<string, unknown> }).usage
      : undefined
  const num = (value: unknown): number =>
    typeof value === 'number' && Number.isFinite(value) ? value : 0
  const promptTokens = num(usage?.['prompt_tokens'])
  const completionTokens = num(usage?.['completion_tokens'])
  const totalTokens =
    num(usage?.['total_tokens']) || promptTokens + completionTokens
  const details = usage?.['prompt_tokens_details']
  const cachedInputTokens =
    typeof details === 'object' && details !== null
      ? num((details as Record<string, unknown>)['cached_tokens'])
      : 0
  return { promptTokens, completionTokens, totalTokens, cachedInputTokens }
}

const costBasisMsat = (
  promptTokens: number,
  completionTokens: number,
  cachedInputTokens: number,
  rate: LaneRateCardMsat,
): number => {
  const billablePrompt =
    promptTokens - cachedInputTokens * (1 - rate.cachedPromptBilledFraction)
  return Math.round(
    (Math.max(0, billablePrompt) / 1000) * rate.perKPromptMsat +
      (completionTokens / 1000) * rate.perKCompletionMsat,
  )
}

export type OpenAICompatibleTransportOptions = Readonly<{
  lane: RealLaneTransport['lane']
  billable: boolean
  baseUrl: string
  // The model id sent on the wire (overrides the benchmark cell model id when the
  // provider names the model differently). Defaults to the cell's model id.
  wireModelRef?: string | undefined
  // Bearer API key (owner-supplied). Omitted for the public Khala endpoint when it
  // accepts an agent token via a different header (passed in `extraHeaders`).
  apiKey?: string | undefined
  extraHeaders?: Readonly<Record<string, string>> | undefined
  rateCard?: LaneRateCardMsat | undefined
  region?: string | undefined
  deps: TransportDeps
}>

// Build a generic OpenAI-compatible chat-completions transport. The owner uses
// this for Khala (no key, public endpoint), Fireworks, and Vertex-OpenAI-compatible
// lanes by supplying the base URL + key + rate card. PURE construction; the IO is
// the injected `fetch`.
export const makeOpenAICompatibleTransport = (
  options: OpenAICompatibleTransportOptions,
): RealLaneTransport => {
  const rate = options.rateCard ?? ZERO_RATE_CARD
  const region = options.region ?? options.lane
  return {
    lane: options.lane,
    billable: options.billable,
    execute: async (
      cell: BenchmarkCell,
      _sampleIndex: number,
      attribution: ServedTokensRequestAttribution | null,
    ): Promise<RealLaneHttpResult> => {
      const wireModel = options.wireModelRef ?? modelIdForBenchmarkCell(cell)
      const headers: Record<string, string> = {
        'content-type': 'application/json',
        ...(options.apiKey === undefined
          ? {}
          : { authorization: `Bearer ${options.apiKey}` }),
        ...(options.extraHeaders ?? {}),
        // Tag the Khala lane's own benchmark load internal + segmented (#6298).
        ...(attribution === null
          ? {}
          : {
              'x-openagents-demand-kind': attribution.demandKind,
              ...(attribution.demandSource === undefined
                ? {}
                : { 'x-openagents-demand-source': attribution.demandSource }),
              ...(attribution.demandClient === undefined
                ? {}
                : { 'x-openagents-demand-client': attribution.demandClient }),
            }),
      }
      const start = options.deps.now()
      const response = await options.deps.fetch(
        `${options.baseUrl.replace(/\/$/, '')}/chat/completions`,
        {
          method: 'POST',
          headers,
          body: buildRequestBody(cell, wireModel),
        },
      )
      const end = options.deps.now()
      if (!response.ok) {
        throw new RealLaneRequestFailedError(options.lane, response.status)
      }
      const json = (await response.json()) as unknown
      const usage = parseUsage(json)
      const totalWallClockMs = Math.max(0, end - start)
      // Non-streaming: TTFT is not separately observable, so it equals the full
      // wall-clock (honest — we do not invent a first-token split for a buffered
      // call). A streaming transport variant can measure a real TTFT.
      const ttftMs = totalWallClockMs
      return {
        promptTokens: usage.promptTokens,
        completionTokens: usage.completionTokens,
        totalTokens: usage.totalTokens,
        cachedInputTokens: usage.cachedInputTokens,
        ttftMs,
        totalWallClockMs,
        region,
        costBasisMsat: costBasisMsat(
          usage.promptTokens,
          usage.completionTokens,
          usage.cachedInputTokens,
          rate,
        ),
      }
    },
  }
}

// The public Khala base URL for the OpenAI-compatible endpoint.
export const KHALA_PUBLIC_API_BASE_URL = 'https://openagents.com/api/v1'

// Build the Khala transport against the public endpoint. OWN-CAPACITY / no
// third-party cost basis (zero rate card). The owner supplies an agent token via
// the bearer key (the public `/api/v1` accepts `OPENAGENTS_AGENT_TOKEN`); the
// internal benchmark-sweep attribution is attached automatically by the runner.
export const makeKhalaPublicTransport = (
  deps: TransportDeps,
  options?: Readonly<{ agentToken?: string | undefined; baseUrl?: string | undefined }>,
): RealLaneTransport =>
  makeOpenAICompatibleTransport({
    lane: 'khala',
    billable: false,
    baseUrl: options?.baseUrl ?? KHALA_PUBLIC_API_BASE_URL,
    wireModelRef: 'openagents/khala',
    apiKey: options?.agentToken,
    rateCard: ZERO_RATE_CARD,
    region: 'openagents',
    deps,
  })

export { ZERO_RATE_CARD }
