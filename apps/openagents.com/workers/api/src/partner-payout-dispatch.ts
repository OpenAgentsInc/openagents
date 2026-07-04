import {
  type PartnerPayoutLedgerEntry,
  type PartnerPayoutLedgerStorageError,
  type PartnerPayoutLedgerValidationError,
  readCurrentPartnerPayout,
  transitionPartnerPayout,
} from './partner-payout-ledger'
import type { MdkPayoutModeGateProjection } from './mdk-payout-mode-gate'
import {
  treasuryAuthorityDb,
  type TreasuryDatabase,
} from './treasury-domain-store'
import { isoTimestampAfterIso } from './runtime-primitives'

export type PartnerPayoutAdapter = Readonly<{
  adapterKind: string
  dispatch: (input: {
    amountSats: number
    idempotencyKey: string
    payoutRef: string
  }) => Promise<{ receiptRef: string }>
}>

export type PartnerPayoutDispatchDependencies = Readonly<{
  adapter: PartnerPayoutAdapter
  nowIso: () => string
  readReadiness: () => Promise<MdkPayoutModeGateProjection>
}>

export type PartnerPayoutDispatchInput = Readonly<{
  payoutRef: string
}>

export type PartnerPayoutDispatchOutcome =
  | Readonly<{
      _tag: 'settled'
      entry: PartnerPayoutLedgerEntry
      receiptRef: string
    }>
  | Readonly<{
      _tag: 'already_settled'
      entry: PartnerPayoutLedgerEntry
    }>
  | Readonly<{
      _tag: 'refused'
      entry: PartnerPayoutLedgerEntry | null
      reasonRef: string
    }>

export class PartnerPayoutDispatchError extends Error {
  readonly _tag = 'PartnerPayoutDispatchError'
  readonly reason: string

  constructor(reason: string, cause?: unknown) {
    super(reason, cause === undefined ? undefined : { cause })
    this.name = 'PartnerPayoutDispatchError'
    this.reason = reason
  }
}

export type PartnerPayoutDispatchFailure =
  | PartnerPayoutDispatchError
  | PartnerPayoutLedgerStorageError
  | PartnerPayoutLedgerValidationError

const SAFE_RECEIPT_REF_PATTERN =
  /^receipt\.partner_payout\.[A-Za-z0-9][A-Za-z0-9_.:/-]{0,300}$/

const approveKey = (payoutRef: string): string =>
  `partner_payout_dispatch.approve.${payoutRef}`
const dispatchedKey = (payoutRef: string): string =>
  `partner_payout_dispatch.dispatched.${payoutRef}`
const settledKey = (payoutRef: string): string =>
  `partner_payout_dispatch.settled.${payoutRef}`
const adapterIdempotencyKey = (payoutRef: string): string =>
  `partner_payout.adapter.${payoutRef}`

export const dispatchPartnerPayoutSettlement = async (
  db: TreasuryDatabase,
  dependencies: PartnerPayoutDispatchDependencies,
  input: PartnerPayoutDispatchInput,
): Promise<PartnerPayoutDispatchOutcome> => {
  const current = await readCurrentPartnerPayout(
    treasuryAuthorityDb(db),
    input.payoutRef,
  )

  if (current === null) {
    return {
      _tag: 'refused',
      entry: null,
      reasonRef: 'reason.public.partner_payout.unknown_payout_ref',
    }
  }

  if (current.state === 'settled') {
    return { _tag: 'already_settled', entry: current }
  }

  if (
    current.state !== 'eligible' &&
    current.state !== 'approved' &&
    current.state !== 'dispatched'
  ) {
    return {
      _tag: 'refused',
      entry: current,
      reasonRef: `reason.public.partner_payout.not_dispatchable_state.${current.state}`,
    }
  }

  if (current.amount <= 0) {
    return {
      _tag: 'refused',
      entry: current,
      reasonRef: 'reason.public.partner_payout.no_qualifying_paid_amount',
    }
  }

  if (current.asset !== 'sats') {
    return {
      _tag: 'refused',
      entry: current,
      reasonRef:
        'reason.public.partner_payout.non_sats_asset_not_withdrawable_bitcoin',
    }
  }

  const readiness = await dependencies.readReadiness()

  if (!readiness.livePayoutClaimAllowed) {
    return {
      _tag: 'refused',
      entry: current,
      reasonRef: 'reason.public.partner_payout.payout_target_not_ready',
    }
  }

  const approveIso = dependencies.nowIso()
  const dispatchedIso = isoTimestampAfterIso(approveIso, 1)
  const settledIso = isoTimestampAfterIso(approveIso, 2)

  let entry = current

  if (entry.state === 'eligible') {
    entry = await transitionPartnerPayout(db, {
      action: 'approve_dispatch',
      idempotencyKey: approveKey(input.payoutRef),
      nowIso: approveIso,
      payoutRef: input.payoutRef,
      stateReasonRef: 'reason.public.partner_payout.operator_dispatch_approved',
    })
  }

  if (entry.state === 'approved') {
    entry = await transitionPartnerPayout(db, {
      action: 'mark_dispatched',
      idempotencyKey: dispatchedKey(input.payoutRef),
      nowIso: dispatchedIso,
      payoutRef: input.payoutRef,
      stateReasonRef: 'reason.public.partner_payout.dispatch_requested_to_adapter',
    })
  }

  let receiptRef: string

  try {
    const result = await dependencies.adapter.dispatch({
      amountSats: entry.amount,
      idempotencyKey: adapterIdempotencyKey(input.payoutRef),
      payoutRef: input.payoutRef,
    })
    receiptRef = result.receiptRef
  } catch (cause) {
    throw new PartnerPayoutDispatchError(
      'partner_payout_adapter_dispatch_failed',
      cause,
    )
  }

  if (!SAFE_RECEIPT_REF_PATTERN.test(receiptRef)) {
    throw new PartnerPayoutDispatchError(
      'partner_payout_adapter_returned_unsafe_receipt_ref',
    )
  }

  const settled = await transitionPartnerPayout(db, {
    action: 'mark_settled',
    evidenceRefs: [
      receiptRef,
      `evidence.partner_payout.adapter.${dependencies.adapter.adapterKind}`,
    ],
    idempotencyKey: settledKey(input.payoutRef),
    nowIso: settledIso,
    payoutRef: input.payoutRef,
    stateReasonRef: 'reason.public.partner_payout.settled_with_adapter_receipt',
  })

  return { _tag: 'settled', entry: settled, receiptRef }
}
