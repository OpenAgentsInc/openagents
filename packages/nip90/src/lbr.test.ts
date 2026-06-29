import { describe, expect, test } from "bun:test"

import {
  KIND_JOB_LABOR_CODE_TASK,
  LBR_AGENTIC_CODING_REQUEST_KIND,
  LBR_AGENTIC_CODING_RESULT_KIND,
  LBR_FEEDBACK_KIND,
  LBR_OUTPUT_DELIVERY_POLICY,
  LBR_RESERVED_LABOR_KIND_MAX,
  LBR_RESERVED_LABOR_KIND_MIN,
  LbrProtocolError,
  decodeLbrAcceptanceEvent,
  decodeLbrAgenticCodingRequestEvent,
  decodeLbrQuoteEvent,
  decodeLbrResultEvent,
  lbrAcceptanceToDraft,
  lbrAgenticCodingRequestToDraft,
  lbrQuoteToDraft,
  lbrResultToDraft,
  makeLbrAcceptance,
  makeLbrAgenticCodingRequest,
  makeLbrQuote,
  makeLbrResult,
} from "./index.js"

const requesterPubkey = "11".repeat(32)
const providerPubkey = "22".repeat(32)
const requestId = "aa".repeat(32)
const eventId = "bb".repeat(32)
const sig = "33".repeat(64)

const eventFromDraft = (
  draft: Readonly<{
    kind: number
    tags: ReadonlyArray<readonly string[]>
    content: string
  }>,
  overrides: Partial<{
    id: string
    pubkey: string
    content: string
  }> = {},
) => ({
  id: overrides.id ?? eventId,
  pubkey: overrides.pubkey ?? requesterPubkey,
  created_at: 1_781_107_200,
  kind: draft.kind,
  tags: draft.tags,
  content: overrides.content ?? draft.content,
  sig,
})

describe("NIP-LBR typed labor contract helpers", () => {
  test("pins LBR to the current nostr-effect labor kind allocation", () => {
    expect(LBR_RESERVED_LABOR_KIND_MIN).toBe(5930)
    expect(LBR_RESERVED_LABOR_KIND_MAX).toBe(5939)
    expect(LBR_AGENTIC_CODING_REQUEST_KIND).toBe(KIND_JOB_LABOR_CODE_TASK)
    expect(LBR_AGENTIC_CODING_REQUEST_KIND).toBe(5934)
    expect(LBR_AGENTIC_CODING_RESULT_KIND).toBe(6934)
    expect(LBR_FEEDBACK_KIND).toBe(7000)
  })

  test("round-trips an output-only agentic coding request", () => {
    const request = makeLbrAgenticCodingRequest({
      objectiveRef: "objective.public.lbr.fix_test",
      repositoryRefs: ["repo.public.openagents"],
      verificationCommandRef: "command.public.bun_test",
      requiredCapabilityRefs: ["capability.pylon.local_claude_agent"],
      bidMsats: 2_000_000,
      deadline: "deadline.public.lbr.20260610",
      forumTopicRef: "topic.public.forum.labor_1",
      relays: ["wss://relay.openagents.com"],
    })

    const draft = lbrAgenticCodingRequestToDraft(request)
    const parsed = decodeLbrAgenticCodingRequestEvent(eventFromDraft(draft))

    expect(draft.kind).toBe(5934)
    expect(draft.content).toBe("")
    expect(parsed.objectiveRef).toBe("objective.public.lbr.fix_test")
    expect(parsed.repositoryRefs).toEqual(["repo.public.openagents"])
    expect(parsed.verificationCommandRef).toBe("command.public.bun_test")
    expect(parsed.requiredCapabilityRefs).toEqual([
      "capability.pylon.local_claude_agent",
    ])
    expect(parsed.bidMsats).toBe(2_000_000)
    expect(parsed.deadline).toBe("deadline.public.lbr.20260610")
    expect(parsed.forumTopicRef).toBe("topic.public.forum.labor_1")
    expect(parsed.outputDelivery).toBe(LBR_OUTPUT_DELIVERY_POLICY)
  })

  test("round-trips quote, acceptance, and result events", () => {
    const quote = makeLbrQuote({
      requestId,
      requesterPubkey,
      amountMsats: 1_500_000,
      providerRef: "provider.public.pylon.local_claude",
      capabilityRefs: ["capability.pylon.local_claude_agent"],
      quoteRef: "quote.public.lbr.1",
      expiresAt: "expiry.public.lbr.20260610",
      requestRelay: "wss://relay.openagents.com",
    })
    const parsedQuote = decodeLbrQuoteEvent(
      eventFromDraft(lbrQuoteToDraft(quote), { pubkey: providerPubkey }),
    )

    expect(parsedQuote.requestId).toBe(requestId)
    expect(parsedQuote.requesterPubkey).toBe(requesterPubkey)
    expect(parsedQuote.amountMsats).toBe(1_500_000)
    expect(parsedQuote.providerRef).toBe("provider.public.pylon.local_claude")
    expect(parsedQuote.quoteRef).toBe("quote.public.lbr.1")

    const acceptance = makeLbrAcceptance({
      requestId,
      providerPubkey,
      escrowReceiptRef: "receipt.public.escrow.1",
      acceptanceRef: "acceptance.public.lbr.1",
      requestRelay: "wss://relay.openagents.com",
    })
    const parsedAcceptance = decodeLbrAcceptanceEvent(
      eventFromDraft(lbrAcceptanceToDraft(acceptance)),
    )

    expect(parsedAcceptance.requestId).toBe(requestId)
    expect(parsedAcceptance.providerPubkey).toBe(providerPubkey)
    expect(parsedAcceptance.escrowReceiptRef).toBe("receipt.public.escrow.1")
    expect(parsedAcceptance.acceptanceRef).toBe("acceptance.public.lbr.1")

    const result = makeLbrResult({
      requestId,
      requesterPubkey,
      artifactRefs: ["artifact.public.lbr.patch_1"],
      platformCloseoutRef: "closeout.public.lbr.1",
      summaryRef: "summary.public.lbr.1",
      testRef: "test.public.lbr.bun",
      buildRef: "build.public.lbr.1",
      requestRelay: "wss://relay.openagents.com",
    })
    const parsedResult = decodeLbrResultEvent(
      eventFromDraft(lbrResultToDraft(result), { pubkey: providerPubkey }),
    )

    expect(parsedResult.kind).toBe(6934)
    expect(parsedResult.artifactRefs).toEqual(["artifact.public.lbr.patch_1"])
    expect(parsedResult.platformCloseoutRef).toBe("closeout.public.lbr.1")
    expect(parsedResult.summaryRef).toBe("summary.public.lbr.1")
    expect(parsedResult.testRef).toBe("test.public.lbr.bun")
    expect(parsedResult.buildRef).toBe("build.public.lbr.1")
  })

  test("rejects raw prompts, private paths, and payment material at decode time", () => {
    expect(() =>
      makeLbrAgenticCodingRequest({
        objectiveRef: "objective.public./Users/alice/private",
        repositoryRefs: ["repo.public.openagents"],
        verificationCommandRef: "command.public.bun_test",
        requiredCapabilityRefs: ["capability.pylon.local_claude_agent"],
        bidMsats: 1_000,
      }),
    ).toThrow(LbrProtocolError)

    const request = makeLbrAgenticCodingRequest({
      objectiveRef: "objective.public.lbr.fix_test",
      repositoryRefs: ["repo.public.openagents"],
      verificationCommandRef: "command.public.bun_test",
      requiredCapabilityRefs: ["capability.pylon.local_claude_agent"],
      bidMsats: 1_000,
    })
    const requestDraft = lbrAgenticCodingRequestToDraft(request)
    expect(() =>
      decodeLbrAgenticCodingRequestEvent(
        eventFromDraft(requestDraft, { content: "raw prompt: fix this" }),
      ),
    ).toThrow(LbrProtocolError)
    expect(() =>
      decodeLbrAgenticCodingRequestEvent(
        eventFromDraft({
          ...requestDraft,
          tags: requestDraft.tags.map((tag) =>
            tag[0] === "param" && tag[1] === "lbr_repository_ref"
              ? ["param", "lbr_repository_ref", "repo.public./Users/alice/private"]
              : tag,
          ),
        }),
      ),
    ).toThrow(LbrProtocolError)

    const quote = makeLbrQuote({
      requestId,
      requesterPubkey,
      amountMsats: 1_500_000,
      providerRef: "provider.public.pylon.local_claude",
      capabilityRefs: ["capability.pylon.local_claude_agent"],
      quoteRef: "quote.public.lbr.1",
    })
    const quoteDraft = lbrQuoteToDraft(quote)
    expect(() =>
      decodeLbrQuoteEvent(
        eventFromDraft({
          ...quoteDraft,
          tags: quoteDraft.tags.map((tag) =>
            tag[0] === "amount" ? ["amount", "1500000", "lnbc1unsafe"] : tag,
          ),
        }),
      ),
    ).toThrow(LbrProtocolError)

    const result = makeLbrResult({
      requestId,
      requesterPubkey,
      artifactRefs: ["artifact.public.lbr.patch_1"],
      platformCloseoutRef: "closeout.public.lbr.1",
      summaryRef: "summary.public.lbr.1",
      testRef: "test.public.lbr.bun",
    })
    const resultDraft = lbrResultToDraft(result)
    expect(() =>
      decodeLbrResultEvent(
        eventFromDraft(resultDraft, { content: "payment_hash=abc123" }),
      ),
    ).toThrow(LbrProtocolError)
  })
})
