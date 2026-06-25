export const DEFAULT_GLM_52_REAP_504B_OWNED_COST_PROFILE_REF =
  'cost_profile.hydralisk.glm_52_reap_504b.g4_4g.spot.2026_06_25'

const GLM_52_REAP_504B_MODEL_REF = 'openagents/glm-5.2-reap-504b'
const HYDRALISK_SUPPLY_LANE = 'hydralisk'
const HOURS_PER_MONTH = 730
const NOT_MEASURED = 'not_measured' as const
const COST_SOURCE_REF = 'evidence.gcp.g4_gpu_costs.2026_06_25.owner_estimate'
const COST_EVIDENCE_REFS = [
  'evidence.gcp.g4_standard_192.spot_usd_2696_month.2026_06_25',
  'evidence.gcp.g4_standard_192.ondemand_usd_13140_month.2026_06_25',
  'evidence.gcp.g4_standard_192.dws_flex_usd_6570_month.2026_06_25',
  'evidence.gcp.g4_standard_384.spot_usd_5392_month.2026_06_25',
  'evidence.gcp.g4_standard_384.ondemand_usd_26280_month.2026_06_25',
  'evidence.gcp.g4_standard_384.dws_flex_usd_13140_month.2026_06_25',
]

export type OwnedInferenceProvisioningModel = 'spot' | 'on_demand' | 'dws_flex'

export type OwnedInferenceCostProfile = Readonly<{
  profileRef: string
  supplyLane: string
  modelRef: string
  machineShape: 'g4-standard-192' | 'g4-standard-384'
  gpuCount: 4 | 8
  provisioningModel: OwnedInferenceProvisioningModel
  monthlyComputeUsd: number
  hourlyComputeUsd: number
  monthlyStorageOverheadUsd: number | typeof NOT_MEASURED
  hourlyStorageOverheadUsd: number | typeof NOT_MEASURED
  sourceRef: string
  evidenceRefs: ReadonlyArray<string>
}>

export type OwnedInferenceCostMetadataRow = Readonly<{
  accepted_outcomes: number | null
  benchmark_reserved: boolean | number | string | null
  demand_client: string | null
  demand_kind: string | null
  demand_source: string | null
  observed_at: string | null
  replica_cost_profile_ref: string | null
  selected_replica_id: string | null
  selected_replica_ref: string | null
  total_tokens: number | null
  total_wall_clock_ms: number | null
}>

export type OwnedInferenceReplicaCostSummary = Readonly<{
  effectiveCostPerServedTokenUsd: number | typeof NOT_MEASURED
  idleHours: number
  uptimeHours: number
}>

export type OwnedInferenceCostSummary = Readonly<{
  summary: {
    acceptedOutcomes: number | typeof NOT_MEASURED
    activeDemandBurnUsd: number
    activeServingHours: number
    benchmarkReservedBurnUsd: number | typeof NOT_MEASURED
    blockerRefs: ReadonlyArray<string>
    costCoverage: 'measured' | 'not_measured' | 'partial'
    costPerAcceptedOutcomeUsd: number | typeof NOT_MEASURED
    demand: ReadonlyArray<{
      activeDemandBurnUsd: number
      activeServingHours: number
      costPerServedTokenUsd: number | typeof NOT_MEASURED
      demandClient: string
      demandKind: 'external' | 'internal' | 'unlabeled'
      demandSource: string
      key: string
      label: string
      totalTokens: number
      usageEvents: number
    }>
    effectiveCostPerServedTokenUsd: number | typeof NOT_MEASURED
    externalDemandBurnUsd: number
    hourlyBurnUsd: number | typeof NOT_MEASURED
    idleBurnUsd: number | typeof NOT_MEASURED
    idleHours: number | typeof NOT_MEASURED
    internalDemandBurnUsd: number
    keepWarmBurnUsd: number | typeof NOT_MEASURED
    monthlyBurnUsd: number | typeof NOT_MEASURED
    profiles: ReadonlyArray<OwnedInferenceCostProfile>
    scenarios: ReadonlyArray<{
      acceptedOutcomes: number | typeof NOT_MEASURED
      activeDemandBurnUsd: number
      activeServingHours: number
      benchmarkReservedBurnUsd: number | typeof NOT_MEASURED
      costPerAcceptedOutcomeUsd: number | typeof NOT_MEASURED
      effectiveCostPerServedTokenUsd: number | typeof NOT_MEASURED
      externalDemandBurnUsd: number
      gpuCount: 4 | 8
      hourlyBurnUsd: number
      idleBurnUsd: number
      idleHours: number
      internalDemandBurnUsd: number
      keepWarmBurnUsd: number | typeof NOT_MEASURED
      machineShape: 'g4-standard-192' | 'g4-standard-384'
      monthlyComputeUsd: number
      profileRef: string
      provisioningModel: OwnedInferenceProvisioningModel
      replicaCount: number
      sourceRef: string
      storageOverheadUsd: number | typeof NOT_MEASURED
      unlabeledDemandBurnUsd: number
      uptimeHours: number
      windowBurnUsd: number
    }>
    storageOverheadUsd: number | typeof NOT_MEASURED
    unlabeledDemandBurnUsd: number
    uptimeHours: number | typeof NOT_MEASURED
    windowBurnUsd: number | typeof NOT_MEASURED
  }
  byReplicaRef: ReadonlyMap<string, OwnedInferenceReplicaCostSummary>
}>

type ProfileSeed = Readonly<{
  gpuCount: 4 | 8
  machineShape: 'g4-standard-192' | 'g4-standard-384'
  monthlyComputeUsd: number
  profileRef: string
  provisioningModel: OwnedInferenceProvisioningModel
}>

const roundUsd = (value: number): number =>
  Math.round(Math.max(0, value) * 1_000_000) / 1_000_000

const roundHours = (value: number): number =>
  Math.round(Math.max(0, value) * 1_000_000) / 1_000_000

const profileSeeds: ReadonlyArray<ProfileSeed> = [
  {
    gpuCount: 4,
    machineShape: 'g4-standard-192',
    monthlyComputeUsd: 2696,
    profileRef: DEFAULT_GLM_52_REAP_504B_OWNED_COST_PROFILE_REF,
    provisioningModel: 'spot',
  },
  {
    gpuCount: 4,
    machineShape: 'g4-standard-192',
    monthlyComputeUsd: 13140,
    profileRef:
      'cost_profile.hydralisk.glm_52_reap_504b.g4_4g.on_demand.2026_06_25',
    provisioningModel: 'on_demand',
  },
  {
    gpuCount: 4,
    machineShape: 'g4-standard-192',
    monthlyComputeUsd: 6570,
    profileRef:
      'cost_profile.hydralisk.glm_52_reap_504b.g4_4g.dws_flex.2026_06_25',
    provisioningModel: 'dws_flex',
  },
  {
    gpuCount: 8,
    machineShape: 'g4-standard-384',
    monthlyComputeUsd: 5392,
    profileRef: 'cost_profile.hydralisk.glm_52_reap_504b.g4_8g.spot.2026_06_25',
    provisioningModel: 'spot',
  },
  {
    gpuCount: 8,
    machineShape: 'g4-standard-384',
    monthlyComputeUsd: 26280,
    profileRef:
      'cost_profile.hydralisk.glm_52_reap_504b.g4_8g.on_demand.2026_06_25',
    provisioningModel: 'on_demand',
  },
  {
    gpuCount: 8,
    machineShape: 'g4-standard-384',
    monthlyComputeUsd: 13140,
    profileRef:
      'cost_profile.hydralisk.glm_52_reap_504b.g4_8g.dws_flex.2026_06_25',
    provisioningModel: 'dws_flex',
  },
]

const profileFromSeed = (seed: ProfileSeed): OwnedInferenceCostProfile => ({
  evidenceRefs: COST_EVIDENCE_REFS,
  gpuCount: seed.gpuCount,
  hourlyComputeUsd: roundUsd(seed.monthlyComputeUsd / HOURS_PER_MONTH),
  hourlyStorageOverheadUsd: NOT_MEASURED,
  machineShape: seed.machineShape,
  modelRef: GLM_52_REAP_504B_MODEL_REF,
  monthlyComputeUsd: seed.monthlyComputeUsd,
  monthlyStorageOverheadUsd: NOT_MEASURED,
  profileRef: seed.profileRef,
  provisioningModel: seed.provisioningModel,
  sourceRef: COST_SOURCE_REF,
  supplyLane: HYDRALISK_SUPPLY_LANE,
})

export const OWNED_INFERENCE_COST_PROFILES: ReadonlyArray<OwnedInferenceCostProfile> =
  profileSeeds.map(profileFromSeed)

const profileByRef = new Map(
  OWNED_INFERENCE_COST_PROFILES.map(profile => [profile.profileRef, profile]),
)

const profileAliases = new Map<string, string>([
  [
    'cost.hydralisk.glm_52_reap_504b.g4_spot.tp4.v1',
    DEFAULT_GLM_52_REAP_504B_OWNED_COST_PROFILE_REF,
  ],
  [
    'cost_profile.hydralisk.glm_52_reap_504b.g4_4g.spot.primary.v1',
    DEFAULT_GLM_52_REAP_504B_OWNED_COST_PROFILE_REF,
  ],
])

const finiteNonNegative = (value: number | null | undefined): number =>
  typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : 0

const positiveIntOrUndefined = (
  value: number | null | undefined,
): number | undefined => {
  const parsed = finiteNonNegative(value)
  return Number.isFinite(parsed) && parsed > 0 ? Math.trunc(parsed) : undefined
}

const optionalText = (value: string | null | undefined): string | undefined => {
  const trimmed = value?.trim()
  return trimmed === undefined || trimmed === '' ? undefined : trimmed
}

const normalizeDemandKind = (
  value: string | null | undefined,
): 'external' | 'internal' | 'unlabeled' =>
  value === 'external' || value === 'internal' ? value : 'unlabeled'

const isBenchmarkReserved = (
  value: boolean | number | string | null | undefined,
): boolean =>
  value === true ||
  value === 1 ||
  (typeof value === 'string' && value.trim().toLowerCase() === 'true')

const hoursBetween = (earlierIso: string, laterIso: string): number => {
  const earlier = Date.parse(earlierIso)
  const later = Date.parse(laterIso)
  if (
    !Number.isFinite(earlier) ||
    !Number.isFinite(later) ||
    later <= earlier
  ) {
    return 0
  }
  return roundHours((later - earlier) / 3_600_000)
}

const profileForRef = (
  profileRef: string | null | undefined,
): OwnedInferenceCostProfile | undefined => {
  const trimmed = optionalText(profileRef)
  if (trimmed === undefined) {
    return undefined
  }
  return profileByRef.get(profileAliases.get(trimmed) ?? trimmed)
}

export const ownedInferenceCostProfileForRef = profileForRef

const scenarioProfilesFor = (
  profile: OwnedInferenceCostProfile,
): ReadonlyArray<OwnedInferenceCostProfile> =>
  OWNED_INFERENCE_COST_PROFILES.filter(
    candidate =>
      candidate.machineShape === profile.machineShape &&
      candidate.gpuCount === profile.gpuCount,
  )

const defaultProfile = (): OwnedInferenceCostProfile =>
  profileByRef.get(DEFAULT_GLM_52_REAP_504B_OWNED_COST_PROFILE_REF)!

const selectedReplicaKey = (
  row: OwnedInferenceCostMetadataRow,
  fallbackIndex: number,
): string =>
  optionalText(row.selected_replica_ref) ??
  optionalText(row.selected_replica_id) ??
  `replica.hydralisk.glm_52_reap_504b.unattributed.${fallbackIndex}`

const isOwnedGlmRow = (row: OwnedInferenceCostMetadataRow): boolean =>
  profileForRef(row.replica_cost_profile_ref) !== undefined ||
  optionalText(row.selected_replica_ref)?.includes(
    'replica.hydralisk.glm_52_reap_504b',
  ) === true

const costPer = (
  numeratorUsd: number,
  denominator: number,
): number | typeof NOT_MEASURED =>
  denominator <= 0 ? NOT_MEASURED : roundUsd(numeratorUsd / denominator)

export const summarizeOwnedInferenceHourlyCost = (input: {
  nowIso: string
  rows: ReadonlyArray<OwnedInferenceCostMetadataRow>
  sinceIso: string | undefined
}): OwnedInferenceCostSummary => {
  const ownedRows = input.rows.filter(isOwnedGlmRow)
  const observedRows = ownedRows
    .map(row => optionalText(row.observed_at))
    .filter((value): value is string => value !== undefined)
    .sort()
  const sinceIso = input.sinceIso ?? observedRows[0]
  const windowHours =
    sinceIso === undefined ? 0 : hoursBetween(sinceIso, input.nowIso)
  const blockerRefs = new Set<string>([
    'blocker.inference_analytics.owned_hourly_host_lifecycle_derived_window_assumption',
    'blocker.inference_analytics.glm_storage_overhead_not_measured',
    'blocker.inference_analytics.glm_keepwarm_burn_not_measured',
    'blocker.inference_analytics.glm_benchmark_reserved_burn_not_measured',
  ])

  type ReplicaBucket = {
    acceptedOutcomes: number
    acceptedOutcomesMeasured: boolean
    activeServingHours: number
    benchmarkReserved: boolean
    demandRows: Array<OwnedInferenceCostMetadataRow>
    profile: OwnedInferenceCostProfile
    totalTokens: number
  }
  const replicas = new Map<string, ReplicaBucket>()

  if (ownedRows.length === 0) {
    replicas.set('replica.hydralisk.glm_52_reap_504b.primary.assumed', {
      acceptedOutcomes: 0,
      acceptedOutcomesMeasured: false,
      activeServingHours: 0,
      benchmarkReserved: false,
      demandRows: [],
      profile: defaultProfile(),
      totalTokens: 0,
    })
  }

  ownedRows.forEach((row, index) => {
    const replicaRef = selectedReplicaKey(row, index)
    const profile = profileForRef(row.replica_cost_profile_ref)
    if (profile === undefined) {
      blockerRefs.add(
        'blocker.inference_analytics.glm_replica_cost_profile_missing_defaulted',
      )
    }
    const bucket =
      replicas.get(replicaRef) ??
      ({
        acceptedOutcomes: 0,
        acceptedOutcomesMeasured: false,
        activeServingHours: 0,
        benchmarkReserved: false,
        demandRows: [],
        profile: profile ?? defaultProfile(),
        totalTokens: 0,
      } satisfies ReplicaBucket)
    bucket.totalTokens += Math.max(0, Math.trunc(row.total_tokens ?? 0))
    bucket.activeServingHours +=
      finiteNonNegative(row.total_wall_clock_ms) / 3_600_000
    bucket.benchmarkReserved =
      bucket.benchmarkReserved || isBenchmarkReserved(row.benchmark_reserved)
    const acceptedOutcomes = positiveIntOrUndefined(row.accepted_outcomes)
    if (acceptedOutcomes !== undefined) {
      bucket.acceptedOutcomes += acceptedOutcomes
      bucket.acceptedOutcomesMeasured = true
    }
    bucket.demandRows.push(row)
    replicas.set(replicaRef, bucket)
  })

  const replicaEntries = [...replicas.entries()].map(
    ([replicaRef, bucket]) => ({
      ...bucket,
      activeServingHours: roundHours(bucket.activeServingHours),
      activeServingHoursRaw: bucket.activeServingHours,
      idleHours: roundHours(
        Math.max(0, windowHours - bucket.activeServingHours),
      ),
      idleHoursRaw: Math.max(0, windowHours - bucket.activeServingHours),
      replicaRef,
    }),
  )
  const profiles = [
    ...new Set(replicaEntries.map(replica => replica.profile)),
  ].sort((left, right) => left.profileRef.localeCompare(right.profileRef))

  const totalTokens = replicaEntries.reduce(
    (sum, replica) => sum + replica.totalTokens,
    0,
  )
  const acceptedOutcomesMeasured = replicaEntries.some(
    replica => replica.acceptedOutcomesMeasured,
  )
  const acceptedOutcomes = acceptedOutcomesMeasured
    ? replicaEntries.reduce((sum, replica) => sum + replica.acceptedOutcomes, 0)
    : NOT_MEASURED
  if (!acceptedOutcomesMeasured) {
    blockerRefs.add(
      'blocker.inference_analytics.accepted_outcomes_not_measured',
    )
  }

  const actualHourlyBurnUsd = replicaEntries.reduce(
    (sum, replica) => sum + replica.profile.hourlyComputeUsd,
    0,
  )
  const actualMonthlyBurnUsd = replicaEntries.reduce(
    (sum, replica) => sum + replica.profile.monthlyComputeUsd,
    0,
  )
  const actualActiveHours = roundHours(
    replicaEntries.reduce(
      (sum, replica) => sum + replica.activeServingHours,
      0,
    ),
  )
  const actualUptimeHours = roundHours(windowHours * replicaEntries.length)
  const actualIdleHours = roundHours(
    replicaEntries.reduce((sum, replica) => sum + replica.idleHours, 0),
  )
  const actualWindowBurnUsd = roundUsd(
    replicaEntries.reduce(
      (sum, replica) => sum + replica.profile.hourlyComputeUsd * windowHours,
      0,
    ),
  )
  const actualActiveDemandBurnUsd = roundUsd(
    replicaEntries.reduce(
      (sum, replica) =>
        sum + replica.profile.hourlyComputeUsd * replica.activeServingHoursRaw,
      0,
    ),
  )
  const actualIdleBurnUsd = roundUsd(
    replicaEntries.reduce(
      (sum, replica) =>
        sum + replica.profile.hourlyComputeUsd * replica.idleHoursRaw,
      0,
    ),
  )

  type DemandBucket = {
    activeServingHours: number
    totalTokens: number
    usageEvents: number
  }
  const demandBuckets = new Map<string, DemandBucket>()
  for (const replica of replicaEntries) {
    for (const row of replica.demandRows) {
      const demandKind = normalizeDemandKind(row.demand_kind)
      const demandSource = optionalText(row.demand_source) ?? 'unknown'
      const demandClient = optionalText(row.demand_client) ?? 'unknown'
      const key = `${demandKind}:${demandSource}:${demandClient}`
      const bucket =
        demandBuckets.get(key) ??
        ({
          activeServingHours: 0,
          totalTokens: 0,
          usageEvents: 0,
        } satisfies DemandBucket)
      bucket.activeServingHours +=
        finiteNonNegative(row.total_wall_clock_ms) / 3_600_000
      bucket.totalTokens += Math.max(0, Math.trunc(row.total_tokens ?? 0))
      bucket.usageEvents += 1
      demandBuckets.set(key, bucket)
    }
  }

  const weightedDemandCost = (
    kind: 'external' | 'internal' | 'unlabeled',
  ): number =>
    roundUsd(
      replicaEntries.reduce(
        (sum, replica) =>
          sum +
          replica.demandRows.reduce((rowSum, row) => {
            if (normalizeDemandKind(row.demand_kind) !== kind) {
              return rowSum
            }
            return (
              rowSum +
              replica.profile.hourlyComputeUsd *
                (finiteNonNegative(row.total_wall_clock_ms) / 3_600_000)
            )
          }, 0),
        0,
      ),
    )

  const demand = [...demandBuckets.entries()]
    .map(([key, bucket]) => {
      const [demandKindRaw, demandSource, demandClient] = key.split(':')
      const demandKind = normalizeDemandKind(demandKindRaw)
      const activeServingHours = roundHours(bucket.activeServingHours)
      const activeDemandBurnUsd = roundUsd(
        replicaEntries.reduce(
          (sum, replica) =>
            sum +
            replica.demandRows.reduce((rowSum, row) => {
              const rowKind = normalizeDemandKind(row.demand_kind)
              const rowSource = optionalText(row.demand_source) ?? 'unknown'
              const rowClient = optionalText(row.demand_client) ?? 'unknown'
              if (
                rowKind !== demandKind ||
                rowSource !== demandSource ||
                rowClient !== demandClient
              ) {
                return rowSum
              }
              return (
                rowSum +
                replica.profile.hourlyComputeUsd *
                  (finiteNonNegative(row.total_wall_clock_ms) / 3_600_000)
              )
            }, 0),
          0,
        ),
      )
      return {
        activeDemandBurnUsd,
        activeServingHours,
        costPerServedTokenUsd: costPer(activeDemandBurnUsd, bucket.totalTokens),
        demandClient: demandClient ?? 'unknown',
        demandKind,
        demandSource: demandSource ?? 'unknown',
        key,
        label: `${demandKind} / ${demandSource ?? 'unknown'} / ${
          demandClient ?? 'unknown'
        }`,
        totalTokens: bucket.totalTokens,
        usageEvents: bucket.usageEvents,
      }
    })
    .sort((left, right) =>
      right.activeDemandBurnUsd === left.activeDemandBurnUsd
        ? left.key.localeCompare(right.key)
        : right.activeDemandBurnUsd - left.activeDemandBurnUsd,
    )

  const scenarioGroups = new Map<string, typeof replicaEntries>()
  for (const replica of replicaEntries) {
    const key = `${replica.profile.machineShape}:${replica.profile.gpuCount}`
    scenarioGroups.set(key, [...(scenarioGroups.get(key) ?? []), replica])
  }

  const scenarios = [...scenarioGroups.values()].flatMap(group => {
    const representative = group[0]!.profile
    const groupReplicaCount = group.length
    const groupActiveHours = roundHours(
      group.reduce((sum, replica) => sum + replica.activeServingHours, 0),
    )
    const groupActiveHoursRaw = group.reduce(
      (sum, replica) => sum + replica.activeServingHoursRaw,
      0,
    )
    const groupIdleHours = roundHours(
      group.reduce((sum, replica) => sum + replica.idleHours, 0),
    )
    const groupIdleHoursRaw = group.reduce(
      (sum, replica) => sum + replica.idleHoursRaw,
      0,
    )
    const groupUptimeHours = roundHours(windowHours * groupReplicaCount)
    const groupTokens = group.reduce(
      (sum, replica) => sum + replica.totalTokens,
      0,
    )
    const groupAcceptedOutcomes = acceptedOutcomesMeasured
      ? group.reduce((sum, replica) => sum + replica.acceptedOutcomes, 0)
      : NOT_MEASURED
    const groupInternalHours = group.reduce(
      (sum, replica) =>
        sum +
        replica.demandRows.reduce(
          (rowSum, row) =>
            normalizeDemandKind(row.demand_kind) === 'internal'
              ? rowSum + finiteNonNegative(row.total_wall_clock_ms) / 3_600_000
              : rowSum,
          0,
        ),
      0,
    )
    const groupExternalHours = group.reduce(
      (sum, replica) =>
        sum +
        replica.demandRows.reduce(
          (rowSum, row) =>
            normalizeDemandKind(row.demand_kind) === 'external'
              ? rowSum + finiteNonNegative(row.total_wall_clock_ms) / 3_600_000
              : rowSum,
          0,
        ),
      0,
    )
    const groupUnlabeledHours = group.reduce(
      (sum, replica) =>
        sum +
        replica.demandRows.reduce(
          (rowSum, row) =>
            normalizeDemandKind(row.demand_kind) === 'unlabeled'
              ? rowSum + finiteNonNegative(row.total_wall_clock_ms) / 3_600_000
              : rowSum,
          0,
        ),
      0,
    )

    return scenarioProfilesFor(representative).map(profile => {
      const hourlyBurnUsd = roundUsd(
        profile.hourlyComputeUsd * groupReplicaCount,
      )
      const windowBurnUsd = roundUsd(hourlyBurnUsd * windowHours)
      const activeDemandBurnUsd = roundUsd(
        profile.hourlyComputeUsd * groupActiveHoursRaw,
      )
      return {
        acceptedOutcomes: groupAcceptedOutcomes,
        activeDemandBurnUsd,
        activeServingHours: groupActiveHours,
        benchmarkReservedBurnUsd: NOT_MEASURED,
        costPerAcceptedOutcomeUsd:
          groupAcceptedOutcomes === NOT_MEASURED
            ? NOT_MEASURED
            : costPer(windowBurnUsd, groupAcceptedOutcomes),
        effectiveCostPerServedTokenUsd: costPer(windowBurnUsd, groupTokens),
        externalDemandBurnUsd: roundUsd(
          profile.hourlyComputeUsd * groupExternalHours,
        ),
        gpuCount: profile.gpuCount,
        hourlyBurnUsd,
        idleBurnUsd: roundUsd(profile.hourlyComputeUsd * groupIdleHoursRaw),
        idleHours: groupIdleHours,
        internalDemandBurnUsd: roundUsd(
          profile.hourlyComputeUsd * groupInternalHours,
        ),
        keepWarmBurnUsd: NOT_MEASURED,
        machineShape: profile.machineShape,
        monthlyComputeUsd: profile.monthlyComputeUsd * groupReplicaCount,
        profileRef: profile.profileRef,
        provisioningModel: profile.provisioningModel,
        replicaCount: groupReplicaCount,
        sourceRef: profile.sourceRef,
        storageOverheadUsd: NOT_MEASURED,
        unlabeledDemandBurnUsd: roundUsd(
          profile.hourlyComputeUsd * groupUnlabeledHours,
        ),
        uptimeHours: groupUptimeHours,
        windowBurnUsd,
      }
    })
  })

  const byReplicaRef = new Map<string, OwnedInferenceReplicaCostSummary>()
  for (const replica of replicaEntries) {
    byReplicaRef.set(replica.replicaRef, {
      effectiveCostPerServedTokenUsd: costPer(
        replica.profile.hourlyComputeUsd * windowHours,
        replica.totalTokens,
      ),
      idleHours: replica.idleHours,
      uptimeHours: windowHours,
    })
  }

  return {
    byReplicaRef,
    summary: {
      acceptedOutcomes,
      activeDemandBurnUsd: actualActiveDemandBurnUsd,
      activeServingHours: actualActiveHours,
      benchmarkReservedBurnUsd: NOT_MEASURED,
      blockerRefs: [...blockerRefs].sort(),
      costCoverage:
        profiles.length === 0 || actualHourlyBurnUsd <= 0
          ? 'not_measured'
          : 'partial',
      costPerAcceptedOutcomeUsd:
        acceptedOutcomes === NOT_MEASURED
          ? NOT_MEASURED
          : costPer(actualWindowBurnUsd, acceptedOutcomes),
      demand,
      effectiveCostPerServedTokenUsd: costPer(actualWindowBurnUsd, totalTokens),
      externalDemandBurnUsd: weightedDemandCost('external'),
      hourlyBurnUsd:
        actualHourlyBurnUsd <= 0 ? NOT_MEASURED : roundUsd(actualHourlyBurnUsd),
      idleBurnUsd: actualIdleBurnUsd,
      idleHours: actualIdleHours,
      internalDemandBurnUsd: weightedDemandCost('internal'),
      keepWarmBurnUsd: NOT_MEASURED,
      monthlyBurnUsd:
        actualMonthlyBurnUsd <= 0
          ? NOT_MEASURED
          : roundUsd(actualMonthlyBurnUsd),
      profiles,
      scenarios,
      storageOverheadUsd: NOT_MEASURED,
      unlabeledDemandBurnUsd: weightedDemandCost('unlabeled'),
      uptimeHours: actualUptimeHours,
      windowBurnUsd: actualWindowBurnUsd,
    },
  }
}
