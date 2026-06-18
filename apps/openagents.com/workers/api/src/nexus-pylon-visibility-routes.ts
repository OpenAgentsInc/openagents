import { Effect, Match as M, Schema as S } from 'effect'

import { sha256Hex } from './agent-registration'
import {
  type ArtanisAdminCloseoutReceiptStore,
  artanisAdminCloseoutReceiptDetail,
} from './artanis-admin-closeout-receipts'
import {
  ArtanisPylonProofTraceDispatchEvidence,
  ArtanisPylonProofTracePylonEvent,
  type ArtanisPylonProofTracePylonEventKind,
  ArtanisPylonProofTraceReceiptEvidence,
  ArtanisPylonProofTraceRecord,
  projectArtanisPylonProofTrace,
} from './artanis-pylon-proof-trace'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { decodeUnknownWithSchema, readJsonObject } from './json-boundary'
import {
  type NexusPylonPublicReceiptDetail,
  NexusPylonVisibilityNotFound,
  NexusPylonVisibilityUnsafe,
  exampleNexusPylonVisibilityFixture,
  nexusPylonOperatorDashboard,
  nexusPylonPublicReceiptDetail,
  nexusPylonPublicReceiptDetailFromLedger,
} from './nexus-pylon-visibility'
import {
  type NexusPaymentAuthorityReceiptRecord,
  type NexusPayoutTargetApprovalRecord,
  type NexusTreasuryPayoutAmount,
  type NexusTreasuryPayoutAttemptRecord,
  type NexusTreasuryPayoutIntentRecord,
  type NexusTreasuryPayoutLedgerStorageError,
  type NexusTreasuryPayoutLedgerStore,
  type NexusTreasuryPayoutReconciliationEventRecord,
  projectNexusTreasuryPayoutLedgerRecord,
} from './nexus-treasury-payout-ledger'
import type {
  PylonApiEventRecord,
  PylonApiRegistrationRecord,
  PylonApiStore,
} from './pylon-api'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  TreasuryPaymentAuthorityError,
  type TreasuryPaymentAuthorityShape,
  type TreasuryPaymentAuthorityWalletReadiness,
} from './treasury-payment-authority'

type HttpResponse = globalThis.Response

type NexusPylonVisibilitySession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type NexusPylonVisibilityLedgerStore = Pick<
  NexusTreasuryPayoutLedgerStore,
  | 'createPayoutAttempt'
  | 'createPayoutIntent'
  | 'createPayoutTargetApproval'
  | 'createPaymentAuthorityReceipt'
  | 'createReconciliationEvent'
  | 'readPayoutAttemptByIdempotencyKeyHash'
  | 'readPayoutAttemptByRef'
  | 'readPayoutIntentByIdempotencyKeyHash'
  | 'readPayoutIntentByBuyerPaymentRef'
  | 'readPayoutIntentByRef'
  | 'readPaymentAuthorityReceiptByRef'
  | 'readReconciliationEventByRef'
>

type NexusPylonVisibilityPylonStore = Pick<
  PylonApiStore,
  | 'listEventsForAssignment'
  | 'listEventsForPylon'
  | 'readAssignment'
  | 'readRegistration'
>

// This deprecated Nexus visibility route only needs to read a Lightning
// Address from the recipient readiness, but the canonical readiness now carries
// a discriminated direct-payment union (spark_address | bolt12_offer |
// lightning_address, #5345). Mirror that union structurally so the full
// readiness reader stays assignable; only the lightning_address variant
// exposes a Lightning Address.
type NexusPylonVisibilityDirectPayment =
  | Readonly<{
      sparkAddress: string
      kind: 'spark_address'
      settlementAuthority: 'recipient_wallet_direct'
    }>
  | Readonly<{
      bolt12Offer: string
      lightningAddress?: string | undefined
      kind: 'bolt12_offer'
      settlementAuthority: 'recipient_wallet_direct'
    }>
  | Readonly<{
      lightningAddress: string
      kind: 'lightning_address'
      settlementAuthority: 'recipient_wallet_direct'
    }>

type NexusPylonVisibilityTipRecipientReadiness = Readonly<{
  directPayment: null | NexusPylonVisibilityDirectPayment
  state: string
}>

type NexusPylonVisibilityTipRecipientReadinessReader = Readonly<{
  readForActor: (
    actorRef: string,
  ) => Effect.Effect<NexusPylonVisibilityTipRecipientReadiness, unknown>
}>

type NexusPylonVisibilityDependencies<
  Session extends NexusPylonVisibilitySession,
  Bindings,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  currentIsoTimestamp?: () => string
  isOpenAgentsAdminEmail: (email: string) => boolean
  makeArtanisAdminCloseoutReceiptStore?: (
    env: Bindings,
  ) => ArtanisAdminCloseoutReceiptStore
  makeLedgerStore?: (env: Bindings) => NexusPylonVisibilityLedgerStore
  makePaymentAuthority?: (
    env: Bindings,
    context: Readonly<{
      adapterKind: 'hosted_mdk' | 'mdk_agent_wallet' | 'spark_treasury'
      ledgerStore: NexusPylonVisibilityLedgerStore
      privatePayoutDestination?: string | undefined
      providerRef: string
    }>,
  ) => TreasuryPaymentAuthorityShape
  makePylonApiStore?: (env: Bindings) => NexusPylonVisibilityPylonStore
  makeTipRecipientReadinessReader?: (
    env: Bindings,
  ) => NexusPylonVisibilityTipRecipientReadinessReader
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

class NexusPylonVisibilityUnauthorized extends S.TaggedErrorClass<NexusPylonVisibilityUnauthorized>()(
  'NexusPylonVisibilityUnauthorized',
  {},
) {}

class NexusPylonVisibilityForbidden extends S.TaggedErrorClass<NexusPylonVisibilityForbidden>()(
  'NexusPylonVisibilityForbidden',
  {},
) {}

class NexusPylonVisibilityBridgeBlocked extends S.TaggedErrorClass<NexusPylonVisibilityBridgeBlocked>()(
  'NexusPylonVisibilityBridgeBlocked',
  {
    reason: S.String,
  },
) {}

class NexusPylonVisibilitySessionError extends S.TaggedErrorClass<NexusPylonVisibilitySessionError>()(
  'NexusPylonVisibilitySessionError',
  {
    error: S.Defect,
  },
) {}

type NexusPylonVisibilityRouteError =
  | NexusPylonVisibilityBridgeBlocked
  | NexusPylonVisibilityForbidden
  | NexusPylonVisibilityNotFound
  | NexusPylonVisibilitySessionError
  | NexusPylonVisibilityUnauthorized
  | NexusPylonVisibilityUnsafe

const decodedReceiptRef = (receiptRef: string): string =>
  decodeURIComponent(receiptRef).trim()

const BridgeRequest = S.Struct({
  adapterKind: S.optionalKey(
    S.Literals(['hosted_mdk', 'mdk_agent_wallet', 'spark_treasury']),
  ),
  amountSats: S.Number,
  artanisDispatchRef: S.optionalKey(S.String),
  buyerPaymentRef: S.optionalKey(S.String),
  payoutTargetApprovalRef: S.String,
  payoutTargetRef: S.String,
  policySnapshotRef: S.String,
  providerRef: S.optionalKey(S.String),
  pylonJobRef: S.optionalKey(S.String),
  redactedDestinationRef: S.optionalKey(S.String),
  spendCapSats: S.optionalKey(S.Number),
})

type BridgeRequest = typeof BridgeRequest.Type

const AcceptedWorkPayoutRequest = S.Struct({
  adapterKind: S.optionalKey(
    S.Literals(['hosted_mdk', 'mdk_agent_wallet', 'spark_treasury']),
  ),
  amountSats: S.Number,
  artanisDispatchRef: S.optionalKey(S.String),
  buyerPaymentRef: S.optionalKey(S.String),
  payoutTargetApprovalRef: S.String,
  payoutTargetRef: S.String,
  policySnapshotRef: S.String,
  privatePayoutDestination: S.optionalKey(S.String),
  providerRef: S.optionalKey(S.String),
  pylonJobRef: S.optionalKey(S.String),
  redactedDestinationRef: S.String,
  spendCapSats: S.optionalKey(S.Number),
})

type AcceptedWorkPayoutRequest = typeof AcceptedWorkPayoutRequest.Type

const ProofRunRequest = S.Struct({
  adapterKind: S.optionalKey(
    S.Literals(['hosted_mdk', 'mdk_agent_wallet', 'spark_treasury']),
  ),
  amountSats: S.Number,
  artanisRunRef: S.String,
  assignmentRef: S.String,
  buyerPaymentRef: S.optionalKey(S.String),
  payoutTargetApprovalRef: S.String,
  payoutTargetRef: S.String,
  policySnapshotRef: S.String,
  providerRef: S.optionalKey(S.String),
  pylonJobRef: S.optionalKey(S.String),
  redactedDestinationRef: S.optionalKey(S.String),
  settlementIntentRef: S.optionalKey(S.String),
  spendCapSats: S.optionalKey(S.Number),
})

type ProofRunRequest = typeof ProofRunRequest.Type

const bridgeSafeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const bridgeUnsafePattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|cookie|customer|email|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|invoice|preimage|raw|secret)|payout[_-]?(address|destination|private|raw)|preimage|private|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|invoice|log|payment|payload|payout|prompt|provider|runner|run[_-]?log|state)|secret|seed[_-]?phrase|sk-[A-Za-z0-9]|token|wallet[._-]?(config|key|material|mnemonic|payment|preimage|secret|seed))/i
const bridgeRawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const bridgeBlocked = (reason: string): NexusPylonVisibilityBridgeBlocked =>
  new NexusPylonVisibilityBridgeBlocked({ reason })

const bridgeSuffix = (value: string): string => {
  const suffix = value
    .trim()
    .replaceAll(/[^A-Za-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 96)

  return suffix === '' ? 'unknown' : suffix
}

const assertBridgeSafeRef = (label: string, value: string): void => {
  if (
    !bridgeSafeRefPattern.test(value) ||
    bridgeUnsafePattern.test(value) ||
    bridgeRawTimestampPattern.test(value)
  ) {
    throw bridgeBlocked(`${label} must be a public-safe reference.`)
  }
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const bodyStringRefs = (
  body: Record<string, unknown>,
  keys: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  uniqueRefs(
    keys.flatMap(key => {
      const value = body[key]

      if (typeof value === 'string') {
        return [value]
      }

      return Array.isArray(value)
        ? value.filter((item): item is string => typeof item === 'string')
        : []
    }),
  )

const refsFromEvents = (
  events: ReadonlyArray<PylonApiEventRecord>,
  kinds: ReadonlyArray<PylonApiEventRecord['eventKind']>,
  keys: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  uniqueRefs(
    events
      .filter(event => kinds.includes(event.eventKind))
      .flatMap(event => bodyStringRefs(event.eventBody, keys)),
  )

const acceptedEvents = (
  events: ReadonlyArray<PylonApiEventRecord>,
): ReadonlyArray<PylonApiEventRecord> =>
  events.filter(
    event =>
      event.eventKind === 'assignment_acceptance' &&
      event.status !== 'rejected' &&
      event.eventBody.accepted === true,
  )

const firstRef = (refs: ReadonlyArray<string>, fallback: string): string =>
  refs[0] ?? fallback

const bridgeAmount = (sats: number): NexusTreasuryPayoutAmount => {
  if (!Number.isInteger(sats) || sats <= 0) {
    throw bridgeBlocked('amountSats must be a positive integer.')
  }

  return {
    amountMinorUnits: sats * 1000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  }
}

const decodeBridgeRequest = async (request: Request): Promise<BridgeRequest> =>
  decodeUnknownWithSchema(BridgeRequest, await readJsonObject(request))

const decodeAcceptedWorkPayoutRequest = async (
  request: Request,
): Promise<AcceptedWorkPayoutRequest> =>
  decodeUnknownWithSchema(
    AcceptedWorkPayoutRequest,
    await readJsonObject(request),
  )

const validateBridgeRefs = (
  values: ReadonlyArray<Readonly<{ label: string; value: string | undefined }>>,
): void =>
  values.forEach(item => {
    if (item.value !== undefined) {
      assertBridgeSafeRef(item.label, item.value)
    }
  })

const evidenceKeysForTraceEvent: Readonly<
  Record<PylonApiEventRecord['eventKind'], ReadonlyArray<string>>
> = {
  artifact_proof_metadata: ['artifactRefs', 'proofRefs', 'storageRefs'],
  assignment_acceptance: ['acceptanceRefs'],
  assignment_progress: ['artifactRefs', 'blockerRefs', 'progressRefs'],
  heartbeat: ['capacityRefs', 'healthRefs', 'loadRefs'],
  payment_receipt: ['paymentProofRefs', 'receiptRefs', 'settlementRefs'],
  payout_target_admission: ['admissionRefs', 'policyRefs'],
  registration: ['capabilityRefs', 'statusRefs'],
  settlement_status: ['settlementRefs', 'treasuryReceiptRefs'],
  wallet_readiness: ['balanceRefs', 'liquidityRefs', 'readinessRefs'],
  worker_closeout: [
    'artifactRefs',
    'blockerRefs',
    'buildRefs',
    'closeoutRefs',
    'proofRefs',
    'resultRefs',
    'testRefs',
  ],
}

const traceEventKinds: ReadonlyArray<PylonApiEventRecord['eventKind']> = [
  'artifact_proof_metadata',
  'assignment_acceptance',
  'assignment_progress',
  'payment_receipt',
  'settlement_status',
  'worker_closeout',
]

const pylonEventToTraceEvent = (
  event: PylonApiEventRecord,
  assignmentRef: string,
): ArtanisPylonProofTracePylonEvent | undefined => {
  if (!traceEventKinds.includes(event.eventKind)) {
    return undefined
  }
  const eventKind = event.eventKind as ArtanisPylonProofTracePylonEventKind

  return new ArtanisPylonProofTracePylonEvent({
    ...(typeof event.eventBody.accepted === 'boolean'
      ? { accepted: event.eventBody.accepted }
      : {}),
    assignmentRef: event.assignmentRef ?? assignmentRef,
    eventKind,
    evidenceRefs: uniqueRefs([
      event.eventRef,
      ...bodyStringRefs(event.eventBody, evidenceKeysForTraceEvent[eventKind]),
    ]),
    pylonRef: event.pylonRef,
    status: event.status,
  })
}

const receiptDetailToTraceReceipt = (
  detail: NexusPylonPublicReceiptDetail | null,
  assignmentRef: string,
): ArtanisPylonProofTraceReceiptEvidence | null =>
  detail === null
    ? null
    : new ArtanisPylonProofTraceReceiptEvidence({
        assignmentRef: detail.assignmentRef ?? assignmentRef,
        evidenceRefs: uniqueRefs([detail.receiptRef, detail.apiUrl]),
        movementMode: detail.movementMode,
        pylonRef: null,
        realBitcoinMoved: detail.realBitcoinMoved,
        receiptRef: detail.receiptRef,
        settlementStateLabel: detail.settlement.stateLabel,
        terminalSettlementObserved:
          detail.payoutMovement.terminalResultObserved &&
          detail.payoutMovement.terminalSettlementClaimAllowed,
      })

const proofTraceRecord = (
  body: ProofRunRequest,
  events: ReadonlyArray<PylonApiEventRecord>,
  receiptDetail: NexusPylonPublicReceiptDetail | null,
  nowIso: string,
): ArtanisPylonProofTraceRecord =>
  new ArtanisPylonProofTraceRecord({
    assignmentRef: body.assignmentRef,
    createdAtIso: nowIso,
    dispatch: new ArtanisPylonProofTraceDispatchEvidence({
      artanisRunRef: body.artanisRunRef,
      assignmentRef: body.assignmentRef,
      evidenceRefs: uniqueRefs([
        `event.public.artanis.dispatch.${bridgeSuffix(body.assignmentRef)}`,
        body.artanisRunRef,
      ]),
      settlementIntentRef: body.settlementIntentRef ?? null,
    }),
    pylonEvents: events.flatMap(event => {
      const traceEvent = pylonEventToTraceEvent(event, body.assignmentRef)

      return traceEvent === undefined ? [] : [traceEvent]
    }),
    receipt: receiptDetailToTraceReceipt(receiptDetail, body.assignmentRef),
    releaseEvidenceRefs: [],
    updatedAtIso: nowIso,
  })

const walletReadinessFreshnessMs = 24 * 60 * 60 * 1_000

const latestWalletReadinessEvent = (
  events: ReadonlyArray<PylonApiEventRecord>,
): PylonApiEventRecord | undefined =>
  events
    .filter(event => event.eventKind === 'wallet_readiness')
    .sort((left, right) => right.createdAt.localeCompare(left.createdAt))[0]

const walletReadinessFromEvidence = (
  registration: PylonApiRegistrationRecord | undefined,
  events: ReadonlyArray<PylonApiEventRecord>,
  nowIso: string,
): TreasuryPaymentAuthorityWalletReadiness => {
  if (registration === undefined || !registration.walletReady) {
    return 'absent'
  }

  const latest = latestWalletReadinessEvent(events)

  if (latest === undefined || latest.eventBody.walletReady !== true) {
    return 'absent'
  }

  const nowMs = Date.parse(nowIso)
  const eventMs = Date.parse(latest.createdAt)

  if (
    Number.isNaN(nowMs) ||
    Number.isNaN(eventMs) ||
    nowMs - eventMs > walletReadinessFreshnessMs
  ) {
    return 'stale'
  }

  return 'ready'
}

const assertPrivatePayoutDestination = (
  value: string | undefined,
): string | undefined => {
  if (value === undefined) {
    return undefined
  }

  const trimmed = value.trim()

  if (
    trimmed === '' ||
    trimmed.length > 4096 ||
    /[\u0000-\u001f\u007f]/.test(trimmed)
  ) {
    throw bridgeBlocked('privatePayoutDestination is invalid.')
  }

  return trimmed
}

const agentActorRefForUserId = (userId: string): string => `agent:${userId}`

const lightningAddressFromTipRecipientReadiness = (
  readiness: NexusPylonVisibilityTipRecipientReadiness,
): string | undefined => {
  if (readiness.state !== 'ready') {
    return undefined
  }

  const directPayment = readiness.directPayment
  const lightningAddress =
    directPayment !== null && directPayment.kind !== 'spark_address'
      ? directPayment.lightningAddress?.trim()
      : undefined

  return lightningAddress === '' ? undefined : lightningAddress
}

const resolveAcceptedWorkPayoutDestination = <
  Session extends NexusPylonVisibilitySession,
  Bindings,
>(
  dependencies: NexusPylonVisibilityDependencies<Session, Bindings>,
  env: Bindings,
  adapterKind: 'hosted_mdk' | 'mdk_agent_wallet' | 'spark_treasury',
  requestDestination: string | undefined,
  registration: PylonApiRegistrationRecord | undefined,
): Effect.Effect<string | undefined, NexusPylonVisibilityBridgeBlocked> =>
  Effect.gen(function* () {
    const explicitDestination = yield* Effect.try({
      catch: error =>
        error instanceof NexusPylonVisibilityBridgeBlocked
          ? error
          : bridgeBlocked('privatePayoutDestination is invalid.'),
      try: () => assertPrivatePayoutDestination(requestDestination),
    })

    if (
      explicitDestination !== undefined ||
      (adapterKind !== 'hosted_mdk' && adapterKind !== 'spark_treasury')
    ) {
      return explicitDestination
    }

    const makeReader = dependencies.makeTipRecipientReadinessReader

    if (makeReader === undefined || registration === undefined) {
      return undefined
    }

    const actorRef = agentActorRefForUserId(registration.ownerAgentUserId)
    const readiness = yield* makeReader(env)
      .readForActor(actorRef)
      .pipe(
        Effect.mapError(() =>
          bridgeBlocked(
            'Treasury payouts could not read the agent on-file Spark Lightning Address.',
          ),
        ),
      )
    const lightningAddress =
      lightningAddressFromTipRecipientReadiness(readiness)

    return yield* Effect.try({
      catch: error =>
        error instanceof NexusPylonVisibilityBridgeBlocked
          ? error
          : bridgeBlocked('on-file Spark Lightning Address is invalid.'),
      try: () => assertPrivatePayoutDestination(lightningAddress),
    })
  })

const receiptKindForReconciliation = (
  status: NexusTreasuryPayoutReconciliationEventRecord['status'],
): NexusPaymentAuthorityReceiptRecord['receiptKind'] =>
  status === 'matched'
    ? 'settlement_recorded'
    : status === 'observed'
      ? 'confirmation_recorded'
      : 'verification_recorded'

const settlementStateForReconciliation = (
  status: NexusTreasuryPayoutReconciliationEventRecord['status'],
): string =>
  status === 'matched'
    ? 'settled'
    : status === 'observed'
      ? 'pending'
      : status === 'replayed'
        ? 'replayed'
        : 'rejected'

const authorityErrorToBridgeBlocked = (
  error: TreasuryPaymentAuthorityError,
): NexusPylonVisibilityBridgeBlocked =>
  bridgeBlocked(`${error.reason}: ${error.message}`)

const ledgerReadError = (error: unknown): NexusPylonVisibilityUnsafe =>
  new NexusPylonVisibilityUnsafe({
    reason:
      error !== null &&
      typeof error === 'object' &&
      '_tag' in error &&
      (error as NexusTreasuryPayoutLedgerStorageError)._tag ===
        'NexusTreasuryPayoutLedgerStorageError'
        ? `Nexus/Pylon ledger read failed: ${(error as NexusTreasuryPayoutLedgerStorageError).operation}.`
        : 'Nexus/Pylon ledger read failed.',
  })

const routeErrorResponse = (
  error: NexusPylonVisibilityRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      NexusPylonVisibilityBridgeBlocked: blocked =>
        noStoreJsonResponse(
          {
            error: 'nexus_pylon_bridge_blocked',
            reason: blocked.reason,
          },
          { status: 409 },
        ),
      NexusPylonVisibilityForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      NexusPylonVisibilityNotFound: notFoundError =>
        noStoreJsonResponse(
          {
            error: 'nexus_pylon_receipt_not_found',
            receiptRef: notFoundError.receiptRef,
          },
          { status: 404 },
        ),
      NexusPylonVisibilitySessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      NexusPylonVisibilityUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
      NexusPylonVisibilityUnsafe: unsafe =>
        noStoreJsonResponse(
          {
            error: 'nexus_pylon_visibility_unsafe',
            reason: unsafe.reason,
          },
          { status: 500 },
        ),
    }),
    M.exhaustive,
  )

const htmlResponse = (html: string): HttpResponse =>
  new Response(html, {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/html; charset=utf-8',
    },
  })

const escapeHtml = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')

const renderPublicReceiptPage = (
  detail: ReturnType<typeof nexusPylonPublicReceiptDetail>,
): string => `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(detail.receiptRef)} - OpenAgents Nexus/Pylon Receipt</title>
  <style>
    :root { color-scheme: dark; background: #050505; color: #f4f1e8; }
    body { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; margin: 0; }
    main { margin: 0 auto; max-width: 920px; padding: 56px 24px; }
    h1 { font-size: clamp(28px, 6vw, 52px); line-height: 1; margin: 0 0 24px; }
    dl { border: 1px solid #333; display: grid; grid-template-columns: minmax(140px, 220px) 1fr; margin: 28px 0; }
    dt, dd { border-bottom: 1px solid #252525; margin: 0; padding: 16px; }
    dt { color: #888; text-transform: uppercase; letter-spacing: .08em; }
    a { color: #f4f1e8; }
    .label { color: #c9b16b; letter-spacing: .12em; text-transform: uppercase; }
  </style>
</head>
<body>
  <main>
    <p class="label">OpenAgents Nexus / Pylon receipt</p>
    <h1>${escapeHtml(detail.receiptKind.replaceAll('_', ' '))}</h1>
    <dl>
      <dt>Receipt</dt>
      <dd>${escapeHtml(detail.receiptRef)}</dd>
      <dt>Status</dt>
      <dd>${escapeHtml(detail.status)}</dd>
      <dt>Mode</dt>
      <dd>${escapeHtml(detail.movementMode)}; real bitcoin moved: ${detail.realBitcoinMoved ? 'yes' : 'no'}</dd>
      <dt>Dispatch</dt>
      <dd>${detail.payoutMovement.dispatchAccepted ? 'accepted' : 'not accepted'}</dd>
      <dt>Terminal result</dt>
      <dd>${detail.payoutMovement.terminalResultObserved ? 'observed' : 'not observed'}</dd>
      <dt>Settlement</dt>
      <dd>${escapeHtml(detail.settlement.stateLabel)}</dd>
      <dt>Pylon</dt>
      <dd>${escapeHtml(detail.settlement.providerRef)}</dd>
      <dt>API</dt>
      <dd><a href="${escapeHtml(detail.apiUrl)}">${escapeHtml(detail.apiUrl)}</a></dd>
    </dl>
    <p>This receipt page shows public-safe proof only. Dispatch acceptance is separate from terminal bitcoin settlement. Sensitive payment details are omitted.</p>
  </main>
</body>
</html>`

const receiptRefFromPath = (
  path: string,
  prefix: string,
): string | undefined => {
  if (!path.startsWith(prefix)) {
    return undefined
  }

  const receiptRef = path.slice(prefix.length).trim()

  return receiptRef === '' || receiptRef.includes('/') ? undefined : receiptRef
}

const nowIsoFor = <Session extends NexusPylonVisibilitySession, Bindings>(
  dependencies: NexusPylonVisibilityDependencies<Session, Bindings>,
): string => (dependencies.currentIsoTimestamp ?? currentIsoTimestamp)()

const appUrlFromRequest = (request: Request): string =>
  new URL(request.url).origin

const requireAdminSession = <
  Session extends NexusPylonVisibilitySession,
  Bindings,
>(
  dependencies: NexusPylonVisibilityDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const requireAdminApiToken = dependencies.requireAdminApiToken

    if (requireAdminApiToken !== undefined) {
      const hasAdminApiToken = yield* Effect.tryPromise({
        catch: error => new NexusPylonVisibilitySessionError({ error }),
        try: () => requireAdminApiToken(request, env),
      })

      if (hasAdminApiToken) {
        return {
          user: {
            email: 'chris@openagents.com',
            userId: 'github:14167547',
          },
        } as Session
      }
    }

    const session = yield* Effect.tryPromise({
      catch: error => new NexusPylonVisibilitySessionError({ error }),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    })

    if (session === undefined) {
      return yield* new NexusPylonVisibilityUnauthorized({})
    }

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new NexusPylonVisibilityForbidden({})
    }

    return session
  })

const publicReceiptJson = <
  Session extends NexusPylonVisibilitySession,
  Bindings,
>(
  request: Request,
  env: Bindings,
  dependencies: NexusPylonVisibilityDependencies<Session, Bindings>,
  receiptRef: string,
  nowIso: string,
) =>
  Effect.gen(function* () {
    const detail = yield* publicReceiptDetail(
      request,
      env,
      dependencies,
      receiptRef,
      nowIso,
    )

    return noStoreJsonResponse(detail)
  })

const publicReceiptPage = <
  Session extends NexusPylonVisibilitySession,
  Bindings,
>(
  request: Request,
  env: Bindings,
  dependencies: NexusPylonVisibilityDependencies<Session, Bindings>,
  receiptRef: string,
  nowIso: string,
) =>
  Effect.gen(function* () {
    const detail = yield* publicReceiptDetail(
      request,
      env,
      dependencies,
      receiptRef,
      nowIso,
    )

    return htmlResponse(renderPublicReceiptPage(detail))
  })

const publicReceiptDetail = <
  Session extends NexusPylonVisibilitySession,
  Bindings,
>(
  request: Request,
  env: Bindings,
  dependencies: NexusPylonVisibilityDependencies<Session, Bindings>,
  receiptRef: string,
  nowIso: string,
) =>
  Effect.gen(function* () {
    const makeLedgerStore = dependencies.makeLedgerStore
    const makeArtanisAdminCloseoutReceiptStore =
      dependencies.makeArtanisAdminCloseoutReceiptStore
    const normalizedReceiptRef = decodedReceiptRef(receiptRef)
    const fallback = () =>
      Effect.try({
        catch: error =>
          error instanceof NexusPylonVisibilityNotFound ||
          error instanceof NexusPylonVisibilityUnsafe
            ? error
            : new NexusPylonVisibilityUnsafe({
                reason: 'Nexus/Pylon public receipt projection failed.',
              }),
        try: () =>
          nexusPylonPublicReceiptDetail({
            appUrl: appUrlFromRequest(request),
            nowIso,
            receiptRef,
          }),
      })

    if (makeArtanisAdminCloseoutReceiptStore !== undefined) {
      const store = makeArtanisAdminCloseoutReceiptStore(env)
      const artanisRecord = yield* Effect.tryPromise({
        catch: error =>
          error instanceof NexusPylonVisibilityUnsafe
            ? error
            : new NexusPylonVisibilityUnsafe({
                reason: 'Artanis admin closeout receipt projection failed.',
              }),
        try: () => store.readCloseoutReceiptByRef(normalizedReceiptRef),
      })

      if (artanisRecord !== undefined) {
        return yield* Effect.try({
          catch: error =>
            error instanceof NexusPylonVisibilityUnsafe
              ? error
              : new NexusPylonVisibilityUnsafe({
                  reason: 'Artanis admin closeout receipt projection failed.',
                }),
          try: () =>
            artanisAdminCloseoutReceiptDetail({
              appUrl: appUrlFromRequest(request),
              nowIso,
              record: artanisRecord,
            }),
        })
      }
    }

    if (makeLedgerStore === undefined) {
      return yield* fallback()
    }

    const store = makeLedgerStore(env)
    const receipt = yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () => store.readPaymentAuthorityReceiptByRef(normalizedReceiptRef),
    })

    if (receipt === undefined) {
      return yield* fallback()
    }

    const intent = yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () => store.readPayoutIntentByRef(receipt.payoutIntentRef),
    })
    const attempt =
      receipt.payoutAttemptRef === null
        ? undefined
        : yield* Effect.tryPromise({
            catch: ledgerReadError,
            try: () => store.readPayoutAttemptByRef(receipt.payoutAttemptRef!),
          })
    const event =
      receipt.eventRef === null
        ? undefined
        : yield* Effect.tryPromise({
            catch: ledgerReadError,
            try: () => store.readReconciliationEventByRef(receipt.eventRef!),
          })

    return nexusPylonPublicReceiptDetailFromLedger({
      appUrl: appUrlFromRequest(request),
      attempt,
      event,
      intent,
      nowIso,
      receipt,
    })
  })

const operatorDashboard = <
  Session extends NexusPylonVisibilitySession,
  Bindings,
>(
  dependencies: NexusPylonVisibilityDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* requireAdminSession(dependencies, request, env, ctx)
    const dashboard = nexusPylonOperatorDashboard(nowIsoFor(dependencies))

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(dashboard),
      session,
    )
  })

const operatorReceiptJson = <
  Session extends NexusPylonVisibilitySession,
  Bindings,
>(
  dependencies: NexusPylonVisibilityDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  receiptRef: string,
) =>
  Effect.gen(function* () {
    const session = yield* requireAdminSession(dependencies, request, env, ctx)
    const makeLedgerStore = dependencies.makeLedgerStore
    const store =
      makeLedgerStore === undefined ? undefined : makeLedgerStore(env)
    const normalizedReceiptRef = decodedReceiptRef(receiptRef)
    const persistedReceipt =
      store === undefined
        ? undefined
        : yield* Effect.tryPromise({
            catch: ledgerReadError,
            try: () =>
              store.readPaymentAuthorityReceiptByRef(normalizedReceiptRef),
          })

    if (persistedReceipt !== undefined && store !== undefined) {
      const persistedProjection = nexusPylonPublicReceiptDetailFromLedger({
        appUrl: appUrlFromRequest(request),
        attempt:
          persistedReceipt.payoutAttemptRef === null
            ? undefined
            : yield* Effect.tryPromise({
                catch: ledgerReadError,
                try: () =>
                  store.readPayoutAttemptByRef(
                    persistedReceipt.payoutAttemptRef!,
                  ),
              }),
        event:
          persistedReceipt.eventRef === null
            ? undefined
            : yield* Effect.tryPromise({
                catch: ledgerReadError,
                try: () =>
                  store.readReconciliationEventByRef(
                    persistedReceipt.eventRef!,
                  ),
              }),
        intent: yield* Effect.tryPromise({
          catch: ledgerReadError,
          try: () =>
            store.readPayoutIntentByRef(persistedReceipt.payoutIntentRef),
        }),
        nowIso: nowIsoFor(dependencies),
        receipt: persistedReceipt,
      })

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({
          schemaVersion: 'openagents.nexus_pylon.operator_receipt.v1',
          movementMode: persistedProjection.movementMode,
          realBitcoinMoved: persistedProjection.realBitcoinMoved,
          receipt: persistedProjection,
          receiptRecord: persistedReceipt,
          settlementStatus: persistedProjection.settlement,
        }),
        session,
      )
    }

    const fixture = exampleNexusPylonVisibilityFixture(nowIsoFor(dependencies))
    const receipt = fixture.receipts.find(
      candidate => candidate.receiptRef === normalizedReceiptRef,
    )

    if (receipt === undefined) {
      return yield* new NexusPylonVisibilityNotFound({
        receiptRef: normalizedReceiptRef,
      })
    }

    const response = {
      schemaVersion: 'openagents.nexus_pylon.operator_receipt.v1',
      movementMode: fixture.movementMode,
      realBitcoinMoved: fixture.realBitcoinMoved,
      receipt: fixture.receiptProjections.find(
        projection => projection.receiptRef === receipt.receiptRef,
      ),
      receiptRecord: receipt,
      settlementStatus: fixture.settlementProjection,
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(response),
      session,
    )
  })

const operatorAssignmentAcceptedWorkPayout = <
  Session extends NexusPylonVisibilitySession,
  Bindings,
>(
  dependencies: NexusPylonVisibilityDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  encodedAssignmentRef: string,
) =>
  Effect.gen(function* () {
    const session = yield* requireAdminSession(dependencies, request, env, ctx)
    const makeLedgerStore = dependencies.makeLedgerStore
    const makePylonApiStore = dependencies.makePylonApiStore
    const makePaymentAuthority = dependencies.makePaymentAuthority
    const idempotencyKey = request.headers.get('idempotency-key')?.trim()

    if (
      makeLedgerStore === undefined ||
      makePylonApiStore === undefined ||
      makePaymentAuthority === undefined
    ) {
      return yield* bridgeBlocked(
        'Nexus/Pylon accepted-work payout storage or payment authority is not configured.',
      )
    }

    if (idempotencyKey === undefined || idempotencyKey === '') {
      return yield* bridgeBlocked('Idempotency-Key header is required.')
    }

    const assignmentRef = decodedReceiptRef(encodedAssignmentRef)
    const body = yield* Effect.tryPromise({
      catch: () =>
        bridgeBlocked('Accepted-work payout request body is invalid.'),
      try: () => decodeAcceptedWorkPayoutRequest(request),
    })
    const adapterKind = body.adapterKind ?? 'hosted_mdk'

    const pylonStore = makePylonApiStore(env)
    const ledgerStore = makeLedgerStore(env)
    const assignment = yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () => pylonStore.readAssignment(assignmentRef),
    })

    if (assignment === undefined) {
      return yield* bridgeBlocked('Assignment was not found.')
    }

    const assignmentEvents = yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () => pylonStore.listEventsForAssignment(assignmentRef, 100),
    })
    const pylonEvents = yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () => pylonStore.listEventsForPylon(assignment.pylonRef, 100),
    })
    const registration = yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () => pylonStore.readRegistration(assignment.pylonRef),
    })
    const nowIso = nowIsoFor(dependencies)
    const walletReadiness = walletReadinessFromEvidence(
      registration,
      pylonEvents,
      nowIso,
    )
    const suffix = bridgeSuffix(assignmentRef)
    const amounts = yield* Effect.try({
      catch: error =>
        error instanceof NexusPylonVisibilityBridgeBlocked
          ? error
          : new NexusPylonVisibilityUnsafe({
              reason:
                'Nexus/Pylon accepted-work payout amount validation failed.',
            }),
      try: () => ({
        amount: bridgeAmount(body.amountSats),
        spendCap: bridgeAmount(body.spendCapSats ?? body.amountSats),
      }),
    })
    const amount = amounts.amount
    const spendCap = amounts.spendCap
    const acceptedWorkRefs = uniqueRefs(assignment.acceptedWorkRefs)
    const artifactRefs = uniqueRefs([
      ...assignment.artifactRefs,
      ...assignment.proofRefs,
      ...assignment.closeoutRefs,
    ])
    const metadataRefs = uniqueRefs([
      ...artifactRefs,
      ...refsFromEvents(
        assignmentEvents,
        ['artifact_proof_metadata', 'assignment_acceptance'],
        ['acceptanceRefs', 'artifactRefs', 'proofRefs', 'storageRefs'],
      ),
    ])
    const providerRef =
      body.providerRef ?? `provider.public.nexus_pylon.${adapterKind}`
    const payoutKeyHash = yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () =>
        sha256Hex(
          `nexus-pylon-accepted-work-payout:${assignmentRef}:${body.payoutTargetRef}:${body.amountSats}`,
        ),
    })
    const hashRef = `hash.${payoutKeyHash.slice(0, 64)}`
    const payoutIntentRef = `payout_intent.nexus_pylon.accepted_work.${suffix}`
    const payoutAttemptRef = `payout_attempt.nexus_pylon.accepted_work.${suffix}`
    const reconciliationEventRef = `reconciliation.nexus_pylon.accepted_work.${suffix}`
    const receiptRef = `receipt.nexus_pylon.accepted_work_settlement.${suffix}`

    yield* Effect.try({
      catch: error =>
        error instanceof NexusPylonVisibilityBridgeBlocked
          ? error
          : new NexusPylonVisibilityUnsafe({
              reason:
                'Nexus/Pylon accepted-work payout public-safe validation failed.',
            }),
      try: () => {
        validateBridgeRefs([
          { label: 'assignmentRef', value: assignmentRef },
          { label: 'artanisDispatchRef', value: body.artanisDispatchRef },
          { label: 'buyerPaymentRef', value: body.buyerPaymentRef },
          {
            label: 'payoutTargetApprovalRef',
            value: body.payoutTargetApprovalRef,
          },
          { label: 'payoutTargetRef', value: body.payoutTargetRef },
          { label: 'policySnapshotRef', value: body.policySnapshotRef },
          { label: 'providerRef', value: providerRef },
          { label: 'pylonJobRef', value: body.pylonJobRef },
          { label: 'pylonRef', value: assignment.pylonRef },
          { label: 'receiptRef', value: receiptRef },
          {
            label: 'redactedDestinationRef',
            value: body.redactedDestinationRef,
          },
        ])
        uniqueRefs([
          ...acceptedWorkRefs,
          ...metadataRefs,
          payoutIntentRef,
          payoutAttemptRef,
          reconciliationEventRef,
        ]).forEach(ref => assertBridgeSafeRef('accepted-work payout ref', ref))
      },
    })

    if (spendCap.amountMinorUnits < amount.amountMinorUnits) {
      return yield* bridgeBlocked('amountSats must not exceed spendCapSats.')
    }

    if (assignment.state !== 'accepted_work') {
      return yield* bridgeBlocked(
        'Assignment must be closed out as accepted work before payout.',
      )
    }

    if (acceptedWorkRefs.length === 0) {
      return yield* bridgeBlocked(
        'Assignment must include accepted-work references before payout.',
      )
    }

    if (artifactRefs.length === 0) {
      return yield* bridgeBlocked(
        'Assignment must include artifact or proof references before payout.',
      )
    }

    if (walletReadiness !== 'ready') {
      return yield* bridgeBlocked(
        `Payout requires fresh wallet readiness evidence; current state is ${walletReadiness}.`,
      )
    }

    const privatePayoutDestination =
      yield* resolveAcceptedWorkPayoutDestination(
        dependencies,
        env,
        adapterKind,
        body.privatePayoutDestination,
        registration,
      )

    if (
      (adapterKind === 'hosted_mdk' || adapterKind === 'spark_treasury') &&
      privatePayoutDestination === undefined
    ) {
      return yield* bridgeBlocked(
        'Treasury payouts require privatePayoutDestination or an agent on-file Spark Lightning Address.',
      )
    }

    const existingReceipt = yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () => ledgerStore.readPaymentAuthorityReceiptByRef(receiptRef),
    })

    if (existingReceipt !== undefined) {
      const existingIntent = yield* Effect.tryPromise({
        catch: ledgerReadError,
        try: () =>
          ledgerStore.readPayoutIntentByRef(existingReceipt.payoutIntentRef),
      })
      const existingAttempt =
        existingReceipt.payoutAttemptRef === null
          ? undefined
          : yield* Effect.tryPromise({
              catch: ledgerReadError,
              try: () =>
                ledgerStore.readPayoutAttemptByRef(
                  existingReceipt.payoutAttemptRef!,
                ),
            })
      const existingEvent =
        existingReceipt.eventRef === null
          ? undefined
          : yield* Effect.tryPromise({
              catch: ledgerReadError,
              try: () =>
                ledgerStore.readReconciliationEventByRef(
                  existingReceipt.eventRef!,
                ),
            })

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({
          payout: {
            assignmentRef,
            idempotent: true,
            receipt: nexusPylonPublicReceiptDetailFromLedger({
              appUrl: appUrlFromRequest(request),
              attempt: existingAttempt,
              event: existingEvent,
              intent: existingIntent,
              nowIso,
              receipt: existingReceipt,
            }),
            walletReadiness,
          },
          schemaVersion: 'openagents.nexus_pylon.accepted_work_payout.v1',
        }),
        session,
      )
    }

    const targetApproval: NexusPayoutTargetApprovalRecord = {
      agentRef: 'agent.artanis',
      approvalPolicyRef: 'policy.public.nexus_pylon.accepted_work_payout',
      approvalRef: body.payoutTargetApprovalRef,
      approvedByRef: 'operator.openagents.nexus_pylon_accepted_work_payout',
      archivedAt: null,
      createdAt: nowIso,
      expiresAt: null,
      id: `nexus_payout_target_approval_accepted_work_${suffix}`,
      idempotencyKeyHash: `${hashRef}.approval`,
      ownerUserId: 'user_openagents_operator',
      publicProjectionJson: JSON.stringify({
        assignmentRef,
        pylonRef: assignment.pylonRef,
        state: 'active',
      }),
      payoutTargetRef: body.payoutTargetRef,
      pylonRef: assignment.pylonRef,
      redactedDestinationRef: body.redactedDestinationRef,
      scopeRefs: [assignmentRef],
      status: 'active',
      updatedAt: nowIso,
    }
    const intent: NexusTreasuryPayoutIntentRecord = {
      acceptedWorkRefs,
      actorRef: 'agent.artanis',
      adapterKind,
      amount,
      archivedAt: null,
      artanisDispatchRef: body.artanisDispatchRef ?? null,
      assignmentRef,
      buyerPaymentRef: body.buyerPaymentRef ?? null,
      createdAt: nowIso,
      id: `nexus_treasury_payout_intent_accepted_work_${suffix}`,
      idempotencyKeyHash: `${hashRef}.intent`,
      metadataRefs,
      ownerUserId: 'user_openagents_operator',
      payoutIntentRef,
      payoutTargetApprovalRef: body.payoutTargetApprovalRef,
      payoutTargetRef: body.payoutTargetRef,
      policySnapshotRef: body.policySnapshotRef,
      publicProjectionJson: JSON.stringify({
        assignmentRef,
        moneyMovement: 'real_bitcoin',
        pylonRef: assignment.pylonRef,
        state: 'approved',
      }),
      pylonJobRef: body.pylonJobRef ?? assignment.taskRefs[0] ?? null,
      sourceKind: 'accepted_work',
      spendCap,
      status: 'approved',
      updatedAt: nowIso,
    }
    const pendingAttempt: NexusTreasuryPayoutAttemptRecord = {
      adapterAttemptRef: `adapter_attempt.${adapterKind}.${suffix}`,
      adapterKind,
      amount,
      archivedAt: null,
      createdAt: nowIso,
      id: `nexus_treasury_payout_attempt_accepted_work_${suffix}`,
      idempotencyKeyHash: `${hashRef}.attempt`,
      metadataRefs: uniqueRefs([
        ...metadataRefs,
        'metadata.nexus.accepted_work_payout.dispatch_requested',
      ]),
      payoutAttemptRef,
      payoutIntentRef,
      publicProjectionJson: JSON.stringify({
        assignmentRef,
        moneyMovement: 'real_bitcoin',
        pylonRef: assignment.pylonRef,
        state: 'dispatch_requested',
      }),
      redactedDestinationRef: body.redactedDestinationRef,
      redactedPaymentRef: null,
      status: 'pending',
      updatedAt: nowIso,
    }
    const authority = makePaymentAuthority(env, {
      adapterKind,
      ledgerStore,
      privatePayoutDestination,
      providerRef,
    })

    yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () => ledgerStore.createPayoutTargetApproval(targetApproval),
    })

    const existingIntent = yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () =>
        ledgerStore.readPayoutIntentByIdempotencyKeyHash(
          intent.idempotencyKeyHash,
        ),
    })
    const createdIntent =
      existingIntent === undefined
        ? yield* authority
            .createPayoutIntent({
              intent,
              walletReadiness,
            })
            .pipe(Effect.mapError(authorityErrorToBridgeBlocked))
        : {
            intent: existingIntent,
            projection: projectNexusTreasuryPayoutLedgerRecord(
              'intent',
              existingIntent,
              'operator',
            ),
            replayed: true,
          }
    const dispatch = yield* authority
      .dispatchPayout({
        attempt: pendingAttempt,
        payoutIntentRef: createdIntent.intent.payoutIntentRef,
      })
      .pipe(Effect.mapError(authorityErrorToBridgeBlocked))
    const reconciliationInput: NexusTreasuryPayoutReconciliationEventRecord = {
      adapterKind,
      archivedAt: null,
      createdAt: nowIso,
      eventRef: reconciliationEventRef,
      externalEventRef:
        dispatch.attempt.redactedPaymentRef ??
        `payment.redacted.nexus_pylon.accepted_work.${suffix}`,
      id: `nexus_treasury_reconciliation_accepted_work_${suffix}`,
      idempotencyKeyHash: dispatch.attempt.idempotencyKeyHash,
      metadataRefs: uniqueRefs([
        ...metadataRefs,
        ...dispatch.attempt.metadataRefs,
        'metadata.nexus.accepted_work_payout.reconciliation_requested',
      ]),
      payoutAttemptRef: dispatch.attempt.payoutAttemptRef,
      payoutIntentRef: dispatch.intent.payoutIntentRef,
      providerRef,
      publicProjectionJson: JSON.stringify({
        assignmentRef,
        moneyMovement: 'real_bitcoin',
        providerRef,
        pylonRef: assignment.pylonRef,
        state: 'reconciliation_requested',
      }),
      resultRef:
        dispatch.attempt.redactedPaymentRef ??
        `result.hosted_mdk.requested.${suffix}`,
      status: 'observed',
    }
    const reconciliation = yield* authority
      .reconcilePayout({
        event: reconciliationInput,
      })
      .pipe(Effect.mapError(authorityErrorToBridgeBlocked))
    const settlementState = settlementStateForReconciliation(
      reconciliation.event.status,
    )
    const receipt: NexusPaymentAuthorityReceiptRecord = {
      archivedAt: null,
      audience: 'public',
      createdAt: nowIso,
      eventRef: reconciliation.event.eventRef,
      id: `nexus_payment_authority_receipt_accepted_work_${suffix}`,
      metadataRefs: uniqueRefs([
        ...metadataRefs,
        `metadata.nexus.accepted_work_payout.${settlementState}`,
      ]),
      payoutAttemptRef: dispatch.attempt.payoutAttemptRef,
      payoutIntentRef: dispatch.intent.payoutIntentRef,
      publicProjectionJson: JSON.stringify({
        amountSats: body.amountSats,
        assignmentRef,
        moneyMovement: 'real_bitcoin',
        providerRef,
        pylonRef: assignment.pylonRef,
        state: settlementState,
      }),
      receiptKind: receiptKindForReconciliation(reconciliation.event.status),
      receiptRef,
    }

    yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () => ledgerStore.createPaymentAuthorityReceipt(receipt),
    })

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(
        {
          payout: {
            assignmentRef,
            idempotent: false,
            payoutAttempt: dispatch.projection,
            payoutIntent: createdIntent.projection,
            receipt: nexusPylonPublicReceiptDetailFromLedger({
              appUrl: appUrlFromRequest(request),
              attempt: dispatch.attempt,
              event: reconciliation.event,
              intent: dispatch.intent,
              nowIso,
              receipt,
            }),
            walletReadiness,
          },
          schemaVersion: 'openagents.nexus_pylon.accepted_work_payout.v1',
        },
        { status: reconciliation.event.status === 'observed' ? 202 : 201 },
      ),
      session,
    )
  })

const operatorAssignmentSettlementBridge = <
  Session extends NexusPylonVisibilitySession,
  Bindings,
>(
  dependencies: NexusPylonVisibilityDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  encodedAssignmentRef: string,
) =>
  Effect.gen(function* () {
    const session = yield* requireAdminSession(dependencies, request, env, ctx)
    const makeLedgerStore = dependencies.makeLedgerStore
    const makePylonApiStore = dependencies.makePylonApiStore
    const idempotencyKey = request.headers.get('idempotency-key')?.trim()

    if (makeLedgerStore === undefined || makePylonApiStore === undefined) {
      return yield* bridgeBlocked(
        'Nexus/Pylon settlement bridge storage is not configured.',
      )
    }

    if (idempotencyKey === undefined || idempotencyKey === '') {
      return yield* bridgeBlocked('Idempotency-Key header is required.')
    }

    const assignmentRef = decodedReceiptRef(encodedAssignmentRef)
    const body = yield* Effect.tryPromise({
      catch: () => bridgeBlocked('Bridge request body is invalid.'),
      try: () => decodeBridgeRequest(request),
    })
    const pylonStore = makePylonApiStore(env)
    const ledgerStore = makeLedgerStore(env)
    const events = yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () => pylonStore.listEventsForAssignment(assignmentRef, 100),
    })
    const nowIso = nowIsoFor(dependencies)
    const suffix = bridgeSuffix(assignmentRef)
    const amounts = yield* Effect.try({
      catch: error =>
        error instanceof NexusPylonVisibilityBridgeBlocked
          ? error
          : new NexusPylonVisibilityUnsafe({
              reason: 'Nexus/Pylon bridge amount validation failed.',
            }),
      try: () => ({
        amount: bridgeAmount(body.amountSats),
        spendCap: bridgeAmount(body.spendCapSats ?? body.amountSats),
      }),
    })
    const amount = amounts.amount
    const spendCap = amounts.spendCap
    const acceptanceRefs = refsFromEvents(
      events,
      ['assignment_acceptance'],
      ['acceptanceRefs'],
    )
    const artifactRefs = refsFromEvents(
      events,
      ['artifact_proof_metadata'],
      ['artifactRefs', 'proofRefs', 'storageRefs'],
    )
    const paymentRefs = refsFromEvents(
      events,
      ['payment_receipt'],
      ['paymentProofRefs', 'receiptRefs'],
    )
    const settlementRefs = refsFromEvents(
      events,
      ['payment_receipt', 'settlement_status'],
      ['settlementRefs', 'treasuryReceiptRefs'],
    )
    const pylonRef = firstRef(
      uniqueRefs(events.map(event => event.pylonRef)),
      `pylon.public.nexus_pylon_bridge.${suffix}`,
    )
    const acceptedWorkRefs = uniqueRefs([...acceptanceRefs, ...artifactRefs])
    const metadataRefs = uniqueRefs([
      ...artifactRefs,
      ...paymentRefs,
      ...settlementRefs,
    ])
    const receiptRef = `receipt.nexus_pylon.settlement.${suffix}`
    const payoutIntentRef = `payout_intent.nexus_pylon.${suffix}`
    const payoutAttemptRef = `payout_attempt.nexus_pylon.${suffix}`
    const reconciliationEventRef = `reconciliation.nexus_pylon.${suffix}`
    const redactedDestinationRef =
      body.redactedDestinationRef ??
      `destination.redacted.nexus_pylon.${suffix}`
    const providerRef =
      body.providerRef ?? `provider.public.nexus_pylon.${suffix}`
    const redactedPaymentRef = firstRef(
      paymentRefs,
      `payment.redacted.nexus_pylon.${suffix}`,
    )
    const settlementResultRef = firstRef(
      settlementRefs,
      `settlement.public.nexus_pylon.${suffix}`,
    )
    const idempotencyKeyHash = yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () =>
        sha256Hex(
          `nexus-pylon-settlement-bridge:${assignmentRef}:${idempotencyKey}`,
        ),
    })
    const hashRef = `hash.${idempotencyKeyHash.slice(0, 64)}`

    yield* Effect.try({
      catch: error =>
        error instanceof NexusPylonVisibilityBridgeBlocked
          ? error
          : new NexusPylonVisibilityUnsafe({
              reason: 'Nexus/Pylon bridge public-safe validation failed.',
            }),
      try: () => {
        validateBridgeRefs([
          { label: 'assignmentRef', value: assignmentRef },
          { label: 'artanisDispatchRef', value: body.artanisDispatchRef },
          { label: 'buyerPaymentRef', value: body.buyerPaymentRef },
          {
            label: 'payoutTargetApprovalRef',
            value: body.payoutTargetApprovalRef,
          },
          { label: 'payoutTargetRef', value: body.payoutTargetRef },
          { label: 'policySnapshotRef', value: body.policySnapshotRef },
          { label: 'providerRef', value: providerRef },
          { label: 'pylonJobRef', value: body.pylonJobRef },
          { label: 'pylonRef', value: pylonRef },
          { label: 'receiptRef', value: receiptRef },
          { label: 'redactedDestinationRef', value: redactedDestinationRef },
          { label: 'redactedPaymentRef', value: redactedPaymentRef },
          { label: 'settlementResultRef', value: settlementResultRef },
        ])
        uniqueRefs([
          ...acceptedWorkRefs,
          ...metadataRefs,
          payoutIntentRef,
          payoutAttemptRef,
          reconciliationEventRef,
        ]).forEach(ref => assertBridgeSafeRef('event evidence ref', ref))
      },
    })

    if (spendCap.amountMinorUnits < amount.amountMinorUnits) {
      return yield* bridgeBlocked('amountSats must not exceed spendCapSats.')
    }

    if (acceptedEvents(events).length === 0) {
      return yield* bridgeBlocked(
        'Assignment must include an accepted Pylon assignment event.',
      )
    }

    if (acceptedWorkRefs.length === 0) {
      return yield* bridgeBlocked(
        'Assignment must include public accepted-work references.',
      )
    }

    if (artifactRefs.length === 0) {
      return yield* bridgeBlocked(
        'Assignment must include public artifact or proof references.',
      )
    }

    if (paymentRefs.length === 0 || settlementRefs.length === 0) {
      return yield* bridgeBlocked(
        'Assignment must include public payment and settlement references.',
      )
    }

    const existingReceipt = yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () => ledgerStore.readPaymentAuthorityReceiptByRef(receiptRef),
    })

    if (existingReceipt !== undefined) {
      const existingIntent = yield* Effect.tryPromise({
        catch: ledgerReadError,
        try: () =>
          ledgerStore.readPayoutIntentByRef(existingReceipt.payoutIntentRef),
      })
      const existingAttempt =
        existingReceipt.payoutAttemptRef === null
          ? undefined
          : yield* Effect.tryPromise({
              catch: ledgerReadError,
              try: () =>
                ledgerStore.readPayoutAttemptByRef(
                  existingReceipt.payoutAttemptRef!,
                ),
            })
      const existingEvent =
        existingReceipt.eventRef === null
          ? undefined
          : yield* Effect.tryPromise({
              catch: ledgerReadError,
              try: () =>
                ledgerStore.readReconciliationEventByRef(
                  existingReceipt.eventRef!,
                ),
            })

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({
          bridge: {
            assignmentRef,
            eventCount: events.length,
            idempotent: true,
            receipt: nexusPylonPublicReceiptDetailFromLedger({
              appUrl: appUrlFromRequest(request),
              attempt: existingAttempt,
              event: existingEvent,
              intent: existingIntent,
              nowIso,
              receipt: existingReceipt,
            }),
          },
          schemaVersion:
            'openagents.nexus_pylon.assignment_settlement_bridge.v1',
        }),
        session,
      )
    }

    const targetApproval: NexusPayoutTargetApprovalRecord = {
      agentRef: 'agent.artanis',
      approvalPolicyRef: 'policy.public.nexus_pylon.operator_bridge',
      approvalRef: body.payoutTargetApprovalRef,
      approvedByRef: 'operator.openagents.nexus_pylon_bridge',
      archivedAt: null,
      createdAt: nowIso,
      expiresAt: null,
      id: `nexus_payout_target_approval_${suffix}`,
      idempotencyKeyHash: `${hashRef}.approval`,
      ownerUserId: 'user_openagents_operator',
      publicProjectionJson: JSON.stringify({
        assignmentRef,
        pylonRef,
        state: 'active',
      }),
      payoutTargetRef: body.payoutTargetRef,
      pylonRef,
      redactedDestinationRef,
      scopeRefs: [assignmentRef],
      status: 'active',
      updatedAt: nowIso,
    }
    const intent: NexusTreasuryPayoutIntentRecord = {
      acceptedWorkRefs,
      actorRef: 'agent.artanis',
      adapterKind: body.adapterKind ?? 'mdk_agent_wallet',
      amount,
      archivedAt: null,
      artanisDispatchRef: body.artanisDispatchRef ?? null,
      assignmentRef,
      buyerPaymentRef: body.buyerPaymentRef ?? null,
      createdAt: nowIso,
      id: `nexus_treasury_payout_intent_${suffix}`,
      idempotencyKeyHash: `${hashRef}.intent`,
      metadataRefs,
      ownerUserId: 'user_openagents_operator',
      payoutIntentRef,
      payoutTargetApprovalRef: body.payoutTargetApprovalRef,
      payoutTargetRef: body.payoutTargetRef,
      policySnapshotRef: body.policySnapshotRef,
      publicProjectionJson: JSON.stringify({
        assignmentRef,
        moneyMovement: 'real_bitcoin',
        pylonRef,
        state: 'settled',
      }),
      pylonJobRef: body.pylonJobRef ?? null,
      sourceKind: 'pylon_marketplace_assignment',
      spendCap,
      status: 'settled',
      updatedAt: nowIso,
    }
    const attempt: NexusTreasuryPayoutAttemptRecord = {
      adapterAttemptRef: `adapter_attempt.${intent.adapterKind}.${suffix}`,
      adapterKind: intent.adapterKind,
      amount,
      archivedAt: null,
      createdAt: nowIso,
      id: `nexus_treasury_payout_attempt_${suffix}`,
      idempotencyKeyHash: `${hashRef}.attempt`,
      metadataRefs: uniqueRefs([...paymentRefs, ...settlementRefs]),
      payoutAttemptRef,
      payoutIntentRef,
      publicProjectionJson: JSON.stringify({
        assignmentRef,
        moneyMovement: 'real_bitcoin',
        pylonRef,
        state: 'confirmed',
      }),
      redactedDestinationRef,
      redactedPaymentRef,
      status: 'confirmed',
      updatedAt: nowIso,
    }
    const event: NexusTreasuryPayoutReconciliationEventRecord = {
      adapterKind: intent.adapterKind,
      archivedAt: null,
      createdAt: nowIso,
      eventRef: reconciliationEventRef,
      externalEventRef: redactedPaymentRef,
      id: `nexus_treasury_reconciliation_${suffix}`,
      idempotencyKeyHash: `${hashRef}.reconciliation`,
      metadataRefs: uniqueRefs([...paymentRefs, ...settlementRefs]),
      payoutAttemptRef,
      payoutIntentRef,
      providerRef,
      publicProjectionJson: JSON.stringify({
        assignmentRef,
        moneyMovement: 'real_bitcoin',
        providerRef,
        pylonRef,
        state: 'matched',
      }),
      resultRef: settlementResultRef,
      status: 'matched',
    }
    const receipt: NexusPaymentAuthorityReceiptRecord = {
      archivedAt: null,
      audience: 'public',
      createdAt: nowIso,
      eventRef: event.eventRef,
      id: `nexus_payment_authority_receipt_${suffix}`,
      metadataRefs: uniqueRefs([...paymentRefs, ...settlementRefs]),
      payoutAttemptRef,
      payoutIntentRef,
      publicProjectionJson: JSON.stringify({
        amountSats: body.amountSats,
        assignmentRef,
        moneyMovement: 'real_bitcoin',
        providerRef,
        pylonRef,
        state: 'settled',
      }),
      receiptKind: 'settlement_recorded',
      receiptRef,
    }

    yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: async () => {
        await ledgerStore.createPayoutTargetApproval(targetApproval)
        await ledgerStore.createPayoutIntent(intent)
        await ledgerStore.createPayoutAttempt(attempt)
        await ledgerStore.createReconciliationEvent(event)
        await ledgerStore.createPaymentAuthorityReceipt(receipt)
      },
    })

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(
        {
          bridge: {
            assignmentRef,
            eventCount: events.length,
            idempotent: false,
            payoutAttempt: projectNexusTreasuryPayoutLedgerRecord(
              'attempt',
              attempt,
              'operator',
            ),
            payoutIntent: projectNexusTreasuryPayoutLedgerRecord(
              'intent',
              intent,
              'operator',
            ),
            receipt: nexusPylonPublicReceiptDetailFromLedger({
              appUrl: appUrlFromRequest(request),
              attempt,
              event,
              intent,
              nowIso,
              receipt,
            }),
          },
          schemaVersion:
            'openagents.nexus_pylon.assignment_settlement_bridge.v1',
        },
        { status: 201 },
      ),
      session,
    )
  })

const operatorAssignmentProofRun = <
  Session extends NexusPylonVisibilitySession,
  Bindings,
>(
  dependencies: NexusPylonVisibilityDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* requireAdminSession(dependencies, request, env, ctx)
    const idempotencyKey = request.headers.get('Idempotency-Key')?.trim()

    if (idempotencyKey === undefined || idempotencyKey === '') {
      return yield* bridgeBlocked('Idempotency-Key header is required.')
    }

    const body = yield* Effect.tryPromise({
      catch: error =>
        error instanceof Error
          ? bridgeBlocked(error.message)
          : bridgeBlocked('Proof-run request body could not be decoded.'),
      try: async () =>
        decodeUnknownWithSchema(ProofRunRequest, await readJsonObject(request)),
    })

    yield* Effect.try({
      catch: error =>
        error instanceof NexusPylonVisibilityBridgeBlocked
          ? error
          : new NexusPylonVisibilityUnsafe({
              reason: 'Nexus/Pylon proof-run public-safe validation failed.',
            }),
      try: () =>
        validateBridgeRefs([
          { label: 'assignmentRef', value: body.assignmentRef },
          { label: 'artanisRunRef', value: body.artanisRunRef },
          { label: 'buyerPaymentRef', value: body.buyerPaymentRef },
          {
            label: 'payoutTargetApprovalRef',
            value: body.payoutTargetApprovalRef,
          },
          { label: 'payoutTargetRef', value: body.payoutTargetRef },
          { label: 'policySnapshotRef', value: body.policySnapshotRef },
          { label: 'providerRef', value: body.providerRef },
          { label: 'pylonJobRef', value: body.pylonJobRef },
          {
            label: 'redactedDestinationRef',
            value: body.redactedDestinationRef,
          },
          { label: 'settlementIntentRef', value: body.settlementIntentRef },
        ]),
    })

    const makePylonApiStore = dependencies.makePylonApiStore

    if (makePylonApiStore === undefined) {
      return yield* new NexusPylonVisibilityUnsafe({
        reason: 'Nexus/Pylon proof-run Pylon event store is not configured.',
      })
    }

    const pylonStore = makePylonApiStore(env)
    const events = yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () => pylonStore.listEventsForAssignment(body.assignmentRef, 100),
    })
    const nowIso = nowIsoFor(dependencies)
    const idempotencyKeyHash = yield* Effect.tryPromise({
      catch: ledgerReadError,
      try: () =>
        sha256Hex(
          `nexus-pylon-proof-run:${body.assignmentRef}:${idempotencyKey}`,
        ),
    })
    const proofRunRef = `proof_run.public.artanis_pylon.${bridgeSuffix(body.assignmentRef)}.${idempotencyKeyHash.slice(
      0,
      16,
    )}`
    const preTrace = projectArtanisPylonProofTrace(
      proofTraceRecord(body, events, null, nowIso),
      'operator',
      nowIso,
    )
    const bridgeHeaders = new Headers()
    const authorization = request.headers.get('authorization')

    if (authorization !== null) {
      bridgeHeaders.set('authorization', authorization)
    }

    bridgeHeaders.set('content-type', 'application/json')
    bridgeHeaders.set('idempotency-key', idempotencyKey)

    const bridgeRequest = new Request(
      `${new URL(request.url).origin}/api/operator/nexus-pylon/assignments/${encodeURIComponent(
        body.assignmentRef,
      )}/settlement-bridges`,
      {
        body: JSON.stringify({
          adapterKind: body.adapterKind,
          amountSats: body.amountSats,
          artanisDispatchRef: body.artanisRunRef,
          buyerPaymentRef: body.buyerPaymentRef,
          payoutTargetApprovalRef: body.payoutTargetApprovalRef,
          payoutTargetRef: body.payoutTargetRef,
          policySnapshotRef: body.policySnapshotRef,
          providerRef: body.providerRef,
          pylonJobRef: body.pylonJobRef,
          redactedDestinationRef: body.redactedDestinationRef,
          spendCapSats: body.spendCapSats,
        }),
        headers: bridgeHeaders,
        method: 'POST',
      },
    )
    const bridgeResponse = yield* operatorAssignmentSettlementBridge(
      dependencies,
      bridgeRequest,
      env,
      ctx,
      encodeURIComponent(body.assignmentRef),
    ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    const bridgeBody = yield* Effect.tryPromise({
      catch: () =>
        new NexusPylonVisibilityUnsafe({
          reason: 'Nexus/Pylon proof-run bridge response could not be decoded.',
        }),
      try: () => bridgeResponse.clone().json() as Promise<Record<string, any>>,
    })
    const receiptDetail =
      bridgeResponse.status >= 200 &&
      bridgeResponse.status < 300 &&
      bridgeBody.bridge?.receipt !== undefined
        ? (bridgeBody.bridge.receipt as NexusPylonPublicReceiptDetail)
        : null
    const postTrace = projectArtanisPylonProofTrace(
      proofTraceRecord(body, events, receiptDetail, nowIso),
      'operator',
      nowIso,
    )
    const responseStatus =
      bridgeResponse.status >= 400
        ? bridgeResponse.status
        : bridgeResponse.status

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(
        {
          proofRun: {
            bridge: bridgeBody,
            bridgeStatus: bridgeResponse.status,
            idempotent: bridgeBody.bridge?.idempotent === true,
            postTrace,
            preTrace,
            proofRunRef,
            publicReceiptUrl: receiptDetail?.receiptPageUrl ?? null,
          },
          schemaVersion: 'openagents.nexus_pylon.assignment_proof_run.v1',
        },
        { status: responseStatus },
      ),
      session,
    )
  })

export const makeNexusPylonVisibilityRoutes = <
  Session extends NexusPylonVisibilitySession,
  Bindings,
>(
  dependencies: NexusPylonVisibilityDependencies<Session, Bindings>,
) => {
  const routeNexusPylonVisibilityRequest = (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const { pathname } = new URL(request.url)
    const publicReceiptApiRef = receiptRefFromPath(
      pathname,
      '/api/public/nexus-pylon/receipts/',
    )
    const publicReceiptPageRef = receiptRefFromPath(
      pathname,
      '/nexus-pylon/receipts/',
    )
    const operatorReceiptApiRef = receiptRefFromPath(
      pathname,
      '/api/operator/nexus-pylon/receipts/',
    )
    const operatorProofRunMatch =
      /^\/api\/operator\/nexus-pylon\/proof-runs$/.exec(pathname)
    const operatorAssignmentAcceptedWorkPayoutMatch =
      /^\/api\/operator\/nexus-pylon\/assignments\/([^/]+)\/accepted-work-payouts$/.exec(
        pathname,
      )
    const operatorAssignmentBridgeMatch =
      /^\/api\/operator\/nexus-pylon\/assignments\/([^/]+)\/settlement-bridges$/.exec(
        pathname,
      )

    if (operatorProofRunMatch !== null) {
      return request.method !== 'POST'
        ? Effect.succeed(methodNotAllowed(['POST']))
        : operatorAssignmentProofRun(dependencies, request, env, ctx).pipe(
            Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
          )
    }

    if (operatorAssignmentAcceptedWorkPayoutMatch !== null) {
      return request.method !== 'POST'
        ? Effect.succeed(methodNotAllowed(['POST']))
        : operatorAssignmentAcceptedWorkPayout(
            dependencies,
            request,
            env,
            ctx,
            operatorAssignmentAcceptedWorkPayoutMatch[1]!,
          ).pipe(
            Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
          )
    }

    if (operatorAssignmentBridgeMatch !== null) {
      return request.method !== 'POST'
        ? Effect.succeed(methodNotAllowed(['POST']))
        : operatorAssignmentSettlementBridge(
            dependencies,
            request,
            env,
            ctx,
            operatorAssignmentBridgeMatch[1]!,
          ).pipe(
            Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
          )
    }

    if (publicReceiptApiRef !== undefined) {
      return request.method !== 'GET'
        ? Effect.succeed(methodNotAllowed(['GET']))
        : publicReceiptJson(
            request,
            env,
            dependencies,
            publicReceiptApiRef,
            nowIsoFor(dependencies),
          ).pipe(
            Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
          )
    }

    if (publicReceiptPageRef !== undefined) {
      return request.method !== 'GET'
        ? Effect.succeed(methodNotAllowed(['GET']))
        : publicReceiptPage(
            request,
            env,
            dependencies,
            publicReceiptPageRef,
            nowIsoFor(dependencies),
          ).pipe(
            Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
          )
    }

    if (pathname === '/api/operator/nexus-pylon/dashboard') {
      return request.method !== 'GET'
        ? Effect.succeed(methodNotAllowed(['GET']))
        : operatorDashboard(dependencies, request, env, ctx).pipe(
            Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
          )
    }

    if (operatorReceiptApiRef !== undefined) {
      return request.method !== 'GET'
        ? Effect.succeed(methodNotAllowed(['GET']))
        : operatorReceiptJson(
            dependencies,
            request,
            env,
            ctx,
            operatorReceiptApiRef,
          ).pipe(
            Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
          )
    }

    return undefined
  }

  return { routeNexusPylonVisibilityRequest }
}
