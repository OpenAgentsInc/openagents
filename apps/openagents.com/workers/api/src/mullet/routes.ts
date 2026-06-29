import {
  MulletAcceptedWorkProofPacket,
  type MulletAcceptedWorkProofPacket as MulletAcceptedWorkProofPacketType,
  MulletEnergyTelemetryRecord,
  type MulletEnergyTelemetryRecord as MulletEnergyTelemetryRecordType,
  MulletMarketMemory,
  type MulletMarketMemory as MulletMarketMemoryType,
  MulletPowerDataState,
  type MulletPowerDataState as MulletPowerDataStateType,
  MulletProviderSettlementState,
  type MulletProviderSettlementState as MulletProviderSettlementStateType,
  MulletScenario,
  MulletScenarioId,
  decodeMulletSimulationRun,
} from '@openagentsinc/mullet-schema'
import { simulateScenarioHour } from '@openagentsinc/mullet-sim'
import { Effect, Layer, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from '../http/responses'
import { readJsonObject } from '../json-boundary'
import {
  MulletExportFormat,
  MulletExportRedactionError,
  buildMulletRunExport,
} from './export'
import {
  MulletPersistenceValidationError,
  MulletRepository,
  type MulletRepositoryRuntime,
  MulletRunExportNotFound,
  MulletScenarioNotFound,
  type MulletScenarioRecord,
  MulletSimulationRunNotFound,
  MulletStorageError,
  MulletUnsafePersistencePayload,
  systemMulletRepositoryRuntime,
} from './repository'

type MulletEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type HttpResponse = globalThis.Response

type MulletSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type MulletRouteDependencies<
  Session extends MulletSession,
  Bindings extends MulletEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  isOpenAgentsAdminEmail: (email: string) => boolean
  repositoryLayer?: (
    env: Bindings,
    runtime: MulletRepositoryRuntime,
  ) => Layer.Layer<MulletRepository>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  runtime?: MulletRepositoryRuntime
}>

class MulletRouteUnauthorized extends S.TaggedErrorClass<MulletRouteUnauthorized>()(
  'MulletRouteUnauthorized',
  {},
) {}

class MulletRouteForbidden extends S.TaggedErrorClass<MulletRouteForbidden>()(
  'MulletRouteForbidden',
  {},
) {}

class MulletRouteSessionError extends S.TaggedErrorClass<MulletRouteSessionError>()(
  'MulletRouteSessionError',
  {
    error: S.Defect,
  },
) {}

class MulletRouteValidationError extends S.TaggedErrorClass<MulletRouteValidationError>()(
  'MulletRouteValidationError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

type MulletRouteError =
  | MulletPersistenceValidationError
  | MulletExportRedactionError
  | MulletRouteForbidden
  | MulletRouteSessionError
  | MulletRouteUnauthorized
  | MulletRouteValidationError
  | MulletRunExportNotFound
  | MulletScenarioNotFound
  | MulletSimulationRunNotFound
  | MulletStorageError
  | MulletUnsafePersistencePayload

const CreateScenarioBody = S.Struct({
  scenario: MulletScenario,
})

const CreateRunBody = S.Struct({
  energyTelemetry: S.optionalKey(S.Array(MulletEnergyTelemetryRecord)),
  marketMemory: S.optionalKey(S.Array(MulletMarketMemory)),
  powerDataState: S.optionalKey(MulletPowerDataState),
  proofPackets: S.optionalKey(S.Array(MulletAcceptedWorkProofPacket)),
  providerSettlementState: S.optionalKey(MulletProviderSettlementState),
  scenarioId: MulletScenarioId,
})

const CreateRunExportBody = S.Struct({
  format: MulletExportFormat,
})

const decodeUnknownEffect = <A>(
  operation: string,
  schema: S.Decoder<A>,
  value: unknown,
): Effect.Effect<A, MulletRouteValidationError> =>
  Effect.try({
    try: () => S.decodeUnknownSync(schema)(value),
    catch: error => new MulletRouteValidationError({ operation, error }),
  })

const readJsonBody = (
  operation: string,
  request: Request,
): Effect.Effect<Record<string, unknown>, MulletRouteValidationError> =>
  Effect.tryPromise({
    try: () => readJsonObject(request),
    catch: error => new MulletRouteValidationError({ operation, error }),
  })

const responseWithError = (error: MulletRouteError): HttpResponse => {
  switch (error._tag) {
    case 'MulletRouteUnauthorized':
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    case 'MulletRouteForbidden':
      return noStoreJsonResponse({ error: 'forbidden' }, { status: 403 })
    case 'MulletRouteValidationError':
    case 'MulletExportRedactionError':
    case 'MulletPersistenceValidationError':
    case 'MulletUnsafePersistencePayload':
      return noStoreJsonResponse(
        { error: 'invalid_mullet_request' },
        {
          status: 400,
        },
      )
    case 'MulletRunExportNotFound':
    case 'MulletScenarioNotFound':
    case 'MulletSimulationRunNotFound':
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    case 'MulletRouteSessionError':
    case 'MulletStorageError':
      return noStoreJsonResponse(
        { error: 'internal_server_error' },
        { status: 500 },
      )
  }
}

const requireMulletSession = <
  Session extends MulletSession,
  Bindings extends MulletEnv,
>(
  dependencies: MulletRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<Session, MulletRouteError> =>
  Effect.gen(function* () {
    const session = yield* Effect.tryPromise({
      try: () => dependencies.requireBrowserSession(request, env, ctx),
      catch: error => new MulletRouteSessionError({ error }),
    })

    if (session === undefined) {
      return yield* new MulletRouteUnauthorized()
    }

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new MulletRouteForbidden()
    }

    return session
  })

const mulletBootstrap = (email: string) => ({
  access: {
    operatorEmail: email,
    visibility: 'private',
  },
  authorityBoundary: {
    canAssignLiveWork: false,
    canMutateProviders: false,
    canPromotePublicClaims: false,
    canSettlePayouts: false,
    canSpendWalletFunds: false,
  },
  routes: [
    'GET /api/mullet/bootstrap',
    'GET /api/mullet/scenarios',
    'POST /api/mullet/scenarios',
    'GET /api/mullet/scenarios/:scenarioId',
    'POST /api/mullet/runs',
    'GET /api/mullet/runs/:runId',
    'GET /api/mullet/runs/:runId/export',
    'POST /api/mullet/runs/:runId/export',
  ],
  schemaVersion: '2026-06-08.v1',
})

const buildSimulationRun = (
  input: Readonly<{
    ownerEmail: string
    ownerUserId: string
    energyTelemetry?: readonly MulletEnergyTelemetryRecordType[]
    marketMemory?: readonly MulletMarketMemoryType[]
    powerDataState?: MulletPowerDataStateType
    proofPackets?: readonly MulletAcceptedWorkProofPacketType[]
    providerSettlementState?: MulletProviderSettlementStateType
    scenarioRecord: MulletScenarioRecord
    runtime: MulletRepositoryRuntime
  }>,
): Effect.Effect<
  ReturnType<typeof decodeMulletSimulationRun>,
  MulletRouteValidationError
> =>
  Effect.try({
    try: () => {
      const now = input.runtime.nowIso()
      const proofPackets = [...(input.proofPackets ?? [])]
      const marketMemory = [...(input.marketMemory ?? [])]
      const energyTelemetry = [...(input.energyTelemetry ?? [])]
      const dispatchResult = {
        ...simulateScenarioHour(input.scenarioRecord.scenario),
        energyTelemetryRecordIds: energyTelemetry.map(record => record.id),
        marketMemoryUpdateIds: marketMemory.map(record => record.id),
        proofPacketIds: proofPackets.map(packet => packet.id),
      }

      return decodeMulletSimulationRun({
        id: input.runtime.makeId('mullet_run'),
        scenarioId: input.scenarioRecord.scenario.id,
        ownerUserId: input.ownerUserId,
        ownerEmail: input.ownerEmail,
        status: 'succeeded',
        createdAt: now,
        updatedAt: now,
        completedAt: now,
        scenario: input.scenarioRecord.scenario,
        dispatchResults: [dispatchResult],
        capitalReturns: [],
        proofPackets,
        marketMemory,
        energyTelemetry,
        providerSettlementState:
          input.providerSettlementState ?? 'not_payable',
        powerDataState: input.powerDataState ?? 'modeled',
      })
    },
    catch: error =>
      new MulletRouteValidationError({
        operation: 'mullet.runs.buildSimulationRun',
        error,
      }),
  })

const routeMulletRequestWithSession = <
  Session extends MulletSession,
  Bindings extends MulletEnv,
>(
  dependencies: MulletRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  segments: ReadonlyArray<string>,
): Effect.Effect<HttpResponse, MulletRouteError, MulletRepository> =>
  Effect.gen(function* () {
    const session = yield* requireMulletSession(dependencies, request, env, ctx)
    const repository = yield* MulletRepository
    const method = request.method.toUpperCase()
    const runtime = dependencies.runtime ?? systemMulletRepositoryRuntime
    const ok = (value: unknown, init: ResponseInit = {}) =>
      dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(value, init),
        session,
      )

    if (segments.length === 3 && segments[2] === 'bootstrap') {
      if (method !== 'GET') {
        return methodNotAllowed(['GET'])
      }

      return ok(mulletBootstrap(session.user.email))
    }

    if (segments.length === 3 && segments[2] === 'scenarios') {
      if (method === 'GET') {
        const scenarios = yield* repository.listScenarios(session.user.userId)

        return ok({ scenarios })
      }

      if (method === 'POST') {
        const body = yield* readJsonBody('mullet.scenarios.createBody', request)
        const input = yield* decodeUnknownEffect(
          'mullet.scenarios.decodeCreateBody',
          CreateScenarioBody,
          body,
        )
        const scenario = yield* repository.createScenario({
          ownerEmail: session.user.email,
          ownerUserId: session.user.userId,
          scenario: input.scenario,
        })

        return ok({ scenario }, { status: 201 })
      }

      return methodNotAllowed(['GET', 'POST'])
    }

    if (segments.length === 4 && segments[2] === 'scenarios') {
      if (method !== 'GET') {
        return methodNotAllowed(['GET'])
      }

      const scenario = yield* repository.getScenario(
        session.user.userId,
        segments[3] ?? '',
      )

      return ok({ scenario })
    }

    if (segments.length === 3 && segments[2] === 'runs') {
      if (method !== 'POST') {
        return methodNotAllowed(['POST'])
      }

      const body = yield* readJsonBody('mullet.runs.createBody', request)
      const input = yield* decodeUnknownEffect(
        'mullet.runs.decodeCreateBody',
        CreateRunBody,
        body,
      )
      const scenarioRecord = yield* repository.getScenario(
        session.user.userId,
        input.scenarioId,
      )
      const attachmentInput = {
        ...(input.energyTelemetry === undefined
          ? {}
          : { energyTelemetry: input.energyTelemetry }),
        ...(input.marketMemory === undefined
          ? {}
          : { marketMemory: input.marketMemory }),
        ...(input.powerDataState === undefined
          ? {}
          : { powerDataState: input.powerDataState }),
        ...(input.proofPackets === undefined
          ? {}
          : { proofPackets: input.proofPackets }),
        ...(input.providerSettlementState === undefined
          ? {}
          : { providerSettlementState: input.providerSettlementState }),
      }
      const run = yield* buildSimulationRun({
        ownerEmail: session.user.email,
        ownerUserId: session.user.userId,
        ...attachmentInput,
        scenarioRecord,
        runtime,
      })
      const runRecord = yield* repository.createSimulationRun({ run })

      return ok({ run: runRecord }, { status: 201 })
    }

    if (segments.length === 4 && segments[2] === 'runs') {
      if (method !== 'GET') {
        return methodNotAllowed(['GET'])
      }

      const run = yield* repository.getSimulationRun(
        session.user.userId,
        segments[3] ?? '',
      )

      return ok({ run })
    }

    if (
      segments.length === 5 &&
      segments[2] === 'runs' &&
      segments[4] === 'export'
    ) {
      if (method !== 'GET') {
        if (method !== 'POST') {
          return methodNotAllowed(['GET', 'POST'])
        }

        const body = yield* readJsonBody('mullet.exports.createBody', request)
        const input = yield* decodeUnknownEffect(
          'mullet.exports.decodeCreateBody',
          CreateRunExportBody,
          body,
        )
        const run = yield* repository.getSimulationRun(
          session.user.userId,
          segments[3] ?? '',
        )
        const exportId = runtime.makeId('mullet_export')
        const generated = yield* buildMulletRunExport({
          exportId,
          format: input.format,
          generatedAt: runtime.nowIso(),
          runRecord: run,
        })
        const exportRecord = yield* repository.createRunExport({
          exportId,
          ownerEmail: session.user.email,
          ownerUserId: session.user.userId,
          runExport: generated.runExport,
          schemaVersion: run.schemaVersion,
        })

        return ok(
          { content: generated.content, export: exportRecord },
          { status: 201 },
        )
      }

      const runExport = yield* repository.getLatestRunExport(
        session.user.userId,
        segments[3] ?? '',
      )

      return ok({ export: runExport })
    }

    return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
  })

export const makeMulletRoutes = <
  Session extends MulletSession,
  Bindings extends MulletEnv,
>(
  dependencies: MulletRouteDependencies<Session, Bindings>,
) => {
  const runtime = dependencies.runtime ?? systemMulletRepositoryRuntime
  const repositoryLayer =
    dependencies.repositoryLayer ??
    ((env: Bindings, nextRuntime: MulletRepositoryRuntime) =>
      MulletRepository.layer(env, nextRuntime))

  return {
    routeMulletRequest: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      const segments = new URL(request.url).pathname.split('/').filter(Boolean)

      if (segments[0] !== 'api' || segments[1] !== 'mullet') {
        return undefined
      }

      return routeMulletRequestWithSession(
        { ...dependencies, runtime },
        request,
        env,
        ctx,
        segments,
      ).pipe(
        Effect.provide(repositoryLayer(env, runtime)),
        Effect.catch(error => Effect.succeed(responseWithError(error))),
      )
    },
  }
}
