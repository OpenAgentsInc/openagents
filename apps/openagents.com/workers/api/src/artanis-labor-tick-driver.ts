import {
  sealArtanisLaborUnattendedRequestReceipt,
  type ArtanisLaborReceiptPutResult,
  type ArtanisLaborSealedReceipt,
  type ArtanisLaborUnattendedReceiptStore,
} from './artanis-labor-receipt-store'
import {
  handleArtanisLaborResultDelivery,
  runArtanisLaborRequestTick,
  type ArtanisLaborAcceptanceDeps,
  type ArtanisLaborAcceptanceOutcome,
  type ArtanisLaborRequesterDeps,
  type ArtanisLaborRequesterOutcome,
  type ArtanisLaborResultDelivery,
} from './artanis-labor-requester'

// Tick driver for the consolidated Artanis unattended labor receipt (#4731,
// blocker artanis_labor_unattended_request_receipts_missing). The requester
// surface runs a gated tick and returns a typed outcome; the receipt module
// folds that outcome into one public-safe, content-addressed receipt; the store
// persists it tamper-evidently; the route serves it. What was still missing is
// the seam that ties those four together so a REAL gated tick (not a fixture)
// ends with a sealed receipt in the durable store - which is exactly what makes
// the public feed serve receipts minted by an actual run.
//
// This driver mints no payment, identity, or settlement authority: it only runs
// the already-gated requester surface, seals the public-safe projection of its
// outcome, and persists it idempotently by content address. A skipped or refused
// tick is sealed and persisted too, so an operator can audit that the gates ran
// even when nothing was placed.

// A 'requested' outcome is the only one that can later resolve into an
// acceptance, so the delivery driver narrows to it at the type level.
export type ArtanisLaborRequestedOutcome = Extract<
  ArtanisLaborRequesterOutcome,
  { kind: 'requested' }
>

export type ArtanisLaborPersistedTick = Readonly<{
  requestOutcome: ArtanisLaborRequesterOutcome
  sealed: ArtanisLaborSealedReceipt
  put: ArtanisLaborReceiptPutResult
}>

export type ArtanisLaborPersistedDelivery = Readonly<{
  acceptanceOutcome: ArtanisLaborAcceptanceOutcome
  sealed: ArtanisLaborSealedReceipt
  put: ArtanisLaborReceiptPutResult
}>

// Run one gated request tick and persist its consolidated receipt. Every
// terminal state (skipped / refused / requested_pending_delivery) is sealed and
// stored, so the audit feed reflects that the gates ran on this tick regardless
// of whether a work request was placed. Persistence is idempotent by content
// address: re-running the same tick (same outcome, tickRef, and clock) is a
// no-op `already_stored`.
export const runAndPersistArtanisLaborRequestTick = async (
  input: Readonly<{
    store: ArtanisLaborUnattendedReceiptStore
    requesterDeps: ArtanisLaborRequesterDeps
    artanisActorRef: string
    tickRef: string
  }>,
): Promise<ArtanisLaborPersistedTick> => {
  const requestOutcome = await runArtanisLaborRequestTick(input.requesterDeps)
  const sealed = sealArtanisLaborUnattendedRequestReceipt({
    artanisActorRef: input.artanisActorRef,
    nowIso: input.requesterDeps.nowIso,
    requestOutcome,
    tickRef: input.tickRef,
  })
  const put = await input.store.put(sealed)
  return { put, requestOutcome, sealed }
}

// Resolve a delivered result for a previously-placed request and persist the
// consolidated receipt that folds the original request stage together with the
// validator-pass release or validator-fail refund. The caller supplies the
// original 'requested' outcome so the persisted receipt carries the full
// propose -> reserve -> validate -> release/refund lifecycle, not just the
// resolution stage. Persistence is idempotent by content address.
export const resolveAndPersistArtanisLaborDelivery = async (
  input: Readonly<{
    store: ArtanisLaborUnattendedReceiptStore
    acceptanceDeps: ArtanisLaborAcceptanceDeps
    delivery: ArtanisLaborResultDelivery
    requestOutcome: ArtanisLaborRequestedOutcome
    artanisActorRef: string
    nowIso: string
    tickRef: string
  }>,
): Promise<ArtanisLaborPersistedDelivery> => {
  const acceptanceOutcome = await handleArtanisLaborResultDelivery(
    input.delivery,
    input.acceptanceDeps,
  )
  const sealed = sealArtanisLaborUnattendedRequestReceipt({
    acceptanceOutcome,
    artanisActorRef: input.artanisActorRef,
    nowIso: input.nowIso,
    requestOutcome: input.requestOutcome,
    tickRef: input.tickRef,
  })
  const put = await input.store.put(sealed)
  return { acceptanceOutcome, put, sealed }
}
