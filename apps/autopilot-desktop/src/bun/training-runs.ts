import type {
  TrainingPublicMetric,
  TrainingRunMetricsRow,
  TrainingRunProjectionRow,
  TrainingRunRealGradientRow,
  TrainingRunState,
  TrainingRunSummaryRow,
  TrainingRunsResponse,
  TrainingWindowProjectionRow,
  TrainingWindowState,
} from "../shared/rpc"

type FetchTrainingRunsInput = Readonly<{
  baseUrl: string
  fetchFn?: typeof fetch
  nowIso?: () => string
}>

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null

const asString = (value: unknown, fallback = ""): string =>
  typeof value === "string" ? value : fallback

const asNumber = (value: unknown, fallback = 0): number =>
  typeof value === "number" && Number.isFinite(value) ? value : fallback

const asBoolean = (value: unknown, fallback = false): boolean =>
  typeof value === "boolean" ? value : fallback

const asNullableString = (value: unknown): string | null =>
  typeof value === "string" ? value : null

const asArray = (value: unknown): readonly unknown[] =>
  Array.isArray(value) ? value : []

const stringArray = (value: unknown): readonly string[] =>
  asArray(value).filter((item): item is string => typeof item === "string")

const runState = (value: unknown): TrainingRunState => {
  switch (value) {
    case "active":
    case "sealed":
    case "reconciled":
      return value
    default:
      return "planned"
  }
}

const windowState = (value: unknown): TrainingWindowState => {
  switch (value) {
    case "active":
    case "sealed":
    case "reconciled":
      return value
    default:
      return "planned"
  }
}

const publicMetric = (value: unknown): TrainingPublicMetric => {
  const record = isRecord(value) ? value : {}
  return {
    provenanceLabel: asString(record.provenanceLabel),
    sourceRefs: stringArray(record.sourceRefs),
    value: asNumber(record.value),
  }
}

const emptyMetric = publicMetric(null)

const metrics = (value: unknown): TrainingRunMetricsRow => {
  const record = isRecord(value) ? value : {}
  return {
    activeWindowCount: publicMetric(record.activeWindowCount),
    assignedContributorCount: publicMetric(record.assignedContributorCount),
    pendingPayoutCount: publicMetric(record.pendingPayoutCount),
    plannedWindowCount: publicMetric(record.plannedWindowCount),
    providerConfirmedSettledPayoutSats: publicMetric(
      record.providerConfirmedSettledPayoutSats,
    ),
    receiptRefCount: publicMetric(record.receiptRefCount),
    reconciledWindowCount: publicMetric(record.reconciledWindowCount),
    rejectedWorkCount: publicMetric(record.rejectedWorkCount),
    sealedWindowCount: publicMetric(record.sealedWindowCount),
    verifiedWorkCount: publicMetric(record.verifiedWorkCount),
  }
}

const runProjection = (value: unknown): TrainingRunProjectionRow | null => {
  if (!isRecord(value)) return null
  const trainingRunRef = asString(value.trainingRunRef)
  if (trainingRunRef === "") return null
  return {
    createdAtDisplay: asString(value.createdAtDisplay),
    maxAllowedStale: asNumber(value.maxAllowedStale, 5),
    promiseRef: asString(value.promiseRef),
    receiptRefs: stringArray(value.receiptRefs),
    sealInFlight: asBoolean(value.sealInFlight),
    sealPublicationCadenceWindows: asNumber(
      value.sealPublicationCadenceWindows,
      1,
    ),
    sourceRefs: stringArray(value.sourceRefs),
    state: runState(value.state),
    trainingRunRef,
    updatedAtDisplay: asString(value.updatedAtDisplay),
  }
}

const windowProjection = (value: unknown): TrainingWindowProjectionRow | null => {
  if (!isRecord(value)) return null
  const windowRef = asString(value.windowRef)
  const trainingRunRef = asString(value.trainingRunRef)
  if (windowRef === "" || trainingRunRef === "") return null
  return {
    datasetRefs: stringArray(value.datasetRefs),
    homeworkKind: asString(value.homeworkKind),
    plannedAtDisplay: asString(value.plannedAtDisplay),
    priority: asNumber(value.priority),
    receiptRefs: stringArray(value.receiptRefs),
    sealMetadata: value.sealMetadata ?? null,
    sourceRefs: stringArray(value.sourceRefs),
    state: windowState(value.state),
    trainingRunRef,
    updatedAtDisplay: asString(value.updatedAtDisplay),
    windowRef,
  }
}

const realGradient = (value: unknown): TrainingRunRealGradientRow => {
  const record = isRecord(value) ? value : {}
  const closeout = isRecord(record.closeoutRequirement)
    ? record.closeoutRequirement
    : {}
  const device = isRecord(record.deviceRequirement)
    ? record.deviceRequirement
    : {}
  const externalAsk = isRecord(record.externalAsk) ? record.externalAsk : {}
  const loss = isRecord(record.lossUnderBudget) ? record.lossUnderBudget : {}
  return {
    closeoutRequirement: {
      evalRef: asNullableString(closeout.evalRef),
      freivaldsCommitmentRefs: stringArray(closeout.freivaldsCommitmentRefs),
      gradientCloseoutRefs: stringArray(closeout.gradientCloseoutRefs),
      mergeRef: asNullableString(closeout.mergeRef),
      provenanceLabel: asString(closeout.provenanceLabel),
      satisfied: asBoolean(closeout.satisfied),
    },
    deviceRequirement: {
      observedDistinctContributorDevices: asNumber(
        device.observedDistinctContributorDevices,
      ),
      provenanceLabel: asString(device.provenanceLabel),
      requiredDistinctContributorDevices: asNumber(
        device.requiredDistinctContributorDevices,
        2,
      ),
      satisfied: asBoolean(device.satisfied),
      sourceRefs: stringArray(device.sourceRefs),
    },
    externalAsk: {
      blockerRefs: stringArray(externalAsk.blockerRefs),
      psionicLaneRef: asString(externalAsk.psionicLaneRef),
      requirementRefs: stringArray(externalAsk.requirementRefs),
      status: asString(externalAsk.status, "blocked_external"),
    },
    lossUnderBudget: {
      budgetLabel: asString(loss.budgetLabel),
      budgetRef: asNullableString(loss.budgetRef),
      finalValidationLoss:
        typeof loss.finalValidationLoss === "number"
          ? loss.finalValidationLoss
          : null,
      maxValidationLoss:
        typeof loss.maxValidationLoss === "number" ? loss.maxValidationLoss : null,
      provenanceLabel: asString(loss.provenanceLabel),
      satisfied: asBoolean(loss.satisfied),
      sourceRefs: stringArray(loss.sourceRefs),
    },
    scopeBoundaryRefs: stringArray(record.scopeBoundaryRefs),
  }
}

const summaryProjection = (value: unknown): TrainingRunSummaryRow | null => {
  if (!isRecord(value)) return null
  const run = runProjection(value.run)
  if (run === null) return null
  const emptyState = isRecord(value.emptyState) ? value.emptyState : {}
  return {
    copyBoundaryRefs: stringArray(value.copyBoundaryRefs),
    emptyState: {
      idle: asBoolean(emptyState.idle),
      reason: asString(emptyState.reason),
    },
    metrics: metrics(value.metrics),
    realGradient: realGradient(value.realGradient),
    receiptRefs: stringArray(value.receiptRefs),
    run,
    sourceRefs: stringArray(value.sourceRefs),
    windows: asArray(value.windows)
      .map(windowProjection)
      .filter((item): item is TrainingWindowProjectionRow => item !== null),
  }
}

const fallbackSummary = (run: TrainingRunProjectionRow): TrainingRunSummaryRow => ({
  copyBoundaryRefs: [],
  emptyState: { idle: true, reason: "No summary projection returned." },
  metrics: {
    activeWindowCount: emptyMetric,
    assignedContributorCount: emptyMetric,
    pendingPayoutCount: emptyMetric,
    plannedWindowCount: emptyMetric,
    providerConfirmedSettledPayoutSats: emptyMetric,
    receiptRefCount: publicMetric({
      sourceRefs: run.receiptRefs,
      value: run.receiptRefs.length,
    }),
    reconciledWindowCount: emptyMetric,
    rejectedWorkCount: emptyMetric,
    sealedWindowCount: emptyMetric,
    verifiedWorkCount: emptyMetric,
  },
  realGradient: realGradient(null),
  receiptRefs: run.receiptRefs,
  run,
  sourceRefs: run.sourceRefs,
  windows: [],
})

export async function fetchTrainingRuns(
  input: FetchTrainingRunsInput,
): Promise<TrainingRunsResponse> {
  const fetchFn = input.fetchFn ?? fetch
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const sourceUrl = `${input.baseUrl.replace(/\/+$/, "")}/api/training/runs`

  try {
    const response = await fetchFn(sourceUrl, {
      headers: { accept: "application/json" },
    })
    if (!response.ok) {
      return {
        ok: false,
        error: `training runs ${response.status}`,
        fetchedAt,
        sourceUrl,
        runs: [],
        summaries: [],
      }
    }

    const json = (await response.json()) as unknown
    const record = isRecord(json) ? json : {}
    const runs = asArray(record.runs)
      .map(runProjection)
      .filter((item): item is TrainingRunProjectionRow => item !== null)
    const decodedSummaries = asArray(record.summaries)
      .map(summaryProjection)
      .filter((item): item is TrainingRunSummaryRow => item !== null)
    const summariesByRun = new Set(
      decodedSummaries.map(summary => summary.run.trainingRunRef),
    )

    return {
      ok: true,
      fetchedAt,
      sourceUrl,
      runs,
      summaries: [
        ...decodedSummaries,
        ...runs
          .filter(run => !summariesByRun.has(run.trainingRunRef))
          .map(fallbackSummary),
      ],
    }
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : String(error),
      fetchedAt,
      sourceUrl,
      runs: [],
      summaries: [],
    }
  }
}
