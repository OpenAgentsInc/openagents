import { Effect } from "effect"

import { KHALA_CODE_QA_SEED_CORPUS_MANIFEST } from "./seed-corpus.js"
import {
  collectKhalaCodeQaCoverageLedger,
  createEmptyKhalaCodeQaCoverageLedger,
  mergeKhalaCodeQaCoverageLedgers,
  type KhalaCodeQaCoverageLedger,
} from "./coverage-ledger.js"
import type { KhalaCodeQaDriver } from "./driver.js"
import {
  khalaCodeQaFrontierFromLedger,
  makeKhalaCodeQaDeterministicFixtureBrain,
  type KhalaCodeQaExplorerBrain,
  type KhalaCodeQaExplorerBrainDecision,
} from "./explorer-brain.js"
import {
  KhalaCodeRpcMethodNames,
  type KhalaCodeRpcMethodName,
} from "./rpc-client.js"
import {
  runKhalaCodeQaScenario,
  type KhalaCodeQaScenarioRunReport,
} from "./runner.js"
import type {
  KhalaCodeQaAction,
  KhalaCodeQaOracleExpectation,
  KhalaCodeQaScenario,
} from "./scenario.js"

export type KhalaCodeQaMonkeyMode = "fixture_smoke" | "fleet_cockpit_night"

export type KhalaCodeQaMonkeyActionLogEntry = {
  readonly index: number
  readonly prngState: number
  readonly action: KhalaCodeQaAction
  readonly frontierRef?: string
  readonly rationale?: string
}

export type KhalaCodeQaMonkeyRunPlan = {
  readonly schema: "khala_code_qa_seeded_monkey_plan.v1"
  readonly actionLog: readonly KhalaCodeQaMonkeyActionLogEntry[]
  readonly actionSpaceSize: number
  readonly mode: KhalaCodeQaMonkeyMode
  readonly scenario: KhalaCodeQaScenario
  readonly seed: string
}

export type KhalaCodeQaMonkeyRunReport = {
  readonly schema: "khala_code_qa_seeded_monkey_report.v1"
  readonly actionLog: readonly KhalaCodeQaMonkeyActionLogEntry[]
  readonly coverageLedger: KhalaCodeQaCoverageLedger
  readonly mode: KhalaCodeQaMonkeyMode
  readonly scenario: KhalaCodeQaScenario
  readonly seed: string
  readonly status: "pass" | "fail"
  readonly scenarioReports: readonly KhalaCodeQaScenarioRunReport[]
}

export type KhalaCodeQaMonkeyOptions = {
  readonly mode?: KhalaCodeQaMonkeyMode
  readonly previousCoverageLedger?: KhalaCodeQaCoverageLedger
  readonly seed: string
  readonly steps: number
}

export type KhalaCodeQaExplorerTrace = {
  readonly schema: "khala_code_qa_explorer_trace.v1"
  readonly actionLog: readonly KhalaCodeQaMonkeyActionLogEntry[]
  readonly explorer: "seeded_monkey" | "llm"
  readonly oracleExpectations: readonly KhalaCodeQaOracleExpectation[]
  readonly runId: string
  readonly status: "pass" | "fail"
}

export type KhalaCodeQaDistilledScenario = {
  readonly schema: "khala_code_qa_distilled_scenario.v1"
  readonly sourceRunId: string
  readonly sourceStatus: KhalaCodeQaExplorerTrace["status"]
  readonly scenario: KhalaCodeQaScenario
}

const desktopSessionId = "desktop-session-fixture"
const threadId = "thread-fixture"
const turnId = "turn-fixture"
const runRef = "fleet-run-fixture"

const fnv1a32 = (value: string): number => {
  let hash = 0x811c9dc5
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 0x01000193)
  }
  return hash >>> 0
}

const nextPrngState = (state: number): number =>
  (Math.imul(state, 1664525) + 1013904223) >>> 0

const pick = <A>(values: readonly A[], state: number): A =>
  values[state % values.length] as A

const composerFuzzCorpus = [
  "fixture hello",
  "/status",
  "unicode-ish ascii fallback: cafe resume",
  "try @workspace mention",
  "multi word composer fuzz input",
] as const

const fleetSelectors = [
  ...KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.selectors,
  ".khala-fleet-run-header",
  ".khala-fleet-worker-card",
] as const

export const rpcArgsByMethod: Partial<Record<KhalaCodeRpcMethodName, readonly unknown[]>> = {
  appInfo: [],
  codingStatus: [],
  codexApprovalRespond: [{
    action: "accept",
    method: "item/commandExecution/requestApproval",
    requestId: "approval-fixture",
  }],
  codexBackgroundTerminalsList: [{ limit: 10, threadId }],
  codexConfigValueWrite: [{ keyPath: "model", value: "gpt-5.1-codex" }],
  codexEcosystemRead: [{ section: "mcp" }],
  codexFleetDelegateRun: [{ objective: "fixture monkey delegation", mode: "fixture", count: 1, noRun: true }],
  codexFleetStatus: [],
  codexMcpServerReload: [],
  codexMentionCandidates: [{ query: "fixture" }],
  codexSettingsRead: [{}],
  codexThreadArchive: [{ threadId }],
  codexThreadCompact: [{ sessionId: desktopSessionId, threadId }],
  codexThreadFork: [{ sessionId: desktopSessionId, threadId }],
  codexThreadList: [{ sessionId: desktopSessionId }],
  codexThreadRead: [{ includeTurns: true, threadId }],
  codexThreadRename: [{ name: "Monkey fixture thread", threadId }],
  codexThreadStart: [{ sessionId: desktopSessionId }],
  codexThreadUnarchive: [{ threadId }],
  codexTurnInterrupt: [{ sessionId: desktopSessionId, turnId }],
  codexTurnStart: [{
    messages: [{ body: "fixture monkey turn", id: "msg-monkey", role: "user" }],
    sessionId: desktopSessionId,
    threadId,
    turnId,
  }],
  codexTurnSteer: [{ sessionId: desktopSessionId, text: "fixture monkey steer", turnId }],
  fleetRunControl: [{ runRef, verb: "pause" }],
  fleetRunList: [{}],
  fleetRunStart: [{
    objective: "fixture monkey fleet run",
    runRef,
    targetConcurrency: 1,
    workerKind: "codex",
    workSource: { count: 1, kind: "fixture" },
  }],
  fleetRunStatus: [{ runRef }],
  slashCommandDispatch: [{
    activeTurn: false,
    debug: true,
    platform: "darwin",
    raw: "/status",
    sessionId: desktopSessionId,
    sideConversation: false,
  }],
  slashCommandList: [{}],
}

const enabledRpcMethods = KhalaCodeRpcMethodNames
  .filter((method) => rpcArgsByMethod[method] !== undefined)
  .sort()

const rpcAction = (method: KhalaCodeRpcMethodName): KhalaCodeQaAction => {
  const args = rpcArgsByMethod[method]
  return args === undefined || args.length === 0
    ? { kind: "rpc_call", method }
    : { args, kind: "rpc_call", method }
}

export const khalaCodeQaMonkeyEnabledActionSpace = (
  mode: KhalaCodeQaMonkeyMode = "fixture_smoke",
): readonly KhalaCodeQaAction[] => {
  const rpcActions = enabledRpcMethods.map(rpcAction)
  const slashActions = KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.slashCommands.map((command): KhalaCodeQaAction => ({
    kind: "slash_command",
    value: `/${command}`,
  }))
  const hotbarActions = KHALA_CODE_QA_SEED_CORPUS_MANIFEST.coverage.hotbarPanels.map((target): KhalaCodeQaAction => ({
    kind: "hotbar",
    target,
  }))
  const selectorActions = fleetSelectors.map((target): KhalaCodeQaAction => ({
    kind: "click",
    target,
  }))
  const composerActions = composerFuzzCorpus.map((text): KhalaCodeQaAction => ({
    kind: "type",
    target: "composer",
    text,
  }))
  const submitActions: readonly KhalaCodeQaAction[] = [
    { kind: "submit_composer", target: "composer" },
    { kind: "read", query: "screenshot:fleet-cockpit" },
  ]
  const fleetCockpitActions: readonly KhalaCodeQaAction[] = mode === "fleet_cockpit_night"
    ? [
      rpcAction("fleetRunStart"),
      rpcAction("fleetRunStatus"),
      rpcAction("fleetRunList"),
      { kind: "hotbar", target: "fleet" },
    ]
    : []
  return [
    ...rpcActions,
    ...slashActions,
    ...hotbarActions,
    ...selectorActions,
    ...composerActions,
    ...submitActions,
    ...fleetCockpitActions,
  ]
}

const ledgerForActionLog = (
  runId: string,
  actionLog: readonly KhalaCodeQaMonkeyActionLogEntry[],
): KhalaCodeQaCoverageLedger =>
  collectKhalaCodeQaCoverageLedger({
    generatedAt: "1970-01-01T00:00:00.000Z",
    observations: actionLog.map((entry) => ({
      action: entry.action,
      label: entry.action.kind,
      ok: true,
    })),
    runId,
  })

const expectationsForActions = (
  actions: readonly KhalaCodeQaAction[],
  mode: KhalaCodeQaMonkeyMode,
): readonly KhalaCodeQaOracleExpectation[] => {
  const rpcMethods = actions.flatMap((action) =>
    action.kind === "rpc_call" ? [action.method] : []
  )
  return [
    ...[...new Set(rpcMethods)].map((method): KhalaCodeQaOracleExpectation => ({
      oracle: "schema",
      query: method,
    })),
    { oracle: "crash" },
    ...(mode === "fleet_cockpit_night"
      ? [{ id: "claim-invariant", oracle: "invariant" as const }]
      : []),
  ]
}

export const buildKhalaCodeQaSeededMonkeyPlan = (
  options: KhalaCodeQaMonkeyOptions,
): KhalaCodeQaMonkeyRunPlan =>
  Effect.runSync(buildKhalaCodeQaSeededMonkeyPlanEffect({ options }))

export const buildKhalaCodeQaSeededMonkeyPlanEffect = (input: {
  readonly brain?: KhalaCodeQaExplorerBrain
  readonly options: KhalaCodeQaMonkeyOptions
}): Effect.Effect<KhalaCodeQaMonkeyRunPlan, never> => Effect.gen(function* () {
  const options = input.options
  const brain = input.brain ?? makeKhalaCodeQaDeterministicFixtureBrain()
  const mode = options.mode ?? "fixture_smoke"
  const steps = Math.max(1, Math.trunc(options.steps))
  const actionSpace = khalaCodeQaMonkeyEnabledActionSpace(mode)
  const mandatoryActions: readonly KhalaCodeQaAction[] = mode === "fleet_cockpit_night"
    ? [
      { kind: "hotbar", target: "fleet" },
      rpcAction("fleetRunStart"),
      rpcAction("fleetRunStatus"),
      rpcAction("fleetRunList"),
    ]
    : []
  let state = fnv1a32(options.seed)
  const initialLedger = options.previousCoverageLedger ?? createEmptyKhalaCodeQaCoverageLedger()
  const actionLog: KhalaCodeQaMonkeyActionLogEntry[] = mandatoryActions.slice(0, steps).map((action, index) => ({
    action,
    index,
    prngState: state,
    rationale: "mandatory fleet cockpit smoke surface",
  }))
  for (let index = actionLog.length; index < steps; index += 1) {
    state = nextPrngState(state)
    const planningLedger = mergeKhalaCodeQaCoverageLedgers([
      initialLedger,
      ledgerForActionLog(`scenario.khala_code.monkey.plan.${mode}.${options.seed}.v1`, actionLog),
    ])
    const decision = yield* brain.nextAction({
      actionLog,
      actionSpace,
      frontier: khalaCodeQaFrontierFromLedger(planningLedger),
      prngState: state,
      stepIndex: index,
    }).pipe(
      Effect.catch(() =>
        Effect.succeed({
          action: pick(actionSpace, state),
          rationale: "brain failed; seeded fallback",
          tier: "deterministic_fixture" as const,
        } satisfies KhalaCodeQaExplorerBrainDecision)
      ),
    )
    const typedDecision = decision as KhalaCodeQaExplorerBrainDecision
    actionLog.push({
      action: typedDecision.action,
      ...(typedDecision.frontierRef === undefined ? {} : { frontierRef: typedDecision.frontierRef }),
      index,
      prngState: state,
      rationale: typedDecision.rationale,
    })
  }
  const actions = actionLog.map((entry) => entry.action)
  const scenario: KhalaCodeQaScenario = {
    backend: "fixture",
    commitments: [
      {
        claim: "seeded monkey action log completed without crashes",
        evidence: "run-pass",
        id: "monkey.run_pass",
      },
      ...(mode === "fleet_cockpit_night"
        ? [{
          claim: "fleet cockpit monkey preserved the claim invariant",
          evidence: "phase-oracle" as const,
          id: "monkey.claim_invariant",
          match: "monkey-walk:invariant",
        }]
        : []),
    ],
    id: `scenario.khala_code.monkey.${mode}.${options.seed}.v1`,
    modes: ["rpc"],
    phases: [{
      act: actions,
      expect: expectationsForActions(actions, mode),
      name: "monkey-walk",
    }],
  }
  return {
    actionLog,
    actionSpaceSize: actionSpace.length,
    mode,
    scenario,
    schema: "khala_code_qa_seeded_monkey_plan.v1",
    seed: options.seed,
  }
})

export const replayKhalaCodeQaSeededMonkeyPlan = (input: {
  readonly driver: KhalaCodeQaDriver
  readonly plan: KhalaCodeQaMonkeyRunPlan
}): Effect.Effect<KhalaCodeQaMonkeyRunReport, never> =>
  Effect.map(
    runKhalaCodeQaScenario({ driver: input.driver, scenario: input.plan.scenario }),
    (report): KhalaCodeQaMonkeyRunReport => ({
      actionLog: input.plan.actionLog,
      coverageLedger: report.coverageLedger,
      mode: input.plan.mode,
      scenario: input.plan.scenario,
      scenarioReports: [report],
      schema: "khala_code_qa_seeded_monkey_report.v1",
      seed: input.plan.seed,
      status: report.status,
    }),
  )

export const runKhalaCodeQaSeededMonkey = (input: {
  readonly brain?: KhalaCodeQaExplorerBrain
  readonly driver: KhalaCodeQaDriver
  readonly options: KhalaCodeQaMonkeyOptions
}): Effect.Effect<KhalaCodeQaMonkeyRunReport, never> =>
  buildKhalaCodeQaSeededMonkeyPlanEffect({
    options: input.options,
    ...(input.brain === undefined ? {} : { brain: input.brain }),
  }).pipe(
    Effect.flatMap((plan) => replayKhalaCodeQaSeededMonkeyPlan({ driver: input.driver, plan })),
  )

export const mergeKhalaCodeQaMonkeyCoverage = (
  reports: readonly KhalaCodeQaMonkeyRunReport[],
): KhalaCodeQaCoverageLedger =>
  mergeKhalaCodeQaCoverageLedgers(reports.map((report) => report.coverageLedger))

export const khalaCodeQaExplorerTraceFromMonkeyReport = (
  report: KhalaCodeQaMonkeyRunReport,
): KhalaCodeQaExplorerTrace => ({
  actionLog: report.actionLog,
  explorer: "seeded_monkey",
  oracleExpectations: report.scenario.phases.flatMap((phase) => phase.expect),
  runId: `monkey.${report.mode}.${report.seed}`,
  schema: "khala_code_qa_explorer_trace.v1",
  status: report.status,
})

const expectationsForDistilledReport = (
  report: KhalaCodeQaMonkeyRunReport,
): readonly KhalaCodeQaOracleExpectation[] =>
  report.scenario.phases.flatMap((phase) => phase.expect)

export const distillKhalaCodeQaMonkeyReportToScenario = (
  report: KhalaCodeQaMonkeyRunReport,
): KhalaCodeQaDistilledScenario => {
  const expectations = expectationsForDistilledReport(report)
  return {
    scenario: {
      backend: "fixture",
      commitments: [{
        claim: "distilled explorer trace replays deterministically",
        evidence: "run-pass",
        id: "distilled.replay_pass",
      }],
      id: `scenario.khala_code.distilled.${report.mode}.${report.seed}.v1`,
      modes: ["rpc"],
      phases: [{
        act: report.actionLog.map((entry) => entry.action),
        expect: expectations.length === 0 ? [{ oracle: "crash" }] : expectations,
        name: "distilled-replay",
      }],
    },
    schema: "khala_code_qa_distilled_scenario.v1",
    sourceRunId: `monkey.${report.mode}.${report.seed}`,
    sourceStatus: report.status,
  }
}

export const regressKhalaCodeQaDistilledScenario = (input: {
  readonly distilled: KhalaCodeQaDistilledScenario
  readonly driver: KhalaCodeQaDriver
}): Effect.Effect<KhalaCodeQaScenarioRunReport, never> =>
  runKhalaCodeQaScenario({
    driver: input.driver,
    scenario: input.distilled.scenario,
  })
