import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import {
  AutopilotSiteProjectNotFound,
  AutopilotSiteStorageError,
  AutopilotSiteUnsafePayload,
} from './sites'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const SiteProvisioningD1Need = S.Struct({
  bindingName: S.String,
  migrationRef: S.optionalKey(S.String),
  retentionPolicy: S.optionalKey(S.String),
  scope: S.optionalKey(S.Literals(['per_site', 'shared_namespace'])),
})
export type SiteProvisioningD1Need = typeof SiteProvisioningD1Need.Type

export const SiteProvisioningR2Need = S.Struct({
  bindingName: S.String,
  prefix: S.optionalKey(S.String),
  retentionPolicy: S.optionalKey(S.String),
  scope: S.optionalKey(S.Literals(['per_site_prefix', 'shared_bucket'])),
})
export type SiteProvisioningR2Need = typeof SiteProvisioningR2Need.Type

export const SiteProvisioningKvNeed = S.Struct({
  bindingName: S.String,
  namespaceRef: S.optionalKey(S.String),
  retentionPolicy: S.optionalKey(S.String),
  scope: S.optionalKey(S.Literals(['per_site_namespace', 'shared_namespace'])),
})
export type SiteProvisioningKvNeed = typeof SiteProvisioningKvNeed.Type

export const SiteProvisioningEnvNeed = S.Struct({
  key: S.String,
  kind: S.Literals(['plain', 'secret']),
  plainValue: S.optionalKey(S.String),
  required: S.optionalKey(S.Boolean),
  secretRef: S.optionalKey(S.String),
})
export type SiteProvisioningEnvNeed = typeof SiteProvisioningEnvNeed.Type

export const SiteProvisioningManifest = S.Struct({
  d1: S.optionalKey(S.Array(SiteProvisioningD1Need)),
  env: S.optionalKey(S.Array(SiteProvisioningEnvNeed)),
  kv: S.optionalKey(S.Array(SiteProvisioningKvNeed)),
  r2: S.optionalKey(S.Array(SiteProvisioningR2Need)),
})
export type SiteProvisioningManifest = typeof SiteProvisioningManifest.Type

export const SiteProvisioningPlanRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  createdAt: S.String,
  id: S.String,
  idempotencyKey: S.String,
  receipt: S.Record(S.String, S.Unknown),
  requestedByUserId: S.NullOr(S.String),
  resourceManifest: SiteProvisioningManifest,
  reviewedAt: S.NullOr(S.String),
  reviewedByUserId: S.NullOr(S.String),
  siteId: S.String,
  status: S.Literals(['review_required', 'approved']),
  updatedAt: S.String,
})
export type SiteProvisioningPlanRecord =
  typeof SiteProvisioningPlanRecord.Type

export type RecordSiteProvisioningPlanInput = Readonly<{
  id?: string | undefined
  idempotencyKey: string
  receipt?: Readonly<Record<string, unknown>> | undefined
  requestedByUserId?: string | undefined
  resourceManifest: SiteProvisioningManifest
  reviewedByUserId?: string | undefined
  siteId: string
}>

export type SiteProvisioningRuntime = Readonly<{
  nowIso: () => string
  randomId: (prefix: string) => string
}>

export const systemSiteProvisioningRuntime: SiteProvisioningRuntime = {
  nowIso: currentIsoTimestamp,
  randomId: compactRandomId,
}

type SiteProvisioningPlanRow = Readonly<{
  archived_at: string | null
  created_at: string
  id: string
  idempotency_key: string
  receipt_json: string
  requested_by_user_id: string | null
  resource_manifest_json: string
  reviewed_at: string | null
  reviewed_by_user_id: string | null
  site_id: string
  status: 'review_required' | 'approved'
  updated_at: string
}>

const planFromRow = (row: SiteProvisioningPlanRow): SiteProvisioningPlanRecord => ({
  archivedAt: row.archived_at,
  createdAt: row.created_at,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  receipt: parseJsonRecord(row.receipt_json) ?? {},
  requestedByUserId: row.requested_by_user_id,
  resourceManifest: S.decodeUnknownSync(SiteProvisioningManifest)(
    parseJsonRecord(row.resource_manifest_json) ?? {},
  ),
  reviewedAt: row.reviewed_at,
  reviewedByUserId: row.reviewed_by_user_id,
  siteId: row.site_id,
  status: row.status,
  updatedAt: row.updated_at,
})

const rejectIfSecret = (
  value: string | undefined,
  reason: string,
): Effect.Effect<void, AutopilotSiteUnsafePayload> =>
  value !== undefined && containsProviderSecretMaterial(value)
    ? Effect.fail(new AutopilotSiteUnsafePayload({ reason }))
    : Effect.void

const validateEnvironmentNeed = (
  need: SiteProvisioningEnvNeed,
): Effect.Effect<void, AutopilotSiteUnsafePayload> =>
  Effect.gen(function* () {
    if (need.kind === 'secret' && need.plainValue !== undefined) {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Secret environment needs may not include plain values.',
      })
    }

    if (need.kind === 'secret' && need.secretRef === undefined) {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Secret environment needs require a secret reference.',
      })
    }

    yield* rejectIfSecret(
      need.plainValue,
      'Plain environment provisioning value contains secret-shaped material.',
    )
    yield* rejectIfSecret(
      need.secretRef,
      'Secret environment provisioning reference contains secret-shaped material.',
    )
  })

const validateManifest = (
  manifest: SiteProvisioningManifest,
): Effect.Effect<SiteProvisioningManifest, AutopilotSiteUnsafePayload> =>
  Effect.gen(function* () {
    for (const env of manifest.env ?? []) {
      yield* validateEnvironmentNeed(env)
    }

    const manifestJson = JSON.stringify(manifest)

    if (containsProviderSecretMaterial(manifestJson)) {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Site provisioning manifest contains secret-shaped material.',
      })
    }

    return manifest
  })

const readPlanByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<SiteProvisioningPlanRecord | null, AutopilotSiteStorageError> =>
  Effect.tryPromise({
    try: () =>
      db
        .prepare(
          `SELECT *
             FROM site_provisioning_plans
            WHERE idempotency_key = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(idempotencyKey)
        .first<SiteProvisioningPlanRow>(),
    catch: error =>
      new AutopilotSiteStorageError({
        error,
        operation: 'siteProvisioning.readByIdempotencyKey',
      }),
  }).pipe(Effect.map(row => (row === null ? null : planFromRow(row))))

const assertSiteExists = (
  db: D1Database,
  siteId: string,
): Effect.Effect<void, AutopilotSiteProjectNotFound | AutopilotSiteStorageError> =>
  Effect.tryPromise({
    try: () =>
      db
        .prepare(
          `SELECT id
             FROM site_projects
            WHERE id = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(siteId)
        .first<{ id: string }>(),
    catch: error =>
      new AutopilotSiteStorageError({
        error,
        operation: 'siteProvisioning.assertSiteExists',
      }),
  }).pipe(
    Effect.flatMap(row =>
      row === null
        ? Effect.fail(new AutopilotSiteProjectNotFound({ siteId }))
        : Effect.void,
    ),
  )

export const recordSiteProvisioningPlan = (
  db: D1Database,
  input: RecordSiteProvisioningPlanInput,
  runtime: SiteProvisioningRuntime = systemSiteProvisioningRuntime,
): Effect.Effect<
  SiteProvisioningPlanRecord,
  AutopilotSiteProjectNotFound | AutopilotSiteStorageError | AutopilotSiteUnsafePayload
> =>
  Effect.gen(function* () {
    const existing = yield* readPlanByIdempotencyKey(db, input.idempotencyKey)

    if (existing !== null) {
      return existing
    }

    yield* assertSiteExists(db, input.siteId)
    const resourceManifest = yield* validateManifest(input.resourceManifest)
    const receipt = input.receipt ?? {}
    const receiptJson = JSON.stringify(receipt)

    if (containsProviderSecretMaterial(receiptJson)) {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Site provisioning receipt contains secret-shaped material.',
      })
    }

    const now = runtime.nowIso()
    const reviewedByUserId = input.reviewedByUserId ?? null
    const status = reviewedByUserId === null ? 'review_required' : 'approved'

    yield* Effect.tryPromise({
      try: () =>
        db
          .prepare(
            `INSERT OR IGNORE INTO site_provisioning_plans
               (id,
                idempotency_key,
                site_id,
                status,
                requested_by_user_id,
                reviewed_by_user_id,
                resource_manifest_json,
                receipt_json,
                created_at,
                reviewed_at,
                updated_at,
                archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
          )
          .bind(
            input.id ?? runtime.randomId('site_provisioning_plan'),
            input.idempotencyKey,
            input.siteId,
            status,
            input.requestedByUserId ?? null,
            reviewedByUserId,
            JSON.stringify(resourceManifest),
            receiptJson,
            now,
            reviewedByUserId === null ? null : now,
            now,
          )
          .run(),
      catch: error =>
        new AutopilotSiteStorageError({
          error,
          operation: 'siteProvisioning.insert',
        }),
    })

    const inserted = yield* readPlanByIdempotencyKey(db, input.idempotencyKey)

    if (inserted === null) {
      return yield* new AutopilotSiteStorageError({
        error: new Error('Provisioning plan was not readable after insert.'),
        operation: 'siteProvisioning.readInserted',
      })
    }

    return inserted
  })
