import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
  buildOpenAgentsExternalRepoStudyReviewPolicyBinding,
  EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
  openAgentsExternalRepoStudyReviewPolicyBindingHash,
  type ExternalRepoStudyPrivacyReviewRequest,
} from "../src";

const generatedAt = "2026-06-20T00:00:00.000Z" as const;

const SECRET_STRINGS = [
  "gold_answer",
  "rubric_claim",
  "evidence_excerpt",
  "raw customer data",
];

type ReviewRequestSansPolicyRefs = Omit<
  ExternalRepoStudyPrivacyReviewRequest,
  "dataProcessingAgreementRef" | "retentionPolicyRef"
>;

const baseReviewRequest: ReviewRequestSansPolicyRefs = {
  customerRef: "customer.acme.v0",
  repo: "ExampleCorp/widget-service",
  retentionDays: 90,
  customerAuthorizationRef: "auth.acme.widget.v0",
  declaredPiiCategories: ["none"],
  reviewerRef: "reviewer.privacy.casey.v0",
};

const registry = () =>
  buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry({ generatedAt });

describe("external repo studying review<->policy binding", () => {
  test("binds DPA + retention refs from a known published policy, held inert", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        return yield* buildOpenAgentsExternalRepoStudyReviewPolicyBinding({
          generatedAt,
          policyRegistry: reg,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          reviewRequest: baseReviewRequest,
        });
      }),
    );

    expect(binding.schemaRef).toBe(
      "openagents.external_repo_study_review_policy_binding.v0",
    );
    expect(binding.state).toBe("bound_held");
    expect(binding.bound).toBe(true);
    expect(binding.policyPublished).toBe(true);
    expect(binding.dataProcessingAgreementRef).toBe(
      EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
    );
    expect(binding.retentionPolicyRef).toBe(
      EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
    );
    expect(binding.policyTermsDigest?.startsWith("sha256:")).toBe(true);

    // The nested review preflight sees the derived refs as present and would
    // clear when armed.
    expect(binding.reviewPreflight.dataProcessingAgreementPresent).toBe(true);
    expect(binding.reviewPreflight.retentionPolicyPresent).toBe(true);
    expect(binding.reviewPreflight.state).toBe("review_ready_held");

    // Inert by construction.
    expect(binding.reviewCleared).toBe(false);
    expect(binding.effectsApplied).toBe(false);
    expect(binding.customerPublicClaimAllowed).toBe(false);
    expect(binding.marketplacePackageAllowed).toBe(false);
    expect(binding.payoutEligible).toBe(false);
    expect(binding.reviewPreflight.reviewCleared).toBe(false);
    expect(binding.bindingHash).toBe(
      openAgentsExternalRepoStudyReviewPolicyBindingHash(binding),
    );
  });

  test("a forged / unknown policy ref binds no refs and blocks the review", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        return yield* buildOpenAgentsExternalRepoStudyReviewPolicyBinding({
          generatedAt,
          policyRegistry: reg,
          policyRef: "policy.external_repo_study_privacy.forged",
          reviewRequest: baseReviewRequest,
        });
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.bound).toBe(false);
    expect(binding.policyPublished).toBe(false);
    expect(binding.dataProcessingAgreementRef).toBeNull();
    expect(binding.retentionPolicyRef).toBeNull();
    expect(binding.policyTermsDigest).toBeNull();
    expect(binding.mismatchRefs).toContain(
      "blocker.external_repo_study_review_policy_binding.policy_ref_not_published",
    );

    // The review cannot present DPA / retention and is blocked.
    expect(binding.reviewPreflight.dataProcessingAgreementPresent).toBe(false);
    expect(binding.reviewPreflight.retentionPolicyPresent).toBe(false);
    expect(binding.reviewPreflight.state).toBe("blocked");
    expect(binding.reviewPreflight.privacyReviewRef).toBeNull();
  });

  test("an empty policy ref binds no refs (unbound)", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        return yield* buildOpenAgentsExternalRepoStudyReviewPolicyBinding({
          generatedAt,
          policyRegistry: reg,
          policyRef: "",
          reviewRequest: baseReviewRequest,
        });
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.dataProcessingAgreementRef).toBeNull();
    expect(binding.retentionPolicyRef).toBeNull();
  });

  test("retention out of policy still blocks even with a published policy", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        return yield* buildOpenAgentsExternalRepoStudyReviewPolicyBinding({
          generatedAt,
          policyRegistry: reg,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          reviewRequest: { ...baseReviewRequest, retentionDays: 9999 },
        });
      }),
    );

    // The policy ref is bound, but the review preflight still enforces the cap.
    expect(binding.bound).toBe(true);
    expect(binding.reviewPreflight.retentionWithinCap).toBe(false);
    expect(binding.reviewPreflight.state).toBe("blocked");
    expect(binding.wouldClearWhenArmed).toBe(false);
  });

  test("armed + reviewer sign-off is would-clear-when-armed but still inert", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        return yield* buildOpenAgentsExternalRepoStudyReviewPolicyBinding({
          generatedAt,
          policyRegistry: reg,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          reviewRequest: baseReviewRequest,
          reviewFlagArmed: true,
          reviewerSignoffPresent: true,
        });
      }),
    );

    expect(binding.wouldClearWhenArmed).toBe(true);
    expect(binding.reviewPreflight.clearanceGate.state).toBe("armed_ready");
    // Still inert even when armed.
    expect(binding.reviewCleared).toBe(false);
    expect(binding.effectsApplied).toBe(false);
    expect(binding.reviewPreflight.reviewCleared).toBe(false);
    expect(binding.reviewPreflight.effectsApplied).toBe(false);
  });

  test("rejects the OpenAgents repo as a binding target", async () => {
    const program = Effect.gen(function* () {
      const reg = yield* registry();
      return yield* buildOpenAgentsExternalRepoStudyReviewPolicyBinding({
        generatedAt,
        policyRegistry: reg,
        policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
        reviewRequest: {
          ...baseReviewRequest,
          repo: "OpenAgentsInc/openagents",
        },
      });
    });

    await expect(Effect.runPromise(program)).rejects.toThrow();
  });

  test("never leaks private content into the public binding projection", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        return yield* buildOpenAgentsExternalRepoStudyReviewPolicyBinding({
          generatedAt,
          policyRegistry: reg,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          reviewRequest: baseReviewRequest,
        });
      }),
    );
    const serialized = JSON.stringify(binding);
    for (const secret of SECRET_STRINGS) {
      expect(serialized).not.toContain(secret);
    }
  });
});
