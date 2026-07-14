// Optional on-device "decider": a small, local model Khala Code desktop can use
// to make fast, private routing/triage/yes-no decisions without a network round
// trip. It chooses a platform-appropriate backend behind ONE uniform contract:
//
//   - Apple Foundation Models  -> Apple-Silicon macOS (and, later, iOS)
//   - self-hosted GPT-OSS      -> everywhere else (the non-Mac drop-in)
//
// It is OPTIONAL and FAILS SOFT. If no backend is configured/available the
// decider reports unavailable (and `decide` throws `OnDeviceDeciderUnavailable`)
// so callers transparently fall back to their normal cloud path. The decider
// never becomes a hard dependency and never blocks the app.
//
// This module is pure/transport-agnostic: backends are injected and implement
// `OnDeviceDeciderBackend`. The concrete Apple FM and GPT-OSS backends (HTTP
// clients) live under `src/bun/`. Tests drive the selection logic with fakes.

export const ON_DEVICE_DECIDER_SCHEMA =
  "openagents.khala_code.on_device_decider.v0.1" as const

export type OnDeviceBackendKind = "apple_fm" | "gpt_oss"

export type OnDeviceDeciderPlatform = {
  /** Node/Bun `process.platform`, e.g. "darwin" | "linux" | "win32" | "ios". */
  readonly platform: string
  /** Node/Bun `process.arch`, e.g. "arm64" | "x64". */
  readonly arch: string
}

export type OnDeviceDeciderRole = "system" | "user" | "assistant"

export type OnDeviceDeciderMessage = {
  readonly role: OnDeviceDeciderRole
  readonly content: string
}

export type OnDeviceDeciderUsage = {
  readonly promptTokens: number
  readonly completionTokens: number
  readonly totalTokens: number
  readonly truth: "exact" | "estimated"
}

export type OnDeviceBackendReadiness = {
  readonly backend: OnDeviceBackendKind
  readonly available: boolean
  readonly model: string
  /** Short, public-safe reason/detail. Never raw prompts or secrets. */
  readonly detail: string
}

export type OnDeviceDeciderResult = {
  readonly backend: OnDeviceBackendKind
  readonly model: string
  readonly content: string
  readonly usage: OnDeviceDeciderUsage
}

export type OnDeviceDeciderCompleteOptions = {
  readonly maxTokens?: number
  readonly signal?: AbortSignal
}

// The uniform backend contract every on-device backend implements. `probe` must
// never throw (it returns an unavailable readiness on error); `complete` may
// throw on transport failure and the decider treats that as backend-unavailable.
export type OnDeviceDeciderBackend = {
  readonly kind: OnDeviceBackendKind
  readonly model: string
  probe(): Promise<OnDeviceBackendReadiness>
  complete(
    messages: ReadonlyArray<OnDeviceDeciderMessage>,
    options?: OnDeviceDeciderCompleteOptions,
  ): Promise<OnDeviceDeciderResult>
}

export type OnDeviceDeciderSelection = {
  /** The backend that will be used, or null when none are available. */
  readonly selected: OnDeviceBackendKind | null
  /** What this platform prefers, independent of availability. */
  readonly preferred: OnDeviceBackendKind
  /** Public-safe explanation of the selection. */
  readonly reason: string
  /** Readiness for every configured backend, in evaluation order. */
  readonly readiness: ReadonlyArray<OnDeviceBackendReadiness>
}

export class OnDeviceDeciderUnavailable extends Error {
  readonly selection: OnDeviceDeciderSelection
  constructor(selection: OnDeviceDeciderSelection) {
    super(
      `on-device decider unavailable (preferred=${selection.preferred}); ` +
        `fall back to the cloud path`,
    )
    this.name = "OnDeviceDeciderUnavailable"
    this.selection = selection
  }
}

// Platform selection CONTRACT — the heart of "how to select between the tools".
// Apple Silicon macOS and iOS prefer Apple FM (true on-device, no network, most
// private). Everything else prefers the self-hosted GPT-OSS drop-in. This is a
// pure function so the policy is testable and identical across host + view.
export function preferredOnDeviceBackend(
  platform: OnDeviceDeciderPlatform,
): OnDeviceBackendKind {
  const appleSilicon = platform.platform === "darwin" && platform.arch === "arm64"
  const ios = platform.platform === "ios"
  return appleSilicon || ios ? "apple_fm" : "gpt_oss"
}

// Evaluation order: the preferred backend first, then the other as a fallback.
export function backendSelectionOrder(
  preferred: OnDeviceBackendKind,
): ReadonlyArray<OnDeviceBackendKind> {
  return preferred === "apple_fm" ? ["apple_fm", "gpt_oss"] : ["gpt_oss", "apple_fm"]
}

export type OnDeviceDecider = {
  /**
   * Probe configured backends in preference order and pick the first available.
   * Pure status — does not run inference. Safe to call from a status RPC.
   */
  select(): Promise<OnDeviceDeciderSelection>
  /**
   * Run a single decision/inference on the selected backend. Throws
   * `OnDeviceDeciderUnavailable` (carrying the selection) when no backend is
   * available so the caller can fall back to its normal path.
   */
  decide(
    messages: ReadonlyArray<OnDeviceDeciderMessage>,
    options?: OnDeviceDeciderCompleteOptions,
  ): Promise<OnDeviceDeciderResult>
}

const unavailableReadiness = (
  kind: OnDeviceBackendKind,
  detail: string,
): OnDeviceBackendReadiness => ({ backend: kind, available: false, model: kind, detail })

export function createOnDeviceDecider(input: {
  readonly platform: OnDeviceDeciderPlatform
  readonly backends: Partial<Record<OnDeviceBackendKind, OnDeviceDeciderBackend>>
}): OnDeviceDecider {
  const preferred = preferredOnDeviceBackend(input.platform)
  const order = backendSelectionOrder(preferred)

  const probeAll = async (): Promise<
    ReadonlyArray<{ kind: OnDeviceBackendKind; readiness: OnDeviceBackendReadiness }>
  > => {
    const out: Array<{ kind: OnDeviceBackendKind; readiness: OnDeviceBackendReadiness }> = []
    for (const kind of order) {
      const backend = input.backends[kind]
      if (backend === undefined) {
        out.push({ kind, readiness: unavailableReadiness(kind, "backend not configured") })
        continue
      }
      try {
        out.push({ kind, readiness: await backend.probe() })
      } catch (error) {
        out.push({
          kind,
          readiness: unavailableReadiness(
            kind,
            `probe failed: ${error instanceof Error ? error.name : "error"}`,
          ),
        })
      }
    }
    return out
  }

  const select = async (): Promise<OnDeviceDeciderSelection> => {
    const probed = await probeAll()
    const winner = probed.find((entry) => entry.readiness.available)
    const readiness = probed.map((entry) => entry.readiness)
    if (winner === undefined) {
      return {
        selected: null,
        preferred,
        reason: "no configured backend is available",
        readiness,
      }
    }
    const reason =
      winner.kind === preferred
        ? `preferred backend ${preferred} is available`
        : `preferred backend ${preferred} unavailable; fell back to ${winner.kind}`
    return { selected: winner.kind, preferred, reason, readiness }
  }

  const decide = async (
    messages: ReadonlyArray<OnDeviceDeciderMessage>,
    options?: OnDeviceDeciderCompleteOptions,
  ): Promise<OnDeviceDeciderResult> => {
    const selection = await select()
    if (selection.selected === null) throw new OnDeviceDeciderUnavailable(selection)
    const backend = input.backends[selection.selected]
    if (backend === undefined) throw new OnDeviceDeciderUnavailable(selection)
    return backend.complete(messages, options)
  }

  return { select, decide }
}
