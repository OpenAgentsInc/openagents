/**
 * OpenAI-compatible chat/completions endpoint — the Sarah avatar brain
 * (#8598 AV-2). LiveAvatar's Custom LLM add-on calls this with the session
 * context (KB system prompt + `[conversation_ref: …]` marker) and the running
 * conversation; we answer from the owned runtime: deterministic
 * no-improvised-pricing guard BEFORE the model, then Gemma 4 on our Google
 * inference. Turns are recorded to the session index and mirrored to the
 * avatar event bus so the surface pops components as Sarah speaks.
 *
 * This endpoint is bearer-guarded (SARAH_AVATAR_LLM_BEARER) — without the
 * guard it would be a public LLM proxy. The bearer is registered on
 * LiveAvatar's side as the LLM secret.
 */

import { publishSarahAvatarEvent } from "./services/avatar-event-bus.ts"
import {
  generateSarahGemmaReply,
  sarahGoogleInferenceArmed,
  sarahTextModel,
  streamSarahGemmaReply,
} from "./services/google-inference.ts"
import type { GemmaContent } from "./services/google-inference.ts"
import { recordSarahTranscriptTurn } from "./services/session-index.ts"

const CONVERSATION_REF_PATTERN = /\[conversation_ref:\s*([^\]\s]+)\s*\]/

export const PRICING_GUARD_PATTERN = /price|discount|deal/i
export const PRICING_GUARD_REPLY =
  "I only quote public pack prices and owner-approved parameters — I won't improvise discounts. I can evaluate deal rules or open a human handoff."

type ChatMessage = { role: string; content: unknown }

function messageText(content: unknown): string {
  if (typeof content === "string") return content
  if (Array.isArray(content)) {
    return content
      .map((part) =>
        typeof part === "string"
          ? part
          : typeof (part as { text?: string })?.text === "string"
            ? (part as { text: string }).text
            : "",
      )
      .join("")
  }
  return ""
}

export function extractConversationRef(system: string): {
  ref: string | null
  cleanSystem: string
} {
  const match = CONVERSATION_REF_PATTERN.exec(system)
  return {
    ref: match?.[1] ?? null,
    cleanSystem: system.replace(CONVERSATION_REF_PATTERN, "").trim(),
  }
}

function completionPayload(model: string, text: string) {
  const now = Math.floor(Date.now() / 1000)
  return {
    id: `chatcmpl-sarah-${crypto.randomUUID()}`,
    object: "chat.completion",
    created: now,
    model,
    choices: [
      {
        index: 0,
        message: { role: "assistant", content: text },
        finish_reason: "stop",
      },
    ],
    usage: { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 },
  }
}

type ChunkWriter = {
  chunk: (delta: Record<string, unknown>, finish: string | null) => string
}

function makeChunkWriter(model: string): ChunkWriter {
  const id = `chatcmpl-sarah-${crypto.randomUUID()}`
  const now = Math.floor(Date.now() / 1000)
  return {
    chunk: (delta, finish) =>
      `data: ${JSON.stringify({
        id,
        object: "chat.completion.chunk",
        created: now,
        model,
        choices: [{ index: 0, delta, finish_reason: finish }],
      })}\n\n`,
  }
}

/** Stream a fixed string (guard refusals, fallbacks) as one immediate chunk. */
function streamingResponse(model: string, text: string): Response {
  const encoder = new TextEncoder()
  const writer = makeChunkWriter(model)
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(encoder.encode(writer.chunk({ role: "assistant" }, null)))
      controller.enqueue(encoder.encode(writer.chunk({ content: text }, null)))
      controller.enqueue(encoder.encode(writer.chunk({}, "stop")))
      controller.enqueue(encoder.encode("data: [DONE]\n\n"))
      controller.close()
    },
  })
  return new Response(stream, {
    headers: {
      "content-type": "text/event-stream",
      "cache-control": "no-cache",
    },
  })
}

/**
 * True streaming: the role chunk goes out immediately (LiveAvatar's pipeline
 * times out waiting for a first byte — the 2026-07-09 incident showed 11-12s
 * full-buffer responses being retried and dropped), empty-delta keepalives
 * cover Gemma's thinking phase, and answer deltas forward as they arrive.
 */
function liveStreamingResponse(
  model: string,
  system: string,
  contents: GemmaContent[],
  onComplete: (fullText: string) => Promise<void>,
): Response {
  const encoder = new TextEncoder()
  const writer = makeChunkWriter(model)
  const stream = new ReadableStream<Uint8Array>({
    async start(controller) {
      const send = (frame: string) => controller.enqueue(encoder.encode(frame))
      send(writer.chunk({ role: "assistant" }, null))
      let sawDelta = false
      const keepalive = setInterval(() => {
        if (!sawDelta) send(writer.chunk({}, null))
      }, 2000)
      try {
        let fullText = ""
        let failed: string | null = null
        for await (const event of streamSarahGemmaReply({ system, contents })) {
          if (event.type === "delta") {
            sawDelta = true
            fullText += event.text
            send(writer.chunk({ content: event.text }, null))
          } else if (event.type === "error") {
            failed = event.error
          }
        }
        if (!sawDelta) {
          const fallback =
            failed === "google_inference_http_429"
              ? "I'm handling a lot of conversations right now — give me about a minute and ask again."
              : "I'm having trouble reaching my model right now — please try again in a moment."
          fullText = fallback
          send(writer.chunk({ content: fallback }, null))
        }
        send(writer.chunk({}, "stop"))
        send("data: [DONE]\n\n")
        await onComplete(fullText)
      } finally {
        clearInterval(keepalive)
        controller.close()
      }
    },
  })
  return new Response(stream, {
    headers: {
      "content-type": "text/event-stream",
      "cache-control": "no-cache",
    },
  })
}

export async function handleSarahChatCompletions(request: Request): Promise<Response> {
  const configuredBearer = process.env.SARAH_AVATAR_LLM_BEARER?.trim()
  if (!configuredBearer) {
    return Response.json({ error: { code: "brain_not_armed" } }, { status: 503 })
  }
  const auth = request.headers.get("authorization") ?? ""
  if (auth !== `Bearer ${configuredBearer}`) {
    return Response.json({ error: { code: "unauthorized" } }, { status: 401 })
  }

  const body = (await request.json().catch(() => null)) as {
    model?: string
    stream?: boolean
    messages?: ChatMessage[]
  } | null
  if (!body || !Array.isArray(body.messages) || body.messages.length === 0) {
    return Response.json({ error: { code: "invalid_request" } }, { status: 400 })
  }

  const systemRaw = body.messages
    .filter((message) => message.role === "system")
    .map((message) => messageText(message.content))
    .join("\n\n")
  const { ref, cleanSystem } = extractConversationRef(systemRaw)

  const history: GemmaContent[] = []
  for (const message of body.messages) {
    if (message.role !== "user" && message.role !== "assistant") continue
    const text = messageText(message.content).trim()
    if (!text) continue
    history.push({
      role: message.role === "assistant" ? "model" : "user",
      parts: [{ text }],
    })
  }
  const lastUser = [...body.messages]
    .reverse()
    .find((message) => message.role === "user")
  const lastUserText = messageText(lastUser?.content ?? "").trim()
  const model = sarahTextModel()

  const publishAndRecord = async (reply: string) => {
    if (!ref) return
    if (lastUserText) {
      publishSarahAvatarEvent(ref, { type: "transcript", role: "user", text: lastUserText })
    }
    publishSarahAvatarEvent(ref, { type: "transcript", role: "assistant", text: reply })
    const shared = { prospectRef: ref, sessionId: `avatar:${ref}`, threadId: `avatar:${ref}` }
    if (lastUserText) {
      await recordSarahTranscriptTurn({
        ...shared,
        turn: { modality: "voice", role: "user", sourceEvent: "avatar_turn", text: lastUserText },
      })
    }
    await recordSarahTranscriptTurn({
      ...shared,
      turn: { modality: "voice", role: "assistant", sourceEvent: "avatar_turn", text: reply },
    })
  }

  if (PRICING_GUARD_PATTERN.test(lastUserText)) {
    // The hard law holds on the voice lane too: pricing never reaches the model.
    const reply = PRICING_GUARD_REPLY
    if (ref) {
      publishSarahAvatarEvent(ref, {
        type: "guard_refusal",
        title: "Pricing guard",
        body: "Configured deal rules only — no improvised discounts. Human handoff available.",
      })
    }
    await publishAndRecord(reply)
    if (body.stream) return streamingResponse(model, reply)
    return Response.json(completionPayload(model, reply))
  }

  if (!sarahGoogleInferenceArmed()) {
    const reply =
      "I'm having trouble reaching my model right now — please try again in a moment."
    await publishAndRecord(reply)
    if (body.stream) return streamingResponse(model, reply)
    return Response.json(completionPayload(model, reply))
  }

  const system =
    cleanSystem || "You are Sarah, OpenAgents' AI sales employee. Disclose you are an AI."
  const contents: GemmaContent[] = history.length
    ? history
    : [{ role: "user", parts: [{ text: lastUserText || "Hello" }] }]

  if (body.stream) {
    return liveStreamingResponse(model, system, contents, publishAndRecord)
  }

  const result = await generateSarahGemmaReply({ system, contents })
  const reply = result.ok
    ? result.reply
    : result.error === "google_inference_http_429"
      ? "I'm handling a lot of conversations right now — give me about a minute and ask again."
      : "I'm having trouble reaching my model right now — please try again in a moment."
  await publishAndRecord(reply)
  return Response.json(completionPayload(model, reply))
}
