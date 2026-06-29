// Self-hosted GPT-OSS backend for the on-device decider — the non-Mac drop-in.
//
// It speaks the standard OpenAI-compatible API served by the self-hosted GPT-OSS
// deployment (vLLM-style: GET /v1/models, POST /v1/chat/completions). For
// non-Apple-Silicon hosts this is the decider's local small-model path. It is
// OPTIONAL: when no endpoint is configured the backend simply reports
// unavailable and the decider falls back.

import type {
  OnDeviceBackendReadiness,
  OnDeviceDeciderBackend,
  OnDeviceDeciderCompleteOptions,
  OnDeviceDeciderMessage,
  OnDeviceDeciderResult,
} from "../shared/on-device-decider.js"

const GPT_OSS_DEFAULT_MODEL = "gpt-oss-20b"
const GPT_OSS_PROBE_TIMEOUT_MS = 2_500
const GPT_OSS_COMPLETE_TIMEOUT_MS = 60_000

type OpenAiModelsList = { readonly data?: ReadonlyArray<{ readonly id?: unknown }> }

type OpenAiChatCompletion = {
  readonly choices?: ReadonlyArray<{ readonly message?: { readonly content?: unknown } }>
  readonly model?: unknown
  readonly usage?: {
    readonly prompt_tokens?: unknown
    readonly completion_tokens?: unknown
    readonly total_tokens?: unknown
  }
}

const num = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value) ? value : 0

const str = (value: unknown, fallback: string): string =>
  typeof value === "string" && value.length > 0 ? value : fallback

export type GptOssDeciderBackendOptions = {
  /**
   * OpenAI-compatible base URL for the self-hosted GPT-OSS endpoint, e.g.
   * `https://gpt-oss.example/v1`. When omitted (or read from
   * `KHALA_GPT_OSS_BASE_URL`), the backend reports unavailable.
   */
  readonly baseUrl?: string
  readonly model?: string
  /** Optional bearer token (else `KHALA_GPT_OSS_API_KEY`). Never logged. */
  readonly apiKey?: string
  readonly fetchFn?: typeof fetch
  readonly env?: Readonly<Record<string, string | undefined>>
}

export function createGptOssDeciderBackend(
  options: GptOssDeciderBackendOptions = {},
): OnDeviceDeciderBackend {
  const env = options.env ?? {}
  const rawBaseUrl = (options.baseUrl ?? env.KHALA_GPT_OSS_BASE_URL ?? "").trim()
  const baseUrl = rawBaseUrl.replace(/\/+$/, "")
  const model = options.model ?? env.KHALA_GPT_OSS_MODEL ?? GPT_OSS_DEFAULT_MODEL
  const apiKey = (options.apiKey ?? env.KHALA_GPT_OSS_API_KEY ?? "").trim()
  const fetchFn = options.fetchFn ?? fetch

  const authHeaders = (): Record<string, string> =>
    apiKey.length > 0 ? { authorization: `Bearer ${apiKey}` } : {}

  const probe = async (): Promise<OnDeviceBackendReadiness> => {
    if (baseUrl.length === 0) {
      return {
        backend: "gpt_oss",
        available: false,
        model,
        detail: "no GPT-OSS endpoint configured (KHALA_GPT_OSS_BASE_URL)",
      }
    }
    try {
      const response = await fetchFn(`${baseUrl}/models`, {
        headers: { ...authHeaders() },
        signal: AbortSignal.timeout(GPT_OSS_PROBE_TIMEOUT_MS),
      })
      if (!response.ok) {
        return { backend: "gpt_oss", available: false, model, detail: `models ${response.status}` }
      }
      const list = (await response.json()) as OpenAiModelsList
      const ids = (list.data ?? []).map((entry) => str(entry.id, ""))
      const serving = ids.length > 0
      return {
        backend: "gpt_oss",
        available: serving,
        model: ids.includes(model) ? model : str(ids[0], model),
        detail: serving ? `serving ${ids.length} model(s)` : "no models served",
      }
    } catch (error) {
      return {
        backend: "gpt_oss",
        available: false,
        model,
        detail: `endpoint unreachable: ${error instanceof Error ? error.name : "error"}`,
      }
    }
  }

  const complete = async (
    messages: ReadonlyArray<OnDeviceDeciderMessage>,
    completeOptions?: OnDeviceDeciderCompleteOptions,
  ): Promise<OnDeviceDeciderResult> => {
    if (baseUrl.length === 0) throw new Error("gpt_oss endpoint not configured")
    const body: Record<string, unknown> = {
      model,
      messages: messages.map((message) => ({ role: message.role, content: message.content })),
    }
    if (completeOptions?.maxTokens !== undefined) body.max_tokens = completeOptions.maxTokens
    const response = await fetchFn(`${baseUrl}/chat/completions`, {
      method: "POST",
      headers: { "content-type": "application/json", ...authHeaders() },
      body: JSON.stringify(body),
      signal: completeOptions?.signal ?? AbortSignal.timeout(GPT_OSS_COMPLETE_TIMEOUT_MS),
    })
    if (!response.ok) throw new Error(`gpt_oss completion failed: ${response.status}`)
    const json = (await response.json()) as OpenAiChatCompletion
    const usage = json.usage
    return {
      backend: "gpt_oss",
      model: str(json.model, model),
      content: str(json.choices?.[0]?.message?.content, ""),
      usage: {
        promptTokens: num(usage?.prompt_tokens),
        completionTokens: num(usage?.completion_tokens),
        totalTokens: num(usage?.total_tokens),
        truth: "exact",
      },
    }
  }

  return { kind: "gpt_oss", model, probe, complete }
}
