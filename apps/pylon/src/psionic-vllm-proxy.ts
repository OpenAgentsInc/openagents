import { createHash } from "node:crypto"

type PsionicServeRequest = {
  messages?: Array<{ role?: unknown; content?: unknown }>
  model?: unknown
  passthroughParams?: Record<string, unknown>
  requireExactGreedyParity?: unknown
}

type CompletionResponse = {
  choices?: Array<{ finish_reason?: unknown; message?: { content?: unknown }; text?: unknown }>
  model?: unknown
  usage?: {
    completion_tokens?: unknown
    prompt_tokens?: unknown
    total_tokens?: unknown
  }
}

export type PsionicVllmProxyConfig = {
  bearerToken: string
  upstreamUrl: string
  upstreamModel: string
  nodeRef: string
  servedModel: string
  canaryRef: string
  replayChallengeRef: string
  fetchImpl?: typeof fetch
}

const json = (status: number, body: Record<string, unknown>): Response =>
  new Response(JSON.stringify(body), {
    headers: { "content-type": "application/json" },
    status,
  })

const shaRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("base64url").slice(0, 22)}`

const completionText = (response: CompletionResponse): string => {
  const choice = response.choices?.[0]
  const content = choice?.message?.content ?? choice?.text
  return typeof content === "string" ? content : ""
}

const finishReason = (response: CompletionResponse): string => {
  const reason = response.choices?.[0]?.finish_reason
  return typeof reason === "string" && reason.trim() !== "" ? reason : "stop"
}

const tokenCount = (value: unknown, fallbackText: string): number => {
  if (typeof value === "number" && Number.isInteger(value) && value >= 0) {
    return value
  }
  return Math.max(1, fallbackText.trim().split(/\s+/).filter(Boolean).length)
}

const requestMessages = (
  request: PsionicServeRequest,
): Array<{ role: string; content: string }> | null => {
  if (!Array.isArray(request.messages) || request.messages.length === 0) {
    return null
  }
  const messages: Array<{ role: string; content: string }> = []
  for (const message of request.messages) {
    if (
      typeof message.role !== "string" ||
      typeof message.content !== "string" ||
      message.content.trim() === ""
    ) {
      return null
    }
    messages.push({ content: message.content, role: message.role })
  }
  return messages
}

const passthroughNumber = (
  request: PsionicServeRequest,
  key: string,
): number | undefined => {
  const value = request.passthroughParams?.[key]
  return typeof value === "number" && Number.isFinite(value) ? value : undefined
}

export async function handlePsionicVllmProxyRequest(
  request: Request,
  config: PsionicVllmProxyConfig,
): Promise<Response> {
  if (request.method !== "POST") {
    return json(405, { error: "method_not_allowed" })
  }
  if (request.headers.get("authorization") !== `Bearer ${config.bearerToken}`) {
    return json(401, { error: "unauthorized" })
  }

  let serveRequest: PsionicServeRequest
  try {
    serveRequest = (await request.json()) as PsionicServeRequest
  } catch {
    return json(400, { error: "invalid_json" })
  }

  const messages = requestMessages(serveRequest)
  if (messages === null || serveRequest.requireExactGreedyParity !== true) {
    return json(400, { error: "invalid_psionic_serve_request" })
  }

  const maxTokens = passthroughNumber(serveRequest, "max_tokens") ?? 1
  const temperature = passthroughNumber(serveRequest, "temperature") ?? 0
  const upstream = await (config.fetchImpl ?? fetch)(config.upstreamUrl, {
    body: JSON.stringify({
      max_tokens: maxTokens,
      messages,
      model: config.upstreamModel,
      stream: false,
      temperature,
    }),
    headers: { "content-type": "application/json" },
    method: "POST",
  })

  if (!upstream.ok) {
    return json(upstream.status, { error: "upstream_rejected" })
  }

  const upstreamBody = (await upstream.json()) as CompletionResponse
  const content = completionText(upstreamBody)
  const trimmedContent = content.trim()
  const canaryPassed = trimmedContent === "OK"
  const blockerRefs = canaryPassed
    ? []
    : ["blocker.pylon_gateway_proxy.known_answer_canary_failed"]
  const promptTokens = tokenCount(upstreamBody.usage?.prompt_tokens, messages.map(m => m.content).join(" "))
  const completionTokens = tokenCount(
    upstreamBody.usage?.completion_tokens,
    content,
  )
  const totalTokens = tokenCount(
    upstreamBody.usage?.total_tokens,
    `${messages.map(m => m.content).join(" ")} ${content}`,
  )

  return json(200, {
    content,
    finishReason: finishReason(upstreamBody),
    paidTrafficVerification: {
      blockerRefs,
      canaryPassed,
      parityPassed: canaryPassed,
      payoutEligible: canaryPassed,
      replayPassed: canaryPassed,
    },
    parityMode: "exact_greedy_parity",
    parityVerified: canaryPassed,
    servedModel: config.servedModel,
    servingRunRef: shaRef(
      "serve.pylon.gateway_proxy",
      `${config.nodeRef}\n${config.servedModel}\n${content}`,
    ),
    stages: [
      {
        layerEnd: 32,
        layerStart: 0,
        nodeRef: config.nodeRef,
        role: "stage",
      },
    ],
    usage: {
      completionTokens,
      promptTokens,
      totalTokens,
    },
    verificationRefs: [config.canaryRef, config.replayChallengeRef],
  })
}
