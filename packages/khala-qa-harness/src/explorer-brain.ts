import { Effect, Schema as S } from "effect"

import {
  khalaCodeQaCoverageFrontierReport,
  type KhalaCodeQaCoverageFrontierReport,
  type KhalaCodeQaCoverageLedger,
} from "./coverage-ledger.js"
import type { KhalaCodeQaDriver } from "./driver.js"
import { KHALA_CODE_QA_SEED_CORPUS_MANIFEST } from "./seed-corpus.js"
import { runKhalaCodeQaScenario } from "./runner.js"
import {
  KhalaCodeQaAction,
  type KhalaCodeQaAction as KhalaCodeQaActionType,
  type KhalaCodeQaOracleExpectation,
  type KhalaCodeQaScenario,
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
  readonly stepIndex: number
}

export type KhalaCodeQaExplorerBrain = {
  readonly tier: KhalaCodeQaExplorerBrainTier
  readonly nextAction: (
    context: KhalaCodeQaExplorerBrainContext,
  ) => Effect.Effect<KhalaCodeQaExplorerBrainDecision, KhalaCodeQaExplorerBrainFailure>
}

export type KhalaCodeQaExplorerTrace = {
  readonly schema: "khala_code_qa_explorer_trace.v1"
  readonly actionLog: ReadonlyArray<{
    readonly action: ExplorerAction
    readonly frontierRef?: string
    readonly index: number
    readonly rationale: string
  }>
  readonly explorer: "seeded_monkey" | "llm"
  readonly runId: string
  readonly status: "pass" | "fail" | "interesting"
}

export type KhalaCodeQaDistilledScenario = {
  readonly schema: "khala_code_qa_distilled_scenario.v1"
  readonly sourceRunId: string
  readonly sourceStatus: KhalaCodeQaExplorerTrace["status"]
  readonly scenario: KhalaCodeQaScenario
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
    const decoded = S.decodeUnknownSync(KhalaCodeQaExplorerBrainDecision)(input) as KhalaCodeQaExplorerBrainDecision
    return decoded
  } catch (cause) {
    return explorerBrainFailure(
      cause instanceof Error ? cause.message : String(cause),
      cause,
    )
  }
}

const missingRefs = (frontier: KhalaCodeQaCoverageFrontierReport): readonly string[] => [
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

export const chooseKhalaCodeQaFrontierAction = (input: {
  readonly actionSpace: ReadonlyArray<ExplorerAction>
  readonly fallbackIndex: number
  readonly frontier: KhalaCodeQaCoverageFrontierReport
}): KhalaCodeQaExplorerBrainDecision => {
  const frontierRefs = missingRefs(input.frontier)
  for (const frontierRef of frontierRefs) {
    const action = input.actionSpace.find((candidate) =>
      khalaCodeQaActionCoverageRefs(candidate).includes(frontierRef)
    )
    if (action !== undefined) {
      return {
        action,
        frontierRef,
        rationale: `frontier:${frontierRef}`,
        tier: "deterministic_fixture",
      }
    }
  }
  const action = input.actionSpace[input.fallbackIndex % input.actionSpace.length]
  return {
    action: action as ExplorerAction,
    rationale: "frontier exhausted; deterministic fallback",
    tier: "deterministic_fixture",
  }
}

export const makeKhalaCodeQaDeterministicFixtureBrain = (): KhalaCodeQaExplorerBrain => ({
  nextAction: (context) =>
    Effect.succeed(
      chooseKhalaCodeQaFrontierAction({
        actionSpace: context.actionSpace,
        fallbackIndex: context.stepIndex,
        frontier: context.frontier,
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
        return Effect.fail(explorerBrainFailure("Live LLM explorer brain transport is not wired in this package; inject a live brain implementation explicitly"))
      }
      return options.decide(context).pipe(
        Effect.flatMap((decision) => {
          const decoded = decodeKhalaCodeQaExplorerBrainDecision(decision)
          return "_tag" in decoded
            ? Effect.fail(decoded)
            : Effect.succeed({ ...decoded, tier: "live_llm" as const })
        }),
        Effect.mapError((cause) =>
          "_tag" in Object(cause)
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

const expectationsForDistilledActions = (
  actions: readonly ExplorerAction[],
): readonly KhalaCodeQaOracleExpectation[] => {
  const rpcMethods = [...new Set(actions.flatMap((action) =>
    action.kind === "rpc_call" ? [action.method] : []
  ))]
  return [
    ...rpcMethods.map((method): KhalaCodeQaOracleExpectation => ({
      oracle: "schema",
      query: method,
    })),
    { oracle: "crash" },
  ]
}

export const distillKhalaCodeQaExplorerTraceToScenario = (
  trace: KhalaCodeQaExplorerTrace,
): KhalaCodeQaDistilledScenario => {
  const actions = trace.actionLog.map((entry) => entry.action)
  return {
    scenario: {
      backend: "fixture",
      commitments: [{
        claim: "distilled explorer trace replays deterministically",
        evidence: "run-pass",
        id: "distilled.replay_pass",
      }],
      id: `scenario.khala_code.distilled.${trace.runId}.v1`,
      modes: ["rpc"],
      phases: [{
        act: actions,
        expect: expectationsForDistilledActions(actions),
        name: "distilled-replay",
      }],
    },
    schema: "khala_code_qa_distilled_scenario.v1",
    sourceRunId: trace.runId,
    sourceStatus: trace.status,
  }
}

export const regressKhalaCodeQaDistilledScenario = (input: {
  readonly distilled: KhalaCodeQaDistilledScenario
  readonly driver: KhalaCodeQaDriver
}) =>
  runKhalaCodeQaScenario({
    driver: input.driver,
    scenario: input.distilled.scenario,
  })
