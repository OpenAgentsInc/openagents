import { createHash } from 'node:crypto'

import {
  assertArtanisLaborPublicSafe,
  type ArtanisLaborAcceptanceOutcome,
  type ArtanisLaborRequesterOutcome,
} from './artanis-labor-requester'

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
