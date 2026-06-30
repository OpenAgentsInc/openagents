import { describe, expect, test } from "bun:test"

import {
  LBR_CLOSEOUT_RECEIPT_VERSION,
  LbrProtocolError,
  canonicalLbrLaborCloseout,
  lbrAcceptanceToDraft,
  lbrAgenticCodingRequestToDraft,
  lbrBondForfeitToDraft,
  lbrBondReleaseToDraft,
  lbrQuoteToDraft,
  lbrResultToDraft,
  makeLbrBondForfeit,
  makeLbrBondRelease,
  makeLbrAcceptance,
  makeLbrAgenticCodingRequest,
  makeLbrLaborCloseout,
  makeLbrQuote,
  makeLbrResult,
  verifyLbrLaborCloseoutDigest,
  type LbrLifecycleEvent,
} from "./index.js"

const requesterPubkey = "11".repeat(32)
const providerPubkey = "22".repeat(32)
const requestEventId = "aa".repeat(32)
const quoteEventId = "bb".repeat(32)
const acceptanceEventId = "cc".repeat(32)
const resultEventId = "dd".repeat(32)
const bondOutcomeEventId = "ee".repeat(32)
const sig = "33".repeat(64)

type Draft = Readonly<{
  kind: number
  tags: ReadonlyArray<readonly string[]>
  content: string
}>

const eventFrom = (
  draft: Draft,
  overrides: { id: string; pubkey: string },
): LbrLifecycleEvent => ({
  id: overrides.id,
  pubkey: overrides.pubkey,
  kind: draft.kind,
  created_at: 1_781_107_200,
  tags: draft.tags,
  content: draft.content,
  sig,
})

/**
 * Build one complete, internally consistent LBR labor lifecycle (request,
 * quote, acceptance, result) as relay-shaped events. The `requestId` that the
 * quote/acceptance/result reference is the request event id, exactly as a live
 * relay flow would bind them.
 */
const buildLifecycle = () => {
  const request = makeLbrAgenticCodingRequest({
    objectiveRef: "objective.public.lbr.fix_flaky_test",
    repositoryRefs: ["repo.public.openagents"],
    verificationCommandRef: "command.public.bun_test",
    requiredCapabilityRefs: ["capability.pylon.local_claude_agent"],
    bidMsats: 2_000_000,
    deadline: "deadline.public.lbr.20260630",
    forumTopicRef: "topic.public.forum.labor_de6",
    relays: ["wss://relay.openagents.com"],
  })
  const requestEvent = eventFrom(lbrAgenticCodingRequestToDraft(request), {
    id: requestEventId,
    pubkey: requesterPubkey,
  })

  const quote = makeLbrQuote({
    requestId: requestEventId,
    requesterPubkey,
    amountMsats: 1_500_000,
    providerRef: "provider.public.pylon.local_claude",
    capabilityRefs: ["capability.pylon.local_claude_agent"],
    quoteRef: "quote.public.lbr.de6_1",
    expiresAt: "expiry.public.lbr.20260630",
    requestRelay: "wss://relay.openagents.com",
  })
  const quoteEvent = eventFrom(lbrQuoteToDraft(quote), {
    id: quoteEventId,
    pubkey: providerPubkey,
  })

  const acceptance = makeLbrAcceptance({
    requestId: requestEventId,
    providerPubkey,
    escrowReceiptRef: "receipt.public.escrow.de6_1",
    acceptanceRef: "acceptance.public.lbr.de6_1",
    requestRelay: "wss://relay.openagents.com",
  })
  const acceptanceEvent = eventFrom(lbrAcceptanceToDraft(acceptance), {
    id: acceptanceEventId,
    pubkey: requesterPubkey,
  })

  const result = makeLbrResult({
    requestId: requestEventId,
    requesterPubkey,
    artifactRefs: ["artifact.public.lbr.patch_de6_1"],
    platformCloseoutRef: "closeout.public.lbr.de6_1",
    summaryRef: "summary.public.lbr.de6_1",
    testRef: "test.public.lbr.bun",
    buildRef: "build.public.lbr.de6_1",
    requestRelay: "wss://relay.openagents.com",
  })
  const resultEvent = eventFrom(lbrResultToDraft(result), {
    id: resultEventId,
    pubkey: providerPubkey,
  })

  return { requestEvent, quoteEvent, acceptanceEvent, resultEvent }
}

const buildBondReleaseEvent = (): LbrLifecycleEvent => {
  const release = makeLbrBondRelease({
    requestId: requestEventId,
    requesterPubkey,
    bondReceiptRef: "receipt.public.bond.de6_1",
    releaseReceiptRef: "receipt.public.bond_release.de6_1",
    authorityRef: "authority.public.validator.de6",
    requestRelay: "wss://relay.openagents.com",
  })
  return eventFrom(lbrBondReleaseToDraft(release), {
    id: bondOutcomeEventId,
    pubkey: "44".repeat(32),
  })
}

const buildBondForfeitEvent = (): LbrLifecycleEvent => {
  const forfeit = makeLbrBondForfeit({
    requestId: requestEventId,
    requesterPubkey,
    bondReceiptRef: "receipt.public.bond.de6_1",
    forfeitReceiptRef: "receipt.public.bond_forfeit.de6_1",
    forfeitDestination: "counterparty",
    forfeitConditionRef: "condition.public.validator.nonperformance",
    authorityRef: "authority.public.validator.de6",
    requestRelay: "wss://relay.openagents.com",
  })
  return eventFrom(lbrBondForfeitToDraft(forfeit), {
    id: bondOutcomeEventId,
    pubkey: "44".repeat(32),
  })
}

describe("NIP-LBR labor-market closeout receipt", () => {
  test("composes a content-addressed, dereferenceable closeout receipt", () => {
    const lifecycle = buildLifecycle()
    const closeout = makeLbrLaborCloseout(lifecycle)

    expect(closeout.version).toBe(LBR_CLOSEOUT_RECEIPT_VERSION)
    expect(closeout.requestId).toBe(requestEventId)
    expect(closeout.requesterPubkey).toBe(requesterPubkey)
    expect(closeout.providerPubkey).toBe(providerPubkey)
    expect(closeout.bidMsats).toBe(2_000_000)
    expect(closeout.quotedAmountMsats).toBe(1_500_000)
    expect(closeout.objectiveRef).toBe("objective.public.lbr.fix_flaky_test")
    expect(closeout.repositoryRefs).toEqual(["repo.public.openagents"])
    expect(closeout.verificationCommandRef).toBe("command.public.bun_test")
    expect(closeout.providerRef).toBe("provider.public.pylon.local_claude")
    expect(closeout.quoteRef).toBe("quote.public.lbr.de6_1")
    expect(closeout.acceptanceRef).toBe("acceptance.public.lbr.de6_1")
    expect(closeout.escrowReceiptRef).toBe("receipt.public.escrow.de6_1")
    expect(closeout.artifactRefs).toEqual(["artifact.public.lbr.patch_de6_1"])
    expect(closeout.platformCloseoutRef).toBe("closeout.public.lbr.de6_1")
    expect(closeout.summaryRef).toBe("summary.public.lbr.de6_1")
    expect(closeout.testRef).toBe("test.public.lbr.bun")
    expect(closeout.buildRef).toBe("build.public.lbr.de6_1")
    expect(closeout.forumTopicRef).toBe("topic.public.forum.labor_de6")
    expect(closeout.deadline).toBe("deadline.public.lbr.20260630")
    expect(closeout.eventIds).toEqual({
      request: requestEventId,
      quote: quoteEventId,
      acceptance: acceptanceEventId,
      result: resultEventId,
    })

    // The receipt is a 64-hex content address bound to the request id.
    expect(closeout.digest).toMatch(/^[a-f0-9]{64}$/)
    expect(closeout.receiptRef).toBe(
      `lbr-closeout:${requestEventId}:${closeout.digest}`,
    )

    // The dereference check: a reader holding only the receipt re-derives the
    // digest from its public-safe fields and confirms it binds this lifecycle.
    expect(verifyLbrLaborCloseoutDigest(closeout)).toBe(true)
  })

  test("is deterministic: the same lifecycle yields the same receipt", () => {
    const a = makeLbrLaborCloseout(buildLifecycle())
    const b = makeLbrLaborCloseout(buildLifecycle())
    expect(b.digest).toBe(a.digest)
    expect(b.receiptRef).toBe(a.receiptRef)
  })

  test("digest does not depend on canonical field order", () => {
    const closeout = makeLbrLaborCloseout(buildLifecycle())
    const { digest: _digest, receiptRef: _receiptRef, ...fields } = closeout
    // Rebuild the canonical projection from a reordered object; the JSON the
    // digest is taken over must be identical.
    const reordered = {
      eventIds: fields.eventIds,
      deadline: fields.deadline,
      digest: "ignored",
      testRef: fields.testRef,
      version: fields.version,
      requestId: fields.requestId,
      requesterPubkey: fields.requesterPubkey,
      providerPubkey: fields.providerPubkey,
      bidMsats: fields.bidMsats,
      quotedAmountMsats: fields.quotedAmountMsats,
      objectiveRef: fields.objectiveRef,
      repositoryRefs: fields.repositoryRefs,
      verificationCommandRef: fields.verificationCommandRef,
      requiredCapabilityRefs: fields.requiredCapabilityRefs,
      providerRef: fields.providerRef,
      quoteRef: fields.quoteRef,
      acceptanceRef: fields.acceptanceRef,
      escrowReceiptRef: fields.escrowReceiptRef,
      artifactRefs: fields.artifactRefs,
      platformCloseoutRef: fields.platformCloseoutRef,
      summaryRef: fields.summaryRef,
      buildRef: fields.buildRef,
      forumTopicRef: fields.forumTopicRef,
    } as typeof fields
    expect(canonicalLbrLaborCloseout(reordered)).toBe(
      canonicalLbrLaborCloseout(fields),
    )
  })

  test("tampering with any public-safe ref breaks the dereference", () => {
    const closeout = makeLbrLaborCloseout(buildLifecycle())
    const tampered = { ...closeout, summaryRef: "summary.public.lbr.swapped" }
    expect(verifyLbrLaborCloseoutDigest(tampered)).toBe(false)

    const tamperedAmount = { ...closeout, quotedAmountMsats: 1 }
    expect(verifyLbrLaborCloseoutDigest(tamperedAmount)).toBe(false)

    const tamperedReceiptRef = {
      ...closeout,
      receiptRef: `lbr-closeout:${requestEventId}:${"00".repeat(32)}`,
    }
    expect(verifyLbrLaborCloseoutDigest(tamperedReceiptRef)).toBe(false)
  })

  test("binds a provider bond release outcome into the closeout digest", () => {
    const withoutBond = makeLbrLaborCloseout(buildLifecycle())
    const withRelease = makeLbrLaborCloseout({
      ...buildLifecycle(),
      bondOutcomeEvent: buildBondReleaseEvent(),
    })

    expect(withRelease.bondOutcome).toEqual({
      kind: "released",
      eventId: bondOutcomeEventId,
      requestId: requestEventId,
      requesterPubkey,
      bondReceiptRef: "receipt.public.bond.de6_1",
      releaseReceiptRef: "receipt.public.bond_release.de6_1",
      authorityRef: "authority.public.validator.de6",
    })
    expect(withRelease.digest).not.toBe(withoutBond.digest)
    expect(verifyLbrLaborCloseoutDigest(withRelease)).toBe(true)

    if (withRelease.bondOutcome?.kind !== "released") {
      throw new Error("expected a released bond outcome")
    }
    const tampered = {
      ...withRelease,
      bondOutcome: {
        ...withRelease.bondOutcome,
        releaseReceiptRef: "receipt.public.bond_release.swapped",
      },
    }
    expect(verifyLbrLaborCloseoutDigest(tampered)).toBe(false)
  })

  test("binds release versus forfeit as mutually distinct terminal outcomes", () => {
    const withRelease = makeLbrLaborCloseout({
      ...buildLifecycle(),
      bondOutcomeEvent: buildBondReleaseEvent(),
    })
    const withForfeit = makeLbrLaborCloseout({
      ...buildLifecycle(),
      bondOutcomeEvent: buildBondForfeitEvent(),
    })

    expect(withForfeit.bondOutcome).toEqual({
      kind: "forfeited",
      eventId: bondOutcomeEventId,
      requestId: requestEventId,
      requesterPubkey,
      bondReceiptRef: "receipt.public.bond.de6_1",
      forfeitReceiptRef: "receipt.public.bond_forfeit.de6_1",
      forfeitDestination: "counterparty",
      forfeitConditionRef: "condition.public.validator.nonperformance",
      authorityRef: "authority.public.validator.de6",
    })
    expect(withForfeit.digest).not.toBe(withRelease.digest)
    expect(verifyLbrLaborCloseoutDigest(withForfeit)).toBe(true)
  })

  test("rejects a quote that exceeds the request max budget", () => {
    const lifecycle = buildLifecycle()
    const overBudgetQuote = makeLbrQuote({
      requestId: requestEventId,
      requesterPubkey,
      amountMsats: 3_000_000, // request bid is 2_000_000
      providerRef: "provider.public.pylon.local_claude",
      capabilityRefs: ["capability.pylon.local_claude_agent"],
      quoteRef: "quote.public.lbr.de6_over",
    })
    const overBudgetEvent = eventFrom(lbrQuoteToDraft(overBudgetQuote), {
      id: quoteEventId,
      pubkey: providerPubkey,
    })
    expect(() =>
      makeLbrLaborCloseout({ ...lifecycle, quoteEvent: overBudgetEvent }),
    ).toThrow(LbrProtocolError)
  })

  test("rejects a lifecycle whose events do not describe one job", () => {
    const lifecycle = buildLifecycle()
    // Quote for a different request id.
    const foreignQuote = makeLbrQuote({
      requestId: "ee".repeat(32),
      requesterPubkey,
      amountMsats: 1_000_000,
      providerRef: "provider.public.pylon.local_claude",
      capabilityRefs: ["capability.pylon.local_claude_agent"],
      quoteRef: "quote.public.lbr.de6_foreign",
    })
    const foreignQuoteEvent = eventFrom(lbrQuoteToDraft(foreignQuote), {
      id: quoteEventId,
      pubkey: providerPubkey,
    })
    expect(() =>
      makeLbrLaborCloseout({ ...lifecycle, quoteEvent: foreignQuoteEvent }),
    ).toThrow(LbrProtocolError)
  })

  test("rejects mismatched parties (result author is not the quoting provider)", () => {
    const lifecycle = buildLifecycle()
    const impostorResult = {
      ...lifecycle.resultEvent,
      pubkey: "99".repeat(32),
    }
    expect(() =>
      makeLbrLaborCloseout({ ...lifecycle, resultEvent: impostorResult }),
    ).toThrow(LbrProtocolError)
  })

  test("rejects wrong-kind events", () => {
    const lifecycle = buildLifecycle()
    const wrongKindResult = { ...lifecycle.resultEvent, kind: 6930 }
    expect(() =>
      makeLbrLaborCloseout({ ...lifecycle, resultEvent: wrongKindResult }),
    ).toThrow(LbrProtocolError)
  })

  test("inherits the ref-only safety guard (payment material is rejected)", () => {
    const lifecycle = buildLifecycle()
    const unsafeResult = {
      ...lifecycle.resultEvent,
      content: "payment_preimage=deadbeef",
    }
    expect(() =>
      makeLbrLaborCloseout({ ...lifecycle, resultEvent: unsafeResult }),
    ).toThrow(LbrProtocolError)
  })
})
