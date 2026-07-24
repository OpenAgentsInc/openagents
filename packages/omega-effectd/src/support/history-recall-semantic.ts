/**
 * RLM-05 (#9141) — Tier S semantic recall behind the `HistoryRecall` /
 * `history_recall` host-tool seam, running the first-class Effect-native
 * `@openagentsinc/rlm` engine over the same authorized desktop corpus.
 *
 * Tier policy (audit §5.2, §5.4): Tier D is the default and always runs
 * first — it is free, deterministic, and always available. Tier S runs ONLY
 * after host-owned admission plus an explicit escalation basis:
 *
 * - an explicit caller (application/user) tier request, or
 * - an insufficient Tier D answer combined with caller opt-in.
 *
 * A model can NEVER self-authorize Tier S: tool-call arguments are not
 * consulted for tier selection, and the semantic-capable `Rlm` layer is only
 * constructed when a host admission record exists. Without admission the
 * engine returns the typed `Refused` terminal (`semantic_not_admitted`).
 *
 * Honesty and spend boundaries:
 * - `requireExactUsage` is forced on for the first rollout. A model call
 *   whose provider response lacks exact token counts fails typed as
 *   `usage_required_but_unavailable` — usage is unavailable, never zero, and
 *   that provider is ineligible for admitted multi-call semantic recall.
 * - Every completed model call is recorded idempotently under the key
 *   `rlm:<runRef>:<callRef>` (contract retries get fresh callRefs and their
 *   own rows). Rows project into the existing session usage ledger.
 * - Budgets clamp DOWNWARD to finite desktop ceilings; initial rollout depth
 *   is at most one and there is no artifact sink.
 * - Citations come from the SDK citation machinery and are already validated
 *   against the exact corpus digest by the engine; this module only decodes
 *   the durable history cursor addresses for the renderer rows.
 * - The corpus mount honors the host visibility/redaction policy — the same
 *   `buildHistoryCorpus` boundary Tier D uses (nothing semantic widens it).
 */

import { Effect, Stream } from "effect"
import { LanguageModel } from "effect/unstable/ai"
import type { AiError } from "effect/unstable/ai"
import type {
  HistoryCorpusScope,
  HistoryRecallResponse,
} from "@openagentsinc/history-corpus"
import {
  defaultRlmBudget,
  defaultRlmEvidencePolicy,
  makeRlm,
  RlmError,
  type RlmBudget,
  type RlmEvent,
  type RlmModelPlan,
  type RlmSemanticRequest,
  type RlmShape,
  type RlmTerminalResult,
} from "@openagentsinc/rlm"

import {
  DESKTOP_RLM_STRATEGY_REF,
  decodeHistoryCursorAddress,
  desktopHistoryCorpusInputForScope,
  desktopHistoryCorpusSourceLayer,
  desktopRlmRootLimits,
} from "./desktop-history-corpus-source.ts"
import {
  dispatchDesktopHistoryRecall,
  toDesktopHistoryCorpusSourceInput,
  type HistoryRecallCitedSpanRow,
  type HistoryRecallHostDispatchInput,
  type HistoryRecallHostDispatchResult,
  type HistoryRecallHostSources,
} from "./history-recall-host.ts"
import type { UsageLedgerRecordInput } from "./usage-ledger.ts"
import type { UsageLedgerProvider } from "./usage-ledger-contract.ts"

// ---------------------------------------------------------------------------
// Host-owned semantic admission.
// ---------------------------------------------------------------------------

/** Who admitted Tier S. Both are host-side facts, never model output. */
export type DesktopSemanticRecallAdmissionBasis =
  | "user_explicit"
  | "application_policy"

/**
 * Host-owned admission record for semantic recall. Constructed by the
 * application (an explicit user action or an approved product interaction) —
 * never from tool-call arguments and never from model text.
 */
export interface DesktopSemanticRecallAdmission {
  readonly admitted: true
  readonly basis: DesktopSemanticRecallAdmissionBasis
  /** Bounded host-side receipt ref naming the admitting interaction. */
  readonly grantRef: string
}

// ---------------------------------------------------------------------------
// Tier selection policy — D first; S only on explicit escalation.
// ---------------------------------------------------------------------------

/** Caller-supplied (application-side) tier request. Never read from model args. */
export interface DesktopRecallTierRequest {
  readonly requestedTier?: "deterministic" | "semantic"
  /**
   * Escalate to Tier S when the Tier D answer is insufficient. Still
   * requires a host admission record — opt-in alone admits nothing.
   */
  readonly escalateOnInsufficient?: boolean
}

export type DesktopRecallTierDecision =
  | { readonly tier: "deterministic" }
  | {
      readonly tier: "semantic"
      readonly basis: "explicit_request" | "insufficient_deterministic"
      readonly admission: DesktopSemanticRecallAdmission
    }
  | {
      readonly tier: "semantic_refused"
      readonly reason: "not_admitted"
    }

/**
 * Whether a Tier D response is insufficient for escalation purposes: it
 * produced zero cited spans. A truncated-but-cited answer is still evidence
 * and does not auto-escalate.
 */
export const deterministicRecallInsufficient = (
  response: HistoryRecallResponse,
): boolean => response.answers.length === 0

/**
 * Select the execution tier. Deterministic is the default; semantic requires
 * BOTH an escalation basis (explicit request, or insufficient Tier D plus
 * caller opt-in) AND a host admission record. A missing admission with a
 * semantic escalation basis is a typed refusal, never a silent downgrade.
 */
export const selectDesktopRecallTier = (input: {
  readonly request?: DesktopRecallTierRequest | undefined
  readonly admission?: DesktopSemanticRecallAdmission | null | undefined
  readonly deterministicResponse?: HistoryRecallResponse | null | undefined
}): DesktopRecallTierDecision => {
  const admission = input.admission ?? null
  if (input.request?.requestedTier === "semantic") {
    return admission === null
      ? { tier: "semantic_refused", reason: "not_admitted" }
      : { tier: "semantic", basis: "explicit_request", admission }
  }
  const deterministic = input.deterministicResponse ?? null
  if (
    input.request?.escalateOnInsufficient === true &&
    deterministic !== null &&
    deterministicRecallInsufficient(deterministic)
  ) {
    return admission === null
      ? { tier: "semantic_refused", reason: "not_admitted" }
      : { tier: "semantic", basis: "insufficient_deterministic", admission }
  }
  return { tier: "deterministic" }
}

// ---------------------------------------------------------------------------
// Budget ceilings — clamp downward, depth at most one, exact usage required.
// ---------------------------------------------------------------------------

/**
 * Finite desktop ceilings for the first Tier S rollout. Depth is at most one
 * (audit §5.1 depth policy), there is no artifact sink, and exact usage is
 * mandatory. Environment/value clamps mirror {@link desktopRlmRootLimits}.
 */
export const desktopRlmSemanticBudgetCeilings: RlmBudget = {
  ...defaultRlmBudget,
  maxDepth: 1,
  maxIterationsPerLoop: 8,
  maxModelCalls: 16,
  timeoutMs: 60_000,
  maxSubcalls: 16,
  maxProgramNodesPerIteration: desktopRlmRootLimits.maxProgramNodesPerIteration,
  maxProgramNodes: desktopRlmRootLimits.maxProgramNodes,
  maxFanOut: desktopRlmRootLimits.maxFanOut,
  maxFanIn: desktopRlmRootLimits.maxFanIn,
  maxConcurrentCalls: desktopRlmRootLimits.maxConcurrentCalls,
  maxValues: desktopRlmRootLimits.maxValues,
  maxCollectionItems: desktopRlmRootLimits.maxItemsPerValue,
  maxEnvironmentBytes: desktopRlmRootLimits.maxEnvironmentBytes,
  maxInlineOutputBytes: desktopRlmRootLimits.maxInlineOutputBytes,
  maxArtifactOutputBytes: 0,
  requireExactUsage: true,
}

/**
 * Clamp a requested semantic budget DOWNWARD to the desktop ceilings. A
 * request can narrow the budget, never widen it. `requireExactUsage` is
 * always true and the artifact sink stays disabled regardless of the request.
 */
export const clampDesktopSemanticBudget = (
  requested?: Partial<RlmBudget>,
): RlmBudget => {
  const ceil = desktopRlmSemanticBudgetCeilings
  const pos = (value: number | undefined, ceiling: number): number =>
    Math.max(1, Math.min(ceiling, Math.floor(value ?? ceiling)))
  const nonNeg = (value: number | undefined, ceiling: number): number =>
    Math.max(0, Math.min(ceiling, Math.floor(value ?? ceiling)))
  return {
    maxDepth: nonNeg(requested?.maxDepth, ceil.maxDepth),
    maxIterationsPerLoop: pos(requested?.maxIterationsPerLoop, ceil.maxIterationsPerLoop),
    maxModelCalls: pos(requested?.maxModelCalls, ceil.maxModelCalls),
    timeoutMs: pos(requested?.timeoutMs, ceil.timeoutMs),
    maxInputTokens: pos(requested?.maxInputTokens, ceil.maxInputTokens),
    maxOutputTokens: pos(requested?.maxOutputTokens, ceil.maxOutputTokens),
    maxTotalTokens: pos(requested?.maxTotalTokens, ceil.maxTotalTokens),
    maxSubcalls: nonNeg(requested?.maxSubcalls, ceil.maxSubcalls),
    maxProgramNodesPerIteration: pos(
      requested?.maxProgramNodesPerIteration,
      ceil.maxProgramNodesPerIteration,
    ),
    maxProgramNodes: pos(requested?.maxProgramNodes, ceil.maxProgramNodes),
    maxFanOut: pos(requested?.maxFanOut, ceil.maxFanOut),
    maxFanIn: pos(requested?.maxFanIn, ceil.maxFanIn),
    maxConcurrentCalls: pos(requested?.maxConcurrentCalls, ceil.maxConcurrentCalls),
    maxValues: pos(requested?.maxValues, ceil.maxValues),
    maxCollectionItems: pos(requested?.maxCollectionItems, ceil.maxCollectionItems),
    maxValueBytes: pos(requested?.maxValueBytes, ceil.maxValueBytes),
    maxEnvironmentBytes: pos(requested?.maxEnvironmentBytes, ceil.maxEnvironmentBytes),
    maxInlineOutputBytes: pos(requested?.maxInlineOutputBytes, ceil.maxInlineOutputBytes),
    maxArtifactOutputBytes: 0,
    maxPromptTokensPerCall: pos(
      requested?.maxPromptTokensPerCall,
      ceil.maxPromptTokensPerCall,
    ),
    maxOutputTokensPerCall: pos(
      requested?.maxOutputTokensPerCall,
      ceil.maxOutputTokensPerCall,
    ),
    maxObservationChars: pos(requested?.maxObservationChars, ceil.maxObservationChars),
    maxTranscriptChars: pos(requested?.maxTranscriptChars, ceil.maxTranscriptChars),
    maxEntriesScannedPerOperation: pos(
      requested?.maxEntriesScannedPerOperation,
      ceil.maxEntriesScannedPerOperation,
    ),
    maxSpansPerOperation: pos(
      requested?.maxSpansPerOperation,
      ceil.maxSpansPerOperation,
    ),
    maxCharsPerSpan: pos(requested?.maxCharsPerSpan, ceil.maxCharsPerSpan),
    requireExactUsage: true,
  }
}

// ---------------------------------------------------------------------------
// Exact-usage recorder — idempotent per-call rows keyed rlm:<runRef>:<callRef>.
// ---------------------------------------------------------------------------

export interface DesktopRlmModelCallUsageRow {
  /** Idempotency key: `rlm:<runRef>:<callRef>`. */
  readonly key: string
  readonly runRef: string
  readonly callRef: string
  readonly role: "root" | "leaf"
  readonly inputTokens: number
  readonly outputTokens: number
  readonly totalTokens: number
  /** Exact-only: a row exists only when the provider returned exact counts. */
  readonly usageTruth: "exact"
}

export interface DesktopRlmUsageTotals {
  readonly modelCalls: number
  readonly inputTokens: number
  readonly outputTokens: number
  readonly totalTokens: number
}

export interface DesktopRlmUsageRecorder {
  /**
   * Record one completed model call. Returns false (and changes nothing)
   * when the `rlm:<runRef>:<callRef>` key was already recorded — replays and
   * duplicate submissions never double-count.
   */
  readonly record: (input: {
    readonly runRef: string
    readonly callRef: string
    readonly role: "root" | "leaf"
    readonly inputTokens: number
    readonly outputTokens: number
  }) => boolean
  readonly rows: () => ReadonlyArray<DesktopRlmModelCallUsageRow>
  readonly totals: () => DesktopRlmUsageTotals
}

/** Build the idempotency key for one RLM model call. */
export const rlmUsageRowKey = (runRef: string, callRef: string): string =>
  `rlm:${runRef}:${callRef}`

export const makeDesktopRlmUsageRecorder = (): DesktopRlmUsageRecorder => {
  const byKey = new Map<string, DesktopRlmModelCallUsageRow>()
  return {
    record: (input) => {
      const key = rlmUsageRowKey(input.runRef, input.callRef)
      if (byKey.has(key)) return false
      byKey.set(key, {
        key,
        runRef: input.runRef,
        callRef: input.callRef,
        role: input.role,
        inputTokens: input.inputTokens,
        outputTokens: input.outputTokens,
        totalTokens: input.inputTokens + input.outputTokens,
        usageTruth: "exact",
      })
      return true
    },
    rows: () => [...byKey.values()],
    totals: () => {
      let inputTokens = 0
      let outputTokens = 0
      for (const row of byKey.values()) {
        inputTokens += row.inputTokens
        outputTokens += row.outputTokens
      }
      return {
        modelCalls: byKey.size,
        inputTokens,
        outputTokens,
        totalTokens: inputTokens + outputTokens,
      }
    },
  }
}

/**
 * Project exact RLM call rows into the existing session usage-ledger record
 * shape (one `child` record per model call). The caller supplies the real
 * provider lane and account the model plan ran on.
 */
export const usageLedgerInputsFromRlmUsage = (input: {
  readonly rows: ReadonlyArray<DesktopRlmModelCallUsageRow>
  readonly provider: UsageLedgerProvider
  readonly accountRef: string
  readonly requestedModel: string | null
}): ReadonlyArray<UsageLedgerRecordInput> =>
  input.rows.map((row) => ({
    provider: input.provider,
    accountRef: input.accountRef,
    requestedModel: input.requestedModel,
    kind: "child",
    usage: {
      inputTokens: row.inputTokens,
      cachedInputTokens: 0,
      outputTokens: row.outputTokens,
      reasoningTokens: 0,
      totalTokens: row.totalTokens,
    },
  }))

// ---------------------------------------------------------------------------
// Counted model plan — exact-usage enforcement around root/leaf completions.
// ---------------------------------------------------------------------------

export interface DesktopRlmCompleteResult {
  readonly text: string
  readonly inputTokens?: number
  readonly outputTokens?: number
}

export type DesktopRlmCompleteFn = (
  prompt: string,
) => Effect.Effect<DesktopRlmCompleteResult, RlmError>

/**
 * Wrap root/leaf completion functions with the exact-usage contract: every
 * successful call must carry exact token counts (else it fails typed as
 * `usage_required_but_unavailable`) and is recorded idempotently under
 * `rlm:<runRef>:<callRef>`. Contract retries are separate calls with fresh
 * callRefs and their own rows.
 */
export const makeCountedDesktopRlmModelPlan = (input: {
  readonly runRef: string
  readonly completeRoot: DesktopRlmCompleteFn
  readonly completeLeaf?: DesktopRlmCompleteFn
  readonly recorder: DesktopRlmUsageRecorder
}): RlmModelPlan => {
  let rootCalls = 0
  let leafCalls = 0
  const wrap = (
    role: "root" | "leaf",
    complete: DesktopRlmCompleteFn,
  ): DesktopRlmCompleteFn =>
    (prompt) =>
      Effect.gen(function* () {
        const callIndex = role === "root" ? ++rootCalls : ++leafCalls
        const callRef = `${role}.${callIndex}`
        const out = yield* complete(prompt)
        if (out.inputTokens === undefined || out.outputTokens === undefined) {
          return yield* new RlmError({
            reason: "usage_required_but_unavailable",
            retryable: false,
            detailSafe: `exact usage missing for ${callRef}; provider ineligible for admitted semantic recall`,
          })
        }
        input.recorder.record({
          runRef: input.runRef,
          callRef,
          role,
          inputTokens: out.inputTokens,
          outputTokens: out.outputTokens,
        })
        return out
      })
  const completeRoot = wrap("root", input.completeRoot)
  return {
    completeRoot,
    completeLeaf:
      input.completeLeaf === undefined
        ? completeRoot
        : wrap("leaf", input.completeLeaf),
    strategyRef: DESKTOP_RLM_STRATEGY_REF,
  }
}

/**
 * Build a completion function from an already-admitted Effect AI
 * `LanguageModel` layer (existing provider/account readiness owns which
 * layer this is — no credentials or provider config enter the request).
 * Token counts come only from the provider response; missing totals stay
 * missing so the counted plan can refuse them typed.
 */
export const desktopRlmCompleteFromLanguageModel: Effect.Effect<
  DesktopRlmCompleteFn,
  never,
  LanguageModel.LanguageModel
> = Effect.gen(function* () {
  const context = yield* Effect.context<LanguageModel.LanguageModel>()
  return (prompt: string) =>
    LanguageModel.generateText({ prompt }).pipe(
      Effect.provideContext(context),
      Effect.map((response) => ({
        text: response.text,
        ...(response.usage.inputTokens.total === undefined
          ? {}
          : { inputTokens: response.usage.inputTokens.total }),
        ...(response.usage.outputTokens.total === undefined
          ? {}
          : { outputTokens: response.usage.outputTokens.total }),
      })),
      Effect.mapError((error) => rlmErrorFromAiError(error)),
    )
})

/** Map typed Effect AI provider failures onto distinct RLM error reasons. */
export const rlmErrorFromAiError = (error: AiError.AiError): RlmError => {
  const cause = error.reason._tag
  const reason =
    cause === "AuthenticationError"
      ? ("model_authentication" as const)
      : cause === "QuotaExhaustedError"
        ? ("model_quota_exhausted" as const)
        : cause === "RateLimitError"
          ? ("model_rate_limited" as const)
          : ("model_unavailable" as const)
  return new RlmError({
    reason,
    retryable: reason === "model_rate_limited",
    detailSafe: cause,
  })
}

// ---------------------------------------------------------------------------
// The Tier S runner over the first-class Rlm engine.
// ---------------------------------------------------------------------------

/**
 * Build an `Rlm` service over the desktop corpus source whose semantic
 * admission mirrors the host admission record. Without admission the engine
 * refuses semantic requests typed (`semantic_not_admitted`) — the same seam
 * `makeDesktopRlmDeterministic` pins closed.
 */
export const makeDesktopRlmSemantic = (
  sources: HistoryRecallHostSources,
  options: {
    readonly admission: DesktopSemanticRecallAdmission | null
    readonly plan: RlmModelPlan
  },
): Effect.Effect<RlmShape> =>
  makeRlm({
    admitSemantic: options.admission !== null,
    model: options.plan,
  }).pipe(
    Effect.provide(
      desktopHistoryCorpusSourceLayer(toDesktopHistoryCorpusSourceInput(sources)),
    ),
  )

/** Build the clamped semantic request for a host-authorized scope. */
export const desktopSemanticRlmRequest = (input: {
  readonly scope: HistoryCorpusScope
  readonly question: string
  readonly runRef: string
  readonly budget?: Partial<RlmBudget>
}): RlmSemanticRequest => ({
  _tag: "Semantic",
  schemaId: "openagents.ai.rlm_request.v1",
  runRef: input.runRef,
  corpus: desktopHistoryCorpusInputForScope(input.scope),
  question: input.question,
  budget: clampDesktopSemanticBudget(input.budget),
  evidence: defaultRlmEvidencePolicy,
  strategyRef: DESKTOP_RLM_STRATEGY_REF,
})

/** Bounded, display-only progress row. Transient — never persisted. */
export interface DesktopSemanticRecallProgress {
  readonly runRef: string
  readonly label: string
}

/** Project one engine event onto a bounded transient progress row. */
export const semanticRecallProgressFromRlmEvent = (
  event: RlmEvent,
): DesktopSemanticRecallProgress | null => {
  switch (event._tag) {
    case "RunStarted":
      return { runRef: event.runRef, label: `run started (${event.mode})` }
    case "CorpusResolved":
      return {
        runRef: event.runRef,
        label: `corpus resolved · ${event.entryCount} entries`,
      }
    case "IterationStarted":
      return {
        runRef: event.runRef,
        label: `iteration ${event.iteration} · depth ${event.depth}`,
      }
    case "ProgramSelected":
      return {
        runRef: event.runRef,
        label: `program selected · ${event.nodeCount} nodes`,
      }
    case "MapStarted":
      return {
        runRef: event.runRef,
        label: `${event.kind} started · ${event.itemCount} items`,
      }
    case "MapCompleted":
      return {
        runRef: event.runRef,
        label: `${event.kind} completed · ${event.itemCount} items`,
      }
    case "ModelCallCompleted":
      return {
        runRef: event.runRef,
        label: `model call ${event.callRef} (${event.role}) completed`,
      }
    default:
      return null
  }
}

export interface DesktopSemanticRecallInput {
  readonly scope: HistoryCorpusScope
  readonly question: string
  readonly runRef: string
  readonly admission: DesktopSemanticRecallAdmission | null
  readonly plan: RlmModelPlan
  readonly budget?: Partial<RlmBudget>
  /** Transient display-only progress sink. Replay never depends on it. */
  readonly onProgress?: (progress: DesktopSemanticRecallProgress) => void
}

/**
 * Run one admitted (or typed-refused) semantic recall through the
 * first-class engine. The bounded terminal result is the only durable
 * artifact — progress rows are display-only.
 */
export const runDesktopRlmSemanticRecall = (
  sources: HistoryRecallHostSources,
  input: DesktopSemanticRecallInput,
): Effect.Effect<RlmTerminalResult, RlmError> =>
  Effect.gen(function* () {
    const rlm = yield* makeDesktopRlmSemantic(sources, {
      admission: input.admission,
      plan: input.plan,
    })
    const request = desktopSemanticRlmRequest({
      scope: input.scope,
      question: input.question,
      runRef: input.runRef,
      ...(input.budget === undefined ? {} : { budget: input.budget }),
    })
    const events = yield* Stream.runCollect(rlm.stream(request))
    let terminal: RlmTerminalResult | null = null
    for (const event of events) {
      if (input.onProgress !== undefined) {
        const progress = semanticRecallProgressFromRlmEvent(event)
        if (progress !== null) input.onProgress(progress)
      }
      if (event._tag === "Terminal") terminal = event.result
    }
    if (terminal === null) {
      return yield* new RlmError({
        reason: "invariant_violation",
        retryable: false,
        detailSafe: "semantic stream ended without terminal event",
      })
    }
    return terminal
  })

// ---------------------------------------------------------------------------
// Citation + honesty projections (renderer-safe plain data).
// ---------------------------------------------------------------------------

/**
 * Project the engine-validated citations of a semantic terminal result onto
 * the existing renderer cited-span rows. Only citations whose durable
 * history cursor address decodes are navigable — anything else is dropped,
 * never guessed.
 */
export const citedSpansFromSemanticResult = (
  result: RlmTerminalResult,
): ReadonlyArray<HistoryRecallCitedSpanRow> => {
  if (result._tag === "Refused") return []
  const rows: Array<HistoryRecallCitedSpanRow> = []
  for (const citation of result.citations) {
    const cursor = decodeHistoryCursorAddress(citation.sourceAddress)
    if (cursor === null) continue
    rows.push({
      turnId: cursor.turnId,
      sequenceStart: cursor.sequence,
      sequenceEnd: cursor.sequence,
      excerpt: citation.excerpt ?? "",
      kind: "text.delta",
      scopeRef: citation.scopeRef,
    })
  }
  return rows
}

/**
 * Bounded terminal summary for the renderer card. Plain data only — the
 * renderer projection duplicates this interface instead of importing SDK
 * packages (same boundary as the Tier D card).
 */
export interface SemanticRecallTerminalSummary {
  readonly state: "completed" | "partial" | "refused"
  readonly reason: string | null
  readonly modelCalls: number
  readonly subcalls: number
  /** Exact total when known; null when unavailable — never zero-filled. */
  readonly totalTokens: number | null
  readonly usageCompleteness: "complete" | "partial" | "unavailable"
  readonly citationValidated: number
  readonly citationInvalid: number
  readonly strategyRef: string | null
}

/**
 * Fold a terminal result plus the host exact-usage totals into the bounded
 * renderer summary. The recorder is the exact-usage truth: when every engine
 * model call has a recorded exact row the usage is `complete`; a mismatch is
 * reported `partial`; with no recorder rows usage stays `unavailable`.
 */
export const semanticTerminalSummaryFromResult = (
  result: RlmTerminalResult,
  totals?: DesktopRlmUsageTotals | null,
): SemanticRecallTerminalSummary => {
  const exact = totals ?? null
  const usageCompleteness =
    exact === null || exact.modelCalls === 0
      ? result.usage.completeness
      : exact.modelCalls === result.usage.modelCalls
        ? "complete"
        : "partial"
  return {
    state:
      result._tag === "Completed"
        ? "completed"
        : result._tag === "Partial"
          ? "partial"
          : "refused",
    reason: result._tag === "Completed" ? null : result.reason,
    modelCalls: result.usage.modelCalls,
    subcalls: result.usage.subcalls,
    totalTokens:
      exact !== null && exact.modelCalls > 0
        ? exact.totalTokens
        : (result.usage.totalTokens ?? null),
    usageCompleteness,
    citationValidated: result.honesty.citationValidated,
    citationInvalid: result.honesty.citationInvalid,
    strategyRef: result.honesty.strategyRef ?? null,
  }
}

// ---------------------------------------------------------------------------
// Tiered dispatch behind the same host-tool seam.
// ---------------------------------------------------------------------------

/** Host-side semantic configuration for one tiered dispatch. */
export interface DesktopSemanticRecallConfig {
  /** Host-authorized scope (never parsed from model tool args). */
  readonly scope: HistoryCorpusScope
  readonly question: string
  readonly runRef: string
  readonly completeRoot: DesktopRlmCompleteFn
  readonly completeLeaf?: DesktopRlmCompleteFn
  readonly recorder?: DesktopRlmUsageRecorder
  readonly budget?: Partial<RlmBudget>
  readonly onProgress?: (progress: DesktopSemanticRecallProgress) => void
}

export type DesktopSemanticRecallOutcome =
  | {
      readonly _tag: "result"
      readonly result: RlmTerminalResult
      readonly citedSpans: ReadonlyArray<HistoryRecallCitedSpanRow>
      readonly summary: SemanticRecallTerminalSummary
      readonly usageRows: ReadonlyArray<DesktopRlmModelCallUsageRow>
    }
  | {
      readonly _tag: "failure"
      readonly reason: RlmError["reason"]
      readonly detailSafe: string | null
      readonly usageRows: ReadonlyArray<DesktopRlmModelCallUsageRow>
    }

export interface DesktopTieredRecallInput extends HistoryRecallHostDispatchInput {
  readonly tierRequest?: DesktopRecallTierRequest
  readonly admission?: DesktopSemanticRecallAdmission | null
  readonly semantic?: DesktopSemanticRecallConfig
}

export interface DesktopTieredRecallOutcome {
  readonly decision: DesktopRecallTierDecision
  /** Tier D always runs first (free, deterministic). */
  readonly deterministic: HistoryRecallHostDispatchResult
  /** Present only when the tier decision selected and ran Tier S. */
  readonly semantic: DesktopSemanticRecallOutcome | null
}

/**
 * Dispatch one `history_recall` request through the tier policy: Tier D
 * always runs first through the existing host-tool dispatcher (including its
 * neutral-stream re-entry), then Tier S runs only when the host-side tier
 * request and admission select it. Tool-call arguments are never consulted
 * for tier selection, so a model cannot self-authorize semantic recall.
 */
export const dispatchDesktopHistoryRecallTiered = (
  sources: HistoryRecallHostSources,
  input: DesktopTieredRecallInput,
): Effect.Effect<DesktopTieredRecallOutcome> =>
  Effect.gen(function* () {
    const deterministic = yield* dispatchDesktopHistoryRecall(sources, input)
    const decision = selectDesktopRecallTier({
      request: input.tierRequest,
      admission: input.admission ?? null,
      deterministicResponse: deterministic.answer,
    })
    if (decision.tier !== "semantic" || input.semantic === undefined) {
      return { decision, deterministic, semantic: null }
    }
    const config = input.semantic
    const recorder = config.recorder ?? makeDesktopRlmUsageRecorder()
    const plan = makeCountedDesktopRlmModelPlan({
      runRef: config.runRef,
      completeRoot: config.completeRoot,
      ...(config.completeLeaf === undefined
        ? {}
        : { completeLeaf: config.completeLeaf }),
      recorder,
    })
    const semantic = yield* runDesktopRlmSemanticRecall(sources, {
      scope: config.scope,
      question: config.question,
      runRef: config.runRef,
      admission: decision.admission,
      plan,
      ...(config.budget === undefined ? {} : { budget: config.budget }),
      ...(config.onProgress === undefined ? {} : { onProgress: config.onProgress }),
    }).pipe(
      Effect.map(
        (result): DesktopSemanticRecallOutcome => ({
          _tag: "result",
          result,
          citedSpans: citedSpansFromSemanticResult(result),
          summary: semanticTerminalSummaryFromResult(result, recorder.totals()),
          usageRows: recorder.rows(),
        }),
      ),
      Effect.catch(
        (error): Effect.Effect<DesktopSemanticRecallOutcome> =>
          Effect.succeed({
            _tag: "failure",
            reason: error.reason,
            detailSafe: error.detailSafe ?? null,
            usageRows: recorder.rows(),
          }),
      ),
    )
    return { decision, deterministic, semantic }
  })
