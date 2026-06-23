// Bun-host Khala cockpit turn (M1, #6009, EPIC #6017).
//
// Lane A — Cockpit. Submits a cockpit prompt to a `openagents/khala-*` model on
// the OpenAI-compatible `POST /v1/chat/completions` gateway, projects the
// assistant answer AND the NON-BREAKING `openagents` receipt block back to the
// webview, and computes the LIVE gate (only true when a real receipt ref is
// present).
//
// This is the Khala-specific sibling of `shell-turn.ts`. It deliberately keeps
// the Khala model id explicit (the cockpit chooses khala-mini / khala-code)
// instead of the free-tier default, and — unlike the plain shell turn — it
// surfaces the receipt so a Khala completion is auditable, not opaque.
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
  isEventStreamResponse,
  isKhalaCockpitModelId,
  isLiveReceipt,
  parseKhalaReceipt,
  reconstructKhalaCompletionFromSse,
  type KhalaCockpitModelId,
  type KhalaTurnResult,
  KHALA_MINI_MODEL_ID,
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

type KhalaTurnEnv = Readonly<Record<string, string | undefined>>

export type BuildKhalaTurnInput = Readonly<{
  prompt: string
  // The Khala model id to submit to. Defaults to khala-mini.
  model?: KhalaCockpitModelId
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

const errorResult = (text: string): KhalaTurnResult => ({
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
  const model: KhalaCockpitModelId =
    input.model && isKhalaCockpitModelId(input.model)
      ? input.model
      : KHALA_MINI_MODEL_ID
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

    const text = parseAssistantText(body)
    const receipt = parseKhalaReceipt(body)
    if (text === null) {
      // No answer text: still surface the receipt if one came back, but the turn
      // is not "ok".
      return {
        ok: false,
        text: "Khala returned an empty response. Please try again.",
        receipt,
        live: isLiveReceipt(receipt),
      }
    }
    return {
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
