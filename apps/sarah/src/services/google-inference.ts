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
    // The key's per-model Gemma RPM quota is tight; a single Retry-After
    // respecting retry absorbs adjacent-turn collisions without queueing.
    for (let attempt = 0; attempt < 2; attempt += 1) {
      response = await fetch(
        `${baseUrl}/models/${model}:generateContent?key=${apiKey}`,
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
      if (response.status !== 429 || attempt === 1) break
      const retryAfter = Number(response.headers.get("retry-after"))
      const waitMs = Math.min(
        Number.isFinite(retryAfter) && retryAfter > 0 ? retryAfter * 1000 : 4000,
        8000,
      )
      await new Promise((resolve) => setTimeout(resolve, waitMs))
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
