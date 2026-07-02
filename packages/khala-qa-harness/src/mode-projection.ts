import type { KhalaCodeRpcMethodName } from "./rpc-client.js"

export const KHALA_CODE_QA_CROSS_MODE_SURFACES = [
  "thread_list",
  "fleet_counts",
  "gym_state",
  "runtime_badges",
] as const

export type KhalaCodeQaCrossModeSurface =
  typeof KHALA_CODE_QA_CROSS_MODE_SURFACES[number]

export const khalaCodeQaProjectionQuery = (
  surface: KhalaCodeQaCrossModeSurface,
): string => `projection:${surface}`

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)

const numberOrZero = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value) ? value : 0

const stringOrNull = (value: unknown): string | null =>
  typeof value === "string" && value.length > 0 ? value : null

const arrayOrEmpty = (value: unknown): readonly unknown[] =>
  Array.isArray(value) ? value : []

const latest = (
  lookup: (method: KhalaCodeRpcMethodName) => unknown | undefined,
  method: KhalaCodeRpcMethodName,
): unknown | undefined => lookup(method)

const threadListProjection = (value: unknown) => {
  if (!isRecord(value)) return undefined
  const threads = arrayOrEmpty(value.threads ?? value.data)
    .filter(isRecord)
    .map((thread) => ({
      id: stringOrNull(thread.id),
      status: stringOrNull(thread.status),
      title: stringOrNull(thread.title),
    }))
    .sort((left, right) => String(left.id).localeCompare(String(right.id)))
  const groups = arrayOrEmpty(value.groups)
    .filter(isRecord)
    .map((group) => ({
      key: stringOrNull(group.key),
      threadIds: arrayOrEmpty(group.threadIds)
        .map(stringOrNull)
        .filter((threadId): threadId is string => threadId !== null)
        .sort(),
    }))
    .sort((left, right) => String(left.key).localeCompare(String(right.key)))
  return {
    groups,
    threadCount: threads.length,
    threads,
  }
}

const fleetCountsProjection = (value: unknown) => {
  if (!isRecord(value)) return undefined
  const pylon = isRecord(value.pylon) ? value.pylon : {}
  return {
    accountCount: arrayOrEmpty(value.accounts).length,
    activeAssignmentCount: arrayOrEmpty(value.activeAssignments).length,
    availableCodexAssignments: numberOrZero(value.availableCodexAssignments),
    maxCodexAssignments: numberOrZero(value.maxCodexAssignments),
    pylonStatus: stringOrNull(pylon.status),
  }
}

const fleetRunValue = (value: unknown): Record<string, unknown> | undefined => {
  if (!isRecord(value)) return undefined
  return isRecord(value.run) ? value.run : undefined
}

const gymStateProjection = (
  lookup: (method: KhalaCodeRpcMethodName) => unknown | undefined,
) => {
  const statusRun = fleetRunValue(latest(lookup, "fleetRunStatus"))
  const list = latest(lookup, "fleetRunList")
  const listRuns = isRecord(list)
    ? arrayOrEmpty(list.runs).filter(isRecord)
    : []
  const fleetCounts = fleetCountsProjection(latest(lookup, "codexFleetStatus"))
  return {
    activeAssignments: numberOrZero(isRecord(statusRun?.counters) ? statusRun.counters.activeAssignments : undefined),
    fleetAvailableCodexAssignments: isRecord(fleetCounts) ? fleetCounts.availableCodexAssignments : 0,
    listRunCount: listRuns.length,
    runRef: stringOrNull(statusRun?.runRef),
    runState: stringOrNull(statusRun?.state),
    targetConcurrency: numberOrZero(statusRun?.targetConcurrency),
  }
}

const runtimeBadge = (value: unknown) => {
  if (!isRecord(value)) return undefined
  return {
    available: value.available === true,
    capability: stringOrNull(value.capability),
    status: stringOrNull(value.status),
  }
}

const runtimeBadgesProjection = (
  lookup: (method: KhalaCodeRpcMethodName) => unknown | undefined,
) => ({
  codexHarness: runtimeBadge(latest(lookup, "codexHarnessStatus")),
  coding: runtimeBadge(latest(lookup, "codingStatus")),
  pylon: runtimeBadge(latest(lookup, "pylonStatus")),
  tokenAccounting: runtimeBadge(latest(lookup, "tokenAccountingStatus")),
})

export const projectKhalaCodeQaModeState = (
  query: string,
  lookup: (method: KhalaCodeRpcMethodName) => unknown | undefined,
): unknown | undefined => {
  switch (query) {
    case khalaCodeQaProjectionQuery("thread_list"):
      return threadListProjection(latest(lookup, "codexThreadList"))
    case khalaCodeQaProjectionQuery("fleet_counts"):
      return fleetCountsProjection(latest(lookup, "codexFleetStatus"))
    case khalaCodeQaProjectionQuery("gym_state"):
      return gymStateProjection(lookup)
    case khalaCodeQaProjectionQuery("runtime_badges"):
      return runtimeBadgesProjection(lookup)
    default:
      return undefined
  }
}
