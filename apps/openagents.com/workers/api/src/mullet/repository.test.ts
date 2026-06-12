import {
  MulletSimulationRunExport,
  decodeMulletScenario,
  decodeMulletSimulationRun,
} from '@openagentsinc/mullet-schema'
import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  MulletPersistenceValidationError,
  MulletRepository,
  type MulletRepositoryRuntime,
  MulletRunExportNotFound,
  MulletScenarioNotFound,
  MulletUnsafePersistencePayload,
} from './repository'

const timestamp = '2026-06-08T00:00:00.000Z'

type StoredScenarioRow = Record<string, unknown>
type StoredRunRow = Record<string, unknown>
type StoredHourlyResultRow = Record<string, unknown>
type StoredCandidateModeRow = Record<string, unknown>
type StoredExportRow = Record<string, unknown>

type MulletRepositoryStore = Readonly<{
  candidates: Array<StoredCandidateModeRow>
  exports: Array<StoredExportRow>
  hourlyResults: Array<StoredHourlyResultRow>
  runs: Array<StoredRunRow>
  scenarios: Array<StoredScenarioRow>
}>

const makeStore = (): MulletRepositoryStore => ({
  candidates: [],
  exports: [],
  hourlyResults: [],
  runs: [],
  scenarios: [],
})

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 1,
  duration: 0,
  last_row_id: 0,
  rows_read: 1,
  rows_written: 1,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

const jsonFixture = <T>(value: unknown): T => JSON.parse(JSON.stringify(value))

const makeRuntime = (): MulletRepositoryRuntime => {
  let index = 0

  return {
    makeId: prefix => {
      index += 1

      return `${prefix}_${String(index).padStart(4, '0')}`
    },
    nowIso: () => '2026-06-08T00:00:10.000Z',
  }
}

const makeMemoryD1 = (store: MulletRepositoryStore): D1Database => {
  const db: D1Database = {
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare: (query: string) => {
      let values: ReadonlyArray<unknown> = []

      function raw<T = unknown[]>(options: {
        columnNames: true
      }): Promise<[Array<string>, ...Array<T>]>
      function raw<T = unknown[]>(options?: {
        columnNames?: false
      }): Promise<Array<T>>
      function raw<T = unknown[]>(options?: {
        columnNames?: boolean
      }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
        return options?.columnNames === true
          ? Promise.resolve([[]])
          : Promise.resolve([])
      }

      const statement: D1PreparedStatement = {
        all: <T = Record<string, unknown>>() => {
          if (query.includes('FROM mullet_scenarios')) {
            return Promise.resolve(
              makeResult<T>(
                store.scenarios
                  .filter(row => row.owner_user_id === values[0])
                  .map(row => jsonFixture<T>(row)),
              ),
            )
          }

          if (query.includes('FROM mullet_run_candidate_modes')) {
            return Promise.resolve(
              makeResult<T>(
                store.candidates
                  .filter(
                    row =>
                      row.owner_user_id === values[0] &&
                      row.run_id === values[1],
                  )
                  .sort(
                    (left, right) =>
                      Number(left.hour_index) - Number(right.hour_index) ||
                      Number(left.candidate_index) -
                        Number(right.candidate_index),
                  )
                  .map(row => jsonFixture<T>(row)),
              ),
            )
          }

          return Promise.resolve(makeResult<T>())
        },
        bind: (...nextValues: ReadonlyArray<unknown>) => {
          values = nextValues

          return statement
        },
        first: <T = Record<string, unknown>>() => {
          if (query.includes('FROM mullet_scenarios')) {
            const row = store.scenarios.find(
              scenarioRow =>
                scenarioRow.owner_user_id === values[0] &&
                scenarioRow.id === values[1],
            )

            return Promise.resolve(
              row === undefined ? null : jsonFixture<T>(row),
            )
          }

          if (query.includes('FROM mullet_simulation_runs')) {
            const row = store.runs.find(
              runRow =>
                runRow.owner_user_id === values[0] && runRow.id === values[1],
            )

            return Promise.resolve(
              row === undefined ? null : jsonFixture<T>(row),
            )
          }

          if (query.includes('FROM mullet_run_exports')) {
            const row = store.exports
              .filter(
                exportRow =>
                  exportRow.owner_user_id === values[0] &&
                  exportRow.run_id === values[1],
              )
              .sort((left, right) =>
                String(right.created_at).localeCompare(String(left.created_at)),
              )[0]

            return Promise.resolve(
              row === undefined ? null : jsonFixture<T>(row),
            )
          }

          return Promise.resolve(null)
        },
        raw,
        run: <T = Record<string, unknown>>() => {
          if (query.includes('INSERT INTO mullet_scenarios')) {
            store.scenarios.push({
              id: values[0],
              owner_user_id: values[1],
              owner_email: values[2],
              schema_version: values[3],
              name: values[4],
              kind: values[5],
              scenario_json: values[6],
              source_refs_json: values[7],
              provenance_summary_json: values[8],
              visibility: 'private',
              export_redaction_state: 'not_checked',
              created_at: values[9],
              updated_at: values[10],
              deleted_at: null,
            })
          } else if (query.includes('INSERT INTO mullet_simulation_runs')) {
            store.runs.push({
              id: values[0],
              scenario_id: values[1],
              owner_user_id: values[2],
              owner_email: values[3],
              schema_version: values[4],
              status: values[5],
              run_json: values[6],
              source_refs_json: values[7],
              provenance_summary_json: values[8],
              provider_settlement_state: values[9],
              power_data_state: values[10],
              visibility: 'private',
              export_redaction_state: 'not_checked',
              created_at: values[11],
              updated_at: values[12],
              completed_at: values[13],
              deleted_at: null,
            })
          } else if (query.includes('INSERT INTO mullet_run_hourly_results')) {
            store.hourlyResults.push({
              id: values[0],
              run_id: values[1],
              scenario_id: values[2],
              owner_user_id: values[3],
              hour_index: values[4],
              timestamp: values[5],
              selected_mode: values[6],
              reason_code: values[7],
              energy_mwh: values[8],
              result_json: values[9],
              created_at: values[10],
            })
          } else if (query.includes('INSERT INTO mullet_run_candidate_modes')) {
            store.candidates.push({
              id: values[0],
              run_id: values[1],
              hourly_result_id: values[2],
              scenario_id: values[3],
              owner_user_id: values[4],
              hour_index: values[5],
              candidate_index: values[6],
              timestamp: values[7],
              mode: values[8],
              reason_code: values[9],
              risk_adjusted_net_usd_per_mwh: values[10],
              clears_readiness: values[11],
              clears_demand: values[12],
              clears_provider_floor: values[13],
              candidate_json: values[14],
              created_at: values[15],
            })
          } else if (query.includes('INSERT INTO mullet_run_exports')) {
            store.exports.push({
              id: values[0],
              run_id: values[1],
              scenario_id: values[2],
              owner_user_id: values[3],
              owner_email: values[4],
              schema_version: values[5],
              format: values[6],
              export_json: values[7],
              private_visibility: 1,
              redaction_status: values[8],
              content_ref: values[9],
              created_at: values[10],
            })
          }

          return Promise.resolve(makeResult<T>())
        },
      }

      return statement
    },
    withSession: () => ({
      batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
        Promise.all(statements.map(statement => statement.run<T>())),
      getBookmark: () => null,
      prepare: query => db.prepare(query),
    }),
  }

  return db
}

const runWithRepository = <A>(
  db: D1Database,
  runtime: MulletRepositoryRuntime,
  effect: Effect.Effect<A, unknown, MulletRepository>,
): Promise<A> =>
  Effect.runPromise(
    effect.pipe(
      Effect.provide(MulletRepository.layer({ OPENAGENTS_DB: db }, runtime)),
    ),
  )

const sourceRef = {
  id: 'source_mullet_repository_fixture',
  label: 'Mullet repository fixture',
  uri: 'workers/api/src/mullet/repository.test.ts',
  capturedAt: timestamp,
}

const provenancedNumber = (value: number, unit: string) => ({
  value,
  unit,
  provenance: 'modeled',
  confidence: 0.72,
  source: sourceRef,
  lastUpdated: timestamp,
  needsDiligence: true,
})

const provenancedString = (value: string) => ({
  value,
  unit: 'text',
  provenance: 'modeled',
  confidence: 0.72,
  source: sourceRef,
  lastUpdated: timestamp,
  needsDiligence: true,
})

const scenarioFixture = (notes?: string) =>
  decodeMulletScenario({
    id: 'mullet_scenario_repo_fixture',
    name: 'Repository fixture scenario',
    schemaVersion: '2026-06-08.v1',
    kind: 'tinybox_shc_power',
    createdAt: timestamp,
    updatedAt: timestamp,
    dateRange: {
      startAt: timestamp,
      endAt: '2026-06-09T00:00:00.000Z',
    },
    facility: {
      id: 'facility_repo_fixture',
      siteId: 'site_repo_fixture',
      name: 'Repository fixture site',
      market: 'ERCOT',
      zone: 'LZ_WEST',
      capacityMw: provenancedNumber(1, 'megawatts'),
      powerContractType: 'fixed_fixture',
      fixedPriceUsdPerMwh: provenancedNumber(45, 'usd_per_mwh'),
      maxAiAllocationMw: provenancedNumber(0.002, 'megawatts'),
      curtailmentPolicy: 'manual',
      gridServiceTerms: 'none',
      siteOpsCostUsdPerMwh: provenancedNumber(5, 'usd_per_mwh'),
      coolingMultiplier: provenancedNumber(1.1, 'score'),
      remoteHandsMonthlyUsd: provenancedNumber(100, 'usd'),
      physicalReadiness: {
        interconnectionStatus: provenancedString('fixture'),
        energizedCapacityMw: provenancedNumber(1, 'megawatts'),
        transformerStatus: provenancedString('fixture'),
        switchgearStatus: provenancedString('fixture'),
        coolingCapacityKw: provenancedNumber(20, 'kilowatts'),
        pue: provenancedNumber(1.2, 'score'),
        fiberOrNetworkStatus: provenancedString('fixture'),
        redundancyTier: provenancedString('fixture'),
        remoteHandsSla: provenancedString('fixture'),
        uptimeCommitment: provenancedNumber(0.95, 'percent'),
        liquidatedDamagesExposureUsd: provenancedNumber(0, 'usd'),
        permittingOrCommunityRisk: provenancedString('fixture'),
      },
      customerSlaReserveUsdPerMwh: provenancedNumber(25, 'usd_per_mwh'),
      siteClassification: 'mining_led_ai_pilot_not_mullet',
      readinessState: 'benchmark_passed',
      capacityLifecycleState: 'eligible',
    },
    miningFleet: {
      asicModel: 'S21 fixture',
      count: 0,
      wattsPerUnit: 3500,
      thPerUnit: 200,
      joulesPerTh: 17.5,
      capexPerUnitUsd: 4000,
      depreciationMonths: 36,
      poolFeePercent: 0.02,
      firmwareOpsCostUsdPerMwh: 2,
    },
    computeNodes: [
      {
        nodeId: 'node_repo_fixture',
        nodeType: 'tinybox_red_v2',
        ownerParty: 'openagents',
        operatorParty: 'fixture_provider',
        siteId: 'site_repo_fixture',
        gpuModel: 'AMD 9070 XT',
        gpuCount: 4,
        vramGb: 64,
        interconnect: 'pcie',
        cpu: 'fixture',
        ramGb: 128,
        storageGb: 2000,
        networkGbps: 1,
        capexUsd: 12000,
        depreciationMonths: 36,
        idlePowerKw: 0.35,
        loadPowerKw: 1.6,
        powerLimitKw: 1.6,
        supportMonthlyUsd: 150,
        fallbackMarketEligible: true,
        trustTier: 'pilot',
        readinessState: 'benchmark_passed',
        capacityLifecycleState: 'admitted',
        workloadFit: ['work_class_repo_fixture'],
      },
    ],
    runtimeBenchmarks: [
      {
        nodeId: 'node_repo_fixture',
        workClassId: 'work_class_repo_fixture',
        modelId: 'fixture-local-model',
        framework: 'fixture',
        precision: 'fp16',
        batchSize: 1,
        attemptsPerInstanceHour: 12,
        tokensPerSecond: 80,
        joulesPerToken: 10,
        kwhPerAttempt: 0.08,
        wallSecondsPerAttempt: 300,
        observedFailureRate: 0.08,
        source: sourceRef,
        confidence: 0.72,
      },
    ],
    workClasses: [
      {
        id: 'work_class_repo_fixture',
        label: 'Repository fixture work',
        latencyClass: 'batch',
        buyerPriceUsd: 2.5,
        acceptanceRate: 0.9,
        targetMargin: 0.25,
        riskReserveUsd: 0.15,
        frontierInputTokens: 1000,
        frontierOutputTokens: 200,
        frontierInputPriceUsdPerMillion: 1.25,
        frontierOutputPriceUsdPerMillion: 10,
        cheapModelCostUsd: 0.05,
        workroomRuntimeHours: 0.1,
        workroomHourlyCostUsd: 0.25,
        providerComputeHours: 0.1,
        providerPowerKw: 1.6,
        validatorCount: 2,
        validatorPayoutEachUsd: 0.05,
        graderCostUsd: 0.05,
        humanReviewMinutes: 0,
        humanReviewHourlyCostUsd: 60,
        artifactStorageCostUsd: 0.01,
        settlementCostUsd: 0.01,
        supportOverheadUsd: 0.03,
        retryOrFailureCostUsd: 0.04,
        flexibility: {
          canPause: true,
          canResume: true,
          canMigrate: true,
          checkpointIntervalMinutes: 10,
          maxDelayMinutes: 1440,
          deadlineMinutes: 1440,
          customerImpactIfDelayed: 'low',
          privacyTier: 'public_safe',
          stateLocality: 'portable',
          requiredTools: ['artifact-store'],
        },
        demandBacklog: 100,
        eligibleNodeTypes: ['tinybox_red_v2'],
        minimumTrustTier: 'pilot',
      },
    ],
    providerPolicies: [
      {
        nodeType: 'tinybox_red_v2',
        providerMinimumBidUsdPerHour: 1,
        providerMinimumBidUsdPerJob: 0.1,
        wearUsdPerKwh: 0.05,
        bandwidthUsdPerJob: 0.01,
        desiredProfitUsdPerJob: 0.1,
        operatorMarginUsdPerMwh: 25,
        miningFloorPolicy: 'modeled',
        rawGpuFloorPolicy: 'modeled',
        vpsColocationFloorPolicy: 'not_applicable',
        curtailmentGridServiceFloorPolicy: 'none',
      },
    ],
    partySplit: {
      buyerPaysUsd: 2.5,
      openagentsKeepsUsd: 0.7,
      providerReceivesUsd: 1.2,
      facilityOperatorReceivesUsd: 0.2,
      hardwareOwnerReceivesUsd: 0.3,
      validatorsReceiveUsd: 0.1,
      reviewersReceiveUsd: 0,
      settlementCostUsd: 0.01,
      riskReserveUsd: 0.15,
    },
    capitalAssumptions: {
      hardwareCapexUsd: provenancedNumber(12000, 'usd'),
      depreciationMonths: provenancedNumber(36, 'months'),
      discountRatePercent: provenancedNumber(12, 'percent'),
      debtServiceMonthlyUsd: provenancedNumber(0, 'usd'),
      residualValueUsd: provenancedNumber(4000, 'usd'),
    },
    demandAssumptions: {
      acceptedWorkBacklog: provenancedNumber(100, 'outcomes'),
      demandFillPercent: provenancedNumber(0.55, 'percent'),
      rawGpuMarketFillPercent: provenancedNumber(0.35, 'percent'),
      tokenApiFillPercent: provenancedNumber(0.1, 'percent'),
    },
    sourceRefs: [sourceRef],
    ...(notes === undefined ? {} : { notes }),
  })

const candidateFixture = (mode: 'mine' | 'openagents_accepted_work') => ({
  timestamp,
  mode,
  buyerRevenueUsd: mode === 'mine' ? 100 : 12.5,
  providerPayoutUsd: mode === 'mine' ? 0 : 6,
  openagentsMarginUsd: mode === 'mine' ? 45 : 4.5,
  providerNetUsdPerMwh: mode === 'mine' ? 0 : 3750,
  acceptedOutcomes: mode === 'mine' ? 0 : 5,
  acceptedOutcomesPerMwh: mode === 'mine' ? 0 : 3125,
  energyMwh: 0.0016,
  riskAdjustedNetUsdPerMwh: mode === 'mine' ? 45 : 2812.5,
  clearsReadiness: true,
  clearsDemand: true,
  clearsProviderFloor: true,
  reasonCode:
    mode === 'mine'
      ? 'mining_best_available'
      : 'accepted_work_clears_all_gates',
})

const simulationRunFixture = () => {
  const scenario = scenarioFixture()

  return decodeMulletSimulationRun({
    id: 'mullet_run_repo_fixture',
    scenarioId: scenario.id,
    ownerUserId: 'user_chris',
    ownerEmail: 'chris@openagents.com',
    status: 'succeeded',
    createdAt: timestamp,
    updatedAt: timestamp,
    completedAt: timestamp,
    scenario,
    dispatchResults: [
      {
        timestamp,
        effectivePriceUsdPerMwh: 45,
        selectedMode: 'openagents_accepted_work',
        candidates: [
          candidateFixture('mine'),
          candidateFixture('openagents_accepted_work'),
        ],
        miningRevenueUsd: 100,
        miningProfitUsd: 45,
        rawGpuRevenueUsd: 0,
        tokenApiRevenueUsd: 0,
        acceptedWorkBuyerRevenueUsd: 12.5,
        acceptedWorkProviderPayoutUsd: 6,
        acceptedWorkOpenagentsMarginUsd: 4.5,
        acceptedOutcomes: 5,
        acceptedOutcomesPerMwh: 3125,
        proofPacketIds: [],
        marketMemoryUpdateIds: [],
        energyTelemetryRecordIds: [],
        energyMwh: 0.0016,
        curtailedMw: 0,
        idleMw: 0,
        reasonCode: 'accepted_work_clears_all_gates',
        provenance: 'modeled',
        confidence: 0.72,
      },
    ],
    capitalReturns: [],
    proofPackets: [],
    marketMemory: [],
    energyTelemetry: [],
    providerSettlementState: 'not_payable',
    powerDataState: 'modeled',
  })
}

const runExportFixture = (run: ReturnType<typeof simulationRunFixture>) =>
  S.decodeUnknownSync(MulletSimulationRunExport)({
    runId: run.id,
    scenarioId: run.scenarioId,
    generatedAt: timestamp,
    format: 'json',
    privateVisibility: true,
    redactionStatus: 'not_checked',
    modeledValueCount: 1,
    measuredValueCount: 0,
    acceptedValueCount: 0,
    paidValueCount: 0,
    settledValueCount: 0,
    contentRef: 'artifact.mullet.private.repo_fixture',
  })

describe('MulletRepository', () => {
  test('creates, lists, and gets private scenarios', async () => {
    const store = makeStore()
    const db = makeMemoryD1(store)
    const runtime = makeRuntime()
    const created = await runWithRepository(
      db,
      runtime,
      Effect.gen(function* () {
        const repository = yield* MulletRepository

        return yield* repository.createScenario({
          ownerEmail: 'chris@openagents.com',
          ownerUserId: 'user_chris',
          scenario: scenarioFixture(),
        })
      }),
    )
    const listed = await runWithRepository(
      db,
      runtime,
      Effect.gen(function* () {
        const repository = yield* MulletRepository

        return yield* repository.listScenarios('user_chris')
      }),
    )
    const found = await runWithRepository(
      db,
      runtime,
      Effect.gen(function* () {
        const repository = yield* MulletRepository

        return yield* repository.getScenario(
          'user_chris',
          'mullet_scenario_repo_fixture',
        )
      }),
    )

    expect(created.visibility).toBe('private')
    expect(created.exportRedactionState).toBe('not_checked')
    expect(created.provenanceSummary.modeledValueCount).toBeGreaterThan(0)
    expect(listed).toHaveLength(1)
    expect(found.scenario.name).toBe('Repository fixture scenario')
    expect(store.scenarios[0]?.scenario_json).toEqual(expect.any(String))
  })

  test('returns tagged not-found errors', async () => {
    const store = makeStore()
    const db = makeMemoryD1(store)
    const runtime = makeRuntime()

    await expect(
      runWithRepository(
        db,
        runtime,
        Effect.gen(function* () {
          const repository = yield* MulletRepository

          return yield* repository.getScenario('user_chris', 'missing')
        }),
      ),
    ).rejects.toBeInstanceOf(MulletScenarioNotFound)

    await expect(
      runWithRepository(
        db,
        runtime,
        Effect.gen(function* () {
          const repository = yield* MulletRepository

          return yield* repository.getLatestRunExport('user_chris', 'missing')
        }),
      ),
    ).rejects.toBeInstanceOf(MulletRunExportNotFound)
  })

  test('persists simulation runs and candidate modes', async () => {
    const store = makeStore()
    const db = makeMemoryD1(store)
    const runtime = makeRuntime()
    const run = simulationRunFixture()

    const created = await runWithRepository(
      db,
      runtime,
      Effect.gen(function* () {
        const repository = yield* MulletRepository
        yield* repository.createScenario({
          ownerEmail: 'chris@openagents.com',
          ownerUserId: 'user_chris',
          scenario: run.scenario,
        })

        return yield* repository.createSimulationRun({ run })
      }),
    )
    const found = await runWithRepository(
      db,
      runtime,
      Effect.gen(function* () {
        const repository = yield* MulletRepository

        return yield* repository.getSimulationRun(
          'user_chris',
          'mullet_run_repo_fixture',
        )
      }),
    )
    const candidates = await runWithRepository(
      db,
      runtime,
      Effect.gen(function* () {
        const repository = yield* MulletRepository

        return yield* repository.listRunCandidateModes(
          'user_chris',
          'mullet_run_repo_fixture',
        )
      }),
    )

    expect(created.run.status).toBe('succeeded')
    expect(found.run.dispatchResults[0]?.selectedMode).toBe(
      'openagents_accepted_work',
    )
    expect(store.hourlyResults).toHaveLength(1)
    expect(candidates).toHaveLength(2)
    expect(candidates.map(candidate => candidate.candidate.reasonCode)).toEqual(
      ['mining_best_available', 'accepted_work_clears_all_gates'],
    )
  })

  test('persists private run export metadata rows', async () => {
    const store = makeStore()
    const db = makeMemoryD1(store)
    const runtime = makeRuntime()
    const run = simulationRunFixture()
    const exportRecord = await runWithRepository(
      db,
      runtime,
      Effect.gen(function* () {
        const repository = yield* MulletRepository
        yield* repository.createScenario({
          ownerEmail: 'chris@openagents.com',
          ownerUserId: 'user_chris',
          scenario: run.scenario,
        })
        yield* repository.createSimulationRun({ run })
        yield* repository.createRunExport({
          exportId: 'mullet_export_repo_fixture',
          ownerEmail: 'chris@openagents.com',
          ownerUserId: 'user_chris',
          runExport: runExportFixture(run),
          schemaVersion: run.scenario.schemaVersion,
        })

        return yield* repository.getLatestRunExport(
          'user_chris',
          'mullet_run_repo_fixture',
        )
      }),
    )

    expect(exportRecord.id).toBe('mullet_export_repo_fixture')
    expect(exportRecord.runExport.privateVisibility).toBe(true)
    expect(store.exports[0]?.private_visibility).toBe(1)
  })

  test('decodes stored payloads through named schema boundaries', async () => {
    const store = makeStore()
    const db = makeMemoryD1(store)
    const runtime = makeRuntime()

    await runWithRepository(
      db,
      runtime,
      Effect.gen(function* () {
        const repository = yield* MulletRepository

        return yield* repository.createScenario({
          ownerEmail: 'chris@openagents.com',
          ownerUserId: 'user_chris',
          scenario: scenarioFixture(),
        })
      }),
    )

    store.scenarios[0] = {
      ...store.scenarios[0],
      scenario_json: '{"not":"a mullet scenario"}',
    }

    await expect(
      runWithRepository(
        db,
        runtime,
        Effect.gen(function* () {
          const repository = yield* MulletRepository

          return yield* repository.getScenario(
            'user_chris',
            'mullet_scenario_repo_fixture',
          )
        }),
      ),
    ).rejects.toBeInstanceOf(MulletPersistenceValidationError)
  })

  test('rejects secret-shaped payloads before persistence', async () => {
    const store = makeStore()
    const db = makeMemoryD1(store)
    const runtime = makeRuntime()

    await expect(
      runWithRepository(
        db,
        runtime,
        Effect.gen(function* () {
          const repository = yield* MulletRepository

          return yield* repository.createScenario({
            ownerEmail: 'chris@openagents.com',
            ownerUserId: 'user_chris',
            scenario: scenarioFixture('sk_live_do_not_store'),
          })
        }),
      ),
    ).rejects.toBeInstanceOf(MulletUnsafePersistencePayload)
    expect(store.scenarios).toHaveLength(0)
  })
})
