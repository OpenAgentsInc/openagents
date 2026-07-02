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
  chooseKhalaCodeQaFrontierAction,
  khalaCodeQaFrontierFromLedger,
  type KhalaCodeQaExplorerTrace,
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
  readonly explorerTrace: KhalaCodeQaExplorerTrace
  readonly mode: KhalaCodeQaMonkeyMode
  readonly scenario: KhalaCodeQaScenario
  readonly seed: string
}

export type KhalaCodeQaMonkeyRunReport = {
  readonly schema: "khala_code_qa_seeded_monkey_report.v1"
  readonly actionLog: readonly KhalaCodeQaMonkeyActionLogEntry[]
  readonly coverageLedger: KhalaCodeQaCoverageLedger
  readonly mode: KhalaCodeQaMonkeyMode
  readonly seed: string
  readonly status: "pass" | "fail"
  readonly scenarioReports: readonly KhalaCodeQaScenarioRunReport[]
}

export type KhalaCodeQaMonkeyOptions = {
  readonly coverageLedger?: KhalaCodeQaCoverageLedger
  readonly mode?: KhalaCodeQaMonkeyMode
  readonly seed: string
  readonly steps: number
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

const ledgerForActionLog = (
  actionLog: readonly KhalaCodeQaMonkeyActionLogEntry[],
  fallback: KhalaCodeQaCoverageLedger | undefined,
): KhalaCodeQaCoverageLedger =>
  mergeKhalaCodeQaCoverageLedgers([
    fallback ?? createEmptyKhalaCodeQaCoverageLedger(),
    collectKhalaCodeQaCoverageLedger({
      observations: actionLog.map((entry) => ({
        action: entry.action,
        label: `explore:${entry.index}`,
        ok: true,
      })),
      runId: "khala-code-qa-explorer-in-progress",
    }),
  ])

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
): KhalaCodeQaMonkeyRunPlan => {
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
  const actionLog: KhalaCodeQaMonkeyActionLogEntry[] = mandatoryActions.slice(0, steps).map((action, index) => ({
    action,
    index,
    prngState: state,
  }))
  for (let index = actionLog.length; index < steps; index += 1) {
    state = nextPrngState(state)
    const frontierDecision = chooseKhalaCodeQaFrontierAction({
      actionSpace,
      fallbackIndex: state,
      frontier: khalaCodeQaFrontierFromLedger(ledgerForActionLog(actionLog, options.coverageLedger)),
    })
    actionLog.push({
      action: frontierDecision.action,
      ...(frontierDecision.frontierRef === undefined ? {} : { frontierRef: frontierDecision.frontierRef }),
      index,
      prngState: state,
      rationale: frontierDecision.rationale,
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
    explorerTrace: {
      actionLog: actionLog.map((entry) => ({
        action: entry.action,
        ...(entry.frontierRef === undefined ? {} : { frontierRef: entry.frontierRef }),
        index: entry.index,
        rationale: entry.rationale ?? "mandatory seeded action",
      })),
      explorer: "seeded_monkey",
      runId: `khala_code.monkey.${mode}.${options.seed}`,
      schema: "khala_code_qa_explorer_trace.v1",
      status: "interesting",
    },
    mode,
    scenario,
    schema: "khala_code_qa_seeded_monkey_plan.v1",
    seed: options.seed,
  }
}

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
      scenarioReports: [report],
      schema: "khala_code_qa_seeded_monkey_report.v1",
      seed: input.plan.seed,
      status: report.status,
    }),
  )

export const runKhalaCodeQaSeededMonkey = (input: {
  readonly driver: KhalaCodeQaDriver
  readonly options: KhalaCodeQaMonkeyOptions
}): Effect.Effect<KhalaCodeQaMonkeyRunReport, never> => {
  const plan = buildKhalaCodeQaSeededMonkeyPlan(input.options)
  return replayKhalaCodeQaSeededMonkeyPlan({ driver: input.driver, plan })
}

export const mergeKhalaCodeQaMonkeyCoverage = (
  reports: readonly KhalaCodeQaMonkeyRunReport[],
): KhalaCodeQaCoverageLedger =>
  mergeKhalaCodeQaCoverageLedgers(reports.map((report) => report.coverageLedger))
