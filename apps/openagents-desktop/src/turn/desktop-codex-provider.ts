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

/**
 * The delegate candidates the host can dispatch as a real subagent (#9091).
 * Codex was first (AFS-04); Claude Code (`claude`) and Grok (`grok_acp`) reuse
 * the SAME kernel provider, projection, and redaction boundary.
 */
export type DelegateCandidate = "codex" | "claude" | "grok_acp"

/** A ref-safe token for each delegate candidate (branded refs disallow `_`). */
const refTokenFor = (candidate: DelegateCandidate): string =>
  candidate === "grok_acp" ? "grok" : candidate

/** The main-owned delegate lane readiness snapshot, derived from lane availability. */
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

export interface DelegateProviderConfig {
  /**
   * The delegate candidate this registry represents. Selects the kernel
   * descriptor identity (`codex`, `claude`, or `grok_acp`) so the shared router
   * can dispatch the right lane. Defaults to `codex` for the AFS-04 call site.
   */
  readonly candidate?: DelegateCandidate
  readonly providerRef?: string
  readonly model?: string
  /** Current delegate lane readiness, from main-owned availability (never the renderer). */
  readonly readiness: () => CodexLaneReadiness | Promise<CodexLaneReadiness>
  /**
   * Run one real delegate turn, streaming the frozen `ClaudeLocalEvent` envelope
   * through `emit`. The host wires this to the lane dispatcher (codex-local,
   * claude-local, or the Grok ACP lane); a test passes a scripted fake. The
   * adapter never selects an account or builds history — the host lane owns that.
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

/** @deprecated Use {@link DelegateProviderConfig}. Retained for the AFS-04 codex call site. */
export type CodexProviderConfig = DelegateProviderConfig

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

/** Build a delegate provider descriptor from main-owned readiness. */
export const makeDelegateDescriptor = (input: {
  readonly candidate: DelegateCandidate
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
    candidate: input.candidate,
    model: input.model,
    placement: "owner_local",
    supportedIntents: ["Ask", "RecommendRoute", "ProposeEdit"],
    supportedCandidateKinds: ["answer"],
    // The delegate lane runs locally but sends turn input to the provider backend.
    dataDestination: "remote_provider",
    // The delegate runtime reports exact provider token usage.
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

/** Build the codex descriptor from main-owned readiness (AFS-04 back-compat wrapper). */
export const makeCodexDescriptor = (input: {
  readonly providerRef: string
  readonly model: string
  readonly readiness: CodexLaneReadiness
}): InferenceProviderDescriptor => makeDelegateDescriptor({ candidate: "codex", ...input })

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
      // Reasoning is ACTIVITY detail, not an agent message. Keeping it off the
      // `assistant` role means the chain's assistant entries are exactly the
      // agent's message text, so the single-delegate promotion (#9127) can pick
      // the streamed/final answer without mistaking a reasoning summary for it.
      return { role: "system", text: event.text }
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

/** The default provider ref for one delegate candidate's owner-local lane. */
const defaultProviderRefFor = (candidate: DelegateCandidate): string =>
  candidate === "codex" ? CODEX_LOCAL_PROVIDER_REF : `provider.${refTokenFor(candidate)}.local`

/** The default descriptor model label for one delegate candidate. */
const defaultModelFor = (candidate: DelegateCandidate): string =>
  candidate === "codex" ? CODEX_LOCAL_MODEL_ID : candidate === "claude" ? "anthropic/claude" : "xai/grok"

/**
 * Create a delegate `ProviderRegistry` interface for one owner-local lane
 * (codex-local, claude-local, or the Grok ACP lane). All three share the same
 * kernel provider shape, redaction boundary, and readiness gate; only the
 * candidate identity, provider ref, and model label differ (#9091).
 */
export const makeDelegateProviderRegistry = (config: DelegateProviderConfig): ProviderRegistryInterface => {
  const candidate = config.candidate ?? "codex"
  const refToken = refTokenFor(candidate)
  const providerRef = config.providerRef ?? defaultProviderRefFor(candidate)
  const model = config.model ?? defaultModelFor(candidate)
  let counter = 0
  const nextId = config.nextId ?? (() => `${(counter += 1)}`)

  const describe = Effect.gen(function* () {
    const readiness = yield* Effect.promise(async () => config.readiness())
    return [makeDelegateDescriptor({ candidate, providerRef, model, readiness })]
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
      candidateRef: decodeCandidateRef(`candidate.${refToken}.${requestRef}`),
      provenance: {
        providerRef: decodeProviderRef(providerRef),
        candidate,
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
      const providerTurnRef = decodeProviderTurnRef(`providerturn.${refToken}.${seed}`)
      const message = startTurnMessage(input)

      const events = Stream.callback<ProviderStreamEvent>((queue) =>
        Effect.gen(function* () {
          // Accumulate the redacted chain; every growth emits the LATEST snapshot.
          const activities: Array<ObservedAgentActivity> = []
          const toolActivityIndexByItemRef = new Map<string, number>()
          // The live streaming assistant answer: text_delta events are coalesced
          // into ONE growing assistant entry so the reply streams token-by-token
          // instead of appearing only at completion. A tool/reasoning activity
          // closes the run so the next text starts a fresh entry.
          let liveAssistantIndex = -1
          let liveAssistantText = ""
          const publishChain = (): void => {
            const entries: ReadonlyArray<SafeMessageChainEntry> = projectSafeMessageChain(
              input.requestRef,
              activities,
            )
            Queue.offerUnsafe(queue, ProviderStreamEvent.Chain({ entries }))
          }
          const recordActivity = (event: ClaudeLocalEvent, activity: ObservedAgentActivity): void => {
            if (event.kind !== "tool_use" && event.kind !== "tool_progress" && event.kind !== "tool_result") {
              activities.push(activity)
              return
            }
            const itemRef = event.itemRef
            let activityIndex = itemRef === undefined ? undefined : toolActivityIndexByItemRef.get(itemRef)
            if (activityIndex === undefined && event.kind !== "tool_use") {
              for (let cursor = activities.length - 1; cursor >= 0; cursor -= 1) {
                const candidate = activities[cursor]
                if (candidate?.role === "tool" && candidate.toolLabel === activity.toolLabel &&
                    candidate.commandOutputByteCount === undefined) {
                  activityIndex = cursor
                  break
                }
              }
            }
            if (activityIndex === undefined) {
              activityIndex = activities.length
              activities.push(activity)
            } else {
              activities[activityIndex] = { ...activities[activityIndex], ...activity }
            }
            if (itemRef !== undefined) toolActivityIndexByItemRef.set(itemRef, activityIndex)
          }
          const emit = (event: ClaudeLocalEvent): void => {
            if (event.kind === "text_delta") {
              liveAssistantText += event.text
              const entry: ObservedAgentActivity = { role: "assistant", text: liveAssistantText }
              if (liveAssistantIndex === -1) {
                liveAssistantIndex = activities.length
                activities.push(entry)
              } else {
                activities[liveAssistantIndex] = entry
              }
              Queue.offerUnsafe(queue, ProviderStreamEvent.Progress())
              publishChain()
              return
            }
            const activity = redactCodexEvent(event)
            if (activity === null) {
              Queue.offerUnsafe(queue, ProviderStreamEvent.Progress())
              return
            }
            liveAssistantIndex = -1
            liveAssistantText = ""
            recordActivity(event, activity)
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
              Effect.succeed<CodexLaneTurnResult>({ ok: false, reason: "session_failed", detail: "delegate lane stopped" }),
            ),
          )

          if (result.ok) {
            // Finalize the assistant answer. If it streamed, replace the live
            // entry with the authoritative final text (no duplicate). Otherwise
            // append it as the last chain entry.
            const finalEntry: ObservedAgentActivity = { role: "assistant", text: result.text }
            if (liveAssistantIndex === -1) {
              activities.push(finalEntry)
            } else {
              activities[liveAssistantIndex] = finalEntry
            }
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

/**
 * Create the codex-local delegate `ProviderRegistry` (AFS-04 back-compat alias
 * for {@link makeDelegateProviderRegistry} with the `codex` candidate).
 */
export const makeCodexProviderRegistry = (config: CodexProviderConfig): ProviderRegistryInterface =>
  makeDelegateProviderRegistry({ candidate: "codex", ...config })
