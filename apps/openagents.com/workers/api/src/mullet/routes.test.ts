import { Effect, Layer } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  MulletRepository,
  type MulletRepositoryShape,
  MulletRunExportNotFound,
  type MulletRunExportRecord,
  MulletScenarioNotFound,
  type MulletScenarioRecord,
  MulletSimulationRunNotFound,
  type MulletSimulationRunRecord,
} from './repository'
import { makeMulletRoutes } from './routes'
import { scenarioFixture } from './test-fixtures.test-support'

type TestSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

const makeExecutionContext = (): ExecutionContext =>
  ({
    passThroughOnException: () => undefined,
    waitUntil: () => undefined,
  }) as unknown as ExecutionContext

const emptyProvenanceSummary = {
  acceptedValueCount: 0,
  measuredValueCount: 0,
  modeledValueCount: 1,
  needsDiligenceCount: 0,
  paidValueCount: 0,
  settledValueCount: 0,
  sourceRefCount: 1,
}

const makeMemoryRepository = (): MulletRepositoryShape => {
  const exports = new Map<string, MulletRunExportRecord>()
  const scenarios = new Map<string, MulletScenarioRecord>()
  const runs = new Map<string, MulletSimulationRunRecord>()

  return {
    createScenario: input => {
      const record = {
        id: input.scenario.id,
        ownerEmail: input.ownerEmail,
        ownerUserId: input.ownerUserId,
        scenario: input.scenario,
        schemaVersion: input.scenario.schemaVersion,
        sourceRefs: input.scenario.sourceRefs,
        provenanceSummary: emptyProvenanceSummary,
        visibility: 'private' as const,
        exportRedactionState: 'not_checked' as const,
        createdAt: input.scenario.createdAt,
        updatedAt: input.scenario.updatedAt,
      }
      scenarios.set(`${input.ownerUserId}:${input.scenario.id}`, record)

      return Effect.succeed(record)
    },
    createRunExport: input => {
      const exportId = input.exportId ?? 'mullet_export_route_fixture'
      const record = {
        id: exportId,
        ownerEmail: input.ownerEmail,
        ownerUserId: input.ownerUserId,
        runExport: input.runExport,
        runId: input.runExport.runId,
        scenarioId: input.runExport.scenarioId,
        schemaVersion: input.schemaVersion,
        createdAt: input.runExport.generatedAt,
      }
      exports.set(`${input.ownerUserId}:${input.runExport.runId}`, record)

      return Effect.succeed(record)
    },
    createSimulationRun: input => {
      const record = {
        id: input.run.id,
        ownerEmail: input.run.ownerEmail,
        ownerUserId: input.run.ownerUserId,
        run: input.run,
        scenarioId: input.run.scenarioId,
        schemaVersion: input.run.scenario.schemaVersion,
        sourceRefs: input.run.scenario.sourceRefs,
        provenanceSummary: emptyProvenanceSummary,
        visibility: 'private' as const,
        exportRedactionState: 'not_checked' as const,
        createdAt: input.run.createdAt,
        updatedAt: input.run.updatedAt,
        completedAt: input.run.completedAt ?? null,
      }
      runs.set(`${input.run.ownerUserId}:${input.run.id}`, record)

      return Effect.succeed(record)
    },
    getLatestRunExport: (ownerUserId, runId) => {
      const exportRecord = exports.get(`${ownerUserId}:${runId}`)

      return exportRecord === undefined
        ? Effect.fail(
            new MulletRunExportNotFound({
              ownerUserId,
              runId,
            }),
          )
        : Effect.succeed(exportRecord)
    },
    getScenario: (ownerUserId, scenarioId) => {
      const scenario = scenarios.get(`${ownerUserId}:${scenarioId}`)

      return scenario === undefined
        ? Effect.fail(
            new MulletScenarioNotFound({
              ownerUserId,
              scenarioId,
            }),
          )
        : Effect.succeed(scenario)
    },
    getSimulationRun: (ownerUserId, runId) => {
      const run = runs.get(`${ownerUserId}:${runId}`)

      return run === undefined
        ? Effect.fail(
            new MulletSimulationRunNotFound({
              ownerUserId,
              runId,
            }),
          )
        : Effect.succeed(run)
    },
    listRunCandidateModes: () => Effect.succeed([]),
    listScenarios: ownerUserId =>
      Effect.succeed(
        Array.from(scenarios.values()).filter(
          scenario => scenario.ownerUserId === ownerUserId,
        ),
      ),
  }
}

const makeRoutes = (session: TestSession | undefined) => {
  const repository = makeMemoryRepository()
  const routes = makeMulletRoutes({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-test-session-refreshed', 'true')

      return response
    },
    isOpenAgentsAdminEmail: email => email === 'chris@openagents.com',
    repositoryLayer: () => Layer.succeed(MulletRepository, repository),
    requireBrowserSession: async () => session,
    runtime: {
      makeId: prefix => `${prefix}_route_fixture`,
      nowIso: () => '2026-06-08T00:05:00.000Z',
    },
  })

  return { repository, routes }
}

const env = { OPENAGENTS_DB: {} as D1Database }

const runRoute = async (
  session: TestSession | undefined,
  request: Request,
): Promise<Response> => {
  const { routes } = makeRoutes(session)
  const routed = routes.routeMulletRequest(request, env, makeExecutionContext())

  if (routed === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(routed)
}

const readJson = async (response: Response): Promise<Record<string, unknown>> =>
  (await response.json()) as Record<string, unknown>

const chrisSession: TestSession = {
  user: {
    email: 'chris@openagents.com',
    userId: 'user_chris',
  },
}

const proofPacketFixture = {
  id: 'proof_packet_route_1',
  workId: 'work_route_1',
  workClassId: 'work_class_repo_fixture',
  nodeId: 'node_repo_fixture',
  nodeCapabilitySnapshotRef: 'node_capability_route_1',
  assignmentId: 'assignment_route_1',
  executionArtifactRef: 'execution_artifact_route_1',
  validatorVerdictRef: 'validator_verdict_route_1',
  acceptedCloseoutRef: 'accepted_closeout_route_1',
  buyerPriceUsd: 2.5,
  providerPayoutUsd: 1.2,
  settlementReceiptRef: 'settlement_receipt_route_1',
  routingConsequence: 'attached_existing_ref',
  provenance: 'accepted',
}

const energyTelemetryFixture = {
  id: 'energy_telemetry_route_1',
  timestamp: '2026-06-08T00:00:00.000Z',
  siteId: 'site_repo_fixture',
  nodeId: 'node_repo_fixture',
  workId: 'work_route_1',
  powerKw: 1.6,
  energyKwh: 1.6,
  powerDataState: 'measured',
  gridSignal: 'none',
  curtailmentOrShiftAction: 'none',
  priceCounterfactual: 'modeled',
  emissionsCounterfactual: 'modeled',
  customerImpact: 'none',
  payoutUsd: 6,
  marginUsd: 4.5,
  provenance: 'measured',
}

const marketMemoryFixture = {
  id: 'market_memory_route_1',
  nodeId: 'node_repo_fixture',
  siteId: 'site_repo_fixture',
  workClassId: 'work_class_repo_fixture',
  acceptedCount: 5,
  rejectedCount: 1,
  acceptanceProbability: 0.83,
  medianRuntimeSeconds: 300,
  medianPayoutSeconds: 120,
  payoutSuccessRate: 1,
  repeatProviderScore: 0.8,
  repeatBuyerScore: 0.7,
  validatorReliabilityScore: 0.9,
  commonFailureModes: [],
  lastUpdated: '2026-06-08T00:00:00.000Z',
}

describe('mullet routes', () => {
  test('returns 401 without a browser session', async () => {
    const response = await runRoute(
      undefined,
      new Request('https://openagents.com/api/mullet/bootstrap'),
    )

    expect(response.status).toBe(401)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(await readJson(response)).toEqual({ error: 'unauthorized' })
  })

  test('returns 403 for non-authorized emails', async () => {
    const response = await runRoute(
      {
        user: {
          email: 'other@openagents.com',
          userId: 'user_other',
        },
      },
      new Request('https://openagents.com/api/mullet/bootstrap'),
    )

    expect(response.status).toBe(403)
    expect(await readJson(response)).toEqual({ error: 'forbidden' })
  })

  test('returns bootstrap data for chris@openagents.com', async () => {
    const response = await runRoute(
      chrisSession,
      new Request('https://openagents.com/api/mullet/bootstrap'),
    )
    const body = await readJson(response)

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(response.headers.get('x-test-session-refreshed')).toBe('true')
    expect(body.access).toEqual({
      operatorEmail: 'chris@openagents.com',
      visibility: 'private',
    })
    expect(body.authorityBoundary).toMatchObject({
      canAssignLiveWork: false,
      canSpendWalletFunds: false,
    })
  })

  test('rejects invalid scenario payloads', async () => {
    const response = await runRoute(
      chrisSession,
      new Request('https://openagents.com/api/mullet/scenarios', {
        body: JSON.stringify({
          scenario: {
            id: 'mullet_scenario_invalid',
          },
        }),
        method: 'POST',
      }),
    )

    expect(response.status).toBe(400)
    expect(await readJson(response)).toEqual({
      error: 'invalid_mullet_request',
    })
  })

  test('creates a run from a scenario and reads it back', async () => {
    const { routes } = makeRoutes(chrisSession)
    const ctx = makeExecutionContext()
    const createScenario = routes.routeMulletRequest(
      new Request('https://openagents.com/api/mullet/scenarios', {
        body: JSON.stringify({ scenario: scenarioFixture() }),
        method: 'POST',
      }),
      env,
      ctx,
    )

    if (createScenario === undefined) {
      throw new Error('create scenario route did not match')
    }

    const createScenarioResponse = await Effect.runPromise(createScenario)
    expect(createScenarioResponse.status).toBe(201)

    const createRun = routes.routeMulletRequest(
      new Request('https://openagents.com/api/mullet/runs', {
        body: JSON.stringify({ scenarioId: 'mullet_scenario_repo_fixture' }),
        method: 'POST',
      }),
      env,
      ctx,
    )

    if (createRun === undefined) {
      throw new Error('create run route did not match')
    }

    const createRunResponse = await Effect.runPromise(createRun)
    const createRunBody = await readJson(createRunResponse)
    const runRecord = createRunBody.run as {
      id: string
      run: {
        dispatchResults: Array<{
          reasonCode: string
          selectedMode: string
        }>
        providerSettlementState: string
      }
    }

    expect(createRunResponse.status).toBe(201)
    expect(runRecord.id).toBe('mullet_run_route_fixture')
    expect(runRecord.run.providerSettlementState).toBe('not_payable')
    expect(runRecord.run.dispatchResults[0]).toMatchObject({
      reasonCode: 'accepted_work_clears_all_gates',
      selectedMode: 'openagents_accepted_work',
    })

    const getRun = routes.routeMulletRequest(
      new Request(`https://openagents.com/api/mullet/runs/${runRecord.id}`),
      env,
      ctx,
    )

    if (getRun === undefined) {
      throw new Error('get run route did not match')
    }

    const getRunResponse = await Effect.runPromise(getRun)
    const getRunBody = await readJson(getRunResponse)

    expect(getRunResponse.status).toBe(200)
    expect(getRunBody.run).toMatchObject({
      id: 'mullet_run_route_fixture',
      ownerEmail: 'chris@openagents.com',
      ownerUserId: 'user_chris',
      visibility: 'private',
    })
  })

  test('attaches optional proof, telemetry, settlement, and market-memory refs to runs', async () => {
    const { routes } = makeRoutes(chrisSession)
    const ctx = makeExecutionContext()
    const createScenario = routes.routeMulletRequest(
      new Request('https://openagents.com/api/mullet/scenarios', {
        body: JSON.stringify({ scenario: scenarioFixture() }),
        method: 'POST',
      }),
      env,
      ctx,
    )

    if (createScenario === undefined) {
      throw new Error('create scenario route did not match')
    }

    await Effect.runPromise(createScenario)

    const createRun = routes.routeMulletRequest(
      new Request('https://openagents.com/api/mullet/runs', {
        body: JSON.stringify({
          energyTelemetry: [energyTelemetryFixture],
          marketMemory: [marketMemoryFixture],
          powerDataState: 'measured',
          proofPackets: [proofPacketFixture],
          providerSettlementState: 'settled_bitcoin',
          scenarioId: 'mullet_scenario_repo_fixture',
        }),
        method: 'POST',
      }),
      env,
      ctx,
    )

    if (createRun === undefined) {
      throw new Error('create run route did not match')
    }

    const response = await Effect.runPromise(createRun)
    const body = await readJson(response)
    const runRecord = body.run as {
      run: {
        dispatchResults: Array<{
          energyTelemetryRecordIds: Array<string>
          marketMemoryUpdateIds: Array<string>
          proofPacketIds: Array<string>
        }>
        energyTelemetry: Array<{ id: string }>
        marketMemory: Array<{ id: string }>
        proofPackets: Array<{ id: string; settlementReceiptRef: string }>
        providerSettlementState: string
        powerDataState: string
      }
    }

    expect(response.status).toBe(201)
    expect(runRecord.run.proofPackets).toEqual([
      expect.objectContaining({
        id: 'proof_packet_route_1',
        settlementReceiptRef: 'settlement_receipt_route_1',
      }),
    ])
    expect(runRecord.run.energyTelemetry).toEqual([
      expect.objectContaining({ id: 'energy_telemetry_route_1' }),
    ])
    expect(runRecord.run.marketMemory).toEqual([
      expect.objectContaining({ id: 'market_memory_route_1' }),
    ])
    expect(runRecord.run.dispatchResults[0]).toMatchObject({
      energyTelemetryRecordIds: ['energy_telemetry_route_1'],
      marketMemoryUpdateIds: ['market_memory_route_1'],
      proofPacketIds: ['proof_packet_route_1'],
    })
    expect(runRecord.run.providerSettlementState).toBe('settled_bitcoin')
    expect(runRecord.run.powerDataState).toBe('measured')
  })

  test('creates private Markdown exports and reads latest export metadata', async () => {
    const { routes } = makeRoutes(chrisSession)
    const ctx = makeExecutionContext()
    const createScenario = routes.routeMulletRequest(
      new Request('https://openagents.com/api/mullet/scenarios', {
        body: JSON.stringify({ scenario: scenarioFixture() }),
        method: 'POST',
      }),
      env,
      ctx,
    )

    if (createScenario === undefined) {
      throw new Error('create scenario route did not match')
    }

    await Effect.runPromise(createScenario)

    const createRun = routes.routeMulletRequest(
      new Request('https://openagents.com/api/mullet/runs', {
        body: JSON.stringify({ scenarioId: 'mullet_scenario_repo_fixture' }),
        method: 'POST',
      }),
      env,
      ctx,
    )

    if (createRun === undefined) {
      throw new Error('create run route did not match')
    }

    const createRunResponse = await Effect.runPromise(createRun)
    const createRunBody = await readJson(createRunResponse)
    const runRecord = createRunBody.run as { id: string }
    const createExport = routes.routeMulletRequest(
      new Request(
        `https://openagents.com/api/mullet/runs/${runRecord.id}/export`,
        {
          body: JSON.stringify({ format: 'markdown' }),
          method: 'POST',
        },
      ),
      env,
      ctx,
    )

    if (createExport === undefined) {
      throw new Error('create export route did not match')
    }

    const createExportResponse = await Effect.runPromise(createExport)
    const createExportBody = await readJson(createExportResponse)

    expect(createExportResponse.status).toBe(201)
    expect(createExportBody.content).toContain(
      'Private Mullet Simulation Export',
    )
    expect(createExportBody.content).toContain('Public claim projection: no')
    expect(createExportBody.export).toMatchObject({
      id: 'mullet_export_route_fixture',
      runExport: {
        format: 'markdown',
        privateVisibility: true,
        redactionStatus: 'passed',
      },
    })

    const getExport = routes.routeMulletRequest(
      new Request(
        `https://openagents.com/api/mullet/runs/${runRecord.id}/export`,
      ),
      env,
      ctx,
    )

    if (getExport === undefined) {
      throw new Error('get export route did not match')
    }

    const getExportResponse = await Effect.runPromise(getExport)
    const getExportBody = await readJson(getExportResponse)

    expect(getExportResponse.status).toBe(200)
    expect(getExportBody.export).toMatchObject({
      id: 'mullet_export_route_fixture',
      runExport: {
        format: 'markdown',
        privateVisibility: true,
      },
    })
  })

  test('returns 404 when a run is not found', async () => {
    const response = await runRoute(
      chrisSession,
      new Request('https://openagents.com/api/mullet/runs/missing'),
    )

    expect(response.status).toBe(404)
    expect(await readJson(response)).toEqual({ error: 'not_found' })
  })
})
