// Operator RLM trace projection route (#6686).
//
//   GET /api/operator/rlm/traces
//
// Admin scoped. Lists redacted/ref-only trace metadata for Recursive Language
// Model / FRLM-conductor work. It never returns raw trajectory JSON or raw
// executor payloads; Blueprint signature refs describe the governing boundary.

import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

const RLM_TRACES_DEFAULT_LIMIT = 25
const RLM_TRACES_MAX_LIMIT = 100
const RLM_TRACES_SCHEMA_VERSION = 'openagents.operator.rlm_traces.v1'

export type OperatorRlmTraceSummary = Readonly<{
  traceUuid: string
  traceRef: string
  createdAt: string
  updatedAt: string
  agentRef: string
  schemaVersion: string
  visibility: string
  stepCount: number
  demandKind: string
  demandSource: string
  trajectoryRef: string | null
  blueprintSignatureRefs: ReadonlyArray<string>
  evidenceRefs: ReadonlyArray<string>
  authority: {
    directExecutionAuthority: false
    payoutAuthority: false
    publicClaimAuthority: false
    trainingPromotionAuthority: false
  }
}>

export type OperatorRlmTracesProjection = Readonly<{
  schemaVersion: typeof RLM_TRACES_SCHEMA_VERSION
  generatedAt: string
  scope: 'operator'
  sourceTables: ReadonlyArray<'agent_traces'>
  filters: {
    limit: number
    ownerUserId: string | null
    visibility: string | null
  }
  route: '/api/operator/rlm/traces'
  traces: ReadonlyArray<OperatorRlmTraceSummary>
  blueprint: {
    programFamily: 'recursive_language_model'
    conductorRef: 'program_signature.frlm_conductor.v1'
    leafExecutorRef: 'program_signature.rlm_leaf_executor.v1'
    privacyPolicy: 'operator_refs_only'
    releaseGateRefs: ReadonlyArray<string>
  }
}>

export type OperatorRlmTraceStore = Readonly<{
  listTraces: (input: {
    limit: number
    ownerUserId: string | null
    visibility: string | null
  }) => Promise<ReadonlyArray<OperatorRlmTraceSummary>>
}>

export type OperatorRlmTracesDependencies = Readonly<{
  requireAdminApiToken: (request: Request) => Promise<boolean>
  store: OperatorRlmTraceStore
  nowIso?: (() => string) | undefined
}>

class OperatorRlmTracesStorageError extends S.TaggedErrorClass<OperatorRlmTracesStorageError>()(
  'OperatorRlmTracesStorageError',
  {
    reason: S.String,
  },
) {}

const n = (value: unknown): number => {
  const parsed = typeof value === 'number' ? value : Number(value ?? 0)
  return Number.isFinite(parsed) ? parsed : 0
}

const s = (value: unknown, fallback = 'unknown'): string => {
  const text = typeof value === 'string' ? value.trim() : ''
  return text.length === 0 ? fallback : text
}

const clampInteger = (value: number, min: number, max: number): number =>
  Math.min(Math.max(Math.trunc(value), min), max)

const parseLimit = (request: Request): number => {
  const url = new URL(request.url)
  const requestedLimit = Number(url.searchParams.get('limit') ?? RLM_TRACES_DEFAULT_LIMIT)
  return Number.isFinite(requestedLimit)
    ? clampInteger(requestedLimit, 1, RLM_TRACES_MAX_LIMIT)
    : RLM_TRACES_DEFAULT_LIMIT
}

const optionalQueryValue = (request: Request, key: string): string | null => {
  const value = new URL(request.url).searchParams.get(key)?.trim()
  return value === undefined || value === '' ? null : value
}

const blueprintSignatureRefs = (): ReadonlyArray<string> => [
  'program_signature.frlm_conductor.v1',
  'program_signature.rlm_leaf_executor.v1',
  'program_signature.blueprint_action_submission.evidence_only.v1',
]

const rlmTraceSummaryFromRow = (
  row: Record<string, unknown>,
): OperatorRlmTraceSummary => {
  const traceUuid = s(row.trace_uuid)
  const trajectoryId = s(row.trajectory_id, '')
  return {
    agentRef: s(row.agent_ref),
    authority: {
      directExecutionAuthority: false,
      payoutAuthority: false,
      publicClaimAuthority: false,
      trainingPromotionAuthority: false,
    },
    blueprintSignatureRefs: blueprintSignatureRefs(),
    createdAt: s(row.created_at),
    demandKind: s(row.demand_kind, 'unlabeled'),
    demandSource: s(row.demand_source),
    evidenceRefs: [
      `trace.${traceUuid}`,
      'evidence.rlm_trace.redacted_operator_projection',
      'evidence.blueprint_signature_lookup.safe_projection',
    ],
    schemaVersion: s(row.schema_version),
    stepCount: n(row.step_count),
    traceRef: `trace.${traceUuid}`,
    traceUuid,
    trajectoryRef:
      trajectoryId.length === 0 ? null : `trajectory.${trajectoryId}`,
    updatedAt: s(row.updated_at, s(row.created_at)),
    visibility: s(row.visibility),
  }
}

const assertProjectionSafe = (
  projection: OperatorRlmTracesProjection,
): OperatorRlmTracesProjection => {
  const serialized = JSON.stringify(projection)
  if (containsProviderSecretMaterial(serialized)) {
    throw new OperatorRlmTracesStorageError({
      reason: 'RLM trace projection contained private-data-shaped material',
    })
  }
  return projection
}

export const makeD1OperatorRlmTraceStore = (
  db: D1Database,
): OperatorRlmTraceStore => ({
  listTraces: async input => {
    const visibilityFilter =
      input.visibility === null ? '' : ' AND visibility = ?'
    const ownerFilter =
      input.ownerUserId === null ? '' : ' AND owner_user_id = ?'
    const bindings: Array<string | number> = []

    if (input.visibility !== null) {
      bindings.push(input.visibility)
    }
    if (input.ownerUserId !== null) {
      bindings.push(input.ownerUserId)
    }
    bindings.push(input.limit)

    const rows = await db
      .prepare(
        `SELECT trace_uuid,
                owner_user_id,
                agent_ref,
                schema_version,
                visibility,
                step_count,
                demand_kind,
                demand_source,
                trajectory_id,
                created_at,
                updated_at
           FROM agent_traces
          WHERE (
                lower(COALESCE(demand_kind, '')) LIKE '%rlm%'
             OR lower(COALESCE(demand_source, '')) LIKE '%rlm%'
             OR lower(COALESCE(agent_ref, '')) LIKE '%rlm%'
             OR lower(COALESCE(trajectory_id, '')) LIKE '%rlm%'
             OR lower(COALESCE(demand_kind, '')) LIKE '%frlm%'
             OR lower(COALESCE(demand_source, '')) LIKE '%frlm%'
             OR lower(COALESCE(agent_ref, '')) LIKE '%frlm%'
             OR lower(COALESCE(trajectory_id, '')) LIKE '%frlm%'
          )
          ${visibilityFilter}
          ${ownerFilter}
          ORDER BY created_at DESC
          LIMIT ?`,
      )
      .bind(...bindings)
      .all<Record<string, unknown>>()

    return (rows.results ?? []).map(rlmTraceSummaryFromRow)
  },
})

export const buildOperatorRlmTracesProjection = (input: {
  generatedAt: string
  limit: number
  ownerUserId: string | null
  traces: ReadonlyArray<OperatorRlmTraceSummary>
  visibility: string | null
}): OperatorRlmTracesProjection =>
  assertProjectionSafe({
    blueprint: {
      conductorRef: 'program_signature.frlm_conductor.v1',
      leafExecutorRef: 'program_signature.rlm_leaf_executor.v1',
      privacyPolicy: 'operator_refs_only',
      programFamily: 'recursive_language_model',
      releaseGateRefs: [
        'release_gate.rlm_trace.redacted_operator_projection',
        'release_gate.blueprint_signature.safe_projection',
      ],
    },
    filters: {
      limit: input.limit,
      ownerUserId: input.ownerUserId,
      visibility: input.visibility,
    },
    generatedAt: input.generatedAt,
    route: '/api/operator/rlm/traces',
    schemaVersion: RLM_TRACES_SCHEMA_VERSION,
    scope: 'operator',
    sourceTables: ['agent_traces'],
    traces: input.traces,
  })

export const handleOperatorRlmTraces = (
  request: Request,
  dependencies: OperatorRlmTracesDependencies,
): Effect.Effect<HttpResponse> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.gen(function* () {
    const authorized = yield* Effect.tryPromise({
      try: () => dependencies.requireAdminApiToken(request),
      catch: () => false,
    })
    if (!authorized) {
      return unauthorized()
    }

    const limit = parseLimit(request)
    const ownerUserId = optionalQueryValue(request, 'owner_user_id')
    const visibility = optionalQueryValue(request, 'visibility')
    const traces = yield* Effect.tryPromise({
      try: () =>
        dependencies.store.listTraces({ limit, ownerUserId, visibility }),
      catch: error =>
        new OperatorRlmTracesStorageError({
          reason: error instanceof Error ? error.message : String(error),
        }),
    })

    const projection = yield* Effect.try({
      try: () =>
        buildOperatorRlmTracesProjection({
          generatedAt: (dependencies.nowIso ?? currentIsoTimestamp)(),
          limit,
          ownerUserId,
          traces,
          visibility,
        }),
      catch: error =>
        error instanceof OperatorRlmTracesStorageError
          ? error
          : new OperatorRlmTracesStorageError({
              reason: error instanceof Error ? error.message : String(error),
            }),
    })

    return noStoreJsonResponse(projection)
  }).pipe(
    Effect.catchTag('OperatorRlmTracesStorageError', () =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'operator_rlm_traces_unavailable' },
          { status: 500 },
        ),
      ),
    ),
  )
}
