// Enterprise claim-upgrade audit projection (proof.claim_upgrade_receipts.v1).
//
// This is a pure, read-only audit surface that JOINS the promise-transition
// receipt feed against the live product-promise registry so a third party can
// dereference and audit every state change — especially every green flip —
// without trusting any narrative copy.
//
// For each promise the projection answers: what is its current registry state,
// when was it last verified by a passing receipt, and which transition receipts
// (from -> to, registryVersion, receiptRef, result, evidenceRefs, owner
// signoff) back it. A registry-wide summary answers the load-bearing audit
// question directly: are all green promises backed by a recorded green-flip
// receipt, and which ones trail the registry (no transition receipt on file)?
//
// Safe-by-construction: no mutation, no secrets, no spend, no authority. It
// only re-projects the already-public registry and the already-public receipt
// feed.

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type {
  PromiseTransitionReceipt,
  PromiseTransitionReceiptStore,
} from './promise-transition-receipt-routes'
import { lastVerifiedAtByPromise } from './promise-transition-receipt-routes'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'
import { publicProductPromisesDocument } from './product-promises'

export type PromiseAuditState =
  | 'green'
  | 'yellow'
  | 'red'
  | 'degraded'
  | 'planned'
  | 'withdrawn'

export type PromiseAuditReceipt = Readonly<{
  receiptRef: string
  fromState: string
  toState: string
  registryVersion: string
  result: 'exception' | 'failed' | 'passed'
  checkedAt: string
  evidenceRefs: ReadonlyArray<string>
  // Owner / policy signoff. A mechanical pass has none; an exception receipt
  // carries the owner-authorized approver, reason, and expiry.
  ownerSignoff: Readonly<{
    approvedByRef: string
    reasonRef: string
    expiresAt: string
  }> | null
  // True when the receipt records an already-applied flip (from === to). Those
  // are recorded as exception receipts so auditors are not misled into reading
  // them as a fresh transition.
  alreadyApplied: boolean
  isGreenFlip: boolean
}>

export type PromiseAuditRow = Readonly<{
  promiseId: string
  productArea: string
  audience: ReadonlyArray<string>
  currentState: PromiseAuditState
  lastVerifiedAt: string | null
  blockerRefs: ReadonlyArray<string>
  evidenceRefCount: number
  // Receipts that target this promise, newest first.
  transitionReceipts: ReadonlyArray<PromiseAuditReceipt>
  greenFlipReceiptCount: number
  // For a green promise: does a recorded green-flip receipt back it? For a
  // non-green promise this is null (not applicable).
  greenReceiptBacked: boolean | null
}>

export type PromiseAuditSummary = Readonly<{
  promiseCount: number
  transitionReceiptCount: number
  greenPromiseCount: number
  // Green promises with at least one recorded green-flip transition receipt.
  greenPromisesReceiptBacked: number
  // Green promises with NO recorded green-flip receipt — these trail the
  // registry and are surfaced explicitly for owner/auditor attention.
  greenPromisesWithoutReceipt: ReadonlyArray<string>
  greenFlipReceiptCount: number
  ownerSignedExceptionCount: number
  failedReceiptCount: number
}>

export type PromiseAuditFilter = Readonly<{
  promiseId?: string
  state?: string
  // When true, only include rows for promises whose current state is green.
  greenOnly?: boolean
}>

export type PromiseAuditProjection = Readonly<{
  kind: 'product_promise_claim_upgrade_audit'
  schemaVersion: 'openagents.product_promise_audit.v1'
  publicSafe: true
  registryVersion: string
  generatedAt: string
  // This projection is composed live at read from the registry + receipt feed,
  // so it can never be older than the request.
  maxStalenessSeconds: number
  staleness: PublicProjectionStalenessContract
  filter: PromiseAuditFilter
  summary: PromiseAuditSummary
  rows: ReadonlyArray<PromiseAuditRow>
  feeds: Readonly<{
    registry: string
    transitions: string
  }>
  rule: string
  authorityBoundary: string
}>

const AUDIT_RULE =
  'A passing or exception transition receipt is dereferenceable mechanical evidence for a state change. It is not the change itself: registry state changes remain maintainer actions shipped through the versioned registry. greenReceiptBacked=false / greenPromisesWithoutReceipt means a green promise has no recorded green-flip receipt yet (its backing transition receipt has not been recorded against the deployed registry).'

const AUDIT_AUTHORITY_BOUNDARY =
  'This audit projection exposes no private data, grants no production authority, moves no money, and changes no registry state. It only re-projects the public product-promise registry and the public transition-receipt feed.'

const receiptToAudit = (
  receipt: PromiseTransitionReceipt,
): PromiseAuditReceipt => ({
  receiptRef: receipt.receiptId,
  fromState: receipt.fromState,
  toState: receipt.toState,
  registryVersion: receipt.registryVersion,
  result: receipt.result,
  checkedAt: receipt.checkedAt,
  evidenceRefs: receipt.evidenceRefs,
  ownerSignoff:
    receipt.exception === null
      ? null
      : {
          approvedByRef: receipt.exception.approvedByRef,
          reasonRef: receipt.exception.reasonRef,
          expiresAt: receipt.exception.expiresAt,
        },
  alreadyApplied: receipt.fromState === receipt.toState,
  isGreenFlip: receipt.toState === 'green',
})

// A green flip is "recorded" when there is a transition receipt targeting the
// promise with toState === green and a non-failed result (passed or owner-signed
// exception). A bare failed receipt is not acceptance evidence.
const hasBackingGreenReceipt = (
  receipts: ReadonlyArray<PromiseAuditReceipt>,
): boolean =>
  receipts.some(receipt => receipt.isGreenFlip && receipt.result !== 'failed')

export const buildPromiseAuditProjection = (
  input: Readonly<{
    receipts: ReadonlyArray<PromiseTransitionReceipt>
    filter?: PromiseAuditFilter
    generatedAt: string
  }>,
): PromiseAuditProjection => {
  const document = publicProductPromisesDocument()
  const filter = input.filter ?? {}
  const verifiedAt = lastVerifiedAtByPromise(input.receipts)
  const staleness = liveAtReadStaleness([
    'product_promise_registry_changed',
    'product_promise_transition_receipt_recorded',
  ])

  const receiptsByPromise = new Map<string, PromiseAuditReceipt[]>()
  for (const receipt of input.receipts) {
    const audit = receiptToAudit(receipt)
    const existing = receiptsByPromise.get(receipt.promiseId)
    if (existing === undefined) {
      receiptsByPromise.set(receipt.promiseId, [audit])
    } else {
      existing.push(audit)
    }
  }

  const allRows: PromiseAuditRow[] = document.promises.map(promise => {
    const transitionReceipts = receiptsByPromise.get(promise.promiseId) ?? []
    const greenFlipReceipts = transitionReceipts.filter(
      receipt => receipt.isGreenFlip,
    )
    const currentState = promise.state as PromiseAuditState

    return {
      promiseId: promise.promiseId,
      productArea: promise.productArea,
      audience: promise.audience,
      currentState,
      lastVerifiedAt: verifiedAt.get(promise.promiseId) ?? null,
      blockerRefs: promise.blockerRefs,
      evidenceRefCount: promise.evidenceRefs.length,
      transitionReceipts,
      greenFlipReceiptCount: greenFlipReceipts.length,
      greenReceiptBacked:
        currentState === 'green'
          ? hasBackingGreenReceipt(transitionReceipts)
          : null,
    }
  })

  const rows = allRows.filter(row => {
    if (
      filter.promiseId !== undefined &&
      row.promiseId !== filter.promiseId
    ) {
      return false
    }
    if (filter.state !== undefined && row.currentState !== filter.state) {
      return false
    }
    if (filter.greenOnly === true && row.currentState !== 'green') {
      return false
    }
    return true
  })

  // The summary is always computed over the FULL registry (not the filtered
  // view) so the headline audit claim — are all greens receipt-backed — cannot
  // be hidden by a filter.
  const greenRows = allRows.filter(row => row.currentState === 'green')
  const greenPromisesWithoutReceipt = greenRows
    .filter(row => row.greenReceiptBacked !== true)
    .map(row => row.promiseId)

  const summary: PromiseAuditSummary = {
    promiseCount: allRows.length,
    transitionReceiptCount: input.receipts.length,
    greenPromiseCount: greenRows.length,
    greenPromisesReceiptBacked: greenRows.filter(
      row => row.greenReceiptBacked === true,
    ).length,
    greenPromisesWithoutReceipt,
    greenFlipReceiptCount: input.receipts.filter(
      receipt => receipt.toState === 'green',
    ).length,
    ownerSignedExceptionCount: input.receipts.filter(
      receipt => receipt.result === 'exception',
    ).length,
    failedReceiptCount: input.receipts.filter(
      receipt => receipt.result === 'failed',
    ).length,
  }

  return {
    kind: 'product_promise_claim_upgrade_audit',
    schemaVersion: 'openagents.product_promise_audit.v1',
    publicSafe: true,
    registryVersion: document.version,
    generatedAt: input.generatedAt,
    maxStalenessSeconds: staleness.maxStalenessSeconds,
    staleness,
    filter,
    summary,
    rows,
    feeds: {
      registry: '/api/public/product-promises',
      transitions: '/api/public/product-promises/transitions',
    },
    rule: AUDIT_RULE,
    authorityBoundary: AUDIT_AUTHORITY_BOUNDARY,
  }
}

const parseAuditFilter = (url: URL): PromiseAuditFilter => {
  const promiseId = url.searchParams.get('promiseId') ?? undefined
  const state = url.searchParams.get('state') ?? undefined
  const greenOnlyParam = url.searchParams.get('greenOnly')
  const greenOnly =
    greenOnlyParam === 'true' || greenOnlyParam === '1' ? true : undefined

  return {
    ...(promiseId === undefined || promiseId === ''
      ? {}
      : { promiseId }),
    ...(state === undefined || state === '' ? {} : { state }),
    ...(greenOnly === undefined ? {} : { greenOnly }),
  }
}

export const handlePublicPromiseAuditApi = (
  request: Request,
  input: Readonly<{
    store: PromiseTransitionReceiptStore
    nowIso?: () => string
  }>,
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.promise(async () => {
    const filter = parseAuditFilter(new URL(request.url))
    const receipts = await input.store.listReceipts(200).catch(() => [])
    const projection = buildPromiseAuditProjection({
      receipts,
      filter,
      generatedAt: input.nowIso?.() ?? currentIsoTimestamp(),
    })

    return noStoreJsonResponse(projection)
  })
}
