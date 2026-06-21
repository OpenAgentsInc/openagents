import { readFileSync, writeFileSync } from "node:fs"

import type {
  TrainingDashboardSummaryResponse,
  TrainingEvidencePacketBuildResponse,
  TrainingEvidencePacketSummaryResponse,
  TrainingLeaderboardLane,
  TrainingLeaderboardLaneSummary,
  TrainingLeaderboardTopRow,
  TrainingBootstrapGrantResponse,
  TrainingBootstrapGrantRow,
  TrainingBootstrapOutcome,
  TrainingEvidenceAdmissionResponse,
  TrainingPromiseGatesResponse,
  TrainingPromiseState,
  TrainingPromiseSummary,
  TrainingPublicMetric,
  TrainingPlanResponse,
  PublicTassadarRunSummary,
  TrainingRunMetricsRow,
  TrainingRunProjectionRow,
  TrainingRunRealGradientRow,
  TrainingRunState,
  TrainingRunSummaryRow,
  TrainingRunsResponse,
  TrainingWindowActionResponse,
  TrainingWindowLeaseResponse,
  TrainingWindowLeaseRow,
  TrainingWindowProjectionRow,
  TrainingWindowState,
} from "../shared/rpc.js"

type FetchTrainingRunsInput = Readonly<{
  baseUrl: string
  fetchFn?: typeof fetch
  nowIso?: () => string
}>

type FetchTrainingDashboardInput = Readonly<{
  baseUrl: string
  fetchFn?: typeof fetch
  nowIso?: () => string
}>

type FetchTrainingPromiseGatesInput = Readonly<{
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

type ReconcileTrainingWindowInput = Readonly<{
  adminToken: string | null
  baseUrl: string
  enabled: boolean
  fetchFn?: typeof fetch
  nowIso?: () => string
  windowRef: string
}>

type ClaimTrainingWindowLeaseInput = Readonly<{
  baseUrl: string
  enabled: boolean
  fetchFn?: typeof fetch
  leaseSeconds?: number
  nowIso?: () => string
  pylonRef: string | null
}>

type RequestTrainingBootstrapGrantInput = Readonly<{
  baseUrl: string
  fetchFn?: typeof fetch
  nowIso?: () => string
  pylonRef: string | null
  trainingRunRef: string
}>

type AdmitTrainingRealGradientEvidenceInput = Readonly<{
  adminToken: string | null
  baseUrl: string
  enabled: boolean
  evidencePacketPath: string | null
  fetchFn?: typeof fetch
  nowIso?: () => string
  readPacket?: (path: string) => unknown
  trainingRunRef: string
}>

type ReadTrainingEvidencePacketSummaryInput = Readonly<{
  evidencePacketPath: string | null
  nowIso?: () => string
  readPacket?: (path: string) => unknown
}>

type BuildTrainingEvidencePacketInput = Readonly<{
  enabled: boolean
  evidencePacketPath: string | null
  nowIso?: () => string
  readBundle?: (path: string) => unknown
  trainingRunRef: string
  workerReceiptsPath: string | null
  writePacket?: (path: string, packet: unknown) => void
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
const publicSafePylonRefPattern = /^[a-z0-9][a-z0-9_.:-]*$/
const trainingLeaderboardLanes = new Set<TrainingLeaderboardLane>([
  "a1_loss",
  "a2_throughput",
  "a3_isoflop",
  "a4_eval_delta",
  "a5_accuracy",
])
const trainingPromiseStates = new Set<TrainingPromiseState>([
  "degraded",
  "green",
  "planned",
  "red",
  "withdrawn",
  "yellow",
])
const trainingPromiseIds = new Set([
  "pylon.first_real_model_training_run.v1",
  "training.public_distributed_training_run.v1",
  "training.monday_decentralized_training_launch.v1",
  "pylon.largest_decentralized_training_claim.v1",
  "models.tassadar_percepta_executor.v1",
  "training.model_ladder.v1",
  "training.marathon_operations.v1",
  "training.verification_classes.v1",
])

const evidencePacketSource = "env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH"
const workerReceiptsSource = "local.training_worker_receipts"
const evidencePacketWriteSource =
  "env.OPENAGENTS_DESKTOP_TRAINING_EVIDENCE_WRITE_ENABLE"

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

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${safeRefStamp(value)}`

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

const postPublicJson = async (
  fetchFn: typeof fetch,
  url: string,
  body: unknown,
): Promise<
  | { readonly ok: true; readonly json: unknown }
  | { readonly ok: false; readonly error: string }
> => {
  const response = await fetchFn(url, {
    method: "POST",
    headers: {
      accept: "application/json",
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  })

  const json = (await response.json().catch(() => null)) as unknown
  if (!response.ok) {
    return {
      ok: false,
      error: errorMessageFromJson(json, `training lease ${response.status}`),
    }
  }

  return { ok: true, json }
}

const getPublicJson = async (
  fetchFn: typeof fetch,
  url: string,
): Promise<
  | { readonly ok: true; readonly json: unknown }
  | { readonly ok: false; readonly error: string }
> => {
  const response = await fetchFn(url, {
    headers: { accept: "application/json" },
  })

  const json = (await response.json().catch(() => null)) as unknown
  if (!response.ok) {
    return {
      ok: false,
      error: errorMessageFromJson(json, `training dashboard ${response.status}`),
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

const disabledLeaseResponse = (input: {
  enabled: boolean
  fetchedAt: string
  message: string
  pylonRef: string | null
  reason: TrainingWindowLeaseResponse["reason"]
  sourceUrl: string
}): TrainingWindowLeaseResponse => ({
  ok: false,
  enabled: input.enabled,
  fetchedAt: input.fetchedAt,
  sourceUrl: input.sourceUrl,
  pylonRef: input.pylonRef,
  lease: null,
  reason: input.reason,
  message: input.message,
})

const disabledBootstrapResponse = (input: {
  fetchedAt: string
  message: string
  pylonRef: string | null
  reason: TrainingBootstrapGrantResponse["reason"]
  sourceUrl: string
  trainingRunRef: string | null
}): TrainingBootstrapGrantResponse => ({
  ok: false,
  fetchedAt: input.fetchedAt,
  sourceUrl: input.sourceUrl,
  pylonRef: input.pylonRef,
  trainingRunRef: input.trainingRunRef,
  outcome: null,
  reason: input.reason,
  message: input.message,
})

const disabledEvidenceAdmissionResponse = (input: {
  enabled: boolean
  fetchedAt: string
  message: string
  packetSource: string | null
  reason: TrainingEvidenceAdmissionResponse["reason"]
  sourceUrl: string
  trainingRunRef: string | null
  error?: string
}): TrainingEvidenceAdmissionResponse => ({
  ok: false,
  enabled: input.enabled,
  fetchedAt: input.fetchedAt,
  sourceUrl: input.sourceUrl,
  trainingRunRef: input.trainingRunRef,
  packetSource: input.packetSource,
  run: null,
  realGradient: null,
  reason: input.reason,
  message: input.message,
  evidenceRefCount: 0,
  receiptRefCount: 0,
  shardContributionCount: 0,
  distinctPylonCount: 0,
  ...(input.error === undefined ? {} : { error: input.error }),
})

const emptyTrainingDashboardResponse = (input: {
  error?: string
  fetchedAt: string
  ok: boolean
  sourceUrl: string
}): TrainingDashboardSummaryResponse => ({
  ok: input.ok,
  fetchedAt: input.fetchedAt,
  sourceUrl: input.sourceUrl,
  leaderboards: { blockerRefs: [], lanes: [] },
  a2: {
    blockerRefs: [],
    observedDeviceClassCount: 0,
    observedMeasurementCount: 0,
    verifiedMeasurementCount: 0,
  },
  a3: {
    blockerRefs: [],
    cellCount: 0,
    fitArtifactCount: 0,
    verifiedCellCount: 0,
  },
  a4: {
    blockerRefs: [],
    evalDeltaBonusBlockerRefs: [],
    observedVerifiedStages: [],
    requiredVerifiedStageCount: 0,
    shardCount: 0,
  },
  a5: {
    blockerRefs: [],
    evalSuiteCount: 0,
    updateBoundaryRef: null,
    verifiedSuiteCount: 0,
  },
  ...(input.error === undefined ? {} : { error: input.error }),
})

const emptyTrainingPromiseGatesResponse = (input: {
  error?: string
  fetchedAt: string
  ok: boolean
  sourceUrl: string
}): TrainingPromiseGatesResponse => ({
  ok: input.ok,
  fetchedAt: input.fetchedAt,
  registryVersion: "",
  sourceUrl: input.sourceUrl,
  blockerRefs: [],
  promises: [],
  stateCounts: {
    degraded: 0,
    green: 0,
    planned: 0,
    red: 0,
    withdrawn: 0,
    yellow: 0,
    unknown: 0,
  },
  ...(input.error === undefined ? {} : { error: input.error }),
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

const leaseProjection = (
  value: unknown,
  nowIso: string,
): TrainingWindowLeaseRow | null => {
  if (!isRecord(value)) return null
  const leaseRef = asString(value.leaseRef)
  const pylonRef = asString(value.pylonRef)
  const trainingRunRef = asString(value.trainingRunRef)
  const windowRef = asString(value.windowRef)
  if (
    leaseRef === "" ||
    pylonRef === "" ||
    trainingRunRef === "" ||
    windowRef === ""
  ) {
    return null
  }
  const expiresIn =
    typeof value.leaseExpiresInSeconds === "number"
      ? value.leaseExpiresInSeconds
      : typeof value.leaseExpiresAt === "string"
        ? Math.max(
            0,
            Math.floor(
              (Date.parse(value.leaseExpiresAt) - Date.parse(nowIso)) / 1000,
            ),
          )
        : 0
  return {
    claimedAtDisplay: asString(
      value.claimedAtDisplay,
      asString(value.claimedAt),
    ),
    leaseExpiresInSeconds: Number.isFinite(expiresIn)
      ? Math.max(0, Math.floor(expiresIn))
      : 0,
    leaseRef,
    pylonRef,
    receiptRefs: stringArray(value.receiptRefs),
    state: value.state === "released" ? "released" : "active",
    trainingRunRef,
    windowRef,
  }
}

const bootstrapGrant = (value: unknown): TrainingBootstrapGrantRow | null => {
  if (!isRecord(value)) return null
  const checkpointDigestRef = asString(value.checkpointDigestRef)
  const grantRef = asString(value.grantRef)
  const joinerRef = asString(value.joinerRef)
  const sealedWindowRef = asString(value.sealedWindowRef)
  const trainingRunRef = asString(value.trainingRunRef)
  if (
    checkpointDigestRef === "" ||
    grantRef === "" ||
    joinerRef === "" ||
    sealedWindowRef === "" ||
    trainingRunRef === ""
  ) {
    return null
  }

  return {
    checkpointDigestRef,
    grantRef,
    joinerReceiptRefs: stringArray(value.joinerReceiptRefs),
    joinerRef,
    sealReceiptRefs: stringArray(value.sealReceiptRefs),
    sealedAtDisplay: asString(value.sealedAtDisplay),
    sealedWindowRef,
    trainingRunRef,
  }
}

const bootstrapOutcome = (value: unknown): TrainingBootstrapOutcome | null => {
  if (!isRecord(value)) return null
  const kind = value.kind
  if (kind === "granted") {
    const grant = bootstrapGrant(value.grant)
    return grant === null ? null : { kind, grant }
  }

  if (kind === "queued") {
    const joinerRef = asString(value.joinerRef)
    const trainingRunRef = asString(value.trainingRunRef)
    if (joinerRef === "" || trainingRunRef === "") return null
    return {
      joinerRef,
      kind,
      reasonCode: asString(value.reasonCode),
      trainingRunRef,
    }
  }

  if (kind === "refused") {
    const joinerRef = asString(value.joinerRef)
    const trainingRunRef = asString(value.trainingRunRef)
    if (joinerRef === "" || trainingRunRef === "") return null
    return {
      joinerRef,
      kind,
      reason: asString(value.reason),
      reasonCode: asString(value.reasonCode),
      trainingRunRef,
    }
  }

  return null
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

const evidencePacketStats = (value: unknown): {
  readonly distinctPylonCount: number
  readonly evidenceRefCount: number
  readonly receiptRefCount: number
  readonly shardContributionCount: number
} => {
  const packet = isRecord(value) ? value : {}
  const shards = asArray(packet.shardContributions).filter(isRecord)
  const shardPylons = shards
    .map(shard => asString(shard.pylonRef))
    .filter(ref => ref !== "")
  const receiptRefs = [
    ...stringArray(packet.receiptRefs),
    ...shards.flatMap(shard => stringArray(shard.receiptRefs)),
  ]
  const evidenceRefs = [
    ...stringArray(packet.freivaldsCommitmentRefs),
    ...stringArray(packet.gradientCloseoutRefs),
    ...stringArray(packet.lossSourceRefs),
    ...stringArray(packet.sourceRefs),
    asString(packet.budgetRef),
    asString(packet.evalRef),
    asString(packet.mergeRef),
    ...shards.flatMap(shard => [
      asString(shard.deviceClassRef),
      asString(shard.gradientCommitmentRef),
      ...stringArray(shard.sourceRefs),
      ...stringArray(shard.verificationRefs),
    ]),
  ].filter(ref => ref !== "")

  return {
    distinctPylonCount: new Set(shardPylons).size,
    evidenceRefCount: new Set(evidenceRefs).size,
    receiptRefCount: new Set(receiptRefs).size,
    shardContributionCount: shards.length,
  }
}

const emptyEvidencePacketSummary = (input: {
  blockerRefs: readonly string[]
  configured: boolean
  error?: string
  fetchedAt: string
  packetSource: string | null
}): TrainingEvidencePacketSummaryResponse => ({
  ok: false,
  configured: input.configured,
  fetchedAt: input.fetchedAt,
  sourceUrl: "desktop:training-evidence-packet",
  packetSource: input.packetSource,
  budgetLabel: null,
  budgetRefPresent: false,
  evalRefPresent: false,
  mergeRefPresent: false,
  finalValidationLoss: null,
  maxValidationLoss: null,
  lossPointCount: 0,
  freivaldsCommitmentRefCount: 0,
  gradientCloseoutRefCount: 0,
  evidenceRefCount: 0,
  receiptRefCount: 0,
  shardContributionCount: 0,
  distinctPylonCount: 0,
  blockerRefs: input.blockerRefs,
  ...(input.error === undefined ? {} : { error: input.error }),
})

const uniqueStringCount = (value: unknown): number =>
  new Set(stringArray(value).filter(ref => ref.trim() !== "")).size

const packetFinalValidationLoss = (
  lossCurve: readonly Record<string, unknown>[],
): number | null => {
  const losses = lossCurve
    .map(point => point.validationLoss)
    .filter(
      (value): value is number =>
        typeof value === "number" && Number.isFinite(value),
    )
  return losses.length === 0 ? null : losses[losses.length - 1] ?? null
}

export function readTrainingEvidencePacketSummary(
  input: ReadTrainingEvidencePacketSummaryInput,
): TrainingEvidencePacketSummaryResponse {
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const packetPath = input.evidencePacketPath?.trim() ?? ""

  if (packetPath === "") {
    return emptyEvidencePacketSummary({
      blockerRefs: ["env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH"],
      configured: false,
      fetchedAt,
      packetSource: null,
    })
  }

  let packet: unknown
  try {
    packet =
      input.readPacket?.(packetPath) ??
      (JSON.parse(readFileSync(packetPath, "utf8")) as unknown)
  } catch {
    return emptyEvidencePacketSummary({
      blockerRefs: ["training.evidence_packet.read_failed"],
      configured: true,
      error: "training evidence packet read failed",
      fetchedAt,
      packetSource: evidencePacketSource,
    })
  }

  if (!isRecord(packet)) {
    return emptyEvidencePacketSummary({
      blockerRefs: ["training.evidence_packet.invalid_json_object"],
      configured: true,
      error: "training evidence packet must be a JSON object",
      fetchedAt,
      packetSource: evidencePacketSource,
    })
  }

  const stats = evidencePacketStats(packet)
  const lossCurve = asArray(packet.lossCurve).filter(isRecord)
  const finalValidationLoss = packetFinalValidationLoss(lossCurve)
  const maxValidationLoss =
    typeof packet.maxValidationLoss === "number" &&
    Number.isFinite(packet.maxValidationLoss)
      ? packet.maxValidationLoss
      : null
  const budgetLabel = asString(packet.budgetLabel).trim()
  const budgetRefPresent = asString(packet.budgetRef).trim() !== ""
  const evalRefPresent = asString(packet.evalRef).trim() !== ""
  const mergeRefPresent = asString(packet.mergeRef).trim() !== ""
  const freivaldsCommitmentRefCount = uniqueStringCount(
    packet.freivaldsCommitmentRefs,
  )
  const gradientCloseoutRefCount = uniqueStringCount(
    packet.gradientCloseoutRefs,
  )
  const blockerRefs: string[] = []

  if (!budgetRefPresent) {
    blockerRefs.push("training.evidence_packet.budget_ref_missing")
  }
  if (!evalRefPresent) {
    blockerRefs.push("training.evidence_packet.eval_ref_missing")
  }
  if (!mergeRefPresent) {
    blockerRefs.push("training.evidence_packet.merge_ref_missing")
  }
  if (lossCurve.length < 2) {
    blockerRefs.push("training.evidence_packet.loss_curve_missing")
  }
  if (finalValidationLoss === null) {
    blockerRefs.push("training.evidence_packet.final_validation_loss_missing")
  }
  if (maxValidationLoss === null) {
    blockerRefs.push("training.evidence_packet.max_validation_loss_missing")
  }
  if (
    finalValidationLoss !== null &&
    maxValidationLoss !== null &&
    finalValidationLoss > maxValidationLoss
  ) {
    blockerRefs.push("training.evidence_packet.loss_exceeds_budget")
  }
  if (freivaldsCommitmentRefCount === 0) {
    blockerRefs.push("training.evidence_packet.freivalds_commitment_missing")
  }
  if (gradientCloseoutRefCount === 0) {
    blockerRefs.push("training.evidence_packet.gradient_closeout_missing")
  }
  if (stats.receiptRefCount === 0) {
    blockerRefs.push("training.evidence_packet.receipt_refs_missing")
  }
  if (stats.shardContributionCount === 0) {
    blockerRefs.push("training.evidence_packet.shard_contributions_missing")
  }
  if (stats.distinctPylonCount < 2) {
    blockerRefs.push("training.evidence_packet.requires_two_distinct_pylons")
  }

  return {
    ok: blockerRefs.length === 0,
    configured: true,
    fetchedAt,
    sourceUrl: "desktop:training-evidence-packet",
    packetSource: evidencePacketSource,
    budgetLabel: budgetLabel === "" ? null : budgetLabel,
    budgetRefPresent,
    evalRefPresent,
    mergeRefPresent,
    finalValidationLoss,
    maxValidationLoss,
    lossPointCount: lossCurve.length,
    freivaldsCommitmentRefCount,
    gradientCloseoutRefCount,
    evidenceRefCount: stats.evidenceRefCount,
    receiptRefCount: stats.receiptRefCount,
    shardContributionCount: stats.shardContributionCount,
    distinctPylonCount: stats.distinctPylonCount,
    blockerRefs,
  }
}

const unsafeGeneratedPacketRefPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|checkpoint[-_]?path|invoice|lnbc|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|prompt|secret|token|wallet|weights\.(bin|gguf|safetensors|pt|pth))/i

const buildEvidencePacketResponse = (input: {
  blockerRefs?: readonly string[]
  enabled: boolean
  error?: string
  fetchedAt: string
  inputSource: string | null
  message: string
  packetSource: string | null
  reason: TrainingEvidencePacketBuildResponse["reason"]
  summary?: TrainingEvidencePacketSummaryResponse | null
  trainingRunRef: string | null
}): TrainingEvidencePacketBuildResponse => ({
  ok: input.reason === "written",
  enabled: input.enabled,
  fetchedAt: input.fetchedAt,
  sourceUrl: "desktop:training-evidence-packet-build",
  trainingRunRef: input.trainingRunRef,
  inputSource: input.inputSource,
  packetSource: input.packetSource,
  reason: input.reason,
  message: input.message,
  summary: input.summary ?? null,
  blockerRefs: input.blockerRefs ?? input.summary?.blockerRefs ?? [],
  ...(input.error === undefined ? {} : { error: input.error }),
})

const safePacketRef = (value: unknown): string | null => {
  const ref = asString(value).trim()
  return ref !== "" &&
    publicSafeRefPattern.test(ref) &&
    !unsafeGeneratedPacketRefPattern.test(ref)
    ? ref
    : null
}

const safePacketRefs = (value: unknown): readonly string[] =>
  [
    ...new Set(
      stringArray(value)
        .map(safePacketRef)
        .filter((ref): ref is string => ref !== null),
    ),
  ]

const asNumericLossCurve = (
  value: unknown,
): readonly { readonly step: number; readonly validationLoss: number }[] =>
  asArray(value)
    .filter(isRecord)
    .flatMap(point => {
      const step = point.step
      const validationLoss = point.validationLoss
      return typeof step === "number" &&
        Number.isFinite(step) &&
        typeof validationLoss === "number" &&
        Number.isFinite(validationLoss)
        ? [{ step, validationLoss }]
        : []
    })

const workerReceiptRecords = (
  value: unknown,
): readonly Record<string, unknown>[] => {
  const record = isRecord(value) ? value : {}
  const rawReceipts = Array.isArray(record.workerReceipts)
    ? record.workerReceipts
    : Array.isArray(record.receipts)
      ? record.receipts
      : Array.isArray(value)
        ? value
        : isRecord(value)
          ? [value]
          : []
  return rawReceipts.filter(isRecord)
}

const signatureRecord = (value: unknown): Record<string, unknown> =>
  isRecord(value) ? value : {}

const generatedPacketFromWorkerReceipts = (input: {
  bundle: unknown
  fetchedAt: string
  trainingRunRef: string
}): {
  readonly blockerRefs: readonly string[]
  readonly packet: Record<string, unknown> | null
} => {
  const bundle = isRecord(input.bundle) ? input.bundle : {}
  const receipts = workerReceiptRecords(input.bundle)
  const blockerRefs: string[] = []

  if (receipts.length === 0) {
    blockerRefs.push("training.worker_receipts.empty")
  }

  const shards = receipts.flatMap((receipt, index) => {
    const receiptRef = safePacketRef(receipt.receiptRef)
    const workerRef = safePacketRef(receipt.workerRef)
    const assignmentRef = safePacketRef(receipt.assignmentRef)
    const runRef = safePacketRef(receipt.runRef)
    const artifactRefs = safePacketRefs(receipt.artifactRefs)
    const checkpointRefs = safePacketRefs(receipt.checkpointRefs)
    const metricRefs = safePacketRefs(receipt.metricRefs)
    const proofRefs = safePacketRefs(receipt.proofRefs)
    const signature = signatureRecord(receipt.signature)
    const verificationRef = safePacketRef(signature.verificationRef)

    if (receiptRef === null) {
      blockerRefs.push("training.worker_receipts.receipt_ref_missing")
    }
    if (workerRef === null) {
      blockerRefs.push("training.worker_receipts.worker_ref_missing")
    }
    if (receiptRef === null || workerRef === null) return []

    const sourceRefs = [
      assignmentRef,
      runRef,
      ...artifactRefs,
      ...checkpointRefs,
      ...metricRefs,
      ...proofRefs,
      safePacketRef(signature.signatureRef),
      safePacketRef(signature.signerRef),
      verificationRef,
    ].filter((ref): ref is string => ref !== null)

    return [
      {
        dataUnitCount: asNumber(receipt.dataUnitCount, 0),
        gradientCommitmentRef:
          proofRefs[0] ??
          checkpointRefs[0] ??
          stableRef("gradient.worker_receipt", receiptRef),
        pylonRef: workerRef,
        receiptRefs: [receiptRef],
        shardIndex: index,
        shardLoss:
          typeof receipt.shardLoss === "number" && Number.isFinite(receipt.shardLoss)
            ? receipt.shardLoss
            : null,
        sourceRefs,
        stepIndex: asNumber(receipt.stepIndex, index),
        verificationRefs: verificationRef === null ? [] : [verificationRef],
      },
    ]
  })

  const allProofRefs = receipts.flatMap(receipt => safePacketRefs(receipt.proofRefs))
  const allCheckpointRefs = receipts.flatMap(receipt =>
    safePacketRefs(receipt.checkpointRefs),
  )
  const receiptRefs = shards.flatMap(shard => shard.receiptRefs)
  const sourceRefs = [
    ...safePacketRefs(bundle.sourceRefs),
    ...receipts.flatMap(receipt => [
      safePacketRef(receipt.assignmentRef),
      safePacketRef(receipt.runRef),
      ...safePacketRefs(receipt.artifactRefs),
      ...safePacketRefs(receipt.metricRefs),
    ]),
  ].filter((ref): ref is string => ref !== null)
  const maxValidationLoss =
    typeof bundle.maxValidationLoss === "number" &&
    Number.isFinite(bundle.maxValidationLoss)
      ? bundle.maxValidationLoss
      : undefined
  const lossCurve = asNumericLossCurve(bundle.lossCurve)

  if (blockerRefs.length > 0) {
    return { blockerRefs: [...new Set(blockerRefs)], packet: null }
  }

  return {
    blockerRefs: [],
    packet: {
      schema: "openagents.training.real_gradient_evidence_packet.v0.1",
      budgetLabel: asString(bundle.budgetLabel, "worker receipt packet"),
      budgetRef: safePacketRef(bundle.budgetRef),
      evalRef: safePacketRef(bundle.evalRef),
      freivaldsCommitmentRefs: [...new Set(allProofRefs)],
      generatedAt: input.fetchedAt,
      gradientCloseoutRefs: [...new Set(allCheckpointRefs)],
      lossCurve,
      maxValidationLoss,
      mergeRef: safePacketRef(bundle.mergeRef),
      receiptRefs: [...new Set(receiptRefs)],
      shardContributions: shards,
      sourceRefs: [...new Set(sourceRefs)],
      trainingRunRef: input.trainingRunRef,
    },
  }
}

export function buildTrainingEvidencePacket(
  input: BuildTrainingEvidencePacketInput,
): TrainingEvidencePacketBuildResponse {
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const trimmedRunRef = input.trainingRunRef.trim()

  if (!input.enabled) {
    return buildEvidencePacketResponse({
      blockerRefs: [evidencePacketWriteSource],
      enabled: false,
      fetchedAt,
      inputSource: null,
      message: "training evidence packet writing disabled",
      packetSource: null,
      reason: "disabled",
      trainingRunRef: trimmedRunRef === "" ? null : trimmedRunRef,
    })
  }

  if (
    trimmedRunRef.length < 3 ||
    trimmedRunRef.length > 260 ||
    !publicSafeRefPattern.test(trimmedRunRef)
  ) {
    return buildEvidencePacketResponse({
      blockerRefs: ["training.run_ref.invalid"],
      enabled: true,
      fetchedAt,
      inputSource: null,
      message: "invalid training run ref",
      packetSource: null,
      reason: "invalid_run_ref",
      trainingRunRef: trimmedRunRef === "" ? null : trimmedRunRef,
    })
  }

  const receiptsPath = input.workerReceiptsPath?.trim() ?? ""
  if (receiptsPath === "") {
    return buildEvidencePacketResponse({
      blockerRefs: [workerReceiptsSource],
      enabled: true,
      fetchedAt,
      inputSource: null,
      message: "training worker receipts path unavailable",
      packetSource: null,
      reason: "worker_receipts_path_missing",
      trainingRunRef: trimmedRunRef,
    })
  }

  const packetPath = input.evidencePacketPath?.trim() ?? ""
  if (packetPath === "") {
    return buildEvidencePacketResponse({
      blockerRefs: [evidencePacketSource],
      enabled: true,
      fetchedAt,
      inputSource: workerReceiptsSource,
      message: "training evidence packet path unavailable",
      packetSource: null,
      reason: "packet_path_missing",
      trainingRunRef: trimmedRunRef,
    })
  }

  let bundle: unknown
  try {
    bundle =
      input.readBundle?.(receiptsPath) ??
      (JSON.parse(readFileSync(receiptsPath, "utf8")) as unknown)
  } catch {
    return buildEvidencePacketResponse({
      blockerRefs: ["training.worker_receipts.read_failed"],
      enabled: true,
      error: "training worker receipts read failed",
      fetchedAt,
      inputSource: workerReceiptsSource,
      message: "training worker receipts read failed",
      packetSource: evidencePacketSource,
      reason: "worker_receipts_read_failed",
      trainingRunRef: trimmedRunRef,
    })
  }

  if (!isRecord(bundle) && !Array.isArray(bundle)) {
    return buildEvidencePacketResponse({
      blockerRefs: ["training.worker_receipts.invalid_json"],
      enabled: true,
      error: "training worker receipts must be a JSON object or array",
      fetchedAt,
      inputSource: workerReceiptsSource,
      message: "training worker receipts must be a JSON object or array",
      packetSource: evidencePacketSource,
      reason: "worker_receipts_invalid",
      trainingRunRef: trimmedRunRef,
    })
  }

  const generated = generatedPacketFromWorkerReceipts({
    bundle,
    fetchedAt,
    trainingRunRef: trimmedRunRef,
  })

  if (generated.packet === null) {
    return buildEvidencePacketResponse({
      blockerRefs: generated.blockerRefs,
      enabled: true,
      fetchedAt,
      inputSource: workerReceiptsSource,
      message: "training worker receipts did not contain enough public-safe refs",
      packetSource: evidencePacketSource,
      reason: "worker_receipts_invalid",
      trainingRunRef: trimmedRunRef,
    })
  }

  let summary = readTrainingEvidencePacketSummary({
    evidencePacketPath: packetPath,
    nowIso: () => fetchedAt,
    readPacket: () => generated.packet,
  })

  try {
    if (input.writePacket !== undefined) {
      input.writePacket(packetPath, generated.packet)
    } else {
      writeFileSync(
        packetPath,
        `${JSON.stringify(generated.packet, null, 2)}\n`,
        "utf8",
      )
    }
  } catch {
    return buildEvidencePacketResponse({
      blockerRefs: ["training.evidence_packet.write_failed"],
      enabled: true,
      error: "training evidence packet write failed",
      fetchedAt,
      inputSource: workerReceiptsSource,
      message: "training evidence packet write failed",
      packetSource: evidencePacketSource,
      reason: "packet_write_failed",
      summary,
      trainingRunRef: trimmedRunRef,
    })
  }

  summary = {
    ...summary,
    fetchedAt,
  }
  const blockerCount = summary.blockerRefs.length
  return buildEvidencePacketResponse({
    enabled: true,
    fetchedAt,
    inputSource: workerReceiptsSource,
    message: summary.ok
      ? `wrote evidence packet candidate · ${summary.receiptRefCount} receipts`
      : `wrote evidence packet candidate · ${blockerCount} blockers`,
    packetSource: evidencePacketSource,
    reason: summary.ok ? "written" : "packet_blocked",
    summary,
    trainingRunRef: trimmedRunRef,
  })
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

const leaderboardLane = (value: unknown): TrainingLeaderboardLane | null =>
  typeof value === "string" &&
  trainingLeaderboardLanes.has(value as TrainingLeaderboardLane)
    ? (value as TrainingLeaderboardLane)
    : null

const leaderboardTopRow = (value: unknown): TrainingLeaderboardTopRow | null => {
  if (!isRecord(value)) return null
  const contributorRef = asString(value.contributorRef)
  const trainingRunRef = asString(value.trainingRunRef)
  if (contributorRef === "" || trainingRunRef === "") return null
  return {
    contributorRef,
    rank: asNumber(value.rank),
    score: asNumber(value.score),
    scoreLabel: asString(value.scoreLabel),
    settledPayoutSats: asNumber(value.settledPayoutSats),
    trainingRunRef,
  }
}

const leaderboardSection = (
  value: unknown,
): TrainingLeaderboardLaneSummary | null => {
  if (!isRecord(value)) return null
  const lane = leaderboardLane(value.lane)
  if (lane === null) return null
  const rows = asArray(value.rows).filter(isRecord)
  const topRow =
    [...rows]
      .sort(
        (a, b) =>
          asNumber(a.rank, Number.MAX_SAFE_INTEGER) -
          asNumber(b.rank, Number.MAX_SAFE_INTEGER),
      )
      .map(leaderboardTopRow)
      .find((row): row is TrainingLeaderboardTopRow => row !== null) ?? null
  return {
    blockerRefs: stringArray(value.blockerRefs),
    lane,
    rowCount: rows.length,
    title: asString(value.title, lane),
    topRow,
  }
}

export async function fetchTrainingDashboard(
  input: FetchTrainingDashboardInput,
): Promise<TrainingDashboardSummaryResponse> {
  const fetchFn = input.fetchFn ?? fetch
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const baseUrl = normalizeBaseUrl(input.baseUrl)
  const sourceUrl = `${baseUrl}/api/training/leaderboards`
  const endpoints = {
    leaderboards: sourceUrl,
    a2: `${baseUrl}/api/training/device-capabilities/a2`,
    a3: `${baseUrl}/api/training/isoflop/a3`,
    a4: `${baseUrl}/api/training/refinery/a4`,
    a5: `${baseUrl}/api/training/evals/a5`,
  } as const

  try {
    const [leaderboards, a2, a3, a4, a5] = await Promise.all([
      getPublicJson(fetchFn, endpoints.leaderboards),
      getPublicJson(fetchFn, endpoints.a2),
      getPublicJson(fetchFn, endpoints.a3),
      getPublicJson(fetchFn, endpoints.a4),
      getPublicJson(fetchFn, endpoints.a5),
    ])

    if (!leaderboards.ok) {
      return emptyTrainingDashboardResponse({
        ok: false,
        fetchedAt,
        sourceUrl,
        error: `leaderboards: ${leaderboards.error}`,
      })
    }
    if (!a2.ok) {
      return emptyTrainingDashboardResponse({
        ok: false,
        fetchedAt,
        sourceUrl,
        error: `a2: ${a2.error}`,
      })
    }
    if (!a3.ok) {
      return emptyTrainingDashboardResponse({
        ok: false,
        fetchedAt,
        sourceUrl,
        error: `a3: ${a3.error}`,
      })
    }
    if (!a4.ok) {
      return emptyTrainingDashboardResponse({
        ok: false,
        fetchedAt,
        sourceUrl,
        error: `a4: ${a4.error}`,
      })
    }
    if (!a5.ok) {
      return emptyTrainingDashboardResponse({
        ok: false,
        fetchedAt,
        sourceUrl,
        error: `a5: ${a5.error}`,
      })
    }

    const leaderboardsRecord = isRecord(leaderboards.json) ? leaderboards.json : {}
    const a2Record = isRecord(a2.json) ? a2.json : {}
    const a3Record = isRecord(a3.json) ? a3.json : {}
    const a4Record = isRecord(a4.json) ? a4.json : {}
    const a5Record = isRecord(a5.json) ? a5.json : {}
    const a2ClassDistributions = asArray(a2Record.classDistributions)
    const a3Cells = asArray(a3Record.cells)
    const a5Suites = asArray(a5Record.evalSuites)

    return {
      ok: true,
      fetchedAt,
      sourceUrl,
      leaderboards: {
        blockerRefs: stringArray(leaderboardsRecord.blockerRefs),
        lanes: asArray(leaderboardsRecord.lanes)
          .map(leaderboardSection)
          .filter((lane): lane is TrainingLeaderboardLaneSummary => lane !== null),
      },
      a2: {
        blockerRefs: stringArray(a2Record.blockerRefs),
        observedDeviceClassCount: asNumber(a2Record.observedDeviceClassCount),
        observedMeasurementCount: asNumber(a2Record.observedMeasurementCount),
        verifiedMeasurementCount: a2ClassDistributions.filter(
          item => isRecord(item) && item.verified === true,
        ).length,
      },
      a3: {
        blockerRefs: stringArray(a3Record.blockerRefs),
        cellCount: a3Cells.length,
        fitArtifactCount: asArray(a3Record.fitArtifacts).length,
        verifiedCellCount: a3Cells.filter(
          item => isRecord(item) && item.verified === true,
        ).length,
      },
      a4: {
        blockerRefs: stringArray(a4Record.blockerRefs),
        evalDeltaBonusBlockerRefs: stringArray(
          a4Record.evalDeltaBonusBlockerRefs,
        ),
        observedVerifiedStages: stringArray(a4Record.observedVerifiedStages),
        requiredVerifiedStageCount: asNumber(
          a4Record.requiredVerifiedStageCount,
        ),
        shardCount: asArray(a4Record.shards).length,
      },
      a5: {
        blockerRefs: stringArray(a5Record.blockerRefs),
        evalSuiteCount: a5Suites.length,
        updateBoundaryRef: asNullableString(a5Record.updateBoundaryRef),
        verifiedSuiteCount: a5Suites.filter(
          item =>
            isRecord(item) && stringArray(item.verificationRefs).length > 0,
        ).length,
      },
    }
  } catch (error) {
    return emptyTrainingDashboardResponse({
      ok: false,
      fetchedAt,
      sourceUrl,
      error: error instanceof Error ? error.message : String(error),
    })
  }
}

const promiseState = (value: unknown): TrainingPromiseState =>
  typeof value === "string" &&
  trainingPromiseStates.has(value as TrainingPromiseState)
    ? (value as TrainingPromiseState)
    : "unknown"

const trainingPromiseSummary = (
  value: unknown,
): TrainingPromiseSummary | null => {
  if (!isRecord(value)) return null
  const promiseId = asString(value.promiseId)
  const productArea = asString(value.productArea)
  if (
    promiseId === "" ||
    !(productArea === "training" || trainingPromiseIds.has(promiseId))
  ) {
    return null
  }

  return {
    blockerRefs: stringArray(value.blockerRefs),
    claim: asString(value.claim),
    evidenceRefCount: stringArray(value.evidenceRefs).length,
    productArea,
    promiseId,
    safeCopy: asString(value.safeCopy),
    state: promiseState(value.state),
    verification: asString(value.verification),
  }
}

const emptyTrainingPromiseStateCounts = (): Record<
  TrainingPromiseState,
  number
> => ({
  degraded: 0,
  green: 0,
  planned: 0,
  red: 0,
  withdrawn: 0,
  yellow: 0,
  unknown: 0,
})

const promiseStateSortRank = (state: TrainingPromiseState): number => {
  switch (state) {
    case "red":
      return 0
    case "yellow":
      return 1
    case "degraded":
      return 2
    case "planned":
      return 3
    case "green":
      return 4
    case "withdrawn":
      return 5
    case "unknown":
      return 6
  }
}

export async function fetchTrainingPromiseGates(
  input: FetchTrainingPromiseGatesInput,
): Promise<TrainingPromiseGatesResponse> {
  const fetchFn = input.fetchFn ?? fetch
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const sourceUrl = `${normalizeBaseUrl(input.baseUrl)}/api/public/product-promises`

  try {
    const result = await getPublicJson(fetchFn, sourceUrl)
    if (!result.ok) {
      return emptyTrainingPromiseGatesResponse({
        ok: false,
        fetchedAt,
        sourceUrl,
        error: result.error,
      })
    }

    const record = isRecord(result.json) ? result.json : {}
    const promises = asArray(record.promises)
      .map(trainingPromiseSummary)
      .filter((item): item is TrainingPromiseSummary => item !== null)
      .sort((left, right) => {
        const stateRank =
          promiseStateSortRank(left.state) - promiseStateSortRank(right.state)
        return stateRank === 0
          ? left.promiseId.localeCompare(right.promiseId)
          : stateRank
      })
    const stateCounts = emptyTrainingPromiseStateCounts()
    for (const promise of promises) {
      stateCounts[promise.state] += 1
    }

    return {
      ok: true,
      fetchedAt,
      registryVersion: asString(
        record.registryVersion,
        asString(record.version),
      ),
      sourceUrl,
      blockerRefs: [
        ...new Set(promises.flatMap(promise => promise.blockerRefs)),
      ].sort(),
      promises,
      stateCounts,
    }
  } catch (error) {
    return emptyTrainingPromiseGatesResponse({
      ok: false,
      fetchedAt,
      sourceUrl,
      error: error instanceof Error ? error.message : String(error),
    })
  }
}

export async function fetchTrainingRuns(
  input: FetchTrainingRunsInput,
): Promise<TrainingRunsResponse> {
  const fetchFn = input.fetchFn ?? fetch
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const baseUrl = normalizeBaseUrl(input.baseUrl)
  const sourceUrl = `${baseUrl}/api/training/runs`
  const tassadarSummaryUrl = `${baseUrl}/api/public/tassadar-run-summary`

  try {
    const [response, tassadarSummaryResult] = await Promise.all([
      fetchFn(sourceUrl, {
        headers: { accept: "application/json" },
      }),
      getPublicJson(fetchFn, tassadarSummaryUrl).catch((error) => ({
        ok: false as const,
        error: error instanceof Error ? error.message : String(error),
      })),
    ])
    const tassadarSummary =
      tassadarSummaryResult.ok && isRecord(tassadarSummaryResult.json)
        ? (tassadarSummaryResult.json as PublicTassadarRunSummary)
        : null
    if (!response.ok) {
      return {
        ok: false,
        error: `training runs ${response.status}`,
        fetchedAt,
        sourceUrl,
        runs: [],
        summaries: [],
        tassadarSummary,
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
      tassadarSummary,
    }
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : String(error),
      fetchedAt,
      sourceUrl,
      runs: [],
      summaries: [],
      tassadarSummary: null,
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
  return transitionTrainingWindow({
    ...input,
    action: "activate",
    actorRef: "operator.openagents.autopilot_desktop",
    disabledMessage: "training admin activation disabled",
    failurePrefix: "window activation failed",
    receiptPrefix: "receipt.desktop.training.window.activate",
    requestFailurePrefix: "training admin activation failed",
    successReason: "activated",
  })
}

export async function reconcileTrainingWindow(
  input: ReconcileTrainingWindowInput,
): Promise<TrainingWindowActionResponse> {
  return transitionTrainingWindow({
    ...input,
    action: "reconcile",
    actorRef: "operator.openagents.autopilot_desktop",
    disabledMessage: "training admin reconciliation disabled",
    failurePrefix: "window reconciliation failed",
    receiptPrefix: "receipt.desktop.training.window.reconcile",
    requestFailurePrefix: "training admin reconciliation failed",
    successReason: "reconciled",
  })
}

async function transitionTrainingWindow(
  input: (ActivateTrainingWindowInput | ReconcileTrainingWindowInput) &
    Readonly<{
      action: "activate" | "reconcile"
      actorRef: string
      disabledMessage: string
      failurePrefix: string
      receiptPrefix: string
      requestFailurePrefix: string
      successReason: "activated" | "reconciled"
    }>,
): Promise<TrainingWindowActionResponse> {
  const fetchFn = input.fetchFn ?? fetch
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const baseUrl = normalizeBaseUrl(input.baseUrl)
  const trimmedWindowRef = input.windowRef.trim()
  const sourceUrl =
    trimmedWindowRef === ""
      ? `${baseUrl}/api/training/windows/${input.action}`
      : `${baseUrl}/api/training/windows/${encodeURIComponent(trimmedWindowRef)}/${input.action}`

  if (!input.enabled) {
    return disabledWindowActionResponse({
      enabled: false,
      fetchedAt,
      message: input.disabledMessage,
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
      actorRef: input.actorRef,
      receiptRef: `${input.receiptPrefix}.${stamp}`,
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
        message: `${input.failurePrefix}: ${result.error}`,
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
      reason: input.successReason,
      message: `${input.successReason} ${activatedWindowRef}`,
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
      message: `${input.requestFailurePrefix}: ${text}`,
      error: text,
    }
  }
}

export async function claimTrainingWindowLease(
  input: ClaimTrainingWindowLeaseInput,
): Promise<TrainingWindowLeaseResponse> {
  const fetchFn = input.fetchFn ?? fetch
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const baseUrl = normalizeBaseUrl(input.baseUrl)
  const sourceUrl = `${baseUrl}/api/training/leases/claim`
  const trimmedPylonRef = input.pylonRef?.trim() ?? ""

  if (!input.enabled) {
    return disabledLeaseResponse({
      enabled: false,
      fetchedAt,
      message: "training lease claiming disabled",
      pylonRef: trimmedPylonRef === "" ? null : trimmedPylonRef,
      reason: "disabled",
      sourceUrl,
    })
  }

  if (trimmedPylonRef === "") {
    return disabledLeaseResponse({
      enabled: true,
      fetchedAt,
      message: "training Pylon ref unavailable",
      pylonRef: null,
      reason: "pylon_ref_missing",
      sourceUrl,
    })
  }

  if (
    trimmedPylonRef.length < 3 ||
    trimmedPylonRef.length > 120 ||
    !publicSafePylonRefPattern.test(trimmedPylonRef)
  ) {
    return disabledLeaseResponse({
      enabled: true,
      fetchedAt,
      message: "invalid training Pylon ref",
      pylonRef: trimmedPylonRef,
      reason: "invalid_pylon_ref",
      sourceUrl,
    })
  }

  const stamp = safeRefStamp(fetchedAt)

  try {
    const result = await postPublicJson(fetchFn, sourceUrl, {
      ...(input.leaseSeconds === undefined
        ? {}
        : { leaseSeconds: input.leaseSeconds }),
      pylonRef: trimmedPylonRef,
      receiptRefs: [`receipt.desktop.training.lease.claim.${stamp}`],
    })

    if (!result.ok) {
      return {
        ok: false,
        enabled: true,
        fetchedAt,
        sourceUrl,
        pylonRef: trimmedPylonRef,
        lease: null,
        reason: "claim_failed",
        message: `lease claim failed: ${result.error}`,
        error: result.error,
      }
    }

    const record = isRecord(result.json) ? result.json : {}
    const lease = leaseProjection(record.lease, fetchedAt)
    return {
      ok: lease !== null,
      enabled: true,
      fetchedAt,
      sourceUrl,
      pylonRef: trimmedPylonRef,
      lease,
      reason: lease === null ? "claim_failed" : "claimed",
      message:
        lease === null
          ? "lease claim response did not include a lease"
          : `claimed ${lease.leaseRef} for ${lease.windowRef}`,
      ...(lease === null
        ? { error: "lease claim response did not include a lease" }
        : {}),
    }
  } catch (error) {
    const text = error instanceof Error ? error.message : String(error)
    return {
      ok: false,
      enabled: true,
      fetchedAt,
      sourceUrl,
      pylonRef: trimmedPylonRef,
      lease: null,
      reason: "request_failed",
      message: `training lease claim failed: ${text}`,
      error: text,
    }
  }
}

export async function requestTrainingBootstrapGrant(
  input: RequestTrainingBootstrapGrantInput,
): Promise<TrainingBootstrapGrantResponse> {
  const fetchFn = input.fetchFn ?? fetch
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const baseUrl = normalizeBaseUrl(input.baseUrl)
  const trimmedRunRef = input.trainingRunRef.trim()
  const sourceUrl =
    trimmedRunRef === ""
      ? `${baseUrl}/api/training/runs/bootstrap-grant`
      : `${baseUrl}/api/training/runs/${encodeURIComponent(trimmedRunRef)}/bootstrap-grant`
  const trimmedPylonRef = input.pylonRef?.trim() ?? ""

  if (trimmedRunRef.length < 3 || !publicSafeRefPattern.test(trimmedRunRef)) {
    return disabledBootstrapResponse({
      fetchedAt,
      message: "invalid training run ref",
      pylonRef: trimmedPylonRef === "" ? null : trimmedPylonRef,
      reason: "invalid_run_ref",
      sourceUrl,
      trainingRunRef: trimmedRunRef === "" ? null : trimmedRunRef,
    })
  }

  if (trimmedPylonRef === "") {
    return disabledBootstrapResponse({
      fetchedAt,
      message: "training Pylon ref unavailable",
      pylonRef: null,
      reason: "pylon_ref_missing",
      sourceUrl,
      trainingRunRef: trimmedRunRef,
    })
  }

  if (
    trimmedPylonRef.length < 3 ||
    trimmedPylonRef.length > 120 ||
    !publicSafePylonRefPattern.test(trimmedPylonRef)
  ) {
    return disabledBootstrapResponse({
      fetchedAt,
      message: "invalid training Pylon ref",
      pylonRef: trimmedPylonRef,
      reason: "invalid_pylon_ref",
      sourceUrl,
      trainingRunRef: trimmedRunRef,
    })
  }

  const stamp = safeRefStamp(fetchedAt)

  try {
    const result = await postPublicJson(fetchFn, sourceUrl, {
      joinerRef: trimmedPylonRef,
      receiptRefs: [`receipt.desktop.training.bootstrap.request.${stamp}`],
    })

    if (!result.ok) {
      return {
        ok: false,
        fetchedAt,
        sourceUrl,
        pylonRef: trimmedPylonRef,
        trainingRunRef: trimmedRunRef,
        outcome: null,
        reason: "request_failed",
        message: `bootstrap grant failed: ${result.error}`,
        error: result.error,
      }
    }

    const record = isRecord(result.json) ? result.json : {}
    const outcome = bootstrapOutcome(record.outcome)
    const reason =
      outcome?.kind === "granted"
        ? "granted"
        : outcome?.kind === "queued"
          ? "queued"
          : outcome?.kind === "refused"
            ? "refused"
            : "request_failed"
    const message =
      outcome?.kind === "granted"
        ? `bootstrap grant ${outcome.grant.grantRef} from ${outcome.grant.sealedWindowRef}`
        : outcome?.kind === "queued"
          ? `bootstrap queued: ${outcome.reasonCode}`
          : outcome?.kind === "refused"
            ? `bootstrap refused: ${outcome.reasonCode}`
            : "bootstrap grant response did not include an outcome"

    return {
      ok: outcome?.kind === "granted",
      fetchedAt,
      sourceUrl,
      pylonRef: trimmedPylonRef,
      trainingRunRef: trimmedRunRef,
      outcome,
      reason,
      message,
      ...(outcome === null
        ? { error: "bootstrap grant response did not include an outcome" }
        : {}),
    }
  } catch (error) {
    const text = error instanceof Error ? error.message : String(error)
    return {
      ok: false,
      fetchedAt,
      sourceUrl,
      pylonRef: trimmedPylonRef,
      trainingRunRef: trimmedRunRef,
      outcome: null,
      reason: "request_failed",
      message: `training bootstrap grant failed: ${text}`,
      error: text,
    }
  }
}

export async function admitTrainingRealGradientEvidence(
  input: AdmitTrainingRealGradientEvidenceInput,
): Promise<TrainingEvidenceAdmissionResponse> {
  const fetchFn = input.fetchFn ?? fetch
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const baseUrl = normalizeBaseUrl(input.baseUrl)
  const trimmedRunRef = input.trainingRunRef.trim()
  const sourceUrl =
    trimmedRunRef === ""
      ? `${baseUrl}/api/training/runs/real-gradient-evidence`
      : `${baseUrl}/api/training/runs/${encodeURIComponent(trimmedRunRef)}/real-gradient-evidence`
  const packetSource = evidencePacketSource

  if (!input.enabled) {
    return disabledEvidenceAdmissionResponse({
      enabled: false,
      fetchedAt,
      message: "training evidence admission disabled",
      packetSource: null,
      reason: "disabled",
      sourceUrl,
      trainingRunRef: trimmedRunRef === "" ? null : trimmedRunRef,
    })
  }

  const token = input.adminToken?.trim() ?? ""
  if (token === "") {
    return disabledEvidenceAdmissionResponse({
      enabled: true,
      fetchedAt,
      message: "training admin token unavailable",
      packetSource: null,
      reason: "admin_token_missing",
      sourceUrl,
      trainingRunRef: trimmedRunRef === "" ? null : trimmedRunRef,
    })
  }

  if (
    trimmedRunRef.length < 3 ||
    trimmedRunRef.length > 260 ||
    !publicSafeRefPattern.test(trimmedRunRef)
  ) {
    return disabledEvidenceAdmissionResponse({
      enabled: true,
      fetchedAt,
      message: "invalid training run ref",
      packetSource: null,
      reason: "invalid_run_ref",
      sourceUrl,
      trainingRunRef: trimmedRunRef === "" ? null : trimmedRunRef,
    })
  }

  const packetPath = input.evidencePacketPath?.trim() ?? ""
  if (packetPath === "") {
    return disabledEvidenceAdmissionResponse({
      enabled: true,
      fetchedAt,
      message: "training evidence packet path unavailable",
      packetSource: null,
      reason: "packet_path_missing",
      sourceUrl,
      trainingRunRef: trimmedRunRef,
    })
  }

  let packet: unknown
  try {
    packet =
      input.readPacket?.(packetPath) ??
      JSON.parse(readFileSync(packetPath, "utf8")) as unknown
  } catch {
    return disabledEvidenceAdmissionResponse({
      enabled: true,
      error: "training evidence packet read failed",
      fetchedAt,
      message: "training evidence packet read failed",
      packetSource,
      reason: "packet_read_failed",
      sourceUrl,
      trainingRunRef: trimmedRunRef,
    })
  }

  if (!isRecord(packet)) {
    return disabledEvidenceAdmissionResponse({
      enabled: true,
      error: "training evidence packet must be a JSON object",
      fetchedAt,
      message: "training evidence packet must be a JSON object",
      packetSource,
      reason: "packet_invalid",
      sourceUrl,
      trainingRunRef: trimmedRunRef,
    })
  }

  const stats = evidencePacketStats(packet)

  try {
    const result = await postJson(fetchFn, sourceUrl, token, packet)

    if (!result.ok) {
      return {
        ok: false,
        enabled: true,
        fetchedAt,
        sourceUrl,
        trainingRunRef: trimmedRunRef,
        packetSource,
        run: null,
        realGradient: null,
        reason: "admission_failed",
        message: `real-gradient evidence admission failed: ${result.error}`,
        evidenceRefCount: stats.evidenceRefCount,
        receiptRefCount: stats.receiptRefCount,
        shardContributionCount: stats.shardContributionCount,
        distinctPylonCount: stats.distinctPylonCount,
        error: result.error,
      }
    }

    const record = isRecord(result.json) ? result.json : {}
    const run = runProjection(record.run)
    const admittedGradient = isRecord(record.realGradient)
      ? realGradient(record.realGradient)
      : null

    if (run === null || admittedGradient === null) {
      return {
        ok: false,
        enabled: true,
        fetchedAt,
        sourceUrl,
        trainingRunRef: trimmedRunRef,
        packetSource,
        run,
        realGradient: admittedGradient,
        reason: "admission_failed",
        message: "real-gradient evidence response did not include run and realGradient projections",
        evidenceRefCount: stats.evidenceRefCount,
        receiptRefCount: stats.receiptRefCount,
        shardContributionCount: stats.shardContributionCount,
        distinctPylonCount: stats.distinctPylonCount,
        error:
          "real-gradient evidence response did not include run and realGradient projections",
      }
    }

    return {
      ok: true,
      enabled: true,
      fetchedAt,
      sourceUrl,
      trainingRunRef: run.trainingRunRef,
      packetSource,
      run,
      realGradient: admittedGradient,
      reason: "admitted",
      message: `admitted A1 real-gradient evidence for ${run.trainingRunRef} · ${stats.receiptRefCount} receipts`,
      evidenceRefCount: stats.evidenceRefCount,
      receiptRefCount: stats.receiptRefCount,
      shardContributionCount: stats.shardContributionCount,
      distinctPylonCount: stats.distinctPylonCount,
    }
  } catch (error) {
    const text = error instanceof Error ? error.message : String(error)
    return {
      ok: false,
      enabled: true,
      fetchedAt,
      sourceUrl,
      trainingRunRef: trimmedRunRef,
      packetSource,
      run: null,
      realGradient: null,
      reason: "request_failed",
      message: `training evidence admission failed: ${text}`,
      evidenceRefCount: stats.evidenceRefCount,
      receiptRefCount: stats.receiptRefCount,
      shardContributionCount: stats.shardContributionCount,
      distinctPylonCount: stats.distinctPylonCount,
      error: text,
    }
  }
}
