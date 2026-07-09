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

function streamingResponse(model: string, text: string): Response {
  const encoder = new TextEncoder()
  const id = `chatcmpl-sarah-${crypto.randomUUID()}`
  const now = Math.floor(Date.now() / 1000)
  const chunk = (delta: Record<string, unknown>, finish: string | null) =>
    `data: ${JSON.stringify({
      id,
      object: "chat.completion.chunk",
      created: now,
      model,
      choices: [{ index: 0, delta, finish_reason: finish }],
    })}\n\n`
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(encoder.encode(chunk({ role: "assistant" }, null)))
      // Sentence-sized chunks keep LiveAvatar's TTS pipeline fed steadily.
      for (const piece of text.match(/[^.!?]+[.!?]*\s*/g) ?? [text]) {
        controller.enqueue(encoder.encode(chunk({ content: piece }, null)))
      }
      controller.enqueue(encoder.encode(chunk({}, "stop")))
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

  let reply: string
  if (PRICING_GUARD_PATTERN.test(lastUserText)) {
    // The hard law holds on the voice lane too: pricing never reaches the model.
    reply = PRICING_GUARD_REPLY
    if (ref) {
      publishSarahAvatarEvent(ref, {
        type: "guard_refusal",
        title: "Pricing guard",
        body: "Configured deal rules only — no improvised discounts. Human handoff available.",
      })
    }
  } else if (!sarahGoogleInferenceArmed()) {
    reply =
      "I'm having trouble reaching my model right now — please try again in a moment."
  } else {
    const result = await generateSarahGemmaReply({
      system:
        cleanSystem ||
        "You are Sarah, OpenAgents' AI sales employee. Disclose you are an AI.",
      contents: history.length
        ? history
        : [{ role: "user", parts: [{ text: lastUserText || "Hello" }] }],
    })
    reply = result.ok
      ? result.reply
      : result.error === "google_inference_http_429"
        ? "I'm handling a lot of conversations right now — give me about a minute and ask again."
        : "I'm having trouble reaching my model right now — please try again in a moment."
  }

  if (ref) {
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

  if (body.stream) return streamingResponse(model, reply)
  return Response.json(completionPayload(model, reply))
}
