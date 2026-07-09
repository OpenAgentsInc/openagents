/**
 * Google inference client for Sarah's text turns (#8594 flip-to-live).
 *
 * Talks to the Generative Language API (our existing Google inference,
 * project openagentsgemini) with a Gemma 4 model — owner direction
 * 2026-07-09: Gemma 4, not Gemini. Catalog slugs verified live:
 * `gemma-4-31b-it` (dense, default) and `gemma-4-26b-a4b-it` (MoE).
 *
 * Gemma 4 is a thinking model: candidates carry scratchpad parts flagged
 * `thought: true` before the answer parts. Thought parts are filtered out
 * and never stored, surfaced, or echoed into transcripts or receipts.
 * Gemma exposes only generateContent/countTokens — no native tool calling —
 * so the text path keeps its deterministic pricing guard upstream and the
 * tool loop stays on the realtime/voice path until a tools-capable lane.
 */

export const SARAH_TEXT_MODEL_DEFAULT = "gemma-4-31b-it"

const BASE_URL_DEFAULT = "https://generativelanguage.googleapis.com/v1beta"

export type GemmaContent = {
  role: "user" | "model"
  parts: Array<{ text: string }>
}

type GemmaPart = { text?: string; thought?: boolean }

export function sarahGoogleInferenceArmed(): boolean {
  return Boolean(process.env.GEMINI_API_KEY?.trim())
}

export function sarahTextModel(): string {
  return process.env.SARAH_TEXT_MODEL?.trim() || SARAH_TEXT_MODEL_DEFAULT
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

export async function generateSarahGemmaReply({
  system,
  contents,
}: {
  system: string
  contents: ReadonlyArray<GemmaContent>
}): Promise<
  | { ok: true; reply: string; model: string; usage: GemmaTurnUsage }
  | { ok: false; error: string }
> {
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
      response = await fetch(
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
}: {
  system: string
  contents: ReadonlyArray<GemmaContent>
}): AsyncGenerator<GemmaStreamEvent> {
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
      response = await fetch(
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
