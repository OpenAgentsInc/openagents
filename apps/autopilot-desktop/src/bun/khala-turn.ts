// Bun-host Khala cockpit turn (M1, #6009, EPIC #6017).
//
// Lane A — Cockpit. Issues Verse Khala prompts through the standard
// OpenAI-compatible `POST /api/v1/chat/completions` streaming path by default,
// including the public `openagents` receipt block and LIVE gate. Explicit
// `OPENAGENTS_DESKTOP_KHALA_ISSUER=local` / `remote` still exercises the MCP
// issuer contract for development, but the unset/auto path is direct gateway.
//
// This is the Khala-specific sibling of `shell-turn.ts`. It submits to the single
// public `openagents/khala` model (not the free-tier default), and — unlike the
// plain shell turn — it surfaces the receipt so a Khala completion is auditable,
// not opaque. The old khala-mini / khala-code split ids are deprecated; the
// gateway only serves `openagents/khala`.
//
// SECRET BOUNDARY: the agent token lives ONLY in the Bun host. It is placed in
// the outbound Authorization header here and NEVER crosses the RPC boundary.
//
// HONEST, NEVER-FAKE: no token, or a gateway/network error, returns a clean
// in-conversation message and `live:false` with no receipt. It never invents an
// answer and never claims "live" without a receipt.
//
// NOT OWNER-DEPENDENT: works against a local/stub or staging gateway via the
// base-url env override. The prod gateway is inert/owner-gated; this path does
// not depend on it — the live badge is gated on a real receipt, not on prod.
//
// STREAMING IS THE DEFAULT (the 524 fix). The cockpit submits `stream:true` and
// consumes the SSE stream INCREMENTALLY: each chunk resets the Cloudflare edge
// idle timer, so a multi-minute generation (the crossy-road north-star prompt)
// never trips the ~100s edge timeout and returns 524. A blocking `stream:false`
// request buffered the whole completion server-side and was the root cause of
// the 524 (see
// docs/inference/2026-06-22-long-running-inference-response-strategies.md).
// `onToken` lets the cockpit render tokens live; the terminal `openagents`
// receipt block rides on the FINAL chunk and is attached on stream close. The
// path still consumes a non-streaming JSON body for a server/route that does not
// stream, so it degrades gracefully.

import {
  inferenceGatewayChatCompletionsUrl,
  resolveInferenceGatewaySettings,
} from "../shared/inference-gateway.js"
import {
  durableRequestIdFromUrl,
} from "../../../pylon/src/khala-requester.js"
import { handlePylonKhalaMcpRequest } from "../../../pylon/src/khala-mcp.js"
import {
  isEventStreamResponse,
  isLiveReceipt,
  normalizeKhalaCockpitModelId,
  parseKhalaReceipt,
  reconstructKhalaCompletionFromSse,
  type KhalaCockpitModelId,
  type KhalaDurableHandleProjection,
  type KhalaTurnIssuerPath,
  type KhalaTurnResult,
} from "../shared/khala-cockpit.js"

// A short, neutral TASK steer for the cockpit. IDENTITY IS OWNED BY THE GATEWAY:
// the `openagents.com` inference gateway injects the authoritative Khala identity
// system message (`KHALA_IDENTITY_SYSTEM_PROMPT`) for every `openagents/khala-*`
// request, so Khala presents only as Khala by OpenAgents and never reveals or
// implies its underlying model/provider (the Gemini/Google leak fix). This
// client steer deliberately carries NO identity claim — it would otherwise
// compete with the gateway contract — and only nudges answer shape.
export const KHALA_COCKPIT_SYSTEM_PROMPT =
  "Answer the user's message directly. " +
  "When asked to build something, return complete, runnable code."

const NO_TOKEN_MESSAGE =
  "I can't reach Khala yet — no OpenAgents account token is configured. " +
  "Set OPENAGENTS_AGENT_TOKEN in the desktop app's environment (the same token " +
  "the app uses to talk to openagents.com) and submit again. " +
  "You can create or copy a token from your account at https://openagents.com."

const DESKTOP_KHALA_ISSUER_ENV = "OPENAGENTS_DESKTOP_KHALA_ISSUER"
const DESKTOP_KHALA_TARGET_PYLON_ENV =
  "OPENAGENTS_DESKTOP_KHALA_TARGET_PYLON_REF"
const KHALA_CODING_WORKFLOW = "codex_agent_task" as const

type KhalaTurnEnv = Readonly<Record<string, string | undefined>>

export type BuildKhalaTurnInput = Readonly<{
  prompt: string
  // The Khala model id to submit to. Always normalized to the single public
  // `openagents/khala` id; a stale split slug is accepted and collapsed.
  model?: KhalaCockpitModelId | string
  env: KhalaTurnEnv
  // The OpenAgents agent token (kept in the Bun host); never crosses to webview.
  agentToken: string | null
  fetchFn?: typeof fetch
  // INTERACTIVE STREAMING DEFAULT (the 524 fix). When true (default), submit
  // `stream:true` and consume the SSE stream incrementally. Set false only for
  // explicit blocking comparison/debug.
  stream?: boolean
  // Optional live-render hook, fired per content delta as the stream arrives.
  // Public-safe (token text only); never carries credentials or the receipt.
  onToken?: (delta: string) => void
}>

type KhalaMcpToolOutcome = Readonly<{
  content?: unknown
  isError?: unknown
  structuredContent?: unknown
}>

const EMPTY_DURABLE_HANDLE: KhalaDurableHandleProjection = {
  assignmentRef: null,
  durableRequestId: null,
  durableStreamUrl: null,
}

const envString = (env: KhalaTurnEnv, key: string): string | null => {
  const value = env[key]?.trim()
  return value && value.length > 0 ? value : null
}

const resolveKhalaTurnIssuerPath = (env: KhalaTurnEnv): KhalaTurnIssuerPath => {
  const configured = envString(env, DESKTOP_KHALA_ISSUER_ENV)
  if (configured === null) {
    return "legacy_gateway"
  }
  const raw = configured.toLowerCase()
  if (raw === "0" || raw === "off" || raw === "legacy") {
    return "legacy_gateway"
  }
  if (raw === "remote" || raw === "http" || raw === "openagents") {
    return "remote_mcp"
  }
  if (raw === "auto" || raw === "1" || raw === "on" || raw === "true") {
    return "legacy_gateway"
  }
  if (
    raw === "local" ||
    raw === "mcp" ||
    raw === "pylon" ||
    raw === "stdio"
  ) {
    return "pylon_mcp_local"
  }
  return "legacy_gateway"
}

const targetPylonRefFromEnv = (env: KhalaTurnEnv): string | undefined =>
  envString(env, DESKTOP_KHALA_TARGET_PYLON_ENV) ??
  envString(env, "OPENAGENTS_TRAINING_PYLON_REF") ??
  envString(env, "PYLON_REF") ??
  undefined

const mcpEndpointUrl = (baseUrl: string): string =>
  `${baseUrl.replace(/\/+$/, "")}/api/mcp`

const durableHandleFromResponse = (res: Response): KhalaDurableHandleProjection => {
  const durableStreamUrl = res.headers.get("openagents-durable-stream-url")
  return {
    assignmentRef: res.headers.get("openagents-coding-assignment-ref"),
    durableRequestId: durableRequestIdFromUrl(durableStreamUrl),
    durableStreamUrl,
  }
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const stringField = (
  record: Record<string, unknown>,
  key: string,
): string | null => {
  const value = record[key]
  return typeof value === "string" && value.trim().length > 0
    ? value.trim()
    : null
}

const firstTextContent = (outcome: KhalaMcpToolOutcome): string | null => {
  const content = outcome.content
  if (!Array.isArray(content)) return null
  for (const item of content) {
    if (!isRecord(item)) continue
    if (item.type === "text" && typeof item.text === "string") {
      const text = item.text.trim()
      if (text.length > 0) return text
    }
  }
  return null
}

const parseJsonTextContent = (
  outcome: KhalaMcpToolOutcome,
): Record<string, unknown> | null => {
  const text = firstTextContent(outcome)
  if (text === null) return null
  try {
    const parsed = JSON.parse(text) as unknown
    return isRecord(parsed) ? parsed : null
  } catch {
    return null
  }
}

const structuredMcpData = (
  outcome: KhalaMcpToolOutcome,
): Record<string, unknown> | null => {
  if (isRecord(outcome.structuredContent)) return outcome.structuredContent
  return parseJsonTextContent(outcome)
}

const mcpEnvelopeOutcome = (envelope: unknown): KhalaMcpToolOutcome => {
  if (!isRecord(envelope)) {
    throw new Error("invalid MCP response")
  }
  const rpcError = envelope.error
  if (isRecord(rpcError)) {
    const message = stringField(rpcError, "message") ?? "MCP request failed"
    throw new Error(message)
  }
  const result = envelope.result
  if (!isRecord(result)) {
    throw new Error("MCP response did not include a tool result")
  }
  return result
}

const mcpArguments = (
  prompt: string,
  env: KhalaTurnEnv,
): Record<string, unknown> => {
  const targetPylonRef = targetPylonRefFromEnv(env)
  return {
    prompt,
    workflow: KHALA_CODING_WORKFLOW,
    ...(targetPylonRef === undefined ? {} : { targetPylonRef }),
  }
}

const mcpToolCallRequest = (
  prompt: string,
  env: KhalaTurnEnv,
): Record<string, unknown> => ({
  id: "desktop.khala.request",
  jsonrpc: "2.0",
  method: "tools/call",
  params: {
    arguments: mcpArguments(prompt, env),
    name: "khala.request",
  },
})

const issuerLabel = (issuerPath: KhalaTurnIssuerPath): string =>
  issuerPath === "pylon_mcp_local"
    ? "local Pylon MCP"
    : issuerPath === "remote_mcp"
      ? "remote MCP"
      : "gateway"

const textForMcpIssuedRequest = (
  issuerPath: KhalaTurnIssuerPath,
  data: Record<string, unknown>,
): string => {
  const text = stringField(data, "text")
  if (text !== null) return text
  const durableRequestId = stringField(data, "durableRequestId")
  const label = issuerLabel(issuerPath)
  return durableRequestId === null
    ? `Khala coding request issued through ${label}.`
    : `Khala coding request issued through ${label}. Resume handle: ${durableRequestId}.`
}

const resultFromMcpOutcome = (
  issuerPath: KhalaTurnIssuerPath,
  outcome: KhalaMcpToolOutcome,
): KhalaTurnResult => {
  const data = structuredMcpData(outcome)
  if (outcome.isError === true) {
    return errorResult(
      (data === null ? null : stringField(data, "error")) ??
        firstTextContent(outcome) ??
        "Khala issuer returned an error.",
      issuerPath,
    )
  }
  if (data === null) {
    return errorResult("Khala issuer returned an empty MCP result.", issuerPath)
  }
  return {
    assignmentRef: stringField(data, "assignmentRef"),
    durableRequestId: stringField(data, "durableRequestId"),
    durableStreamUrl: stringField(data, "durableStreamUrl"),
    issuerPath,
    live: false,
    ok: true,
    receipt: null,
    text: textForMcpIssuedRequest(issuerPath, data),
  }
}

const requestThroughLocalPylonMcp = async (input: {
  env: KhalaTurnEnv
  fetchFn: typeof fetch
  prompt: string
  settingsBaseUrl: string
  token: string
}): Promise<KhalaTurnResult> => {
  const response = await handlePylonKhalaMcpRequest(
    mcpToolCallRequest(input.prompt, input.env),
    {
      network: {
        agentToken: input.token,
        baseUrl: input.settingsBaseUrl,
        fetch: input.fetchFn,
      },
    },
  )
  return resultFromMcpOutcome("pylon_mcp_local", mcpEnvelopeOutcome(response))
}

const requestThroughRemoteMcp = async (input: {
  env: KhalaTurnEnv
  fetchFn: typeof fetch
  prompt: string
  settingsBaseUrl: string
  token: string
}): Promise<KhalaTurnResult> => {
  const res = await input.fetchFn(mcpEndpointUrl(input.settingsBaseUrl), {
    body: JSON.stringify(mcpToolCallRequest(input.prompt, input.env)),
    headers: {
      accept: "application/json",
      authorization: `Bearer ${input.token}`,
      "content-type": "application/json",
    },
    method: "POST",
  })
  let body: unknown = null
  try {
    body = await res.json()
  } catch {
    body = null
  }
  if (!res.ok) {
    return errorResult(
      `Khala MCP issuer request failed (${res.status}). Please try again.`,
      "remote_mcp",
    )
  }
  return resultFromMcpOutcome("remote_mcp", mcpEnvelopeOutcome(body))
}

// Pull the assistant text out of an OpenAI-compatible chat-completions body.
const parseAssistantText = (body: unknown): string | null => {
  if (typeof body !== "object" || body === null) return null
  const choices = (body as { choices?: unknown }).choices
  if (!Array.isArray(choices) || choices.length === 0) return null
  const first = choices[0] as { message?: { content?: unknown } } | undefined
  const content = first?.message?.content
  return typeof content === "string" && content.trim().length > 0
    ? content
    : null
}

const parseErrorMessage = (body: unknown, status: number): string => {
  if (typeof body === "object" && body !== null) {
    const record = body as { error?: unknown; message?: unknown }
    if (typeof record.message === "string" && record.message.trim().length > 0) {
      return record.message
    }
    if (typeof record.error === "string" && record.error.trim().length > 0) {
      return record.error
    }
  }
  return `request failed (${status})`
}

const errorResult = (
  text: string,
  issuerPath: KhalaTurnIssuerPath = "legacy_gateway",
): KhalaTurnResult => ({
  ...EMPTY_DURABLE_HANDLE,
  issuerPath,
  ok: false,
  text,
  receipt: null,
  live: false,
})

// Read an SSE response body incrementally and reconstruct the completion. The
// incremental `reader.read()` loop is what resets the edge idle timer per chunk
// (the 524 fix); `onToken` fires live as deltas arrive. Falls back to
// `res.text()` for fetch impls without a readable body (e.g. test fakes).
const consumeSseBody = async (
  res: Response,
  onToken?: (delta: string) => void,
): Promise<unknown> => {
  const body = res.body
  if (body && typeof body.getReader === "function") {
    const reader = body.getReader()
    const decoder = new TextDecoder()
    let rawText = ""
    let pending = ""
    for (;;) {
      const { value, done } = await reader.read()
      if (done) break
      const piece = decoder.decode(value, { stream: true })
      rawText += piece
      if (onToken !== undefined) {
        // Emit deltas as soon as a complete frame arrives.
        pending += piece
        const frames = pending.split("\n\n")
        pending = frames.pop() ?? ""
        for (const frame of frames) {
          for (const line of frame.split("\n")) {
            if (!line.startsWith("data:")) continue
            const payload = line.slice(line.indexOf(":") + 1).trim()
            if (payload === "" || payload === "[DONE]") continue
            try {
              const parsed = JSON.parse(payload) as {
                choices?: Array<{ delta?: { content?: unknown } }>
              }
              const delta = parsed.choices?.[0]?.delta?.content
              if (typeof delta === "string" && delta.length > 0) onToken(delta)
            } catch {
              // ignore partial / non-JSON frames
            }
          }
        }
      }
    }
    // Authoritative reconstruction from the full text (no double onToken — we
    // already streamed above when a reader was available).
    return reconstructKhalaCompletionFromSse(rawText)
  }
  // No reader: read the whole text and reconstruct (onToken fires once here).
  const rawText = await res.text()
  return reconstructKhalaCompletionFromSse(rawText, onToken)
}

export const buildKhalaTurn = async (
  input: BuildKhalaTurnInput,
): Promise<KhalaTurnResult> => {
  const fetchFn = input.fetchFn ?? fetch
  const settings = resolveInferenceGatewaySettings(input.env)
  // The public model collapses to the single `openagents/khala` id. Any supplied
  // value (including a stale `khala-mini`/`khala-code` slug) normalizes to it so
  // the gateway never rejects the request with `model_unavailable`.
  const model: KhalaCockpitModelId = normalizeKhalaCockpitModelId(input.model)
  const url = inferenceGatewayChatCompletionsUrl(settings.baseUrl)
  const prompt = input.prompt.trim()

  if (prompt === "") {
    return errorResult("Type a message and submit to get a response.")
  }

  const token =
    input.agentToken !== null && input.agentToken.trim().length > 0
      ? input.agentToken.trim()
      : null
  if (token === null) {
    return errorResult(NO_TOKEN_MESSAGE)
  }

  const issuerPath = resolveKhalaTurnIssuerPath(input.env)
  if (issuerPath === "pylon_mcp_local") {
    try {
      return await requestThroughLocalPylonMcp({
        env: input.env,
        fetchFn,
        prompt,
        settingsBaseUrl: settings.baseUrl,
        token,
      })
    } catch (error) {
      const reason = error instanceof Error ? error.message : "network error"
      return errorResult(
        `I couldn't reach Khala through the local Pylon MCP issuer (${reason}). Check your connection and try again.`,
        issuerPath,
      )
    }
  }
  if (issuerPath === "remote_mcp") {
    try {
      return await requestThroughRemoteMcp({
        env: input.env,
        fetchFn,
        prompt,
        settingsBaseUrl: settings.baseUrl,
        token,
      })
    } catch (error) {
      const reason = error instanceof Error ? error.message : "network error"
      return errorResult(
        `I couldn't reach Khala through the remote MCP issuer (${reason}). Check your connection and try again.`,
        issuerPath,
      )
    }
  }

  // Streaming is the interactive default; `stream:false` only on explicit opt-out.
  const useStream = input.stream !== false

  try {
    const res = await fetchFn(url, {
      method: "POST",
      headers: {
        accept: useStream ? "text/event-stream" : "application/json",
        authorization: `Bearer ${token}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        model,
        stream: useStream,
        messages: [
          { role: "system", content: KHALA_COCKPIT_SYSTEM_PROMPT },
          { role: "user", content: prompt },
        ],
      }),
    })

    // Consume the response body. When the gateway streams SSE, read it
    // INCREMENTALLY so every chunk resets the edge idle timer (the 524 fix),
    // firing onToken live, then reconstruct the full completion + the terminal
    // `openagents` receipt block. Otherwise parse the non-streaming JSON body.
    // Error responses (non-2xx) are JSON, so parse those as JSON regardless.
    let body: unknown = null
    const contentType =
      typeof res.headers?.get === "function"
        ? res.headers.get("content-type")
        : null
    const streamed = res.ok && useStream && isEventStreamResponse(contentType)
    if (streamed) {
      body = await consumeSseBody(res, input.onToken)
    } else {
      try {
        body = await res.json()
      } catch {
        body = null
      }
    }

    if (!res.ok) {
      const reason = parseErrorMessage(body, res.status)
      if (res.status === 401 || res.status === 403) {
        return errorResult(
          "I couldn't authenticate with openagents.com. The configured " +
            "account token may be invalid or expired — refresh it and try again.",
        )
      }
      if (res.status === 402) {
        return errorResult(
          "Your credit balance is empty. Add credit at https://openagents.com " +
            "to keep running Khala.",
        )
      }
      if (res.status === 404) {
        return errorResult(
          "Khala isn't available right now. Please try again in a moment.",
        )
      }
      return errorResult(
        `I couldn't get a response right now: ${reason}. Please try again.`,
      )
    }

    const durableHandle = durableHandleFromResponse(res)
    const text = parseAssistantText(body)
    const receipt = parseKhalaReceipt(body)
    if (text === null) {
      // No answer text: still surface the receipt if one came back, but the turn
      // is not "ok".
      return {
        ...durableHandle,
        issuerPath,
        ok: false,
        text: "Khala returned an empty response. Please try again.",
        receipt,
        live: isLiveReceipt(receipt),
      }
    }
    return {
      ...durableHandle,
      issuerPath,
      ok: true,
      text,
      receipt,
      live: isLiveReceipt(receipt),
    }
  } catch (error) {
    const reason = error instanceof Error ? error.message : "network error"
    return errorResult(
      `I couldn't reach Khala (${reason}). Check your connection and try again.`,
    )
  }
}
