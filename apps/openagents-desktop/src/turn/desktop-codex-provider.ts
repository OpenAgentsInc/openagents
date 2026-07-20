import { Effect, Queue, Schema as S, Stream } from "effect"

import {
  CANDIDATE_SCHEMA_LITERAL,
  CandidateRef,
  InferenceProviderDescriptor,
  MAX_TURN_CONTEXT_CHARS,
  MAX_TURN_OUTPUT_CHARS,
  PROVIDER_SCHEMA_LITERAL,
  ProviderTurnRef,
  TurnProviderRef,
  type ProviderReadiness,
  type SafeMessageChainEntry,
  type TurnCandidate,
} from "@openagentsinc/agent-runtime-schema"
import {
  ProviderStartError,
  ProviderStreamEvent,
  type ProviderRegistryInterface,
  type ProviderRun,
  type ProviderStartInput,
} from "@openagentsinc/agent-turn-runtime"
import { projectSafeMessageChain, type ObservedAgentActivity } from "@openagentsinc/agent-surface"

import type { ClaudeLocalEvent } from "../claude-local-contract.ts"

/**
 * AFS-04 Desktop codex-local kernel provider.
 *
 * The Apple FM router can recommend delegating a turn to Codex. This adapter
 * registers the desktop-native `codex-local` lane as ONE kernel inference
 * provider, so the shared `TurnService` drives the real Codex turn, folds its
 * lifecycle into the safe turn projection, and produces the subagent card and
 * the right-pane message chain.
 *
 * The adapter is the REDACTION BOUNDARY. It observes the frozen `ClaudeLocalEvent`
 * stream the codex runtime emits and projects it into a bounded, owner-only safe
 * message chain that carries ONLY labels and counts: an agent message, a
 * reasoning summary, a tool LABEL, a file-change COUNT, and a command-output
 * BYTE COUNT. It never reads a raw command argument, raw command output, a local
 * path, a diff, a token, or a secret. The raw event stream and the raw workbench
 * item payloads stay on the device; only the redacted chain crosses into the
 * projection.
 *
 * Readiness and account state come from MAIN-OWNED codex availability, never from
 * renderer input. An unavailable, unauthenticated, or unadmitted codex lane fails
 * `start` with a typed `ProviderStartError` and produces NO provider start.
 *
 * The lane execution is injected structurally (`runTurn`) so this module is
 * unit-testable without Electron, the codex CLI, or a real account.
 */
export const CODEX_LOCAL_PROVIDER_REF = "provider.codex.local" as const
export const CODEX_LOCAL_MODEL_ID = "openai/codex" as const

/** The main-owned codex lane readiness snapshot, derived from `codexLocal.availability()`. */
export interface CodexLaneReadiness {
  readonly ready: boolean
  /** The health-ordered verified account ref (never a path), when ready. */
  readonly accountRef?: string
  /** Why the lane cannot run, when not ready. */
  readonly unavailableReason?:
    | "no_codex_account"
    | "no_verified_account"
    | "policy_denied"
    | "quota_exhausted"
    | "rate_limited"
    | "invalid_config"
    | "not_ready"
}

/** The bounded terminal result of one codex lane run. */
export type CodexLaneTurnResult =
  | { readonly ok: true; readonly text: string }
  | { readonly ok: false; readonly reason: string; readonly detail: string }

export interface CodexProviderConfig {
  readonly providerRef?: string
  readonly model?: string
  /** Current codex lane readiness, from main-owned availability (never the renderer). */
  readonly readiness: () => CodexLaneReadiness | Promise<CodexLaneReadiness>
  /**
   * Run one real codex turn, streaming the frozen `ClaudeLocalEvent` envelope
   * through `emit`. The host wires this to the codex-local lane dispatcher; a
   * test passes a scripted fake. The adapter never selects an account or builds
   * history — the host lane owns that.
   */
  readonly runTurn: (input: {
    readonly requestRef: string
    readonly threadRef: string
    readonly message: string
    readonly emit: (event: ClaudeLocalEvent) => void
  }) => Promise<CodexLaneTurnResult>
  /** Deterministic id suffix source (tests inject a counter). */
  readonly nextId?: () => string
}

const decodeProviderRef = S.decodeUnknownSync(TurnProviderRef)
const decodeProviderTurnRef = S.decodeUnknownSync(ProviderTurnRef)
const decodeCandidateRef = S.decodeUnknownSync(CandidateRef)

/** Map main-owned codex unavailability into the frozen provider readiness reason. */
const readinessReasonMap = (
  reason: CodexLaneReadiness["unavailableReason"],
): ProviderReadiness => {
  switch (reason) {
    case "no_codex_account":
      return { state: "unavailable", reason: "account_missing" }
    case "no_verified_account":
      return { state: "unavailable", reason: "account_unhealthy" }
    case "policy_denied":
      return { state: "unavailable", reason: "permission_denied" }
    case "quota_exhausted":
    case "rate_limited":
    case "invalid_config":
    case "not_ready":
    case undefined:
      return { state: "unavailable", reason: "not_ready" }
  }
}

/** Map a not-ready readiness into a typed provider-start refusal. */
const startErrorReason = (
  reason: CodexLaneReadiness["unavailableReason"],
): ProviderStartError["reason"] => {
  switch (reason) {
    case "no_codex_account":
    case "no_verified_account":
      return "unauthorized"
    case "policy_denied":
      return "unadmitted"
    case "quota_exhausted":
    case "rate_limited":
    case "invalid_config":
    case "not_ready":
    case undefined:
      return "unavailable"
  }
}

/** Build the codex descriptor from main-owned readiness. */
export const makeCodexDescriptor = (input: {
  readonly providerRef: string
  readonly model: string
  readonly readiness: CodexLaneReadiness
}): InferenceProviderDescriptor => {
  const readiness: ProviderReadiness = input.readiness.ready
    ? { state: "ready" }
    : readinessReasonMap(input.readiness.unavailableReason)
  return {
    schema: PROVIDER_SCHEMA_LITERAL,
    providerRef: decodeProviderRef(input.providerRef),
    candidate: "codex",
    model: input.model,
    placement: "owner_local",
    supportedIntents: ["Ask", "RecommendRoute", "ProposeEdit"],
    supportedCandidateKinds: ["answer"],
    // Codex runs locally but sends turn input to the provider backend.
    dataDestination: "remote_provider",
    // The codex runtime reports exact provider token usage.
    usageTruth: "exact",
    costClass: "metered_provider_tokens",
    maxContextChars: MAX_TURN_CONTEXT_CHARS,
    maxOutputChars: MAX_TURN_OUTPUT_CHARS,
    supportsStreaming: true,
    supportsCancellation: true,
    supportsExternalTools: true,
    supportsExternalActions: true,
    readiness,
  }
}

/** Read ONLY the file-change COUNT from a workbench item; never a path or diff. */
const fileChangeCountOf = (item: unknown): number | undefined => {
  if (typeof item !== "object" || item === null) return undefined
  const record = item as { readonly kind?: unknown; readonly changes?: unknown }
  if (record.kind !== "fileChange") return undefined
  return Array.isArray(record.changes) ? record.changes.length : 0
}

/** The byte COUNT of a bounded output summary; the summary string itself is discarded. */
const byteCountOf = (value: string): number => {
  let count = 0
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    count += code < 0x80 ? 1 : code < 0x800 ? 2 : 3
  }
  return count
}

/**
 * Redact one codex `ClaudeLocalEvent` into a safe observed activity, or `null`
 * when the event carries nothing safe to show. It reads only agent/reasoning
 * text and tool labels/counts. Command text, output text, paths, diffs, args,
 * prompts, responses, and tokens are never read.
 */
export const redactCodexEvent = (event: ClaudeLocalEvent): ObservedAgentActivity | null => {
  switch (event.kind) {
    case "reasoning":
      return { role: "assistant", text: event.text }
    case "tool_use":
    case "tool_progress": {
      const fileChangeCount = fileChangeCountOf((event as { readonly item?: unknown }).item)
      return {
        role: "tool",
        toolLabel: event.toolName,
        ...(fileChangeCount === undefined ? {} : { fileChangeCount }),
      }
    }
    case "tool_result": {
      const fileChangeCount = fileChangeCountOf((event as { readonly item?: unknown }).item)
      // `summary` is the redacted output tail; take only its byte count.
      const summary = typeof event.summary === "string" ? event.summary : ""
      return {
        role: "tool",
        toolLabel: event.toolName,
        commandOutputByteCount: byteCountOf(summary),
        ...(fileChangeCount === undefined ? {} : { fileChangeCount }),
      }
    }
    default:
      return null
  }
}

/** Create the codex-local `ProviderRegistry` interface for one owner-local lane. */
export const makeCodexProviderRegistry = (config: CodexProviderConfig): ProviderRegistryInterface => {
  const providerRef = config.providerRef ?? CODEX_LOCAL_PROVIDER_REF
  const model = config.model ?? CODEX_LOCAL_MODEL_ID
  let counter = 0
  const nextId = config.nextId ?? (() => `${(counter += 1)}`)

  const describe = Effect.gen(function* () {
    const readiness = yield* Effect.promise(async () => config.readiness())
    return [makeCodexDescriptor({ providerRef, model, readiness })]
  })

  const startTurnMessage = (input: ProviderStartInput): string => {
    const intent = input.intent
    if (intent._tag === "Ask") return intent.text
    if (intent._tag === "RecommendRoute") return intent.objective
    if (intent._tag === "ProposeEdit") return intent.instruction
    return "Continue the delegated turn."
  }

  const answerCandidate = (requestRef: string, text: string): TurnCandidate => {
    const trimmed = text.trim()
    return {
      schema: CANDIDATE_SCHEMA_LITERAL,
      kind: "answer",
      candidateRef: decodeCandidateRef(`candidate.codex.${requestRef}`),
      provenance: {
        providerRef: decodeProviderRef(providerRef),
        candidate: "codex",
        model,
        taskClass: "delegate",
        usageTruth: "exact",
        dataDestination: "remote_provider",
        stale: false,
      },
      text: (trimmed === "" ? "(no answer text)" : trimmed).slice(0, MAX_TURN_OUTPUT_CHARS),
    }
  }

  const start = (input: ProviderStartInput): Effect.Effect<ProviderRun, ProviderStartError> =>
    Effect.gen(function* () {
      // Main-owned readiness gate. No start on an unavailable/unauthenticated/
      // unadmitted lane; the renderer never supplies this.
      const readiness = yield* Effect.promise(async () => config.readiness())
      if (!readiness.ready) {
        return yield* Effect.fail(new ProviderStartError({ reason: startErrorReason(readiness.unavailableReason) }))
      }

      const seed = nextId()
      const providerTurnRef = decodeProviderTurnRef(`providerturn.codex.${seed}`)
      const message = startTurnMessage(input)

      const events = Stream.callback<ProviderStreamEvent>((queue) =>
        Effect.gen(function* () {
          // Accumulate the redacted chain; every growth emits the LATEST snapshot.
          const activities: Array<ObservedAgentActivity> = []
          const publishChain = (): void => {
            const entries: ReadonlyArray<SafeMessageChainEntry> = projectSafeMessageChain(
              input.requestRef,
              activities,
            )
            Queue.offerUnsafe(queue, ProviderStreamEvent.Chain({ entries }))
          }
          const emit = (event: ClaudeLocalEvent): void => {
            const activity = redactCodexEvent(event)
            if (activity === null) {
              Queue.offerUnsafe(queue, ProviderStreamEvent.Progress())
              return
            }
            activities.push(activity)
            Queue.offerUnsafe(queue, ProviderStreamEvent.Progress())
            publishChain()
          }

          const result: CodexLaneTurnResult = yield* Effect.tryPromise(() =>
            config.runTurn({
              requestRef: input.requestRef,
              threadRef: input.threadRef,
              message,
              emit,
            }),
          ).pipe(
            Effect.catch(() =>
              Effect.succeed<CodexLaneTurnResult>({ ok: false, reason: "session_failed", detail: "codex lane stopped" }),
            ),
          )

          if (result.ok) {
            // Append the final assistant answer as the last chain entry.
            activities.push({ role: "assistant", text: result.text })
            publishChain()
            Queue.offerUnsafe(queue, ProviderStreamEvent.Completed({ candidate: answerCandidate(input.requestRef, result.text) }))
          } else {
            Queue.offerUnsafe(queue, ProviderStreamEvent.Failed({ detail: result.reason }))
          }
          yield* Queue.end(queue)
        }),
      )

      const run: ProviderRun = { providerTurnRef, events }
      return run
    })

  return { describe, start }
}
