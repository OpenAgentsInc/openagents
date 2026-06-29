/**
 * NIP-LBR labor-market closeout receipt.
 *
 * The four LBR lifecycle helpers in `./lbr` produce the individual relay events
 * for an agentic-coding labor job: request (`5934`), quote (`7000`), acceptance
 * (`7000`), and result (`6934`). What was missing was a single composed object
 * that binds one complete lifecycle into a content-addressed, public-safe
 * receipt the labor market can dereference and re-verify — the same shape the
 * NIP-DS data market already has via `verifyDatasetDeliveryDescriptorDigest`.
 *
 * This module adds exactly that, and nothing more. It is protocol-only and
 * ref-only:
 *
 * - It does NOT create invoices, move sats, open wallets, escrow funds, or
 *   grant settlement authority. The relay is transport; settlement authority
 *   stays in the platform receipt systems, per `docs/nips/LBR.md`.
 * - The receipt carries only the public-safe refs that already passed the
 *   ref/payment-material guards in `./lbr`, plus a SHA-256 digest over a
 *   canonical projection of those refs.
 * - `verifyLbrLaborCloseoutDigest` lets any reader independently re-derive the
 *   digest and confirm the receipt dereferences the exact lifecycle that
 *   produced it.
 */

import { sha256Hex } from "nostr-effect/nip90"

import {
  LBR_AGENTIC_CODING_REQUEST_KIND,
  LBR_AGENTIC_CODING_RESULT_KIND,
  LBR_FEEDBACK_KIND,
  LbrProtocolError,
  decodeLbrAcceptanceEvent,
  decodeLbrAgenticCodingRequestEvent,
  decodeLbrQuoteEvent,
  decodeLbrResultEvent,
  type LbrAcceptance,
  type LbrAgenticCodingRequest,
  type LbrQuote,
  type LbrResult,
} from "./lbr.js"

export const LBR_CLOSEOUT_RECEIPT_VERSION = "nip-lbr-closeout.v1"

/**
 * The signed relay-event shape the closeout receipt binds to. This is the same
 * shape the NIP-DS smoke flow signs and reads back from the relay (a Nostr
 * event with `id`, author `pubkey`, `kind`, `created_at`, `tags`, `content`,
 * and `sig`), so callers can pass either a freshly signed event or one read
 * back from a relay without coupling this package to a signer. The signature is
 * not re-verified here — the relay does that on accept; the closeout binds the
 * accepted event ids and re-decodes the ref-only payloads through `./lbr`.
 */
export type LbrLifecycleEvent = Readonly<{
  id: string
  pubkey: string
  kind: number
  created_at: number
  tags: ReadonlyArray<readonly string[]>
  content: string
  sig: string
}>

export type LbrLaborCloseoutInput = Readonly<{
  requestEvent: LbrLifecycleEvent
  quoteEvent: LbrLifecycleEvent
  acceptanceEvent: LbrLifecycleEvent
  resultEvent: LbrLifecycleEvent
}>

/**
 * The public-safe, content-addressed labor-market closeout receipt. Every field
 * is a ref or hex id that already cleared the `./lbr` ref/payment guards; the
 * `digest` is a SHA-256 over the canonical projection below, so the receipt is
 * dereferenceable: a reader re-derives the digest from the refs and confirms it
 * matches.
 */
export type LbrLaborCloseout = Readonly<{
  version: typeof LBR_CLOSEOUT_RECEIPT_VERSION
  /** `lbr-closeout:<requestId>:<digest>` — a stable, ref-only receipt ref. */
  receiptRef: string
  requestId: string
  requesterPubkey: string
  providerPubkey: string
  bidMsats: number
  quotedAmountMsats: number
  objectiveRef: string
  repositoryRefs: ReadonlyArray<string>
  verificationCommandRef: string
  requiredCapabilityRefs: ReadonlyArray<string>
  providerRef: string
  quoteRef: string
  acceptanceRef: string
  escrowReceiptRef: string
  artifactRefs: ReadonlyArray<string>
  platformCloseoutRef: string
  summaryRef: string
  testRef: string
  buildRef?: string
  forumTopicRef?: string
  deadline?: string
  eventIds: Readonly<{
    request: string
    quote: string
    acceptance: string
    result: string
  }>
  /** SHA-256 hex over `canonicalLbrLaborCloseout(...)`. */
  digest: string
}>

const eventIdPattern = /^[a-f0-9]{64}$/i

const fail = (code: string, message: string): never => {
  throw new LbrProtocolError(code, message)
}

const ensureEventId = (value: unknown, field: string): string => {
  if (typeof value !== "string" || !eventIdPattern.test(value)) {
    return fail("invalid_event_id", `${field} must be a 32-byte hex event id`)
  }
  return value.toLowerCase()
}

const ensureKind = (value: number, expected: number, field: string): void => {
  if (value !== expected) {
    fail("invalid_kind", `${field} must use kind ${expected}`)
  }
}

/**
 * Canonical, deterministic projection of the closeout's public-safe content.
 * The digest is taken over this string, so it is independent of object key
 * order and of any non-content fields (the `digest`/`receiptRef` themselves are
 * excluded to avoid a self-referential hash).
 */
export const canonicalLbrLaborCloseout = (
  fields: Omit<LbrLaborCloseout, "digest" | "receiptRef">,
): string =>
  JSON.stringify({
    version: fields.version,
    requestId: fields.requestId,
    requesterPubkey: fields.requesterPubkey,
    providerPubkey: fields.providerPubkey,
    bidMsats: fields.bidMsats,
    quotedAmountMsats: fields.quotedAmountMsats,
    objectiveRef: fields.objectiveRef,
    repositoryRefs: [...fields.repositoryRefs],
    verificationCommandRef: fields.verificationCommandRef,
    requiredCapabilityRefs: [...fields.requiredCapabilityRefs],
    providerRef: fields.providerRef,
    quoteRef: fields.quoteRef,
    acceptanceRef: fields.acceptanceRef,
    escrowReceiptRef: fields.escrowReceiptRef,
    artifactRefs: [...fields.artifactRefs],
    platformCloseoutRef: fields.platformCloseoutRef,
    summaryRef: fields.summaryRef,
    testRef: fields.testRef,
    buildRef: fields.buildRef ?? null,
    forumTopicRef: fields.forumTopicRef ?? null,
    deadline: fields.deadline ?? null,
    eventIds: {
      request: fields.eventIds.request,
      quote: fields.eventIds.quote,
      acceptance: fields.eventIds.acceptance,
      result: fields.eventIds.result,
    },
  })

const buildCloseoutFields = (
  events: {
    requestEvent: LbrLifecycleEvent
    quoteEvent: LbrLifecycleEvent
    acceptanceEvent: LbrLifecycleEvent
    resultEvent: LbrLifecycleEvent
  },
  decoded: {
    request: LbrAgenticCodingRequest
    quote: LbrQuote
    acceptance: LbrAcceptance
    result: LbrResult
  },
): Omit<LbrLaborCloseout, "digest" | "receiptRef"> => {
  const requestId = decoded.quote.requestId

  // The four events must describe ONE job: every lifecycle event's requestId
  // must equal the request event id, and the parties must be consistent.
  const requestEventId = ensureEventId(events.requestEvent.id, "request event id")
  if (decoded.quote.requestId !== requestEventId) {
    fail("mismatched_request", "quote requestId must equal the request event id")
  }
  if (decoded.acceptance.requestId !== requestEventId) {
    fail(
      "mismatched_request",
      "acceptance requestId must equal the request event id",
    )
  }
  if (decoded.result.labor.result.requestId !== requestEventId) {
    fail("mismatched_request", "result requestId must equal the request event id")
  }

  const requesterPubkey = ensureEventId(
    events.requestEvent.pubkey,
    "requester pubkey",
  )
  if (decoded.quote.requesterPubkey !== requesterPubkey) {
    fail("mismatched_party", "quote requester must equal the request author")
  }
  if (decoded.result.labor.result.customerPubkey !== requesterPubkey) {
    fail("mismatched_party", "result customer must equal the request author")
  }

  const providerPubkey = ensureEventId(
    events.quoteEvent.pubkey,
    "provider pubkey",
  )
  if (decoded.acceptance.providerPubkey !== providerPubkey) {
    fail(
      "mismatched_party",
      "acceptance provider must equal the quote author",
    )
  }
  if (ensureEventId(events.resultEvent.pubkey, "result author") !== providerPubkey) {
    fail("mismatched_party", "result author must equal the quote author")
  }

  if (decoded.quote.amountMsats > decoded.request.bidMsats) {
    fail(
      "quote_over_budget",
      "quoted amount must not exceed the request max budget bid",
    )
  }

  return {
    version: LBR_CLOSEOUT_RECEIPT_VERSION,
    requestId,
    requesterPubkey,
    providerPubkey,
    bidMsats: decoded.request.bidMsats,
    quotedAmountMsats: decoded.quote.amountMsats,
    objectiveRef: decoded.request.objectiveRef,
    repositoryRefs: decoded.request.repositoryRefs,
    verificationCommandRef: decoded.request.verificationCommandRef,
    requiredCapabilityRefs: decoded.request.requiredCapabilityRefs,
    providerRef: decoded.quote.providerRef,
    quoteRef: decoded.quote.quoteRef,
    acceptanceRef: decoded.acceptance.acceptanceRef,
    escrowReceiptRef: decoded.acceptance.escrowReceiptRef,
    artifactRefs: decoded.result.artifactRefs,
    platformCloseoutRef: decoded.result.platformCloseoutRef,
    summaryRef: decoded.result.summaryRef,
    testRef: decoded.result.testRef,
    ...(decoded.result.buildRef === undefined
      ? {}
      : { buildRef: decoded.result.buildRef }),
    ...(decoded.request.forumTopicRef === undefined
      ? {}
      : { forumTopicRef: decoded.request.forumTopicRef }),
    ...(decoded.request.deadline === undefined
      ? {}
      : { deadline: decoded.request.deadline }),
    eventIds: {
      request: requestEventId,
      quote: ensureEventId(events.quoteEvent.id, "quote event id"),
      acceptance: ensureEventId(events.acceptanceEvent.id, "acceptance event id"),
      result: ensureEventId(events.resultEvent.id, "result event id"),
    },
  }
}

/**
 * Compose one complete LBR labor lifecycle into a content-addressed,
 * public-safe closeout receipt. Each input event is re-decoded through the
 * existing `./lbr` guards (so raw prompts, private paths, credentials, and
 * payment material are rejected here too), the four events are checked for
 * consistency (one job, consistent parties, quote within budget), and the
 * canonical projection is hashed.
 */
export const makeLbrLaborCloseout = (
  input: LbrLaborCloseoutInput,
): LbrLaborCloseout => {
  ensureKind(
    input.requestEvent.kind,
    LBR_AGENTIC_CODING_REQUEST_KIND,
    "request event",
  )
  ensureKind(input.quoteEvent.kind, LBR_FEEDBACK_KIND, "quote event")
  ensureKind(input.acceptanceEvent.kind, LBR_FEEDBACK_KIND, "acceptance event")
  ensureKind(input.resultEvent.kind, LBR_AGENTIC_CODING_RESULT_KIND, "result event")

  const decoded = {
    request: decodeLbrAgenticCodingRequestEvent(input.requestEvent),
    quote: decodeLbrQuoteEvent(input.quoteEvent),
    acceptance: decodeLbrAcceptanceEvent(input.acceptanceEvent),
    result: decodeLbrResultEvent(input.resultEvent),
  }

  const fields = buildCloseoutFields(
    {
      requestEvent: input.requestEvent,
      quoteEvent: input.quoteEvent,
      acceptanceEvent: input.acceptanceEvent,
      resultEvent: input.resultEvent,
    },
    decoded,
  )

  const digest = sha256Hex(canonicalLbrLaborCloseout(fields))

  return {
    ...fields,
    receiptRef: `lbr-closeout:${fields.requestId}:${digest}`,
    digest,
  }
}

/**
 * Re-derive the closeout digest from the receipt's own public-safe fields and
 * confirm it matches the recorded `digest`. This is the dereference check: a
 * reader who holds only the receipt can independently prove it binds the exact
 * lifecycle that produced it, and that the `receiptRef` is consistent with the
 * content. Returns `true` only when both the digest and the derived
 * `receiptRef` match.
 */
export const verifyLbrLaborCloseoutDigest = (
  closeout: LbrLaborCloseout,
): boolean => {
  const { digest: _digest, receiptRef: _receiptRef, ...fields } = closeout
  const derivedDigest = sha256Hex(canonicalLbrLaborCloseout(fields))
  const derivedReceiptRef = `lbr-closeout:${closeout.requestId}:${derivedDigest}`
  return (
    derivedDigest === closeout.digest &&
    derivedReceiptRef === closeout.receiptRef
  )
}
