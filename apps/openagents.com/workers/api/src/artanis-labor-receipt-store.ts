import {
  ArtanisLaborReceiptError,
  buildArtanisLaborUnattendedRequestReceipt,
  deriveArtanisLaborUnattendedRequestReceiptRef,
  serializeArtanisLaborUnattendedRequestReceipt,
  verifyArtanisLaborUnattendedRequestReceipt,
  type ArtanisLaborRequestReceiptInput,
  type ArtanisLaborUnattendedRequestReceipt,
} from './artanis-labor-request-receipt'

// Tick-ledger store for consolidated, public-safe Artanis unattended labor
// request receipts (#4731, blocker
// artanis_labor_unattended_request_receipts_missing). The receipt module already
// builds, serializes, content-addresses, parses, and verifies one receipt; what
// was still missing is the persistence boundary an operator route or the tick
// ledger needs: a place to WRITE a sealed receipt keyed by its own content
// address and to READ it back tamper-evidently by that same ref.
//
// This module supplies (1) a `seal` step that folds build -> serialize ->
// derive-ref into ONE artifact a caller can hand to a store, and (2) an
// in-memory store (the reference contract a durable KV/D1 backing can later
// mirror, exactly like the hygiene debt-receipt store does). It mints no
// payment, identity, or settlement authority - it only persists and serves an
// already public-safe, content-addressed projection, re-verifying integrity on
// every write and read so the store never trusts itself to have kept the bytes
// intact.

export type ArtanisLaborSealedReceipt = Readonly<{
  receipt: ArtanisLaborUnattendedRequestReceipt
  receiptRef: string
  serialized: string
}>

// Fold build -> serialize -> derive-ref into one sealed artifact. The receipt is
// content-addressed over its own canonical bytes, so the same lifecycle always
// seals to the same ref (idempotent persistence falls out for free). Throws via
// the underlying builder/serializer on any impossible state or private material.
export const sealArtanisLaborUnattendedRequestReceipt = (
  input: ArtanisLaborRequestReceiptInput,
): ArtanisLaborSealedReceipt => {
  const receipt = buildArtanisLaborUnattendedRequestReceipt(input)
  const serialized = serializeArtanisLaborUnattendedRequestReceipt(receipt)
  const receiptRef = deriveArtanisLaborUnattendedRequestReceiptRef(receipt)
  return { receipt, receiptRef, serialized }
}

// Re-seal a sealed receipt from its serialized bytes alone and confirm the three
// fields agree: the ref must address the bytes (verify) and the typed receipt
// must re-serialize to those same bytes. A sealed receipt that fails this is
// internally inconsistent and is refused before it can be written or returned.
const assertSealedConsistent = (
  sealed: ArtanisLaborSealedReceipt,
): ArtanisLaborSealedReceipt => {
  const verified = verifyArtanisLaborUnattendedRequestReceipt(
    sealed.serialized,
    sealed.receiptRef,
  )
  if (
    serializeArtanisLaborUnattendedRequestReceipt(sealed.receipt) !==
    sealed.serialized
  ) {
    throw new ArtanisLaborReceiptError(
      'Sealed receipt object does not match its serialized bytes.',
    )
  }
  return { receipt: verified, receiptRef: sealed.receiptRef, serialized: sealed.serialized }
}

export type ArtanisLaborReceiptPutResult =
  | Readonly<{ kind: 'stored'; sealed: ArtanisLaborSealedReceipt }>
  | Readonly<{ kind: 'already_stored'; sealed: ArtanisLaborSealedReceipt }>

export type ArtanisLaborUnattendedReceiptStore = Readonly<{
  // Persist a sealed receipt under its content-addressed ref. Idempotent: a
  // re-put of the same lifecycle (identical bytes -> identical ref) returns
  // `already_stored` and never overwrites. Refuses an internally inconsistent
  // sealed receipt.
  put: (
    sealed: ArtanisLaborSealedReceipt,
  ) => Promise<ArtanisLaborReceiptPutResult>
  // Read a stored receipt by ref, re-verifying the persisted bytes still address
  // that ref (tamper-evident). Undefined when absent. Throws if stored bytes no
  // longer verify against the ref they are keyed under.
  get: (
    receiptRef: string,
  ) => Promise<ArtanisLaborSealedReceipt | undefined>
  // All stored receipts in insertion order (deterministic for audit/snapshot).
  list: () => Promise<ReadonlyArray<ArtanisLaborSealedReceipt>>
}>

// In-memory store (tests / fixtures / the reference contract a durable backing
// can mirror). Keys on the content-addressed receipt ref, so persistence is
// idempotent by construction.
export const makeInMemoryArtanisLaborUnattendedReceiptStore =
  (): ArtanisLaborUnattendedReceiptStore & {
    readonly rows: ReadonlyMap<string, ArtanisLaborSealedReceipt>
  } => {
    const rows = new Map<string, ArtanisLaborSealedReceipt>()

    return {
      get: async receiptRef => {
        const existing = rows.get(receiptRef)
        if (existing === undefined) {
          return undefined
        }
        // Re-verify on read: the persisted bytes must still address this ref.
        return assertSealedConsistent(existing)
      },
      list: async () => [...rows.values()],
      put: async sealed => {
        const checked = assertSealedConsistent(sealed)
        const existing = rows.get(checked.receiptRef)
        if (existing !== undefined) {
          return { kind: 'already_stored', sealed: existing }
        }
        rows.set(checked.receiptRef, checked)
        return { kind: 'stored', sealed: checked }
      },
      rows,
    }
  }
