/**
 * The Artanis cloud mind: model inference for the Nexus administrator,
 * running inside the OpenAgents worker (owner directive 2026-06-10 -
 * docs/artanis/2026-06-10-artanis-production-tick-and-tassadar-evolution-audit.md
 * section 7.1, embodiment (a)).
 *
 * Inference path: Cloudflare AI Gateway (provider google-ai-studio,
 * BYOK) when a gateway resolves, with direct Google AI Studio fallback
 * so the mind never depends on gateway availability. The smoke route
 * reports which path served. The mind proposes; typed schemas validate;
 * approval gates hold - intelligence never upgrades authority.
 */

export const ArtanisMindModelDefault = 'gemini-3.5-flash'
export const ArtanisMindGatewayProvider = 'google-ai-studio'
export const ArtanisMindAccountId = '54fac8b750a29fdda9f2fa0f0afaed90'
export const ArtanisMindGatewayCandidates = ['openagents-ai-gateway'] as const

export type ArtanisMindServedVia =
  | 'cloudflare_ai_gateway'
  | 'google_direct'
  | 'openagents_khala'

export type ArtanisMindResult = Readonly<{
  servedVia: ArtanisMindServedVia
  gatewayId: string | null
  model: string
  text: string
  promptChars: number
  responseChars: number
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

const geminiBody = (system: string, prompt: string, maxOutputTokens: number) =>
  JSON.stringify({
    contents: [{ parts: [{ text: prompt }], role: 'user' }],
    // gemini-3.5-flash spends "thinking" tokens from the same output
    // budget; without disabling it a 1024 cap leaves ~90 chars of text
    // (live truncation incident, topic 479e4480, 2026-06-10).
    generationConfig: {
      maxOutputTokens,
      temperature: 0.2,
      thinkingConfig: { thinkingBudget: 0 },
    },
    systemInstruction: { parts: [{ text: system }] },
  })

const textFromGeminiResponse = (payload: unknown): GeminiCandidateText => {
  const candidate = (payload as {
    candidates?: ReadonlyArray<{
      content?: { parts?: ReadonlyArray<{ text?: string }> }
      finishReason?: string
    }>
  }).candidates?.[0]
  const parts = candidate?.content?.parts
  if (parts === undefined) return { kind: 'empty' }
  const text = parts.map(part => part.text ?? '').join('')
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
  gatewayId?: string | undefined
  gatewayToken?: string | undefined
  model: string
  prompt: string
  system: string
  maxOutputTokens: number
}>): Promise<
  | { ok: true; result: ArtanisMindResult }
  | { ok: false; truncated: boolean; failure: ArtanisMindFailure }
> => {
  const body = geminiBody(input.system, input.prompt, input.maxOutputTokens)
  const attempts: Array<{ path: string; status: number; detail: string }> = []
  // Skip the Cloudflare AI Gateway entirely when no `cf-aig` token is
  // available: the gateway 401s without it, and we are exiting Cloudflare
  // (the account is cancelled). Going straight to the direct Google AI Studio
  // path — which needs only the Gemini API key — avoids a guaranteed-failing
  // round-trip and is the working inference path post-CF. An explicit
  // `gatewayId` (with a token) still opts back in.
  const gatewayIds =
    input.gatewayToken === undefined || input.gatewayToken === ''
      ? []
      : input.gatewayId !== undefined
        ? [input.gatewayId]
        : [...ArtanisMindGatewayCandidates]
  let truncated = false

  for (const gatewayId of gatewayIds) {
    const url = `https://gateway.ai.cloudflare.com/v1/${ArtanisMindAccountId}/${gatewayId}/${ArtanisMindGatewayProvider}/v1beta/models/${input.model}:generateContent`
    const response = await input.fetchImpl(url, {
      body,
      headers: {
        'content-type': 'application/json',
        'x-goog-api-key': input.apiKey,
        ...(input.gatewayToken === undefined
          ? {}
          : { 'cf-aig-authorization': `Bearer ${input.gatewayToken}` }),
      },
      method: 'POST',
    })
    if (response.ok) {
      const payload = await response.json()
      const candidate = textFromGeminiResponse(payload)
      if (candidate.kind === 'text') {
        return {
          ok: true,
          result: {
            gatewayId,
            model: input.model,
            promptChars: input.prompt.length,
            responseChars: candidate.text.length,
            servedVia: 'cloudflare_ai_gateway',
            text: candidate.text,
          },
        }
      }
      if (candidate.kind === 'truncated') truncated = true
      attempts.push({
        detail:
          candidate.kind === 'truncated'
            ? 'gateway 200 truncated (MAX_TOKENS)'
            : 'gateway 200 without candidate text',
        path: `gateway:${gatewayId}`,
        status: response.status,
      })
      continue
    }
    attempts.push({
      detail: (await response.text()).slice(0, 120),
      path: `gateway:${gatewayId}`,
      status: response.status,
    })
  }

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
  gatewayId?: string | undefined
  gatewayToken?: string | undefined
  model?: string | undefined
  prompt: string
  system: string
  maxOutputTokens?: number | undefined
}>): Promise<ArtanisMindResult | ArtanisMindFailure> => {
  const fetchImpl = input.fetchImpl ?? fetch
  const model = input.model ?? ArtanisMindModelDefault
  const baseMax = input.maxOutputTokens ?? ArtanisMindDefaultMaxOutputTokens

  const first = await artanisMindCompleteOnce({
    apiKey: input.apiKey,
    fetchImpl,
    gatewayId: input.gatewayId,
    gatewayToken: input.gatewayToken,
    maxOutputTokens: baseMax,
    model,
    prompt: input.prompt,
    system: input.system,
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
      gatewayId: input.gatewayId,
      gatewayToken: input.gatewayToken,
      maxOutputTokens: ArtanisMindEscalatedMaxOutputTokens,
      model,
      prompt: input.prompt,
      system: input.system,
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
