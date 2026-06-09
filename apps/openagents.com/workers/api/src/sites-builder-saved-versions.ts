import { Effect, Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import {
  type AutopilotSiteError,
  type AutopilotSiteStaticAssetsManifest,
  type AutopilotSiteVersion,
  type AutopilotSitesRuntime,
  AutopilotSitesService,
  SaveAutopilotSiteVersionInput,
  systemAutopilotSitesRuntime,
} from './sites'
import {
  type SiteBuilderRuntime,
  SiteBuilderSessionStorageError,
  SiteBuilderSessionValidationError,
  appendSiteBuilderEvent,
  readSiteBuilderSessionProjection,
  systemSiteBuilderRuntime,
} from './sites-builder-sessions'

const BuilderSavedVersionMetadata = S.Record(S.String, S.Unknown)

export const SiteBuilderSavedVersionRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  artifactRef: S.NullOr(S.String),
  buildReceiptRef: S.NullOr(S.String),
  createdAt: S.String,
  id: S.String,
  idempotencyKey: S.String,
  notes: S.NullOr(S.String),
  previewId: S.NullOr(S.String),
  sessionId: S.String,
  siteId: S.String,
  siteMetadata: BuilderSavedVersionMetadata,
  siteVersionId: S.String,
  sourceHash: S.NullOr(S.String),
})
export type SiteBuilderSavedVersionRecord =
  typeof SiteBuilderSavedVersionRecord.Type

export const SaveSiteBuilderVersionInput = S.Struct({
  idempotencyKey: S.String,
  sessionId: S.String,
  siteId: S.String,
  staticAssetsManifest: SaveAutopilotSiteVersionInput.fields.staticAssetsManifest,
  actorRunId: S.optionalKey(S.String),
  actorUserId: S.optionalKey(S.String),
  artifactRef: S.optionalKey(S.String),
  buildCommand: S.optionalKey(S.String),
  buildLogText: S.optionalKey(S.String),
  buildReceiptRef: S.optionalKey(S.String),
  d1BindingName: S.optionalKey(S.String),
  notes: S.optionalKey(S.String),
  previewId: S.optionalKey(S.String),
  r2BindingName: S.optionalKey(S.String),
  siteMetadata: S.optionalKey(BuilderSavedVersionMetadata),
  sourceArchiveText: S.optionalKey(S.String),
  sourceCommitSha: S.optionalKey(S.String),
  sourceHash: S.optionalKey(S.String),
  workerModuleR2Key: S.optionalKey(S.String),
  workerModuleText: S.optionalKey(S.String),
})
export type SaveSiteBuilderVersionInput =
  typeof SaveSiteBuilderVersionInput.Type

export type SaveSiteBuilderVersionResult = Readonly<{
  savedVersion: SiteBuilderSavedVersionRecord
  version: AutopilotSiteVersion | null
}>

type SavedVersionRow = Readonly<{
  archived_at: string | null
  artifact_ref: string | null
  build_receipt_ref: string | null
  created_at: string
  id: string
  idempotency_key: string
  notes: string | null
  preview_id: string | null
  session_id: string
  site_id: string
  site_metadata_json: string
  site_version_id: string
  source_hash: string | null
}>

const savedVersionFromRow = (
  row: SavedVersionRow,
): SiteBuilderSavedVersionRecord => ({
  archivedAt: row.archived_at,
  artifactRef: row.artifact_ref,
  buildReceiptRef: row.build_receipt_ref,
  createdAt: row.created_at,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  notes: row.notes,
  previewId: row.preview_id,
  sessionId: row.session_id,
  siteId: row.site_id,
  siteMetadata: parseJsonRecord(row.site_metadata_json) ?? {},
  siteVersionId: row.site_version_id,
  sourceHash: row.source_hash,
})

const safeOptional = (value: string | undefined): string | undefined => {
  const trimmed = value?.trim()

  return trimmed === undefined || trimmed === '' ? undefined : trimmed
}

const validatedInput = (
  input: SaveSiteBuilderVersionInput,
): Effect.Effect<SaveSiteBuilderVersionInput, SiteBuilderSessionValidationError> =>
  Effect.try({
    try: () => S.decodeUnknownSync(SaveSiteBuilderVersionInput)(input),
    catch: error =>
      new SiteBuilderSessionValidationError({
        reason:
          error instanceof Error
            ? error.message
            : 'Invalid builder saved version input.',
      }),
  })

const readSavedVersionByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<SiteBuilderSavedVersionRecord | null, SiteBuilderSessionStorageError> =>
  Effect.tryPromise({
    try: () =>
      db
        .prepare(
          `SELECT *
             FROM site_builder_saved_versions
            WHERE idempotency_key = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(idempotencyKey)
        .first<SavedVersionRow>(),
    catch: error =>
      new SiteBuilderSessionStorageError({
        operation: 'saveSiteBuilderVersion.readByIdempotencyKey',
        reason: error instanceof Error ? error.message : String(error),
      }),
  }).pipe(Effect.map(row => (row === null ? null : savedVersionFromRow(row))))

const insertSavedVersionMapping = (
  db: D1Database,
  record: SiteBuilderSavedVersionRecord,
): Effect.Effect<void, SiteBuilderSessionStorageError> =>
  Effect.tryPromise({
    try: () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO site_builder_saved_versions (
             id,
             idempotency_key,
             session_id,
             site_id,
             site_version_id,
             preview_id,
             artifact_ref,
             build_receipt_ref,
             source_hash,
             notes,
             site_metadata_json,
             created_at,
             archived_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          record.id,
          record.idempotencyKey,
          record.sessionId,
          record.siteId,
          record.siteVersionId,
          record.previewId,
          record.artifactRef,
          record.buildReceiptRef,
          record.sourceHash,
          record.notes,
          JSON.stringify(record.siteMetadata),
          record.createdAt,
        )
        .run(),
    catch: error =>
      new SiteBuilderSessionStorageError({
        operation: 'saveSiteBuilderVersion.insert',
        reason: error instanceof Error ? error.message : String(error),
      }),
  }).pipe(Effect.asVoid)

const saveMetadata = (
  input: SaveSiteBuilderVersionInput,
  sessionOrderId: string | null,
): Readonly<Record<string, unknown>> => ({
  ...(input.siteMetadata ?? {}),
  builder: {
    artifactRef: safeOptional(input.artifactRef) ?? null,
    buildReceiptRef: safeOptional(input.buildReceiptRef) ?? null,
    idempotencyKey: input.idempotencyKey,
    notes: safeOptional(input.notes) ?? null,
    orderId: sessionOrderId,
    previewId: safeOptional(input.previewId) ?? null,
    sessionId: input.sessionId,
    sourceHash: safeOptional(input.sourceHash) ?? null,
  },
})

export const saveSiteBuilderVersion = (
  db: D1Database,
  artifacts: R2Bucket | undefined,
  input: SaveSiteBuilderVersionInput,
  runtime: AutopilotSitesRuntime = systemAutopilotSitesRuntime,
  builderRuntime: SiteBuilderRuntime = systemSiteBuilderRuntime,
): Effect.Effect<
  SaveSiteBuilderVersionResult,
  AutopilotSiteError | SiteBuilderSessionStorageError | SiteBuilderSessionValidationError
> =>
  Effect.gen(function* () {
    const valid = yield* validatedInput(input)
    const existing = yield* readSavedVersionByIdempotencyKey(
      db,
      valid.idempotencyKey,
    )

    if (existing !== null) {
      return {
        savedVersion: existing,
        version: null,
      }
    }

    const session = yield* readSiteBuilderSessionProjection(db, valid.sessionId)

    if (
      session.operator.siteId !== null &&
      session.operator.siteId !== valid.siteId
    ) {
      return yield* new SiteBuilderSessionValidationError({
        reason: 'Builder session is linked to a different Site.',
      })
    }

    const sites = AutopilotSitesService.fromBindings(db, artifacts, runtime)
    const version = yield* sites.saveVersion({
      siteId: valid.siteId,
      sourceKind: 'autopilot_generated',
      buildStatus: 'saved',
      staticAssetsManifest: valid.staticAssetsManifest,
      ...(valid.actorRunId === undefined ? {} : { actorRunId: valid.actorRunId }),
      ...(valid.actorUserId === undefined
        ? {}
        : { actorUserId: valid.actorUserId }),
      ...(valid.buildCommand === undefined
        ? {}
        : { buildCommand: valid.buildCommand }),
      ...(valid.buildLogText === undefined
        ? {}
        : { buildLogText: valid.buildLogText }),
      ...(valid.d1BindingName === undefined
        ? {}
        : { d1BindingName: valid.d1BindingName }),
      metadata: saveMetadata(valid, session.operator.orderId),
      ...(valid.r2BindingName === undefined
        ? {}
        : { r2BindingName: valid.r2BindingName }),
      ...(valid.sourceArchiveText === undefined
        ? {}
        : { sourceArchiveText: valid.sourceArchiveText }),
      ...(valid.sourceCommitSha === undefined
        ? {}
        : { sourceCommitSha: valid.sourceCommitSha }),
      ...(valid.workerModuleR2Key === undefined
        ? {}
        : { workerModuleR2Key: valid.workerModuleR2Key }),
      ...(valid.workerModuleText === undefined
        ? {}
        : { workerModuleText: valid.workerModuleText }),
    })

    const now = runtime.nowIso()
    const savedVersion: SiteBuilderSavedVersionRecord = {
      archivedAt: null,
      artifactRef: safeOptional(valid.artifactRef) ?? null,
      buildReceiptRef: safeOptional(valid.buildReceiptRef) ?? null,
      createdAt: now,
      id: builderRuntime.randomId('site_builder_saved_version'),
      idempotencyKey: valid.idempotencyKey,
      notes: safeOptional(valid.notes) ?? null,
      previewId: safeOptional(valid.previewId) ?? null,
      sessionId: valid.sessionId,
      siteId: valid.siteId,
      siteMetadata: valid.siteMetadata ?? {},
      siteVersionId: version.id,
      sourceHash: safeOptional(valid.sourceHash) ?? null,
    }

    yield* insertSavedVersionMapping(db, savedVersion)
    yield* appendSiteBuilderEvent(
      db,
      {
        eventKind: 'save_requested',
        idempotencyKey: `${valid.idempotencyKey}:event`,
        payload: {
          artifactRef: savedVersion.artifactRef,
          buildReceiptRef: savedVersion.buildReceiptRef,
          previewId: savedVersion.previewId,
          siteId: savedVersion.siteId,
          siteVersionId: savedVersion.siteVersionId,
          sourceHash: savedVersion.sourceHash,
        },
        phaseKind: 'save',
        sessionId: valid.sessionId,
        sourceRef: version.id,
        status: 'succeeded',
        summary: `Saved reviewable Site version ${version.id}.`,
        title: 'Saved review candidate',
        visibility: 'customer',
      },
      builderRuntime,
    )

    return {
      savedVersion,
      version,
    }
  })

export const emptyStaticAssetsManifest =
  (): AutopilotSiteStaticAssetsManifest => ({
    assets: {},
  })
