/**
 * RLM-06 (#9142) — long-run Full Auto RLM recall consumer.
 *
 * A long Full Auto run may consult its OWN authorized full event history while
 * framing the next continuation, without transferring any run or verification
 * authority to RLM. This module is the host-owned consumer that sits ON TOP of
 * the Tier D / Tier S seam from #9141 (`history-recall-semantic.ts`) and the
 * `DesktopHistoryCorpusSource` from #9154. It reuses that admission, citation,
 * and exact-usage machinery unchanged.
 *
 * Security core (run-scope isolation):
 * - Current-run scope resolves through `DesktopHistoryCorpusSource` using the
 *   authoritative run-registry thread membership ONLY. A run can NEVER see an
 *   unrelated thread or owner: the wrapped sources authorize a thread only when
 *   the registry says it belongs to THIS run, re-read at each call, and the
 *   `Run` membership resolver answers only for this exact `runRef` and only its
 *   registry-bound threads. Model-supplied thread ids are ignored by the SDK
 *   corpus source and cannot widen scope.
 *
 * Boundaries (invariants — NONE of these move into RLM):
 * - Full Auto leases, generation fencing, run/concurrency caps,
 *   provider/account custody, journal, receipts, stop/reconcile, teardown,
 *   acceptance, and verification authority stay outside recall.
 * - Recall is a bounded CITED CANDIDATE inserted into continuation context —
 *   never raw corpus slices, never the recursive transcript, never verified.
 * - Deterministic recall runs first for structural questions; semantic recall
 *   is admitted only through the same #9141 application policy and within a
 *   finite per-run recall budget.
 * - Every recall records its result refs and exact per-call usage in the run
 *   recall ledger; replay is idempotent and never double-counts or
 *   double-consumes budget.
 * - Refused, partial, unavailable, interrupted, and failed recall all continue
 *   per existing run policy: recall failure cannot stall teardown, leak a
 *   lease, or make a run falsely successful.
 *
 * Paper-fidelity: Full Auto consumes only the bounded inline committed value
 * from the terminal result. It does not fetch or create an RLM artifact during
 * continuation framing (the semantic budget already forces
 * `maxArtifactOutputBytes: 0`). Program/map fan-out, environment bytes, model
 * calls, and total usage are all charged to the finite per-run recall budget.
 */

import { Effect, Stream } from "effect"
import {
  HISTORY_RECALL_TOOL_NAME,
  type HarnessHostToolCall,
} from "@openagentsinc/agent-harness-contract"
import type {
  HistoryCorpusScope,
  HistoryRecallQuestion,
  HistoryRecallResponse,
} from "@openagentsinc/history-corpus"
import {
  defaultRlmDeterministicLimits,
  defaultRlmEvidencePolicy,
  makeRlm,
  type RlmBudget,
  type RlmCitation,
  type RlmDeterministicLimits,
  type RlmDeterministicRequest,
  type RlmSemanticRequest,
  type RlmTerminalResult,
} from "@openagentsinc/rlm"

import { DESKTOP_RLM_STRATEGY_REF } from "./desktop-history-corpus-source.ts"
import {
  FOLDER_CORPUS_STRATEGY_REF,
  folderCorpusSourceInput,
  folderRlmCorpusSourceLayer,
  splitFolderCorpusAddress,
  type FolderRlmCorpusConfig,
} from "./folder-corpus-source.ts"
import type {
  HistoryRecallCitedSpanRow,
  HistoryRecallHostSources,
} from "./history-recall-host.ts"
import {
  clampDesktopSemanticBudget,
  dispatchDesktopHistoryRecallTiered,
  makeCountedDesktopRlmModelPlan,
  makeDesktopRlmUsageRecorder,
  semanticRecallProgressFromRlmEvent,
  semanticTerminalSummaryFromResult,
  usageLedgerInputsFromRlmUsage,
  type DesktopRecallTierRequest,
  type DesktopRlmCompleteFn,
  type DesktopRlmModelCallUsageRow,
  type DesktopRlmUsageRecorder,
  type DesktopRlmUsageTotals,
  type DesktopSemanticRecallAdmission,
  type DesktopSemanticRecallOutcome,
  type DesktopSemanticRecallProgress,
  type SemanticRecallTerminalSummary,
} from "./history-recall-semantic.ts"
import type { FullAutoRunRegistry } from "./full-auto-run-registry.ts"
import type { UsageLedgerRecordInput } from "./usage-ledger.ts"
import type { UsageLedgerProvider } from "./usage-ledger-contract.ts"

// ---------------------------------------------------------------------------
// Run-scope resolution — authoritative registry membership, never model input.
// ---------------------------------------------------------------------------

/**
 * The threads this run may read, resolved authoritatively from the run
 * registry. A run reads ONLY its own bound thread. An unbound or unknown run
 * resolves to zero threads (recall becomes `unavailable`, never a foreign
 * read).
 */
export const fullAutoRunThreadMembership = (
  registry: FullAutoRunRegistry,
  runRef: string,
): ReadonlyArray<string> => {
  const run = registry.get(runRef)
  if (run === null) return []
  return run.threadRef === undefined ? [] : [run.threadRef]
}

/**
 * The authoritative `Run` corpus scope for this run, or null when the run has
 * no readable thread yet. The `threadIds` here are informational for the Tier
 * D path; the SDK corpus source re-resolves `Run` membership from
 * `threadIdsForRun` and ignores any supplied ids.
 */
export const fullAutoRunRecallScope = (
  registry: FullAutoRunRegistry,
  runRef: string,
): Extract<HistoryCorpusScope, { readonly _tag: "Run" }> | null => {
  const threadIds = fullAutoRunThreadMembership(registry, runRef)
  if (threadIds.length === 0) return null
  return { _tag: "Run", runRef, threadIds: [...threadIds] }
}

/**
 * Wrap the host recall sources so this run may read ONLY its registry-bound
 * threads. Both the authorizer and the `Run` membership resolver re-read the
 * registry on every call, so a rebind or unbind takes effect immediately and a
 * foreign thread is refused even when a scope somehow names it. Any base
 * owner-session authorizer is intersected — recall never widens the owner's
 * own visibility policy.
 */
export const makeFullAutoRunRecallSources = (input: {
  readonly base: HistoryRecallHostSources
  readonly registry: FullAutoRunRegistry
  readonly runRef: string
}): HistoryRecallHostSources => {
  const baseAuthorize = input.base.authorizeThread
  return {
    ...input.base,
    authorizeThread: async (threadId) => {
      const membership = new Set(
        fullAutoRunThreadMembership(input.registry, input.runRef),
      )
      if (!membership.has(threadId)) return false
      if (baseAuthorize === undefined) return true
      try {
        return await baseAuthorize(threadId)
      } catch {
        return false
      }
    },
    threadIdsForRun: (rf) =>
      rf === input.runRef
        ? fullAutoRunThreadMembership(input.registry, input.runRef)
        : [],
  }
}

// ---------------------------------------------------------------------------
// The per-run recall ledger — finite budget plus idempotent result/usage rows.
// ---------------------------------------------------------------------------

/** Finite default recall budget per Full Auto run. */
export const FULL_AUTO_RUN_RECALL_BUDGET_DEFAULT = 8

export type FullAutoRecallTier = "deterministic" | "semantic" | "none"

export type FullAutoRecallStatus =
  | "completed"
  | "partial"
  | "refused"
  | "failed"
  | "unavailable"
  | "budget_exhausted"
  | "deterministic_only"

/** Bounded, replay-stable references to what a recall committed. */
export interface FullAutoRecallResultRefs {
  /** The SDK strategy profile pin, when the semantic engine ran. */
  readonly strategyRef: string | null
  /** The committed inline value ref (InlineValue/Artifact), when present. */
  readonly committedValueRef: string | null
  readonly committedValueDigest: string | null
  /** The exact corpus content digest the recall resolved against. */
  readonly contentDigest: string | null
  /** Bounded validated citation entry refs (never raw excerpts). */
  readonly citationEntryRefs: ReadonlyArray<string>
}

export const EMPTY_FULL_AUTO_RECALL_RESULT_REFS: FullAutoRecallResultRefs = {
  strategyRef: null,
  committedValueRef: null,
  committedValueDigest: null,
  contentDigest: null,
  citationEntryRefs: [],
}

/** One recall's durable, bounded, replay-stable outcome snapshot. */
export interface FullAutoRecallOutcome {
  readonly runRef: string
  readonly recallRef: string
  /** Distinct engine run ref so usage keys never collide across recalls in the
   * same Full Auto run: `<runRef>::recall::<recallRef>`. */
  readonly recallRunRef: string
  readonly status: FullAutoRecallStatus
  readonly reason: string | null
  readonly tier: FullAutoRecallTier
  readonly scopeResolved: boolean
  /** Whether a NEW recall consumed one unit of the finite per-run budget. */
  readonly consumedBudget: boolean
  /** The bounded validated cited spans to frame — the ONLY corpus-derived text
   * allowed into continuation context. */
  readonly citedSpans: ReadonlyArray<HistoryRecallCitedSpanRow>
  readonly deterministicCitedSpans: ReadonlyArray<HistoryRecallCitedSpanRow>
  readonly summary: SemanticRecallTerminalSummary | null
  readonly resultRefs: FullAutoRecallResultRefs
  readonly usage: DesktopRlmUsageTotals
  readonly usageRows: ReadonlyArray<DesktopRlmModelCallUsageRow>
  readonly capsHit: ReadonlyArray<string>
  readonly coverageNote: string | null
}

export interface FullAutoRecallLedgerEntry {
  readonly runRef: string
  readonly recallRef: string
  readonly outcome: FullAutoRecallOutcome
}

export interface FullAutoRecallLedger {
  readonly budgetPerRun: number
  /** Remaining budget for a run: budget minus recalls that consumed a unit. */
  readonly remaining: (runRef: string) => number
  /** Whether a recall may run: an already-recorded recallRef always replays;
   * a new one needs a free budget unit. */
  readonly canRecall: (runRef: string, recallRef: string) => boolean
  /** Record an outcome idempotently. Returns false when this (runRef,recallRef)
   * was already recorded — replays never double-count or double-consume. */
  readonly record: (outcome: FullAutoRecallOutcome) => boolean
  readonly get: (runRef: string, recallRef: string) => FullAutoRecallOutcome | null
  readonly entries: (runRef?: string) => ReadonlyArray<FullAutoRecallLedgerEntry>
  /** Project every recorded exact usage row into the session usage-ledger
   * record shape. */
  readonly usageLedgerInputs: (input: {
    readonly provider: UsageLedgerProvider
    readonly accountRef: string
    readonly requestedModel: string | null
    readonly runRef?: string
  }) => ReadonlyArray<UsageLedgerRecordInput>
}

const recallLedgerKey = (runRef: string, recallRef: string): string =>
  `${runRef} ${recallRef}`

export const makeFullAutoRecallLedger = (options?: {
  readonly budgetPerRun?: number
}): FullAutoRecallLedger => {
  const budgetPerRun = Math.max(
    0,
    Math.floor(options?.budgetPerRun ?? FULL_AUTO_RUN_RECALL_BUDGET_DEFAULT),
  )
  const byKey = new Map<string, FullAutoRecallLedgerEntry>()

  const consumedCount = (runRef: string): number => {
    let count = 0
    for (const entry of byKey.values()) {
      if (entry.runRef === runRef && entry.outcome.consumedBudget) count += 1
    }
    return count
  }

  const remaining = (runRef: string): number =>
    Math.max(0, budgetPerRun - consumedCount(runRef))

  return {
    budgetPerRun,
    remaining,
    canRecall: (runRef, recallRef) =>
      byKey.has(recallLedgerKey(runRef, recallRef)) || remaining(runRef) > 0,
    record: (outcome) => {
      const key = recallLedgerKey(outcome.runRef, outcome.recallRef)
      if (byKey.has(key)) return false
      byKey.set(key, {
        runRef: outcome.runRef,
        recallRef: outcome.recallRef,
        outcome,
      })
      return true
    },
    get: (runRef, recallRef) =>
      byKey.get(recallLedgerKey(runRef, recallRef))?.outcome ?? null,
    entries: (runRef) =>
      [...byKey.values()].filter(
        (entry) => runRef === undefined || entry.runRef === runRef,
      ),
    usageLedgerInputs: (input) => {
      const rows: Array<DesktopRlmModelCallUsageRow> = []
      for (const entry of byKey.values()) {
        if (input.runRef !== undefined && entry.runRef !== input.runRef) continue
        rows.push(...entry.outcome.usageRows)
      }
      return usageLedgerInputsFromRlmUsage({
        rows,
        provider: input.provider,
        accountRef: input.accountRef,
        requestedModel: input.requestedModel,
      })
    },
  }
}

// ---------------------------------------------------------------------------
// Result-ref extraction from a bounded terminal result.
// ---------------------------------------------------------------------------

/** Bound the number of citation refs carried into the ledger / fragment. */
export const FULL_AUTO_RECALL_MAX_CITATION_REFS = 16

const committedRefsFromResult = (
  result: RlmTerminalResult | null,
): FullAutoRecallResultRefs => {
  if (result === null) return EMPTY_FULL_AUTO_RECALL_RESULT_REFS
  if (result._tag === "Refused") {
    return {
      ...EMPTY_FULL_AUTO_RECALL_RESULT_REFS,
      strategyRef: result.honesty.strategyRef ?? null,
      contentDigest: result.run.contentDigest,
    }
  }
  const output = result._tag === "Completed" ? result.output : result.bestOutput
  let committedValueRef: string | null = null
  let committedValueDigest: string | null = null
  if (output !== undefined) {
    if (output._tag === "InlineValue") {
      committedValueRef = output.valueRef
      committedValueDigest = output.digest
    } else if (output._tag === "Artifact") {
      committedValueRef = output.valueRef
    }
  }
  const citationEntryRefs = result.citations
    .map((citation) => citation.entryRefStart)
    .slice(0, FULL_AUTO_RECALL_MAX_CITATION_REFS)
  return {
    strategyRef: result.honesty.strategyRef ?? null,
    committedValueRef,
    committedValueDigest,
    contentDigest: result.run.contentDigest,
    citationEntryRefs,
  }
}

const resultRefsFromDeterministicSpans = (
  spans: ReadonlyArray<HistoryRecallCitedSpanRow>,
): FullAutoRecallResultRefs => ({
  ...EMPTY_FULL_AUTO_RECALL_RESULT_REFS,
  citationEntryRefs: spans
    .map((span) => `${span.turnId}#${span.sequenceStart}`)
    .slice(0, FULL_AUTO_RECALL_MAX_CITATION_REFS),
})

const capsAndCoverageFrom = (
  response: HistoryRecallResponse | null,
): { readonly capsHit: ReadonlyArray<string>; readonly coverageNote: string | null } => {
  if (response === null) return { capsHit: [], coverageNote: null }
  return {
    capsHit: [...response.honesty.capsHit],
    coverageNote: response.honesty.coverageNote,
  }
}

// ---------------------------------------------------------------------------
// The consumer — deterministic-first, admitted-semantic, fail-soft.
// ---------------------------------------------------------------------------

/** Host-supplied semantic escalation config (already-admitted completion). */
export interface FullAutoRecallSemanticConfig {
  readonly question: string
  readonly completeRoot: DesktopRlmCompleteFn
  readonly completeLeaf?: DesktopRlmCompleteFn
  readonly budget?: Partial<RlmBudget>
}

export interface FullAutoRecallInput {
  readonly runRef: string
  /** Idempotency key AND engine run-ref seed. Normally the continuation turn
   * ref, so replaying a continuation replays its recall deterministically. */
  readonly recallRef: string
  readonly registry: FullAutoRunRegistry
  readonly base: HistoryRecallHostSources
  readonly ledger: FullAutoRecallLedger
  /** Structural (deterministic) question — Tier D always runs first. */
  readonly deterministicQuestion: HistoryRecallQuestion
  readonly caps?: unknown
  /** Optional host tier request (#9141 policy). Absent stays deterministic. */
  readonly tierRequest?: DesktopRecallTierRequest
  /** Host-owned semantic admission (#9141). Never from model output. */
  readonly admission?: DesktopSemanticRecallAdmission | null
  readonly semantic?: FullAutoRecallSemanticConfig
  /** Transient display-only progress. Replay never depends on it. */
  readonly onProgress?: (progress: DesktopSemanticRecallProgress) => void
}

export const fullAutoRecallRunRef = (
  runRef: string,
  recallRef: string,
): string => `${runRef}::recall::${recallRef}`

const emptyOutcome = (input: {
  readonly runRef: string
  readonly recallRef: string
  readonly recallRunRef: string
  readonly status: FullAutoRecallStatus
  readonly reason: string | null
  readonly scopeResolved: boolean
}): FullAutoRecallOutcome => ({
  runRef: input.runRef,
  recallRef: input.recallRef,
  recallRunRef: input.recallRunRef,
  status: input.status,
  reason: input.reason,
  tier: "none",
  scopeResolved: input.scopeResolved,
  consumedBudget: false,
  citedSpans: [],
  deterministicCitedSpans: [],
  summary: null,
  resultRefs: EMPTY_FULL_AUTO_RECALL_RESULT_REFS,
  usage: { modelCalls: 0, inputTokens: 0, outputTokens: 0, totalTokens: 0 },
  usageRows: [],
  capsHit: [],
  coverageNote: null,
})

const semanticResult = (
  outcome: DesktopSemanticRecallOutcome | null,
): RlmTerminalResult | null =>
  outcome !== null && outcome._tag === "result" ? outcome.result : null

const statusFromSemantic = (
  outcome: DesktopSemanticRecallOutcome,
): { readonly status: FullAutoRecallStatus; readonly reason: string | null } => {
  if (outcome._tag === "failure") {
    return { status: "failed", reason: outcome.reason }
  }
  switch (outcome.result._tag) {
    case "Completed":
      return { status: "completed", reason: null }
    case "Partial":
      return { status: "partial", reason: outcome.result.reason }
    case "Refused":
      return { status: "refused", reason: outcome.result.reason }
  }
}

/**
 * Run one host-owned recall for a Full Auto continuation. Deterministic recall
 * always runs first; admitted semantic runs only through the #9141 policy and
 * only within the finite per-run budget. The returned Effect never fails typed
 * — every refused/partial/unavailable/failed disposition is a value the caller
 * continues past. Interruption from stop/reconcile/teardown propagates and
 * records nothing (no fabricated success, no leaked ledger entry).
 */
export const runFullAutoRecall = (
  input: FullAutoRecallInput,
): Effect.Effect<FullAutoRecallOutcome> =>
  Effect.gen(function* () {
    const recallRunRef = fullAutoRecallRunRef(input.runRef, input.recallRef)

    // Idempotent replay: a recorded recall returns its stored bounded outcome
    // without re-dispatching, re-counting usage, or re-consuming budget.
    const existing = input.ledger.get(input.runRef, input.recallRef)
    if (existing !== null) return existing

    // Run-scope resolution — authoritative registry membership only.
    const scope = fullAutoRunRecallScope(input.registry, input.runRef)
    if (scope === null) {
      const outcome = emptyOutcome({
        runRef: input.runRef,
        recallRef: input.recallRef,
        recallRunRef,
        status: "unavailable",
        reason: "run has no readable registry-bound thread",
        scopeResolved: false,
      })
      input.ledger.record(outcome)
      return outcome
    }

    // Finite per-run recall budget.
    if (!input.ledger.canRecall(input.runRef, input.recallRef)) {
      const outcome = emptyOutcome({
        runRef: input.runRef,
        recallRef: input.recallRef,
        recallRunRef,
        status: "budget_exhausted",
        reason: `per-run recall budget of ${input.ledger.budgetPerRun} exhausted`,
        scopeResolved: true,
      })
      input.ledger.record(outcome)
      return outcome
    }

    const sources = makeFullAutoRunRecallSources({
      base: input.base,
      registry: input.registry,
      runRef: input.runRef,
    })
    const recorder = makeDesktopRlmUsageRecorder()
    const threadId = scope.threadIds[0]!
    const call: HarnessHostToolCall = {
      toolCallId: `fullauto.recall.${input.recallRef}`,
      toolName: HISTORY_RECALL_TOOL_NAME,
      input: {
        scope,
        question: input.deterministicQuestion,
        ...(input.caps === undefined ? {} : { caps: input.caps }),
      },
    }

    const tiered = yield* dispatchDesktopHistoryRecallTiered(sources, {
      call,
      turnId: input.recallRef,
      threadId,
      sequence: 0,
      ...(input.tierRequest === undefined ? {} : { tierRequest: input.tierRequest }),
      ...(input.admission === undefined ? {} : { admission: input.admission }),
      ...(input.semantic === undefined
        ? {}
        : {
            semantic: {
              scope,
              question: input.semantic.question,
              runRef: recallRunRef,
              completeRoot: input.semantic.completeRoot,
              recorder,
              ...(input.semantic.completeLeaf === undefined
                ? {}
                : { completeLeaf: input.semantic.completeLeaf }),
              ...(input.semantic.budget === undefined
                ? {}
                : { budget: input.semantic.budget }),
              ...(input.onProgress === undefined
                ? {}
                : { onProgress: input.onProgress }),
            },
          }),
    })

    const deterministicCitedSpans = tiered.deterministic.citedSpans
    const { capsHit, coverageNote } = capsAndCoverageFrom(tiered.deterministic.answer)

    // Deterministic-only path — no admitted semantic run occurred.
    if (tiered.decision.tier !== "semantic" || tiered.semantic === null) {
      const outcome: FullAutoRecallOutcome = {
        runRef: input.runRef,
        recallRef: input.recallRef,
        recallRunRef,
        status: "deterministic_only",
        reason:
          tiered.decision.tier === "semantic_refused"
            ? "semantic recall not admitted"
            : null,
        tier: "deterministic",
        scopeResolved: true,
        consumedBudget: true,
        citedSpans: deterministicCitedSpans,
        deterministicCitedSpans,
        summary: null,
        resultRefs: resultRefsFromDeterministicSpans(deterministicCitedSpans),
        usage: { modelCalls: 0, inputTokens: 0, outputTokens: 0, totalTokens: 0 },
        usageRows: [],
        capsHit,
        coverageNote,
      }
      input.ledger.record(outcome)
      return outcome
    }

    // Admitted semantic path.
    const semantic = tiered.semantic
    const { status, reason } = statusFromSemantic(semantic)
    const result = semanticResult(semantic)
    const summary =
      semantic._tag === "result"
        ? semantic.summary
        : result === null
          ? null
          : semanticTerminalSummaryFromResult(result, recorder.totals())
    const framedSpans =
      status === "completed" || status === "partial"
        ? semantic._tag === "result"
          ? semantic.citedSpans
          : []
        : []
    const usageRows = semantic.usageRows

    const outcome: FullAutoRecallOutcome = {
      runRef: input.runRef,
      recallRef: input.recallRef,
      recallRunRef,
      status,
      reason,
      tier: "semantic",
      scopeResolved: true,
      consumedBudget: true,
      citedSpans: framedSpans,
      deterministicCitedSpans,
      summary,
      resultRefs: committedRefsFromResult(result),
      usage: recorder.totals(),
      usageRows,
      capsHit,
      coverageNote,
    }
    input.ledger.record(outcome)
    return outcome
  })

// ---------------------------------------------------------------------------
// Continuation framing — bounded cited candidate, never verified.
// ---------------------------------------------------------------------------

export const FULL_AUTO_RECALL_FRAGMENT_MAX_SPANS = 6
export const FULL_AUTO_RECALL_FRAGMENT_MAX_EXCERPT = 160

/** Recall NEVER blocks a continuation or changes run success. Every recall
 * disposition — including refused, failed, unavailable, and budget_exhausted —
 * lets the run continue per existing policy. */
export const fullAutoRecallShouldContinue = (
  _outcome: FullAutoRecallOutcome,
): true => true

/** A recall status that can contribute a framed cited candidate. */
export const fullAutoRecallIsFrameable = (
  outcome: FullAutoRecallOutcome,
): boolean =>
  outcome.status === "completed" ||
  outcome.status === "partial" ||
  outcome.status === "deterministic_only"

/** Bounded honesty caveats — never the word "verified". */
export const fullAutoRecallCaveats = (
  outcome: FullAutoRecallOutcome,
): ReadonlyArray<string> => {
  const caveats: Array<string> = ["cited candidate — not verified"]
  if (outcome.status === "partial") {
    caveats.push(`partial recall (${outcome.reason ?? "capped"})`)
  }
  if (outcome.status === "refused") {
    caveats.push(`recall refused (${outcome.reason ?? "not admitted"})`)
  }
  if (outcome.status === "failed") {
    caveats.push(`recall failed (${outcome.reason ?? "error"})`)
  }
  if (outcome.status === "unavailable") {
    caveats.push("recall unavailable for this run scope")
  }
  if (outcome.status === "budget_exhausted") {
    caveats.push("per-run recall budget exhausted")
  }
  if (outcome.capsHit.length > 0) {
    caveats.push(`coverage truncated (caps: ${outcome.capsHit.join(", ")})`)
  }
  if (outcome.summary?.usageCompleteness === "unavailable") {
    caveats.push("usage unavailable")
  }
  return caveats
}

/** Bounded per-recall honesty surface. Carries tier/status/caps/coverage/usage
 * for the run monitor and NEVER asserts verification. */
export interface FullAutoRecallHonesty {
  readonly runRef: string
  readonly recallRef: string
  readonly tier: FullAutoRecallTier
  readonly status: FullAutoRecallStatus
  readonly reason: string | null
  readonly modelCalls: number
  readonly totalTokens: number | null
  readonly usageCompleteness: "complete" | "partial" | "unavailable"
  readonly capsHit: ReadonlyArray<string>
  readonly coverageNote: string | null
  readonly citationCount: number
  readonly strategyRef: string | null
  /** Always false — recall output is a cited candidate, never verified. */
  readonly verified: false
}

export const fullAutoRecallHonesty = (
  outcome: FullAutoRecallOutcome,
): FullAutoRecallHonesty => ({
  runRef: outcome.runRef,
  recallRef: outcome.recallRef,
  tier: outcome.tier,
  status: outcome.status,
  reason: outcome.reason,
  modelCalls: outcome.usage.modelCalls,
  totalTokens:
    outcome.summary?.totalTokens ??
    (outcome.usage.modelCalls > 0 ? outcome.usage.totalTokens : null),
  usageCompleteness:
    outcome.summary?.usageCompleteness ??
    (outcome.usage.modelCalls > 0 ? "complete" : "unavailable"),
  capsHit: outcome.capsHit,
  coverageNote: outcome.coverageNote,
  citationCount: outcome.citedSpans.length,
  strategyRef: outcome.resultRefs.strategyRef ?? (outcome.tier === "semantic" ? DESKTOP_RLM_STRATEGY_REF : null),
  verified: false,
})

/** The bounded cited candidate inserted into continuation context. Its `text`
 * is safe to prepend to a continuation prompt: it carries ONLY the bounded
 * validated citations plus honesty caveats — never raw corpus slices and never
 * the recursive transcript. */
export interface FullAutoRecallContinuationFragment {
  readonly runRef: string
  readonly recallRef: string
  readonly tier: FullAutoRecallTier
  readonly status: FullAutoRecallStatus
  readonly text: string
  readonly citedSpans: ReadonlyArray<HistoryRecallCitedSpanRow>
  readonly caveats: ReadonlyArray<string>
  readonly usageLine: string
}

const boundExcerpt = (excerpt: string): string =>
  excerpt.length <= FULL_AUTO_RECALL_FRAGMENT_MAX_EXCERPT
    ? excerpt
    : `${excerpt.slice(0, FULL_AUTO_RECALL_FRAGMENT_MAX_EXCERPT)}…`

const recallUsageLine = (outcome: FullAutoRecallOutcome): string => {
  const honesty = fullAutoRecallHonesty(outcome)
  const tokens =
    honesty.totalTokens === null
      ? "tokens unavailable"
      : `${honesty.totalTokens} tokens (${honesty.usageCompleteness})`
  return `${honesty.modelCalls} model calls · ${tokens}`
}

/**
 * Build the bounded cited-candidate fragment for a recall outcome, or null when
 * there is nothing validated to frame (refused/failed/unavailable/budget, or a
 * frameable status that produced zero validated citations). A null fragment
 * means the continuation proceeds with no recalled context — never a stall.
 */
export const fullAutoRecallContinuationFragment = (
  outcome: FullAutoRecallOutcome,
): FullAutoRecallContinuationFragment | null => {
  if (!fullAutoRecallIsFrameable(outcome)) return null
  const spans = outcome.citedSpans.slice(0, FULL_AUTO_RECALL_FRAGMENT_MAX_SPANS)
  if (spans.length === 0) return null
  const caveats = fullAutoRecallCaveats(outcome)
  const usageLine = recallUsageLine(outcome)
  const header = `RECALL (cited candidate — NOT verified · tier=${outcome.tier} · status=${outcome.status}):`
  const spanLines = spans.map(
    (span) =>
      `- [${span.turnId}#${span.sequenceStart}-${span.sequenceEnd}] ${boundExcerpt(span.excerpt)}`,
  )
  const text = [
    header,
    ...spanLines,
    `caveats: ${caveats.join("; ")}`,
    `usage: ${usageLine}`,
  ].join("\n")
  return {
    runRef: outcome.runRef,
    recallRef: outcome.recallRef,
    tier: outcome.tier,
    status: outcome.status,
    text,
    citedSpans: spans,
    caveats,
    usageLine,
  }
}

/**
 * Splice a recall fragment into a continuation prompt. When there is no
 * fragment the base continuation text is returned unchanged, so a
 * refused/failed/unavailable recall never alters continuation behavior beyond
 * omitting recalled context.
 */
export const applyFullAutoRecallToContinuation = (
  baseContinuation: string,
  fragment: FullAutoRecallContinuationFragment | null,
): string =>
  fragment === null ? baseContinuation : `${fragment.text}\n\n${baseContinuation}`

// ===========================================================================
// HANDS-5 (#9176) — FOLDER-corpus roadmap recall for Full Auto.
//
// The recall consumer above reads a run's OWN authorized event-history corpus.
// This second entry point lets a Full Auto run mine a bounded read-only FOLDER
// of Markdown (the transcript archive under `docs/transcripts/`, and by
// extension repository docs) into a CITED CANDIDATE feature / roadmap list.
//
// It is host-owned and paper-faithful in the same way as the history path:
//   - Tier D deterministic grep runs first and is ALWAYS available, zero spend.
//   - Tier S semantic synthesis runs ONLY when the host passes an admission
//     record plus an injected model plan; the budget is clamped and exact usage
//     is required.
//   - Every result is labeled `cited-candidate` and `verified: false`. It is
//     NEVER a roadmap authority. Citations are validated by the SDK against the
//     exact corpus digest; a Commit without a citation value yields
//     `invalid_citations` and no candidates.
//   - The returned Effect never fails typed: a corpus/engine error becomes a
//     `failed` result the Full Auto loop continues past, exactly like the
//     history recall consumer.
//
// The Full Auto reconcile loop (owned by another lane) integrates the call
// site. It should call {@link runFullAutoRoadmapRecall} and, if it wants to
// frame the result, {@link fullAutoRoadmapRecallText}. This module deliberately
// does NOT touch `full-auto-reconcile.ts` / `full-auto-mission.ts` /
// `full-auto-run-registry.ts` / `full-auto-run-analyzer.ts`.
// ===========================================================================

/**
 * Default feature-signal grep pattern (a real regular expression, case
 * insensitive) for mining a transcript / docs corpus. It is a bounded starting
 * heuristic, not a promise catalogue.
 */
export const FULL_AUTO_ROADMAP_SIGNAL_PATTERN =
  "(we need|we should|we want|missing|instead of|the product (should|must|needs)|must (support|have|be)|would be great|roadmap|feature)"

export const FULL_AUTO_ROADMAP_MAX_CANDIDATES_DEFAULT = 24
export const FULL_AUTO_ROADMAP_MAX_EXCERPT = 200

const RLM_REQUEST_SCHEMA_ID = "openagents.ai.rlm_request.v1" as const

export type FullAutoRoadmapRecallTier = "deterministic" | "semantic"

export type FullAutoRoadmapRecallStatus = "completed" | "partial" | "refused" | "failed"

/** One bounded cited candidate mined from the folder corpus. */
export interface FullAutoRoadmapCandidate {
  /** Stable corpus entry ref (`<relPath>#p<index>`). */
  readonly entryRef: string
  /** Source file relative path, decoded from the citation address. */
  readonly sourceFile: string | null
  /** Bounded validated excerpt — the ONLY corpus-derived text surfaced. */
  readonly excerpt: string
}

/**
 * A folder-corpus recall outcome. It is ALWAYS a cited candidate, never a
 * verified roadmap and never product authority.
 */
export interface FullAutoRoadmapRecallResult {
  readonly runRef: string
  readonly recallRef: string
  readonly tier: FullAutoRoadmapRecallTier
  readonly status: FullAutoRoadmapRecallStatus
  readonly reason: string | null
  /** ALWAYS "cited-candidate". */
  readonly label: "cited-candidate"
  /** ALWAYS false — recall output is never verified. */
  readonly verified: false
  readonly corpusRef: string | null
  readonly contentDigest: string | null
  readonly candidates: ReadonlyArray<FullAutoRoadmapCandidate>
  /** Committed synthesis text for a semantic run; null for Tier D. */
  readonly synthesis: string | null
  readonly citationCount: number
  readonly capsHit: ReadonlyArray<string>
  readonly usage: DesktopRlmUsageTotals
  readonly usageRows: ReadonlyArray<DesktopRlmModelCallUsageRow>
}

/** Host-supplied semantic escalation for the folder path (already admitted). */
export interface FullAutoRoadmapSemanticConfig {
  readonly question: string
  /** Host-owned admission — never from model output. */
  readonly admission: DesktopSemanticRecallAdmission | null
  readonly completeRoot: DesktopRlmCompleteFn
  readonly completeLeaf?: DesktopRlmCompleteFn
  readonly budget?: Partial<RlmBudget>
  readonly recorder?: DesktopRlmUsageRecorder
  readonly onProgress?: (progress: DesktopSemanticRecallProgress) => void
}

export interface FullAutoRoadmapRecallInput {
  readonly runRef: string
  /** Idempotency-friendly ref; also the engine run-ref seed. */
  readonly recallRef: string
  /** Host-owned bounded read-only folder config (root + caps). */
  readonly corpus: FolderRlmCorpusConfig
  /** Tier D grep pattern (full regex). Defaults to the feature-signal pattern. */
  readonly deterministicPattern?: string
  readonly deterministicLimits?: Partial<RlmDeterministicLimits>
  readonly maxCandidates?: number
  /** Optional host-admitted semantic synthesis over the same folder corpus. */
  readonly semantic?: FullAutoRoadmapSemanticConfig
}

const EMPTY_USAGE: DesktopRlmUsageTotals = {
  modelCalls: 0,
  inputTokens: 0,
  outputTokens: 0,
  totalTokens: 0,
}

const boundRoadmapExcerpt = (excerpt: string): string =>
  excerpt.length <= FULL_AUTO_ROADMAP_MAX_EXCERPT
    ? excerpt
    : `${excerpt.slice(0, FULL_AUTO_ROADMAP_MAX_EXCERPT)}…`

/** Decode the source file from a folder citation address (best effort). */
const roadmapSourceFile = (citation: RlmCitation): string | null =>
  splitFolderCorpusAddress(citation.sourceAddress.encodedAddress)?.relPath ?? null

const candidatesFromResult = (
  result: RlmTerminalResult,
  max: number,
): ReadonlyArray<FullAutoRoadmapCandidate> => {
  if (result._tag === "Refused") return []
  const seen = new Set<string>()
  const candidates: Array<FullAutoRoadmapCandidate> = []
  for (const citation of result.citations) {
    if (candidates.length >= max) break
    if (seen.has(citation.entryRefStart)) continue
    seen.add(citation.entryRefStart)
    candidates.push({
      entryRef: citation.entryRefStart,
      sourceFile: roadmapSourceFile(citation),
      excerpt: boundRoadmapExcerpt(citation.excerpt ?? ""),
    })
  }
  return candidates
}

const synthesisFromResult = (result: RlmTerminalResult): string | null => {
  if (result._tag === "Refused") return null
  const output = result._tag === "Completed" ? result.output : result.bestOutput
  if (output === undefined) return null
  return output._tag === "InlineValue" ? output.value : null
}

const roadmapStatusFromResult = (
  result: RlmTerminalResult,
): { readonly status: FullAutoRoadmapRecallStatus; readonly reason: string | null } => {
  switch (result._tag) {
    case "Completed":
      return { status: "completed", reason: null }
    case "Partial":
      return { status: "partial", reason: result.reason }
    case "Refused":
      return { status: "refused", reason: result.reason }
  }
}

const roadmapResultFromTerminal = (
  input: {
    readonly runRef: string
    readonly recallRef: string
    readonly tier: FullAutoRoadmapRecallTier
    readonly maxCandidates: number
    readonly usage: DesktopRlmUsageTotals
    readonly usageRows: ReadonlyArray<DesktopRlmModelCallUsageRow>
  },
  result: RlmTerminalResult,
): FullAutoRoadmapRecallResult => {
  const { status, reason } = roadmapStatusFromResult(result)
  return {
    runRef: input.runRef,
    recallRef: input.recallRef,
    tier: input.tier,
    status,
    reason,
    label: "cited-candidate",
    verified: false,
    corpusRef: result.run.corpusRef,
    contentDigest: result.run.contentDigest,
    candidates: candidatesFromResult(result, input.maxCandidates),
    synthesis: synthesisFromResult(result),
    citationCount: result._tag === "Refused" ? 0 : result.citations.length,
    capsHit: result._tag === "Refused" ? [] : [...result.honesty.capsHit],
    usage: input.usage,
    usageRows: input.usageRows,
  }
}

const failedRoadmapResult = (input: {
  readonly runRef: string
  readonly recallRef: string
  readonly tier: FullAutoRoadmapRecallTier
  readonly reason: string
  readonly usage: DesktopRlmUsageTotals
  readonly usageRows: ReadonlyArray<DesktopRlmModelCallUsageRow>
}): FullAutoRoadmapRecallResult => ({
  runRef: input.runRef,
  recallRef: input.recallRef,
  tier: input.tier,
  status: "failed",
  reason: input.reason,
  label: "cited-candidate",
  verified: false,
  corpusRef: null,
  contentDigest: null,
  candidates: [],
  synthesis: null,
  citationCount: 0,
  capsHit: [],
  usage: input.usage,
  usageRows: input.usageRows,
})

/**
 * Run one folder-corpus roadmap recall for a Full Auto run.
 *
 * Deterministic (Tier D) grep over the bounded folder always runs when no
 * admitted semantic config is supplied — it is free and produces cited
 * candidates directly. When `semantic.admission` is a real admission record and
 * a model plan is supplied, an admitted Tier S synthesis runs over the same
 * folder corpus instead (clamped budget, exact usage required, no artifact
 * sink). A missing admission with a semantic config falls back to Tier D.
 *
 * The returned Effect NEVER fails typed: a corpus or engine error becomes a
 * `failed` result the Full Auto loop continues past. The output is a cited
 * candidate list, never authority.
 *
 * This is the clean entry point the Full Auto reconcile loop should call.
 */
export const runFullAutoRoadmapRecall = (
  input: FullAutoRoadmapRecallInput,
): Effect.Effect<FullAutoRoadmapRecallResult> =>
  Effect.gen(function* () {
    const maxCandidates = Math.max(
      1,
      Math.floor(input.maxCandidates ?? FULL_AUTO_ROADMAP_MAX_CANDIDATES_DEFAULT),
    )
    const recallRunRef = `${input.runRef}::roadmap::${input.recallRef}`
    const admitted = input.semantic !== undefined && input.semantic.admission !== null

    if (admitted && input.semantic !== undefined) {
      const semantic = input.semantic
      const recorder = semantic.recorder ?? makeDesktopRlmUsageRecorder()
      const plan = makeCountedDesktopRlmModelPlan({
        runRef: recallRunRef,
        completeRoot: semantic.completeRoot,
        ...(semantic.completeLeaf === undefined ? {} : { completeLeaf: semantic.completeLeaf }),
        recorder,
      })
      const request: RlmSemanticRequest = {
        _tag: "Semantic",
        schemaId: RLM_REQUEST_SCHEMA_ID,
        runRef: recallRunRef,
        corpus: folderCorpusSourceInput(input.corpus),
        question: semantic.question,
        budget: clampDesktopSemanticBudget(semantic.budget),
        evidence: defaultRlmEvidencePolicy,
        strategyRef: FOLDER_CORPUS_STRATEGY_REF,
      }
      return yield* makeRlm({ admitSemantic: true, model: plan })
        .pipe(Effect.provide(folderRlmCorpusSourceLayer(input.corpus)))
        .pipe(
          Effect.flatMap((rlm) =>
            Stream.runCollect(rlm.stream(request)).pipe(
              Effect.map((events) => {
                let terminal: RlmTerminalResult | null = null
                for (const event of events) {
                  if (semantic.onProgress !== undefined) {
                    const progress = semanticRecallProgressFromRlmEvent(event)
                    if (progress !== null) semantic.onProgress(progress)
                  }
                  if (event._tag === "Terminal") terminal = event.result
                }
                return terminal
              }),
            ),
          ),
          Effect.map((terminal) =>
            terminal === null
              ? failedRoadmapResult({
                  runRef: input.runRef,
                  recallRef: input.recallRef,
                  tier: "semantic",
                  reason: "semantic stream ended without terminal event",
                  usage: recorder.totals(),
                  usageRows: recorder.rows(),
                })
              : roadmapResultFromTerminal(
                  {
                    runRef: input.runRef,
                    recallRef: input.recallRef,
                    tier: "semantic",
                    maxCandidates,
                    usage: recorder.totals(),
                    usageRows: recorder.rows(),
                  },
                  terminal,
                ),
          ),
          Effect.catch((error) =>
            Effect.succeed(
              failedRoadmapResult({
                runRef: input.runRef,
                recallRef: input.recallRef,
                tier: "semantic",
                reason: error.detailSafe ?? error.reason,
                usage: recorder.totals(),
                usageRows: recorder.rows(),
              }),
            ),
          ),
        )
    }

    // Tier D deterministic — free, always available, zero spend.
    const limits: RlmDeterministicLimits = {
      ...defaultRlmDeterministicLimits,
      ...input.deterministicLimits,
    }
    const request: RlmDeterministicRequest = {
      _tag: "Deterministic",
      schemaId: RLM_REQUEST_SCHEMA_ID,
      runRef: recallRunRef,
      corpus: folderCorpusSourceInput(input.corpus),
      operation: {
        _tag: "Grep",
        pattern: input.deterministicPattern ?? FULL_AUTO_ROADMAP_SIGNAL_PATTERN,
        caseSensitive: false,
      },
      limits,
    }
    return yield* makeRlm({})
      .pipe(Effect.provide(folderRlmCorpusSourceLayer(input.corpus)))
      .pipe(
        Effect.flatMap((rlm) => rlm.run(request)),
        Effect.map((terminal) =>
          roadmapResultFromTerminal(
            {
              runRef: input.runRef,
              recallRef: input.recallRef,
              tier: "deterministic",
              maxCandidates,
              usage: EMPTY_USAGE,
              usageRows: [],
            },
            terminal,
          ),
        ),
        Effect.catch((error) =>
          Effect.succeed(
            failedRoadmapResult({
              runRef: input.runRef,
              recallRef: input.recallRef,
              tier: "deterministic",
              reason: error.detailSafe ?? error.reason,
              usage: EMPTY_USAGE,
              usageRows: [],
            }),
          ),
        ),
      )
  })

/** Whether a roadmap recall produced anything frameable (never blocks a run). */
export const fullAutoRoadmapRecallIsFrameable = (
  result: FullAutoRoadmapRecallResult,
): boolean =>
  (result.status === "completed" || result.status === "partial") &&
  result.candidates.length > 0

/**
 * Render a bounded, agent-readable cited-candidate roadmap fragment, or null
 * when there is nothing validated to frame. The text is always headed as a
 * cited candidate and NEVER asserts verification, so it is safe to prepend to a
 * continuation prompt. Raw corpus slices and the recursive transcript are never
 * included — only the bounded validated citations.
 */
export const fullAutoRoadmapRecallText = (
  result: FullAutoRoadmapRecallResult,
): string | null => {
  if (!fullAutoRoadmapRecallIsFrameable(result)) return null
  const header = `ROADMAP RECALL (cited candidate — NOT verified · tier=${result.tier} · status=${result.status}):`
  const lines = result.candidates.map((candidate) => {
    const where = candidate.sourceFile ?? candidate.entryRef
    return `- [${where}] ${candidate.excerpt}`
  })
  const caveats = [
    "cited candidate — not verified",
    ...(result.status === "partial" ? [`partial (${result.reason ?? "capped"})`] : []),
    ...(result.capsHit.length > 0 ? [`coverage truncated (${result.capsHit.join(", ")})`] : []),
  ]
  const usageLine =
    result.usage.modelCalls > 0
      ? `usage: ${result.usage.modelCalls} model calls · ${result.usage.totalTokens} tokens`
      : "usage: 0 model calls (deterministic)"
  return [header, ...lines, `caveats: ${caveats.join("; ")}`, usageLine].join("\n")
}
