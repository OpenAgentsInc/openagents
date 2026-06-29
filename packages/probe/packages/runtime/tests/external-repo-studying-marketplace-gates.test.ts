import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyMarketplaceGates,
  decodeOpenAgentsExternalRepoStudyMarketplaceGates,
  openAgentsExternalRepoStudyMarketplaceGatesHash,
  type ExternalRepoStudyMarketplaceGateRequest,
} from "../src";

const generatedAt = "2026-06-29T00:00:00.000Z" as const;

const request: ExternalRepoStudyMarketplaceGateRequest = {
  contributorRef: "contributor.pylon.448ba824.v0",
  customerRef: "customer.examplecorp.v0",
  meteringPolicyRef: "policy.external_repo_study.metering.usage_unit.v0",
  packagePolicyRef: "policy.external_repo_study.package.refs_only.v0",
  packetRef: "external_repo_study_packet.examplecorp_widget.sha256abcdef",
  payoutPolicyRef: "policy.external_repo_study.payout.receipt_first.v0",
  pricingPolicyRef: "policy.external_repo_study.pricing.package_v0",
  repo: "ExampleCorp/widget-service",
  settlementPolicyRef: "policy.external_repo_study.settlement.no_spend_until_receipt.v0",
  termsAccepted: true,
  usageUnitRef: "usage_unit.external_repo_study.packet_access.v0",
};

describe("external repo studying marketplace gates", () => {
  test("passes complete economic refs while holding package, billing, payout, and settlement inert", async () => {
    const gates = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyMarketplaceGates({
        generatedAt,
        request,
      }),
    );

    expect(gates.schemaRef).toBe(
      "openagents.external_repo_study_marketplace_gates.v0",
    );
    expect(gates.state).toBe("economic_gates_ready_held");
    expect(gates.gate.state).toBe("inert_disabled");
    expect(gates.meteringPolicyPresent).toBe(true);
    expect(gates.usageUnitPresent).toBe(true);
    expect(gates.packagePolicyPresent).toBe(true);
    expect(gates.pricingPolicyPresent).toBe(true);
    expect(gates.payoutPolicyPresent).toBe(true);
    expect(gates.settlementPolicyPresent).toBe(true);
    expect(gates.termsAccepted).toBe(true);

    expect(gates.customerPublicClaimAllowed).toBe(false);
    expect(gates.marketplacePackageAllowed).toBe(false);
    expect(gates.packageListed).toBe(false);
    expect(gates.payoutEligible).toBe(false);
    expect(gates.effectsApplied).toBe(false);
    expect(gates.gate.effectsApplied).toBe(false);
    expect(gates.wouldAllowMarketplaceWhenArmed).toBe(false);
    expect(gates.sourceBoundary).toBe("customer_refs_withheld");
    expect(gates.marketplaceGatesHash).toBe(
      openAgentsExternalRepoStudyMarketplaceGatesHash(gates),
    );
    expect(gates.evidenceRefs).toEqual(
      expect.arrayContaining([
        request.packetRef,
        request.meteringPolicyRef,
        request.pricingPolicyRef,
        request.payoutPolicyRef,
        request.settlementPolicyRef,
        request.usageUnitRef,
      ]),
    );
    expect(`${gates.safeCopy} ${gates.unsafeCopyRefs.join(" ")}`).not.toMatch(
      /marketplace package live|payout eligible|settlement live/i,
    );
  });

  test("would allow marketplace only when flag and owner signoff are present, still inert", async () => {
    const gates = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyMarketplaceGates({
        generatedAt,
        marketplaceFlagArmed: true,
        ownerSignoffPresent: true,
        request,
      }),
    );

    expect(gates.gate.state).toBe("armed_ready");
    expect(gates.wouldAllowMarketplaceWhenArmed).toBe(true);
    expect(gates.marketplacePackageAllowed).toBe(false);
    expect(gates.packageListed).toBe(false);
    expect(gates.payoutEligible).toBe(false);
    expect(gates.effectsApplied).toBe(false);
  });

  test("armed marketplace blocks without owner signoff", async () => {
    const gates = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyMarketplaceGates({
        generatedAt,
        marketplaceFlagArmed: true,
        request,
      }),
    );

    expect(gates.gate.state).toBe("armed_blocked");
    expect(gates.gate.blockedReasonRefs).toContain(
      "marketplace.blocked.owner_signoff_missing",
    );
    expect(gates.wouldAllowMarketplaceWhenArmed).toBe(false);
  });

  test("blocks missing metering, pricing, payout, settlement, and terms refs", async () => {
    const gates = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyMarketplaceGates({
        generatedAt,
        request: {
          ...request,
          meteringPolicyRef: undefined,
          payoutPolicyRef: undefined,
          pricingPolicyRef: undefined,
          settlementPolicyRef: undefined,
          termsAccepted: false,
          usageUnitRef: undefined,
        },
      }),
    );

    expect(gates.state).toBe("blocked");
    expect(gates.blockerRefs).toEqual(
      expect.arrayContaining([
        "blocker.external_repo_study_marketplace.metering_missing",
        "blocker.external_repo_study_marketplace.pricing_missing",
        "blocker.external_repo_study_marketplace.payout_policy_missing",
        "blocker.external_repo_study_marketplace.settlement_policy_missing",
        "blocker.external_repo_study_marketplace.terms_not_accepted",
      ]),
    );
    expect(gates.marketplacePackageAllowed).toBe(false);
    expect(gates.payoutEligible).toBe(false);
  });

  test("rejects unsafe decoded projections that claim payout eligibility", async () => {
    const gates = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyMarketplaceGates({
        generatedAt,
        request,
      }),
    );

    await expect(
      Effect.runPromise(
        decodeOpenAgentsExternalRepoStudyMarketplaceGates({
          ...gates,
          payoutEligible: true,
        }),
      ),
    ).rejects.toMatchObject({
      path: "externalRepoStudyMarketplace",
    });
  });
});
