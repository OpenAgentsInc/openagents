import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  KHALA_CODE_OUTSIDE_USER_RUNS_ENDPOINT,
  handlePublicKhalaCodeOutsideUserRunReceiptRead,
  handlePublicKhalaCodeOutsideUserRunsApi,
  khalaCodeOutsideUserRunReceiptRecordFromSql,
  type KhalaCodeOutsideUserRunReceiptDraft,
  type KhalaCodeOutsideUserRunReceiptRecord,
  type KhalaCodeOutsideUserRunStore,
} from './khala-code-outside-user-run-routes'

const nowIso = '2026-07-04T13:00:00.000Z'

const validBody = (
  idempotencyKey = 'outside-user-run-test-1',
): Record<string, unknown> => ({
  schemaVersion: 'openagents.khala_code.outside_user_run_intake.v1',
  consent: {
    publicReceipt: true,
    noPrivateDataIncluded: true,
  },
  appVersion: '0.0.1',
  platform: 'darwin',
  arch: 'arm64',
  distributionChannel: 'source_build',
  harnessReadiness: {
    codexCli: 'ready',
    codexAuth: 'ready',
    pylon: 'unknown',
  },
  idempotencyKey,
})

const request = (method: string, body?: unknown): Request => {
  const init =
    body === undefined
      ? { method }
      : {
          method,
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify(body),
        }

  return new Request(
    `https://openagents.com${KHALA_CODE_OUTSIDE_USER_RUNS_ENDPOINT}`,
    init,
  )
}

const receiptRequest = (receiptRef: string, method = 'GET'): Request =>
  new Request(
    `https://openagents.com${KHALA_CODE_OUTSIDE_USER_RUNS_ENDPOINT}/${encodeURIComponent(
      receiptRef,
    )}`,
    { method },
  )

const toRecord = (
  draft: KhalaCodeOutsideUserRunReceiptDraft,
): KhalaCodeOutsideUserRunReceiptRecord => ({
  receiptRef: draft.receiptRef,
  appVersion: draft.appVersion,
  platform: draft.platform,
  arch: draft.arch,
  distributionChannel: draft.distributionChannel,
  harnessReadiness: draft.harnessReadiness,
  submittedAt: draft.submittedAt,
})

const memoryStore = (): KhalaCodeOutsideUserRunStore => {
  const byIdempotencyKey = new Map<string, KhalaCodeOutsideUserRunReceiptRecord>()
  const byReceiptRef = new Map<string, KhalaCodeOutsideUserRunReceiptRecord>()

  return {
    recordReceipt: draft =>
      Effect.sync(() => {
        const existing = byIdempotencyKey.get(draft.idempotencyKey)
        if (existing !== undefined) {
          return { record: existing, idempotent: true }
        }
        const record = toRecord(draft)
        byIdempotencyKey.set(draft.idempotencyKey, record)
        byReceiptRef.set(record.receiptRef, record)
        return { record, idempotent: false }
      }),
    readReceipt: receiptRef =>
      Effect.sync(() => byReceiptRef.get(receiptRef) ?? null),
  }
}

describe('Khala Code outside-user run receipt routes', () => {
  test('records an explicit public-safe run receipt only on POST', async () => {
    const store = memoryStore()
    const response = await Effect.runPromise(
      handlePublicKhalaCodeOutsideUserRunsApi(request('POST', validBody()), {
        nowIso: () => nowIso,
        store,
      }),
    )

    expect(response.status).toBe(201)
    expect(response.headers.get('cache-control')).toContain('no-store')

    const body = (await response.json()) as {
      ok: boolean
      idempotent: boolean
      generatedAt: string
      staleness: { composition: string; maxStalenessSeconds: number }
      receipt: {
        schemaVersion: string
        product: string
        promiseId: string
        receiptRef: string
        receiptUrl: string
        appVersion: string
        platform: string
        arch: string
        distributionChannel: string
        harnessReadiness: Record<string, string>
        publicSafety: Record<string, boolean>
        caveatRefs: ReadonlyArray<string>
        sourceRefs: ReadonlyArray<string>
        staleness: { composition: string; maxStalenessSeconds: number }
        rawPath?: string
        prompt?: string
        token?: string
      }
    }

    expect(body.ok).toBe(true)
    expect(body.idempotent).toBe(false)
    expect(body.generatedAt).toBe(nowIso)
    expect(body.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
    })
    expect(body.receipt.schemaVersion).toBe(
      'openagents.khala_code.outside_user_run_receipt.v1',
    )
    expect(body.receipt.product).toBe('khala-code')
    expect(body.receipt.promiseId).toBe('khala_code.desktop_codex_wrapper.v1')
    expect(body.receipt.receiptRef).toMatch(
      /^receipt\.khala_code\.outside_user_run\./,
    )
    expect(body.receipt.receiptUrl).toContain(
      '/api/public/khala-code/outside-user-runs/',
    )
    expect(body.receipt.appVersion).toBe('0.0.1')
    expect(body.receipt.platform).toBe('darwin')
    expect(body.receipt.arch).toBe('arm64')
    expect(body.receipt.distributionChannel).toBe('source_build')
    expect(body.receipt.harnessReadiness).toEqual({
      codexCli: 'ready',
      codexAuth: 'ready',
      pylon: 'unknown',
    })
    expect(body.receipt.publicSafety).toMatchObject({
      userActionRequired: true,
      noPhoneHome: true,
      noPaths: true,
      noPrompts: true,
      noTokens: true,
      noLogs: true,
    })
    expect(body.receipt.rawPath).toBeUndefined()
    expect(body.receipt.prompt).toBeUndefined()
    expect(body.receipt.token).toBeUndefined()
    expect(body.receipt.caveatRefs).toContain(
      'caveat.khala_code_outside_user_run.no_promise_state_change',
    )
    expect(body.receipt.sourceRefs).toContain(
      'table:khala_code_outside_user_run_receipts',
    )
    expect(body.receipt.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
    })
  })

  test('replays the same receipt for the same idempotency key', async () => {
    const store = memoryStore()
    const first = await Effect.runPromise(
      handlePublicKhalaCodeOutsideUserRunsApi(request('POST', validBody()), {
        nowIso: () => nowIso,
        store,
      }),
    )
    const second = await Effect.runPromise(
      handlePublicKhalaCodeOutsideUserRunsApi(request('POST', validBody()), {
        nowIso: () => nowIso,
        store,
      }),
    )

    const firstBody = (await first.json()) as {
      receipt: { receiptRef: string }
    }
    const secondBody = (await second.json()) as {
      idempotent: boolean
      receipt: { receiptRef: string }
    }

    expect(second.status).toBe(200)
    expect(secondBody.idempotent).toBe(true)
    expect(secondBody.receipt.receiptRef).toBe(firstBody.receipt.receiptRef)
  })

  test('rejects request bodies carrying private-material keys', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaCodeOutsideUserRunsApi(
        request('POST', {
          ...validBody(),
          prompt: 'please fix my private repo',
          rawPath: '/Users/alice/private',
          token: 'secret',
        }),
        { nowIso: () => nowIso, store: memoryStore() },
      ),
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toMatchObject({
      error: 'private_material_not_allowed',
    })
  })

  test('rejects invalid consent and invalid idempotency keys', async () => {
    const invalidConsent = await Effect.runPromise(
      handlePublicKhalaCodeOutsideUserRunsApi(
        request('POST', {
          ...validBody(),
          consent: {
            publicReceipt: false,
            noPrivateDataIncluded: true,
          },
        }),
        { nowIso: () => nowIso, store: memoryStore() },
      ),
    )

    expect(invalidConsent.status).toBe(400)
    await expect(invalidConsent.json()).resolves.toMatchObject({
      error: 'invalid_request_schema',
    })

    const invalidKey = await Effect.runPromise(
      handlePublicKhalaCodeOutsideUserRunsApi(
        request('POST', validBody('not allowed spaces')),
        { nowIso: () => nowIso, store: memoryStore() },
      ),
    )

    expect(invalidKey.status).toBe(400)
    await expect(invalidKey.json()).resolves.toMatchObject({
      error: 'invalid_idempotency_key',
    })
  })

  test('dereferences a posted receipt by public receiptRef', async () => {
    const store = memoryStore()
    const createResponse = await Effect.runPromise(
      handlePublicKhalaCodeOutsideUserRunsApi(request('POST', validBody()), {
        nowIso: () => nowIso,
        store,
      }),
    )
    const createBody = (await createResponse.json()) as {
      receipt: { receiptRef: string }
    }

    const readResponse = await Effect.runPromise(
      handlePublicKhalaCodeOutsideUserRunReceiptRead(
        receiptRequest(createBody.receipt.receiptRef),
        {
          receiptRef: createBody.receipt.receiptRef,
          nowIso: () => nowIso,
          store,
        },
      ),
    )

    expect(readResponse.status).toBe(200)
    await expect(readResponse.json()).resolves.toMatchObject({
      generatedAt: nowIso,
      staleness: {
        composition: 'live_at_read',
        maxStalenessSeconds: 0,
      },
      receipt: {
        receiptRef: createBody.receipt.receiptRef,
        publicSafety: { noPhoneHome: true },
      },
    })
  })

  test('does not fabricate missing receipts and rejects wrong methods', async () => {
    const missing = await Effect.runPromise(
      handlePublicKhalaCodeOutsideUserRunReceiptRead(
        receiptRequest('receipt.khala_code.outside_user_run.missing'),
        {
          receiptRef: 'receipt.khala_code.outside_user_run.missing',
          nowIso: () => nowIso,
          store: memoryStore(),
        },
      ),
    )
    expect(missing.status).toBe(404)

    const wrongCollectionMethod = await Effect.runPromise(
      handlePublicKhalaCodeOutsideUserRunsApi(request('GET'), {
        nowIso: () => nowIso,
        store: memoryStore(),
      }),
    )
    expect(wrongCollectionMethod.status).toBe(405)

    const wrongReceiptMethod = await Effect.runPromise(
      handlePublicKhalaCodeOutsideUserRunReceiptRead(
        receiptRequest('receipt.khala_code.outside_user_run.any', 'POST'),
        {
          receiptRef: 'receipt.khala_code.outside_user_run.any',
          nowIso: () => nowIso,
          store: memoryStore(),
        },
      ),
    )
    expect(wrongReceiptMethod.status).toBe(405)
  })

  test('SQL row normalization rejects private-shape drift and unknown enum values', () => {
    expect(
      khalaCodeOutsideUserRunReceiptRecordFromSql({
        receipt_ref: 'receipt.khala_code.outside_user_run.abc',
        app_version: '0.0.1',
        platform: 'darwin',
        arch: 'arm64',
        distribution_channel: 'source_build',
        codex_cli_state: 'ready',
        codex_auth_state: 'ready',
        pylon_state: 'unknown',
        submitted_at: nowIso,
      }),
    ).toMatchObject({
      receiptRef: 'receipt.khala_code.outside_user_run.abc',
    })

    expect(
      khalaCodeOutsideUserRunReceiptRecordFromSql({
        receipt_ref: 'receipt.khala_code.outside_user_run.bad',
        app_version: '0.0.1',
        platform: 'darwin',
        arch: 'ppc',
        distribution_channel: 'source_build',
        codex_cli_state: 'ready',
        codex_auth_state: 'ready',
        pylon_state: 'unknown',
        submitted_at: nowIso,
      }),
    ).toBeNull()
  })
})
