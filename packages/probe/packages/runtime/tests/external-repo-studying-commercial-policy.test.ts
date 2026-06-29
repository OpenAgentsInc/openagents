import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyCommercialPolicyPreflight,
  openAgentsExternalRepoStudyCommercialPolicyHash,
} from "../src";

const completeRequest = {
  customerRef: "customer.example.repo_study.v0",
  entitlementPolicyRef: "policy.repo_study.entitlement.v0",
  meteringPolicyRef: "policy.repo_study.metering.v0",
  packagePolicyRef: "policy.repo_study.package_review.v0",
  payoutPolicyRef: "policy.repo_study.payout_eligibility.v0",
  pricingPolicyRef: "policy.repo_study.pricing.v0",
  refundDisputePolicyRef: "policy.repo_study.refund_dispute.v0",
  repo: "ExampleCorp/widget-service",
  reviewerRef: "reviewer.commercial.repo_study.v0",
  settlementGateRef: "gate.repo_study.settlement_receipts.v0",
  studyPacketRef: "external_repo_study_packet.examplecorp_widget_service.abc123",
  usageSubjectRef: "usage_subject.repo_study.examplecorp_widget_service.v0",
  validationRef: "customer_private_validation.examplecorp_widget_service.abc123",
} as const;

describe("external repo studying commercial policy", () => {
  test("requires metering, pricing, package, payout, settlement, and dispute refs before paid-package readiness", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyCommercialPolicyPreflight({
        commercialPolicyFlagArmed: true,
        generatedAt: "2026-06-29T00:00:00.000Z",
        ownerSignoffPresent: true,
        request: completeRequest,
      }),
    );

    expect(preflight).toMatchObject({
      commercialGate: {
        effectsApplied: false,
        ownerSignoffPresent: true,
        state: "armed_ready",
      },
      customerPublicClaimAllowed: false,
      effectsApplied: false,
      marketplacePackageAllowed: false,
      payoutEligible: false,
      settlementReady: false,
      sourceBoundary: "public_refs_only",
      state: "policy_ready_held",
      wouldAllowPaidPackageWhenArmed: true,
    });
    expect(preflight.blockerRefs).toEqual([]);
    expect(preflight.policyHash).toBe(
      openAgentsExternalRepoStudyCommercialPolicyHash(preflight),
    );
    expect(preflight.evidenceRefs).toEqual(
      expect.arrayContaining([
        completeRequest.usageSubjectRef,
        completeRequest.meteringPolicyRef,
        completeRequest.packagePolicyRef,
        completeRequest.pricingPolicyRef,
        completeRequest.payoutPolicyRef,
        completeRequest.settlementGateRef,
      ]),
    );
    expect(preflight.safeCopy).not.toMatch(
      /paid package is live|marketplace listing is live|money moved/i,
    );
  });

  test("keeps the policy held inert when refs are complete but the flag is disabled", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyCommercialPolicyPreflight({
        request: completeRequest,
      }),
    );

    expect(preflight.state).toBe("policy_ready_held");
    expect(preflight.commercialGate.state).toBe("inert_disabled");
    expect(preflight.commercialGate.blockedReasonRefs).toContain(
      "blocker.external_repo_study_commercial_policy.flag_disabled",
    );
    expect(preflight.wouldAllowPaidPackageWhenArmed).toBe(false);
    expect(preflight.marketplacePackageAllowed).toBe(false);
    expect(preflight.payoutEligible).toBe(false);
    expect(preflight.settlementReady).toBe(false);
  });

  test("blocks paid-package readiness when pricing or settlement refs are missing", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyCommercialPolicyPreflight({
        commercialPolicyFlagArmed: true,
        ownerSignoffPresent: true,
        request: {
          ...completeRequest,
          pricingPolicyRef: undefined,
          settlementGateRef: undefined,
        },
      }),
    );

    expect(preflight.state).toBe("blocked");
    expect(preflight.commercialGate.state).toBe("armed_blocked");
    expect(preflight.wouldAllowPaidPackageWhenArmed).toBe(false);
    expect(preflight.blockerRefs).toEqual(
      expect.arrayContaining([
        "blocker.external_repo_study_commercial_policy.pricing_policy_missing",
        "blocker.external_repo_study_commercial_policy.settlement_gate_missing",
      ]),
    );
  });

  test("rejects forged claim widening during decode", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyCommercialPolicyPreflight({
        request: completeRequest,
      }),
    );

    await expect(
      Effect.runPromise(
        buildOpenAgentsExternalRepoStudyCommercialPolicyPreflight({
          request: {
            ...completeRequest,
            studyPacketRef: "",
          },
        }),
      ),
    ).rejects.toMatchObject({
      path: "externalRepoStudyCommercialPolicy.studyPacketRef",
      reason: "must be non-empty",
    });

    expect(preflight.marketplacePackageAllowed).toBe(false);
    expect(preflight.payoutEligible).toBe(false);
  });
});
