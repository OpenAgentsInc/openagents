import { notFound } from '@openagentsinc/sync-worker'
import { Data, Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonUnknown } from './json-boundary'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export const KHALA_CODE_OUTSIDE_USER_RUNS_ENDPOINT =
  '/api/public/khala-code/outside-user-runs' as const
export const KHALA_CODE_OUTSIDE_USER_RUNS_RECEIPT_PATH =
  '/api/public/khala-code/outside-user-runs/:receiptRef' as const
export const KHALA_CODE_OUTSIDE_USER_RUNS_TABLE =
  'khala_code_outside_user_run_receipts' as const

export const KhalaCodeOutsideUserRunPlatform = S.Literals([
  'darwin',
  'linux',
  'win32',
  'other',
])
export type KhalaCodeOutsideUserRunPlatform =
  typeof KhalaCodeOutsideUserRunPlatform.Type

export const KhalaCodeOutsideUserRunArch = S.Literals([
  'arm64',
  'x64',
  'other',
])
export type KhalaCodeOutsideUserRunArch =
  typeof KhalaCodeOutsideUserRunArch.Type

export const KhalaCodeOutsideUserRunDistributionChannel = S.Literals([
  'desktop_dmg',
  'npm_cli',
  'source_build',
  'unknown',
])
export type KhalaCodeOutsideUserRunDistributionChannel =
  typeof KhalaCodeOutsideUserRunDistributionChannel.Type

export const KhalaCodeOutsideUserRunCodexCliState = S.Literals([
  'ready',
  'missing',
  'unknown',
])
export type KhalaCodeOutsideUserRunCodexCliState =
  typeof KhalaCodeOutsideUserRunCodexCliState.Type

export const KhalaCodeOutsideUserRunCodexAuthState = S.Literals([
  'ready',
  'credentials_missing',
  'invalid',
  'error',
  'unknown',
])
export type KhalaCodeOutsideUserRunCodexAuthState =
  typeof KhalaCodeOutsideUserRunCodexAuthState.Type

export const KhalaCodeOutsideUserRunPylonState = S.Literals([
  'ready',
  'unavailable',
  'not_configured',
  'unknown',
])
export type KhalaCodeOutsideUserRunPylonState =
  typeof KhalaCodeOutsideUserRunPylonState.Type

export const KhalaCodeOutsideUserRunHarnessReadiness = S.Struct({
  codexCli: KhalaCodeOutsideUserRunCodexCliState,
  codexAuth: KhalaCodeOutsideUserRunCodexAuthState,
  pylon: KhalaCodeOutsideUserRunPylonState,
})
export type KhalaCodeOutsideUserRunHarnessReadiness =
  typeof KhalaCodeOutsideUserRunHarnessReadiness.Type

export const KhalaCodeOutsideUserRunConsent = S.Struct({
  publicReceipt: S.Literal(true),
  noPrivateDataIncluded: S.Literal(true),
})

export const KhalaCodeOutsideUserRunIntakeRequest = S.Struct({
  schemaVersion: S.Literal(
    'openagents.khala_code.outside_user_run_intake.v1',
  ),
  consent: KhalaCodeOutsideUserRunConsent,
  appVersion: S.String,
  platform: KhalaCodeOutsideUserRunPlatform,
  arch: KhalaCodeOutsideUserRunArch,
  distributionChannel: KhalaCodeOutsideUserRunDistributionChannel,
  harnessReadiness: KhalaCodeOutsideUserRunHarnessReadiness,
  idempotencyKey: S.optionalKey(S.String),
})
export type KhalaCodeOutsideUserRunIntakeRequest =
  typeof KhalaCodeOutsideUserRunIntakeRequest.Type

export const PublicKhalaCodeOutsideUserRunReceipt = S.Struct({
  schemaVersion: S.Literal(
    'openagents.khala_code.outside_user_run_receipt.v1',
  ),
  product: S.Literal('khala-code'),
  promiseId: S.Literal('khala_code.desktop_codex_wrapper.v1'),
  receiptRef: S.String,
  receiptUrl: S.String,
  generatedAt: S.String,
  submittedAt: S.String,
  appVersion: S.String,
  platform: KhalaCodeOutsideUserRunPlatform,
  arch: KhalaCodeOutsideUserRunArch,
  distributionChannel: KhalaCodeOutsideUserRunDistributionChannel,
  harnessReadiness: KhalaCodeOutsideUserRunHarnessReadiness,
  publicSafety: S.Struct({
    userActionRequired: S.Literal(true),
    noPhoneHome: S.Literal(true),
    noPaths: S.Literal(true),
    noPrompts: S.Literal(true),
    noTokens: S.Literal(true),
    noLogs: S.Literal(true),
  }),
  evidenceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
})
export type PublicKhalaCodeOutsideUserRunReceipt =
  typeof PublicKhalaCodeOutsideUserRunReceipt.Type

export const PublicKhalaCodeOutsideUserRunReceiptIntake = S.Struct({
  ok: S.Literal(true),
  idempotent: S.Boolean,
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
  receipt: PublicKhalaCodeOutsideUserRunReceipt,
})
export type PublicKhalaCodeOutsideUserRunReceiptIntake =
  typeof PublicKhalaCodeOutsideUserRunReceiptIntake.Type

export const PublicKhalaCodeOutsideUserRunReceiptEnvelope = S.Struct({
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
  receipt: PublicKhalaCodeOutsideUserRunReceipt,
})
export type PublicKhalaCodeOutsideUserRunReceiptEnvelope =
  typeof PublicKhalaCodeOutsideUserRunReceiptEnvelope.Type

export type KhalaCodeOutsideUserRunReceiptRecord = Readonly<{
  receiptRef: string
  appVersion: string
  platform: KhalaCodeOutsideUserRunPlatform
  arch: KhalaCodeOutsideUserRunArch
  distributionChannel: KhalaCodeOutsideUserRunDistributionChannel
  harnessReadiness: KhalaCodeOutsideUserRunHarnessReadiness
  submittedAt: string
}>

export type KhalaCodeOutsideUserRunReceiptDraft = Omit<
  KhalaCodeOutsideUserRunReceiptRecord,
  'submittedAt'
> &
  Readonly<{
    idempotencyKey: string
    submittedAt: string
  }>

export type KhalaCodeOutsideUserRunStore = Readonly<{
  recordReceipt: (
    draft: KhalaCodeOutsideUserRunReceiptDraft,
  ) => Effect.Effect<
    Readonly<{
      record: KhalaCodeOutsideUserRunReceiptRecord
      idempotent: boolean
    }>,
    unknown
  >
  readReceipt: (
    receiptRef: string,
  ) => Effect.Effect<KhalaCodeOutsideUserRunReceiptRecord | null, unknown>
}>

type KhalaCodeOutsideUserRunRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  nowIso?: (() => string) | undefined
  store?: KhalaCodeOutsideUserRunStore | undefined
}>

type KhalaCodeOutsideUserRunSqlRow = Readonly<{
  receipt_ref: unknown
  app_version: unknown
  platform: unknown
  arch: unknown
  distribution_channel: unknown
  codex_cli_state: unknown
  codex_auth_state: unknown
  pylon_state: unknown
  submitted_at: unknown
}>

export class KhalaCodeOutsideUserRunStoreUnavailable extends Data.TaggedError(
  'KhalaCodeOutsideUserRunStoreUnavailable',
)<{ readonly reason: string }> {}

const stringValue = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

const isPlatform = (
  value: string | undefined,
): value is KhalaCodeOutsideUserRunPlatform =>
  value === 'darwin' ||
  value === 'linux' ||
  value === 'win32' ||
  value === 'other'

const isArch = (value: string | undefined): value is KhalaCodeOutsideUserRunArch =>
  value === 'arm64' || value === 'x64' || value === 'other'

const isDistributionChannel = (
  value: string | undefined,
): value is KhalaCodeOutsideUserRunDistributionChannel =>
  value === 'desktop_dmg' ||
  value === 'npm_cli' ||
  value === 'source_build' ||
  value === 'unknown'

const isCodexCliState = (
  value: string | undefined,
): value is KhalaCodeOutsideUserRunCodexCliState =>
  value === 'ready' || value === 'missing' || value === 'unknown'

const isCodexAuthState = (
  value: string | undefined,
): value is KhalaCodeOutsideUserRunCodexAuthState =>
  value === 'ready' ||
  value === 'credentials_missing' ||
  value === 'invalid' ||
  value === 'error' ||
  value === 'unknown'

const isPylonState = (
  value: string | undefined,
): value is KhalaCodeOutsideUserRunPylonState =>
  value === 'ready' ||
  value === 'unavailable' ||
  value === 'not_configured' ||
  value === 'unknown'

export const khalaCodeOutsideUserRunReceiptRecordFromSql = (
  row: KhalaCodeOutsideUserRunSqlRow,
): KhalaCodeOutsideUserRunReceiptRecord | null => {
  const receiptRef = stringValue(row.receipt_ref)
  const appVersion = stringValue(row.app_version)
  const platform = stringValue(row.platform)
  const arch = stringValue(row.arch)
  const distributionChannel = stringValue(row.distribution_channel)
  const codexCli = stringValue(row.codex_cli_state)
  const codexAuth = stringValue(row.codex_auth_state)
  const pylon = stringValue(row.pylon_state)
  const submittedAt = stringValue(row.submitted_at)

  if (
    receiptRef === undefined ||
    appVersion === undefined ||
    !isPlatform(platform) ||
    !isArch(arch) ||
    !isDistributionChannel(distributionChannel) ||
    !isCodexCliState(codexCli) ||
    !isCodexAuthState(codexAuth) ||
    !isPylonState(pylon) ||
    submittedAt === undefined
  ) {
    return null
  }

  return {
    receiptRef,
    appVersion,
    platform,
    arch,
    distributionChannel,
    harnessReadiness: { codexCli, codexAuth, pylon },
    submittedAt,
  }
}

const readReceiptSql = `
  SELECT
    receipt_ref,
    app_version,
    platform,
    arch,
    distribution_channel,
    codex_cli_state,
    codex_auth_state,
    pylon_state,
    submitted_at
  FROM khala_code_outside_user_run_receipts
`

const recordFromDbRow = (
  row: KhalaCodeOutsideUserRunSqlRow | null | undefined,
): KhalaCodeOutsideUserRunReceiptRecord | null =>
  row === null || row === undefined
    ? null
    : khalaCodeOutsideUserRunReceiptRecordFromSql(row)

export const makeD1KhalaCodeOutsideUserRunStore = (
  db: D1Database | undefined,
): KhalaCodeOutsideUserRunStore => {
  const readByIdempotencyKey = async (
    idempotencyKey: string,
  ): Promise<KhalaCodeOutsideUserRunReceiptRecord | null> => {
    if (db === undefined) {
      throw new KhalaCodeOutsideUserRunStoreUnavailable({
        reason: 'OPENAGENTS_DB missing',
      })
    }
    const row = await db
      .prepare(`${readReceiptSql} WHERE idempotency_key = ? LIMIT 1`)
      .bind(idempotencyKey)
      .first<KhalaCodeOutsideUserRunSqlRow>()
    return recordFromDbRow(row)
  }

  return {
    recordReceipt: draft =>
      Effect.tryPromise({
        try: async () => {
          if (db === undefined) {
            throw new KhalaCodeOutsideUserRunStoreUnavailable({
              reason: 'OPENAGENTS_DB missing',
            })
          }

          const existing = await readByIdempotencyKey(draft.idempotencyKey)
          if (existing !== null) {
            return { record: existing, idempotent: true }
          }

          await db
            .prepare(
              `
                INSERT INTO khala_code_outside_user_run_receipts (
                  receipt_ref,
                  idempotency_key,
                  app_version,
                  platform,
                  arch,
                  distribution_channel,
                  codex_cli_state,
                  codex_auth_state,
                  pylon_state,
                  submitted_at,
                  created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
              `,
            )
            .bind(
              draft.receiptRef,
              draft.idempotencyKey,
              draft.appVersion,
              draft.platform,
              draft.arch,
              draft.distributionChannel,
              draft.harnessReadiness.codexCli,
              draft.harnessReadiness.codexAuth,
              draft.harnessReadiness.pylon,
              draft.submittedAt,
              draft.submittedAt,
            )
            .run()

          const inserted = await readByIdempotencyKey(draft.idempotencyKey)
          if (inserted === null) {
            throw new KhalaCodeOutsideUserRunStoreUnavailable({
              reason: 'inserted receipt could not be read',
            })
          }
          return { record: inserted, idempotent: false }
        },
        catch: error =>
          new KhalaCodeOutsideUserRunStoreUnavailable({
            reason: error instanceof Error ? error.message : String(error),
          }),
      }),
    readReceipt: receiptRef =>
      Effect.tryPromise({
        try: async () => {
          if (db === undefined) {
            throw new KhalaCodeOutsideUserRunStoreUnavailable({
              reason: 'OPENAGENTS_DB missing',
            })
          }
          const row = await db
            .prepare(`${readReceiptSql} WHERE receipt_ref = ? LIMIT 1`)
            .bind(receiptRef)
            .first<KhalaCodeOutsideUserRunSqlRow>()
          return recordFromDbRow(row)
        },
        catch: error =>
          new KhalaCodeOutsideUserRunStoreUnavailable({
            reason: error instanceof Error ? error.message : String(error),
          }),
      }),
  }
}

const publicSafety = {
  userActionRequired: true,
  noPhoneHome: true,
  noPaths: true,
  noPrompts: true,
  noTokens: true,
  noLogs: true,
} as const

const evidenceRefs = [
  'issue:OpenAgentsInc/openagents#8247',
  'promise:khala_code.desktop_codex_wrapper.v1',
  'docs/fable/2026-07-02-khala-code-business-opportunity-and-openagents-analysis.md',
] as const

const caveatRefs = [
  'caveat.khala_code_outside_user_run.no_signed_installer_claim',
  'caveat.khala_code_outside_user_run.no_payment_claim',
  'caveat.khala_code_outside_user_run.no_capture_or_trace_claim',
  'caveat.khala_code_outside_user_run.no_promise_state_change',
] as const

const sourceRefs = [
  `table:${KHALA_CODE_OUTSIDE_USER_RUNS_TABLE}`,
  'route:/api/public/khala-code/outside-user-runs',
  'route:/api/public/khala-code/outside-user-runs/:receiptRef',
] as const

export const publicKhalaCodeOutsideUserRunReceipt = (
  record: KhalaCodeOutsideUserRunReceiptRecord,
  generatedAt: string,
): PublicKhalaCodeOutsideUserRunReceipt => ({
  schemaVersion: 'openagents.khala_code.outside_user_run_receipt.v1',
  product: 'khala-code',
  promiseId: 'khala_code.desktop_codex_wrapper.v1',
  receiptRef: record.receiptRef,
  receiptUrl: `${KHALA_CODE_OUTSIDE_USER_RUNS_ENDPOINT}/${encodeURIComponent(
    record.receiptRef,
  )}`,
  generatedAt,
  submittedAt: record.submittedAt,
  appVersion: record.appVersion,
  platform: record.platform,
  arch: record.arch,
  distributionChannel: record.distributionChannel,
  harnessReadiness: record.harnessReadiness,
  publicSafety,
  evidenceRefs: [...evidenceRefs],
  caveatRefs: [...caveatRefs],
  sourceRefs: [...sourceRefs],
  staleness: liveAtReadStaleness([KHALA_CODE_OUTSIDE_USER_RUNS_TABLE]),
})

const boundedIdempotencyKey = (
  value: string | undefined,
): string | undefined => {
  const trimmed = value?.trim()
  return trimmed !== undefined &&
    trimmed !== '' &&
    /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$/.test(trimmed)
    ? trimmed
    : undefined
}

const makeReceiptRef = (): string =>
  compactRandomId('receipt.khala_code.outside_user_run').replace(
    'receipt.khala_code.outside_user_run_',
    'receipt.khala_code.outside_user_run.',
  )

const privateMaterialKeyPattern =
  /(^|_|\b)(authorization|bearer|cwd|home|log|path|prompt|secret|token)(_|$|\b)/i

const containsPrivateMaterialKey = (value: unknown): boolean => {
  if (Array.isArray(value)) {
    return value.some(item => containsPrivateMaterialKey(item))
  }
  if (typeof value !== 'object' || value === null) {
    return false
  }
  for (const [key, child] of Object.entries(value)) {
    if (privateMaterialKeyPattern.test(key)) return true
    if (containsPrivateMaterialKey(child)) return true
  }
  return false
}

const decodeRequest = (
  value: unknown,
): KhalaCodeOutsideUserRunIntakeRequest | undefined => {
  try {
    return S.decodeUnknownSync(KhalaCodeOutsideUserRunIntakeRequest)(value)
  } catch {
    return undefined
  }
}

const readBody = (request: Request) =>
  Effect.promise(() => request.text().catch(() => ''))

export const handlePublicKhalaCodeOutsideUserRunsApi = (
  request: Request,
  input: KhalaCodeOutsideUserRunRouteInput,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const text = yield* readBody(request)
    const parsed = yield* Effect.try({
      try: () => parseJsonUnknown(text),
      catch: () => undefined,
    }).pipe(Effect.catch(() => Effect.void))

    if (parsed === undefined) {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }

    if (containsPrivateMaterialKey(parsed)) {
      return noStoreJsonResponse(
        { error: 'private_material_not_allowed' },
        { status: 400 },
      )
    }

    const body = decodeRequest(parsed)
    if (body === undefined) {
      return noStoreJsonResponse(
        { error: 'invalid_request_schema' },
        { status: 400 },
      )
    }

    const clientKey = boundedIdempotencyKey(body.idempotencyKey)
    if (body.idempotencyKey !== undefined && clientKey === undefined) {
      return noStoreJsonResponse(
        { error: 'invalid_idempotency_key' },
        { status: 400 },
      )
    }

    const nowIso = input.nowIso ?? currentIsoTimestamp
    const submittedAt = nowIso()
    const receiptRef = makeReceiptRef()
    const idempotencyKey = `khala-code-outside-user-run:${
      clientKey ?? receiptRef
    }`
    const store =
      input.store ?? makeD1KhalaCodeOutsideUserRunStore(input.OPENAGENTS_DB)
    const recorded = yield* store
      .recordReceipt({
        receiptRef,
        idempotencyKey,
        appVersion: body.appVersion.trim(),
        platform: body.platform,
        arch: body.arch,
        distributionChannel: body.distributionChannel,
        harnessReadiness: body.harnessReadiness,
        submittedAt,
      })
      .pipe(
        Effect.catch(() =>
          Effect.succeed<
            Readonly<{
              record: KhalaCodeOutsideUserRunReceiptRecord
              idempotent: boolean
            }> | null
          >(null),
        ),
      )

    if (recorded === null) {
      return noStoreJsonResponse(
        { error: 'khala_code_outside_user_run_receipt_unavailable' },
        { status: 503 },
      )
    }

    const generatedAt = nowIso()
    const receipt = publicKhalaCodeOutsideUserRunReceipt(
      recorded.record,
      generatedAt,
    )

    return noStoreJsonResponse(
      {
        ok: true,
        idempotent: recorded.idempotent,
        generatedAt,
        staleness: receipt.staleness,
        receipt,
      } satisfies PublicKhalaCodeOutsideUserRunReceiptIntake,
      { status: recorded.idempotent ? 200 : 201 },
    )
  })

const receiptRefFromPath = (pathname: string): string | null => {
  const prefix = `${KHALA_CODE_OUTSIDE_USER_RUNS_ENDPOINT}/`
  return pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null
}

const receiptRefPattern = /^receipt\.khala_code\.outside_user_run\.[A-Za-z0-9_-]+$/

export const handlePublicKhalaCodeOutsideUserRunReceiptRead = (
  request: Request,
  input: KhalaCodeOutsideUserRunRouteInput & Readonly<{ receiptRef: string }>,
): Effect.Effect<HttpResponse> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }
  if (!receiptRefPattern.test(input.receiptRef)) {
    return Effect.succeed(notFound())
  }

  const nowIso = input.nowIso ?? currentIsoTimestamp
  const store =
    input.store ?? makeD1KhalaCodeOutsideUserRunStore(input.OPENAGENTS_DB)

  return store.readReceipt(input.receiptRef).pipe(
    Effect.map(record => {
      if (record === null) {
        return notFound()
      }

      const generatedAt = nowIso()
      const receipt = publicKhalaCodeOutsideUserRunReceipt(record, generatedAt)

      return noStoreJsonResponse({
        generatedAt,
        staleness: receipt.staleness,
        receipt,
      } satisfies PublicKhalaCodeOutsideUserRunReceiptEnvelope)
    }),
    Effect.catch(() =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'khala_code_outside_user_run_receipt_unavailable' },
          { status: 503 },
        ),
      ),
    ),
  )
}

export const makePublicKhalaCodeOutsideUserRunReceiptRoutes = <Bindings>(
  dependencies: Readonly<{
    makeStore: (env: Bindings) => KhalaCodeOutsideUserRunStore
    nowIso: () => string
  }>,
) => ({
  routePublicKhalaCodeOutsideUserRunReceiptRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const receiptRef = receiptRefFromPath(new URL(request.url).pathname)
    return receiptRef === null
      ? undefined
      : handlePublicKhalaCodeOutsideUserRunReceiptRead(request, {
          receiptRef,
          store: dependencies.makeStore(env),
          nowIso: dependencies.nowIso,
        })
  },
})
