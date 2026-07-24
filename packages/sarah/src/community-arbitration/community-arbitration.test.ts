import { describe, expect, it } from "vite-plus/test";
import { readFileSync } from "node:fs";

import { AUTHORITY_DECISION_RECEIPT_SCHEMA } from "@openagentsinc/authority";

import {
  ARBITRATION_REASON_CLASSES,
  COMMUNITY_ARBITRATION_FEEDBACK_KIND,
  OWNER_APPEAL_IDENTITY_REGISTRY_PATH,
  OWNER_APPEAL_IDENTITY_BLOCKER_REF,
  SARAH_ARBITRATION_DECISION_SCHEMA,
  SARAH_CW_05_PACKET,
  SARAH_DISPUTE_APPEAL_SCHEMA,
  SARAH_OWNER_RULING_SCHEMA,
  ArbitrationRuleError,
  assertPayoutBlockedUntilDisputePath,
  buildArbitrationDecisionTemplate,
  buildDisputeAppealTemplate,
  buildOwnerAppealIdentityRotation,
  buildOwnerRulingTemplate,
  isOwnerAppealIdentityReady,
  missingOwnerAppealIdentity,
  resolveOwnerAppealIdentity,
  tagValue,
  validateArbitrationDecision,
  validateOwnerRuling,
  type OwnerAppealIdentity,
  type OwnerRuling,
  type SarahArbitrationDecision,
} from "./index.ts";

const hex = (nibble: string): string => nibble.repeat(64);
const fixtureNpub = `npub1${"q".repeat(58)}`;
const sarahPubkey = hex("a");
const providerPubkey = hex("b");
const producerAgent = hex("c");
const verifierAgent = hex("d");
const ownerPubkey = hex("e");
const appellantPubkey = hex("f");
const requestEventId = hex("1");
const resultEventId = hex("2");
const decisionEventId = hex("3");
const appealEventId = hex("4");

const acceptedDecision = (
  overrides: Partial<SarahArbitrationDecision> = {},
): SarahArbitrationDecision =>
  ({
    schema: SARAH_ARBITRATION_DECISION_SCHEMA,
    packet: SARAH_CW_05_PACKET,
    decisionRef: "decision.community.unit.1",
    requestEventId,
    resultEventId,
    unitRef: "unit.community.tick.1.a",
    providerPubkey,
    sarahPubkey,
    outcome: "accepted",
    authorityReceiptSchema: AUTHORITY_DECISION_RECEIPT_SCHEMA,
    authorityReceiptRef: "receipt.authority.community.1",
    independence: {
      producerOperatorRef: "operator.member.alice",
      producerAgentPubkey: producerAgent,
      verifierOperatorRef: "operator.member.bob",
      verifierAgentPubkey: verifierAgent,
      verificationReceiptRef: "receipt.verification.community.1",
    },
    evidenceRefs: ["evidence.public.artifact.1", "evidence.public.verify.1"],
    decidesPayment: false,
    decidedAt: "2026-07-24T20:00:00.000Z",
    ...overrides,
  }) as SarahArbitrationDecision;

const rejectedDecision = (): SarahArbitrationDecision =>
  acceptedDecision({
    outcome: "rejected",
    reasonClass: "verification_failed",
    reasonSummary: "Independent verifier reported failed criteria.",
    decisionRef: "decision.community.unit.2",
  });

const admittedOwnerIdentity = (): OwnerAppealIdentity => ({
  schema: "openagents.sarah.owner_appeal_identity.v1",
  packet: SARAH_CW_05_PACKET,
  lifecycle: "admitted",
  pubkey: ownerPubkey,
  npub: fixtureNpub,
  revision: 1,
  registeredAt: "2026-07-24T18:00:00.000Z",
  registeredByRef: "operator.owner.registration",
  registrationRef: "registration.owner_appeal.v1",
});

describe("SARAH-CW-05 community arbitration decisions", () => {
  it("builds a public kind-7000 accept template with authority receipt schema", () => {
    const { template, decision } = buildArbitrationDecisionTemplate({
      decision: acceptedDecision(),
      feedsSarahClaim: true,
      createdAt: 1_790_000_000,
    });

    expect(template.kind).toBe(COMMUNITY_ARBITRATION_FEEDBACK_KIND);
    expect(template.content).toBe("");
    expect(template.created_at).toBe(1_790_000_000);
    expect(tagValue(template.tags, "status")).toBe("accepted");
    expect(tagValue(template.tags, "cw_feedback_type")).toBe(
      "arbitration_decision",
    );
    expect(tagValue(template.tags, "cw_authority_receipt_schema")).toBe(
      AUTHORITY_DECISION_RECEIPT_SCHEMA,
    );
    expect(tagValue(template.tags, "cw_decides_payment")).toBe("false");
    expect(tagValue(template.tags, "cw_reason_class")).toBeUndefined();
    expect(decision.packet).toBe(SARAH_CW_05_PACKET);
  });

  it("requires a typed reason class on rejection and publishes it", () => {
    const { template } = buildArbitrationDecisionTemplate({
      decision: rejectedDecision(),
    });
    expect(tagValue(template.tags, "status")).toBe("rejected");
    expect(tagValue(template.tags, "cw_reason_class")).toBe(
      "verification_failed",
    );
    expect(tagValue(template.tags, "cw_reason_summary")).toContain(
      "Independent verifier",
    );
  });

  it("refuses a silent rejection", () => {
    expect(() =>
      validateArbitrationDecision(
        acceptedDecision({
          outcome: "rejected",
          reasonClass: undefined,
          reasonSummary: undefined,
        }),
      ),
    ).toThrow(ArbitrationRuleError);
    try {
      validateArbitrationDecision(
        acceptedDecision({
          outcome: "rejected",
        }),
      );
      expect.unreachable("expected throw");
    } catch (error) {
      expect(error).toBeInstanceOf(ArbitrationRuleError);
      expect((error as ArbitrationRuleError).code).toBe(
        "rejection_missing_reason_class",
      );
    }
  });

  it("refuses self-dealing when producer and verifier share an operator", () => {
    try {
      validateArbitrationDecision(
        acceptedDecision({
          independence: {
            producerOperatorRef: "operator.member.alice",
            producerAgentPubkey: producerAgent,
            verifierOperatorRef: "operator.member.alice",
            verifierAgentPubkey: verifierAgent,
            verificationReceiptRef: "receipt.verification.community.1",
          },
        }),
        { feedsSarahClaim: true },
      );
      expect.unreachable("expected throw");
    } catch (error) {
      expect((error as ArbitrationRuleError).code).toBe(
        "self_dealing_operators",
      );
    }
  });

  it("requires independence evidence when the unit feeds a Sarah claim", () => {
    try {
      validateArbitrationDecision(
        acceptedDecision({ independence: undefined }),
        { feedsSarahClaim: true },
      );
      expect.unreachable("expected throw");
    } catch (error) {
      expect((error as ArbitrationRuleError).code).toBe(
        "independence_required_for_sarah_claim",
      );
    }
  });

  it("exposes the full reason-class catalog for members", () => {
    expect(ARBITRATION_REASON_CLASSES).toContain("self_dealing_operators");
    expect(ARBITRATION_REASON_CLASSES).toContain("result_replay");
    expect(ARBITRATION_REASON_CLASSES).toContain("grant_expired");
    expect(ARBITRATION_REASON_CLASSES.length).toBeGreaterThanOrEqual(10);
  });
});

describe("SARAH-CW-05 dispute appeals and owner rulings", () => {
  it("builds a dispute-appeal template targeting the owner as arbiter", () => {
    const { template, appeal } = buildDisputeAppealTemplate({
      appeal: {
        schema: SARAH_DISPUTE_APPEAL_SCHEMA,
        packet: SARAH_CW_05_PACKET,
        appealRef: "appeal.community.unit.2.1",
        decisionRef: "decision.community.unit.2",
        decisionEventId,
        requestEventId,
        resultEventId,
        appellantOperatorRef: "operator.member.alice",
        appellantPubkey,
        grounds: "reason_disputed",
        groundsSummary: "Verifier applied the wrong acceptance criterion.",
        evidenceRefs: ["evidence.public.criterion.ref"],
        arbiterOfLastResort: "owner",
        filedAt: "2026-07-24T21:00:00.000Z",
      },
      createdAt: 1_790_000_100,
    });

    expect(template.kind).toBe(COMMUNITY_ARBITRATION_FEEDBACK_KIND);
    expect(template.content).toBe("");
    expect(tagValue(template.tags, "cw_feedback_type")).toBe("dispute_appeal");
    expect(tagValue(template.tags, "cw_arbiter")).toBe("owner");
    expect(tagValue(template.tags, "status")).toBe("appeal_open");
    expect(tagValue(template.tags, "cw_grounds")).toBe("reason_disputed");
    expect(appeal.arbiterOfLastResort).toBe("owner");
  });

  it("builds an owner-ruling template that Sarah cannot author", () => {
    const ruling: OwnerRuling = {
      schema: SARAH_OWNER_RULING_SCHEMA,
      packet: SARAH_CW_05_PACKET,
      rulingRef: "ruling.community.appeal.1",
      appealRef: "appeal.community.unit.2.1",
      appealEventId,
      decisionRef: "decision.community.unit.2",
      ownerAppealPubkey: ownerPubkey,
      outcome: "overturn_accept",
      reasonSummary: "Criterion tag was misapplied; accept the result.",
      evidenceRefs: ["evidence.public.owner.review.1"],
      authorRole: "owner_arbiter_of_last_resort",
      ruledAt: "2026-07-24T22:00:00.000Z",
    };

    const { template } = buildOwnerRulingTemplate({
      ruling,
      createdAt: 1_790_000_200,
    });
    expect(tagValue(template.tags, "cw_feedback_type")).toBe("owner_ruling");
    expect(tagValue(template.tags, "cw_author_role")).toBe(
      "owner_arbiter_of_last_resort",
    );
    expect(tagValue(template.tags, "status")).toBe("overturn_accept");

    const validated = validateOwnerRuling(ruling, {
      authorPubkey: ownerPubkey,
      appealIdentity: admittedOwnerIdentity(),
      sarahPubkey,
    });
    expect(validated.outcome).toBe("overturn_accept");

    try {
      validateOwnerRuling(ruling, {
        authorPubkey: sarahPubkey,
        appealIdentity: admittedOwnerIdentity(),
        sarahPubkey,
      });
      expect.unreachable("Sarah must not author owner rulings");
    } catch (error) {
      expect((error as ArbitrationRuleError).code).toBe(
        "sarah_authored_owner_ruling",
      );
    }
  });

  it("refuses owner rulings when the appeal identity is missing", () => {
    const ruling: OwnerRuling = {
      schema: SARAH_OWNER_RULING_SCHEMA,
      packet: SARAH_CW_05_PACKET,
      rulingRef: "ruling.community.appeal.1",
      appealRef: "appeal.community.unit.2.1",
      appealEventId,
      decisionRef: "decision.community.unit.2",
      ownerAppealPubkey: ownerPubkey,
      outcome: "uphold",
      reasonSummary: "Sarah decision stands.",
      evidenceRefs: [],
      authorRole: "owner_arbiter_of_last_resort",
      ruledAt: "2026-07-24T22:00:00.000Z",
    };

    try {
      validateOwnerRuling(ruling, {
        authorPubkey: ownerPubkey,
        appealIdentity: missingOwnerAppealIdentity(),
        sarahPubkey,
      });
      expect.unreachable("expected missing identity refusal");
    } catch (error) {
      expect((error as ArbitrationRuleError).code).toBe(
        "owner_appeal_identity_missing",
      );
    }
  });

  it("blocks payout while the dispute path is missing or an appeal is open", () => {
    expect(() =>
      assertPayoutBlockedUntilDisputePath({
        disputePathExists: false,
        openAppealRefs: [],
        attemptingPayout: true,
      }),
    ).toThrow(ArbitrationRuleError);

    expect(() =>
      assertPayoutBlockedUntilDisputePath({
        disputePathExists: true,
        openAppealRefs: ["appeal.community.unit.2.1"],
        attemptingPayout: true,
      }),
    ).toThrow(ArbitrationRuleError);

    expect(() =>
      assertPayoutBlockedUntilDisputePath({
        disputePathExists: true,
        openAppealRefs: [],
        attemptingPayout: true,
      }),
    ).not.toThrow();
  });
});

describe("SARAH-CW-05 owner appeal identity registry", () => {
  it("admits the registered Omega owner appeal identity", () => {
    const registration = JSON.parse(
      readFileSync(
        new URL(
          "../../../../docs/omega/owner-appeal-identity.json",
          import.meta.url,
        ),
        "utf8",
      ),
    );
    const resolution = resolveOwnerAppealIdentity({ registration });

    expect(isOwnerAppealIdentityReady(resolution)).toBe(true);
    if (resolution.lifecycle === "admitted") {
      expect(resolution.pubkey).toBe(
        "48c3bd00ce0ffdaa1b5b974ffea4bbac1c37a0ddfb164ac6bd1f5a3299bb21b3",
      );
      expect(resolution.npub).toBe(
        "npub1frpm6qxwpl765x6mja8laf9m4swr0gxalvty434aradr9xdmyxes2xa9vw",
      );
    }
  });

  it("resolves missing when no registration and no env pubkey", () => {
    const resolution = resolveOwnerAppealIdentity({});
    expect(resolution.lifecycle).toBe("missing");
    if (resolution.lifecycle === "missing") {
      expect(resolution.needsOwner).toBe(true);
      expect(resolution.blockerRef).toBe(OWNER_APPEAL_IDENTITY_BLOCKER_REF);
    }
    expect(isOwnerAppealIdentityReady(resolution)).toBe(false);
    expect(OWNER_APPEAL_IDENTITY_REGISTRY_PATH).toBe(
      "docs/omega/owner-appeal-identity.json",
    );
  });

  it("admits a registration payload and supports rotation", () => {
    const current = admittedOwnerIdentity();
    const resolved = resolveOwnerAppealIdentity({ registration: current });
    expect(isOwnerAppealIdentityReady(resolved)).toBe(true);

    const nextPubkey = hex("9");
    const rotated = buildOwnerAppealIdentityRotation({
      current,
      nextPubkey,
      registeredByRef: "operator.owner.rotation",
      registrationRef: "registration.owner_appeal.v2",
      registeredAt: "2026-07-25T00:00:00.000Z",
    });
    expect(rotated.revision).toBe(2);
    expect(rotated.pubkey).toBe(nextPubkey);
    expect(rotated.supersedesRef).toBe(current.registrationRef);
  });

  it("admits env public key without requiring a committed file", () => {
    const resolution = resolveOwnerAppealIdentity({
      envPubkey: ownerPubkey,
      envNpub: fixtureNpub,
    });
    expect(resolution.lifecycle).toBe("admitted");
    if (resolution.lifecycle === "admitted") {
      expect(resolution.pubkey).toBe(ownerPubkey);
    }
  });
});
