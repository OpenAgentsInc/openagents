import { describe, expect, it } from "vite-plus/test";

import {
  LBR_AGENTIC_CODING_REQUEST_KIND,
  LBR_FEEDBACK_KIND,
  LBR_OUTPUT_DELIVERY_POLICY,
  LBR_RESERVED_LABOR_KIND_MAX,
  LBR_RESERVED_LABOR_KIND_MIN,
  SARAH_LBR_JOB_TYPE,
  SARAH_LBR_REQUEST_QUOTE_PACKET,
  SARAH_LBR_REQUEST_QUOTE_SCHEMA,
  SARAH_LBR_SETTLEMENT_MODE_V1,
  SarahLbrLaneError,
  assertSarahLbrRequestNotExpired,
  buildSarahLbrQuote,
  buildSarahLbrWorkRequest,
  decodeSarahLbrQuoteEvent,
  decodeSarahLbrWorkRequestEvent,
  validateSarahLbrQuoteAgainstRequest,
} from "./index.ts";

const requesterPubkey = "11".repeat(32);
const providerPubkey = "22".repeat(32);
const requestEventId = "aa".repeat(32);
const eventId = "bb".repeat(32);
const sig = "33".repeat(64);

const baseWorkUnit = {
  workUnitRef: "workunit.public.community.fix_docs_1",
  grantRef: "grant.public.community.code_task.narrow_1",
  repositoryRefs: ["repo.public.openagents"],
  allowedActionRefs: ["action.public.repository.patch"],
  budgetMsats: 2_000_000,
  expiresAtUnix: 1_900_000_000,
  idempotencyRef: "idempotency.public.community.fix_docs_1",
} as const;

const baseRequestInput = {
  schema: SARAH_LBR_REQUEST_QUOTE_SCHEMA,
  workUnit: baseWorkUnit,
  objectiveRef: "objective.public.community.fix_docs",
  verificationCommandRef: "command.public.pnpm_test_docs",
  requiredCapabilityRefs: ["capability.public.local_codex"],
  groupId: "community-workroom-v1",
  groupRef: "group.public.community.workroom",
  forumTopicRef: "topic.public.forum.community_1",
  deadlineRef: "deadline.public.community.20260724",
  relays: ["wss://relay.openagents.com"],
  createdAt: 1_753_387_300,
} as const;

const eventFromDraft = (
  draft: Readonly<{
    kind: number;
    tags: ReadonlyArray<readonly string[]>;
    content: string;
  }>,
  overrides: Partial<{
    id: string;
    pubkey: string;
    content: string;
    tags: ReadonlyArray<readonly string[]>;
  }> = {},
) => ({
  id: overrides.id ?? eventId,
  pubkey: overrides.pubkey ?? requesterPubkey,
  created_at: 1_753_387_300,
  kind: draft.kind,
  tags: overrides.tags ?? draft.tags,
  content: overrides.content ?? draft.content,
  sig,
});

describe("SARAH-CW-04 NIP-LBR request and quote lane", () => {
  it("pins kinds to the shared NIP-LBR allocation", () => {
    expect(LBR_RESERVED_LABOR_KIND_MIN).toBe(5930);
    expect(LBR_RESERVED_LABOR_KIND_MAX).toBe(5939);
    expect(LBR_AGENTIC_CODING_REQUEST_KIND).toBe(5934);
    expect(LBR_FEEDBACK_KIND).toBe(7000);
    expect(SARAH_LBR_JOB_TYPE).toBe("code_task");
    expect(SARAH_LBR_SETTLEMENT_MODE_V1).toBe("no_spend");
    expect(SARAH_LBR_REQUEST_QUOTE_PACKET).toBe("SARAH-CW-04");
  });

  it("builds a ref-only budgeted work request with narrow grant tags", () => {
    const { request, draft, template } =
      buildSarahLbrWorkRequest(baseRequestInput);

    expect(request.kind).toBe(5934);
    expect(request.settlementMode).toBe("no_spend");
    expect(request.labor.outputDelivery).toBe(LBR_OUTPUT_DELIVERY_POLICY);
    expect(request.workUnit.budgetMsats).toBe(2_000_000);
    expect(request.workUnit.grantRef).toBe(
      "grant.public.community.code_task.narrow_1",
    );
    expect(draft.content).toBe("");
    expect(template.content).toBe("");
    expect(template.kind).toBe(5934);
    expect(
      draft.tags.some(
        (t) =>
          t[0] === "param" &&
          t[1] === "lbr_settlement_mode" &&
          t[2] === "no_spend",
      ),
    ).toBe(true);
    expect(
      draft.tags.some(
        (t) => t[0] === "expiration" && t[1] === "1900000000",
      ),
    ).toBe(true);
    expect(
      draft.tags.some((t) => t[0] === "h" && t[1] === "community-workroom-v1"),
    ).toBe(true);
  });

  it("round-trips request and quote events through decode", () => {
    const built = buildSarahLbrWorkRequest(baseRequestInput);
    const requestEvent = eventFromDraft(built.draft, {
      id: requestEventId,
      pubkey: requesterPubkey,
    });
    const parsedRequest = decodeSarahLbrWorkRequestEvent(requestEvent);

    expect(parsedRequest.objectiveRef).toBe(
      "objective.public.community.fix_docs",
    );
    expect(parsedRequest.workUnit.workUnitRef).toBe(
      "workunit.public.community.fix_docs_1",
    );
    expect(parsedRequest.workUnit.repositoryRefs).toEqual([
      "repo.public.openagents",
    ]);
    expect(parsedRequest.groupId).toBe("community-workroom-v1");
    expect(parsedRequest.settlementMode).toBe("no_spend");

    const quoteBuilt = buildSarahLbrQuote({
      schema: SARAH_LBR_REQUEST_QUOTE_SCHEMA,
      requestId: requestEventId,
      requesterPubkey,
      workUnitRef: baseWorkUnit.workUnitRef,
      amountMsats: 1_500_000,
      providerRef: "provider.public.member.codex_1",
      capabilityRefs: ["capability.public.local_codex"],
      quoteRef: "quote.public.community.1",
      operatorRef: "operator.public.member.dev_1",
      expiresAtRef: "expiry.public.community.quote_1",
      requestRelay: "wss://relay.openagents.com",
      createdAt: 1_753_387_400,
    });

    expect(quoteBuilt.draft.kind).toBe(7000);
    expect(quoteBuilt.quote.settlementMode).toBe("no_spend");

    const quoteEvent = eventFromDraft(quoteBuilt.draft, {
      pubkey: providerPubkey,
    });
    const parsedQuote = decodeSarahLbrQuoteEvent(quoteEvent);

    expect(parsedQuote.requestId).toBe(requestEventId);
    expect(parsedQuote.workUnitRef).toBe(baseWorkUnit.workUnitRef);
    expect(parsedQuote.amountMsats).toBe(1_500_000);
    expect(parsedQuote.operatorRef).toBe("operator.public.member.dev_1");

    validateSarahLbrQuoteAgainstRequest(parsedQuote, parsedRequest, {
      requestEventId,
      nowUnix: 1_753_387_400,
    });
  });

  it("refuses Sarah principal grants on work units", () => {
    expect(() =>
      buildSarahLbrWorkRequest({
        ...baseRequestInput,
        workUnit: {
          ...baseWorkUnit,
          grantRef: "grant.sarah.repository_delivery",
        },
      }),
    ).toThrow(SarahLbrLaneError);

    expect(() =>
      buildSarahLbrWorkRequest({
        ...baseRequestInput,
        workUnit: {
          ...baseWorkUnit,
          allowedActionRefs: ["capability.sarah.full_auto_control"],
        },
      }),
    ).toThrow(SarahLbrLaneError);
  });

  it("refuses unsafe material and private paths", () => {
    expect(() =>
      buildSarahLbrWorkRequest({
        ...baseRequestInput,
        objectiveRef: "objective.public./Users/alice/secret",
      }),
    ).toThrow(SarahLbrLaneError);

    const built = buildSarahLbrWorkRequest(baseRequestInput);
    expect(() =>
      decodeSarahLbrWorkRequestEvent(
        eventFromDraft(built.draft, { content: "raw prompt: fix this" }),
      ),
    ).toThrow(SarahLbrLaneError);
  });

  it("refuses paid settlement mode and over-budget quotes", () => {
    const built = buildSarahLbrWorkRequest(baseRequestInput);
    const paidTags = built.draft.tags.map((tag) =>
      tag[0] === "param" && tag[1] === "lbr_settlement_mode"
        ? (["param", "lbr_settlement_mode", "escrow_settle"] as const)
        : tag,
    );
    expect(() =>
      decodeSarahLbrWorkRequestEvent(
        eventFromDraft({ ...built.draft, tags: paidTags }),
      ),
    ).toThrow(SarahLbrLaneError);

    const request = built.request;
    const overBudget = buildSarahLbrQuote({
      schema: SARAH_LBR_REQUEST_QUOTE_SCHEMA,
      requestId: requestEventId,
      requesterPubkey,
      workUnitRef: baseWorkUnit.workUnitRef,
      amountMsats: 3_000_000,
      providerRef: "provider.public.member.codex_1",
      capabilityRefs: ["capability.public.local_codex"],
      quoteRef: "quote.public.community.over",
    });

    expect(() =>
      validateSarahLbrQuoteAgainstRequest(overBudget.quote, request, {
        requestEventId,
        nowUnix: 1_753_387_400,
      }),
    ).toThrow(/over_budget|exceeds request budget/);
  });

  it("refuses expired grants rather than extending them", () => {
    const built = buildSarahLbrWorkRequest(baseRequestInput);
    expect(() =>
      assertSarahLbrRequestNotExpired(built.request, 1_900_000_001),
    ).toThrow(SarahLbrLaneError);

    const quote = buildSarahLbrQuote({
      schema: SARAH_LBR_REQUEST_QUOTE_SCHEMA,
      requestId: requestEventId,
      requesterPubkey,
      workUnitRef: baseWorkUnit.workUnitRef,
      amountMsats: 1000,
      providerRef: "provider.public.member.codex_1",
      capabilityRefs: ["capability.public.local_codex"],
      quoteRef: "quote.public.community.late",
    });

    expect(() =>
      validateSarahLbrQuoteAgainstRequest(quote.quote, built.request, {
        requestEventId,
        nowUnix: 1_900_000_001,
      }),
    ).toThrow(/grant_expired|expired/);
  });

  it("refuses work-unit mismatch between quote and request", () => {
    const built = buildSarahLbrWorkRequest(baseRequestInput);
    const quote = buildSarahLbrQuote({
      schema: SARAH_LBR_REQUEST_QUOTE_SCHEMA,
      requestId: requestEventId,
      requesterPubkey,
      workUnitRef: "workunit.public.community.other_unit",
      amountMsats: 1000,
      providerRef: "provider.public.member.codex_1",
      capabilityRefs: ["capability.public.local_codex"],
      quoteRef: "quote.public.community.mismatch",
    });

    expect(() =>
      validateSarahLbrQuoteAgainstRequest(quote.quote, built.request, {
        requestEventId,
        nowUnix: 1_753_387_400,
      }),
    ).toThrow(SarahLbrLaneError);
    try {
      validateSarahLbrQuoteAgainstRequest(quote.quote, built.request, {
        requestEventId,
        nowUnix: 1_753_387_400,
      });
      expect.unreachable("expected work unit mismatch");
    } catch (error) {
      expect(error).toBeInstanceOf(SarahLbrLaneError);
      expect((error as SarahLbrLaneError).code).toBe("work_unit_mismatch");
    }
  });
});
