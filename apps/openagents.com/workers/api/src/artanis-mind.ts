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

export const ArtanisMindModelDefault = 'gemini-2.5-flash'
export const ArtanisMindGatewayProvider = 'google-ai-studio'
export const ArtanisMindAccountId = '54fac8b750a29fdda9f2fa0f0afaed90'
export const ArtanisMindGatewayCandidates = ['openagents-ai-gateway'] as const

export type ArtanisMindServedVia =
  | 'cloudflare_ai_gateway'
  | 'google_direct'

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

const geminiBody = (system: string, prompt: string) =>
  JSON.stringify({
    contents: [{ parts: [{ text: prompt }], role: 'user' }],
    // gemini-2.5-flash spends "thinking" tokens from the same output
    // budget; without disabling it a 1024 cap leaves ~90 chars of text
    // (live truncation incident, topic 479e4480, 2026-06-10).
    generationConfig: {
      maxOutputTokens: 4096,
      temperature: 0.2,
      thinkingConfig: { thinkingBudget: 0 },
    },
    systemInstruction: { parts: [{ text: system }] },
  })

const textFromGeminiResponse = (payload: unknown): string | null => {
  const candidates = (payload as {
    candidates?: ReadonlyArray<{
      content?: { parts?: ReadonlyArray<{ text?: string }> }
    }>
  }).candidates
  const parts = candidates?.[0]?.content?.parts
  if (parts === undefined) return null
  const text = parts.map(part => part.text ?? '').join('')
  return text === '' ? null : text
}

export const artanisMindComplete = async (input: Readonly<{
  apiKey: string
  fetchImpl?: typeof fetch
  gatewayId?: string | undefined
  gatewayToken?: string | undefined
  model?: string | undefined
  prompt: string
  system: string
}>): Promise<ArtanisMindResult | ArtanisMindFailure> => {
  const fetchImpl = input.fetchImpl ?? fetch
  const model = input.model ?? ArtanisMindModelDefault
  const body = geminiBody(input.system, input.prompt)
  const attempts: Array<{ path: string; status: number; detail: string }> = []
  const gatewayIds = input.gatewayId !== undefined
    ? [input.gatewayId]
    : [...ArtanisMindGatewayCandidates]

  for (const gatewayId of gatewayIds) {
    const url = `https://gateway.ai.cloudflare.com/v1/${ArtanisMindAccountId}/${gatewayId}/${ArtanisMindGatewayProvider}/v1beta/models/${model}:generateContent`
    const response = await fetchImpl(url, {
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
      const text = textFromGeminiResponse(payload)
      if (text !== null) {
        return {
          gatewayId,
          model,
          promptChars: input.prompt.length,
          responseChars: text.length,
          servedVia: 'cloudflare_ai_gateway',
          text,
        }
      }
      attempts.push({
        detail: 'gateway 200 without candidate text',
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

  const directUrl = `https://generativelanguage.googleapis.com/v1beta/models/${model}:generateContent`
  const direct = await fetchImpl(directUrl, {
    body,
    headers: {
      'content-type': 'application/json',
      'x-goog-api-key': input.apiKey,
    },
    method: 'POST',
  })
  if (direct.ok) {
    const payload = await direct.json()
    const text = textFromGeminiResponse(payload)
    if (text !== null) {
      return {
        gatewayId: null,
        model,
        promptChars: input.prompt.length,
        responseChars: text.length,
        servedVia: 'google_direct',
        text,
      }
    }
    attempts.push({
      detail: 'direct 200 without candidate text',
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

  return { attempts, error: 'artanis_mind_unavailable' }
}

export const ArtanisMindSmokeSystem = [
  'You are Artanis, the Nexus administrator of the OpenAgents Pylon fleet.',
  'You propose actions; typed schemas validate them; approval gates hold.',
  'Answer in one short sentence, no markdown.',
].join(' ')
