import { Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonStringArray, parseJsonUnknown } from './json-boundary'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'
import { publicProductPromisesDocument } from './product-promises'

export const PromiseTransitionState = S.Literals([
  'green',
  'yellow',
  'red',
  'degraded',
  'planned',
  'withdrawn',
])
export type PromiseTransitionState = typeof PromiseTransitionState.Type

const PromiseTransitionRequest = S.Struct({
  evidenceRefs: S.optionalKey(S.Array(S.Trim.check(S.isMaxLength(300)))),
  exception: S.optionalKey(
    S.Struct({
      approvedByRef: S.Trim.check(S.isNonEmpty(), S.isMaxLength(200)),
      expiresAt: S.Trim.check(S.isNonEmpty(), S.isMaxLength(40)),
      reasonRef: S.Trim.check(S.isNonEmpty(), S.isMaxLength(300)),
    }),
  ),
  promiseId: S.Trim.check(S.isNonEmpty(), S.isMaxLength(120)),
  toState: PromiseTransitionState,
})
type PromiseTransitionRequest = typeof PromiseTransitionRequest.Type

export type PromiseTransitionCheck = Readonly<{
  kind:
    | 'blockers_clear_for_green'
    | 'evidence_refs_present'
    | 'from_state_differs'
    | 'promise_exists'
    | 'verification_named'
  result: 'failed' | 'passed'
}>

export type PromiseTransitionReceipt = Readonly<{
  checkedAt: string
  checks: ReadonlyArray<PromiseTransitionCheck>
  evidenceRefs: ReadonlyArray<string>
  exception: Readonly<{
    approvedByRef: string
    expiresAt: string
    reasonRef: string
  }> | null
  fromState: string
  promiseId: string
  receiptId: string
  registryVersion: string
  result: 'exception' | 'failed' | 'passed'
  toState: PromiseTransitionState
}>

export type PromiseTransitionReceiptStore = Readonly<{
  createReceipt: (receipt: PromiseTransitionReceipt) => Promise<void>
  listReceipts: (
    limit: number,
  ) => Promise<ReadonlyArray<PromiseTransitionReceipt>>
}>

type ReceiptRow = Readonly<{
  checked_at: string
  checks_json: string
  evidence_refs_json: string
  exception_json: string | null
  from_state: string
  id: string
  promise_id: string
  registry_version: string
  result: string
  to_state: string
}>

const receiptFromRow = (row: ReceiptRow): PromiseTransitionReceipt => ({
  checkedAt: row.checked_at,
  checks: (parseJsonUnknown(row.checks_json) ??
    []) as ReadonlyArray<PromiseTransitionCheck>,
  evidenceRefs: parseJsonStringArray(row.evidence_refs_json),
  exception:
    row.exception_json === null
      ? null
      : ((parseJsonUnknown(row.exception_json) ??
          null) as PromiseTransitionReceipt['exception']),
  fromState: row.from_state,
  promiseId: row.promise_id,
  receiptId: row.id,
  registryVersion: row.registry_version,
  result: row.result as PromiseTransitionReceipt['result'],
  toState: row.to_state as PromiseTransitionState,
})

export const makeD1PromiseTransitionReceiptStore = (
  db: D1Database,
): PromiseTransitionReceiptStore => ({
  createReceipt: async receipt => {
    await db
      .prepare(
        `INSERT INTO promise_transition_receipts (
          id, promise_id, from_state, to_state, registry_version, result,
          checks_json, evidence_refs_json, exception_json, checked_at,
          created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        receipt.receiptId,
        receipt.promiseId,
        receipt.fromState,
        receipt.toState,
        receipt.registryVersion,
        receipt.result,
        JSON.stringify(receipt.checks),
        JSON.stringify(receipt.evidenceRefs),
        receipt.exception === null ? null : JSON.stringify(receipt.exception),
        receipt.checkedAt,
        receipt.checkedAt,
      )
      .run()
  },
  listReceipts: async limit => {
    const rows = await db
      .prepare(
        `SELECT *
         FROM promise_transition_receipts
         ORDER BY checked_at DESC, id DESC
         LIMIT ?`,
      )
      .bind(limit)
      .all<ReceiptRow>()

    return (rows.results ?? []).map(receiptFromRow)
  },
})

export const evaluatePromiseTransition = (
  input: Readonly<{
    evidenceRefs: ReadonlyArray<string>
    promiseId: string
    toState: PromiseTransitionState
  }>,
): Readonly<{
  checks: ReadonlyArray<PromiseTransitionCheck>
  fromState: string
  registryVersion: string
  result: 'failed' | 'passed'
}> => {
  const document = publicProductPromisesDocument()
  const promise = document.promises.find(
    candidate => candidate.promiseId === input.promiseId,
  )
  const checks: PromiseTransitionCheck[] = [
    { kind: 'promise_exists', result: promise === undefined ? 'failed' : 'passed' },
  ]

  if (promise !== undefined) {
    checks.push({
      kind: 'from_state_differs',
      result: promise.state === input.toState ? 'failed' : 'passed',
    })
    checks.push({
      kind: 'evidence_refs_present',
      result:
        promise.evidenceRefs.length + input.evidenceRefs.length > 0
          ? 'passed'
          : 'failed',
    })
    checks.push({
      kind: 'verification_named',
      result: promise.verification.trim() === '' ? 'failed' : 'passed',
    })

    if (input.toState === 'green') {
      checks.push({
        kind: 'blockers_clear_for_green',
        result: promise.blockerRefs.length === 0 ? 'passed' : 'failed',
      })
    }
  }

  return {
    checks,
    fromState: promise?.state ?? 'unknown',
    registryVersion: document.version,
    result: checks.every(check => check.result === 'passed')
      ? 'passed'
      : 'failed',
  }
}

export const lastVerifiedAtByPromise = (
  receipts: ReadonlyArray<PromiseTransitionReceipt>,
): ReadonlyMap<string, string> => {
  const latest = new Map<string, string>()

  for (const receipt of receipts) {
    if (receipt.result === 'failed') {
      continue
    }

    const existing = latest.get(receipt.promiseId)

    if (existing === undefined || receipt.checkedAt > existing) {
      latest.set(receipt.promiseId, receipt.checkedAt)
    }
  }

  return latest
}

type PromiseTransitionRouteInput = Readonly<{
  nowIso?: () => string
  makeUuid?: () => string
  store: PromiseTransitionReceiptStore
}>

export const handlePublicPromiseTransitionsApi = (
  request: Request,
  input: PromiseTransitionRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.promise(async () => {
    const receipts = await input.store.listReceipts(200)
    const registry = publicProductPromisesDocument()

    return noStoreJsonResponse({
      kind: 'product_promise_transitions',
      generatedAt: registry.generatedAt,
      registryGeneratedAt: registry.generatedAt,
      registryVersion: registry.registryVersion,
      maxStalenessSeconds: registry.maxStalenessSeconds,
      staleness: registry.staleness,
      publicSafe: true,
      receipts,
      rule: 'A passing receipt is mechanical evidence for a proposed state transition. It is not the transition itself: registry state changes remain maintainer actions shipped through the versioned registry.',
    })
  })
}

export const handleOperatorPromiseTransitionApi = (
  request: Request,
  input: PromiseTransitionRouteInput &
    Readonly<{ requireAdminApiToken: () => Promise<boolean> }>,
) => {
  if (request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['POST']))
  }

  return Effect.promise(async () => {
    if (!(await input.requireAdminApiToken())) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    let body: PromiseTransitionRequest

    try {
      body = S.decodeUnknownSync(PromiseTransitionRequest)(
        await request.json(),
      )
    } catch (error) {
      return noStoreJsonResponse(
        {
          error: 'bad_request',
          reason: error instanceof Error ? error.message : String(error),
        },
        { status: 400 },
      )
    }

    const evidenceRefs = body.evidenceRefs ?? []
    const evaluation = evaluatePromiseTransition({
      evidenceRefs,
      promiseId: body.promiseId,
      toState: body.toState,
    })
    const exception = body.exception ?? null
    const result: PromiseTransitionReceipt['result'] =
      evaluation.result === 'passed'
        ? 'passed'
        : exception !== null
          ? 'exception'
          : 'failed'
    const nowIso = input.nowIso?.() ?? currentIsoTimestamp()
    const receipt: PromiseTransitionReceipt = {
      checkedAt: nowIso,
      checks: evaluation.checks,
      evidenceRefs,
      exception,
      fromState: evaluation.fromState,
      promiseId: body.promiseId,
      receiptId: `promise_transition_${input.makeUuid?.() ?? randomUuid()}`,
      registryVersion: evaluation.registryVersion,
      result,
      toState: body.toState,
    }

    await input.store.createReceipt(receipt)

    return noStoreJsonResponse(
      {
        receipt,
        rule: 'A passing or exception receipt does not change the registry. Apply the state change through the versioned registry and cite this receipt as evidence.',
      },
      { status: 201 },
    )
  })
}
