import type {
  TrainingPublicMetric,
  TrainingPlanResponse,
  TrainingRunMetricsRow,
  TrainingRunProjectionRow,
  TrainingRunRealGradientRow,
  TrainingRunState,
  TrainingRunSummaryRow,
  TrainingRunsResponse,
  TrainingWindowActionResponse,
  TrainingWindowProjectionRow,
  TrainingWindowState,
} from "../shared/rpc"

type FetchTrainingRunsInput = Readonly<{
  baseUrl: string
  fetchFn?: typeof fetch
  nowIso?: () => string
}>

type PlanTrainingRunWindowInput = Readonly<{
  adminToken: string | null
  baseUrl: string
  enabled: boolean
  fetchFn?: typeof fetch
  nowIso?: () => string
}>

type ActivateTrainingWindowInput = Readonly<{
  adminToken: string | null
  baseUrl: string
  enabled: boolean
  fetchFn?: typeof fetch
  nowIso?: () => string
  windowRef: string
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

const publicSafeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/

const normalizeBaseUrl = (baseUrl: string): string => baseUrl.replace(/\/+$/, "")

const safeRefStamp = (value: string): string => {
  const stamp = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, ".")
    .replace(/^\.+|\.+$/g, "")
    .slice(0, 80)
  return stamp === "" ? "manual" : stamp
}

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

const errorMessageFromJson = (json: unknown, fallback: string): string => {
  const record = isRecord(json) ? json : {}
  return asString(record.reason, asString(record.error, fallback))
}

const postJson = async (
  fetchFn: typeof fetch,
  url: string,
  token: string,
  body: unknown,
): Promise<
  | { readonly ok: true; readonly json: unknown }
  | { readonly ok: false; readonly error: string }
> => {
  const response = await fetchFn(url, {
    method: "POST",
    headers: {
      accept: "application/json",
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  })

  const json = (await response.json().catch(() => null)) as unknown
  if (!response.ok) {
    return {
      ok: false,
      error: errorMessageFromJson(json, `training admin ${response.status}`),
    }
  }

  return { ok: true, json }
}

const disabledPlanResponse = (input: {
  enabled: boolean
  fetchedAt: string
  message: string
  reason: TrainingPlanResponse["reason"]
  sourceUrl: string
}): TrainingPlanResponse => ({
  ok: false,
  enabled: input.enabled,
  fetchedAt: input.fetchedAt,
  sourceUrl: input.sourceUrl,
  trainingRunRef: null,
  windowRef: null,
  run: null,
  window: null,
  runPlanned: false,
  windowPlanned: false,
  reason: input.reason,
  message: input.message,
})

const disabledWindowActionResponse = (input: {
  enabled: boolean
  fetchedAt: string
  message: string
  reason: TrainingWindowActionResponse["reason"]
  sourceUrl: string
  windowRef: string | null
}): TrainingWindowActionResponse => ({
  ok: false,
  enabled: input.enabled,
  fetchedAt: input.fetchedAt,
  sourceUrl: input.sourceUrl,
  windowRef: input.windowRef,
  window: null,
  reason: input.reason,
  message: input.message,
})

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
  const sourceUrl = `${normalizeBaseUrl(input.baseUrl)}/api/training/runs`

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

export async function planTrainingRunWindow(
  input: PlanTrainingRunWindowInput,
): Promise<TrainingPlanResponse> {
  const fetchFn = input.fetchFn ?? fetch
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const baseUrl = normalizeBaseUrl(input.baseUrl)
  const runUrl = `${baseUrl}/api/training/runs`
  const windowUrl = `${baseUrl}/api/training/windows/plan`

  if (!input.enabled) {
    return disabledPlanResponse({
      enabled: false,
      fetchedAt,
      message: "training admin planning disabled",
      reason: "disabled",
      sourceUrl: runUrl,
    })
  }

  const token = input.adminToken?.trim() ?? ""
  if (token === "") {
    return disabledPlanResponse({
      enabled: true,
      fetchedAt,
      message: "training admin token unavailable",
      reason: "admin_token_missing",
      sourceUrl: runUrl,
    })
  }

  const stamp = safeRefStamp(fetchedAt)
  const trainingRunRef = `training.run.desktop.r1.${stamp}`
  const windowRef = `training.window.desktop.r1.${stamp}`
  const sourceRefs = [
    "issue.github.openagents.4855",
    "docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md",
    "docs/training/2026-06-14-autopilot-desktop-training-ui-audit.md",
    "docs/tassadar/RESEARCH_PLAN.md",
  ] as const

  try {
    const runResult = await postJson(fetchFn, runUrl, token, {
      maxAllowedStale: 5,
      promiseRef: "pylon.first_real_model_training_run.v1",
      receiptRefs: [`receipt.desktop.training.run.planned.${stamp}`],
      sealPublicationCadenceWindows: 1,
      sourceRefs,
      trainingRunRef,
    })

    if (!runResult.ok) {
      return {
        ok: false,
        enabled: true,
        fetchedAt,
        sourceUrl: runUrl,
        trainingRunRef,
        windowRef: null,
        run: null,
        window: null,
        runPlanned: false,
        windowPlanned: false,
        reason: "run_plan_failed",
        message: `run plan failed: ${runResult.error}`,
        error: runResult.error,
      }
    }

    const runRecord = isRecord(runResult.json) ? runResult.json : {}
    const run = runProjection(runRecord.run)
    const plannedRunRef = run?.trainingRunRef ?? trainingRunRef

    const windowResult = await postJson(fetchFn, windowUrl, token, {
      datasetRefs: ["dataset.cs336.a1.public"],
      homeworkKind: "admin_dispatched_homework",
      priority: 100,
      receiptRefs: [`receipt.desktop.training.window.planned.${stamp}`],
      sourceRefs,
      trainingRunRef: plannedRunRef,
      windowRef,
    })

    if (!windowResult.ok) {
      return {
        ok: false,
        enabled: true,
        fetchedAt,
        sourceUrl: windowUrl,
        trainingRunRef: plannedRunRef,
        windowRef,
        run,
        window: null,
        runPlanned: true,
        windowPlanned: false,
        reason: "window_plan_failed",
        message: `run planned; window plan failed: ${windowResult.error}`,
        error: windowResult.error,
      }
    }

    const windowRecord = isRecord(windowResult.json) ? windowResult.json : {}
    const window = windowProjection(windowRecord.window)
    const plannedWindowRef = window?.windowRef ?? windowRef

    return {
      ok: true,
      enabled: true,
      fetchedAt,
      sourceUrl: windowUrl,
      trainingRunRef: plannedRunRef,
      windowRef: plannedWindowRef,
      run,
      window,
      runPlanned: true,
      windowPlanned: true,
      reason: "planned",
      message: `planned ${plannedRunRef} / ${plannedWindowRef}`,
    }
  } catch (error) {
    const text = error instanceof Error ? error.message : String(error)
    return {
      ok: false,
      enabled: true,
      fetchedAt,
      sourceUrl: runUrl,
      trainingRunRef: null,
      windowRef: null,
      run: null,
      window: null,
      runPlanned: false,
      windowPlanned: false,
      reason: "request_failed",
      message: `training admin request failed: ${text}`,
      error: text,
    }
  }
}

export async function activateTrainingWindow(
  input: ActivateTrainingWindowInput,
): Promise<TrainingWindowActionResponse> {
  const fetchFn = input.fetchFn ?? fetch
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const baseUrl = normalizeBaseUrl(input.baseUrl)
  const trimmedWindowRef = input.windowRef.trim()
  const sourceUrl =
    trimmedWindowRef === ""
      ? `${baseUrl}/api/training/windows/activate`
      : `${baseUrl}/api/training/windows/${encodeURIComponent(trimmedWindowRef)}/activate`

  if (!input.enabled) {
    return disabledWindowActionResponse({
      enabled: false,
      fetchedAt,
      message: "training admin activation disabled",
      reason: "disabled",
      sourceUrl,
      windowRef: trimmedWindowRef === "" ? null : trimmedWindowRef,
    })
  }

  const token = input.adminToken?.trim() ?? ""
  if (token === "") {
    return disabledWindowActionResponse({
      enabled: true,
      fetchedAt,
      message: "training admin token unavailable",
      reason: "admin_token_missing",
      sourceUrl,
      windowRef: trimmedWindowRef === "" ? null : trimmedWindowRef,
    })
  }

  if (
    trimmedWindowRef.length < 3 ||
    trimmedWindowRef.length > 260 ||
    !publicSafeRefPattern.test(trimmedWindowRef)
  ) {
    return disabledWindowActionResponse({
      enabled: true,
      fetchedAt,
      message: "invalid training window ref",
      reason: "invalid_window_ref",
      sourceUrl,
      windowRef: trimmedWindowRef === "" ? null : trimmedWindowRef,
    })
  }

  const stamp = safeRefStamp(fetchedAt)

  try {
    const result = await postJson(fetchFn, sourceUrl, token, {
      actorRef: "operator.openagents.autopilot_desktop",
      receiptRef: `receipt.desktop.training.window.activate.${stamp}`,
    })

    if (!result.ok) {
      return {
        ok: false,
        enabled: true,
        fetchedAt,
        sourceUrl,
        windowRef: trimmedWindowRef,
        window: null,
        reason: "transition_failed",
        message: `window activation failed: ${result.error}`,
        error: result.error,
      }
    }

    const record = isRecord(result.json) ? result.json : {}
    const window = windowProjection(record.window)
    const activatedWindowRef = window?.windowRef ?? trimmedWindowRef
    return {
      ok: true,
      enabled: true,
      fetchedAt,
      sourceUrl,
      windowRef: activatedWindowRef,
      window,
      reason: "activated",
      message: `activated ${activatedWindowRef}`,
    }
  } catch (error) {
    const text = error instanceof Error ? error.message : String(error)
    return {
      ok: false,
      enabled: true,
      fetchedAt,
      sourceUrl,
      windowRef: trimmedWindowRef,
      window: null,
      reason: "request_failed",
      message: `training admin activation failed: ${text}`,
      error: text,
    }
  }
}
