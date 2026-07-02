import { Effect, Schema as S } from "effect"

import {
  khalaCodeQaCoverageFrontierReport,
  type KhalaCodeQaCoverageFrontierReport,
  type KhalaCodeQaCoverageLedger,
} from "./coverage-ledger.js"
import { KHALA_CODE_QA_SEED_CORPUS_MANIFEST } from "./seed-corpus.js"
import {
  KhalaCodeQaAction,
  type KhalaCodeQaAction as KhalaCodeQaActionType,
} from "./scenario.js"

type ExplorerAction = KhalaCodeQaActionType

export const KhalaCodeQaExplorerBrainTier = S.Literals(["deterministic_fixture", "live_llm"])
export type KhalaCodeQaExplorerBrainTier = "deterministic_fixture" | "live_llm"

export const KhalaCodeQaExplorerBrainDecision = S.Struct({
  action: KhalaCodeQaAction,
  frontierRef: S.optional(S.String),
  rationale: S.String,
  tier: KhalaCodeQaExplorerBrainTier,
})
export type KhalaCodeQaExplorerBrainDecision = {
  readonly action: ExplorerAction
  readonly frontierRef?: string
  readonly rationale: string
  readonly tier: KhalaCodeQaExplorerBrainTier
}

export type KhalaCodeQaExplorerBrainFailure = {
  readonly _tag: "KhalaCodeQaExplorerBrainFailure"
  readonly message: string
  readonly cause?: unknown
}

export type KhalaCodeQaExplorerBrainContext = {
  readonly actionLog: ReadonlyArray<{
    readonly action: ExplorerAction
    readonly index: number
  }>
  readonly actionSpace: ReadonlyArray<ExplorerAction>
  readonly frontier: KhalaCodeQaCoverageFrontierReport
  readonly prngState: number
  readonly stepIndex: number
}

export type KhalaCodeQaExplorerBrain = {
  readonly tier: KhalaCodeQaExplorerBrainTier
  readonly nextAction: (
    context: KhalaCodeQaExplorerBrainContext,
  ) => Effect.Effect<KhalaCodeQaExplorerBrainDecision, KhalaCodeQaExplorerBrainFailure>
}

const explorerBrainFailure = (
  message: string,
  cause?: unknown,
): KhalaCodeQaExplorerBrainFailure => ({
  _tag: "KhalaCodeQaExplorerBrainFailure",
  message,
  ...(cause === undefined ? {} : { cause }),
})

export const decodeKhalaCodeQaExplorerBrainDecision = (
  input: unknown,
): KhalaCodeQaExplorerBrainDecision | KhalaCodeQaExplorerBrainFailure => {
  try {
    return S.decodeUnknownSync(KhalaCodeQaExplorerBrainDecision)(input) as KhalaCodeQaExplorerBrainDecision
  } catch (cause) {
    return explorerBrainFailure(cause instanceof Error ? cause.message : String(cause), cause)
  }
}

const frontierRefs = (frontier: KhalaCodeQaCoverageFrontierReport): readonly string[] => [
  ...frontier.missing.rpcMethods.map((item) => `rpcMethods:${item}`),
  ...frontier.missing.hotbarPanels.map((item) => `hotbarPanels:${item}`),
  ...frontier.missing.slashCommands.map((item) => `slashCommands:${item}`),
  ...frontier.missing.selectors.map((item) => `selectors:${item}`),
  ...frontier.missing.settingsKeys.map((item) => `settingsKeys:${item}`),
  ...frontier.missing.approvalDecisionKinds.map((item) => `approvalDecisionKinds:${item}`),
  ...frontier.missing.threadItemVariants.map((item) => `threadItemVariants:${item}`),
]

export const khalaCodeQaActionCoverageRefs = (
  action: ExplorerAction,
): readonly string[] => {
  if (action.kind === "rpc_call") {
    const refs = [`rpcMethods:${action.method}`]
    const firstArg = action.args?.[0]
    if (
      action.method === "codexConfigValueWrite" &&
      firstArg !== null &&
      typeof firstArg === "object" &&
      !Array.isArray(firstArg) &&
      typeof (firstArg as { readonly keyPath?: unknown }).keyPath === "string"
    ) {
      refs.push(`settingsKeys:${(firstArg as { readonly keyPath: string }).keyPath}`)
    }
    if (
      action.method === "codexApprovalRespond" &&
      firstArg !== null &&
      typeof firstArg === "object" &&
      !Array.isArray(firstArg) &&
      typeof (firstArg as { readonly action?: unknown }).action === "string"
    ) {
      refs.push(`approvalDecisionKinds:${(firstArg as { readonly action: string }).action}`)
    }
    return refs
  }
  if (action.kind === "hotbar" && action.target !== undefined) return [`hotbarPanels:${action.target}`]
  if (action.kind === "slash_command") {
    const raw = action.value ?? action.text ?? action.target ?? ""
    const command = raw.trim().replace(/^\/+/, "").split(/\s+/)[0]
    return command === "" ? [] : [`slashCommands:${command}`]
  }
  if (action.kind === "click" && action.target !== undefined) return [`selectors:${action.target}`]
  if (action.kind === "approve" && action.value !== undefined) return [`approvalDecisionKinds:${action.value}`]
  return []
}

const pick = <A>(values: readonly A[], state: number): A =>
  values[state % values.length] as A

const prngUnit = (state: number): number => state / 0x100000000

export const chooseKhalaCodeQaFrontierAction = (input: {
  readonly actionSpace: ReadonlyArray<ExplorerAction>
  readonly epsilon?: number
  readonly fallbackIndex?: number
  readonly frontier: KhalaCodeQaCoverageFrontierReport
  readonly prngState: number
}): KhalaCodeQaExplorerBrainDecision => {
  const refs = new Set(frontierRefs(input.frontier))
  const frontierCandidates = input.actionSpace.flatMap((action) =>
    khalaCodeQaActionCoverageRefs(action)
      .filter((ref) => refs.has(ref))
      .map((frontierRef) => ({ action, frontierRef }))
  )
  const epsilon = input.epsilon ?? 0.2
  const exploreAll = frontierCandidates.length === 0 || prngUnit(input.prngState) < epsilon
  if (!exploreAll) {
    const candidate = pick(frontierCandidates, input.prngState)
    return {
      action: candidate.action,
      frontierRef: candidate.frontierRef,
      rationale: `frontier-bias:${candidate.frontierRef}`,
      tier: "deterministic_fixture",
    }
  }
  const action = pick(
    input.actionSpace,
    input.fallbackIndex === undefined ? input.prngState : input.prngState + input.fallbackIndex,
  )
  return {
    action,
    rationale: frontierCandidates.length === 0
      ? "frontier exhausted; seeded fallback"
      : "epsilon exploration; seeded fallback",
    tier: "deterministic_fixture",
  }
}

export const makeKhalaCodeQaDeterministicFixtureBrain = (options: {
  readonly epsilon?: number
} = {}): KhalaCodeQaExplorerBrain => ({
  nextAction: (context) =>
    Effect.succeed(
      chooseKhalaCodeQaFrontierAction({
        actionSpace: context.actionSpace,
        fallbackIndex: context.stepIndex,
        frontier: context.frontier,
        prngState: context.prngState,
        ...(options.epsilon === undefined ? {} : { epsilon: options.epsilon }),
      }),
    ),
  tier: "deterministic_fixture",
})

export const makeKhalaCodeQaLiveLlmExplorerBrain = (options: {
  readonly decide?: (
    context: KhalaCodeQaExplorerBrainContext,
  ) => Effect.Effect<unknown, unknown>
  readonly env?: Record<string, string | undefined>
} = {}): KhalaCodeQaExplorerBrain => {
  const env = options.env ?? (typeof process === "undefined" ? {} : process.env)
  return {
    nextAction: (context) => {
      if (env.KHALA_QA_LIVE_EXPLORER_BRAIN !== "1") {
        return Effect.fail(explorerBrainFailure("Live LLM explorer brain is disabled; set KHALA_QA_LIVE_EXPLORER_BRAIN=1 to opt in"))
      }
      if (options.decide === undefined) {
        return Effect.fail(explorerBrainFailure("Live LLM explorer brain transport must be injected explicitly"))
      }
      return options.decide(context).pipe(
        Effect.flatMap((decision) => {
          const decoded = decodeKhalaCodeQaExplorerBrainDecision(decision)
          return "_tag" in decoded
            ? Effect.fail(decoded)
            : Effect.succeed({ ...decoded, tier: "live_llm" as const })
        }),
        Effect.mapError((cause) =>
          typeof cause === "object" &&
          cause !== null &&
          "_tag" in cause
            ? cause as KhalaCodeQaExplorerBrainFailure
            : explorerBrainFailure("Live LLM explorer brain failed", cause)
        ),
      )
    },
    tier: "live_llm",
  }
}

export const khalaCodeQaFrontierFromLedger = (
  ledger: KhalaCodeQaCoverageLedger,
): KhalaCodeQaCoverageFrontierReport =>
  khalaCodeQaCoverageFrontierReport({
    ledger,
    manifest: KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
  })
