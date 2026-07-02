import { KhalaCodeRpcMethodNames, type KhalaCodeRpcMethodName } from "./rpc-client.js"
import type { KhalaCodeQaObservation } from "./driver.js"
import type { KhalaCodeQaSeedCorpusManifest } from "./seed-corpus.js"

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
  readonly threadItemVariantRenderCounts: Readonly<Record<string, number>>
  readonly threadItemVariantsRendered: readonly string[]
  readonly selectorsClicked: readonly string[]
  readonly screensScreenshotted: readonly string[]
}

export type KhalaCodeQaCoverageFrontierReport = {
  readonly schema: "khala_code_qa_coverage_frontier.v1"
  readonly generatedAt: string
  readonly missing: {
    readonly rpcMethods: readonly string[]
    readonly slashCommands: readonly string[]
    readonly hotbarPanels: readonly string[]
    readonly settingsKeys: readonly string[]
    readonly approvalDecisionKinds: readonly string[]
    readonly threadItemVariants: readonly string[]
    readonly selectors: readonly string[]
    readonly slashCommandAvailabilityStates: readonly string[]
  }
  readonly zeroForAWeekIssueCandidates: readonly string[]
}

type MutableRpcCoverage = Map<string, { calls: number; argumentShapes: Set<string> }>
type MutableSlashCoverage = Map<string, { dispatches: number; availabilityStates: Set<KhalaCodeQaCoverageAvailability> }>
type MutableCountCoverage = Map<string, number>

const emptyGeneratedAt = "1970-01-01T00:00:00.000Z"

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

export const createEmptyKhalaCodeQaCoverageLedger = (
  options: { readonly generatedAt?: string; readonly runId?: string } = {},
): KhalaCodeQaCoverageLedger => ({
  approvalDecisionKinds: [],
  generatedAt: options.generatedAt ?? emptyGeneratedAt,
  hotbarPanelsOpened: [],
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
  const rpcMethods: MutableRpcCoverage = new Map()
  const slashCommands: MutableSlashCoverage = new Map()
  const hotbarPanelsOpened = new Set<string>()
  const settingsKeysWritten = new Set<string>()
  const approvalDecisionKinds = new Set<string>()
  const threadItemVariantRenderCounts: MutableCountCoverage = new Map()
  const threadItemVariantsRendered = new Set<string>()
  const selectorsClicked = new Set<string>()
  const screensScreenshotted = new Set<string>()

  for (const observation of input.observations) {
    if (!observation.ok) continue
    const action = observation.action
    if (action.kind === "rpc_call") {
      recordRpc(rpcMethods, action.method, action.args ?? [])
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
      for (const variant of collectThreadItemVariants(observation.data)) {
        recordCount(threadItemVariantRenderCounts, variant)
        threadItemVariantsRendered.add(variant)
      }
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
  }

  return {
    approvalDecisionKinds: sorted(approvalDecisionKinds),
    generatedAt: input.generatedAt ?? new Date().toISOString(),
    hotbarPanelsOpened: sorted(hotbarPanelsOpened),
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
  const rpcMethods: MutableRpcCoverage = new Map()
  const slashCommands: MutableSlashCoverage = new Map()
  const threadItemVariantRenderCounts: MutableCountCoverage = new Map()
  const runIds = new Set<string>()
  const generatedAt = ledgers.map((ledger) => ledger.generatedAt).sort().at(-1) ?? emptyGeneratedAt
  const addSet = <K extends keyof KhalaCodeQaCoverageLedger>(key: K): Set<string> =>
    new Set(ledgers.flatMap((ledger) => ledger[key] as readonly string[]))

  for (const ledger of ledgers) {
    for (const runId of ledger.runIds) runIds.add(runId)
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
    generatedAt,
    hotbarPanelsOpened: sorted(addSet("hotbarPanelsOpened")),
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
    hotbarPanels: sorted(input.manifest.coverage.hotbarPanels.filter((item) =>
      !input.ledger.hotbarPanelsOpened.includes(item)
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
