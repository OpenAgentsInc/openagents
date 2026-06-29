import { createHash } from 'node:crypto'

import {
  assertArtanisLaborPublicSafe,
  type ArtanisLaborAcceptanceOutcome,
  type ArtanisLaborRequesterOutcome,
} from './artanis-labor-requester'
import { parseJsonUnknown } from './json-boundary'

// Consolidated, public-safe receipt for a single unattended Artanis labor
// request lifecycle (#4731, blocker
// artanis_labor_unattended_request_receipts_missing). The requester surface
// already emits per-stage tick receipts through injected callbacks and returns
// typed outcomes; this module folds the propose/reserve and the
// validate/release-or-refund outcomes into ONE dereferenceable receipt an
// operator or reviewer can read to confirm an unattended tick ran the whole
// flow under its gates. It only projects refs that already exist on the
// outcomes - it mints no payment, identity, or settlement authority and carries
// no private or payment material (assertArtanisLaborPublicSafe holds the line).

export type ArtanisLaborReceiptTerminalState =
  | 'skipped_config_disabled'
  | 'refused'
  | 'requested_pending_delivery'
  | 'accepted_released'
  | 'rejected_refunded'

export type ArtanisLaborUnattendedRequestReceipt = Readonly<{
  artanisActorRef: string
  budgetMsat: number | null
  issuedAtIso: string
  lifecycleRefs: ReadonlyArray<string>
  schema: 'artanis.labor.unattended_request_receipt.v1'
  terminalState: ArtanisLaborReceiptTerminalState
  tickRef: string
  workRequestId: string | null
}>

export type ArtanisLaborRequestReceiptInput = Readonly<{
  acceptanceOutcome?: ArtanisLaborAcceptanceOutcome | undefined
  artanisActorRef: string
  nowIso: string
  requestOutcome: ArtanisLaborRequesterOutcome
  tickRef: string
}>

export class ArtanisLaborReceiptError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ArtanisLaborReceiptError'
  }
}

const requireNonEmpty = (value: string, label: string): string => {
  if (value.trim().length === 0) {
    throw new ArtanisLaborReceiptError(`${label} must be a non-empty ref.`)
  }
  return value
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> => {
  const seen = new Set<string>()
  const out: string[] = []
  for (const ref of refs) {
    if (ref.trim().length === 0 || seen.has(ref)) {
      continue
    }
    seen.add(ref)
    out.push(ref)
  }
  return out
}

// Build the consolidated lifecycle receipt. Throws on impossible state
// combinations (e.g. an acceptance outcome for a request that never reserved
// escrow) so a malformed receipt can never be projected.
export const buildArtanisLaborUnattendedRequestReceipt = (
  input: ArtanisLaborRequestReceiptInput,
): ArtanisLaborUnattendedRequestReceipt => {
  const tickRef = requireNonEmpty(input.tickRef, 'tickRef')
  const artanisActorRef = requireNonEmpty(input.artanisActorRef, 'artanisActorRef')
  const issuedAtIso = requireNonEmpty(input.nowIso, 'nowIso')

  const { acceptanceOutcome, requestOutcome } = input

  const base = {
    artanisActorRef,
    issuedAtIso,
    schema: 'artanis.labor.unattended_request_receipt.v1' as const,
    tickRef,
  }

  if (requestOutcome.kind !== 'requested' && acceptanceOutcome !== undefined) {
    throw new ArtanisLaborReceiptError(
      'Acceptance outcome requires a requested labor request.',
    )
  }

  let receipt: ArtanisLaborUnattendedRequestReceipt
  switch (requestOutcome.kind) {
    case 'skipped': {
      receipt = {
        ...base,
        budgetMsat: null,
        lifecycleRefs: ['stage.artanis_labor_request.skipped.config_disabled'],
        terminalState: 'skipped_config_disabled',
        workRequestId: null,
      }
      break
    }
    case 'refused': {
      receipt = {
        ...base,
        budgetMsat: null,
        lifecycleRefs: uniqueRefs([
          'stage.artanis_labor_request.refused',
          requestOutcome.refusalRef,
        ]),
        terminalState: 'refused',
        workRequestId: null,
      }
      break
    }
    case 'requested': {
      const proposedRefs = uniqueRefs([
        'stage.artanis_labor_request.proposed',
        `work_request.public.${requestOutcome.receipt.workRequestId}`,
        `nostr.event.${requestOutcome.receipt.jobEventId}`,
        requestOutcome.reserveReceiptRef,
      ])

      if (acceptanceOutcome === undefined) {
        receipt = {
          ...base,
          budgetMsat: requestOutcome.budgetMsat,
          lifecycleRefs: proposedRefs,
          terminalState: 'requested_pending_delivery',
          workRequestId: requestOutcome.receipt.workRequestId,
        }
        break
      }

      receipt = {
        ...base,
        budgetMsat: requestOutcome.budgetMsat,
        lifecycleRefs: uniqueRefs(
          acceptanceOutcome.kind === 'accepted'
            ? [
                ...proposedRefs,
                'stage.artanis_labor_request.accepted',
                acceptanceOutcome.releaseReceiptRef,
              ]
            : [
                ...proposedRefs,
                'stage.artanis_labor_request.rejected_refunded',
                acceptanceOutcome.reasonRef,
                acceptanceOutcome.refundReceiptRef,
              ],
        ),
        terminalState:
          acceptanceOutcome.kind === 'accepted'
            ? 'accepted_released'
            : 'rejected_refunded',
        workRequestId: requestOutcome.receipt.workRequestId,
      }
      break
    }
  }

  assertArtanisLaborPublicSafe(receipt)
  return receipt
}

// Canonical, deterministic wire form of a consolidated receipt. Top-level keys
// are emitted in a fixed (alphabetical) order so the same lifecycle always
// serializes to the same bytes; `lifecycleRefs` keeps its array order because
// that order encodes the propose -> reserve -> validate -> release/refund
// sequence and is meaningful, not incidental. Re-runs the public-safety guard so
// nothing private can leak into a persisted or transported form.
export const serializeArtanisLaborUnattendedRequestReceipt = (
  receipt: ArtanisLaborUnattendedRequestReceipt,
): string => {
  assertArtanisLaborPublicSafe(receipt)
  const canonical = {
    artanisActorRef: receipt.artanisActorRef,
    budgetMsat: receipt.budgetMsat,
    issuedAtIso: receipt.issuedAtIso,
    lifecycleRefs: [...receipt.lifecycleRefs],
    schema: receipt.schema,
    terminalState: receipt.terminalState,
    tickRef: receipt.tickRef,
    workRequestId: receipt.workRequestId,
  }
  return JSON.stringify(canonical)
}

// Content-addressed identity for a consolidated receipt. The receipt projection
// itself carries no id, so it could not be persisted alongside the tick ledger
// nor dereferenced from a public route. This mints a stable, collision-resistant
// ref over the canonical serialization (same lifecycle -> same ref) so an
// operator or reviewer can address one unattended tick's receipt by name. It
// derives no payment, identity, or settlement authority - it is a name for an
// already public-safe artifact.
export const deriveArtanisLaborUnattendedRequestReceiptRef = (
  receipt: ArtanisLaborUnattendedRequestReceipt,
): string => {
  const digest = createHash('sha256')
    .update(serializeArtanisLaborUnattendedRequestReceipt(receipt), 'utf8')
    .digest('hex')
  return `receipt.artanis_labor.unattended_request.${digest.slice(0, 16)}`
}

const TERMINAL_STATES: ReadonlyArray<ArtanisLaborReceiptTerminalState> = [
  'skipped_config_disabled',
  'refused',
  'requested_pending_delivery',
  'accepted_released',
  'rejected_refunded',
]

// Terminal states that name a placed work request: they MUST carry a numeric
// budget and a work-request id. The pre-request terminals (skipped/refused) MUST
// carry neither, because no escrow was ever reserved. Enforcing this on read
// means a tampered or hand-edited receipt that, say, attaches a budget to a
// "refused" tick can never be parsed back into a typed receipt.
const PLACED_TERMINAL_STATES: ReadonlySet<ArtanisLaborReceiptTerminalState> =
  new Set(['requested_pending_delivery', 'accepted_released', 'rejected_refunded'])

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const isTerminalState = (
  value: unknown,
): value is ArtanisLaborReceiptTerminalState =>
  typeof value === 'string' &&
  (TERMINAL_STATES as ReadonlyArray<string>).includes(value)

const requireParsedString = (value: unknown, label: string): string => {
  if (typeof value !== 'string' || value.trim().length === 0) {
    throw new ArtanisLaborReceiptError(`${label} must be a non-empty string.`)
  }
  return value
}

// Read side of `serializeArtanisLaborUnattendedRequestReceipt`. Before a public
// route or the tick ledger store can serve a persisted receipt it must be able
// to take untrusted wire bytes and reconstruct a validated, typed, public-safe
// receipt - or refuse. This:
//   1. parses the JSON and validates every field's type and the terminal-state
//      enum,
//   2. enforces the placed-vs-pre-request invariant (budget/workRequestId
//      presence must match the terminal state),
//   3. re-runs assertArtanisLaborPublicSafe so nothing private survives a round
//      trip, and
//   4. requires the input to already be in canonical form by re-serializing the
//      reconstructed receipt and rejecting any mismatch (extra keys, reordered
//      keys, or non-canonical spacing all fail).
// It mints no payment, identity, or settlement authority - it only validates and
// re-types an already public-safe artifact.
export const parseArtanisLaborUnattendedRequestReceipt = (
  serialized: string,
): ArtanisLaborUnattendedRequestReceipt => {
  let decoded: unknown
  try {
    decoded = parseJsonUnknown(serialized)
  } catch {
    throw new ArtanisLaborReceiptError('Receipt wire form is not valid JSON.')
  }
  if (!isRecord(decoded)) {
    throw new ArtanisLaborReceiptError('Receipt wire form must be a JSON object.')
  }

  if (decoded.schema !== 'artanis.labor.unattended_request_receipt.v1') {
    throw new ArtanisLaborReceiptError('Receipt schema is unrecognized.')
  }
  if (!isTerminalState(decoded.terminalState)) {
    throw new ArtanisLaborReceiptError('Receipt terminalState is unrecognized.')
  }

  const { budgetMsat, lifecycleRefs, workRequestId } = decoded

  if (
    budgetMsat !== null &&
    !(typeof budgetMsat === 'number' && Number.isFinite(budgetMsat))
  ) {
    throw new ArtanisLaborReceiptError('Receipt budgetMsat must be a number or null.')
  }
  if (workRequestId !== null && typeof workRequestId !== 'string') {
    throw new ArtanisLaborReceiptError('Receipt workRequestId must be a string or null.')
  }
  if (
    !Array.isArray(lifecycleRefs) ||
    lifecycleRefs.length === 0 ||
    !lifecycleRefs.every((ref) => typeof ref === 'string' && ref.trim().length > 0)
  ) {
    throw new ArtanisLaborReceiptError(
      'Receipt lifecycleRefs must be a non-empty array of non-empty strings.',
    )
  }

  const placed = PLACED_TERMINAL_STATES.has(decoded.terminalState)
  if (placed && (typeof budgetMsat !== 'number' || typeof workRequestId !== 'string')) {
    throw new ArtanisLaborReceiptError(
      'A placed-request receipt must carry a numeric budget and a work-request id.',
    )
  }
  if (!placed && (budgetMsat !== null || workRequestId !== null)) {
    throw new ArtanisLaborReceiptError(
      'A pre-request receipt must not carry a budget or a work-request id.',
    )
  }

  const receipt: ArtanisLaborUnattendedRequestReceipt = {
    artanisActorRef: requireParsedString(decoded.artanisActorRef, 'artanisActorRef'),
    budgetMsat: budgetMsat as number | null,
    issuedAtIso: requireParsedString(decoded.issuedAtIso, 'issuedAtIso'),
    lifecycleRefs: [...(lifecycleRefs as ReadonlyArray<string>)],
    schema: 'artanis.labor.unattended_request_receipt.v1',
    terminalState: decoded.terminalState,
    tickRef: requireParsedString(decoded.tickRef, 'tickRef'),
    workRequestId: workRequestId as string | null,
  }

  // Canonical-form gate: re-serializing the reconstructed receipt must reproduce
  // the input byte-for-byte, so only the canonical wire form is accepted back.
  if (serializeArtanisLaborUnattendedRequestReceipt(receipt) !== serialized) {
    throw new ArtanisLaborReceiptError('Receipt wire form is not canonical.')
  }
  return receipt
}

// Tamper check for a persisted/transported receipt: parse the wire form and
// confirm its content-addressed ref matches the one it was stored or served
// under. Returns the validated receipt on success; throws on any mismatch so a
// route or store can never hand back a receipt addressed by the wrong name. It
// asserts no authority - it only confirms a name still addresses its bytes.
export const verifyArtanisLaborUnattendedRequestReceipt = (
  serialized: string,
  expectedRef: string,
): ArtanisLaborUnattendedRequestReceipt => {
  const receipt = parseArtanisLaborUnattendedRequestReceipt(serialized)
  const actualRef = deriveArtanisLaborUnattendedRequestReceiptRef(receipt)
  if (actualRef !== expectedRef) {
    throw new ArtanisLaborReceiptError(
      `Receipt ref mismatch: expected ${expectedRef}, derived ${actualRef}.`,
    )
  }
  return receipt
}
