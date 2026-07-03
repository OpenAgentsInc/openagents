import { KhalaCodeRpcMethodNames, type KhalaCodeRpcMethodName } from "./rpc-client.js"
import type { KhalaCodeQaObservation } from "./driver.js"
import type { KhalaCodeQaSeedCorpusManifest, SeedCorpusGroup } from "./seed-corpus.js"

export type KhalaCodeQaCoverageAvailability =
  | "always"
  | "args"
  | "debug"
  | "gap"
  | "side_conversation"
  | "task"
  | "thread"
  | "unavailable"

export type KhalaCodeQaCoverageLedger = {
  readonly schema: "khala_code_qa_coverage_ledger.v1"
  readonly generatedAt: string
  readonly runIds: readonly string[]
  readonly rpcGroups: Readonly<Record<string, {
    readonly calls: number
    readonly methods: readonly string[]
  }>>
  readonly rpcMethods: Readonly<Record<string, {
    readonly calls: number
    readonly distinctArgumentShapeCount: number
    readonly argumentShapes: readonly string[]
  }>>
  readonly slashCommands: Readonly<Record<string, {
    readonly availabilityStateCount: number
    readonly dispatches: number
    readonly availabilityStates: readonly KhalaCodeQaCoverageAvailability[]
  }>>
  readonly hotbarPanelsOpened: readonly string[]
  readonly settingsKeysWritten: readonly string[]
  readonly approvalDecisionKinds: readonly string[]
  readonly crossModeSurfacesExercised: readonly string[]
  readonly fleetRunControlVerbs: readonly string[]
  readonly inboxRoutingFlagKinds: readonly string[]
  readonly errorStateCasesExercised: readonly string[]
  readonly threadItemVariantRenderCounts: Readonly<Record<string, number>>
  readonly threadItemVariantsRendered: readonly string[]
  readonly plannerCoderJudge: {
    readonly advisorAdvisorySeverities: readonly string[]
    readonly advisorGuardRefs: readonly string[]
    readonly architectPlanDecisions: readonly string[]
    readonly judgeVerdictKinds: readonly string[]
    readonly liveSmokeModes: readonly string[]
    readonly modelRoleRegistryRoles: readonly string[]
    readonly roleEconomicsRoleRefs: readonly string[]
  }
  readonly selectorsClicked: readonly string[]
  readonly screensScreenshotted: readonly string[]
}

export type KhalaCodeQaCoverageFrontierReport = {
  readonly schema: "khala_code_qa_coverage_frontier.v1"
  readonly generatedAt: string
  readonly missing: {
    readonly rpcGroups: readonly string[]
    readonly rpcMethods: readonly string[]
    readonly slashCommands: readonly string[]
    readonly hotbarPanels: readonly string[]
    readonly settingsKeys: readonly string[]
    readonly approvalDecisionKinds: readonly string[]
    readonly crossModeSurfaces: readonly string[]
    readonly fleetRunControlVerbs: readonly string[]
    readonly inboxRoutingFlagKinds: readonly string[]
    readonly errorStateCases: readonly string[]
    readonly plannerCoderJudgeAdvisorAdvisorySeverities: readonly string[]
    readonly plannerCoderJudgeAdvisorGuardRefs: readonly string[]
    readonly plannerCoderJudgeArchitectPlanDecisions: readonly string[]
    readonly plannerCoderJudgeJudgeVerdictKinds: readonly string[]
    readonly plannerCoderJudgeLiveSmokeModes: readonly string[]
    readonly plannerCoderJudgeModelRoleRegistryRoles: readonly string[]
    readonly plannerCoderJudgeRoleEconomicsRoleRefs: readonly string[]
    readonly threadItemVariants: readonly string[]
    readonly selectors: readonly string[]
    readonly slashCommandAvailabilityStates: readonly string[]
  }
  readonly zeroForAWeekIssueCandidates: readonly string[]
}

type MutableRpcCoverage = Map<string, { calls: number; argumentShapes: Set<string> }>
type MutableRpcGroupCoverage = Map<string, { calls: number; methods: Set<string> }>
type MutableSlashCoverage = Map<string, { dispatches: number; availabilityStates: Set<KhalaCodeQaCoverageAvailability> }>
type MutableCountCoverage = Map<string, number>
type MutablePlannerCoderJudgeCoverage = {
  advisorAdvisorySeverities: string[]
  advisorGuardRefs: string[]
  architectPlanDecisions: string[]
  judgeVerdictKinds: string[]
  liveSmokeModes: string[]
  modelRoleRegistryRoles: string[]
  roleEconomicsRoleRefs: string[]
}

const emptyGeneratedAt = "1970-01-01T00:00:00.000Z"

type RoadmapRpcGroup = Extract<SeedCorpusGroup, `rpc.${string}`>

export const KHALA_CODE_QA_ROADMAP_RPC_METHOD_GROUPS = {
  "rpc.threads": [
    "codexThreadStart",
    "codexThreadList",
    "codexThreadRead",
    "codexThreadRename",
    "codexThreadArchive",
    "codexThreadUnarchive",
    "codexThreadDelete",
    "codexThreadFork",
    "codexThreadCompact",
    "codexThreadResume",
  ],
  "rpc.turns": ["codexTurnStart", "codexTurnSteer", "codexTurnInterrupt"],
  "rpc.approvals": ["claudeApprovalPending", "claudeApprovalRespond", "codexApprovalRespond"],
  "rpc.settings_config": ["codexSettingsRead", "codexConfigValueWrite", "harnessSettingRead", "harnessSettingWrite"],
  "rpc.models_personality": ["codexSettingsRead", "codexConfigValueWrite", "onDeviceDeciderStatus", "appleFmReadiness"],
  "rpc.ecosystem": [
    "codexEcosystemRead",
    "toolCatalog",
    "codexMcpServerReload",
    "codexMcpResourceRead",
    "codexMcpToolCall",
    "codexMcpOauthLogin",
    "codexSkillsExtraRootsSet",
    "codexSkillsConfigWrite",
    "codexMarketplaceAdd",
    "codexMarketplaceUpgrade",
    "codexMarketplaceRemove",
    "codexPluginInstall",
    "codexPluginUninstall",
    "codexExternalAgentConfigDetect",
    "codexExternalAgentConfigImport",
    "codexExternalAgentConfigImportHistoriesRead",
  ],
  "rpc.fs_mentions_attachments": [
    "codexMentionCandidates",
    "codexFsGetMetadata",
    "codexFsReadFile",
    "codexFsWriteFile",
    "submitChatMessage",
  ],
  "rpc.background_terminals": [
    "codexBackgroundTerminalsList",
    "codexBackgroundTerminalsClean",
    "codexBackgroundTerminalsTerminate",
  ],
  "rpc.slash_commands": ["slashCommandList", "slashCommandDispatch"],
  "rpc.token_summaries": ["tokenAccountingStatus", "threadTokenSummary"],
  "rpc.fleet": ["codexFleetStatus", "codexFleetDelegateRun", "codexFleetPromoteThread"],
  "rpc.fleet_run": ["fleetRunStart", "fleetRunStatus", "fleetRunList", "fleetRunControl"],
  "rpc.session_catalog": ["sessionCatalog"],
  "rpc.forum_panel": ["forumRequest"],
  "rpc.inbox_routing": [
    "codexFleetStatus",
    "pylonStatus",
    "codingStatus",
    "tokenAccountingStatus",
    "codexHarnessStatus",
    "codexEcosystemRead",
    "fleetWorkerControl",
  ],
  "rpc.gym_pane": ["fleetRunStatus", "fleetRunList", "codexFleetStatus"],
  "rpc.plans_billing": ["khalaCodePlanCatalog", "khalaCodePlanStatus", "khalaCodePlanPurchase"],
  "rpc.headless_events": ["submitChatMessage", "fleetRunStatus"],
  "rpc.qa_metrics": ["qaMetricSample", "qaMetrics"],
} as const satisfies Record<RoadmapRpcGroup, readonly KhalaCodeRpcMethodName[]>

const rpcGroupsForMethod = (method: string): readonly string[] =>
  Object.entries(KHALA_CODE_QA_ROADMAP_RPC_METHOD_GROUPS).flatMap(([group, methods]) =>
    (methods as readonly string[]).includes(method) ? [group] : []
  )

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)

const sorted = (values: Iterable<string>): readonly string[] => [...new Set(values)].sort()

const stableShape = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(stableShape).join(",")}]`
  if (!isRecord(value)) return typeof value
  return `{${Object.keys(value).sort().map((key) => `${key}:${stableShape(value[key])}`).join(",")}}`
}

const rpcCoverageToObject = (
  coverage: MutableRpcCoverage,
): KhalaCodeQaCoverageLedger["rpcMethods"] =>
  Object.fromEntries([...coverage.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([method, entry]) => [
    method,
    {
      argumentShapes: sorted(entry.argumentShapes),
      calls: entry.calls,
      distinctArgumentShapeCount: entry.argumentShapes.size,
    },
  ]))

const rpcGroupCoverageToObject = (
  coverage: MutableRpcGroupCoverage,
): KhalaCodeQaCoverageLedger["rpcGroups"] =>
  Object.fromEntries([...coverage.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([group, entry]) => [
    group,
    {
      calls: entry.calls,
      methods: sorted(entry.methods),
    },
  ]))

const slashCoverageToObject = (
  coverage: MutableSlashCoverage,
): KhalaCodeQaCoverageLedger["slashCommands"] =>
  Object.fromEntries([...coverage.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([command, entry]) => {
    const availabilityStates = sorted(entry.availabilityStates) as readonly KhalaCodeQaCoverageAvailability[]
    return [
      command,
      {
        availabilityStateCount: availabilityStates.length,
        availabilityStates,
        dispatches: entry.dispatches,
      },
    ]
  }))

const countCoverageToObject = (
  coverage: MutableCountCoverage,
): Readonly<Record<string, number>> =>
  Object.fromEntries([...coverage.entries()].sort(([left], [right]) => left.localeCompare(right)))

const parseSlashRaw = (raw: unknown): string | undefined => {
  if (typeof raw !== "string") return undefined
  const command = raw.trim().replace(/^\/+/, "").split(/\s+/)[0]
  return command === "" ? undefined : command
}

const availabilityStatesForCommand = (command: Record<string, unknown>): readonly KhalaCodeQaCoverageAvailability[] => {
  const states: KhalaCodeQaCoverageAvailability[] = []
  const availability = isRecord(command.availability) ? command.availability : undefined
  if (availability?.available === false) states.push("unavailable")
  if (command.debug === true) states.push("debug")
  if (command.availableDuringTask === true) states.push("task")
  if (command.availableInSideConversation === true) states.push("side_conversation")
  const dispatch = command.dispatch
  if (isRecord(dispatch) && dispatch.kind === "gap") states.push("gap")
  if (isRecord(dispatch) && dispatch.requiresThread === true) states.push("thread")
  if (isRecord(dispatch) && dispatch.requiresArgs === true) states.push("args")
  const canBecomeUnavailable =
    command.availableDuringTask === false ||
    command.availableInSideConversation === false ||
    isRecord(dispatch) && (dispatch.requiresThread === true || dispatch.requiresArgs === true)
  return states.length === 0 && !canBecomeUnavailable ? ["always"] : states
}

const availabilityStatesForDispatchResult = (
  result: unknown,
): readonly KhalaCodeQaCoverageAvailability[] => {
  if (!isRecord(result)) return []
  switch (result.status) {
    case "blocked":
    case "unavailable":
      return ["unavailable"]
    case "gap":
      return ["gap"]
    default:
      return []
  }
}

const recordRpc = (
  coverage: MutableRpcCoverage,
  method: string,
  args: readonly unknown[],
): void => {
  const entry = coverage.get(method) ?? { calls: 0, argumentShapes: new Set<string>() }
  entry.calls += 1
  entry.argumentShapes.add(stableShape(args))
  coverage.set(method, entry)
}

const recordRpcGroup = (
  coverage: MutableRpcGroupCoverage,
  group: string,
  method: string,
): void => {
  const entry = coverage.get(group) ?? { calls: 0, methods: new Set<string>() }
  entry.calls += 1
  entry.methods.add(method)
  coverage.set(group, entry)
}

const recordSlash = (
  coverage: MutableSlashCoverage,
  command: string,
  options: {
    readonly dispatched?: boolean
    readonly availabilityStates?: readonly KhalaCodeQaCoverageAvailability[]
  } = {},
): void => {
  const entry = coverage.get(command) ?? { dispatches: 0, availabilityStates: new Set<KhalaCodeQaCoverageAvailability>() }
  if (options.dispatched === true) entry.dispatches += 1
  for (const state of options.availabilityStates ?? []) entry.availabilityStates.add(state)
  coverage.set(command, entry)
}

const recordCount = (
  coverage: MutableCountCoverage,
  key: string,
  increment = 1,
): void => {
  coverage.set(key, (coverage.get(key) ?? 0) + increment)
}

const collectThreadItemVariants = (value: unknown): readonly string[] => {
  if (Array.isArray(value)) return value.flatMap(collectThreadItemVariants)
  if (!isRecord(value)) return []
  const variants = [
    typeof value.itemType === "string" ? value.itemType : undefined,
  ].filter((variant): variant is string => variant !== undefined)
  return [...variants, ...Object.values(value).flatMap(collectThreadItemVariants)]
}

const collectErrorStateCaseIds = (value: unknown): readonly string[] => {
  if (Array.isArray(value)) return value.flatMap(collectErrorStateCaseIds)
  if (!isRecord(value)) return []
  const direct = [
    isRecord(value.degradedState) && typeof value.degradedState.caseId === "string"
      ? value.degradedState.caseId
      : undefined,
    typeof value.caseId === "string" && value.kind === "khala_code_qa_error_state"
      ? value.caseId
      : undefined,
  ].filter((caseId): caseId is string => caseId !== undefined)
  const refs = Object.values(value).flatMap((entry) => {
    if (typeof entry !== "string") return collectErrorStateCaseIds(entry)
    const match = entry.match(/qa\.error_state\.([a-z0-9_]+)/)
    return match?.[1] === undefined ? [] : [match[1]]
  })
  return [...direct, ...refs]
}

const collectPlannerCoderJudgeCoverage = (value: unknown): {
  readonly advisorAdvisorySeverities: readonly string[]
  readonly advisorGuardRefs: readonly string[]
  readonly architectPlanDecisions: readonly string[]
  readonly judgeVerdictKinds: readonly string[]
  readonly liveSmokeModes: readonly string[]
  readonly modelRoleRegistryRoles: readonly string[]
  readonly roleEconomicsRoleRefs: readonly string[]
} => {
  if (Array.isArray(value)) {
    return mergePlannerCoderJudgeCoverage(value.map(collectPlannerCoderJudgeCoverage))
  }
  if (!isRecord(value)) return emptyPlannerCoderJudgeCoverage()
  const body = typeof value.body === "string" ? value.body : ""
  const bodyHas = (needle: string): boolean => body.includes(needle)
  const direct = {
    advisorAdvisorySeverities: [
      value.schema === "openagents.khala_code.advisor_advisory.v1" && typeof value.severity === "string"
        ? value.severity
        : undefined,
      ...["blocker", "concern", "nit"].filter((severity) =>
        bodyHas(`advisor_advisory:${severity}`)
      ),
    ].filter((entry): entry is string => entry !== undefined),
    advisorGuardRefs: [
      Array.isArray(value.droppedAdvisories) ? "dedupe_guard" : undefined,
      typeof value.immuneTurnsRemaining === "number" ? "interrupt_budget" : undefined,
      ...["dedupe_guard", "interrupt_budget"].filter((guardRef) =>
        bodyHas(`advisor_guard:${guardRef}`)
      ),
    ].filter((entry): entry is string => entry !== undefined),
    architectPlanDecisions: [
      typeof value.decision === "string" && (value.decision === "approve" || value.decision === "reject")
        ? value.decision
        : undefined,
    ].filter((entry): entry is string => entry !== undefined),
    judgeVerdictKinds: [
      value.schema === "openagents.khala_code.judge_diff_verdict.v1" && typeof value.verdict === "string"
        ? value.verdict
        : undefined,
      ...["accept", "request_changes", "replan"].filter((verdict) =>
        bodyHas(`judge_verdict:${verdict}`)
      ),
    ].filter((entry): entry is string => entry !== undefined),
    liveSmokeModes: [
      value.schema === "openagents.khala_code.architect_coder_judge_live_smoke.v1" && typeof value.mode === "string"
        ? value.mode
        : undefined,
      ...["skip_safe_default", "env_armed"].filter((mode) =>
        bodyHas(`architect_coder_judge_live_smoke:${mode}`)
      ),
    ].filter((entry): entry is string => entry !== undefined),
    modelRoleRegistryRoles: isRecord(value.registry) && isRecord(value.registry.roles)
      ? Object.keys(value.registry.roles)
      : isRecord(value.roles)
        ? Object.keys(value.roles)
        : [],
    roleEconomicsRoleRefs: Array.isArray(value.roleEconomics)
      ? value.roleEconomics.flatMap((entry) =>
        isRecord(entry) && typeof entry.roleRef === "string" ? [entry.roleRef] : []
      )
      : [],
  }
  return mergePlannerCoderJudgeCoverage([
    direct,
    ...Object.values(value).map(collectPlannerCoderJudgeCoverage),
  ])
}

const emptyPlannerCoderJudgeCoverage = (): MutablePlannerCoderJudgeCoverage => ({
  advisorAdvisorySeverities: [],
  advisorGuardRefs: [],
  architectPlanDecisions: [],
  judgeVerdictKinds: [],
  liveSmokeModes: [],
  modelRoleRegistryRoles: [],
  roleEconomicsRoleRefs: [],
})

const mergePlannerCoderJudgeCoverage = (
  entries: readonly (MutablePlannerCoderJudgeCoverage | KhalaCodeQaCoverageLedger["plannerCoderJudge"])[],
): MutablePlannerCoderJudgeCoverage => ({
  advisorAdvisorySeverities: [...sorted(entries.flatMap((entry) => entry.advisorAdvisorySeverities))],
  advisorGuardRefs: [...sorted(entries.flatMap((entry) => entry.advisorGuardRefs))],
  architectPlanDecisions: [...sorted(entries.flatMap((entry) => entry.architectPlanDecisions))],
  judgeVerdictKinds: [...sorted(entries.flatMap((entry) => entry.judgeVerdictKinds))],
  liveSmokeModes: [...sorted(entries.flatMap((entry) => entry.liveSmokeModes))],
  modelRoleRegistryRoles: [...sorted(entries.flatMap((entry) => entry.modelRoleRegistryRoles))],
  roleEconomicsRoleRefs: [...sorted(entries.flatMap((entry) => entry.roleEconomicsRoleRefs))],
})

const collectArmedErrorStateCases = (args: readonly unknown[] | undefined): readonly string[] => {
  const sample = args?.[0]
  if (!isRecord(sample) || !isRecord(sample.context)) return []
  return typeof sample.context.errorStateCase === "string" ? [sample.context.errorStateCase] : []
}

const crossModeSurfaceForReadQuery = (query: string): string | null => {
  const prefix = "projection:"
  return query.startsWith(prefix) ? query.slice(prefix.length) : null
}

export const createEmptyKhalaCodeQaCoverageLedger = (
  options: { readonly generatedAt?: string; readonly runId?: string } = {},
): KhalaCodeQaCoverageLedger => ({
  approvalDecisionKinds: [],
  crossModeSurfacesExercised: [],
  fleetRunControlVerbs: [],
  generatedAt: options.generatedAt ?? emptyGeneratedAt,
  hotbarPanelsOpened: [],
  inboxRoutingFlagKinds: [],
  errorStateCasesExercised: [],
  plannerCoderJudge: emptyPlannerCoderJudgeCoverage(),
  rpcGroups: {},
  rpcMethods: {},
  runIds: options.runId === undefined ? [] : [options.runId],
  schema: "khala_code_qa_coverage_ledger.v1",
  screensScreenshotted: [],
  selectorsClicked: [],
  settingsKeysWritten: [],
  slashCommands: {},
  threadItemVariantRenderCounts: {},
  threadItemVariantsRendered: [],
})

export const collectKhalaCodeQaCoverageLedger = (input: {
  readonly generatedAt?: string
  readonly observations: ReadonlyArray<KhalaCodeQaObservation>
  readonly runId: string
}): KhalaCodeQaCoverageLedger => {
  const rpcGroups: MutableRpcGroupCoverage = new Map()
  const rpcMethods: MutableRpcCoverage = new Map()
  const slashCommands: MutableSlashCoverage = new Map()
  const hotbarPanelsOpened = new Set<string>()
  const settingsKeysWritten = new Set<string>()
  const approvalDecisionKinds = new Set<string>()
  const crossModeSurfacesExercised = new Set<string>()
  const fleetRunControlVerbs = new Set<string>()
  const inboxRoutingFlagKinds = new Set<string>()
  const errorStateCasesExercised = new Set<string>()
  const threadItemVariantRenderCounts: MutableCountCoverage = new Map()
  const threadItemVariantsRendered = new Set<string>()
  const plannerCoderJudge = emptyPlannerCoderJudgeCoverage()
  const selectorsClicked = new Set<string>()
  const screensScreenshotted = new Set<string>()

  for (const observation of input.observations) {
    if (!observation.ok) continue
    const action = observation.action
    if (action.kind === "rpc_call") {
      recordRpc(rpcMethods, action.method, action.args ?? [])
      for (const group of rpcGroupsForMethod(action.method)) {
        recordRpcGroup(rpcGroups, group, action.method)
      }
      const firstArg = action.args?.[0]
      if (action.method === "slashCommandDispatch" && isRecord(firstArg)) {
        const command = parseSlashRaw(firstArg.raw)
        const value = isRecord(observation.data) && isRecord(observation.data.value)
          ? observation.data.value
          : undefined
        if (command !== undefined) {
          recordSlash(slashCommands, command, {
            availabilityStates: availabilityStatesForDispatchResult(value),
            dispatched: true,
          })
        }
      }
      if (action.method === "slashCommandList") {
        const value = isRecord(observation.data) && isRecord(observation.data.value)
          ? observation.data.value
          : undefined
        const commands = Array.isArray(value?.commands) ? value.commands : []
        for (const command of commands) {
          if (isRecord(command) && typeof command.command === "string") {
            recordSlash(slashCommands, command.command, {
              availabilityStates: availabilityStatesForCommand(command),
            })
          }
        }
      }
      if (action.method === "codexConfigValueWrite" && isRecord(firstArg) && typeof firstArg.keyPath === "string") {
        settingsKeysWritten.add(firstArg.keyPath)
      }
      if (action.method === "codexApprovalRespond" && isRecord(firstArg) && typeof firstArg.action === "string") {
        approvalDecisionKinds.add(firstArg.action)
      }
      if (action.method === "fleetRunControl" && isRecord(firstArg) && typeof firstArg.verb === "string") {
        fleetRunControlVerbs.add(firstArg.verb)
      }
      if (action.method === "fleetWorkerControl" && isRecord(firstArg) && typeof firstArg.verb === "string") {
        inboxRoutingFlagKinds.add(firstArg.verb)
      }
      if (action.method === "architectPlanDecision" && isRecord(firstArg) && typeof firstArg.decision === "string") {
        plannerCoderJudge.architectPlanDecisions.push(firstArg.decision)
      }
      if (
        action.method === "qaMetricSample" &&
        isRecord(firstArg) &&
        isRecord(firstArg.context) &&
        firstArg.context.schema === "openagents.khala_code.architect_coder_judge_live_smoke.v1" &&
        typeof firstArg.context.mode === "string"
      ) {
        plannerCoderJudge.liveSmokeModes.push(firstArg.context.mode)
      }
      for (const caseId of collectArmedErrorStateCases(action.args)) {
        errorStateCasesExercised.add(caseId)
      }
      for (const caseId of collectErrorStateCaseIds(observation.data)) {
        errorStateCasesExercised.add(caseId)
      }
      for (const variant of collectThreadItemVariants(observation.data)) {
        recordCount(threadItemVariantRenderCounts, variant)
        threadItemVariantsRendered.add(variant)
      }
      const pcj = collectPlannerCoderJudgeCoverage(observation.data)
      plannerCoderJudge.advisorAdvisorySeverities.push(...pcj.advisorAdvisorySeverities)
      plannerCoderJudge.advisorGuardRefs.push(...pcj.advisorGuardRefs)
      plannerCoderJudge.architectPlanDecisions.push(...pcj.architectPlanDecisions)
      plannerCoderJudge.judgeVerdictKinds.push(...pcj.judgeVerdictKinds)
      plannerCoderJudge.liveSmokeModes.push(...pcj.liveSmokeModes)
      plannerCoderJudge.modelRoleRegistryRoles.push(...pcj.modelRoleRegistryRoles)
      plannerCoderJudge.roleEconomicsRoleRefs.push(...pcj.roleEconomicsRoleRefs)
      continue
    }

    if (action.kind === "hotbar" && action.target !== undefined) hotbarPanelsOpened.add(action.target)
    if (action.kind === "click" && action.target !== undefined) selectorsClicked.add(action.target)
    if (action.kind === "approve" && action.value !== undefined) approvalDecisionKinds.add(action.value)
    if (action.kind === "slash_command") {
      const command = parseSlashRaw(action.value ?? action.text ?? action.target)
      if (command !== undefined) recordSlash(slashCommands, command, { dispatched: true })
    }
    if (action.kind === "read" && action.query.startsWith("screenshot:")) {
      screensScreenshotted.add(action.query.slice("screenshot:".length))
    }
    if (action.kind === "read") {
      const surface = crossModeSurfaceForReadQuery(action.query)
      if (surface !== null) crossModeSurfacesExercised.add(surface)
    }
  }

  return {
    approvalDecisionKinds: sorted(approvalDecisionKinds),
    crossModeSurfacesExercised: sorted(crossModeSurfacesExercised),
    fleetRunControlVerbs: sorted(fleetRunControlVerbs),
    generatedAt: input.generatedAt ?? new Date().toISOString(),
    hotbarPanelsOpened: sorted(hotbarPanelsOpened),
    inboxRoutingFlagKinds: sorted(inboxRoutingFlagKinds),
    errorStateCasesExercised: sorted(errorStateCasesExercised),
    plannerCoderJudge: mergePlannerCoderJudgeCoverage([plannerCoderJudge]),
    rpcGroups: rpcGroupCoverageToObject(rpcGroups),
    rpcMethods: rpcCoverageToObject(rpcMethods),
    runIds: [input.runId],
    schema: "khala_code_qa_coverage_ledger.v1",
    screensScreenshotted: sorted(screensScreenshotted),
    selectorsClicked: sorted(selectorsClicked),
    settingsKeysWritten: sorted(settingsKeysWritten),
    slashCommands: slashCoverageToObject(slashCommands),
    threadItemVariantRenderCounts: countCoverageToObject(threadItemVariantRenderCounts),
    threadItemVariantsRendered: sorted(threadItemVariantsRendered),
  }
}

export const mergeKhalaCodeQaCoverageLedgers = (
  ledgers: ReadonlyArray<KhalaCodeQaCoverageLedger>,
): KhalaCodeQaCoverageLedger => {
  const rpcGroups: MutableRpcGroupCoverage = new Map()
  const rpcMethods: MutableRpcCoverage = new Map()
  const slashCommands: MutableSlashCoverage = new Map()
  const threadItemVariantRenderCounts: MutableCountCoverage = new Map()
  const plannerCoderJudge = mergePlannerCoderJudgeCoverage(ledgers.map((ledger) =>
    ledger.plannerCoderJudge ?? emptyPlannerCoderJudgeCoverage()
  ))
  const runIds = new Set<string>()
  const generatedAt = ledgers.map((ledger) => ledger.generatedAt).sort().at(-1) ?? emptyGeneratedAt
  const addSet = <K extends keyof KhalaCodeQaCoverageLedger>(key: K): Set<string> =>
    new Set(ledgers.flatMap((ledger) => ledger[key] as readonly string[]))

  for (const ledger of ledgers) {
    for (const runId of ledger.runIds) runIds.add(runId)
    for (const [group, entry] of Object.entries(ledger.rpcGroups ?? {})) {
      const target = rpcGroups.get(group) ?? { calls: 0, methods: new Set<string>() }
      target.calls += entry.calls
      for (const method of entry.methods) target.methods.add(method)
      rpcGroups.set(group, target)
    }
    for (const [method, entry] of Object.entries(ledger.rpcMethods)) {
      const target = rpcMethods.get(method) ?? { calls: 0, argumentShapes: new Set<string>() }
      target.calls += entry.calls
      for (const shape of entry.argumentShapes) target.argumentShapes.add(shape)
      rpcMethods.set(method, target)
    }
    for (const [command, entry] of Object.entries(ledger.slashCommands)) {
      const target = slashCommands.get(command) ?? { dispatches: 0, availabilityStates: new Set<KhalaCodeQaCoverageAvailability>() }
      target.dispatches += entry.dispatches
      for (const state of entry.availabilityStates) target.availabilityStates.add(state)
      slashCommands.set(command, target)
    }
    for (const [variant, count] of Object.entries(ledger.threadItemVariantRenderCounts ?? {})) {
      recordCount(threadItemVariantRenderCounts, variant, count)
    }
  }

  return {
    approvalDecisionKinds: sorted(addSet("approvalDecisionKinds")),
    crossModeSurfacesExercised: sorted(addSet("crossModeSurfacesExercised")),
    fleetRunControlVerbs: sorted(addSet("fleetRunControlVerbs")),
    generatedAt,
    hotbarPanelsOpened: sorted(addSet("hotbarPanelsOpened")),
    inboxRoutingFlagKinds: sorted(addSet("inboxRoutingFlagKinds")),
    errorStateCasesExercised: sorted(addSet("errorStateCasesExercised")),
    plannerCoderJudge,
    rpcGroups: rpcGroupCoverageToObject(rpcGroups),
    rpcMethods: rpcCoverageToObject(rpcMethods),
    runIds: sorted(runIds),
    schema: "khala_code_qa_coverage_ledger.v1",
    screensScreenshotted: sorted(addSet("screensScreenshotted")),
    selectorsClicked: sorted(addSet("selectorsClicked")),
    settingsKeysWritten: sorted(addSet("settingsKeysWritten")),
    slashCommands: slashCoverageToObject(slashCommands),
    threadItemVariantRenderCounts: countCoverageToObject(threadItemVariantRenderCounts),
    threadItemVariantsRendered: sorted(addSet("threadItemVariantsRendered")),
  }
}

export const khalaCodeQaCoverageFrontierReport = (input: {
  readonly generatedAt?: string
  readonly ledger: KhalaCodeQaCoverageLedger
  readonly manifest: KhalaCodeQaSeedCorpusManifest
  readonly zeroForAWeek?: ReadonlyArray<string>
}): KhalaCodeQaCoverageFrontierReport => {
  const missing = {
    approvalDecisionKinds: sorted(input.manifest.coverage.approvalDecisionKinds.filter((item) =>
      !input.ledger.approvalDecisionKinds.includes(item)
    )),
    crossModeSurfaces: sorted(input.manifest.coverage.crossModeSurfaces.filter((item) =>
      !input.ledger.crossModeSurfacesExercised.includes(item)
    )),
    fleetRunControlVerbs: sorted(input.manifest.coverage.fleetRunControlVerbs.filter((item) =>
      !input.ledger.fleetRunControlVerbs.includes(item)
    )),
    hotbarPanels: sorted(input.manifest.coverage.hotbarPanels.filter((item) =>
      !input.ledger.hotbarPanelsOpened.includes(item)
    )),
    inboxRoutingFlagKinds: sorted(input.manifest.coverage.inboxRoutingFlagKinds.filter((item) =>
      !input.ledger.inboxRoutingFlagKinds.includes(item)
    )),
    errorStateCases: sorted(input.manifest.coverage.errorStateCases.filter((item) =>
      !input.ledger.errorStateCasesExercised.includes(item)
    )),
    plannerCoderJudgeAdvisorAdvisorySeverities: sorted(input.manifest.coverage.plannerCoderJudge.advisorAdvisorySeverities.filter((item) =>
      !input.ledger.plannerCoderJudge.advisorAdvisorySeverities.includes(item)
    )),
    plannerCoderJudgeAdvisorGuardRefs: sorted(input.manifest.coverage.plannerCoderJudge.advisorGuardRefs.filter((item) =>
      !input.ledger.plannerCoderJudge.advisorGuardRefs.includes(item)
    )),
    plannerCoderJudgeArchitectPlanDecisions: sorted(input.manifest.coverage.plannerCoderJudge.architectPlanDecisions.filter((item) =>
      !input.ledger.plannerCoderJudge.architectPlanDecisions.includes(item)
    )),
    plannerCoderJudgeJudgeVerdictKinds: sorted(input.manifest.coverage.plannerCoderJudge.judgeVerdictKinds.filter((item) =>
      !input.ledger.plannerCoderJudge.judgeVerdictKinds.includes(item)
    )),
    plannerCoderJudgeLiveSmokeModes: sorted(input.manifest.coverage.plannerCoderJudge.liveSmokeModes.filter((item) =>
      !input.ledger.plannerCoderJudge.liveSmokeModes.includes(item)
    )),
    plannerCoderJudgeModelRoleRegistryRoles: sorted(input.manifest.coverage.plannerCoderJudge.modelRoleRegistryRoles.filter((item) =>
      !input.ledger.plannerCoderJudge.modelRoleRegistryRoles.includes(item)
    )),
    plannerCoderJudgeRoleEconomicsRoleRefs: sorted(input.manifest.coverage.plannerCoderJudge.roleEconomicsRoleRefs.filter((item) =>
      !input.ledger.plannerCoderJudge.roleEconomicsRoleRefs.includes(item)
    )),
    rpcGroups: sorted(input.manifest.coverage.rpcGroups.filter((item) =>
      input.ledger.rpcGroups[item] === undefined
    )),
    rpcMethods: sorted(KhalaCodeRpcMethodNames.filter((method: KhalaCodeRpcMethodName) =>
      input.ledger.rpcMethods[method] === undefined
    )),
    selectors: sorted(input.manifest.coverage.selectors.filter((item) =>
      !input.ledger.selectorsClicked.includes(item)
    )),
    slashCommandAvailabilityStates: sorted(
      Object.entries(input.manifest.coverage.slashCommandAvailabilityStates).flatMap(([command, states]) =>
        states
          .filter((state) => !input.ledger.slashCommands[command]?.availabilityStates.includes(state))
          .map((state) => `${command}:${state}`)
      ),
    ),
    settingsKeys: sorted(input.manifest.coverage.settingsKeys.filter((item) =>
      !input.ledger.settingsKeysWritten.includes(item)
    )),
    slashCommands: sorted(input.manifest.coverage.slashCommands.filter((item) =>
      input.ledger.slashCommands[item] === undefined || input.ledger.slashCommands[item]?.dispatches === 0
    )),
    threadItemVariants: sorted(input.manifest.coverage.threadItemVariants.filter((item) =>
      !input.ledger.threadItemVariantsRendered.includes(item)
    )),
  }
  const allMissing = Object.entries(missing).flatMap(([coverageClass, values]) =>
    values.map((value) => `${coverageClass}:${value}`)
  )
  const zeroForAWeek = new Set(input.zeroForAWeek ?? [])
  return {
    generatedAt: input.generatedAt ?? new Date().toISOString(),
    missing,
    schema: "khala_code_qa_coverage_frontier.v1",
    zeroForAWeekIssueCandidates: allMissing.filter((item) => zeroForAWeek.has(item)),
  }
}
