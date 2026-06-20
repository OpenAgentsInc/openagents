// Card -> credit -> inference-spend receipt RESOLVER seam
// (blocker.product_promises.inference_card_credit_inference_spend_receipt_missing).
//
// THE GAP this closes: `assembleCardCreditSpendReceipt` (card-credit-spend-receipt.ts)
// is a PURE linker that needs all three already-resolved ledger legs handed to
// it. Nothing turned a single Stripe checkout session id into those three legs
// by READING stored ledger state, so the assembler could not yet back a
// resolvable surface. The fleet note for this promise flags exactly this as the
// remaining step ("Wiring the assembler into a resolvable GET endpoint ...").
//
// This module is that seam: given a session id and three injected leg-reader
// functions (the Worker wires them to the real D1 reads; tests inject fixtures),
// it reads each leg, reports HONESTLY which legs have not settled yet, and only
// when the full chain is present hands it to the assembler. It adds NO ledger
// writes and moves no money — it derefs a movement the three real writes already
// made.
//
// PURE apart from the injected readers: no D1, no clock, no network, no secrets.
// A leg that has not settled yet is a normal `pending` outcome (the typical
// state today: a card purchase may exist before the bridge grant, which exists
// before any metered spend), DISTINCT from an `invalid` chain where the legs are
// present but violate a conservation invariant. This keeps a not-yet-complete
// chain from ever masquerading as a dishonest receipt.

import {
  type CardCreditPurchaseLeg,
  type CardCreditSpendReceipt,
  type CreditToMsatGrantLeg,
  type InferenceSpendLeg,
  assembleCardCreditSpendReceipt,
} from './card-credit-spend-receipt'

// One reader per hop. Each returns the resolved leg for the session, or
// `undefined` when that hop has not settled yet. Async to match real D1 reads;
// a pure caller may resolve synchronously. Implementations MUST key strictly by
// the originating Stripe checkout session id so the three legs provably share a
// chain genesis (provenance binding is then re-checked inside the assembler).
export type CardCreditSpendLegReaders = Readonly<{
  readPurchaseLeg: (
    sessionId: string,
  ) => Promise<CardCreditPurchaseLeg | undefined>
  readGrantLeg: (
    sessionId: string,
  ) => Promise<CreditToMsatGrantLeg | undefined>
  readSpendLeg: (
    sessionId: string,
  ) => Promise<InferenceSpendLeg | undefined>
}>

// Which hop is the first one not yet settled, for a `pending` chain. Ordered by
// the only sequence the chain can legitimately settle in.
export type CardCreditSpendPendingLeg = 'purchase' | 'grant' | 'spend'

export type CardCreditSpendReceiptResolution =
  | Readonly<{ ok: true; receipt: CardCreditSpendReceipt }>
  // The session id was blank — there is nothing to resolve.
  | Readonly<{ ok: false; status: 'blank_session' }>
  // The chain is not complete yet: `missing` names the first unsettled hop. This
  // is the EXPECTED state until the paid loop runs end to end; it is NOT an error.
  | Readonly<{
      ok: false
      status: 'pending'
      missing: CardCreditSpendPendingLeg
    }>
  // All three legs are present but violate a conservation invariant; `reason`
  // carries the assembler's typed failure so a caller can refuse to publish a
  // dishonest receipt.
  | Readonly<{
      ok: false
      status: 'invalid'
      reason: Extract<
        ReturnType<typeof assembleCardCreditSpendReceipt>,
        { ok: false }
      >['reason']
      message: string
    }>

const isBlank = (value: string): boolean => value.trim() === ''

// Resolve the dereferenceable card->credit->inference-spend receipt for one
// Stripe checkout session by reading the three real ledger legs and assembling
// them. Returns a typed `pending` (chain incomplete) or `invalid` (chain
// inconsistent) rather than throwing, so a route handler can map each outcome to
// the right HTTP status. The legs are read in chain order and short-circuit on
// the first unsettled hop, so a not-yet-funded session never triggers a
// downstream read.
export const resolveCardCreditSpendReceipt = async (
  sessionId: string,
  readers: CardCreditSpendLegReaders,
): Promise<CardCreditSpendReceiptResolution> => {
  if (isBlank(sessionId)) {
    return { ok: false, status: 'blank_session' }
  }

  const purchase = await readers.readPurchaseLeg(sessionId)
  if (purchase === undefined) {
    return { missing: 'purchase', ok: false, status: 'pending' }
  }

  const grant = await readers.readGrantLeg(sessionId)
  if (grant === undefined) {
    return { missing: 'grant', ok: false, status: 'pending' }
  }

  const spend = await readers.readSpendLeg(sessionId)
  if (spend === undefined) {
    return { missing: 'spend', ok: false, status: 'pending' }
  }

  const assembled = assembleCardCreditSpendReceipt({ grant, purchase, spend })
  if (!assembled.ok) {
    return {
      message: assembled.message,
      ok: false,
      reason: assembled.reason,
      status: 'invalid',
    }
  }

  return { ok: true, receipt: assembled.receipt }
}
