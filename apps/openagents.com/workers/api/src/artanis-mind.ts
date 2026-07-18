/**
 * The Artanis cloud mind: model inference for the Nexus administrator,
 * running inside the OpenAgents worker (owner directive 2026-06-10 -
 * docs/artanis/2026-06-10-artanis-production-tick-and-tassadar-evolution-audit.md
 * section 7.1, embodiment (a)).
 *
 * Inference path: Google AI Studio directly. The mind proposes; typed schemas validate;
 * approval gates hold - intelligence never upgrades authority.
 */

import { DEFAULT_GEMMA4_MODEL_ID } from './inference/gemma4-model'

export const ArtanisMindModelDefault = DEFAULT_GEMMA4_MODEL_ID
export type ArtanisMindServedVia =
  | 'google_direct'
  | 'openagents_khala'

// Exact token usage parsed from Gemini's `usageMetadata` (the receipt-first
// source the hosted-Khala metering lane prices + debits against — #8555). Every
// field is the raw Gemini counter; the metering helper normalizes them (thoughts
// fold into output, prompt is input, etc). `null` on `ArtanisMindResult.usage`
// means the provider returned no usageMetadata, so the caller must NOT fabricate
// a usage row (exact-only money path).
export type ArtanisMindUsage = Readonly<{
  /** Gemini `promptTokenCount` — billed input tokens. */
  promptTokens: number
  /** Gemini `candidatesTokenCount` — visible output tokens. */
  candidatesTokens: number
  /** Gemini `thoughtsTokenCount` — thinking tokens (0 when thinking disabled). */
  thoughtsTokens: number
  /** Gemini `cachedContentTokenCount` — cached-input subset of prompt tokens. */
  cachedInputTokens: number
  /** Gemini `totalTokenCount`. */
  totalTokens: number
}>

export type ArtanisMindResult = Readonly<{
  servedVia: ArtanisMindServedVia
  gatewayId: string | null
  model: string
  text: string
  promptChars: number
  responseChars: number
  /** Exact provider usage, or `null` when Gemini reported no usageMetadata. */
  usage: ArtanisMindUsage | null
}>

export type ArtanisMindFailure = Readonly<{
  error: 'artanis_mind_unavailable'
  attempts: ReadonlyArray<{ path: string; status: number; detail: string }>
}>

// A model response that stopped because it hit the output-token cap is
// truncated mid-thought (the live `mer`/`re-ex`/`r.` artifacts seen by an
// external verifier on topic 7ba5d586). Truncated text is never a valid
// answer: returning it would post a cut-off reply. We surface it as a typed
// candidate so callers can raise the cap and retry, and treat an
// unrecoverable truncation as unavailability rather than emitting the
// partial text.
type GeminiCandidateText =
  | { kind: 'text'; text: string }
  | { kind: 'truncated'; text: string }
  | { kind: 'empty' }

// The smallest cap we will ever ask Gemini for, and the cap we escalate to
// once when a first attempt truncates. The composer's grounded replies are
// long; a single bump avoids both the historical 1024-cap incident and the
// new mid-word truncation without unbounded retries.
export const ArtanisMindDefaultMaxOutputTokens = 4096
export const ArtanisMindEscalatedMaxOutputTokens = 8192

export type ArtanisMindImage = Readonly<{
  dataBase64: string
  mediaType: 'image/png' | 'image/jpeg' | 'image/gif' | 'image/webp'
}>

const geminiBody = (
  model: string,
  system: string,
  prompt: string,
  maxOutputTokens: number,
  images: ReadonlyArray<ArtanisMindImage> = [],
) =>
  JSON.stringify({
    contents: [{
      parts: [
        { text: prompt },
        ...images.map(image => ({
          inlineData: { data: image.dataBase64, mimeType: image.mediaType },
        })),
      ],
      role: 'user',
    }],
    // Gemma 4 supports thinking levels rather than Gemini's numeric thinking
    // budget. Minimal preserves its reasoning behavior while keeping Sarah's
    // conversational turns responsive. Keep the old setting for explicit
    // legacy Gemini callers of this shared completion seam.
    generationConfig: {
      maxOutputTokens,
      temperature: 0.2,
      thinkingConfig: model.startsWith('gemma-4-')
        ? { thinkingLevel: 'minimal' }
        : { thinkingBudget: 0 },
    },
    systemInstruction: { parts: [{ text: system }] },
  })

// Parse Gemini's `usageMetadata` receipt into exact token counts. Returns
// `null` when the block is absent or carries no usable counts, so the caller
// records no fabricated usage (exact-only money path). Non-finite/negative
// counters are clamped to 0 (never a negative charge basis).
const nonNegInt = (value: unknown): number => {
  const n = typeof value === 'number' ? value : Number(value)
  return Number.isFinite(n) && n > 0 ? Math.trunc(n) : 0
}

const usageFromGeminiResponse = (payload: unknown): ArtanisMindUsage | null => {
  const meta = (payload as {
    usageMetadata?: {
      promptTokenCount?: unknown
      candidatesTokenCount?: unknown
      thoughtsTokenCount?: unknown
      cachedContentTokenCount?: unknown
      totalTokenCount?: unknown
    }
  }).usageMetadata
  if (meta === undefined || meta === null || typeof meta !== 'object') {
    return null
  }
  const usage: ArtanisMindUsage = {
    candidatesTokens: nonNegInt(meta.candidatesTokenCount),
    cachedInputTokens: nonNegInt(meta.cachedContentTokenCount),
    promptTokens: nonNegInt(meta.promptTokenCount),
    thoughtsTokens: nonNegInt(meta.thoughtsTokenCount),
    totalTokens: nonNegInt(meta.totalTokenCount),
  }
  // No exact input OR output tokens => nothing billable; treat as absent so the
  // metering lane skips the row rather than writing a zero-token exact receipt.
  return usage.promptTokens + usage.candidatesTokens + usage.totalTokens > 0
    ? usage
    : null
}

const textFromGeminiResponse = (payload: unknown): GeminiCandidateText => {
  const candidate = (payload as {
    candidates?: ReadonlyArray<{
      content?: { parts?: ReadonlyArray<{ text?: string; thought?: boolean }> }
      finishReason?: string
    }>
  }).candidates?.[0]
  const parts = candidate?.content?.parts
  if (parts === undefined) return { kind: 'empty' }
  // Gemma 4 can return private scratchpad parts before its answer. Those are
  // metered through usageMetadata but must never become user-visible chat text.
  const text = parts
    .filter(part => part.thought !== true)
    .map(part => part.text ?? '')
    .join('')
  if (text === '') return { kind: 'empty' }
  // MAX_TOKENS means the model was cut off mid-output - the text is a
  // truncated fragment, not a complete answer.
  return candidate?.finishReason === 'MAX_TOKENS'
    ? { kind: 'truncated', text }
    : { kind: 'text', text }
}

const artanisMindCompleteOnce = async (input: Readonly<{
  apiKey: string
  fetchImpl: typeof fetch
  model: string
  prompt: string
  system: string
  maxOutputTokens: number
  images?: ReadonlyArray<ArtanisMindImage> | undefined
}>): Promise<
  | { ok: true; result: ArtanisMindResult }
  | { ok: false; truncated: boolean; failure: ArtanisMindFailure }
> => {
  const body = geminiBody(
    input.model,
    input.system,
    input.prompt,
    input.maxOutputTokens,
    input.images,
  )
  const attempts: Array<{ path: string; status: number; detail: string }> = []
  let truncated = false

  const directUrl = `https://generativelanguage.googleapis.com/v1beta/models/${input.model}:generateContent`
  const direct = await input.fetchImpl(directUrl, {
    body,
    headers: {
      'content-type': 'application/json',
      'x-goog-api-key': input.apiKey,
    },
    method: 'POST',
  })
  if (direct.ok) {
    const payload = await direct.json()
    const candidate = textFromGeminiResponse(payload)
    if (candidate.kind === 'text') {
      return {
        ok: true,
        result: {
          gatewayId: null,
          model: input.model,
          promptChars: input.prompt.length,
          responseChars: candidate.text.length,
          servedVia: 'google_direct',
          text: candidate.text,
          usage: usageFromGeminiResponse(payload),
        },
      }
    }
    if (candidate.kind === 'truncated') truncated = true
    attempts.push({
      detail:
        candidate.kind === 'truncated'
          ? 'direct 200 truncated (MAX_TOKENS)'
          : 'direct 200 without candidate text',
      path: 'google_direct',
      status: direct.status,
    })
  } else {
    attempts.push({
      detail: (await direct.text()).slice(0, 120),
      path: 'google_direct',
      status: direct.status,
    })
  }

  return {
    failure: { attempts, error: 'artanis_mind_unavailable' },
    ok: false,
    truncated,
  }
}

export const artanisMindComplete = async (input: Readonly<{
  apiKey: string
  fetchImpl?: typeof fetch
  model?: string | undefined
  prompt: string
  system: string
  maxOutputTokens?: number | undefined
  images?: ReadonlyArray<ArtanisMindImage> | undefined
}>): Promise<ArtanisMindResult | ArtanisMindFailure> => {
  const fetchImpl = input.fetchImpl ?? fetch
  const model = input.model ?? ArtanisMindModelDefault
  const baseMax = input.maxOutputTokens ?? ArtanisMindDefaultMaxOutputTokens

  const first = await artanisMindCompleteOnce({
    apiKey: input.apiKey,
    fetchImpl,
    maxOutputTokens: baseMax,
    model,
    prompt: input.prompt,
    system: input.system,
    images: input.images,
  })
  if (first.ok) return first.result

  // A truncated answer is recoverable exactly once by escalating the cap.
  // We never emit the partial text - if the escalated attempt still
  // truncates (or any path fails), the caller gets typed unavailability and
  // records a blocked action rather than posting a cut-off reply.
  if (first.truncated && baseMax < ArtanisMindEscalatedMaxOutputTokens) {
    const escalated = await artanisMindCompleteOnce({
      apiKey: input.apiKey,
      fetchImpl,
      maxOutputTokens: ArtanisMindEscalatedMaxOutputTokens,
      model,
      prompt: input.prompt,
      system: input.system,
      images: input.images,
    })
    if (escalated.ok) return escalated.result
    return escalated.failure
  }

  return first.failure
}

export const ArtanisMindSmokeSystem = [
  'You are Artanis, the Nexus administrator of the OpenAgents Pylon fleet.',
  'You propose actions; typed schemas validate them; approval gates hold.',
  'Answer in one short sentence, no markdown.',
].join(' ')
