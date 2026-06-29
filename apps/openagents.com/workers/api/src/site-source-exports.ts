import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import {
  compactRandomId,
  currentIsoTimestamp,
  isoTimestampAfterIso,
} from './runtime-primitives'

export const SiteSourceExportStatus = S.Literals([
  'requested',
  'approved',
  'exported',
  'failed',
  'expired',
  'revoked',
])
export type SiteSourceExportStatus = typeof SiteSourceExportStatus.Type

export const SiteSourceExportKind = S.Literals([
  'download_token',
  'github_branch',
  'github_pull_request',
])
export type SiteSourceExportKind = typeof SiteSourceExportKind.Type

export const SiteSourceExportSecretScanStatus = S.Literals([
  'passed',
  'failed',
])
export type SiteSourceExportSecretScanStatus =
  typeof SiteSourceExportSecretScanStatus.Type

export const SiteSourceExportDestinationProvider = S.Literals([
  'github',
  'download',
])
export type SiteSourceExportDestinationProvider =
  typeof SiteSourceExportDestinationProvider.Type

export const SiteSourceExportDestination = S.Struct({
  provider: SiteSourceExportDestinationProvider,
  owner: S.optionalKey(S.String),
  repository: S.optionalKey(S.String),
  branch: S.optionalKey(S.String),
  pullRequestUrl: S.optionalKey(S.String),
  url: S.optionalKey(S.String),
})
export type SiteSourceExportDestination =
  typeof SiteSourceExportDestination.Type

export const SiteSourceExportSecretScan = S.Struct({
  status: SiteSourceExportSecretScanStatus,
  scannerRef: S.optionalKey(S.String),
  summary: S.optionalKey(S.String),
})
export type SiteSourceExportSecretScan =
  typeof SiteSourceExportSecretScan.Type

export const SiteSourceExportReceipt = S.Struct({
  id: S.String,
  idempotencyKey: S.String,
  siteId: S.String,
  versionId: S.String,
  status: SiteSourceExportStatus,
  exportKind: SiteSourceExportKind,
  actorUserId: S.NullOr(S.String),
  approvedByUserId: S.NullOr(S.String),
  destination: SiteSourceExportDestination,
  sourceArchiveR2Key: S.NullOr(S.String),
  artifactManifestR2Key: S.NullOr(S.String),
  workerModuleR2Key: S.NullOr(S.String),
  sourceArtifactRef: S.NullOr(S.String),
  tokenRef: S.NullOr(S.String),
  tokenExpiresAt: S.NullOr(S.String),
  secretScanStatus: SiteSourceExportSecretScanStatus,
  secretScanRef: S.NullOr(S.String),
  receipt: S.Record(S.String, S.Unknown),
  createdAt: S.String,
  updatedAt: S.String,
  archivedAt: S.NullOr(S.String),
})
export type SiteSourceExportReceipt = typeof SiteSourceExportReceipt.Type

export type SiteSourceExportRuntime = Readonly<{
  makeExportId: () => string
  makeTokenHash: () => string
  makeTokenRef: () => string
  nowIso: () => string
}>

export const systemSiteSourceExportRuntime: SiteSourceExportRuntime = {
  makeExportId: () => compactRandomId('site_source_export'),
  makeTokenHash: () => compactRandomId('site_source_export_token_hash'),
  makeTokenRef: () => compactRandomId('site_source_export_token'),
  nowIso: currentIsoTimestamp,
}

export type RecordSiteSourceExportInput = Readonly<{
  actorUserId?: string | undefined
  approve?: boolean | undefined
  destination: SiteSourceExportDestination
  exportKind: SiteSourceExportKind
  expiresInSeconds?: number | undefined
  id?: string | undefined
  idempotencyKey: string
  receipt?: Readonly<Record<string, unknown>> | undefined
  secretScan: SiteSourceExportSecretScan
  siteId: string
  sourceArtifactRef?: string | undefined
  versionId: string
}>

export class SiteSourceExportStorageError extends S.TaggedErrorClass<SiteSourceExportStorageError>()(
  'SiteSourceExportStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export class SiteSourceExportValidationError extends S.TaggedErrorClass<SiteSourceExportValidationError>()(
  'SiteSourceExportValidationError',
  {
    reason: S.String,
  },
) {}

export type SiteSourceExportError =
  | SiteSourceExportStorageError
  | SiteSourceExportValidationError

type SiteVersionSourceRow = Readonly<{
  artifact_manifest_r2_key: string | null
  build_status: string
  id: string
  site_id: string
  source_archive_r2_key: string | null
  worker_module_r2_key: string | null
}>

type SiteSourceExportRow = Readonly<{
  actor_user_id: string | null
  approved_by_user_id: string | null
  archived_at: string | null
  artifact_manifest_r2_key: string | null
  created_at: string
  destination_branch: string | null
  destination_owner: string | null
  destination_provider: SiteSourceExportDestinationProvider
  destination_pull_request_url: string | null
  destination_repository: string | null
  destination_url: string | null
  export_kind: SiteSourceExportKind
  id: string
  idempotency_key: string
  receipt_json: string
  secret_scan_ref: string | null
  secret_scan_status: SiteSourceExportSecretScanStatus
  site_id: string
  source_archive_r2_key: string | null
  source_artifact_ref: string | null
  status: SiteSourceExportStatus
  token_expires_at: string | null
  token_hash: string | null
  token_ref: string | null
  updated_at: string
  version_id: string
  worker_module_r2_key: string | null
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/@-]{0,240}$/
const SAFE_GITHUB_NAME_PATTERN = /^[A-Za-z0-9_.-]{1,100}$/
const SAFE_BRANCH_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.\/-]{0,180}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(access_token|refresh_token|device_auth_id|code_verifier|private_key|wallet_secret|webhook_secret|payment_secret|payment_preimage|gho_[a-z0-9_]+|xprv|mnemonic|lnbc|lntb|lno1)\b/i

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new SiteSourceExportValidationError({
      reason: `${field} must be a bounded public-safe reference.`,
    })
  }
}

const assertSafeGithubName = (
  field: string,
  value: string | undefined,
): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_GITHUB_NAME_PATTERN.test(value) || !textIsSafe(value)) {
    throw new SiteSourceExportValidationError({
      reason: `${field} must be a bounded GitHub owner or repository name.`,
    })
  }
}

const assertSafeBranch = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_BRANCH_PATTERN.test(value) || !textIsSafe(value)) {
    throw new SiteSourceExportValidationError({
      reason: `${field} must be a bounded branch ref.`,
    })
  }
}

const assertSafeUrl = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  let parsed: URL

  try {
    parsed = new URL(value)
  } catch {
    throw new SiteSourceExportValidationError({
      reason: `${field} must be a valid https URL.`,
    })
  }

  if (parsed.protocol !== 'https:' || !textIsSafe(value)) {
    throw new SiteSourceExportValidationError({
      reason: `${field} must be a public-safe https URL.`,
    })
  }
}

const assertSafeReceipt = (
  receipt: Readonly<Record<string, unknown>> | undefined,
): Readonly<Record<string, unknown>> => {
  const safeReceipt = receipt ?? {}
  const text = JSON.stringify(safeReceipt)

  if (!textIsSafe(text)) {
    throw new SiteSourceExportValidationError({
      reason: 'receipt must not contain token, provider, wallet, or payment secrets.',
    })
  }

  return safeReceipt
}

const assertValidInput = (
  input: RecordSiteSourceExportInput,
): Readonly<{ expiresInSeconds: number; receipt: Readonly<Record<string, unknown>> }> => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('siteId', input.siteId)
  assertSafeRef('versionId', input.versionId)
  assertSafeRef('actorUserId', input.actorUserId)
  assertSafeRef('sourceArtifactRef', input.sourceArtifactRef)
  assertSafeRef('secretScan.scannerRef', input.secretScan.scannerRef)
  assertSafeGithubName('destination.owner', input.destination.owner)
  assertSafeGithubName('destination.repository', input.destination.repository)
  assertSafeBranch('destination.branch', input.destination.branch)
  assertSafeUrl('destination.pullRequestUrl', input.destination.pullRequestUrl)
  assertSafeUrl('destination.url', input.destination.url)

  if (input.secretScan.status !== 'passed') {
    throw new SiteSourceExportValidationError({
      reason: 'source export requires a passed secret scan.',
    })
  }

  if (
    input.exportKind !== 'download_token' &&
    input.destination.provider !== 'github'
  ) {
    throw new SiteSourceExportValidationError({
      reason: 'GitHub exports require a GitHub destination.',
    })
  }

  if (
    input.destination.provider === 'github' &&
    (input.destination.owner === undefined ||
      input.destination.repository === undefined)
  ) {
    throw new SiteSourceExportValidationError({
      reason: 'GitHub exports require destination owner and repository.',
    })
  }

  const expiresInSeconds = input.expiresInSeconds ?? 86_400

  if (
    !Number.isSafeInteger(expiresInSeconds) ||
    expiresInSeconds < 900 ||
    expiresInSeconds > 604_800
  ) {
    throw new SiteSourceExportValidationError({
      reason: 'export token expiry must be between 15 minutes and 7 days.',
    })
  }

  return { expiresInSeconds, receipt: assertSafeReceipt(input.receipt) }
}

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, SiteSourceExportStorageError> =>
  Effect.tryPromise({
    catch: error =>
      new SiteSourceExportStorageError({
        operation,
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: run,
  })

const validationEffect = <A>(validate: () => A) =>
  Effect.try({
    catch: error =>
      error instanceof SiteSourceExportValidationError
        ? error
        : new SiteSourceExportValidationError({
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: validate,
  })

const exportFromRow = (row: SiteSourceExportRow): SiteSourceExportReceipt => ({
  actorUserId: row.actor_user_id,
  approvedByUserId: row.approved_by_user_id,
  archivedAt: row.archived_at,
  artifactManifestR2Key: row.artifact_manifest_r2_key,
  createdAt: row.created_at,
  destination: {
    provider: row.destination_provider,
    ...(row.destination_owner === null ? {} : { owner: row.destination_owner }),
    ...(row.destination_repository === null
      ? {}
      : { repository: row.destination_repository }),
    ...(row.destination_branch === null
      ? {}
      : { branch: row.destination_branch }),
    ...(row.destination_pull_request_url === null
      ? {}
      : { pullRequestUrl: row.destination_pull_request_url }),
    ...(row.destination_url === null ? {} : { url: row.destination_url }),
  },
  exportKind: row.export_kind,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  receipt: parseJsonRecord(row.receipt_json) ?? {},
  secretScanRef: row.secret_scan_ref,
  secretScanStatus: row.secret_scan_status,
  siteId: row.site_id,
  sourceArchiveR2Key: row.source_archive_r2_key,
  sourceArtifactRef: row.source_artifact_ref,
  status: row.status,
  tokenExpiresAt: row.token_expires_at,
  tokenRef: row.token_ref,
  updatedAt: row.updated_at,
  versionId: row.version_id,
  workerModuleR2Key: row.worker_module_r2_key,
})

const readByIdempotencyKey = (db: D1Database, idempotencyKey: string) =>
  d1Effect('siteSourceExports.readByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM site_source_exports
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<SiteSourceExportRow>(),
  ).pipe(Effect.map(row => (row === null ? null : exportFromRow(row))))

const readVersionSource = (
  db: D1Database,
  siteId: string,
  versionId: string,
) =>
  d1Effect('siteSourceExports.readVersionSource', () =>
    db
      .prepare(
        `SELECT id,
                site_id,
                build_status,
                source_archive_r2_key,
                artifact_manifest_r2_key,
                worker_module_r2_key
           FROM site_versions
          WHERE id = ?
            AND site_id = ?
          LIMIT 1`,
      )
      .bind(versionId, siteId)
      .first<SiteVersionSourceRow>(),
  )

const expiryFrom = (nowIso: string, seconds: number): string =>
  isoTimestampAfterIso(nowIso, seconds * 1000)

export const recordSiteSourceExport = (
  db: D1Database,
  input: RecordSiteSourceExportInput,
  runtime: SiteSourceExportRuntime = systemSiteSourceExportRuntime,
): Effect.Effect<SiteSourceExportReceipt, SiteSourceExportError> =>
  Effect.gen(function* () {
    const validated = yield* validationEffect(() => assertValidInput(input))
    const existing = yield* readByIdempotencyKey(db, input.idempotencyKey)

    if (existing !== null) {
      return existing
    }

    const version = yield* readVersionSource(db, input.siteId, input.versionId)

    if (version === null) {
      return yield* new SiteSourceExportValidationError({
        reason: 'site version was not found.',
      })
    }

    if (
      version.source_archive_r2_key === null &&
      version.artifact_manifest_r2_key === null &&
      version.worker_module_r2_key === null &&
      input.sourceArtifactRef === undefined
    ) {
      return yield* new SiteSourceExportValidationError({
        reason: 'source export requires a source archive, artifact manifest, worker module, or explicit source artifact ref.',
      })
    }

    const now = runtime.nowIso()
    const tokenRef = runtime.makeTokenRef()
    const tokenHash = runtime.makeTokenHash()
    const tokenExpiresAt = expiryFrom(now, validated.expiresInSeconds)
    const status: SiteSourceExportStatus =
      input.approve === true ? 'approved' : 'requested'

    yield* d1Effect('siteSourceExports.insert', () =>
      db
        .prepare(
          `INSERT INTO site_source_exports (
             id,
             idempotency_key,
             site_id,
             version_id,
             status,
             export_kind,
             actor_user_id,
             approved_by_user_id,
             destination_provider,
             destination_owner,
             destination_repository,
             destination_branch,
             destination_pull_request_url,
             destination_url,
             source_archive_r2_key,
             artifact_manifest_r2_key,
             worker_module_r2_key,
             source_artifact_ref,
             token_ref,
             token_hash,
             token_expires_at,
             secret_scan_status,
             secret_scan_ref,
             receipt_json,
             created_at,
             updated_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.id ?? runtime.makeExportId(),
          input.idempotencyKey,
          input.siteId,
          input.versionId,
          status,
          input.exportKind,
          input.actorUserId ?? null,
          input.approve === true ? (input.actorUserId ?? null) : null,
          input.destination.provider,
          input.destination.owner ?? null,
          input.destination.repository ?? null,
          input.destination.branch ?? null,
          input.destination.pullRequestUrl ?? null,
          input.destination.url ?? null,
          version.source_archive_r2_key,
          version.artifact_manifest_r2_key,
          version.worker_module_r2_key,
          input.sourceArtifactRef ?? null,
          tokenRef,
          tokenHash,
          tokenExpiresAt,
          input.secretScan.status,
          input.secretScan.scannerRef ?? null,
          JSON.stringify(validated.receipt),
          now,
          now,
        )
        .run(),
    )

    const inserted = yield* readByIdempotencyKey(db, input.idempotencyKey)

    if (inserted === null) {
      return yield* new SiteSourceExportStorageError({
        operation: 'siteSourceExports.readInserted',
        reason: 'source export was not readable after insert.',
      })
    }

    return inserted
  })
