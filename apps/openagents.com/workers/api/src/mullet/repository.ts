import {
  MulletHourlyCandidateMode,
  type MulletHourlyCandidateMode as MulletHourlyCandidateModeType,
  MulletHourlyDispatchResult,
  type MulletScenario,
  type MulletSimulationRun,
  MulletSimulationRunExport,
  type MulletSimulationRunExport as MulletSimulationRunExportType,
  MulletSourceRef,
  type MulletSourceRef as MulletSourceRefType,
  decodeMulletScenario,
  decodeMulletScenarioJson,
  decodeMulletSimulationRun,
  decodeMulletSimulationRunJson,
  encodeMulletScenarioJson,
  encodeMulletSimulationRunJson,
} from '@openagentsinc/mullet-schema'
import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import { openAgentsDatabase } from '../runtime'
import { compactRandomId, currentIsoTimestamp } from '../runtime-primitives'

type MulletEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

export type MulletRepositoryRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
}>

export const systemMulletRepositoryRuntime: MulletRepositoryRuntime = {
  makeId: compactRandomId,
  nowIso: currentIsoTimestamp,
}

export type MulletProvenanceSummary = Readonly<{
  acceptedValueCount: number
  measuredValueCount: number
  modeledValueCount: number
  needsDiligenceCount: number
  paidValueCount: number
  settledValueCount: number
  sourceRefCount: number
}>

export type MulletScenarioRecord = Readonly<{
  id: string
  ownerEmail: string
  ownerUserId: string
  scenario: MulletScenario
  schemaVersion: string
  sourceRefs: ReadonlyArray<MulletSourceRefType>
  provenanceSummary: MulletProvenanceSummary
  visibility: 'private'
  exportRedactionState: 'not_checked' | 'passed' | 'failed'
  createdAt: string
  updatedAt: string
}>

export type MulletSimulationRunRecord = Readonly<{
  id: string
  ownerEmail: string
  ownerUserId: string
  run: MulletSimulationRun
  scenarioId: string
  schemaVersion: string
  sourceRefs: ReadonlyArray<MulletSourceRefType>
  provenanceSummary: MulletProvenanceSummary
  visibility: 'private'
  exportRedactionState: 'not_checked' | 'passed' | 'failed'
  createdAt: string
  updatedAt: string
  completedAt: string | null
}>

export type MulletRunCandidateModeRecord = Readonly<{
  id: string
  runId: string
  hourlyResultId: string
  scenarioId: string
  ownerUserId: string
  hourIndex: number
  candidateIndex: number
  candidate: MulletHourlyCandidateModeType
  createdAt: string
}>

export type MulletRunExportRecord = Readonly<{
  id: string
  ownerEmail: string
  ownerUserId: string
  runExport: MulletSimulationRunExportType
  runId: string
  scenarioId: string
  schemaVersion: string
  createdAt: string
}>

type ScenarioRow = Readonly<{
  id: string
  owner_user_id: string
  owner_email: string
  schema_version: string
  scenario_json: string
  source_refs_json: string
  provenance_summary_json: string
  visibility: 'private'
  export_redaction_state: 'not_checked' | 'passed' | 'failed'
  created_at: string
  updated_at: string
}>

type SimulationRunRow = Readonly<{
  id: string
  scenario_id: string
  owner_user_id: string
  owner_email: string
  schema_version: string
  run_json: string
  source_refs_json: string
  provenance_summary_json: string
  visibility: 'private'
  export_redaction_state: 'not_checked' | 'passed' | 'failed'
  created_at: string
  updated_at: string
  completed_at: string | null
}>

type CandidateModeRow = Readonly<{
  id: string
  run_id: string
  hourly_result_id: string
  scenario_id: string
  owner_user_id: string
  hour_index: number
  candidate_index: number
  candidate_json: string
  created_at: string
}>

type ExportRow = Readonly<{
  id: string
  run_id: string
  scenario_id: string
  owner_user_id: string
  owner_email: string
  schema_version: string
  export_json: string
  created_at: string
}>

export class MulletScenarioNotFound extends S.TaggedErrorClass<MulletScenarioNotFound>()(
  'MulletScenarioNotFound',
  {
    ownerUserId: S.String,
    scenarioId: S.String,
  },
) {}

export class MulletSimulationRunNotFound extends S.TaggedErrorClass<MulletSimulationRunNotFound>()(
  'MulletSimulationRunNotFound',
  {
    ownerUserId: S.String,
    runId: S.String,
  },
) {}

export class MulletRunExportNotFound extends S.TaggedErrorClass<MulletRunExportNotFound>()(
  'MulletRunExportNotFound',
  {
    ownerUserId: S.String,
    runId: S.String,
  },
) {}

export class MulletPersistenceValidationError extends S.TaggedErrorClass<MulletPersistenceValidationError>()(
  'MulletPersistenceValidationError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class MulletUnsafePersistencePayload extends S.TaggedErrorClass<MulletUnsafePersistencePayload>()(
  'MulletUnsafePersistencePayload',
  {
    path: S.String,
    reason: S.String,
  },
) {}

export class MulletStorageError extends S.TaggedErrorClass<MulletStorageError>()(
  'MulletStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export const MulletRepositoryError = S.Union([
  MulletScenarioNotFound,
  MulletSimulationRunNotFound,
  MulletRunExportNotFound,
  MulletPersistenceValidationError,
  MulletUnsafePersistencePayload,
  MulletStorageError,
])
export type MulletRepositoryError = typeof MulletRepositoryError.Type

export type CreateMulletScenarioInput = Readonly<{
  ownerEmail: string
  ownerUserId: string
  scenario: MulletScenario
}>

export type CreateMulletSimulationRunInput = Readonly<{
  run: MulletSimulationRun
}>

export type CreateMulletRunExportInput = Readonly<{
  exportId?: string
  ownerEmail: string
  ownerUserId: string
  runExport: MulletSimulationRunExportType
  schemaVersion: string
}>

export type MulletRepositoryShape = Readonly<{
  createScenario: (
    input: CreateMulletScenarioInput,
  ) => Effect.Effect<MulletScenarioRecord, MulletRepositoryError>
  listScenarios: (
    ownerUserId: string,
  ) => Effect.Effect<ReadonlyArray<MulletScenarioRecord>, MulletRepositoryError>
  getScenario: (
    ownerUserId: string,
    scenarioId: string,
  ) => Effect.Effect<MulletScenarioRecord, MulletRepositoryError>
  createSimulationRun: (
    input: CreateMulletSimulationRunInput,
  ) => Effect.Effect<MulletSimulationRunRecord, MulletRepositoryError>
  getSimulationRun: (
    ownerUserId: string,
    runId: string,
  ) => Effect.Effect<MulletSimulationRunRecord, MulletRepositoryError>
  listRunCandidateModes: (
    ownerUserId: string,
    runId: string,
  ) => Effect.Effect<
    ReadonlyArray<MulletRunCandidateModeRecord>,
    MulletRepositoryError
  >
  createRunExport: (
    input: CreateMulletRunExportInput,
  ) => Effect.Effect<MulletRunExportRecord, MulletRepositoryError>
  getLatestRunExport: (
    ownerUserId: string,
    runId: string,
  ) => Effect.Effect<MulletRunExportRecord, MulletRepositoryError>
}>

export class MulletRepository extends Context.Service<
  MulletRepository,
  MulletRepositoryShape
>()('@openagentsinc/MulletRepository') {
  static layer = (
    env: MulletEnv,
    runtime: MulletRepositoryRuntime = systemMulletRepositoryRuntime,
  ) => Layer.succeed(MulletRepository, makeMulletRepository(env, runtime))
}

const SourceRefsJson = S.fromJsonString(S.Array(MulletSourceRef))
const ProvenanceSummarySchema = S.Struct({
  acceptedValueCount: S.Number,
  measuredValueCount: S.Number,
  modeledValueCount: S.Number,
  needsDiligenceCount: S.Number,
  paidValueCount: S.Number,
  settledValueCount: S.Number,
  sourceRefCount: S.Number,
})
const ProvenanceSummaryJson = S.fromJsonString(ProvenanceSummarySchema)
const HourlyDispatchResultJson = S.fromJsonString(MulletHourlyDispatchResult)
const CandidateModeJson = S.fromJsonString(MulletHourlyCandidateMode)
const SimulationRunExportJson = S.fromJsonString(MulletSimulationRunExport)

const encodeSourceRefsJson = S.encodeSync(SourceRefsJson)
const decodeSourceRefsJson = S.decodeUnknownSync(SourceRefsJson)
const encodeProvenanceSummaryJson = S.encodeSync(ProvenanceSummaryJson)
const decodeProvenanceSummaryJson = S.decodeUnknownSync(ProvenanceSummaryJson)
const encodeHourlyDispatchResultJson = S.encodeSync(HourlyDispatchResultJson)
const encodeCandidateModeJson = S.encodeSync(CandidateModeJson)
const decodeCandidateModeJson = S.decodeUnknownSync(CandidateModeJson)
const encodeSimulationRunExportJson = S.encodeSync(SimulationRunExportJson)
const decodeSimulationRunExportJson = S.decodeUnknownSync(
  SimulationRunExportJson,
)

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, MulletStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new MulletStorageError({ operation, error }),
  })

const decodeEffect = <A>(
  operation: string,
  run: () => A,
): Effect.Effect<A, MulletPersistenceValidationError> =>
  Effect.try({
    try: run,
    catch: error => new MulletPersistenceValidationError({ operation, error }),
  })

const unsafeKeyFragments = [
  'secret',
  'mnemonic',
  'private_key',
  'privatekey',
  'wallet',
  'preimage',
  'raw_prompt',
  'rawprompt',
  'raw_trace',
  'rawtrace',
  'private_artifact',
  'private_repo',
  'payment_preimage',
  'provider_secret',
  'customer_data',
  'raw_log',
  'raw_timestamp',
  'invoice',
]

const unsafeStringFragments = [
  'sk_live_',
  'gho_',
  'github_pat_',
  'xoxb-',
  '-----begin private key-----',
  'payment preimage',
  'raw prompt',
  'raw trace',
  'private repo',
  'wallet mnemonic',
  'bolt11',
  'lnbc',
]

const normalizeUnsafeScanText = (value: string): string =>
  value.toLowerCase().replaceAll('-', '_').replaceAll(' ', '_')

const unsafePayloadReason = (
  value: unknown,
  path: ReadonlyArray<string>,
): MulletUnsafePersistencePayload | undefined => {
  const pathKey = normalizeUnsafeScanText(path.at(-1) ?? '')

  if (
    pathKey !== '' &&
    unsafeKeyFragments.some(fragment => pathKey.includes(fragment))
  ) {
    return new MulletUnsafePersistencePayload({
      path: path.join('.'),
      reason: 'unsafe_key',
    })
  }

  if (typeof value === 'string') {
    const text = value.toLowerCase()
    const normalizedText = normalizeUnsafeScanText(value)

    if (
      unsafeStringFragments.some(
        fragment =>
          text.includes(fragment) || normalizedText.includes(fragment),
      )
    ) {
      return new MulletUnsafePersistencePayload({
        path: path.join('.'),
        reason: 'unsafe_string',
      })
    }
  }

  return undefined
}

const findUnsafePayload = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): MulletUnsafePersistencePayload | undefined => {
  const directReason = unsafePayloadReason(value, path)

  if (directReason !== undefined) {
    return directReason
  }

  if (Array.isArray(value)) {
    return value
      .map((item, index) => findUnsafePayload(item, [...path, String(index)]))
      .find(reason => reason !== undefined)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.entries(value)
      .map(([key, item]) => findUnsafePayload(item, [...path, key]))
      .find(reason => reason !== undefined)
  }

  return undefined
}

const assertSafePayload = (
  value: unknown,
): Effect.Effect<void, MulletUnsafePersistencePayload> => {
  const unsafePayload = findUnsafePayload(value)

  if (unsafePayload !== undefined) {
    return Effect.fail(unsafePayload)
  }

  return Effect.void
}

const countProvenance = (
  value: unknown,
  summary: MulletProvenanceSummary,
): MulletProvenanceSummary => {
  if (Array.isArray(value)) {
    return value.reduce<MulletProvenanceSummary>(
      (nextSummary, item) => countProvenance(item, nextSummary),
      summary,
    )
  }

  if (typeof value !== 'object' || value === null) {
    return summary
  }

  const record = value as Record<string, unknown>
  const provenance = record.provenance
  const nextSummary =
    provenance === 'modeled'
      ? { ...summary, modeledValueCount: summary.modeledValueCount + 1 }
      : provenance === 'measured'
        ? { ...summary, measuredValueCount: summary.measuredValueCount + 1 }
        : provenance === 'accepted'
          ? { ...summary, acceptedValueCount: summary.acceptedValueCount + 1 }
          : provenance === 'paid'
            ? { ...summary, paidValueCount: summary.paidValueCount + 1 }
            : provenance === 'settled'
              ? {
                  ...summary,
                  settledValueCount: summary.settledValueCount + 1,
                }
              : summary
  const diligenceSummary =
    record.needsDiligence === true
      ? {
          ...nextSummary,
          needsDiligenceCount: nextSummary.needsDiligenceCount + 1,
        }
      : nextSummary

  return Object.values(record).reduce<MulletProvenanceSummary>(
    (nextSummary, item) => countProvenance(item, nextSummary),
    diligenceSummary,
  )
}

const provenanceSummaryForScenario = (
  scenario: MulletScenario,
): MulletProvenanceSummary => ({
  ...countProvenance(scenario, {
    acceptedValueCount: 0,
    measuredValueCount: 0,
    modeledValueCount: 0,
    needsDiligenceCount: 0,
    paidValueCount: 0,
    settledValueCount: 0,
    sourceRefCount: scenario.sourceRefs.length,
  }),
  sourceRefCount: scenario.sourceRefs.length,
})

const provenanceSummaryForRun = (
  run: MulletSimulationRun,
): MulletProvenanceSummary => ({
  ...countProvenance(run, {
    acceptedValueCount: 0,
    measuredValueCount: 0,
    modeledValueCount: 0,
    needsDiligenceCount: 0,
    paidValueCount: 0,
    settledValueCount: 0,
    sourceRefCount: run.scenario.sourceRefs.length,
  }),
  sourceRefCount: run.scenario.sourceRefs.length,
})

const toScenarioRecord = (
  row: ScenarioRow,
): Effect.Effect<MulletScenarioRecord, MulletPersistenceValidationError> =>
  decodeEffect('mullet.scenarios.decodeRow', () => ({
    id: row.id,
    ownerEmail: row.owner_email,
    ownerUserId: row.owner_user_id,
    scenario: decodeMulletScenarioJson(row.scenario_json),
    schemaVersion: row.schema_version,
    sourceRefs: decodeSourceRefsJson(row.source_refs_json),
    provenanceSummary: decodeProvenanceSummaryJson(row.provenance_summary_json),
    visibility: row.visibility,
    exportRedactionState: row.export_redaction_state,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  }))

const toSimulationRunRecord = (
  row: SimulationRunRow,
): Effect.Effect<MulletSimulationRunRecord, MulletPersistenceValidationError> =>
  decodeEffect('mullet.runs.decodeRow', () => ({
    id: row.id,
    ownerEmail: row.owner_email,
    ownerUserId: row.owner_user_id,
    run: decodeMulletSimulationRunJson(row.run_json),
    scenarioId: row.scenario_id,
    schemaVersion: row.schema_version,
    sourceRefs: decodeSourceRefsJson(row.source_refs_json),
    provenanceSummary: decodeProvenanceSummaryJson(row.provenance_summary_json),
    visibility: row.visibility,
    exportRedactionState: row.export_redaction_state,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    completedAt: row.completed_at,
  }))

const toCandidateRecord = (
  row: CandidateModeRow,
): Effect.Effect<
  MulletRunCandidateModeRecord,
  MulletPersistenceValidationError
> =>
  decodeEffect('mullet.candidates.decodeRow', () => ({
    id: row.id,
    runId: row.run_id,
    hourlyResultId: row.hourly_result_id,
    scenarioId: row.scenario_id,
    ownerUserId: row.owner_user_id,
    hourIndex: row.hour_index,
    candidateIndex: row.candidate_index,
    candidate: decodeCandidateModeJson(row.candidate_json),
    createdAt: row.created_at,
  }))

const toRunExportRecord = (
  row: ExportRow,
): Effect.Effect<MulletRunExportRecord, MulletPersistenceValidationError> =>
  decodeEffect('mullet.exports.decodeRow', () => ({
    id: row.id,
    ownerEmail: row.owner_email,
    ownerUserId: row.owner_user_id,
    runExport: decodeSimulationRunExportJson(row.export_json),
    runId: row.run_id,
    scenarioId: row.scenario_id,
    schemaVersion: row.schema_version,
    createdAt: row.created_at,
  }))

const createScenarioRecord = (
  db: D1Database,
  input: CreateMulletScenarioInput,
): Effect.Effect<MulletScenarioRecord, MulletRepositoryError> =>
  Effect.gen(function* () {
    const scenario = yield* decodeEffect('mullet.scenarios.decodeInput', () =>
      decodeMulletScenario(input.scenario),
    )
    const summary = provenanceSummaryForScenario(scenario)
    const scenarioJson = encodeMulletScenarioJson(scenario)
    const sourceRefsJson = encodeSourceRefsJson(scenario.sourceRefs)
    const provenanceSummaryJson = encodeProvenanceSummaryJson(summary)

    yield* assertSafePayload(scenario)

    yield* d1Effect('mullet.scenarios.create', () =>
      db
        .prepare(
          `INSERT INTO mullet_scenarios
             (id, owner_user_id, owner_email, schema_version, name, kind,
              scenario_json, source_refs_json, provenance_summary_json,
              visibility, export_redaction_state, created_at, updated_at,
              deleted_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'private', 'not_checked', ?, ?, NULL)`,
        )
        .bind(
          scenario.id,
          input.ownerUserId,
          input.ownerEmail,
          scenario.schemaVersion,
          scenario.name,
          scenario.kind,
          scenarioJson,
          sourceRefsJson,
          provenanceSummaryJson,
          scenario.createdAt,
          scenario.updatedAt,
        )
        .run(),
    )

    return {
      id: scenario.id,
      ownerEmail: input.ownerEmail,
      ownerUserId: input.ownerUserId,
      scenario,
      schemaVersion: scenario.schemaVersion,
      sourceRefs: scenario.sourceRefs,
      provenanceSummary: summary,
      visibility: 'private',
      exportRedactionState: 'not_checked',
      createdAt: scenario.createdAt,
      updatedAt: scenario.updatedAt,
    }
  })

const listScenarioRecords = (
  db: D1Database,
  ownerUserId: string,
): Effect.Effect<ReadonlyArray<MulletScenarioRecord>, MulletRepositoryError> =>
  Effect.gen(function* () {
    const rows = yield* d1Effect('mullet.scenarios.list', () =>
      db
        .prepare(
          `SELECT id, owner_user_id, owner_email, schema_version,
                  scenario_json, source_refs_json, provenance_summary_json,
                  visibility, export_redaction_state, created_at, updated_at
           FROM mullet_scenarios
           WHERE owner_user_id = ?
             AND deleted_at IS NULL
           ORDER BY updated_at DESC
           LIMIT 100`,
        )
        .bind(ownerUserId)
        .all<ScenarioRow>(),
    )

    return yield* Effect.all(rows.results.map(toScenarioRecord))
  })

const getScenarioRecord = (
  db: D1Database,
  ownerUserId: string,
  scenarioId: string,
): Effect.Effect<MulletScenarioRecord, MulletRepositoryError> =>
  Effect.gen(function* () {
    const row = yield* d1Effect('mullet.scenarios.get', () =>
      db
        .prepare(
          `SELECT id, owner_user_id, owner_email, schema_version,
                  scenario_json, source_refs_json, provenance_summary_json,
                  visibility, export_redaction_state, created_at, updated_at
           FROM mullet_scenarios
           WHERE owner_user_id = ?
             AND id = ?
             AND deleted_at IS NULL
           LIMIT 1`,
        )
        .bind(ownerUserId, scenarioId)
        .first<ScenarioRow>(),
    )

    if (row === null) {
      return yield* new MulletScenarioNotFound({ ownerUserId, scenarioId })
    }

    return yield* toScenarioRecord(row)
  })

const createSimulationRunRecord = (
  db: D1Database,
  runtime: MulletRepositoryRuntime,
  input: CreateMulletSimulationRunInput,
): Effect.Effect<MulletSimulationRunRecord, MulletRepositoryError> =>
  Effect.gen(function* () {
    const run = yield* decodeEffect('mullet.runs.decodeInput', () =>
      decodeMulletSimulationRun(input.run),
    )
    const summary = provenanceSummaryForRun(run)
    const runJson = encodeMulletSimulationRunJson(run)
    const sourceRefsJson = encodeSourceRefsJson(run.scenario.sourceRefs)
    const provenanceSummaryJson = encodeProvenanceSummaryJson(summary)
    const childStatements = run.dispatchResults.flatMap((result, hourIndex) => {
      const hourlyResultId = runtime.makeId('mullet_hour')
      const resultJson = encodeHourlyDispatchResultJson(result)
      const resultStatement = db
        .prepare(
          `INSERT INTO mullet_run_hourly_results
             (id, run_id, scenario_id, owner_user_id, hour_index, timestamp,
              selected_mode, reason_code, energy_mwh, result_json, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          hourlyResultId,
          run.id,
          run.scenarioId,
          run.ownerUserId,
          hourIndex,
          result.timestamp,
          result.selectedMode,
          result.reasonCode,
          result.energyMwh,
          resultJson,
          run.createdAt,
        )
      const candidateStatements = result.candidates.map(
        (candidate, candidateIndex) =>
          db
            .prepare(
              `INSERT INTO mullet_run_candidate_modes
                 (id, run_id, hourly_result_id, scenario_id, owner_user_id,
                  hour_index, candidate_index, timestamp, mode, reason_code,
                  risk_adjusted_net_usd_per_mwh, clears_readiness,
                  clears_demand, clears_provider_floor, candidate_json,
                  created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
            )
            .bind(
              runtime.makeId('mullet_candidate'),
              run.id,
              hourlyResultId,
              run.scenarioId,
              run.ownerUserId,
              hourIndex,
              candidateIndex,
              candidate.timestamp,
              candidate.mode,
              candidate.reasonCode,
              candidate.riskAdjustedNetUsdPerMwh,
              candidate.clearsReadiness ? 1 : 0,
              candidate.clearsDemand ? 1 : 0,
              candidate.clearsProviderFloor ? 1 : 0,
              encodeCandidateModeJson(candidate),
              run.createdAt,
            ),
      )

      return [resultStatement, ...candidateStatements]
    })

    yield* assertSafePayload(run)

    yield* d1Effect('mullet.runs.create', () =>
      db.batch([
        db
          .prepare(
            `INSERT INTO mullet_simulation_runs
               (id, scenario_id, owner_user_id, owner_email, schema_version,
                status, run_json, source_refs_json, provenance_summary_json,
                provider_settlement_state, power_data_state, visibility,
                export_redaction_state, created_at, updated_at, completed_at,
                deleted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'private',
                     'not_checked', ?, ?, ?, NULL)`,
          )
          .bind(
            run.id,
            run.scenarioId,
            run.ownerUserId,
            run.ownerEmail,
            run.scenario.schemaVersion,
            run.status,
            runJson,
            sourceRefsJson,
            provenanceSummaryJson,
            run.providerSettlementState,
            run.powerDataState,
            run.createdAt,
            run.updatedAt,
            run.completedAt ?? null,
          ),
        ...childStatements,
      ]),
    )

    return {
      id: run.id,
      ownerEmail: run.ownerEmail,
      ownerUserId: run.ownerUserId,
      run,
      scenarioId: run.scenarioId,
      schemaVersion: run.scenario.schemaVersion,
      sourceRefs: run.scenario.sourceRefs,
      provenanceSummary: summary,
      visibility: 'private',
      exportRedactionState: 'not_checked',
      createdAt: run.createdAt,
      updatedAt: run.updatedAt,
      completedAt: run.completedAt ?? null,
    }
  })

const getSimulationRunRecord = (
  db: D1Database,
  ownerUserId: string,
  runId: string,
): Effect.Effect<MulletSimulationRunRecord, MulletRepositoryError> =>
  Effect.gen(function* () {
    const row = yield* d1Effect('mullet.runs.get', () =>
      db
        .prepare(
          `SELECT id, scenario_id, owner_user_id, owner_email, schema_version,
                  run_json, source_refs_json, provenance_summary_json,
                  visibility, export_redaction_state, created_at, updated_at,
                  completed_at
           FROM mullet_simulation_runs
           WHERE owner_user_id = ?
             AND id = ?
             AND deleted_at IS NULL
           LIMIT 1`,
        )
        .bind(ownerUserId, runId)
        .first<SimulationRunRow>(),
    )

    if (row === null) {
      return yield* new MulletSimulationRunNotFound({ ownerUserId, runId })
    }

    return yield* toSimulationRunRecord(row)
  })

const listRunCandidateModeRecords = (
  db: D1Database,
  ownerUserId: string,
  runId: string,
): Effect.Effect<
  ReadonlyArray<MulletRunCandidateModeRecord>,
  MulletRepositoryError
> =>
  Effect.gen(function* () {
    const rows = yield* d1Effect('mullet.candidates.list', () =>
      db
        .prepare(
          `SELECT id, run_id, hourly_result_id, scenario_id, owner_user_id,
                  hour_index, candidate_index, candidate_json, created_at
           FROM mullet_run_candidate_modes
           WHERE owner_user_id = ?
             AND run_id = ?
           ORDER BY hour_index ASC, candidate_index ASC`,
        )
        .bind(ownerUserId, runId)
        .all<CandidateModeRow>(),
    )

    return yield* Effect.all(rows.results.map(toCandidateRecord))
  })

const createRunExportRecord = (
  db: D1Database,
  runtime: MulletRepositoryRuntime,
  input: CreateMulletRunExportInput,
): Effect.Effect<MulletRunExportRecord, MulletRepositoryError> =>
  Effect.gen(function* () {
    const runExport = yield* decodeEffect('mullet.exports.decodeInput', () =>
      S.decodeUnknownSync(MulletSimulationRunExport)(input.runExport),
    )
    const createdAt = runtime.nowIso()
    const exportId = input.exportId ?? runtime.makeId('mullet_export')
    const exportJson = encodeSimulationRunExportJson(runExport)

    yield* assertSafePayload(runExport)

    yield* d1Effect('mullet.exports.create', () =>
      db
        .prepare(
          `INSERT INTO mullet_run_exports
             (id, run_id, scenario_id, owner_user_id, owner_email,
              schema_version, format, export_json, private_visibility,
              redaction_status, content_ref, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?)`,
        )
        .bind(
          exportId,
          runExport.runId,
          runExport.scenarioId,
          input.ownerUserId,
          input.ownerEmail,
          input.schemaVersion,
          runExport.format,
          exportJson,
          runExport.redactionStatus,
          runExport.contentRef,
          createdAt,
        )
        .run(),
    )

    return {
      id: exportId,
      ownerEmail: input.ownerEmail,
      ownerUserId: input.ownerUserId,
      runExport,
      runId: runExport.runId,
      scenarioId: runExport.scenarioId,
      schemaVersion: input.schemaVersion,
      createdAt,
    }
  })

const getLatestRunExportRecord = (
  db: D1Database,
  ownerUserId: string,
  runId: string,
): Effect.Effect<MulletRunExportRecord, MulletRepositoryError> =>
  Effect.gen(function* () {
    const row = yield* d1Effect('mullet.exports.getLatest', () =>
      db
        .prepare(
          `SELECT id, run_id, scenario_id, owner_user_id, owner_email,
                  schema_version, export_json, created_at
           FROM mullet_run_exports
           WHERE owner_user_id = ?
             AND run_id = ?
           ORDER BY created_at DESC
           LIMIT 1`,
        )
        .bind(ownerUserId, runId)
        .first<ExportRow>(),
    )

    if (row === null) {
      return yield* new MulletRunExportNotFound({ ownerUserId, runId })
    }

    return yield* toRunExportRecord(row)
  })

export const makeMulletRepository = (
  env: MulletEnv,
  runtime: MulletRepositoryRuntime = systemMulletRepositoryRuntime,
): MulletRepositoryShape => {
  const db = openAgentsDatabase(env)

  return {
    createScenario: input =>
      createScenarioRecord(db, input).pipe(
        Effect.withSpan('MulletRepository.createScenario'),
      ),
    listScenarios: ownerUserId =>
      listScenarioRecords(db, ownerUserId).pipe(
        Effect.withSpan('MulletRepository.listScenarios'),
      ),
    getScenario: (ownerUserId, scenarioId) =>
      getScenarioRecord(db, ownerUserId, scenarioId).pipe(
        Effect.withSpan('MulletRepository.getScenario'),
      ),
    createSimulationRun: input =>
      createSimulationRunRecord(db, runtime, input).pipe(
        Effect.withSpan('MulletRepository.createSimulationRun'),
      ),
    getSimulationRun: (ownerUserId, runId) =>
      getSimulationRunRecord(db, ownerUserId, runId).pipe(
        Effect.withSpan('MulletRepository.getSimulationRun'),
      ),
    listRunCandidateModes: (ownerUserId, runId) =>
      listRunCandidateModeRecords(db, ownerUserId, runId).pipe(
        Effect.withSpan('MulletRepository.listRunCandidateModes'),
      ),
    createRunExport: input =>
      createRunExportRecord(db, runtime, input).pipe(
        Effect.withSpan('MulletRepository.createRunExport'),
      ),
    getLatestRunExport: (ownerUserId, runId) =>
      getLatestRunExportRecord(db, ownerUserId, runId).pipe(
        Effect.withSpan('MulletRepository.getLatestRunExport'),
      ),
  }
}
