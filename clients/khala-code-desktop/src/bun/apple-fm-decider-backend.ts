// Apple Foundation Models backend for the on-device decider.
//
// It speaks the loopback OpenAI-compatible contract exposed by the
// `foundation-bridge` helper (GET /health, POST /v1/chat/completions). Optionally
// it is given the Apple FM sidecar host so a probe also nudges the helper to
// launch (in packaged/adopted contexts). Readiness here is the DIRECT helper
// health — distinct from `apple-fm-readiness`'s Pylon-supervised "ready" state,
// which gates product-promise evidence. The decider only needs the helper to be
// reachable and reporting `ready`.

import {
  APPLE_FM_MODEL_ID,
  type KhalaAppleFmReadiness,
} from "../shared/apple-fm-readiness.js"
import type {
  OnDeviceBackendReadiness,
  OnDeviceDeciderBackend,
  OnDeviceDeciderCompleteOptions,
  OnDeviceDeciderMessage,
  OnDeviceDeciderResult,
} from "../shared/on-device-decider.js"

const APPLE_FM_BRIDGE_DEFAULT_BASE_URL = "http://127.0.0.1:11435"
const APPLE_FM_PROBE_TIMEOUT_MS = 2_000
const APPLE_FM_COMPLETE_TIMEOUT_MS = 60_000

type AppleFmHealth = {
  readonly ready?: unknown
  readonly model?: unknown
  readonly modelId?: unknown
  readonly message?: unknown
}

type AppleFmChatCompletion = {
  readonly choices?: ReadonlyArray<{
    readonly message?: { readonly content?: unknown }
  }>
  readonly model?: unknown
  readonly usage?: {
    readonly promptTokens?: unknown
    readonly completionTokens?: unknown
    readonly totalTokens?: unknown
    readonly truth?: unknown
  }
}

const num = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value) ? value : 0

const str = (value: unknown, fallback: string): string =>
  typeof value === "string" && value.length > 0 ? value : fallback

export type AppleFmDeciderBackendOptions = {
  /** Helper base URL. Defaults to the bridge loopback origin. */
  readonly baseUrl?: string
  /** Injected fetch (tests). */
  readonly fetchFn?: typeof fetch
  /**
   * Optional sidecar host. When provided, `probe` calls its `readiness()` first
   * so the helper is launched/adopted before the health check (no-op when the
   * sidecar declines to launch in bare dev).
   */
  readonly sidecar?: { readiness(): Promise<KhalaAppleFmReadiness> }
}

export function createAppleFmDeciderBackend(
  options: AppleFmDeciderBackendOptions = {},
): OnDeviceDeciderBackend {
  const baseUrl = (options.baseUrl ?? APPLE_FM_BRIDGE_DEFAULT_BASE_URL).replace(/\/+$/, "")
  const fetchFn = options.fetchFn ?? fetch

  const probe = async (): Promise<OnDeviceBackendReadiness> => {
    // Nudge the helper to launch if a sidecar is wired (ignore its verdict; we
    // trust the direct /health below as the decider's source of truth).
    if (options.sidecar !== undefined) {
      try {
        await options.sidecar.readiness()
      } catch {
        /* sidecar is best-effort */
      }
    }
    try {
      const response = await fetchFn(`${baseUrl}/health`, {
        signal: AbortSignal.timeout(APPLE_FM_PROBE_TIMEOUT_MS),
      })
      if (!response.ok) {
        return {
          backend: "apple_fm",
          available: false,
          model: APPLE_FM_MODEL_ID,
          detail: `helper health ${response.status}`,
        }
      }
      const health = (await response.json()) as AppleFmHealth
      const ready = health.ready === true
      return {
        backend: "apple_fm",
        available: ready,
        model: str(health.model ?? health.modelId, APPLE_FM_MODEL_ID),
        detail: ready
          ? str(health.message, "Apple Foundation Models available")
          : str(health.message, "Apple Foundation Models not ready"),
      }
    } catch (error) {
      return {
        backend: "apple_fm",
        available: false,
        model: APPLE_FM_MODEL_ID,
        detail: `helper unreachable: ${error instanceof Error ? error.name : "error"}`,
      }
    }
  }

  const complete = async (
    messages: ReadonlyArray<OnDeviceDeciderMessage>,
    completeOptions?: OnDeviceDeciderCompleteOptions,
  ): Promise<OnDeviceDeciderResult> => {
    const body: Record<string, unknown> = {
      model: APPLE_FM_MODEL_ID,
      messages: messages.map((message) => ({ role: message.role, content: message.content })),
    }
    if (completeOptions?.maxTokens !== undefined) body.max_tokens = completeOptions.maxTokens
    const response = await fetchFn(`${baseUrl}/v1/chat/completions`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
      signal: completeOptions?.signal ?? AbortSignal.timeout(APPLE_FM_COMPLETE_TIMEOUT_MS),
    })
    if (!response.ok) {
      throw new Error(`apple_fm completion failed: ${response.status}`)
    }
    const json = (await response.json()) as AppleFmChatCompletion
    const content = str(json.choices?.[0]?.message?.content, "")
    const usage = json.usage
    return {
      backend: "apple_fm",
      model: str(json.model, APPLE_FM_MODEL_ID),
      content,
      usage: {
        promptTokens: num(usage?.promptTokens),
        completionTokens: num(usage?.completionTokens),
        totalTokens: num(usage?.totalTokens),
        truth: usage?.truth === "exact" ? "exact" : "estimated",
      },
    }
  }

  return { kind: "apple_fm", model: APPLE_FM_MODEL_ID, probe, complete }
}
